//! The engine behind `api rest` — the upstream OpenAPI CLI, embedded as wasm.
//!
//! `github.com/speakeasy-api/openapi` (MIT) is a Go CLI. Rather than reimplement
//! validate/lint/bundle, or shell out to a binary the user has to install, its
//! `cmd/openapi` is compiled to `GOOS=wasip1 GOARCH=wasm` and embedded here.
//! `api rest <args…>` forwards argv to it verbatim, so every upstream command
//! and flag works and nothing drifts as upstream evolves.
//!
//! See `wasm/openapi/README.md` for how the module is produced and pinned.

use std::fmt;

mod cache;
mod run;
mod wasi;

/// The vendored module, zstd-compressed.
///
/// Raw it is ~66 MB, which would make `api` a ~70 MB download. Compressed it is
/// ~8 MB, and the cost is paid only on a cache miss — once the compiled artifact
/// is cached these bytes are never touched again.
const MODULE_ZSTD: &[u8] = include_bytes!("openapi.wasm.zst");

/// Bumped whenever the vendored module changes. Part of the cache key, so a new
/// module can never be run against an artifact compiled from the old one.
const MODULE_VERSION: &str = include_str!("openapi.wasm.version");

/// A host-side failure. Guest-side failures are not errors here — the guest
/// prints its own diagnostics and exits, and that status comes back as `Ok`.
#[derive(Debug)]
pub enum Error {
    /// The module could not be decompressed, compiled or instantiated.
    Host(String),
    /// The guest trapped — a genuine crash, not an exit.
    Trap(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Host(message) => write!(f, "{message}"),
            Error::Trap(message) => write!(f, "the rest engine crashed: {message}"),
        }
    }
}

impl std::error::Error for Error {}

/// Run the embedded OpenAPI CLI with `args` (excluding argv[0]).
///
/// Returns the guest's exit status. A non-zero status is `Ok(code)`, not an
/// error: the guest has already printed its own diagnostics, and the caller's
/// job is to exit with that code rather than to report a failure on top of it.
pub fn run(args: &[String]) -> Result<i32, Error> {
    run::run(run::Source::Cached(MODULE_ZSTD), args)
}

/// Run an arbitrary wasip1 command module under the same host wiring.
///
/// Public so the host wiring — argv injection, WASI linking, exit-status
/// extraction — can be tested against a ten-line WAT module rather than the
/// real 66 MB one. Every assertion about this crate's plumbing goes through
/// here; nothing needs the vendored blob.
pub fn run_wasm(wasm: &[u8], args: &[String]) -> Result<i32, Error> {
    run::run(run::Source::Raw(wasm), args)
}

/// Decompress the vendored module. Called only on a cache miss.
fn decompress(compressed: &[u8]) -> Result<Vec<u8>, Error> {
    zstd::decode_all(compressed)
        .map_err(|e| Error::Host(format!("cannot decompress the rest engine: {e}")))
}

/// The vendored module's version string, for diagnostics.
pub fn module_version() -> &'static str {
    MODULE_VERSION.trim()
}
