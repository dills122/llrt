use llrt_utils::option::Undefined;
use rquickjs::{
    class::Trace,
    prelude::{Opt, This},
    Class, Ctx, Exception, JsLifetime, Object, Promise, Result, Value,
};

use crate::{
    queuing_strategy::QueuingStrategy,
    readable::{
        readable_stream_default_controller_error_stream,
        stream::{
            algorithms::{CancelAlgorithm, PullAlgorithm, StartAlgorithm},
            ReadableStream, ReadableStreamState,
        },
        ReadableStreamControllerClass, ReadableStreamDefaultControllerClass,
    },
    utils::promise::ResolveablePromise,
    writable::{WritableStream, WritableStreamState},
};

use super::{
    controller::{
        self, CancelAlgorithm as TsCancelAlgorithm, FlushAlgorithm, TransformAlgorithm,
        TransformStreamDefaultController, TransformStreamDefaultControllerClass,
    },
    transformer::Transformer,
};

#[rquickjs::class]
#[derive(JsLifetime, Trace)]
pub(crate) struct TransformStream<'js> {
    pub(super) readable: Option<Class<'js, ReadableStream<'js>>>,
    pub(super) writable: Option<Class<'js, WritableStream<'js>>>,
    pub(super) controller: Option<TransformStreamDefaultControllerClass<'js>>,
    pub(super) backpressure: bool,
    pub(super) backpressure_change_promise: Option<ResolveablePromise<'js>>,
}

pub(crate) type TransformStreamClass<'js> = Class<'js, TransformStream<'js>>;

#[rquickjs::methods(rename_all = "camelCase")]
impl<'js> TransformStream<'js> {
    pub(crate) fn from_transformer(
        ctx: Ctx<'js>,
        transformer: Object<'js>,
    ) -> Result<Class<'js, Self>> {
        Self::new(
            ctx,
            Opt(Some(Undefined(Some(transformer)))),
            Opt(None),
            Opt(None),
        )
    }

    #[qjs(constructor)]
    fn new(
        ctx: Ctx<'js>,
        transformer: Opt<Undefined<Object<'js>>>,
        writable_strategy: Opt<Undefined<QueuingStrategy<'js>>>,
        readable_strategy: Opt<Undefined<QueuingStrategy<'js>>>,
    ) -> Result<Class<'js, Self>> {
        let transformer_obj = transformer.0.and_then(|u| u.0);
        let transformer_dict = transformer_obj
            .as_ref()
            .map(|obj| Transformer::from_object(obj.clone()))
            .transpose()?
            .unwrap_or_default();

        if transformer_dict.readable_type {
            return Err(Exception::throw_range(
                &ctx,
                "readableType is not supported",
            ));
        }
        if transformer_dict.writable_type {
            return Err(Exception::throw_range(
                &ctx,
                "writableType is not supported",
            ));
        }

        let readable_strategy = readable_strategy.0.and_then(|qs| qs.0);
        let writable_strategy = writable_strategy.0.and_then(|qs| qs.0);

        let readable_size = QueuingStrategy::extract_size_algorithm(readable_strategy.as_ref());
        let writable_size = QueuingStrategy::extract_size_algorithm(writable_strategy.as_ref());
        let readable_hwm = QueuingStrategy::extract_high_water_mark(&ctx, readable_strategy, 0.0)?;
        let writable_hwm = QueuingStrategy::extract_high_water_mark(&ctx, writable_strategy, 1.0)?;

        // Create the TransformStream instance
        let stream_class = Class::instance(
            ctx.clone(),
            Self {
                readable: None,
                writable: None,
                controller: None,
                backpressure: true,
                backpressure_change_promise: None,
            },
        )?;

        // Initial backpressure change promise
        let bp_promise = ResolveablePromise::new(&ctx)?;
        stream_class.borrow_mut().backpressure_change_promise = Some(bp_promise);

        // Build controller algorithms
        let transform_algorithm = transformer_dict
            .transform
            .map(|f| TransformAlgorithm::Function {
                f,
                transformer: transformer_obj.clone(),
            })
            .unwrap_or(TransformAlgorithm::Identity);

        let flush_algorithm = transformer_dict
            .flush
            .map(|f| FlushAlgorithm::Function {
                f,
                transformer: transformer_obj.clone(),
            })
            .unwrap_or(FlushAlgorithm::Noop);

        let cancel_algorithm = transformer_dict
            .cancel
            .map(|f| TsCancelAlgorithm::Function {
                f,
                transformer: transformer_obj.clone(),
            })
            .unwrap_or(TsCancelAlgorithm::Noop);

        // Create controller
        let controller_class = Class::instance(
            ctx.clone(),
            TransformStreamDefaultController {
                stream: stream_class.clone(),
                transform_algorithm: Some(transform_algorithm),
                flush_algorithm: Some(flush_algorithm),
                cancel_algorithm: Some(cancel_algorithm),
                finish_promise: None,
            },
        )?;
        stream_class.borrow_mut().controller = Some(controller_class.clone());

        // Start promise
        let start_promise = ResolveablePromise::new(&ctx)?;

        // --- Create writable side with properly traced algorithm variants ---
        let writable_class = WritableStream::create_for_transform(
            ctx.clone(),
            start_promise.promise.clone(),
            stream_class.clone(),
            controller_class.clone(),
            writable_hwm,
            writable_size,
        )?;

        // --- Create readable side ---
        let pull_algorithm = PullAlgorithm::Transform(stream_class.clone());

        let cancel_algo = CancelAlgorithm::Transform {
            stream: stream_class.clone(),
            controller: controller_class.clone(),
        };

        let readable_objects = ReadableStream::create_readable_stream(
            ctx.clone(),
            StartAlgorithm::ReturnUndefined,
            pull_algorithm,
            cancel_algo,
            Some(readable_hwm),
            Some(readable_size),
        )?;

        {
            let mut stream = stream_class.borrow_mut();
            stream.readable = Some(readable_objects.stream.clone());
            stream.writable = Some(writable_class);
        }

        // Invoke start() if present
        if let Some(start_fn) = transformer_dict.start {
            match start_fn.call::<_, Value>((This(transformer_obj), controller_class)) {
                Ok(val) => {
                    start_promise.resolve(val)?;
                },
                Err(_) => {
                    let err = ctx.catch();
                    start_promise.reject(err)?;
                },
            }
        } else {
            start_promise.resolve_undefined()?;
        }

        Ok(stream_class)
    }

    #[qjs(get)]
    fn readable(&self) -> Option<Class<'js, ReadableStream<'js>>> {
        self.readable.clone()
    }

    #[qjs(get)]
    fn writable(&self) -> Option<Class<'js, WritableStream<'js>>> {
        self.writable.clone()
    }
}

