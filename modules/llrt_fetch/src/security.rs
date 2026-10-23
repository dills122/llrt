// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
use hyper::Uri;
use rquickjs::{Ctx, Error, Exception, Result};
use std::sync::OnceLock;

static HTTP_ALLOW_LIST: OnceLock<Vec<Uri>> = OnceLock::new();

static HTTP_DENY_LIST: OnceLock<Vec<Uri>> = OnceLock::new();

pub fn set_allow_list(values: Vec<Uri>) {
    _ = HTTP_ALLOW_LIST.set(values);
}

pub fn get_allow_list() -> Option<&'static Vec<Uri>> {
    HTTP_ALLOW_LIST.get()
}

pub fn set_deny_list(values: Vec<Uri>) {
    _ = HTTP_DENY_LIST.set(values);
}

pub fn get_deny_list() -> Option<&'static Vec<Uri>> {
    HTTP_DENY_LIST.get()
}

pub fn ensure_url_access(ctx: &Ctx<'_>, uri: &Uri) -> Result<()> {
    if let Some(allow_list) = HTTP_ALLOW_LIST.get() {
        if !url_match(allow_list, uri) {
            return Err(url_restricted_error(ctx, "URL not allowed", uri));
        }
    }

    if let Some(deny_list) = HTTP_DENY_LIST.get() {
        if url_match(deny_list, uri) {
            return Err(url_restricted_error(ctx, "URL denied", uri));
        }
    }

    Ok(())
}

fn url_restricted_error(ctx: &Ctx<'_>, message: &str, uri: &Uri) -> Error {
    let uri_host = uri.host().unwrap_or_default();
    let mut message_string = String::with_capacity(message.len() + 100);
    message_string.push_str(message);
    message_string.push_str(": ");
    message_string.push_str(uri_host);
    if let Some(port) = uri.port_u16() {
        message_string.push(':');
        message_string.push_str(itoa::Buffer::new().format(port))
    }

    Exception::throw_message(ctx, &message_string)
}

fn url_match(list: &[Uri], uri: &Uri) -> bool {
    let host = uri.host().unwrap_or_default();
    let port = effective_port(uri);
    list.iter().any(|entry| {
        let entry_host = entry.host().unwrap_or_default();
        let port_match = effective_port(entry) == port;
        let host_match = host == entry_host || host.ends_with(&format!(".{}", entry_host));
        port_match && host_match
    })
}

fn effective_port(uri: &Uri) -> u16 {
    uri.port_u16().unwrap_or_else(|| {
        if uri.scheme_str() == Some("https") {
            443
        } else {
            80
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_effective_http_ports() {
        for (entry, target, expected) in [
            ("https://example.com:443", "https://example.com/path", true),
            ("https://example.com", "https://example.com:443/path", true),
            ("https://example.com", "https://example.com/path", true),
            ("http://example.com:80", "http://example.com/path", true),
            ("http://example.com", "http://example.com:80/path", true),
            ("http://example.com:80", "https://example.com/path", false),
            ("https://example.com:443", "http://example.com/path", false),
            (
                "https://example.com",
                "https://example.com:8443/path",
                false,
            ),
            (
                "https://example.com:8443",
                "https://example.com:8443/path",
                true,
            ),
            ("http://example.com:443", "https://example.com/path", true),
            ("https://example.com:80", "http://example.com/path", true),
            (
                "https://example.com",
                "https://sub.example.com:443/path",
                true,
            ),
            (
                "https://example.com",
                "https://otherexample.com:443/path",
                false,
            ),
        ] {
            let list = [entry.parse().unwrap()];
            assert_eq!(
                url_match(&list, &target.parse().unwrap()),
                expected,
                "{entry} vs {target}"
            );
        }
    }
}
