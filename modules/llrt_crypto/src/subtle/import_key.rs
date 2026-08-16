// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
use std::future::Future;

use llrt_exceptions::DOMException;
use llrt_utils::{bytes::ObjectBytes, object::ObjectExt};
use rquickjs::{Array, Class, Ctx, FromJs, Result, Value};

use super::{
    crypto_key::{CryptoKey, KeyKind},
    key_algorithm::{
        KeyAlgorithm, KeyAlgorithmMode, KeyAlgorithmWithUsages, KeyFormat, KeyFormatData,
    },
    WebCryptoBufferSource,
};

enum ImportKeyData<'js> {
    Jwk(rquickjs::Object<'js>),
    Raw(WebCryptoBufferSource<'js>),
    Spki(WebCryptoBufferSource<'js>),
    Pkcs8(WebCryptoBufferSource<'js>),
}

pub fn subtle_import_key<'js>(
    ctx: Ctx<'js>,
    format: KeyFormat,
    key_data: Value<'js>,
    algorithm: Value<'js>,
    extractable: bool,
    key_usages: Array<'js>,
) -> impl Future<Output = Result<Class<'js, CryptoKey<'js>>>> + 'js {
    // Web IDL converts keyData before WebCrypto normalizes algorithm, while
    // WebCrypto copies binary key data only after that normalization.
    let key_data = match format {
        KeyFormat::Raw => WebCryptoBufferSource::from_js(&ctx, key_data).map(ImportKeyData::Raw),
        KeyFormat::Pkcs8 => {
            WebCryptoBufferSource::from_js(&ctx, key_data).map(ImportKeyData::Pkcs8)
        },
        KeyFormat::Spki => WebCryptoBufferSource::from_js(&ctx, key_data).map(ImportKeyData::Spki),
        KeyFormat::Jwk => key_data
            .into_object_or_throw(&ctx, "keyData")
            .map(ImportKeyData::Jwk),
    };
    let prepared = key_data.and_then(|key_data| {
        let algorithm = KeyAlgorithm::prepare_import_algorithm(&ctx, algorithm)?;
        let format = match key_data {
            ImportKeyData::Jwk(data) => KeyFormatData::Jwk(data),
            ImportKeyData::Raw(data) => KeyFormatData::Raw(ObjectBytes::Vec(data.snapshot())),
            ImportKeyData::Spki(data) => KeyFormatData::Spki(ObjectBytes::Vec(data.snapshot())),
            ImportKeyData::Pkcs8(data) => KeyFormatData::Pkcs8(ObjectBytes::Vec(data.snapshot())),
        };
        Ok((format, algorithm))
    });

    async move {
        let (format, algorithm) = prepared?;
        import_key(ctx, format, algorithm, extractable, key_usages)
    }
}

pub fn import_key<'js>(
    ctx: Ctx<'js>,
    format: KeyFormatData<'js>,
    algorithm: Value<'js>,
    extractable: bool,
    key_usages: Array<'js>,
) -> Result<Class<'js, CryptoKey<'js>>> {
    if extractable {
        if let KeyFormatData::Jwk(jwk) = &format {
            if matches!(jwk.get_optional::<_, bool>("ext")?, Some(false)) {
                return Err(DOMException::data_error(&ctx, "JWK is not extractable"));
            }
        }
    }

    let mut kind = KeyKind::Public;
    let mut data = Vec::new();

    let KeyAlgorithmWithUsages {
        name,
        algorithm: key_algorithm,
        public_usages,
        private_usages,
    } = KeyAlgorithm::from_js(
        &ctx,
        KeyAlgorithmMode::Import {
            kind: &mut kind,
            data: &mut data,
            format,
        },
        algorithm,
        key_usages,
    )?;

    let usages = match kind {
        KeyKind::Public | KeyKind::Secret => public_usages,
        KeyKind::Private => private_usages,
    };

    Class::instance(
        ctx,
        CryptoKey::new(kind, name, extractable, key_algorithm, usages, data),
    )
}
