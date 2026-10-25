//! Page paths — where a reference's page lives beyond the converter's default
//! (`operationId` / `objects/<Name>`): the explicit `x-docs.path` a page may
//! carry, and the canonical the `operationPaths: "title"` option derives from
//! a page's group and its verb-first summary. The x-docs pass applies both
//! (`xdocs.rs`); the JS twin is `impl-js/xdocs/pagePaths.ts`.
//!
//! The grammar of a derived operation path:
//!
//! ```text
//! summary   := Verb Rest
//! Verb      := a word of [`VERBS`], case-insensitive; a third-person "s" /
//!              "es" is dropped ("Lists" → list, "Searches" → search)
//! Rest      := the remaining words, minus [`FUNCTION_WORDS`] and the group's
//!              noun (every word of the group, singular and plural alike);
//!              what is left, lowercased and hyphenated, is the sub-resource
//! canonical := <group>/[<sub-resource>/]<verb>        (the group slugged)
//! ```
//!
//! "Create widget" in `widgets` → `widgets/create`; "List widget parts" →
//! `widgets/parts/list`; "Retrieve part snapshot" →
//! `widgets/part-snapshot/retrieve`. A summary that does not start with a
//! verb, or an operation without a group, keeps the converter's canonical.
//!
//! A component page derives `<group>/<label>` — the label its constant
//! `object` value VERBATIM (`widgets/widget`, `widgets/widget_part`), else its
//! name in kebab-case (`widgets/part-snapshot`) — and `objects/<label>` without
//! a group (`objects/color`).

use crate::util::slug;

/// The verbs a verb-first summary may start with (lowercase). A summary whose
/// first word is not one of these (after the "s"/"es" rule) is not verb-first
/// and the operation keeps its default canonical.
pub const VERBS: &[&str] = &[
    "accept",
    "activate",
    "add",
    "approve",
    "archive",
    "assign",
    "attach",
    "cancel",
    "check",
    "clear",
    "clone",
    "close",
    "complete",
    "confirm",
    "copy",
    "count",
    "create",
    "deactivate",
    "decline",
    "delete",
    "detach",
    "disable",
    "download",
    "duplicate",
    "enable",
    "execute",
    "export",
    "fetch",
    "finalize",
    "find",
    "generate",
    "get",
    "import",
    "invite",
    "link",
    "list",
    "lock",
    "mark",
    "merge",
    "move",
    "open",
    "patch",
    "pause",
    "preview",
    "publish",
    "query",
    "read",
    "refresh",
    "regenerate",
    "register",
    "reject",
    "release",
    "remove",
    "rename",
    "reopen",
    "replace",
    "reset",
    "resend",
    "resolve",
    "restore",
    "resume",
    "retrieve",
    "retry",
    "revoke",
    "rotate",
    "run",
    "search",
    "send",
    "set",
    "share",
    "start",
    "stop",
    "submit",
    "subscribe",
    "suspend",
    "sync",
    "test",
    "toggle",
    "trigger",
    "unarchive",
    "unassign",
    "unlink",
    "unlock",
    "unpublish",
    "unsubscribe",
    "update",
    "upload",
    "upsert",
    "validate",
    "verify",
    "void",
];

/// The words a summary's rest drops before it becomes the sub-resource.
pub const FUNCTION_WORDS: &[&str] = &[
    "a", "an", "the", "all", "of", "for", "by", "to", "in", "on", "from", "with",
];

/// An explicit `x-docs.path`, normalised: trimmed, repeated slashes collapsed,
/// no leading or trailing slash (`/widgets//create/` → `widgets/create`).
/// Empty → `None` (the key is ignored).
pub fn explicit_page_path(raw: &str) -> Option<String> {
    let mut collapsed = String::with_capacity(raw.len());
    let mut prev_slash = false;
    for ch in raw.trim().chars() {
        if ch == '/' {
            if !prev_slash {
                collapsed.push('/');
            }
            prev_slash = true;
        } else {
            collapsed.push(ch);
            prev_slash = false;
        }
    }
    let path = collapsed.trim_matches('/');
    (!path.is_empty()).then(|| path.to_string())
}