impl<'js> TransformStream<'js> {
    // Return owned handles so callers can release the TransformStream borrow
    // before invoking operations that may run JavaScript.
    pub(super) fn readable_default_controller(
        &self,
    ) -> Option<ReadableStreamDefaultControllerClass<'js>> {
        self.readable.as_ref().and_then(|readable| {
            let readable = readable.borrow();
            match &readable.controller {
                ReadableStreamControllerClass::ReadableStreamDefaultController(controller) => {
                    Some(controller.clone())
                },
                _ => None,
            }
        })
    }

    pub(super) fn readable_stored_error(&self) -> Option<Value<'js>> {
        self.readable.as_ref().and_then(|readable| {
            let readable = readable.borrow();
            match &readable.state {
                ReadableStreamState::Errored(error) => Some(error.clone()),
                _ => None,
            }
        })
    }

    fn writable_stored_error(&self) -> Option<Value<'js>> {
        self.writable
            .as_ref()
            .and_then(|writable| writable.borrow().stored_error())
    }

    // Unlike writable_stored_error(), the finalization algorithms must only
    // prefer the stored error after the writable has reached `Errored`.
    fn writable_stored_error_if_errored(&self) -> Option<Value<'js>> {
        self.writable.as_ref().and_then(|writable| {
            let writable = writable.borrow();
            match &writable.state {
                WritableStreamState::Errored(error) => Some(error.clone()),
                _ => None,
            }
        })
    }
}

// --- Sink algorithms ---

