//! The host wiring, tested against hand-written WAT.
//!
//! None of this needs the vendored 66 MB module: what is being asserted is
//! OUR plumbing — that WASI links, that argv[0] is injected, and that a guest
//! exit is read as a status rather than mistaken for a crash. A ten-line module
//! proves all three in milliseconds.

use apitoolchain_rest::{run_wasm, Error};

/// Exits with its own argc, so the returned status IS the argument count the
/// guest actually saw. That makes one module prove WASI linking, argv injection
/// and exit-status extraction at once.
const ECHO_ARGC: &str = r#"
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (drop (call $args_sizes_get (i32.const 0) (i32.const 8)))
    (call $exit (i32.load (i32.const 0)))))
"#;

const EXIT_WITH: &str = r#"
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start") (call $exit (i32.const 7))))
"#;

const TRAPS: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "_start") unreachable))
"#;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn argv0_is_injected_so_the_guest_sees_one_more_argument() {
    let wasm = wat::parse_str(ECHO_ARGC).expect("valid wat");

    // Two user arguments + the injected "openapi" = 3. If argv[0] were missing,
    // cobra would consume the first real argument as the program name and every
    // command would silently become its own subcommand.
    assert_eq!(run_wasm(&wasm, &args(&["spec", "validate"])).unwrap(), 3);
    assert_eq!(run_wasm(&wasm, &args(&[])).unwrap(), 1, "argv0 alone");
}

#[test]
fn a_guest_exit_is_a_status_not_an_error() {
    // Go's wasip1 runtime ends EVERY run via proc_exit, success included, and
    // wasmtime surfaces that as Err(I32Exit). Without the downcast in run.rs a
    // perfectly successful `spec validate` would be reported as a crash.
    let wasm = wat::parse_str(EXIT_WITH).expect("valid wat");
    assert_eq!(run_wasm(&wasm, &args(&[])).unwrap(), 7);
}

#[test]
fn a_real_trap_is_not_mistaken_for_an_exit() {
    // The other half of the same downcast: if it were written to treat any Err
    // as a status, a genuine crash would be reported as a plausible exit code.
    let wasm = wat::parse_str(TRAPS).expect("valid wat");
    match run_wasm(&wasm, &args(&[])) {
        Err(Error::Trap(_)) => {}
        other => panic!("expected a trap, got {other:?}"),
    }
}

#[test]
fn a_module_that_is_not_wasm_fails_as_a_host_error() {
    match run_wasm(b"not a wasm module at all", &args(&[])) {
        Err(Error::Host(_)) => {}
        other => panic!("expected a host error, got {other:?}"),
    }
}
