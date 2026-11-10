import type {OpenAPIV3} from "openapi-types";

import type {NavigationGroupEntry} from "./collapseObjectsGroup";
import type {XDocs} from "./types";

/** The document key of the tag groups: `x-tagGroups: [{ name, tags: [...] }]`,
 *  the sections of the generated navigation (the extension Redoc reads). */
export const TAG_GROUPS_KEY = "x-tagGroups"

/** The key of a tag's display name: `tags: [{ name, x-displayName }]`, the
 *  label of the tag's groups in the generated navigation (the extension Redoc
 *  reads). */
export const DISPLAY_NAME_KEY = "x-displayName"

/** One `x-tagGroups` entry: a section's name and the group names it lists, in
 *  order. */
export interface TagGroup {
    name: string
    tags: string[]
}

type ExtensionSchema = OpenAPIV3.Document & {
    "x-docs"?: XDocs
    "x-tagGroups"?: unknown
}

/** The generated navigation's second finishing pass — the twin of the Rust
 *  `navigation::group_by_tag_groups`: group the top-level groups into sections
 *  (the document's `x-tagGroups`) and label every generated group with its
 *  tag's display name (the tags' `x-displayName`). `sidebar` is the navigation
 *  after `collapseObjectsGroup` — which finds the objects group by its raw
 *  name, so it runs first; `schema` the document. Returns the new sidebar (the
 *  input is not mutated).
 *
 *  A no-op with an `x-docs.sidebar` (the author's navigation, untouched) or
 *  when the document has neither a well-formed tag group nor a tag with a
 *  non-empty string display name. Otherwise:
 *
 *  - **Sections.** A tag group is `{ name: <a non-empty string>, tags: [<the
 *    group names it lists>] }`; malformed entries (and non-string or empty
 *    names in `tags`) are skipped, and a tag group that repeats an earlier
 *    one's name is skipped. The top-level entries are walked in order: an
 *    entry whose (raw) group name a tag group lists moves into that tag
 *    group's section — the first tag group listing it wins. The section,
 *    `{ group: <name>, pages: [...] }`, takes the place of its first member,
 *    and its members follow the tag group's `tags` order. Every other entry
 *    keeps its place — no page is ever hidden; the objects group is not a tag
 *    and stays where it is. A listed name without an entry adds nothing, and
 *    a section without members is not emitted. A tag group named like a
 *    top-level group that keeps its place (its name or its display name) is
 *    skipped, so no two top-level groups share a name. A section is a plain
 *    group entry: no flag is added — the groups moved into it become nested
 *    groups, which a renderer folds on its own.
 *  - **Labels.** Every generated group entry, at any depth, whose name is a
 *    document tag's `name` takes that tag's `x-displayName` (the first tag of
 *    a name wins) — the groups inside a section and the sub-groups of the
 *    objects group alike. A section's own name is never relabelled; page
 *    entries and the other keys (`collapsed`) are untouched, and the key order
 *    is kept (`group`, then `collapsed` if present, then `pages`). */
export function groupByTagGroups<T extends NavigationGroupEntry>(
    sidebar: T[],
    schema: ExtensionSchema | undefined
): T[] {
    if (!schema || Array.isArray(schema["x-docs"]?.sidebar)) {
        return sidebar
    }
    const tagGroups = tagGroupsOf(schema)
    const displayNames = displayNamesOf(schema)
    if (!tagGroups.length && !displayNames.size) {
        return sidebar
    }
    const owners = sectionOwners(sidebar, tagGroups, displayNames)

    // Each section's slot in `out` (taken by its first member) and its
    // members with their rank in the tag group's `tags`.
    const sections = new Map<number, { slot: number, members: { rank: number, entry: T }[] }>()
    const out: (T | null)[] = []
    sidebar.forEach((original, i) => {
        const entry = relabel(original, displayNames)
        const owner = owners[i]
        if (!owner) {
            out.push(entry)
            return
        }
        let section = sections.get(owner.section)
        if (!section) {
            section = {slot: out.length, members: []}
            sections.set(owner.section, section)
            out.push(null)
        }
        section.members.push({rank: owner.rank, entry})
    })
    for (const [index, {slot, members}] of sections) {
        // Stable: entries of one rank (one name) keep their order.
        members.sort((a, b) => a.rank - b.rank)
        out[slot] = {group: tagGroups[index].name, pages: members.map(m => m.entry)} as unknown as T
    }
    return out as T[]
}

