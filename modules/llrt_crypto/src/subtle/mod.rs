// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
mod crypto_key;
mod derive_algorithm;
mod derive_bits;
mod derive_keys;
mod digest;
mod encryption;
mod encryption_algorithm;
#[cfg(feature = "_subtle-full")]
mod export_key;
mod generate_key;
#[cfg(feature = "_subtle-full")]
mod import_key;
#[cfg(feature = "_subtle-full")]
mod key_algorithm;
mod sign;
mod sign_algorithm;
mod util;
mod verify;
#[cfg(feature = "_subtle-full")]
mod wrapping;

pub use crypto_key::CryptoKey;
pub use derive_bits::subtle_derive_bits;
pub use derive_keys::subtle_derive_key;
pub use digest::subtle_digest;
pub use encryption::subtle_decrypt;
pub use encryption::subtle_encrypt;
#[cfg(feature = "_subtle-full")]
pub use export_key::subtle_export_key;
pub use generate_key::subtle_generate_key;
#[cfg(feature = "_subtle-full")]
pub use import_key::subtle_import_key;
#[cfg(feature = "_subtle-full")]
use key_algorithm::KeyAlgorithm;
pub use sign::subtle_sign;
pub use verify::subtle_verify;
#[cfg(feature = "_subtle-full")]
pub use wrapping::subtle_unwrap_key;
#[cfg(feature = "_subtle-full")]
pub use wrapping::subtle_wrap_key;

// Stub implementations for limited crypto providers (no _subtle-full)
#[cfg(not(feature = "_subtle-full"))]
mod key_algorithm;
#[cfg(not(feature = "_subtle-full"))]
use key_algorithm::KeyAlgorithm;

use llrt_exceptions::DOMException;
use llrt_utils::{
    bytes::ObjectBytes,
    object::ObjectExt,
    primordials::{BasePrimordials, Primordial},
    str_enum,
};
use rquickjs::{
    atom::PredefinedAtom, ArrayBuffer, Ctx, Error, Exception, FromJs, Function, Object, Result,
    Value,
};

use crate::provider::{CryptoProvider, SimpleDigest};

use crate::hash::HashAlgorithm;

#[rquickjs::class]
#[derive(rquickjs::JsLifetime, rquickjs::class::Trace)]
pub struct SubtleCrypto {}

/// A Web IDL `BufferSource`, scoped to WebCrypto so LLRT's deliberately
/// permissive general-purpose byte conversion can remain backward compatible.
pub struct WebCryptoBufferSource<'js> {
    ctx: Ctx<'js>,
    bytes: ObjectBytes<'js>,
}

impl<'js> FromJs<'js> for WebCryptoBufferSource<'js> {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let object = value.as_object().ok_or_else(|| {
            Exception::throw_type(ctx, "value is not an ArrayBuffer or ArrayBufferView")
        })?;

        let is_array_buffer = ArrayBuffer::from_object(object.clone()).is_some();
        let is_view = BasePrimordials::get(ctx)?
            .function_array_buffer_is_view
            .call::<_, bool>((object.clone(),))?;
        if !is_array_buffer && !is_view {
            return Err(Exception::throw_type(
                ctx,
                "value is not an ArrayBuffer or ArrayBufferView",
            ));
        }

        let bytes = ObjectBytes::from_array_buffer(object)?.ok_or_else(|| {
            Exception::throw_type(ctx, "value is not an ArrayBuffer or ArrayBufferView")
        })?;
        let (buffer, length, offset) = bytes.get_array_buffer()?.ok_or_else(|| {
            Exception::throw_type(ctx, "value is not an ArrayBuffer or ArrayBufferView")
        })?;
        if offset
            .checked_add(length)
            .is_none_or(|end| end > buffer.len())
        {
            return Err(Exception::throw_type(
                ctx,
                "ArrayBufferView is outside its backing buffer",
            ));
        }

        Ok(Self {
            ctx: ctx.clone(),
            bytes,
        })
    }
}

impl<'js> WebCryptoBufferSource<'js> {
    pub fn snapshot(&self) -> Vec<u8> {
        self.bytes
            .as_bytes_opt()
            .map(<[u8]>::to_vec)
            .unwrap_or_default()
    }

    pub fn ctx(&self) -> Ctx<'js> {
        self.ctx.clone()
    }
}

