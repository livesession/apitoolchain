// Reference[] → an agent skill: a SKILL.md (frontmatter, an intro, a table of
// the reference files) plus one markdown reference file per operation group
// and an index of every operation.
//
// A pure JavaScript implementation (no native addon, no Node builtins), so a
// generator can run it anywhere the package installs. The Rust twin
// (crates/apitoolchain_uniform/src/skill.rs) is pinned to it line for line by
// the 6.skill.* fixtures; a change here is a change there.
import type { DefinitionProperty, DefinitionVariant, Example, Reference } from "../types";
import {
    arrayItem,
    definitionProperties,
    effectiveProperty,
    enumBaseType,
    enumValue,
    findMeta,
    isJsonContentType,
    isRequired,
    isRestOperation,
    jsonSchemaType,
    metaFlag,
    objectChildren,
    operationMethod,
    operationPath,
    requestBody,
    responseVariants,
    restContext,
    text,
    variantContentType,
} from "../operations";
import { cell, codeBlock, codeLink, collapse, comment, inlineCode, link, row, tableHeader, yamlScalar } from "./markdown";

export interface SkillReferencesOptions {
    /** Where the generated reference files go, relative to SKILL.md (default `references`). */
    dir?: string;
    /** One file per operation, `<dir>/<group>/<operation>.md`, instead of one
     *  per group (default `false`). The group directory is a slug of the group
     *  name; the operation file keeps its canonical's case, with characters
     *  outside `[A-Za-z0-9._-]` turned into `-`. */
    perOperation?: boolean;
    /** The text of the comment heading each generated file (default: a
     *  generated-file notice). Called with the group name, `index` for the index. */
    header?: (group: string) => string;
}

export interface SkillOptions {
    /** The skill's name (frontmatter `name`). */
    name: string;
    /** When to use the skill (frontmatter `description`). */
    description: string;
    /** A `# title` under the frontmatter (none by default — an intro may bring its own). */
    title?: string;
    /** Markdown placed under the frontmatter (and the title), byte for byte. */
    intro?: string;
    /** The group an operation is filed under (default: `context.group[0]`, else `general`). */
    groupBy?: (ref: Reference) => string;
    /** Which REST operations are documented (default: all of them). */
    include?: (ref: Reference) => boolean;
    references?: SkillReferencesOptions;
    /** Each operation's "Example request" (default `"auto"`): `"auto"` takes
     *  the first request example the references carry (a shell / curl tab when
     *  there is one) and synthesizes a curl request when there is none;
     *  `"synthesized"` always synthesizes it; `"none"` leaves it out. A
     *  synthesized request sends only the required inputs plus `limit`, each
     *  with its example or default when it states one (`limit`: else 10) and a
     *  typed placeholder otherwise (`<name>`, `0`, `false`). */
    exampleRequest?: "auto" | "synthesized" | "none";
    /** Hand-written files to ship with the skill, by path relative to SKILL.md,
     *  byte for byte. SKILL.md lists them; one at a generated path replaces it. */
    extras?: Record<string, string>;
}

export const DEFAULT_SKILL_HEADER = "Generated from the API reference. Do not edit by hand.";

const DEFAULT_GROUP = "general";

/**
 * The files of an agent skill documenting the REST operations in
 * `references`, by path relative to the skill root: `SKILL.md`,
 * `<dir>/index.md`, one `<dir>/<group>.md` per group, then `extras`.
 */
