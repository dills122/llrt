// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Stub implementations for SubtleCrypto operations when `_rustcrypto` feature is disabled.
//! These return errors indicating the operation is not supported.

use rquickjs::{Ctx, Exception, FromJs, Object, Result, Value};

use super::{crypto_key::CryptoKey, encryption_algorithm, key_algorithm, WebCryptoBufferSource};

pub async fn subtle_export_key<'js>(
    ctx: Ctx<'js>,
    _format: key_algorithm::KeyFormat,
    _key: rquickjs::Class<'js, CryptoKey<'js>>,
) -> Result<Object<'js>> {
    Err(Exception::throw_message(
        &ctx,
        "exportKey is not supported with this crypto provider",
    ))
}

pub async fn subtle_import_key<'js>(
    ctx: Ctx<'js>,
    format: key_algorithm::KeyFormat,
    key_data: Value<'js>,
    _algorithm: Value<'js>,
    _extractable: bool,
    _key_usages: rquickjs::Array<'js>,
) -> Result<rquickjs::Class<'js, CryptoKey<'js>>> {
    if !matches!(format, key_algorithm::KeyFormat::Jwk) {
        WebCryptoBufferSource::from_js(&ctx, key_data)?;
    }
    Err(Exception::throw_message(
        &ctx,
        "importKey is not supported with this crypto provider",
    ))
}

pub async fn subtle_wrap_key<'js>(
    ctx: Ctx<'js>,
    _format: key_algorithm::KeyFormat,
    _key: rquickjs::Class<'js, CryptoKey<'js>>,
    _wrapping_key: rquickjs::Class<'js, CryptoKey<'js>>,
    wrap_algo: Value<'js>,
) -> Result<rquickjs::ArrayBuffer<'js>> {
    encryption_algorithm::EncryptionAlgorithm::from_js(&ctx, wrap_algo)?;
    Err(Exception::throw_message(
        &ctx,
        "wrapKey is not supported with this crypto provider",
    ))
}

pub async fn subtle_unwrap_key<'js>(
    _format: key_algorithm::KeyFormat,
    wrapped_key: WebCryptoBufferSource<'js>,
    _unwrapping_key: rquickjs::Class<'js, CryptoKey<'js>>,
    unwrap_algo: Value<'js>,
    _unwrapped_key_algo: Value<'js>,
    _extractable: bool,
    _key_usages: rquickjs::Array<'js>,
) -> Result<rquickjs::Class<'js, CryptoKey<'js>>> {
    let ctx = wrapped_key.ctx();
    encryption_algorithm::EncryptionAlgorithm::from_js(&ctx, unwrap_algo)?;
    Err(Exception::throw_message(
        &ctx,
        "unwrapKey is not supported with this crypto provider",
    ))
}