/// Wrap a typed async binding with the Web IDL Promise-returning operation
/// boundary. `rquickjs::Async` converts Rust parameters before it constructs a
/// JavaScript Promise, so conversion failures would otherwise escape
/// synchronously. The wrapper is a JavaScript closure so QuickJS traces its
/// captured implementation function; capturing a JavaScript function in an
/// `rquickjs` Rust callback would leave that reference untraced.
pub fn promise_method<'js>(
    ctx: &Ctx<'js>,
    implementation: Function<'js>,
    name: &'static str,
    length: usize,
) -> Result<Function<'js>> {
    let factory: Function = ctx.eval(
        r#"(implementation, PromiseConstructor) => {
            const reject = PromiseConstructor.reject.bind(PromiseConstructor);
            return function (...args) {
                try {
                    return implementation(...args);
                } catch (error) {
                    return reject(error);
                }
            };
        }"#,
    )?;
    let promise_constructor: Function = ctx.globals().get(PredefinedAtom::Promise)?;
    let function: Function = factory.call((implementation, promise_constructor))?;
    function.set_name(name)?;
    function.set_length(length)?;
    Ok(function)
}

#[rquickjs::methods]
impl SubtleCrypto {
    #[qjs(constructor)]
    pub fn new(ctx: Ctx<'_>) -> Result<Self> {
        Err(Exception::throw_type(&ctx, "Illegal constructor"))
    }

    #[qjs(prop, rename = PredefinedAtom::SymbolToStringTag, configurable)]
    pub fn to_string_tag() -> &'static str {
        stringify!(SubtleCrypto)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EllipticCurve {
    P256,
    P384,
    P521,
}

str_enum!(EllipticCurve,P256 => "P-256", P384 => "P-384", P521 => "P-521");

pub enum EncryptionMode {
    Encryption,
    #[allow(dead_code)]
    Wrapping(u8), //padding byte
}

pub fn rsa_hash_digest<'a>(
    ctx: &Ctx<'_>,
    key: &'a CryptoKey,
    data: &'a [u8],
    algorithm_name: &str,
) -> Result<(&'a HashAlgorithm, Vec<u8>)> {
    let hash = match &key.algorithm {
        KeyAlgorithm::Rsa { hash, .. } => hash,
        _ => return algorithm_mismatch_error(ctx, algorithm_name),
    };
    if !matches!(
        hash,
        HashAlgorithm::Sha256 | HashAlgorithm::Sha384 | HashAlgorithm::Sha512
    ) {
        return Err(Exception::throw_message(
            ctx,
            "Only Sha-256, Sha-384 or Sha-512 is supported for RSA",
        ));
    }

    let mut hasher = crate::CRYPTO_PROVIDER.digest(*hash);
    hasher.update(data);
    let digest = hasher.finalize();

    Ok((hash, digest))
}

pub fn to_name_and_maybe_object<'js>(
    ctx: &Ctx<'js>,
    value: Value<'js>,
) -> Result<(String, Result<Object<'js>>)> {
    let obj;
    let name = if let Some(string) = value.as_string() {
        obj = Err(Error::new_from_js_message(
            "string",
            "object",
            "algorithm is not an object",
        ));
        string.to_string()?
    } else if let Some(object) = value.into_object() {
        let name = object.get_required("name", "algorithm")?;
        obj = Ok(object);
        name
    } else {
        return Err(Exception::throw_message(
            ctx,
            "algorithm must be a string or an object",
        ));
    };
    Ok((name, obj))
}

pub fn normalize_algorithm_name(name: &str) -> String {
    let name = name.trim().to_ascii_uppercase();
    match name.as_str() {
        "ED25519" => "Ed25519".to_string(),
        "RSASSA-PKCS1-V1_5" => "RSASSA-PKCS1-v1_5".to_string(),
        _ => name,
    }
}

pub fn algorithm_mismatch_error<T>(ctx: &Ctx<'_>, expected_algorithm: &str) -> Result<T> {
    Err(DOMException::type_mismatch_error(
        ctx,
        ["Key algorithm must be ", expected_algorithm].concat(),
    ))
}

pub fn algorithm_not_supported_error<T>(ctx: &Ctx<'_>) -> Result<T> {
    Err(DOMException::not_supported_error(
        ctx,
        "Algorithm not supported",
    ))
}

pub fn algorithm_invalid_access_error<T>(ctx: &Ctx<'_>, expected_algorithm: &str) -> Result<T> {
    Err(DOMException::invalid_access_error(
        ctx,
        ["Key algorithm must be ", expected_algorithm].concat(),
    ))
}

// Stub implementations for providers without _subtle-full
#[cfg(not(feature = "_subtle-full"))]
mod stubs;
#[cfg(not(feature = "_subtle-full"))]
pub use stubs::subtle_export_key;
#[cfg(not(feature = "_subtle-full"))]
pub use stubs::subtle_import_key;
#[cfg(not(feature = "_subtle-full"))]
pub use stubs::subtle_unwrap_key;
#[cfg(not(feature = "_subtle-full"))]
pub use stubs::subtle_wrap_key;
