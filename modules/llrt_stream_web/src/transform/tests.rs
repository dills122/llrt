use llrt_test::test_async_with;
use rquickjs::Promise;

fn eval_async<'js>(ctx: &rquickjs::Ctx<'js>, js: &str) -> rquickjs::Result<Promise<'js>> {
    ctx.eval(format!("(async () => {{ {js} }})()"))
}

#[tokio::test]
async fn identity_passthrough() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const ts = new TransformStream();
                const writer = ts.writable.getWriter();
                const reader = ts.readable.getReader();

                writer.write("one");
                writer.write("two");
                writer.close();

                const chunks = [];
                while (true) {
                    const { value, done } = await reader.read();
                    if (done) break;
                    chunks.push(value);
                }
                if (chunks.join(",") !== "one,two") throw new Error("got: " + chunks);
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn transform_chunks() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const ts = new TransformStream({
                    transform(chunk, controller) {
                        controller.enqueue(chunk.toUpperCase());
                    }
                });
                const writer = ts.writable.getWriter();
                const reader = ts.readable.getReader();

                writer.write("hello");
                writer.write("world");
                writer.close();

                const chunks = [];
                while (true) {
                    const { value, done } = await reader.read();
                    if (done) break;
                    chunks.push(value);
                }
                if (chunks.join(" ") !== "HELLO WORLD") throw new Error("got: " + chunks);
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn one_to_many_expansion() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const ts = new TransformStream({
                    transform(chunk, controller) {
                        for (const byte of chunk) {
                            controller.enqueue(byte);
                        }
                    }
                });
                const writer = ts.writable.getWriter();
                const reader = ts.readable.getReader();

                writer.write([1, 2, 3]);
                writer.close();

                const chunks = [];
                while (true) {
                    const { value, done } = await reader.read();
                    if (done) break;
                    chunks.push(value);
                }
                if (chunks.join(",") !== "1,2,3") throw new Error("got: " + chunks);
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn readable_high_water_mark_applies_backpressure() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            ctx.eval::<(), _>(
                r#"
                globalThis.transformed = [];
                globalThis.ts = new TransformStream({
                    transform(chunk, controller) {
                        transformed.push(chunk);
                        controller.enqueue(chunk);
                    }
                }, undefined, { highWaterMark: 3 });
                globalThis.writer = ts.writable.getWriter();
                [0, 1, 2, 3].forEach(chunk => writer.write(chunk));
            "#,
            )
            .unwrap();

            while ctx.execute_pending_job() {}
            assert_eq!(
                ctx.eval::<String, _>("transformed.join(',')").unwrap(),
                "0,1,2"
            );

            ctx.eval::<(), _>("globalThis.reader = ts.readable.getReader(); reader.read();")
                .unwrap();
            while ctx.execute_pending_job() {}
            assert_eq!(
                ctx.eval::<String, _>("transformed.join(',')").unwrap(),
                "0,1,2,3"
            );
        })
    })
    .await;
}

