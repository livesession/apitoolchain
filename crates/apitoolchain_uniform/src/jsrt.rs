//! Shared JS-runtime semantics helpers for converter crates (S6+ W3).
//! These reproduce load-bearing JavaScript behaviors the JS oracles encode.
//! (openapi / openapi2opensdk carry local copies from earlier waves —
//! consolidation onto this module is a reap-time cleanup, not a blocker.)

use serde_json::{Map, Value};

/// `Object.keys()` ordering for a JSON-derived object: array-index keys
/// (canonical unsigned ints < 2^32-1) ascending FIRST, then remaining keys
/// in insertion order.
pub fn js_object_keys(map: &Map<String, Value>) -> Vec<&String> {
    let mut numeric: Vec<(&String, u32)> = Vec::new();
    let mut rest: Vec<&String> = Vec::new();
    for k in map.keys() {
        match as_array_index(k) {
            Some(n) => numeric.push((k, n)),
            None => rest.push(k),
        }
    }
    numeric.sort_by_key(|(_, n)| *n);
    numeric.into_iter().map(|(k, _)| k).chain(rest).collect()
}

fn as_array_index(key: &str) -> Option<u32> {
    if key.is_empty() || key.len() > 10 {
        return None;
    }
    if key != "0" && key.starts_with('0') {
        return None;
    }
    let n: u64 = key.parse().ok()?;
    if n < u32::MAX as u64 {
        Some(n as u32)
    } else {
        None
    }
}

/// JS truthiness for a JSON value gated with a bare `if (x)`.
pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Some(_) => true, // objects and arrays (even empty) are truthy
    }
}

/// Node's `path.join` (POSIX), faithfully.
///
/// The previous port narrowed it to "the docs-relative paths the uniform plugins
/// build" and dropped three behaviours that Node has. None of them can be
/// observed with today's inputs — every `route` in the repo and the fixtures is
/// slash-free — which is exactly what made them dangerous: the first route
/// written as `/docs` would have silently lost its leading slash and produced a
/// relative URL, with no test to catch it. Verified against `node -e` for each
/// case below.
///
/// - `join("/docs", "a")` -> `/docs/a`   (leading slash PRESERVED)
/// - `join("a", "b/")`    -> `a/b/`      (trailing slash PRESERVED)
/// - `join("a", "..", "b")` -> `b`       (`..` RESOLVED)
/// - `join("a", "", "b")` / `join("a", ".", "b")` -> `a/b`
/// - `join()` / all-empty -> `.`
pub fn node_path_join(parts: &[&str]) -> String {
    let joined = parts
        .iter()
        .filter(|p| !p.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("/");
    if joined.is_empty() {
        return ".".to_string();
    }

    let absolute = joined.starts_with('/');
    let trailing = joined.ends_with('/');

    let mut segs: Vec<&str> = Vec::new();
    for seg in joined.split('/') {
        match seg {
            "" | "." => continue,
            ".." => {
                // Node pops a real segment; at the root (or with nothing to pop
                // on a relative path) it keeps the `..`.
                if segs.last().is_some_and(|s| *s != "..") {
                    segs.pop();
                } else if !absolute {
                    segs.push("..");
                }
            }
            _ => segs.push(seg),
        }
    }

    let mut out = segs.join("/");
    if out.is_empty() {
        return if absolute {
            "/".to_string()
        } else {
            ".".to_string()
        };
    }
    if trailing {
        out.push('/');
    }
    if absolute {
        out.insert(0, '/');
    }
    out
}

/// The character set JavaScript's `\s` regex class matches (no `u` flag).
pub fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r' | ' ' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

#[cfg(test)]
mod path_join_tests {
    use super::node_path_join;

    /// Every expectation here was produced by real `node -e "require('path').join(...)"`,
    /// not by reading the docs. The first three are the cases the previous
    /// narrowed port got wrong; they were unobservable with the repo's current
    /// inputs, which is why they needed a test rather than a comment.
    #[test]
    fn matches_node_path_join() {
        let cases: &[(&[&str], &str)] = &[
            (&["/docs", "a"], "/docs/a"), // leading slash preserved
            (&["a", "b/"], "a/b/"),       // trailing slash preserved
            (&["a", "..", "b"], "b"),     // .. resolved
            (&["a", "", "b"], "a/b"),     // empty segment dropped
            (&["a", ".", "b"], "a/b"),    // . dropped
            (&["a/b", "c"], "a/b/c"),     // duplicate separators collapse
            (&["a//b", "c"], "a/b/c"),
            (&["docs/api", "todos"], "docs/api/todos"),
            (&["/a", "..", "b"], "/b"), // .. under an absolute root
            (&["..", "a"], "../a"),     // leading .. on a relative path survives
            (&[""], "."),               // Node: join("") === "."
            (&[], "."),
        ];
        for (input, want) in cases {
            assert_eq!(
                node_path_join(input),
                *want,
                "node_path_join({input:?}) should equal path.join(...) === {want:?}"
            );
        }
    }
}
