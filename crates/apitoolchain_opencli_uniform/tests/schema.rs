//! The fixture inputs are valid OpenCLI documents: each passes the base
//! schema (`packages/apitoolchain-opencli/opencli-spec.json`) and the generic
//! extensions schema (`opencli-extensions.schema.json` beside it), the
//! contract for `x-interactive`, `x-environment` and `x-passthrough`.
//!
//! The extensions schema is vendored byte for byte from opensdk; its source
//! (path, commit, sha256) is recorded in `opencli-extensions.schema.source.json`.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const BASE_SCHEMA: &str = "opencli-spec.json";
const EXTENSIONS_SCHEMA: &str = "opencli-extensions.schema.json";
const EXTENSIONS_SOURCE: &str = "opencli-extensions.schema.source.json";

fn package_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("packages")
        .join("apitoolchain-opencli")
}

fn read(path: &Path) -> Value {
    let raw =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn validator(schema_file: &str) -> jsonschema::Validator {
    let schema = read(&package_dir().join(schema_file));
    jsonschema::draft202012::new(&schema)
        .unwrap_or_else(|e| panic!("{schema_file} does not compile: {e}"))
}

fn errors(validator: &jsonschema::Validator, doc: &Value) -> Vec<String> {
    validator
        .iter_errors(doc)
        .map(|e| format!("{e} at {}", e.instance_path()))
        .collect()
}

/// The fixture cases (directories with an `input.json`), sorted.
fn inputs() -> Vec<PathBuf> {
    let dir = package_dir().join("__fixtures__").join("references");
    let mut cases: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read fixtures dir {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path().join("input.json"))
        .filter(|p| p.exists())
        .collect();
    cases.sort();
    cases
}

fn case_input(name: &str) -> Value {
    read(
        &package_dir()
            .join("__fixtures__")
            .join("references")
            .join(name)
            .join("input.json"),
    )
}

#[test]
fn the_vendored_extensions_schema_is_its_source_byte_for_byte() {
    let source = read(&package_dir().join(EXTENSIONS_SOURCE));
    for key in ["from", "path", "commit", "sha256"] {
        assert!(
            source[key].as_str().is_some_and(|s| !s.is_empty()),
            "{EXTENSIONS_SOURCE}: `{key}` is missing"
        );
    }
    let commit = source["commit"].as_str().unwrap();
    assert!(
        commit.len() == 40 && commit.chars().all(|c| c.is_ascii_hexdigit()),
        "{EXTENSIONS_SOURCE}: `commit` is not a full commit id: {commit}"
    );

    let bytes = std::fs::read(package_dir().join(EXTENSIONS_SCHEMA)).expect("read the schema");
    let actual = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(
        actual,
        source["sha256"].as_str().unwrap(),
        "{EXTENSIONS_SCHEMA} differs from the file its source record names: \
         re-vendor it and update {EXTENSIONS_SOURCE}, never edit it here"
    );
}

#[test]
fn every_fixture_input_passes_the_base_and_the_extensions_schema() {
    let base = validator(BASE_SCHEMA);
    let extensions = validator(EXTENSIONS_SCHEMA);
    let inputs = inputs();
    // Same corpus as tests/references.rs; keep the numbers in step.
    assert_eq!(inputs.len(), 19, "expected the 19 committed fixture inputs");

    // Non-vacuity: the base schema does reject a document (one without `info`).
    assert!(!errors(&base, &json!({ "opencli": "1.0.0" })).is_empty());

    let mut failures: Vec<String> = Vec::new();
    for input in &inputs {
        let doc = read(input);
        let case = input
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy();
        for (schema, v) in [(BASE_SCHEMA, &base), (EXTENSIONS_SCHEMA, &extensions)] {
            for e in errors(v, &doc) {
                failures.push(format!("  {case} vs {schema}: {e}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "fixture inputs that are not valid OpenCLI:\n{}",
        failures.join("\n")
    );
}

/// The schema check above is not vacuous: the mounted-agent case carries every
/// extension, and on the paths the extensions schema walks.
#[test]
fn the_sections_case_carries_every_extension() {
    let doc = case_input("8.acme-mounted-agent");
    let agent = doc["commands"]
        .as_array()
        .and_then(|cs| cs.iter().find(|c| c["name"] == "agent"))
        .expect("the agent command");
    assert!(agent["x-passthrough"].is_object());
    assert!(agent["x-interactive"]["commands"].is_array());
    assert!(agent["x-interactive"]["keys"].is_array());
    assert!(agent["x-environment"].is_array());
    let login = doc["commands"]
        .as_array()
        .and_then(|cs| cs.iter().find(|c| c["name"] == "login"))
        .expect("the login command");
    assert!(login["x-environment"].is_array());
}

/// Breaks one rule of the extensions contract in a valid fixture.
type Breaker = fn(&mut Value);

#[test]
fn the_extensions_schema_rejects_a_broken_extension() {
    let extensions = validator(EXTENSIONS_SCHEMA);
    let original = case_input("8.acme-mounted-agent");
    assert_eq!(errors(&extensions, &original), Vec::<String>::new());

    let agent_index = original["commands"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| c["name"] == "agent")
        .unwrap();
    let cases: Vec<(&str, Breaker)> = vec![
        ("x-passthrough on the root", |d| {
            d["x-passthrough"] = json!({});
        }),
        ("an environment variable without a description", |d| {
            d["commands"][0]["x-environment"] = json!([{ "name": "ACME_TOKEN" }]);
        }),
        ("a key binding without keys", |d| {
            d["x-interactive"] = json!({ "keys": [{ "keys": [], "description": "x" }] });
        }),
        ("a session command without a name", |d| {
            d["x-interactive"] = json!({ "commands": [{ "description": "x" }] });
        }),
    ];
    for (what, break_it) in cases {
        let mut doc = original.clone();
        break_it(&mut doc);
        assert!(
            !errors(&extensions, &doc).is_empty(),
            "{what} should be rejected"
        );
    }

    // And deep in the tree: a passthrough whose program is empty.
    let mut doc = original.clone();
    doc["commands"][agent_index]["x-passthrough"] = json!({ "program": "" });
    assert!(!errors(&extensions, &doc).is_empty());
}
