//! Contact form plugin for Accent CMS, on the WebAssembly Component Model.
//!
//! Implements the `host-services-route-plugin` world:
//!
//! - `content-hooks::on-render` injects a hidden CSRF token into every
//!   `<form>` element of the rendered page.
//! - `routes::handle` handles `POST /contact-submit`: validates the CSRF token
//!   and form fields, then hands the message to the host's built-in SMTP
//!   service through the typed `host-services::send-mail` capability.
//!
//! The typed WIT records (`content-input`, `request`/`response`, `mail`)
//! replace the hand-written JSON envelope structs the Extism version carried;
//! config arrives through the always-granted `config` capability rather than
//! `extism_pdk::config::get`. The plugin needs no network capability and no
//! `server_port`: the former outbound-HTTP call to the host's own
//! `/_internal/smtp/send` bridge is retired in favour of the direct typed
//! import.

#[allow(warnings)]
mod bindings;

use bindings::accent::plugin::config;
use bindings::accent::plugin::host_services::{self, HostError, Mail};
use bindings::exports::accent::plugin::content_hooks::{ContentInput, Guest as ContentHooks};
use bindings::exports::accent::plugin::routes::{Guest as Routes, Request, Response};

// Feature f210 (Component Model port of the f138c contact-form plugin);
// Feature f270 (SMTP bridge call replaced by the host-services capability).
/// The unit struct the host instantiates. Its `Guest` impls are the plugin.
struct Component;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Parsed `application/x-www-form-urlencoded` submission.
struct FormData {
    name: String,
    email: String,
    message: String,
    csrf_token: String,
}

// ---------------------------------------------------------------------------
// CSRF token
// ---------------------------------------------------------------------------

/// Derive the per-site CSRF token from the `csrf_secret` config value (a
/// deterministic FNV-style hash, matching the previous Extism behaviour).
fn generate_csrf_token() -> String {
    let secret = config::get("csrf_secret").unwrap_or_else(|| "accent-contact-default".into());
    let mut hash: u64 = 0;
    for byte in secret.as_bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(u64::from(*byte));
    }
    format!("{hash:016x}")
}

/// Constant-time comparison of a submitted token against the expected one.
fn validate_csrf_token(token: &str) -> bool {
    let expected = generate_csrf_token();
    if token.len() != expected.len() {
        return false;
    }
    let mut result = 0u8;
    for (a, b) in token.bytes().zip(expected.bytes()) {
        result |= a ^ b;
    }
    result == 0
}