#[tokio::test]
async fn flush_on_close() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const ts = new TransformStream({
                    transform(chunk, controller) {
                        controller.enqueue(chunk);
                    },
                    flush(controller) {
                        controller.enqueue("DONE");
                    }
                });
                const writer = ts.writable.getWriter();
                const reader = ts.readable.getReader();

                writer.write("a");
                writer.close();

                const chunks = [];
                while (true) {
                    const { value, done } = await reader.read();
                    if (done) break;
                    chunks.push(value);
                }
                if (chunks.join(",") !== "a,DONE") throw new Error("got: " + chunks);
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn pipe_through_chain() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const source = new ReadableStream({
                    start(controller) {
                        controller.enqueue("hello");
                        controller.enqueue("world");
                        controller.close();
                    }
                });

                const upper = new TransformStream({
                    transform(chunk, c) { c.enqueue(chunk.toUpperCase()); }
                });
                const exclaim = new TransformStream({
                    transform(chunk, c) { c.enqueue(chunk + "!"); }
                });

                const reader = source
                    .pipeThrough(upper)
                    .pipeThrough(exclaim)
                    .getReader();

                const chunks = [];
                while (true) {
                    const { value, done } = await reader.read();
                    if (done) break;
                    chunks.push(value);
                }
                if (chunks.join(" ") !== "HELLO! WORLD!") throw new Error("got: " + chunks);
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn async_transform() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const ts = new TransformStream({
                    async transform(chunk, controller) {
                        await new Promise(r => r());
                        controller.enqueue(chunk * 2);
                    }
                });
                const writer = ts.writable.getWriter();
                const reader = ts.readable.getReader();

                writer.write(5);
                writer.close();

                const { value } = await reader.read();
                if (value !== 10) throw new Error("expected 10, got " + value);
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn error_propagates_to_reader() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const ts = new TransformStream({
                    transform(chunk, controller) {
                        controller.error(new Error("broken"));
                    }
                });
                const writer = ts.writable.getWriter();
                const reader = ts.readable.getReader();

                writer.write("x").catch(() => {});

                try {
                    await reader.read();
                    throw new Error("should have thrown");
                } catch (e) {
                    if (e.message !== "broken") throw new Error("wrong error: " + e.message);
                }
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn caught_enqueue_size_error_still_errors_writable() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const ts = new TransformStream({
                    transform(chunk, controller) {
                        try {
                            controller.enqueue(chunk);
                            throw new Error("enqueue should have thrown");
                        } catch (error) {
                            if (!(error instanceof RangeError)) throw error;
                            enqueueError = error;
                        }
                    }
                }, undefined, {
                    size() { return -1; },
                    highWaterMark: 1
                });
                const writer = ts.writable.getWriter();
                const reader = ts.readable.getReader();
                let enqueueError;

                const rejectsWithEnqueueError = promise => promise.then(
                    () => { throw new Error("stream promise should have rejected"); },
                    reason => {
                        if (reason !== enqueueError) throw reason;
                    }
                );
                const rejectionChecks = [writer.closed, reader.closed]
                    .map(rejectsWithEnqueueError);

                await writer.write("x");
                await Promise.all([
                    rejectsWithEnqueueError(writer.ready),
                    ...rejectionChecks
                ]);
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn enqueue_throws_readable_stored_error() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const storedError = new Error("stored readable error");
                const redundantError = new Error("redundant size error");
                let controller;
                const ts = new TransformStream({
                    transform(chunk, value) {
                        controller = value;
                        controller.enqueue(chunk);
                    }
                }, undefined, {
                    size() {
                        controller.error(storedError);
                        throw redundantError;
                    },
                    highWaterMark: 1
                });
                const writer = ts.writable.getWriter();
                const reader = ts.readable.getReader();

                const rejectsExactly = promise => promise.then(
                    () => { throw new Error("stream promise should have rejected"); },
                    reason => {
                        if (reason !== storedError) throw reason;
                    }
                );

                await Promise.all([
                    rejectsExactly(writer.write("x")),
                    rejectsExactly(writer.closed),
                    rejectsExactly(reader.closed)
                ]);
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn writable_abort_errors_readable_with_exact_reason() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const abortReason = new Error("abort reason");
                let cancelReason;
                const ts = new TransformStream({
                    cancel(reason) { cancelReason = reason; }
                });
                const reader = ts.readable.getReader();
                const readableResult = reader.closed.then(
                    () => { throw new Error("reader.closed should have rejected"); },
                    reason => {
                        if (reason !== abortReason) throw reason;
                    }
                );

                await ts.writable.abort(abortReason);
                if (cancelReason !== abortReason)
                    throw new Error("cancel received the wrong reason");

                const outcome = { settled: false, error: undefined };
                readableResult.then(
                    () => { outcome.settled = true; },
                    error => {
                        outcome.settled = true;
                        outcome.error = error;
                    }
                );
                for (let i = 0; i < 10 && !outcome.settled; ++i) {
                    await Promise.resolve();
                }
                if (!outcome.settled) throw new Error("reader.closed remained pending");
                if (outcome.error !== undefined) throw outcome.error;
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn parallel_cancel_and_close_share_finish_promise() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const cancelReason = new Error("cancel reason");
                const cancelError = new Error("cancel failed");
                let flushCalls = 0;
                const ts = new TransformStream({
                    async cancel(reason) {
                        if (reason !== cancelReason)
                            throw new Error("cancel received the wrong reason");
                        throw cancelError;
                    },
                    flush() { ++flushCalls; }
                });

                const cancelPromise = ts.readable.cancel(cancelReason);
                const closePromise = ts.writable.close();
                const rejectsExactly = promise => promise.then(
                    () => { throw new Error("operation should have rejected"); },
                    reason => {
                        if (reason !== cancelError) throw reason;
                    }
                );

                await Promise.all([
                    rejectsExactly(cancelPromise),
                    rejectsExactly(closePromise)
                ]);
                if (flushCalls !== 0) throw new Error("flush should not have been called");
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn reentrant_cancel_runs_transformer_cancel_once() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const abortReason = new Error("abort reason");
                const nestedReason = new Error("nested cancel reason");
                let cancelCalls = 0;
                let nestedCancel;
                const ts = new TransformStream({
                    cancel() {
                        ++cancelCalls;
                        if (cancelCalls === 1) {
                            nestedCancel = ts.readable.cancel(nestedReason);
                        }
                    }
                });

                await ts.writable.abort(abortReason);
                await nestedCancel;
                if (cancelCalls !== 1)
                    throw new Error("cancel should have been called exactly once");
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn terminate_rejects_write_waiting_on_backpressure() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                let controller;
                const ts = new TransformStream({
                    start(value) { controller = value; }
                }, undefined, { highWaterMark: 0 });
                const writer = ts.writable.getWriter();
                const write = writer.write("blocked");

                await Promise.resolve();
                controller.terminate();

                try {
                    await write;
                    throw new Error("write should have rejected");
                } catch (error) {
                    if (!(error instanceof TypeError)) throw error;
                }

                try {
                    await writer.closed;
                    throw new Error("writer.closed should have rejected");
                } catch (error) {
                    if (!(error instanceof TypeError)) throw error;
                }

                const { done } = await ts.readable.getReader().read();
                if (!done) throw new Error("readable should have closed");
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn synchronous_transform_error_rejects_both_sides() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const error = new Error("transform failed");
                const ts = new TransformStream({
                    transform() { throw error; }
                }, undefined, { highWaterMark: 1 });
                const writer = ts.writable.getWriter();
                const reader = ts.readable.getReader();

                const rejectsExactly = promise => promise.then(
                    () => { throw new Error("stream promise should have rejected"); },
                    reason => {
                        if (reason !== error) throw reason;
                    }
                );
                await Promise.all([
                    rejectsExactly(writer.write("x")),
                    rejectsExactly(writer.closed),
                    rejectsExactly(reader.read()),
                    rejectsExactly(reader.closed)
                ]);
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn asynchronous_transform_error_rejects_both_sides() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const error = new Error("async transform failed");
                const ts = new TransformStream({
                    transform() { return Promise.reject(error); }
                }, undefined, { highWaterMark: 1 });
                const writer = ts.writable.getWriter();
                const reader = ts.readable.getReader();

                const rejectsExactly = promise => promise.then(
                    () => { throw new Error("stream promise should have rejected"); },
                    reason => {
                        if (reason !== error) throw reason;
                    }
                );
                await Promise.all([
                    rejectsExactly(writer.write("x")),
                    rejectsExactly(writer.closed),
                    rejectsExactly(reader.read()),
                    rejectsExactly(reader.closed)
                ]);
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn controller_error_during_flush_rejects_close() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const error = new Error("flush failed");
                const ts = new TransformStream({
                    flush(controller) { controller.error(error); }
                });
                const writer = ts.writable.getWriter();
                const reader = ts.readable.getReader();

                const rejectsExactly = promise => promise.then(
                    () => { throw new Error("stream promise should have rejected"); },
                    reason => {
                        if (reason !== error) throw reason;
                    }
                );
                await Promise.all([
                    rejectsExactly(writer.close()),
                    rejectsExactly(writer.closed),
                    rejectsExactly(reader.closed)
                ]);
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn finish_algorithms_preserve_existing_stream_error() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const rejectsExactly = (promise, expected) => promise.then(
                    () => { throw new Error("operation should have rejected"); },
                    reason => {
                        if (reason !== expected) throw reason;
                    }
                );

                {
                    const storedError = new Error("readable error during flush");
                    const redundantError = new Error("flush rejection");
                    const ts = new TransformStream({
                        flush(controller) {
                            controller.error(storedError);
                            throw redundantError;
                        }
                    });
                    await rejectsExactly(ts.writable.close(), storedError);
                }

                {
                    const storedError = new Error("readable error during abort");
                    const redundantError = new Error("abort rejection");
                    let controller;
                    const ts = new TransformStream({
                        start(value) { controller = value; },
                        cancel() {
                            controller.error(storedError);
                            throw redundantError;
                        }
                    });
                    await rejectsExactly(ts.writable.abort(), storedError);
                }

                {
                    const storedError = new Error("writable error during cancel");
                    const redundantError = new Error("cancel rejection");
                    let controller;
                    const ts = new TransformStream({
                        start(value) { controller = value; },
                        cancel() {
                            controller.error(storedError);
                            throw redundantError;
                        }
                    });
                    await rejectsExactly(ts.readable.cancel(), storedError);
                }
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn fulfilled_cancel_errors_writable_with_original_reason() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const reason = new Error("cancelled");
                let seenReason;
                const ts = new TransformStream({
                    cancel(value) { seenReason = value; }
                });
                const writer = ts.writable.getWriter();

                await ts.readable.cancel(reason);
                if (seenReason !== reason) throw new Error("cancel received the wrong reason");
                try {
                    await writer.closed;
                    throw new Error("writer.closed should have rejected");
                } catch (error) {
                    if (error !== reason) throw error;
                }
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn rejected_cancel_errors_writable_with_rejection_reason() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const reason = new Error("cancelled");
                const cancelError = new Error("cancel failed");
                const ts = new TransformStream({
                    cancel() { throw cancelError; }
                });
                const writer = ts.writable.getWriter();

                const rejectsExactly = promise => promise.then(
                    () => { throw new Error("stream promise should have rejected"); },
                    error => {
                        if (error !== cancelError) throw error;
                    }
                );
                await Promise.all([
                    rejectsExactly(ts.readable.cancel(reason)),
                    rejectsExactly(writer.closed)
                ]);
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn enqueue_after_terminate_throws() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                let controller;
                new TransformStream({ start(value) { controller = value; } });
                controller.terminate();

                try {
                    controller.enqueue("late");
                    throw new Error("enqueue should have thrown");
                } catch (error) {
                    if (!(error instanceof TypeError)) throw error;
                }
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn start_receives_controller() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                let controllerRef;
                const ts = new TransformStream({
                    start(controller) {
                        controllerRef = controller;
                        controller.enqueue("from-start");
                    }
                });

                if (typeof controllerRef.desiredSize !== "number")
                    throw new Error("controller.desiredSize should be a number");

                const reader = ts.readable.getReader();
                const { value } = await reader.read();
                if (value !== "from-start") throw new Error("got: " + value);
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn terminate_closes_readable() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const ts = new TransformStream({
                    transform(chunk, controller) {
                        if (chunk === "stop") {
                            controller.terminate();
                            return;
                        }
                        controller.enqueue(chunk);
                    }
                });
                const writer = ts.writable.getWriter();
                const reader = ts.readable.getReader();

                writer.write("keep").catch(() => {});
                writer.write("stop").catch(() => {});

                const { value } = await reader.read();
                if (value !== "keep") throw new Error("got: " + value);

                const { done } = await reader.read();
                if (!done) throw new Error("expected stream to be closed after terminate");
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn illegal_constructor() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                try {
                    new TransformStreamDefaultController();
                    throw new Error("should have thrown");
                } catch (e) {
                    if (!(e instanceof TypeError)) throw new Error("expected TypeError, got " + e);
                }
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}

#[tokio::test]
async fn pipe_to_writable_stream() {
    test_async_with(|ctx| {
        crate::init(&ctx).unwrap();
        Box::pin(async move {
            eval_async(
                &ctx,
                r#"
                const collected = [];
                const source = new ReadableStream({
                    start(c) { c.enqueue(1); c.enqueue(2); c.enqueue(3); c.close(); }
                });
                const transform = new TransformStream({
                    transform(chunk, c) { c.enqueue(chunk * 10); }
                });
                const sink = new WritableStream({
                    write(chunk) { collected.push(chunk); }
                });

                await source.pipeThrough(transform).pipeTo(sink);

                if (collected.join(",") !== "10,20,30") throw new Error("got: " + collected);
            "#,
            )
            .unwrap()
            .into_future::<()>()
            .await
            .unwrap();
        })
    })
    .await;
}