export function skillFiles(references: Reference[], options: SkillOptions): Map<string, string> {
    const dir = (options.references?.dir ?? "references").replace(/^(\.\/)+/, "").replace(/^\/+|\/+$/g, "");
    const header = options.references?.header ?? (() => DEFAULT_SKILL_HEADER);
    const perOperation = options.references?.perOperation === true;
    const exampleMode = options.exampleRequest ?? "auto";
    const at = (file: string) => (dir ? `${dir}/${file}` : file);

    // Operations by group, groups in order of first appearance.
    const groups: Group[] = [];
    const byName = new Map<string, Group>();
    const taken = new Set<string>(["index"]);
    for (const ref of references || []) {
        if (!isRestOperation(ref) || (options.include && !options.include(ref))) {
            continue;
        }
        const name = (options.groupBy ? options.groupBy(ref) : defaultGroup(ref)) || DEFAULT_GROUP;
        let group = byName.get(name);
        if (!group) {
            const slug = uniqueSlug(name, taken);
            group = { name, slug, file: slug + ".md", operations: [], files: [], takenFiles: new Set() };
            byName.set(name, group);
            groups.push(group);
        }
        group.operations.push(ref);
        group.files.push(`${group.slug}/${operationFileName(operationId(ref), group.takenFiles)}.md`);
    }

    const files = new Map<string, string>();
    files.set("SKILL.md", skillMd(options, groups, at, perOperation));
    files.set(at("index.md"), indexMd(groups, header("index"), perOperation));
    for (const group of groups) {
        if (perOperation) {
            group.operations.forEach((ref, i) => {
                const blocks = [comment(header(group.name)), ...operationBlocks(ref, 1, exampleMode)];
                files.set(at(group.files[i]), blocks.join("\n\n") + "\n");
            });
        } else {
            files.set(at(group.file), groupMd(group, header(group.name), exampleMode));
        }
    }
    const extras = options.extras || {};
    for (const path of Object.keys(extras).sort()) {
        files.set(path, extras[path]);
    }
    return files;
}

interface Group {
    name: string;
    /** The group's slug: its file's name (`<slug>.md`), or its directory. */
    slug: string;
    file: string;
    operations: Reference[];
    /** Per operation, its own file under the group directory. */
    files: string[];
    takenFiles: Set<string>;
}

function defaultGroup(ref: Reference): string {
    const group = restContext(ref).group;
    return Array.isArray(group) && typeof group[0] === "string" ? group[0] : "";
}

/** An operation's file name: its id with every run of characters outside
 *  `[A-Za-z0-9._-]` a `-`, `-` and `.` trimmed from both ends (`operation`
 *  when nothing is left), case kept; `-2`, `-3`… against the names taken in
 *  its group, compared without case (case-insensitive file systems). */
function operationFileName(id: string, taken: Set<string>): string {
    let name = "";
    let inRun = false;
    for (const c of id) {
        if (/^[A-Za-z0-9._-]$/.test(c)) {
            name += c;
            inRun = false;
        } else if (!inRun) {
            name += "-";
            inRun = true;
        }
    }
    name = name.replace(/^[-.]+|[-.]+$/g, "") || "operation";
    let candidate = name;
    for (let n = 2; taken.has(candidate.toLowerCase()); n++) {
        candidate = `${name}-${n}`;
    }
    taken.add(candidate.toLowerCase());
    return candidate;
}

/** A file-name slug: ASCII letters and digits lower-cased, every other run a
 *  `-`; `group` when nothing is left; `-2`, `-3`… against the slugs taken. */
function uniqueSlug(name: string, taken: Set<string>): string {
    let slug = "";
    for (const c of name) {
        slug += /^[A-Za-z0-9]$/.test(c) ? c.toLowerCase() : "-";
    }
    slug = slug.replace(/-+/g, "-").replace(/^-|-$/g, "") || "group";
    let candidate = slug;
    for (let n = 2; taken.has(candidate); n++) {
        candidate = `${slug}-${n}`;
    }
    taken.add(candidate);
    return candidate;
}

// ---------------------------------------------------------------------------
// SKILL.md and the index
// ---------------------------------------------------------------------------