/// Inject a hidden `_csrf` field after every opening `<form ...>` tag.
fn inject_csrf(content: &str) -> String {
    let csrf_token = generate_csrf_token();
    let csrf_field = format!(r#"<input type="hidden" name="_csrf" value="{csrf_token}">"#);

    let mut result = String::with_capacity(content.len() + 200);
    let mut remaining = content;
    while let Some(pos) = remaining.to_lowercase().find("<form") {
        let after_form = &remaining[pos..];
        if let Some(close) = after_form.find('>') {
            result.push_str(&remaining[..pos + close + 1]);
            result.push_str(&csrf_field);
            remaining = &remaining[pos + close + 1..];
        } else {
            result.push_str(remaining);
            remaining = "";
        }
    }
    result.push_str(remaining);
    result
}

// ---------------------------------------------------------------------------
// content-hooks
// ---------------------------------------------------------------------------

impl ContentHooks for Component {
    /// Pass raw markdown through unchanged (CSRF injection happens on render).
    fn on_page_load(input: ContentInput) -> Result<String, String> {
        Ok(input.content)
    }

    /// Inject a CSRF token into the rendered HTML's forms.
    fn on_render(input: ContentInput) -> Result<String, String> {
        Ok(inject_csrf(&input.content))
    }
}

// ---------------------------------------------------------------------------
// routes: POST /contact-submit
// ---------------------------------------------------------------------------

impl Routes for Component {
    fn handle(req: Request) -> Result<Response, String> {
        if req.method != "POST" || req.path != "/contact-submit" {
            return Ok(Response {
                status: 404,
                headers: vec![],
                body: "Not found".to_string(),
            });
        }

        let form = match parse_form_data(&req.body) {
            Ok(f) => f,
            Err(reason) => return Ok(redirect_error(&reason)),
        };

        if !validate_csrf_token(&form.csrf_token) {
            return Ok(redirect_error("csrf"));
        }
        if form.name.trim().is_empty() {
            return Ok(redirect_error("name"));
        }
        if form.email.trim().is_empty() || !form.email.contains('@') {
            return Ok(redirect_error("email"));
        }
        if form.message.trim().is_empty() {
            return Ok(redirect_error("message"));
        }

        let to_email = config::get("to_email").unwrap_or_else(|| "admin@example.com".into());
        let subject_prefix = config::get("subject_prefix").unwrap_or_else(|| "[Contact] ".into());

        let subject = format!("{subject_prefix}{}", form.name);
        let body_text = format!(
            "Name: {}\nEmail: {}\n\nMessage:\n{}",
            form.name, form.email, form.message
        );
        let body_html = format!(
            "<h2>Contact Form Submission</h2>\
             <p><strong>Name:</strong> {}</p>\
             <p><strong>Email:</strong> {}</p>\
             <hr>\
             <p>{}</p>",
            html_escape(&form.name),
            html_escape(&form.email),
            html_escape(&form.message).replace('\n', "<br>")
        );

        // Hand the message to the host's SMTP service through the typed
        // host-services capability. The host knows who is calling; no bridge
        // URL, no server_port, no JSON envelope.
        let mail = Mail {
            to: vec![to_email],
            subject,
            body_text,
            body_html: Some(body_html),
        };
        if let Err(e) = host_services::send_mail(&mail) {
            return Ok(Response {
                status: 302,
                headers: vec![(
                    "Location".to_string(),
                    format!("/contact?error=smtp&detail={}", host_error_code(&e)),
                )],
                body: String::new(),
            });
        }

        Ok(Response {
            status: 302,
            headers: vec![("Location".to_string(), "/contact-sent".to_string())],
            body: String::new(),
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn redirect_error(reason: &str) -> Response {
    Response {
        status: 302,
        headers: vec![("Location".to_string(), format!("/contact?error={reason}"))],
        body: String::new(),
    }
}

/// Stable short code for a host-service failure, carried in the redirect's
/// `detail` query parameter (the typed replacement for the bridge's HTTP
/// status code).
fn host_error_code(e: &HostError) -> &'static str {
    match e {
        HostError::NotPermitted(_) => "not-permitted",
        HostError::NotConfigured(_) => "not-configured",
        HostError::InvalidRequest(_) => "invalid-request",
        HostError::RateLimited(_) => "rate-limited",
        HostError::Internal(_) => "internal",
    }
}

fn parse_form_data(body: &str) -> Result<FormData, String> {
    let mut name = String::new();
    let mut email = String::new();
    let mut message = String::new();
    let mut csrf_token = String::new();

    for pair in body.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or("");
        let value = parts.next().unwrap_or("");
        let decoded = url_decode(value);

        match key {
            "name" => name = decoded,
            "email" => email = decoded,
            "message" => message = decoded,
            "_csrf" => csrf_token = decoded,
            _ => {}
        }
    }

    if csrf_token.is_empty() {
        return Err("csrf".into());
    }

    Ok(FormData {
        name,
        email,
        message,
        csrf_token,
    })
}

/// Percent-decode an `application/x-www-form-urlencoded` value.
///
/// Decoded `%XX` escapes are accumulated as raw **bytes** and the whole buffer
/// is interpreted as UTF-8 at the end, so multi-byte sequences reassemble
/// correctly: `%C3%A9` -> "é". Malformed escapes are emitted verbatim, and any
/// leftover bytes that are not valid UTF-8 fall back to the replacement
/// character rather than panicking.
fn url_decode(input: &str) -> String {
    let input = input.replace('+', " ");
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    bytes.push(byte);
                    continue;
                }
            }
            bytes.push(b'%');
            bytes.extend_from_slice(hex.as_bytes());
        } else {
            let mut buf = [0u8; 4];
            bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// Wires `Component` into the generated component exports. Required by
// cargo-component; keep it as the last item in the crate.
bindings::export!(Component with_types_in bindings);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_decode_ascii_and_plus() {
        assert_eq!(url_decode("hello+world"), "hello world");
        assert_eq!(url_decode("a%20b"), "a b");
        assert_eq!(url_decode("plain"), "plain");
    }

    #[test]
    fn url_decode_multibyte_utf8() {
        assert_eq!(url_decode("Andr%C3%A9"), "André");
        assert_eq!(url_decode("M%C3%BCller"), "Müller");
        assert_eq!(url_decode("%E2%82%AC"), "€");
        assert_eq!(url_decode("%F0%9F%98%80"), "😀");
    }

    #[test]
    fn url_decode_malformed_escape_passed_through() {
        assert_eq!(url_decode("100%"), "100%");
        assert_eq!(url_decode("50%2"), "50%2");
        assert_eq!(url_decode("%zz"), "%zz");
    }

    #[test]
    fn url_decode_form_field_with_accent() {
        assert_eq!(
            url_decode("Gr%C3%BCezi+from+Z%C3%BCrich"),
            "Grüezi from Zürich"
        );
    }
}