/// The verb a summary's first word is, or `None` when the summary is not
/// verb-first: the word slugged (lowercase, punctuation dropped), looked up
/// in [`VERBS`] as it is, then minus a trailing "es", then minus a trailing
/// "s" — so "Create", "Lists" and "Searches" are `create`, `list`, `search`.
pub fn verb_of(word: &str) -> Option<&'static str> {
    let w = slug(word);
    let lookup = |candidate: &str| VERBS.iter().copied().find(|v| *v == candidate);
    if let Some(v) = lookup(&w) {
        return Some(v);
    }
    if let Some(stem) = w.strip_suffix("es") {
        if let Some(v) = lookup(stem) {
            return Some(v);
        }
    }
    if let Some(stem) = w.strip_suffix('s') {
        if let Some(v) = lookup(stem) {
            return Some(v);
        }
    }
    None
}

/// The derived canonical of an operation (see the module docs), or `None`
/// when the summary is not verb-first or the group is empty.
pub fn operation_page_path(summary: &str, group: &str) -> Option<String> {
    let group_slug = slug(group.trim());
    if group_slug.is_empty() {
        return None;
    }
    let mut words = summary.split_whitespace();
    let verb = verb_of(words.next()?)?;
    let nouns = group_noun_forms(group);
    let rest: Vec<String> = words
        .map(slug)
        .filter(|w| {
            if w.is_empty() {
                return false;
            }
            let key: String = w.chars().filter(|c| *c != '-' && *c != '_').collect();
            !FUNCTION_WORDS.contains(&key.as_str()) && !nouns.contains(&key)
        })
        .collect();
    let sub_resource = rest.join("-");
    Some(if sub_resource.is_empty() {
        format!("{group_slug}/{verb}")
    } else {
        format!("{group_slug}/{sub_resource}/{verb}")
    })
}

/// The derived canonical of a component page: `<group>/<label>` with a
/// resource group (the group slugged, the label verbatim), `objects/<label>`
/// without one.
pub fn component_page_path(group: Option<&str>, label: &str) -> String {
    match group.map(|g| slug(g.trim())).filter(|g| !g.is_empty()) {
        Some(group) => format!("{group}/{label}"),
        None => format!("objects/{label}"),
    }
}

