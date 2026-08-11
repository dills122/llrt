use rquickjs::{
    class::{OwnedBorrowMut, Trace},
    prelude::{Opt, This},
    Class, Ctx, Error, Exception, Function, JsLifetime, Object, Promise, Result, Value,
};

use crate::{
    readable::{
        readable_stream_default_controller_close_stream,
        readable_stream_default_controller_enqueue_value,
        readable_stream_default_controller_error_stream,
    },
    utils::promise::{promise_resolved_with, upon_promise, ResolveablePromise},
    writable::writable_stream_default_controller_error_if_needed,
};

use llrt_utils::primordials::{BasePrimordials, Primordial};

use super::stream::TransformStreamClass;

#[rquickjs::class]
#[derive(JsLifetime, Trace)]
pub(crate) struct TransformStreamDefaultController<'js> {
    pub(super) stream: TransformStreamClass<'js>,
    pub(super) transform_algorithm: Option<TransformAlgorithm<'js>>,
    pub(super) flush_algorithm: Option<FlushAlgorithm<'js>>,
    pub(super) cancel_algorithm: Option<CancelAlgorithm<'js>>,
    pub(super) finish_promise: Option<ResolveablePromise<'js>>,
}

pub(crate) type TransformStreamDefaultControllerClass<'js> =
    Class<'js, TransformStreamDefaultController<'js>>;

#[rquickjs::methods(rename_all = "camelCase")]
impl<'js> TransformStreamDefaultController<'js> {
    #[qjs(constructor)]
    fn new(ctx: Ctx<'js>) -> Result<Class<'js, Self>> {
        Err(Exception::throw_type(&ctx, "Illegal constructor"))
    }

    #[qjs(get)]
    fn desired_size(&self) -> Option<f64> {
        let stream = self.stream.borrow();
        let readable_class = stream.readable.as_ref()?;
        let readable = readable_class.borrow();
        match &readable.controller {
            crate::readable::ReadableStreamControllerClass::ReadableStreamDefaultController(c) => {
                let c = c.borrow();
                c.readable_stream_default_controller_get_desired_size(&readable)
                    .0
            },
            _ => None,
        }
    }

    fn enqueue(
        ctx: Ctx<'js>,
        this: This<OwnedBorrowMut<'js, Self>>,
        chunk: Opt<Value<'js>>,
    ) -> Result<()> {
        let chunk = chunk.0.unwrap_or_else(|| Value::new_undefined(ctx.clone()));
        let stream_class = this.stream.clone();
        drop(this);
        transform_stream_default_controller_enqueue(ctx, &stream_class, chunk)
    }

    fn error(
        ctx: Ctx<'js>,
        this: This<OwnedBorrowMut<'js, Self>>,
        reason: Opt<Value<'js>>,
    ) -> Result<()> {
        let reason = reason
            .0
            .unwrap_or_else(|| Value::new_undefined(ctx.clone()));
        let stream_class = this.stream.clone();
        drop(this);
        transform_stream_error(ctx, &stream_class, reason)
    }

    fn terminate(ctx: Ctx<'js>, this: This<OwnedBorrowMut<'js, Self>>) -> Result<()> {
        let stream_class = this.stream.clone();
        drop(this);
        transform_stream_default_controller_terminate(ctx, &stream_class)
    }
}

impl<'js> TransformStreamDefaultController<'js> {
    pub(super) fn clear_algorithms(&mut self) {
        self.transform_algorithm = None;
        self.flush_algorithm = None;
        self.cancel_algorithm = None;
    }
}

#[derive(Trace, JsLifetime, Clone)]
pub(super) enum TransformAlgorithm<'js> {
    Identity,
    Function {
        f: Function<'js>,
        transformer: Option<Object<'js>>,
    },
}

#[derive(Trace, JsLifetime, Clone)]
pub(super) enum FlushAlgorithm<'js> {
    Noop,
    Function {
        f: Function<'js>,
        transformer: Option<Object<'js>>,
    },
}

#[derive(Trace, JsLifetime, Clone)]
pub(super) enum CancelAlgorithm<'js> {
    Noop,
    Function {
        f: Function<'js>,
        transformer: Option<Object<'js>>,
    },
}

// --- Abstract operations ---

