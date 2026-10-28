//! Fixture parity for the agent emitters: `mcp::uniform_to_mcp_tools` against
//! the `5.mcp-tools.*` oracles BYTE FOR BYTE (the pretty-printed JSON text,
//! key order included), `skill::skill_files` against the `6.skill.*` output
//! trees FILE FOR FILE, LINE FOR LINE. The oracles are the TypeScript
//! implementation's output — regen JS-side only:
//!   UNIFORM_BUILD_FIXTURES=1 pnpm vitest run
//!
//! Each case's options.json carries `options` (deserialized as they are) plus
//! the keys the vitest harness (packages/apitoolchain-uniform/__tests__/
//! utils.ts) turns into callbacks; they are rebuilt here the same way.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use apitoolchain_uniform::mcp::{uniform_to_mcp_tools, McpToolsOptions, ReferencePredicate};
use apitoolchain_uniform::skill::{skill_files, SkillOptions, SkillReferencesOptions};
use serde_json::{Map, Value};

fn fixture(name: &str) -> PathBuf {
    apitoolchain_parity::fixtures_dir(env!("CARGO_MANIFEST_DIR"), "apitoolchain-uniform").join(name)
}

fn read_json(path: &Path) -> Value {
    let raw =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn input(case: &Path) -> Vec<Value> {
    match read_json(&case.join("input.json")) {
        Value::Array(items) => items,
        other => panic!("input.json is not a Reference[]: {other}"),
    }
}

fn options_fixture(case: &Path) -> Value {
    let path = case.join("options.json");
    if path.exists() {
        read_json(&path)
    } else {
        Value::Object(Map::new())
    }
}

fn canonical(reference: &Value) -> String {
    reference
        .get("canonical")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string()
}

fn strings(value: Option<&Value>) -> Option<Vec<String>> {
    value.and_then(|v| v.as_array()).map(|items| {
        items
            .iter()
            .filter_map(|i| i.as_str().map(String::from))
            .collect()
    })
}

fn exclude(fixture: &Value) -> Option<ReferencePredicate> {
    let list = strings(fixture.get("exclude"))?;
    Some(Arc::new(move |r: &Value| !list.contains(&canonical(r))))
}

fn mcp_options(fixture: &Value) -> McpToolsOptions {
    let mut options: McpToolsOptions = serde_json::from_value(
        fixture
            .get("options")
            .cloned()
            .unwrap_or(Value::Object(Map::new())),
    )
    .expect("options deserialize");
    if let Some(list) = strings(fixture.get("readOnly")) {
        options.read_only = Some(Arc::new(move |r: &Value| {
            r.pointer("/context/method").and_then(|m| m.as_str()) == Some("get")
                || list.contains(&canonical(r))
        }));
    }
    options.include = exclude(fixture);
    if let Some(names) = fixture.get("toolNames").cloned() {
        options.tool_name = Some(Arc::new(move |r: &Value| {
            names
                .get(canonical(r))
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string()
        }));
    }
    if let Some(extras) = fixture.get("extras").cloned() {
        options.extras = Some(Arc::new(move |r: &Value| {
            extras
                .get(canonical(r))
                .and_then(|e| e.as_object())
                .cloned()
        }));
    }
    options
}

fn skill_options(fixture: &Value) -> SkillOptions {
    let mut options: SkillOptions =
        serde_json::from_value(fixture.get("options").cloned().expect("skill options"))
            .expect("options deserialize");
    if let Some(template) = fixture
        .get("header")
        .and_then(|h| h.as_str())
        .map(String::from)
    {
        let mut references = options.references.take().unwrap_or_default();
        references.header = Some(Arc::new(move |group: &str| {
            template.replacen("{group}", group, 1)
        }));
        options.references = Some(SkillReferencesOptions { ..references });
    }
    options.include = exclude(fixture);
    if let Some(groups) = fixture.get("groups").cloned() {
        options.group_by = Some(Arc::new(move |r: &Value| {
            match groups.get(canonical(r)).and_then(|g| g.as_str()) {
                Some(g) => g.to_string(),
                None => r
                    .pointer("/context/group/0")
                    .and_then(|g| g.as_str())
                    .unwrap_or("")
                    .to_string(),
            }
        }));
    }
    options
}

/// The first differing line of two texts, for a readable failure.
fn first_difference(actual: &str, oracle: &str) -> Option<String> {
    let a: Vec<&str> = actual.split('\n').collect();
    let o: Vec<&str> = oracle.split('\n').collect();
    for i in 0..a.len().max(o.len()) {
        let (x, y) = (a.get(i), o.get(i));
        if x != y {
            return Some(format!(
                "line {}:\n    rust:   {}\n    oracle: {}",
                i + 1,
                x.map(|s| format!("{s:?}"))
                    .unwrap_or_else(|| "<end>".into()),
                y.map(|s| format!("{s:?}"))
                    .unwrap_or_else(|| "<end>".into()),
            ));
        }
    }
    None
}

fn run_mcp(name: &str) {
    let case = fixture(name);
    let tools = uniform_to_mcp_tools(&input(&case), Some(mcp_options(&options_fixture(&case))));
    let actual = serde_json::to_string_pretty(&tools).expect("serialize") + "\n";
    if std::env::var("XYD_PARITY_DUMP").as_deref() == Ok("1") {
        std::fs::write(case.join("output.rust.json"), &actual).expect("dump");
    }
    let oracle = std::fs::read_to_string(case.join("output.json")).expect("read oracle");
    if let Some(diff) = first_difference(&actual, &oracle) {
        panic!("{name}: PARITY FAILED (byte for byte) at {diff}");
    }
}

fn list_files(dir: &Path, prefix: &str, out: &mut BTreeMap<String, PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let entry = entry.expect("entry");
        let file_name = entry.file_name().to_string_lossy().to_string();
        let rel = if prefix.is_empty() {
            file_name
        } else {
            format!("{prefix}/{file_name}")
        };
        if entry.path().is_dir() {
            list_files(&entry.path(), &rel, out);
        } else {
            out.insert(rel, entry.path());
        }
    }
}