/// A schema name in kebab-case: `WidgetPart` → `widget-part`, `APIKey` →
/// `api-key`, `Widget_Part` → `widget-part`, `Color` → `color`.
pub fn kebab_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_alphanumeric() {
            if c.is_uppercase() && i > 0 {
                let prev = chars[i - 1];
                let next_lower = chars.get(i + 1).map(|n| n.is_lowercase()).unwrap_or(false);
                let boundary =
                    prev.is_lowercase() || prev.is_numeric() || (prev.is_uppercase() && next_lower);
                if boundary && !out.is_empty() && !out.ends_with('-') {
                    out.push('-');
                }
            }
            out.extend(c.to_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// The noun forms of a group — every word of it (lowercase, alphanumeric),
/// each with its singular and plural: `widgets` → {widgets, widget, …},
/// `companies` → {companies, company, …}. Heuristic English inflection; the
/// odd extra form is harmless (it only ever REMOVES a matching word).
pub fn group_noun_forms(group: &str) -> Vec<String> {
    let mut forms: Vec<String> = Vec::new();
    let mut add = |w: String| {
        if !w.is_empty() && !forms.contains(&w) {
            forms.push(w);
        }
    };
    for word in group
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        add(word.to_string());
        if let Some(singular) = singular_of(word) {
            add(singular);
        }
        add(plural_of(word));
    }
    forms
}

fn singular_of(w: &str) -> Option<String> {
    if w.len() > 3 {
        if let Some(stem) = w.strip_suffix("ies") {
            return Some(format!("{stem}y"));
        }
    }
    for suffix in ["ses", "xes", "zes", "ches", "shes"] {
        if w.len() > suffix.len() && w.ends_with(suffix) {
            return Some(w[..w.len() - 2].to_string());
        }
    }
    if w.len() > 1 && w.ends_with('s') && !w.ends_with("ss") {
        return Some(w[..w.len() - 1].to_string());
    }
    None
}

fn plural_of(w: &str) -> String {
    let chars: Vec<char> = w.chars().collect();
    if let [.., before, 'y'] = chars.as_slice() {
        if !"aeiou".contains(*before) {
            return format!("{}ies", &w[..w.len() - 1]);
        }
    }
    if ["s", "x", "z", "ch", "sh"].iter().any(|s| w.ends_with(s)) {
        return format!("{w}es");
    }
    format!("{w}s")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The derivation table of the module docs, plus the edges: articles,
    /// the group's noun in either number, a third-person verb, punctuation,
    /// a multi-word group, and the two "keep the default" cases.
    #[test]
    fn operation_paths_follow_the_grammar() {
        let cases: &[(&str, &str, Option<&str>)] = &[
            ("Create widget", "widgets", Some("widgets/create")),
            ("Create a widget", "widgets", Some("widgets/create")),
            ("List widgets", "widgets", Some("widgets/list")),
            ("Lists all widgets", "widgets", Some("widgets/list")),
            ("Retrieve widget", "widgets", Some("widgets/retrieve")),
            ("List widget parts", "widgets", Some("widgets/parts/list")),
            (
                "Retrieve part snapshot",
                "widgets",
                Some("widgets/part-snapshot/retrieve"),
            ),
            ("Delete a widget.", "widgets", Some("widgets/delete")),
            (
                "Searches the widget's parts",
                "widgets",
                Some("widgets/parts/search"),
            ),
            (
                "List parts of a widget",
                "widgets",
                Some("widgets/parts/list"),
            ),
            ("Retrieve company", "companies", Some("companies/retrieve")),
            (
                "List company members",
                "companies",
                Some("companies/members/list"),
            ),
            (
                "Create widget part",
                "Widget parts",
                Some("widget-parts/create"),
            ),
            (
                "Retrieve statistics",
                "company statistics",
                Some("company-statistics/retrieve"),
            ),
            // Not verb-first: the default canonical stays.
            ("Widget archival", "widgets", None),
            ("Returns the widget", "widgets", None),
            ("", "widgets", None),
            // No group: the default canonical stays.
            ("Retrieve health", "", None),
            ("Retrieve health", "  ", None),
        ];
        for (summary, group, expected) in cases {
            assert_eq!(
                operation_page_path(summary, group).as_deref(),
                *expected,
                "{summary:?} in {group:?}"
            );
        }
    }

    #[test]
    fn component_paths_use_the_label_verbatim() {
        assert_eq!(
            component_page_path(Some("widgets"), "widget"),
            "widgets/widget"
        );
        assert_eq!(
            component_page_path(Some("widgets"), "widget_part"),
            "widgets/widget_part"
        );
        assert_eq!(
            component_page_path(Some("Widget parts"), "part"),
            "widget-parts/part"
        );
        assert_eq!(component_page_path(None, "color"), "objects/color");
        assert_eq!(component_page_path(Some(""), "color"), "objects/color");
    }

    #[test]
    fn explicit_paths_are_normalised() {
        assert_eq!(
            explicit_page_path("widgets/create").as_deref(),
            Some("widgets/create")
        );
        assert_eq!(
            explicit_page_path(" /widgets//parts/list/ ").as_deref(),
            Some("widgets/parts/list")
        );
        assert_eq!(explicit_page_path(""), None);
        assert_eq!(explicit_page_path("/"), None);
        assert_eq!(explicit_page_path("  //  "), None);
    }

    #[test]
    fn names_kebab_case() {
        assert_eq!(kebab_case("Color"), "color");
        assert_eq!(kebab_case("WidgetPart"), "widget-part");
        assert_eq!(kebab_case("PartSnapshot"), "part-snapshot");
        assert_eq!(kebab_case("APIKey"), "api-key");
        assert_eq!(kebab_case("Widget_Part"), "widget-part");
        assert_eq!(kebab_case("widget part"), "widget-part");
        assert_eq!(kebab_case("OAuth2Token"), "o-auth2-token");
        assert_eq!(kebab_case("widget"), "widget");
    }

    #[test]
    fn verbs_accept_the_third_person() {
        assert_eq!(verb_of("Create"), Some("create"));
        assert_eq!(verb_of("Lists"), Some("list"));
        assert_eq!(verb_of("Searches"), Some("search"));
        assert_eq!(verb_of("Retrieves"), Some("retrieve"));
        assert_eq!(verb_of("Closes"), Some("close"));
        assert_eq!(verb_of("Widget"), None);
        assert_eq!(verb_of("Returns"), None);
    }

    #[test]
    fn group_nouns_cover_both_numbers() {
        let widgets = group_noun_forms("widgets");
        assert!(widgets.contains(&"widgets".to_string()));
        assert!(widgets.contains(&"widget".to_string()));
        let companies = group_noun_forms("companies");
        assert!(companies.contains(&"company".to_string()));
        let company = group_noun_forms("company");
        assert!(company.contains(&"companies".to_string()));
        let statuses = group_noun_forms("statuses");
        assert!(statuses.contains(&"status".to_string()));
        let parts = group_noun_forms("Widget parts");
        assert!(parts.contains(&"widget".to_string()));
        assert!(parts.contains(&"part".to_string()));
        assert!(parts.contains(&"parts".to_string()));
    }
}