pub(super) fn transform_stream_default_controller_enqueue<'js>(
    ctx: Ctx<'js>,
    stream_class: &TransformStreamClass<'js>,
    chunk: Value<'js>,
) -> Result<()> {
    let controller_class = stream_class
        .borrow()
        .readable_default_controller()
        .ok_or_else(|| Exception::throw_type(&ctx, "readable controller not available"))?;

    let can_enqueue = {
        let stream = stream_class.borrow();
        let readable_class = stream
            .readable
            .as_ref()
            .ok_or_else(|| Exception::throw_type(&ctx, "readable stream not available"))?;
        let readable = readable_class.borrow();
        controller_class
            .borrow()
            .readable_stream_default_controller_can_close_or_enqueue(&readable)
    };
    if !can_enqueue {
        return Err(Exception::throw_type(
            &ctx,
            "The stream is not in a state that permits enqueue",
        ));
    }

    if let Err(err) = readable_stream_default_controller_enqueue_value(
        ctx.clone(),
        controller_class.clone(),
        chunk,
    ) {
        if let Error::Exception = err {
            let reason = ctx.catch();
            transform_stream_error_writable_and_unblock_write(
                ctx.clone(),
                stream_class,
                reason.clone(),
            )?;
            let stored_error = stream_class
                .borrow()
                .readable_stored_error()
                .ok_or_else(|| {
                    Exception::throw_type(&ctx, "readable enqueue failed without a stored error")
                })?;
            return Err(ctx.throw(stored_error));
        }
        return Err(err);
    }

    // Update backpressure
    let has_backpressure = {
        let stream = stream_class.borrow();
        let readable_class = stream.readable.as_ref().unwrap();
        let readable = readable_class.borrow();
        let c = controller_class.borrow();
        let desired = c.readable_stream_default_controller_get_desired_size(&readable);
        desired.0.is_none_or(|size| size <= 0.0)
    };

    let current_bp = stream_class.borrow().backpressure;
    if has_backpressure != current_bp {
        transform_stream_set_backpressure(&ctx, stream_class, true)?;
    }

    Ok(())
}

pub(super) fn transform_stream_default_controller_terminate<'js>(
    ctx: Ctx<'js>,
    stream_class: &TransformStreamClass<'js>,
) -> Result<()> {
    let controller_class = stream_class
        .borrow()
        .readable_default_controller()
        .ok_or_else(|| Exception::throw_type(&ctx, "readable controller not available"))?;

    readable_stream_default_controller_close_stream(ctx.clone(), controller_class)?;

    let constructor_type_error = BasePrimordials::get(&ctx)?.constructor_type_error.clone();
    let error: Value = constructor_type_error.call(("TransformStream terminated",))?;
    transform_stream_error_writable_and_unblock_write(ctx, stream_class, error)?;
    Ok(())
}

pub(super) fn transform_stream_error<'js>(
    ctx: Ctx<'js>,
    stream_class: &TransformStreamClass<'js>,
    e: Value<'js>,
) -> Result<()> {
    let controller_class = stream_class
        .borrow()
        .readable_default_controller()
        .ok_or_else(|| Exception::throw_type(&ctx, "readable controller not available"))?;

    readable_stream_default_controller_error_stream(controller_class, e.clone())?;
    transform_stream_error_writable_and_unblock_write(ctx, stream_class, e)?;
    Ok(())
}

pub(super) fn transform_stream_error_writable_and_unblock_write<'js>(
    ctx: Ctx<'js>,
    stream_class: &TransformStreamClass<'js>,
    e: Value<'js>,
) -> Result<()> {
    let (controller_class, writable_class) = {
        let stream = stream_class.borrow();
        (stream.controller.clone(), stream.writable.clone())
    };

    if let Some(controller_class) = controller_class {
        controller_class.borrow_mut().clear_algorithms();
    }

    if let Some(writable_class) = writable_class {
        let writable_controller = writable_class.borrow().controller.clone();
        if let Some(writable_controller) = writable_controller {
            writable_stream_default_controller_error_if_needed(
                ctx.clone(),
                writable_controller,
                e,
            )?;
        }
    }

    {
        let mut stream = stream_class.borrow_mut();
        // The specification replaces this promise when unblocking a write. LLRT
        // clears it after resolving instead because no further writes can run
        // once the writable side is erroring, and retaining the replacement
        // promise would keep the QuickJS object cycle alive.
        if let Some(backpressure_change_promise) = &stream.backpressure_change_promise {
            backpressure_change_promise.resolve_undefined()?;
        }
        stream.backpressure_change_promise = None;
        stream.backpressure = false;
    }

    Ok(())
}