pub(crate) fn sink_write_algorithm<'js>(
    ctx: Ctx<'js>,
    stream_class: &TransformStreamClass<'js>,
    controller_class: &TransformStreamDefaultControllerClass<'js>,
    chunk: Value<'js>,
) -> Result<Promise<'js>> {
    let stream = stream_class.borrow();
    if stream.backpressure {
        let backpressure_promise = stream
            .backpressure_change_promise
            .as_ref()
            .map(|p| p.promise.clone());
        drop(stream);

        if let Some(backpressure_promise) = backpressure_promise {
            let stream_class = stream_class.clone();
            let controller_class = controller_class.clone();
            return crate::utils::promise::upon_promise::<Value<'js>, _>(
                ctx.clone(),
                backpressure_promise,
                move |ctx, _| {
                    let stored_error = stream_class.borrow().writable_stored_error();
                    if let Some(stored_error) = stored_error {
                        return Err(ctx.throw(stored_error));
                    }
                    let transform_promise =
                        controller::transform_stream_default_controller_perform_transform(
                            ctx.clone(),
                            &stream_class,
                            &controller_class,
                            chunk,
                        )?;
                    Ok(transform_promise.into_value())
                },
            );
        }
    } else {
        drop(stream);
    }

    controller::transform_stream_default_controller_perform_transform(
        ctx,
        stream_class,
        controller_class,
        chunk,
    )
}

pub(crate) fn sink_close_algorithm<'js>(
    ctx: Ctx<'js>,
    stream_class: &TransformStreamClass<'js>,
    controller_class: &TransformStreamDefaultControllerClass<'js>,
) -> Result<Promise<'js>> {
    if let Some(finish_promise) = existing_finish_promise(controller_class) {
        return Ok(finish_promise);
    }

    let (finish, finish_promise) = start_finish(&ctx, controller_class)?;

    let flush_promise = controller::perform_flush(ctx.clone(), controller_class)?;
    controller_class.borrow_mut().clear_algorithms();

    let stream_class = stream_class.clone();
    let _ = crate::utils::promise::upon_promise::<Value<'js>, _>(
        ctx.clone(),
        flush_promise,
        move |ctx, result| {
            match result {
                Ok(_) => {
                    let stored_error = stream_class.borrow().readable_stored_error();
                    if let Some(error) = stored_error {
                        finish.reject(error)?;
                        return Ok(());
                    }

                    let mut stream = stream_class.borrow_mut();
                    // Resolve any pending backpressure promise to break the cycle
                    if let Some(backpressure_promise) = &stream.backpressure_change_promise {
                        backpressure_promise.resolve_undefined()?;
                    }
                    stream.backpressure_change_promise = None;
                    drop(stream);

                    let readable_controller =
                        require_readable_default_controller(&ctx, &stream_class)?;
                    crate::readable::readable_stream_default_controller_close_stream(
                        ctx.clone(),
                        readable_controller,
                    )?;
                    finish.resolve_undefined()?;
                },
                Err(reason) => {
                    let stored_error = stream_class.borrow().readable_stored_error();
                    if let Some(error) = stored_error {
                        finish.reject(error)?;
                    } else {
                        error_readable(&ctx, &stream_class, reason.clone())?;
                        finish.reject(reason)?;
                    }
                },
            }

            Ok(())
        },
    )?;

    Ok(finish_promise)
}

pub(crate) fn sink_abort_algorithm<'js>(
    ctx: Ctx<'js>,
    stream_class: &TransformStreamClass<'js>,
    controller_class: &TransformStreamDefaultControllerClass<'js>,
    reason: Value<'js>,
) -> Result<Promise<'js>> {
    if let Some(finish_promise) = existing_finish_promise(controller_class) {
        return Ok(finish_promise);
    }

    let (finish, finish_promise) = start_finish(&ctx, controller_class)?;

    let cancel_promise = controller::perform_cancel(ctx.clone(), controller_class, reason.clone())?;
    controller_class.borrow_mut().clear_algorithms();

    let stream_class = stream_class.clone();
    let _ = crate::utils::promise::upon_promise::<Value<'js>, _>(
        ctx.clone(),
        cancel_promise,
        move |ctx, result| {
            match result {
                Ok(_) => {
                    let stored_error = stream_class.borrow().readable_stored_error();
                    if let Some(error) = stored_error {
                        finish.reject(error)?;
                    } else {
                        error_readable(&ctx, &stream_class, reason)?;
                        finish.resolve_undefined()?;
                    }
                },
                Err(error) => {
                    let stored_error = stream_class.borrow().readable_stored_error();
                    if let Some(stored_error) = stored_error {
                        finish.reject(stored_error)?;
                    } else {
                        error_readable(&ctx, &stream_class, error.clone())?;
                        finish.reject(error)?;
                    }
                },
            }

            Ok(())
        },
    )?;

    Ok(finish_promise)
}