/** The document's well-formed tag groups, in order (see `groupByTagGroups`). */
function tagGroupsOf(schema: ExtensionSchema): TagGroup[] {
    const groups = schema[TAG_GROUPS_KEY]
    if (!Array.isArray(groups)) {
        return []
    }
    const out: TagGroup[] = []
    for (const group of groups) {
        if (!group || typeof group !== "object" || Array.isArray(group)) {
            continue
        }
        const {name, tags} = group as { name?: unknown, tags?: unknown }
        if (typeof name !== "string" || !name || !Array.isArray(tags)) {
            continue
        }
        if (out.some(earlier => earlier.name === name)) {
            continue
        }
        out.push({
            name,
            tags: tags.filter((t): t is string => typeof t === "string" && !!t),
        })
    }
    return out
}

/** Tag name → display name: the document tags with a non-empty string
 *  `x-displayName` (the first tag of a name wins). */
function displayNamesOf(schema: ExtensionSchema): Map<string, string> {
    const out = new Map<string, string>()
    const tags: unknown = schema.tags
    if (!Array.isArray(tags)) {
        return out
    }
    const seen = new Set<string>()
    for (const tag of tags) {
        if (!tag || typeof tag !== "object") {
            continue
        }
        const name = (tag as { name?: unknown }).name
        if (typeof name !== "string" || !name || seen.has(name)) {
            continue
        }
        seen.add(name)
        const label = (tag as Record<string, unknown>)[DISPLAY_NAME_KEY]
        if (typeof label === "string" && label) {
            out.set(name, label)
        }
    }
    return out
}

/** Per top-level entry: the tag group it moves into and its rank in that tag
 *  group's `tags`, or `null` when it keeps its place. A tag group named like
 *  an entry that keeps its place (its name or its display name — the label it
 *  takes afterwards) is dropped — which can leave more entries in place (the
 *  ones it held), so the assignment repeats until nothing more is dropped (at
 *  most once per tag group). */
function sectionOwners(
    sidebar: NavigationGroupEntry[],
    tagGroups: TagGroup[],
    displayNames: Map<string, string>
): ({ section: number, rank: number } | null)[] {
    const names = sidebar.map(entry => typeof entry?.group === "string" ? entry.group : null)
    const active = tagGroups.map(() => true)
    for (;;) {
        const owners = names.map(name => {
            if (name === null) {
                return null
            }
            for (let i = 0; i < tagGroups.length; i++) {
                if (!active[i]) {
                    continue
                }
                const rank = tagGroups[i].tags.indexOf(name)
                if (rank >= 0) {
                    return {section: i, rank}
                }
            }
            return null
        })
        let dropped = false
        tagGroups.forEach((group, i) => {
            const collides = names.some((name, j) => owners[j] === null && name !== null
                && (name === group.name || displayNames.get(name) === group.name))
            if (active[i] && collides) {
                active[i] = false
                dropped = true
            }
        })
        if (!dropped) {
            return owners
        }
    }
}

/** A group entry and every group under it, labelled with the display names;
 *  a page entry (a `{virtual, page}` leaf, or the page path in store mode) is
 *  returned as it is. */
function relabel<T>(entry: T, displayNames: Map<string, string>): T {
    if (!entry || typeof entry !== "object" || Array.isArray(entry) || !("group" in entry)) {
        return entry
    }
    const out: Record<string, unknown> = {}
    for (const [key, value] of Object.entries(entry)) {
        if (key === "group" && typeof value === "string" && displayNames.has(value)) {
            out[key] = displayNames.get(value)
        } else if (key === "pages" && Array.isArray(value)) {
            out[key] = value.map(page => relabel(page, displayNames))
        } else {
            out[key] = value
        }
    }
    return out as T
}
