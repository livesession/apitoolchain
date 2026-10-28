//! The shared Uniform data model (S6+ Rust migration).
//!
//! Mirrors `packages/uniform/src/types.ts` — the normalized API-docs format
//! every converter emits (`Reference` → `Definition` → `DefinitionProperty`).
//! The TS types stay the CANONICAL public contract; this crate is the Rust
//! mirror whose drift alarm is the fixture-parity suite (a serde shape that
//! diverges from the committed `output.json` oracles fails `cargo test`).
//!
//! Serialization rules (parity-load-bearing):
//! - Every optional field is `Option` + `skip_serializing_if` — the JS
//!   implementations OMIT `undefined` keys (`JSON.stringify` drops them), so
//!   omission-not-null is the correct default.
//! - Open-ended fields (`context`, `Meta.value`, example contexts) are
//!   `serde_json::Value` — their shapes vary per source format.
//! - Function-valued JS fields (`__UNSAFE_selector`, `__UNSAFE_refPath`) never
//!   serialize in fixtures → they are NOT modeled here; converter crates keep
//!   such bookkeeping in internal side-tables.
//!
//! Beside the model: the agent emitters [`mcp`] (`Reference[]` → MCP tool
//! definitions) and [`skill`] (`Reference[]` → an agent skill's files), twins
//! of the package's pure-JavaScript `uniformToMcpTools` and `skillFiles`.

pub mod canon;
pub mod converters;
pub mod jsrt;
pub mod mcp;
mod operations;
pub mod plugins;
pub mod skill;
mod types;

pub use types::*;