function skillMd(options: SkillOptions, groups: Group[], at: (file: string) => string, perOperation: boolean): string {
    const blocks: string[] = [];
    blocks.push(`---\nname: ${yamlScalar(options.name)}\ndescription: ${yamlScalar(options.description)}\n---`);
    if (options.title) {
        blocks.push(`# ${collapse(options.title)}`);
    }

    const sections: string[] = [];
    const table = [tableHeader(["Group", "Operations", perOperation ? "Directory" : "File"])];
    for (const group of groups) {
        const where = perOperation ? inlineCode(at(group.slug) + "/") : link(at(group.file), at(group.file));
        table.push(row([cell(group.name), String(group.operations.length), cell(where)]));
    }
    sections.push("## API reference");
    sections.push(table.join("\n"));
    sections.push(`Every operation in one table: ${link(at("index.md"), at("index.md"))}.`);

    const extras = Object.keys(options.extras || {})
        .filter((path) => path !== "SKILL.md")
        .sort();
    if (extras.length) {
        sections.push("## Other files");
        sections.push(extras.map((path) => `- ${link(path, path)}`).join("\n"));
    }

    let out = blocks.join("\n\n") + "\n\n";
    if (options.intro) {
        out += options.intro + (options.intro.endsWith("\n") ? "\n" : "\n\n");
    }
    return out + sections.join("\n\n") + "\n";
}

/** Every operation, by group. Per operation, the operation links to its file
 *  (and the group is plain text); else the group links to its file. */
function indexMd(groups: Group[], header: string, perOperation: boolean): string {
    const lines = [tableHeader(["Operation", "Method", "Path", "Group", "Title"])];
    for (const group of groups) {
        group.operations.forEach((ref, i) => {
            const id = operationId(ref);
            lines.push(
                row([
                    cell(perOperation ? codeLink(id, group.files[i]) : inlineCode(id)),
                    operationMethod(ref),
                    cell(inlineCode(operationPath(ref))),
                    cell(perOperation ? group.name : link(group.name, group.file)),
                    cell(ref.title || ""),
                ])
            );
        });
    }
    return [comment(header), "# Operations", lines.join("\n")].join("\n\n") + "\n";
}

function operationId(ref: Reference): string {
    return ref.canonical || `${operationMethod(ref)} ${operationPath(ref)}`;
}

// ---------------------------------------------------------------------------
// a group's reference file
// ---------------------------------------------------------------------------

type ExampleMode = NonNullable<SkillOptions["exampleRequest"]>;

function groupMd(group: Group, header: string, exampleMode: ExampleMode): string {
    const blocks: string[] = [comment(header), `# ${collapse(group.name)}`];
    for (const ref of group.operations) {
        blocks.push(...operationBlocks(ref, 2, exampleMode));
    }
    return blocks.join("\n\n") + "\n";
}

/** An operation's blocks, its heading at `level` (2 in a group's file, 1 in
 *  its own) and its sections one level below. */
function operationBlocks(ref: Reference, level: number, exampleMode: ExampleMode): string[] {
    const h = "#".repeat(level);
    const sub = "#".repeat(level + 1);
    const method = operationMethod(ref);
    const path = operationPath(ref);
    const title = collapse(ref.title || "");
    const blocks: string[] = [];

    const id = operationId(ref);
    blocks.push(`${h} ${inlineCode(id)}${title && title !== id ? ` — ${title}` : ""}`);
    blocks.push(inlineCode(`${method} ${path}`));

    const description = text(ref.description).trim();
    if (description && description !== (ref.title || "").trim()) {
        blocks.push(description);
    }

    const pathProps = definitionProperties(ref, "$rest.param.path");
    if (pathProps.length) {
        blocks.push(`${sub} Path parameters`, propertyTable(pathProps));
    }
    const queryProps = definitionProperties(ref, "$rest.param.query");
    if (queryProps.length) {
        blocks.push(`${sub} Query parameters`, propertyTable(queryProps));
    }
    const headerProps = definitionProperties(ref, "$rest.param.header");
    if (headerProps.length) {
        blocks.push(`${sub} Headers`, propertyTable(headerProps));
    }
    const cookieProps = definitionProperties(ref, "$rest.param.cookie");
    if (cookieProps.length) {
        blocks.push(`${sub} Cookies`, propertyTable(cookieProps));
    }

    const body = requestBody(ref, false);
    if (body) {
        blocks.push(`${sub} Request body`);
        const facts: string[] = [];
        if (body.contentType) {
            facts.push(`Content type: ${inlineCode(body.contentType)}.`);
        }
        facts.push(body.required ? "Required." : "Optional.");
        blocks.push(facts.join(" "));
        if (body.description.trim()) {
            blocks.push(collapse(body.description));
        }
        if (body.rootProperty) {
            blocks.push(propertyTable([body.rootProperty], { name: "(body)", required: body.required }));
        } else if (body.properties.length) {
            blocks.push(propertyTable(body.properties));
        }
    }

    const variants = responseVariants(ref);
    if (variants.length) {
        const lines = [tableHeader(["Status", "Content type", "Type", "Description"])];
        for (const variant of variants) {
            lines.push(responseRow(variant));
        }
        blocks.push(`${sub} Responses`, lines.join("\n"));
        // Each response's fields, one level deep, like the request body's.
        for (const variant of variants) {
            const fields = responseFields(variant);
            if (fields) {
                blocks.push(`${sub}# ${responseHeading(variant)}`, fields);
            }
        }
    }

    const example =
        exampleMode === "none"
            ? null
            : (exampleMode === "auto" ? exampleRequest(ref) : null) ?? synthesizedRequest(ref);
    if (example) {
        blocks.push(`${sub} Example request`, codeBlock(example.code, example.language));
    }

    return blocks;
}

