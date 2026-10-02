// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
use std::future::Future;

use llrt_json::{parse::json_parse, stringify::json_stringify};
use llrt_utils::{bytes::ObjectBytes, object::ObjectExt, result::ResultExt};
use rquickjs::{Array, ArrayBuffer, Class, Ctx, FromJs, Result, Value};

use super::{
    crypto_key::CryptoKey,
    encryption::{self, encrypt_decrypt},
    encryption_algorithm::EncryptionAlgorithm,
    export_key::{export_key, ExportOutput},
    import_key::import_key,
    key_algorithm::{KeyFormat, KeyFormatData},
    EncryptionMode, WebCryptoBufferSource,
};

pub fn subtle_wrap_key<'js>(
    ctx: Ctx<'js>,
    format: KeyFormat,
    key: Class<'js, CryptoKey<'js>>,
    wrapping_key: Class<'js, CryptoKey<'js>>,
    wrap_algo: Value<'js>,
) -> impl Future<Output = Result<ArrayBuffer<'js>>> + 'js {
    let wrap_algo = EncryptionAlgorithm::from_js(&ctx, wrap_algo);

    async move {
        let wrap_algo = wrap_algo?;
        let key = key.borrow();

        let export = export_key(&ctx, format, &key)?;

        let (bytes, padding) = match export {
            ExportOutput::Bytes(bytes) => (bytes, 0),
            ExportOutput::Object(value) => {
                let json = json_stringify(&ctx, value.into_value())?.unwrap();
                (json.into_bytes(), b' ')
            },
        };

        let wrapping_key = wrapping_key.borrow();
        wrapping_key.check_validity("wrapKey").or_throw(&ctx)?;

        let bytes = encrypt_decrypt(
            &ctx,
            &wrap_algo,
            &wrapping_key,
            &bytes,
            EncryptionMode::Wrapping(padding),
            encryption::EncryptionOperation::Encrypt,
        )?;

        ArrayBuffer::new(ctx, bytes)
    }
}

pub fn subtle_unwrap_key<'js>(
    format: KeyFormat,
    wrapped_key: WebCryptoBufferSource<'js>,
    unwrapping_key: Class<'js, CryptoKey<'js>>,
    unwrap_algo: Value<'js>,
    unwrapped_key_algo: Value<'js>,
    extractable: bool,
    key_usages: Array<'js>,
) -> impl Future<Output = Result<Class<'js, CryptoKey<'js>>>> + 'js {
    let ctx = wrapped_key.ctx();
    let unwrap_algo = EncryptionAlgorithm::from_js(&ctx, unwrap_algo);
    let wrapped_key = wrapped_key.snapshot();

    async move {
        let unwrap_algo = unwrap_algo?;
        let unwrapping_key = unwrapping_key.borrow();
        unwrapping_key.check_validity("unwrapKey").or_throw(&ctx)?;

        let padding = match format {
            KeyFormat::Jwk => b' ',
            _ => 0,
        };

        let bytes = encrypt_decrypt(
            &ctx,
            &unwrap_algo,
            &unwrapping_key,
            &wrapped_key,
            EncryptionMode::Wrapping(padding),
            encryption::EncryptionOperation::Decrypt,
        )?;

        let key_format = match format {
            KeyFormat::Jwk => KeyFormatData::Jwk(
                json_parse(&ctx, bytes)?.into_object_or_throw(&ctx, "wrappedKey")?,
            ),
            KeyFormat::Raw => KeyFormatData::Raw(ObjectBytes::Vec(bytes)),
            KeyFormat::Spki => KeyFormatData::Spki(ObjectBytes::Vec(bytes)),
            KeyFormat::Pkcs8 => KeyFormatData::Pkcs8(ObjectBytes::Vec(bytes)),
        };

        import_key(ctx, key_format, unwrapped_key_algo, extractable, key_usages)
    }
}
