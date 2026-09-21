//! Instantiate the module and call `_start`.

use wasmtime::{Config, Engine, Linker, Store};
use wasmtime_wasi::p1::{self, WasiP1Ctx};
use wasmtime_wasi::{FsPerms, WasiCtxBuilder};

use crate::wasi;
use crate::Error;

/// Run a wasip1 command module with `args` (argv[0] is added here).
///
/// Separated from [`crate::run`] so the host wiring can be tested against a
/// ten-line WAT module instead of the real 66 MB one.
/// Where the module comes from.
pub(crate) enum Source<'a> {
    /// The vendored, zstd-compressed module — decompressed only on a cache miss.
    Cached(&'a [u8]),
    /// Raw wasm, compiled every time. Used by the host-wiring tests.
    Raw(&'a [u8]),
}

pub(crate) fn run(source: Source<'_>, args: &[String]) -> Result<i32, Error> {
    // On a dedicated thread, for two independent reasons.
    //
    // 1. CORRECTNESS. The CLI's `main` is `#[tokio::main]`, so this runs on a
    //    tokio worker. wasmtime-wasi's synchronous p1 API drives async WASI by
    //    calling `block_on` internally, and tokio panics outright when a
    //    runtime is entered from a thread already inside one:
    //    "Cannot start a runtime from within a runtime."
    // 2. STACK. Go's wasm is compiled recursion-heavy and a deeply nested
    //    schema can push Cranelift-generated frames further than the 2 MB a
    //    default spawned thread gets. 16 MB costs nothing (it is virtual,
    //    committed on demand) and removes a class of failure that would
    //    otherwise appear only on a customer's largest spec.
    //
    // Scoped, so the module bytes can be borrowed rather than copied — at 66 MB
    // a clone per invocation is worth avoiding.
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .name("api-rest".into())
            .stack_size(16 * 1024 * 1024)
            .spawn_scoped(scope, || run_blocking(&source, args))
            .map_err(|e| Error::Host(format!("cannot start the rest engine thread: {e}")))?;
        handle
            .join()
            .map_err(|_| Error::Host("the rest engine thread panicked".into()))?
    })
}

fn run_blocking(source: &Source<'_>, args: &[String]) -> Result<i32, Error> {
    let engine =
        Engine::new(Config::new().wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Disable))
            .map_err(|e| Error::Host(format!("cannot start the wasm engine: {e}")))?;

    let module = match source {
        Source::Cached(compressed) => crate::cache::load(&engine, compressed)?,
        Source::Raw(wasm) => wasmtime::Module::new(&engine, wasm)
            .map_err(|e| Error::Host(format!("cannot load the rest engine: {e}")))?,
    };

    let cwd = std::env::current_dir()
        .map_err(|e| Error::Host(format!("cannot read the working directory: {e}")))?;
    let plan = wasi::plan(args, &cwd, std::env::vars_os());

    let mut builder = WasiCtxBuilder::new();
    builder.args(&plan.argv);
    for (key, value) in &plan.env {
        builder.env(key, value);
    }
    builder.inherit_stdio();
    let (host, guest) = &plan.preopen;
    // ReadWrite: `-w` rewrites in place and `spec bundle <in> <out>` writes a
    // new file. Read-only would break both.
    builder
        .preopened_dir(host, guest.to_string_lossy().as_ref(), FsPerms::ReadWrite)
        .map_err(|e| Error::Host(format!("cannot open {}: {e}", host.display())))?;

    let mut linker: Linker<WasiP1Ctx> = Linker::new(&engine);
    p1::add_to_linker_sync(&mut linker, |ctx| ctx)
        .map_err(|e| Error::Host(format!("cannot wire WASI: {e}")))?;

    let mut store = Store::new(&engine, builder.build_p1());
    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| Error::Host(format!("cannot instantiate the rest engine: {e}")))?;
    let start = instance
        .get_typed_func::<(), ()>(&mut store, "_start")
        .map_err(|e| Error::Host(format!("the rest engine has no _start: {e}")))?;

    // Anything the host buffered must land before the guest writes to the same
    // fds, or the two interleave out of order.
    flush_host_stdio();

    match start.call(&mut store, ()) {
        // Go's wasip1 runtime always ends via proc_exit, so a clean return is
        // not the success path — but treat it as success if it ever happens.
        Ok(()) => Ok(0),
        Err(err) => exit_status(err),
    }
}

/// Turn a `_start` error into an exit status, or a real trap.
///
/// Go's wasip1 runtime calls `proc_exit` for EVERY termination, success
/// included, and wasmtime surfaces that as an `Err` carrying `I32Exit`. Without
/// this downcast a successful run looks exactly like a crash.
fn exit_status(err: wasmtime::Error) -> Result<i32, Error> {
    if let Some(exit) = err.downcast_ref::<wasmtime_wasi::I32Exit>() {
        return Ok(exit.0);
    }
    Err(Error::Trap(format!("{err:?}")))
}

fn flush_host_stdio() {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
}