// ---------------------------------------------------------------------------
// property tables
// ---------------------------------------------------------------------------

/** Name, type, required, description — each property, then its object
 *  children one level down as `name.child` (`name[].child` for an array of
 *  objects). `root` names a lone root property (a body that is not an
 *  object), whose children read `[].child` / `.child`. */
function propertyTable(props: DefinitionProperty[], root?: { name: string; required: boolean }): string {
    const lines = [tableHeader(["Name", "Type", "Required", "Description"])];
    for (const prop of props) {
        const name = root ? "" : prop.name;
        if (!root && !name) {
            continue;
        }
        const nameCell = root ? cell(root.name) : cell(inlineCode(name));
        lines.push(propertyRow(nameCell, prop, root ? root.required : isRequired(prop)));

        const eff = effectiveProperty(prop);
        let children = objectChildren(eff);
        let joiner = ".";
        if (eff.type === "$$array") {
            const item = arrayItem(eff);
            children = item ? objectChildren(effectiveProperty(item)) : [];
            joiner = "[].";
        }
        for (const child of children) {
            if (!child.name) {
                continue;
            }
            lines.push(propertyRow(cell(inlineCode(`${name}${joiner}${child.name}`)), child, isRequired(child)));
        }
    }
    return lines.join("\n");
}

function propertyRow(nameCell: string, prop: DefinitionProperty, required: boolean): string {
    return row([nameCell, cell(typeLabel(prop)), required ? "yes" : "no", cell(describe(prop))]);
}

/** A property's description plus what its meta says: deprecation, the enum
 *  values, the default, the bounds, an example. */
function describe(raw: DefinitionProperty): string {
    const prop = effectiveProperty(raw);
    const parts: string[] = [];
    if (metaFlag(prop.meta, "deprecated")) {
        parts.push("Deprecated.");
    }
    const description = collapse(text(prop.description));
    const facts: string[] = [];
    if (prop.type === "$$enum") {
        facts.push(`One of: ${enumValues(prop)}.`);
    } else if (prop.type === "$$array") {
        const item = arrayItem(prop);
        const eff = item ? effectiveProperty(item) : undefined;
        if (eff?.type === "$$enum") {
            facts.push(`Each one of: ${enumValues(eff)}.`);
        }
    }
    const defaults = findMeta(prop.meta, "defaults");
    if (defaults && "value" in defaults) {
        facts.push(`Default: ${codeValue(defaults.value)}.`);
    }
    const minimum = findMeta(prop.meta, "minimum");
    if (minimum && "value" in minimum) {
        facts.push(`Minimum: ${codeValue(minimum.value)}.`);
    }
    const maximum = findMeta(prop.meta, "maximum");
    if (maximum && "value" in maximum) {
        facts.push(`Maximum: ${codeValue(maximum.value)}.`);
    }
    const example = firstExample(prop);
    if (example !== undefined) {
        facts.push(`Example: ${codeValue(example)}.`);
    }
    if (description) {
        // A sentence of its own before the facts that follow it.
        const last = description[description.length - 1];
        parts.push(facts.length && last !== "." && last !== "!" && last !== "?" ? `${description}.` : description);
    }
    return [...parts, ...facts].join(" ");
}