pub(super) fn transform_stream_set_backpressure<'js>(
    ctx: &Ctx<'js>,
    stream_class: &TransformStreamClass<'js>,
    backpressure: bool,
) -> Result<Promise<'js>> {
    let new_bp_promise = ResolveablePromise::new(ctx)?;
    let promise = new_bp_promise.promise.clone();
    let mut stream = stream_class.borrow_mut();
    if let Some(ref bp_promise) = stream.backpressure_change_promise {
        bp_promise.resolve_undefined()?;
    }
    stream.backpressure_change_promise = Some(new_bp_promise);
    stream.backpressure = backpressure;
    Ok(promise)
}

pub(super) fn transform_stream_default_controller_perform_transform<'js>(
    ctx: Ctx<'js>,
    stream_class: &TransformStreamClass<'js>,
    controller_class: &TransformStreamDefaultControllerClass<'js>,
    chunk: Value<'js>,
) -> Result<Promise<'js>> {
    let controller = controller_class.borrow();
    let algorithm = controller
        .transform_algorithm
        .clone()
        .expect("transform algorithm must exist");
    drop(controller);

    let promise_primordials = crate::utils::promise::PromisePrimordials::get(&ctx)?.clone();

    let transform_promise = match algorithm {
        TransformAlgorithm::Identity => {
            let result =
                transform_stream_default_controller_enqueue(ctx.clone(), stream_class, chunk);
            promise_resolved_with(
                &ctx,
                &promise_primordials,
                result.map(|_| Value::new_undefined(ctx.clone())),
            )?
        },
        TransformAlgorithm::Function { f, transformer } => {
            let result: Result<Value> =
                f.call((This(transformer), chunk, controller_class.clone()));
            promise_resolved_with(&ctx, &promise_primordials, result)?
        },
    };

    let stream_class = stream_class.clone();
    upon_promise::<Value<'js>, _>(
        ctx.clone(),
        transform_promise,
        move |ctx, result| match result {
            Ok(value) => Ok(value),
            Err(reason) => {
                transform_stream_error(ctx.clone(), &stream_class, reason.clone())?;
                Err(ctx.throw(reason))
            },
        },
    )
}

pub(super) fn perform_flush<'js>(
    ctx: Ctx<'js>,
    controller_class: &TransformStreamDefaultControllerClass<'js>,
) -> Result<Promise<'js>> {
    let controller = controller_class.borrow();
    let algorithm = controller
        .flush_algorithm
        .clone()
        .unwrap_or(FlushAlgorithm::Noop);
    drop(controller);

    let promise_primordials = crate::utils::promise::PromisePrimordials::get(&ctx)?.clone();

    match algorithm {
        FlushAlgorithm::Noop => Ok(promise_primordials.promise_resolved_with_undefined.clone()),
        FlushAlgorithm::Function { f, transformer } => {
            let result: Result<Value> = f.call((This(transformer), controller_class.clone()));
            promise_resolved_with(&ctx, &promise_primordials, result)
        },
    }
}

pub(super) fn perform_cancel<'js>(
    ctx: Ctx<'js>,
    controller_class: &TransformStreamDefaultControllerClass<'js>,
    reason: Value<'js>,
) -> Result<Promise<'js>> {
    let controller = controller_class.borrow();
    let algorithm = controller
        .cancel_algorithm
        .clone()
        .unwrap_or(CancelAlgorithm::Noop);
    drop(controller);

    let promise_primordials = crate::utils::promise::PromisePrimordials::get(&ctx)?.clone();

    match algorithm {
        CancelAlgorithm::Noop => Ok(promise_primordials.promise_resolved_with_undefined.clone()),
        CancelAlgorithm::Function { f, transformer } => {
            let result: Result<Value> = f.call((This(transformer), reason));
            promise_resolved_with(&ctx, &promise_primordials, result)
        },
    }
}