fn run_skill(name: &str) {
    let case = fixture(name);
    let files = skill_files(&input(&case), skill_options(&options_fixture(&case)));
    let mut oracle = BTreeMap::new();
    list_files(&case.join("output"), "", &mut oracle);
    assert_eq!(
        files.keys().collect::<Vec<_>>(),
        oracle.keys().collect::<Vec<_>>(),
        "{name}: the file set differs"
    );
    for (path, contents) in &files {
        let expected = std::fs::read_to_string(&oracle[path]).expect("read oracle");
        if let Some(diff) = first_difference(contents, &expected) {
            panic!("{name}: PARITY FAILED in {path} at {diff}");
        }
    }
}

#[test]
fn mcp_tools_default() {
    run_mcp("5.mcp-tools.default");
}

#[test]
fn mcp_tools_options() {
    run_mcp("5.mcp-tools.options");
}

#[test]
fn mcp_tools_grouped() {
    run_mcp("5.mcp-tools.grouped");
}

#[test]
fn skill_default() {
    run_skill("6.skill.default");
}

#[test]
fn skill_options_case() {
    run_skill("6.skill.options");
}

#[test]
fn skill_per_operation() {
    run_skill("6.skill.per-operation");
}

/// The emitted tools deserialize back into the typed model unchanged.
#[test]
fn mcp_tools_roundtrip_through_the_types() {
    let case = fixture("5.mcp-tools.options");
    let raw = std::fs::read_to_string(case.join("output.json")).expect("read oracle");
    let tools: Vec<apitoolchain_uniform::mcp::McpTool> = serde_json::from_str(&raw).expect("typed");
    assert_eq!(serde_json::to_string_pretty(&tools).unwrap() + "\n", raw);
}