function enumValues(prop: DefinitionProperty): string {
    return (prop.properties || []).map((value) => codeValue(String(value.name ?? ""))).join(", ");
}

/** The first example: the `examples` meta's, else the `example` meta, else
 *  the property's own `examples` (the order the MCP emitter reads them in). */
function firstExample(prop: DefinitionProperty): unknown {
    const many = findMeta(prop.meta, "examples");
    if (many && "value" in many) {
        return Array.isArray(many.value) ? (many.value.length ? many.value[0] : undefined) : many.value;
    }
    const one = findMeta(prop.meta, "example");
    if (one && "value" in one) {
        return one.value;
    }
    if (Array.isArray(prop.examples)) {
        return prop.examples.length ? prop.examples[0] : undefined;
    }
    return typeof prop.examples === "string" && prop.examples ? prop.examples : undefined;
}

/** A value as text: strings as they are, everything else as JSON. */
function valueText(value: unknown): string {
    return typeof value === "string" ? value : JSON.stringify(value) ?? "null";
}

/** A value as inline code; the empty string reads `""` (an empty code span
 *  does not render). */
function codeValue(value: unknown): string {
    const text = valueText(value);
    return inlineCode(text === "" ? '""' : text);
}

/** A short type: `string`, `array of Pet`, `Card or Transfer`, `… or null`. */
export function typeLabel(raw: DefinitionProperty): string {
    const prop = effectiveProperty(raw);
    let label: string;
    switch (prop.type) {
        case "$$enum":
            label = enumBaseType(prop) || "enum";
            break;
        case "$$array": {
            const item = arrayItem(prop);
            label = item ? `array of ${typeLabel(item)}` : "array";
            break;
        }
        case "$$union":
        case "$$xor": {
            const labels: string[] = [];
            for (const alt of prop.properties || []) {
                const altLabel = typeLabel(alt);
                if (!labels.includes(altLabel)) {
                    labels.push(altLabel);
                }
            }
            label = labels.length ? labels.join(" or ") : "any";
            break;
        }
        case "$$function":
            label = "function";
            break;
        default: {
            const children = objectChildren(prop);
            const symbol = prop.symbolDef?.label;
            if ((prop.type === "object" || children.length) && symbol) {
                label = symbol;
            } else {
                label = jsonSchemaType(prop.type) || (children.length ? "object" : prop.type || "any");
            }
        }
    }
    if (metaFlag(prop.meta, "nullable") && !label.split(" or ").includes("null")) {
        label += " or null";
    }
    return label;
}

// ---------------------------------------------------------------------------
// responses and the example request
// ---------------------------------------------------------------------------

function responseRow(variant: DefinitionVariant): string {
    const status = findMeta(variant.meta, "status")?.value;
    const contentType = variantContentType(variant);
    let type = "—";
    if (contentType) {
        const root = variant.rootProperty;
        const symbol = variant.symbolDef?.label;
        if (root && root.type === "$$array") {
            const item = arrayItem(root);
            const itemLabel = item?.symbolDef?.label || symbol || (item ? typeLabel(item) : "");
            type = itemLabel ? `array of ${itemLabel}` : "array";
        } else if (root) {
            type = typeLabel(root);
        } else if (symbol) {
            type = symbol;
        } else if ((variant.properties || []).some((p) => p.name)) {
            type = "object";
        }
    }
    const description = text(variant.description) || text(variant.rootProperty?.description);
    return row([
        cell(typeof status === "string" && status ? status : variant.title || ""),
        contentType ? cell(inlineCode(contentType)) : "—",
        cell(type),
        cell(description),
    ]);
}

const SHELL_LANGUAGES = new Set(["shell", "bash", "sh", "curl"]);

