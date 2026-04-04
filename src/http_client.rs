/// Shared HTTP client for the entire process.
///
/// `reqwest::Client::new()` initializes the TLS backend (OpenSSL via native-tls).
/// Calling it more than once per process causes state corruption in certain
/// container environments, leading to SIGSEGV. We use `OnceLock` to ensure
/// initialization happens exactly once, and rustls-tls to avoid OpenSSL entirely.
use std::sync::OnceLock;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Returns a clone of the shared process-wide HTTP client (rustls backend).
pub fn get() -> reqwest::Client {
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .use_rustls_tls()
                .build()
                .expect("failed to build HTTP client")
        })
        .clone()
}
