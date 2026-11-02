## 2024-11-02 - Missing HSTS and XSS headers
**Vulnerability:** The application is missing important security headers in `crates/server/src/security.rs`.
**Learning:** `security_headers_middleware` should add `Strict-Transport-Security`, `X-XSS-Protection`, and `Content-Security-Policy`.
**Prevention:** Add a comprehensive set of headers in middleware.