/** The first request example's tab — a shell (curl) one when there is one. */
function exampleRequest(ref: Reference): { code: string; language: string } | null {
    const groups = ref.examples?.groups || [];
    const group = groups.find((g) => g?.kind === "request") ?? (groups.some((g) => g?.kind) ? undefined : groups[0]);
    const example: Example | undefined = group?.examples?.[0];
    const tabs = example?.codeblock?.tabs || [];
    if (!tabs.length) {
        return null;
    }
    const tab =
        tabs.find((t) => SHELL_LANGUAGES.has((t.language || "").toLowerCase()) || /curl|shell/i.test(t.title || "")) ||
        tabs[0];
    return { code: tab.code || "", language: tab.language || "" };
}

/** `200 — \`application/json\``: a response's status and content type. */
function responseHeading(variant: DefinitionVariant): string {
    const status = findMeta(variant.meta, "status")?.value;
    const contentType = variantContentType(variant);
    const name = collapse(typeof status === "string" && status ? status : variant.title || "");
    return contentType ? `${name} — ${inlineCode(contentType)}` : name;
}

/** A response's fields (name, type, description), each with its object
 *  children one level down — the root object's properties, or an array
 *  root's item properties as `[].name`. `null` when there are none. */
function responseFields(variant: DefinitionVariant): string | null {
    let props = (variant.properties || []).filter((p) => p.name);
    let prefix = "";
    if (!props.length && variant.rootProperty) {
        const root = effectiveProperty(variant.rootProperty);
        if (root.type === "$$array") {
            const item = arrayItem(root);
            props = item ? objectChildren(effectiveProperty(item)).filter((p) => p.name) : [];
            prefix = "[].";
        } else {
            props = objectChildren(root).filter((p) => p.name);
        }
    }
    if (!props.length) {
        return null;
    }
    const lines = [tableHeader(["Name", "Type", "Description"])];
    const field = (name: string, prop: DefinitionProperty) =>
        row([cell(inlineCode(name)), cell(typeLabel(prop)), cell(describe(prop))]);
    for (const prop of props) {
        const name = prefix + prop.name;
        lines.push(field(name, prop));
        const eff = effectiveProperty(prop);
        let children = objectChildren(eff);
        let joiner = ".";
        if (eff.type === "$$array") {
            const item = arrayItem(eff);
            children = item ? objectChildren(effectiveProperty(item)) : [];
            joiner = "[].";
        }
        for (const child of children) {
            if (child.name) {
                lines.push(field(name + joiner + child.name, child));
            }
        }
    }
    return lines.join("\n");
}

// ---------------------------------------------------------------------------
// the synthesized example request
// ---------------------------------------------------------------------------

/** How deep a synthesized body spells objects and arrays out. */
const SAMPLE_DEPTH = 3;
/** `limit` when it states no example or default. */
const SAMPLE_LIMIT = 10;

/** A curl request sending only the required inputs plus `limit`. */
function synthesizedRequest(ref: Reference): { code: string; language: string } {
    const ctx = restContext(ref);
    let url = typeof ctx.fullPath === "string" && ctx.fullPath ? ctx.fullPath : operationPath(ref);
    for (const prop of definitionProperties(ref, "$rest.param.path")) {
        if (prop.name) {
            url = url.split(`{${prop.name}}`).join(urlText(paramText(prop)));
        }
    }
    const query: string[] = [];
    for (const prop of definitionProperties(ref, "$rest.param.query")) {
        if (!prop.name || (!isRequired(prop) && prop.name !== "limit")) {
            continue;
        }
        const value = prop.name === "limit" ? limitValue(prop) : (statedValue(effectiveProperty(prop)) ?? `<${prop.name}>`);
        for (const item of Array.isArray(value) ? value : [value]) {
            query.push(`${urlText(prop.name)}=${urlText(valueText(item))}`);
        }
    }
    if (query.length) {
        url += "?" + query.join("&");
    }

    const lines = [`curl --request ${operationMethod(ref)}`, `--url '${url}'`];
    for (const prop of definitionProperties(ref, "$rest.param.header")) {
        if (prop.name && isRequired(prop)) {
            lines.push(`--header '${shellText(`${prop.name}: ${paramText(prop)}`)}'`);
        }
    }
    const body = requestBody(ref, false);
    if (body && body.required && isJsonContentType(body.contentType)) {
        const data = body.rootProperty
            ? sampleValue(body.rootProperty, "body", 1)
            : sampleObject(body.properties, 1, true);
        lines.push(`--header '${shellText(`content-type: ${body.contentType}`)}'`);
        lines.push(`--data '${shellText(JSON.stringify(data, null, 2))}'`);
    }
    return { code: lines.join(" \\\n     "), language: "shell" };
}

