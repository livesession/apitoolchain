//! Compile the module once per machine, not once per invocation.
//!
//! Cranelift needs ~5 seconds on this 66 MB module (~19 s in a debug build).
//! Per command that is unusable, so the compiled artifact is written to the
//! user's cache directory and mmap'd thereafter.
//!
//! Shipping a prebuilt artifact instead is not an option: Cranelift output runs
//! 1.5–3x the wasm size, which would put a 70–200 MB file in the repo and in
//! every release binary — and it would be per-architecture besides.

use std::hash::{Hash, Hasher};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use wasmtime::{Engine, Module};

use crate::Error;

/// Load the module, preferring a previously compiled artifact.
///
/// Every failure on the cache path falls back to compiling: a cache that cannot
/// be read or written must never be the reason the command fails.
pub(crate) fn load(engine: &Engine, compressed: &[u8]) -> Result<Module, Error> {
    let path = artifact_path(engine, compressed);

    if let Some(path) = &path {
        // SAFETY: this file is written only by `store` below, from
        // `Module::serialize` on an engine whose compatibility hash is part of
        // the filename. wasmtime validates the artifact header and REJECTS a
        // foreign, stale or truncated one rather than executing it, so a
        // corrupted cache costs a recompile, not undefined behaviour.
        if let Ok(module) = unsafe { Module::deserialize_file(engine, path) } {
            return Ok(module);
        }
    }

    // Only now is the module actually needed. Decompressing 8 MB into 66 MB
    // costs ~2 s of CPU, so doing it before the cache lookup would make every
    // warm run pay for bytes it never looks at.
    first_run_notice();
    let wasm = crate::decompress(compressed)?;
    let module = Module::new(engine, &wasm)
        .map_err(|e| Error::Host(format!("cannot load the rest engine: {e}")))?;

    if let Some(path) = &path {
        // Best effort. A read-only HOME, a full disk or a race all just mean
        // the next run pays the compile again.
        let _ = store(&module, path);
    }
    Ok(module)
}

/// Where this engine's artifact lives, if a cache directory is usable at all.
///
/// The filename carries everything that would invalidate it:
///
/// * `precompile_compatibility_hash` — wasmtime's own documented key, covering
///   its version, the `Config` and the target. Its docs: *"If this Hash matches
///   between two Engines then binaries from one are guaranteed to deserialize
///   in the other."*
/// * the vendored module's version, so re-vendoring cannot reuse an artifact
///   compiled from the previous module;
/// * the COMPRESSED blob's length, which catches a module swapped WITHOUT a
///   version bump. The compressed length is used because it is the only one
///   available without decompressing, which is the whole point of this path.
fn artifact_path(engine: &Engine, compressed: &[u8]) -> Option<PathBuf> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    engine.precompile_compatibility_hash().hash(&mut hasher);
    crate::MODULE_VERSION.hash(&mut hasher);
    compressed.len().hash(&mut hasher);
    Some(cache_dir()?.join(format!("openapi-{:016x}.cwasm", hasher.finish())))
}

/// `$APITOOLCHAIN_REST_CACHE_DIR`, else the platform cache directory.
///
/// Hand-rolled rather than pulling in `dirs`: it is fifteen lines and this
/// crate's dependency footprint is already dominated by a wasm engine.
fn cache_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("APITOOLCHAIN_REST_CACHE_DIR") {
        return Some(PathBuf::from(dir));
    }
    let base = if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Caches")
    } else if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(xdg)
    } else {
        PathBuf::from(std::env::var_os("HOME")?).join(".cache")
    };
    Some(base.join("apitoolchain").join("rest"))
}

/// Write the artifact atomically.
///
/// Temp file then rename, so a concurrent reader never sees a half-written
/// file. Two racing processes both write valid artifacts and the later rename
/// wins; both are correct, so no locking is needed.
fn store(module: &Module, path: &Path) -> Result<(), Error> {
    let bytes = module
        .serialize()
        .map_err(|e| Error::Host(format!("cannot serialize the rest engine: {e}")))?;
    let dir = path
        .parent()
        .ok_or_else(|| Error::Host("the cache path has no parent".into()))?;
    std::fs::create_dir_all(dir).map_err(|e| Error::Host(format!("cannot create {dir:?}: {e}")))?;

    let temp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&temp, &bytes)
        .map_err(|e| Error::Host(format!("cannot write {temp:?}: {e}")))?;
    std::fs::rename(&temp, path).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        Error::Host(format!("cannot install {path:?}: {e}"))
    })
}

/// Tell the user why the first run is slow — but only on a terminal, so a pipe
/// or a CI log never sees it.
fn first_run_notice() {
    if std::io::stderr().is_terminal() {
        eprintln!("api: preparing the rest engine (one-time, this takes a few seconds)…");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cache_directory_is_overridable() {
        // Needed by the tests below and by anyone sandboxing the CLI.
        let key = "APITOOLCHAIN_REST_CACHE_DIR";
        let previous = std::env::var_os(key);
        // SAFETY: single-threaded test; restored below.
        unsafe { std::env::set_var(key, "/tmp/some-cache") };
        assert_eq!(cache_dir(), Some(PathBuf::from("/tmp/some-cache")));
        match previous {
            Some(value) => unsafe { std::env::set_var(key, value) },
            None => unsafe { std::env::remove_var(key) },
        }
    }

    #[test]
    fn the_artifact_name_changes_with_the_module() {
        // The failure this prevents: re-vendoring the module and silently
        // running the artifact compiled from the previous one.
        let engine = Engine::default();
        let a = artifact_path(&engine, &[0u8; 10]).unwrap();
        let b = artifact_path(&engine, &[0u8; 11]).unwrap();
        assert_ne!(a, b, "a different module must not reuse an artifact");
    }
}
