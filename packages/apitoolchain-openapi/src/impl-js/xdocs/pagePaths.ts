import {slug} from "../utils";

// Page paths — where a reference's page lives beyond the converter's default
// (`operationId` / `objects/<Name>`): the explicit `x-docs.path` a page may
// carry, and the canonical the `operationPaths: "title"` option derives from a
// page's group and its verb-first summary. The x-docs pass applies both
// (pluginSidebar.ts); the Rust twin is crates/apitoolchain_openapi/src/page_paths.rs.
//
// The grammar of a derived operation path:
//
//   summary   := Verb Rest
//   Verb      := a word of PAGE_PATH_VERBS, case-insensitive; a third-person
//                "s" / "es" is dropped ("Lists" → list, "Searches" → search)
//   Rest      := the remaining words, minus PAGE_PATH_FUNCTION_WORDS and the
//                group's noun (every word of the group, singular and plural
//                alike); what is left, lowercased and hyphenated, is the
//                sub-resource
//   canonical := <group>/[<sub-resource>/]<verb>        (the group slugged)
//
// "Create widget" in `widgets` → `widgets/create`; "List widget parts" →
// `widgets/parts/list`; "Retrieve part snapshot" → `widgets/part-snapshot/retrieve`.
// A summary that does not start with a verb, or an operation without a group,
// keeps the converter's canonical.
//
// A component page derives `<group>/<label>` — the label its constant `object`
// value VERBATIM (`widgets/widget`, `widgets/widget_part`), else its name in
// kebab-case (`widgets/part-snapshot`) — and `objects/<label>` without a group
// (`objects/color`).

/** The verbs a verb-first summary may start with (lowercase). A summary whose
 *  first word is not one of these (after the "s"/"es" rule) is not verb-first
 *  and the operation keeps its default canonical. */
export const PAGE_PATH_VERBS: readonly string[] = [
    "accept", "activate", "add", "approve", "archive", "assign", "attach",
    "cancel", "check", "clear", "clone", "close", "complete", "confirm", "copy",
    "count", "create", "deactivate", "decline", "delete", "detach", "disable",
    "download", "duplicate", "enable", "execute", "export", "fetch", "finalize",
    "find", "generate", "get", "import", "invite", "link", "list", "lock",
    "mark", "merge", "move", "open", "patch", "pause", "preview", "publish",
    "query", "read", "refresh", "regenerate", "register", "reject", "release",
    "remove", "rename", "reopen", "replace", "reset", "resend", "resolve",
    "restore", "resume", "retrieve", "retry", "revoke", "rotate", "run",
    "search", "send", "set", "share", "start", "stop", "submit", "subscribe",
    "suspend", "sync", "test", "toggle", "trigger", "unarchive", "unassign",
    "unlink", "unlock", "unpublish", "unsubscribe", "update", "upload",
    "upsert", "validate", "verify", "void",
]

/** The words a summary's rest drops before it becomes the sub-resource. */
export const PAGE_PATH_FUNCTION_WORDS: readonly string[] = [
    "a", "an", "the", "all", "of", "for", "by", "to", "in", "on", "from", "with",
]

/** An explicit `x-docs.path`, normalised: trimmed, repeated slashes collapsed,
 *  no leading or trailing slash (`/widgets//create/` → `widgets/create`).
 *  Not a string, or empty → `undefined` (the key is ignored). */
export function explicitPagePath(raw: unknown): string | undefined {
    if (typeof raw !== "string") {
        return undefined
    }
    const path = raw.trim().replace(/\/+/g, "/").replace(/^\/|\/$/g, "")
    return path || undefined
}

/** The verb a summary's first word is, or `undefined` when the summary is not
 *  verb-first: the word slugged (lowercase, punctuation dropped), looked up in
 *  PAGE_PATH_VERBS as it is, then minus a trailing "es", then minus a trailing
 *  "s" — so "Create", "Lists" and "Searches" are `create`, `list`, `search`. */
export function verbOf(word: string): string | undefined {
    const w = slug(word)
    if (PAGE_PATH_VERBS.includes(w)) {
        return w
    }
    if (w.endsWith("es") && PAGE_PATH_VERBS.includes(w.slice(0, -2))) {
        return w.slice(0, -2)
    }
    if (w.endsWith("s") && PAGE_PATH_VERBS.includes(w.slice(0, -1))) {
        return w.slice(0, -1)
    }
    return undefined
}