/** A parameter's value as text: what it states, else `<name>`. */
function paramText(prop: DefinitionProperty): string {
    const stated = statedValue(effectiveProperty(prop));
    return stated === undefined ? `<${prop.name}>` : valueText(stated);
}

function limitValue(prop: DefinitionProperty): unknown {
    return statedValue(effectiveProperty(prop)) ?? SAMPLE_LIMIT;
}

/** What a property states as its value: its first example, else its default.
 *  A text example or default of an object or array (the converter writes
 *  structured examples as JSON text) is not used. */
function statedValue(prop: DefinitionProperty): unknown {
    const structured = prop.type === "object" || prop.type === "$$array" || objectChildren(prop).length > 0;
    const example = firstExample(prop);
    if (example !== undefined && !(structured && typeof example === "string")) {
        return example;
    }
    const defaults = findMeta(prop.meta, "defaults");
    if (defaults && "value" in defaults && !(structured && typeof defaults.value === "string")) {
        return defaults.value;
    }
    return undefined;
}

/** A value for a property: what it states, else the first enum value or union
 *  alternative, a one-item array, an object of its required properties, or a
 *  typed placeholder. */
function sampleValue(raw: DefinitionProperty, name: string, depth: number): unknown {
    const prop = effectiveProperty(raw);
    const stated = statedValue(prop);
    if (stated !== undefined) {
        return stated;
    }
    switch (prop.type) {
        case "$$enum": {
            const first = (prop.properties || [])[0];
            return first ? enumValue(String(first.name ?? ""), enumBaseType(prop)) : null;
        }
        case "$$array": {
            const item = arrayItem(prop);
            return item && depth < SAMPLE_DEPTH ? [sampleValue(item, name, depth + 1)] : [];
        }
        case "$$union":
        case "$$xor": {
            const first = (prop.properties || [])[0];
            return first ? sampleValue(first, name, depth) : null;
        }
    }
    const children = objectChildren(prop);
    if (children.length || prop.type === "object") {
        return depth < SAMPLE_DEPTH ? sampleObject(children, depth + 1, false) : {};
    }
    switch (prop.type) {
        case "integer":
        case "number":
            return 0;
        case "boolean":
            return false;
        case "null":
            return null;
        default:
            return `<${name || "value"}>`;
    }
}

/** An object of the required properties (and `limit`, at the top level). */
function sampleObject(props: DefinitionProperty[], depth: number, withLimit: boolean): Record<string, unknown> {
    const out: Record<string, unknown> = Object.create(null);
    for (const prop of props) {
        const name = prop.name;
        if (!name || Object.prototype.hasOwnProperty.call(out, name)) {
            continue;
        }
        const limit = withLimit && name === "limit";
        if (limit) {
            out[name] = limitValue(prop);
        } else if (isRequired(prop)) {
            out[name] = sampleValue(prop, name, depth);
        }
    }
    return out;
}

/** Percent-encodes what would break the URL or its single quotes. */
function urlText(value: string): string {
    let out = "";
    for (const c of value) {
        out += URL_ESCAPES[c] ?? c;
    }
    return out;
}

const URL_ESCAPES: Record<string, string> = {
    " ": "%20",
    '"': "%22",
    "#": "%23",
    "%": "%25",
    "&": "%26",
    "'": "%27",
    "+": "%2B",
    "?": "%3F",
};

/** Text inside a single-quoted shell string. */
function shellText(value: string): string {
    return value.split("'").join("'\\''");
}