// --- Source algorithms ---

pub(crate) fn source_pull_algorithm<'js>(
    ctx: Ctx<'js>,
    stream_class: &TransformStreamClass<'js>,
) -> Result<Promise<'js>> {
    controller::transform_stream_set_backpressure(&ctx, stream_class, false)
}

pub(crate) fn source_cancel_algorithm<'js>(
    ctx: Ctx<'js>,
    stream_class: &TransformStreamClass<'js>,
    controller_class: &TransformStreamDefaultControllerClass<'js>,
    reason: Value<'js>,
) -> Result<Promise<'js>> {
    if let Some(finish_promise) = existing_finish_promise(controller_class) {
        return Ok(finish_promise);
    }

    let (finish, finish_promise) = start_finish(&ctx, controller_class)?;

    let cancel_promise = controller::perform_cancel(ctx.clone(), controller_class, reason.clone())?;
    controller_class.borrow_mut().clear_algorithms();

    let stream_class = stream_class.clone();
    let _ = crate::utils::promise::upon_promise::<Value<'js>, _>(
        ctx.clone(),
        cancel_promise,
        move |ctx, result| {
            match result {
                Ok(_) => {
                    let stored_error = stream_class.borrow().writable_stored_error_if_errored();
                    if let Some(error) = stored_error {
                        finish.reject(error)?;
                        return Ok(());
                    }

                    controller::transform_stream_error_writable_and_unblock_write(
                        ctx.clone(),
                        &stream_class,
                        reason,
                    )?;
                    finish.resolve_undefined()?;
                },
                Err(error) => {
                    let stored_error = stream_class.borrow().writable_stored_error_if_errored();
                    if let Some(stored_error) = stored_error {
                        finish.reject(stored_error)?;
                    } else {
                        controller::transform_stream_error_writable_and_unblock_write(
                            ctx.clone(),
                            &stream_class,
                            error.clone(),
                        )?;
                        finish.reject(error)?;
                    }
                },
            }

            Ok(())
        },
    )?;

    Ok(finish_promise)
}

fn existing_finish_promise<'js>(
    controller_class: &TransformStreamDefaultControllerClass<'js>,
) -> Option<Promise<'js>> {
    controller_class
        .borrow()
        .finish_promise
        .as_ref()
        .map(|finish| finish.promise.clone())
}

fn start_finish<'js>(
    ctx: &Ctx<'js>,
    controller_class: &TransformStreamDefaultControllerClass<'js>,
) -> Result<(ResolveablePromise<'js>, Promise<'js>)> {
    let finish = ResolveablePromise::new(ctx)?;
    let finish_promise = finish.promise.clone();

    // This must be stored before flush() or cancel() invokes user code so a
    // reentrant close, abort, or cancel returns the same promise.
    controller_class.borrow_mut().finish_promise = Some(finish.clone());

    Ok((finish, finish_promise))
}

fn require_readable_default_controller<'js>(
    ctx: &Ctx<'js>,
    stream_class: &TransformStreamClass<'js>,
) -> Result<ReadableStreamDefaultControllerClass<'js>> {
    stream_class
        .borrow()
        .readable_default_controller()
        .ok_or_else(|| Exception::throw_type(ctx, "readable controller not available"))
}

fn error_readable<'js>(
    ctx: &Ctx<'js>,
    stream_class: &TransformStreamClass<'js>,
    error: Value<'js>,
) -> Result<()> {
    let readable_controller = require_readable_default_controller(ctx, stream_class)?;

    readable_stream_default_controller_error_stream(readable_controller, error)
}
