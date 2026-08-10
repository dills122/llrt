use llrt_test::test_sync_with;

#[tokio::test]
async fn high_water_marks_are_coerced_to_numbers() {
    test_sync_with(|ctx| {
        crate::init(&ctx)?;
        ctx.eval::<(), _>(
            r#"
            const objectThatConvertsTo42 = () => ({
                toString() {
                    return "42";
                },
            });

            let readableHighWaterMark;
            new ReadableStream({
                start(controller) {
                    readableHighWaterMark = controller.desiredSize;
                },
            }, { highWaterMark: objectThatConvertsTo42() });

            const writableHighWaterMark = new WritableStream(
                {},
                { highWaterMark: objectThatConvertsTo42() },
            ).getWriter().desiredSize;

            let transformReadableHighWaterMark;
            const transform = new TransformStream({
                start(controller) {
                    transformReadableHighWaterMark = controller.desiredSize;
                },
            }, {
                highWaterMark: objectThatConvertsTo42(),
            }, {
                highWaterMark: objectThatConvertsTo42(),
            });
            const transformWritableHighWaterMark =
                transform.writable.getWriter().desiredSize;

            const values = [
                readableHighWaterMark,
                writableHighWaterMark,
                transformReadableHighWaterMark,
                transformWritableHighWaterMark,
                new CountQueuingStrategy({
                    highWaterMark: objectThatConvertsTo42(),
                }).highWaterMark,
                new ByteLengthQueuingStrategy({
                    highWaterMark: objectThatConvertsTo42(),
                }).highWaterMark,
            ];

            if (values.some(value => value !== 42)) {
                throw new Error(`Expected all highWaterMarks to be 42: ${values}`);
            }
            "#,
        )?;
        Ok(())
    })
    .await;
}

#[tokio::test]
async fn high_water_mark_conversion_errors_are_preserved() {
    test_sync_with(|ctx| {
        crate::init(&ctx)?;
        ctx.eval::<(), _>(
            r#"
            const expected = new Error("conversion failed");
            const throwingValue = () => ({
                valueOf() {
                    throw expected;
                },
            });

            const factories = [
                () => new ReadableStream({}, { highWaterMark: throwingValue() }),
                () => new WritableStream({}, { highWaterMark: throwingValue() }),
                () => new TransformStream({}, { highWaterMark: throwingValue() }),
                () => new TransformStream({}, {}, { highWaterMark: throwingValue() }),
                () => new CountQueuingStrategy({ highWaterMark: throwingValue() }),
                () => new ByteLengthQueuingStrategy({ highWaterMark: throwingValue() }),
            ];

            for (const factory of factories) {
                try {
                    factory();
                    throw new Error("Expected highWaterMark conversion to fail");
                } catch (error) {
                    if (error !== expected) {
                        throw new Error(`Unexpected conversion error: ${error}`);
                    }
                }
            }
            "#,
        )?;
        Ok(())
    })
    .await;
}

#[tokio::test]
async fn coerced_invalid_high_water_marks_throw_range_errors() {
    test_sync_with(|ctx| {
        crate::init(&ctx)?;
        ctx.eval::<(), _>(
            r#"
            for (const highWaterMark of ["-1", "not a number"]) {
                for (const factory of [
                    () => new ReadableStream({}, { highWaterMark }),
                    () => new WritableStream({}, { highWaterMark }),
                    () => new TransformStream({}, { highWaterMark }),
                    () => new TransformStream({}, {}, { highWaterMark }),
                ]) {
                    try {
                        factory();
                        throw new Error("Expected an invalid highWaterMark to fail");
                    } catch (error) {
                        if (!(error instanceof RangeError)) {
                            throw new Error(`Expected RangeError, got: ${error}`);
                        }
                    }
                }
            }
            "#,
        )?;
        Ok(())
    })
    .await;
}