/** The derived canonical of an operation (see the grammar above), or
 *  `undefined` when the summary is not verb-first or the group is empty. */
export function operationPagePath(summary: string | undefined, group: string | undefined): string | undefined {
    const groupSlug = slug((group ?? "").trim())
    if (!groupSlug || !summary) {
        return undefined
    }
    const words = summary.split(/\s+/).filter(Boolean)
    if (!words.length) {
        return undefined
    }
    const verb = verbOf(words[0])
    if (!verb) {
        return undefined
    }
    const nouns = groupNounForms(group ?? "")
    const rest = words.slice(1).map(w => slug(w)).filter(w => {
        if (!w) {
            return false
        }
        const key = w.replace(/[-_]/g, "")
        return !PAGE_PATH_FUNCTION_WORDS.includes(key) && !nouns.includes(key)
    })
    const subResource = rest.join("-")
    return subResource ? `${groupSlug}/${subResource}/${verb}` : `${groupSlug}/${verb}`
}

/** The derived canonical of a component page: `<group>/<label>` with a
 *  resource group (the group slugged, the label verbatim), `objects/<label>`
 *  without one. */
export function componentPagePath(group: string | undefined, label: string): string {
    const g = slug((group ?? "").trim())
    return g ? `${g}/${label}` : `objects/${label}`
}

const isAlphanumeric = (c: string) => /^[\p{L}\p{N}]$/u.test(c)
const isUpper = (c: string) => /^\p{L}$/u.test(c) && c !== c.toLowerCase()
const isLower = (c: string) => /^\p{L}$/u.test(c) && c !== c.toUpperCase()
const isNumeric = (c: string) => /^\p{N}$/u.test(c)

/** A schema name in kebab-case: `WidgetPart` → `widget-part`, `APIKey` →
 *  `api-key`, `Widget_Part` → `widget-part`, `Color` → `color`. */
export function kebabCase(name: string): string {
    const chars = Array.from(name)
    let out = ""
    for (let i = 0; i < chars.length; i++) {
        const c = chars[i]
        if (isAlphanumeric(c)) {
            if (isUpper(c) && i > 0) {
                const prev = chars[i - 1]
                const nextLower = i + 1 < chars.length && isLower(chars[i + 1])
                const boundary = isLower(prev) || isNumeric(prev) || (isUpper(prev) && nextLower)
                if (boundary && out && !out.endsWith("-")) {
                    out += "-"
                }
            }
            out += c.toLowerCase()
        } else if (out && !out.endsWith("-")) {
            out += "-"
        }
    }
    return out.replace(/^-+|-+$/g, "")
}

/** The noun forms of a group — every word of it (lowercase, alphanumeric),
 *  each with its singular and plural: `widgets` → [widgets, widget, …],
 *  `companies` → [companies, company, …]. Heuristic English inflection; the
 *  odd extra form is harmless (it only ever REMOVES a matching word). */
export function groupNounForms(group: string): string[] {
    const forms: string[] = []
    const add = (w: string | undefined) => {
        if (w && !forms.includes(w)) {
            forms.push(w)
        }
    }
    for (const word of group.toLowerCase().split(/[^\p{L}\p{N}]+/u).filter(Boolean)) {
        add(word)
        add(singularOf(word))
        add(pluralOf(word))
    }
    return forms
}

function singularOf(w: string): string | undefined {
    if (w.length > 3 && w.endsWith("ies")) {
        return w.slice(0, -3) + "y"
    }
    for (const suffix of ["ses", "xes", "zes", "ches", "shes"]) {
        if (w.length > suffix.length && w.endsWith(suffix)) {
            return w.slice(0, -2)
        }
    }
    if (w.length > 1 && w.endsWith("s") && !w.endsWith("ss")) {
        return w.slice(0, -1)
    }
    return undefined
}

function pluralOf(w: string): string {
    if (/[^aeiou]y$/.test(w)) {
        return w.slice(0, -1) + "ies"
    }
    if (/(s|x|z|ch|sh)$/.test(w)) {
        return w + "es"
    }
    return w + "s"
}
