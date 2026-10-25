import path from "node:path";

import type {OpenAPIV3} from "openapi-types";

import type {OpenAPIReferenceContext, Reference} from "@xyd-js/uniform";

import {objectsCollapsedOf, objectsGroupOf, objectsLayoutOf} from "../const";
import type {uniformOasOptions} from "../types";
import type {XDocs} from "./types";

/** The key of the flag on a generated group entry: `{ group: "Objects",
 *  collapsed: true, pages: [...] }`. Present (and `true`) on the one entry it
 *  applies to, absent everywhere else — never `false`. */
export const COLLAPSED_KEY = "collapsed"

/** A generated navigation entry as pluginNavigation emits it — a group with
 *  its pages (a `{virtual, page}` leaf, the page path itself in store mode, or
 *  a nested group) — plus the flag this pass adds. */
export interface NavigationGroupEntry {
    group?: string | false
    pages?: unknown[]
    /** The top-level objects group only: a renderer draws a toggle on the
     *  header and starts the group folded. */
    collapsed?: boolean
    [key: string]: unknown
}

type ExtensionSchema = OpenAPIV3.Document & {
    "x-docs"?: XDocs
}

/** The generated navigation's finishing pass — the twin of the Rust
 *  `navigation::collapse_objects_group`: flag the TOP-LEVEL objects group
 *  `collapsed: true`, so a renderer draws a toggle on its header and starts
 *  it folded (the sub-groups inside it are ordinary nested groups, folded by
 *  a renderer on their own). `sidebar` is pluginNavigation's output for
 *  `references` (the final, post-x-docs list) under `urlPrefix`; `schema` the
 *  document. Returns the sidebar with that one entry replaced (the input is
 *  not mutated).
 *
 *  The flag goes on exactly one entry, and only when every one of these holds:
 *  - `objectsCollapsed` (default `true`);
 *  - the `grouped` objects layout — under `per-area` there is no top-level
 *    group of object pages to fold, the layout is unchanged;
 *  - no `x-docs.sidebar` — a sidebar is the author's navigation, untouched;
 *  - the top-level entry named `objectsGroup` holds object pages only
 *    (directly or in its sub-groups). An operation tagged with the objects
 *    group's name shares that group; a group with an operation in it stays
 *    open, so no operation is ever hidden behind the fold — an operation group
 *    never carries the flag. */
export function collapseObjectsGroup<T extends NavigationGroupEntry>(
    sidebar: T[],
    schema: ExtensionSchema | undefined,
    references: Reference[],
    urlPrefix: string,
    options?: uniformOasOptions
): T[] {
    if (!objectsCollapsedOf(options) || objectsLayoutOf(options) !== "grouped" || schema?.["x-docs"]?.sidebar) {
        return sidebar
    }
    const objectPages = new Set<string>()
    for (const ref of references) {
        if ((ref.context as OpenAPIReferenceContext | undefined)?.componentSchema && ref.canonical) {
            objectPages.add(path.join(urlPrefix, ref.canonical))
        }
    }
    if (!objectPages.size) {
        return sidebar
    }
    const objectsGroup = objectsGroupOf(options)
    // Top-level group names are unique (pluginNavigation merges by name), so
    // at most one entry qualifies.
    const index = sidebar.findIndex(entry => entry.group === objectsGroup && holdsOnly(entry, objectPages))
    if (index < 0) {
        return sidebar
    }
    return sidebar.map((entry, i) => i === index ? withCollapsed(entry) : entry)
}

/** Whether every page under a group entry — a direct leaf (`{virtual, page}`,
 *  or the page path itself in store mode) or one inside a nested group — is
 *  one of `pages`. An entry with nothing under it is not "only object pages". */
function holdsOnly(entry: NavigationGroupEntry, pages: Set<string>): boolean {
    if (!Array.isArray(entry.pages) || !entry.pages.length) {
        return false
    }
    return entry.pages.every(child => {
        if (typeof child === "string") {
            return pages.has(child)
        }
        if (!child || typeof child !== "object") {
            return false
        }
        if ("group" in child) {
            return holdsOnly(child as NavigationGroupEntry, pages)
        }
        const page = (child as { page?: unknown }).page
        return typeof page === "string" && pages.has(page)
    })
}

/** The entry with `collapsed: true` right after `group` (the Rust twin's key
 *  order; the goldens are compared key-order-insensitively, this is for the
 *  reader of a dump). */
function withCollapsed<T extends NavigationGroupEntry>(entry: T): T {
    const out: Record<string, unknown> = {}
    for (const [key, value] of Object.entries(entry)) {
        if (key === COLLAPSED_KEY) {
            continue
        }
        out[key] = value
        if (key === "group") {
            out[COLLAPSED_KEY] = true
        }
    }
    if (!(COLLAPSED_KEY in out)) {
        out[COLLAPSED_KEY] = true
    }
    return out as T
}
