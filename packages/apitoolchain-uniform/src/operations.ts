// Shared reading of REST operations for the agent emitters (./mcp and
// ./skill). Pure JavaScript on purpose: the emitters must run where the native
// addon is absent, and the package stays isomorphic (no Node builtins).
//
// The Rust twin is crates/apitoolchain_uniform/src/operations.rs; the two are
// pinned to each other by the 5.mcp-tools.* and 6.skill.* fixtures.
import type {
    Definition,
    DefinitionProperty,
    DefinitionVariant,
    Meta,
    OpenAPIReferenceContext,
    Reference,
} from "./types";

const REST_OPERATION_TYPES = new Set([
    "rest_get",
    "rest_post",
    "rest_put",
    "rest_patch",
    "rest_delete",
    "rest_options",
    "rest_head",
    "rest_trace",
]);

/** A REST operation: category `rest` and a `rest_<method>` type. Component
 *  schemas (`rest_component_schema`) and every other category are not. */
export function isRestOperation(ref: Reference): boolean {
    return ref?.category === "rest" && typeof ref.type === "string" && REST_OPERATION_TYPES.has(ref.type);
}

export function restContext(ref: Reference): OpenAPIReferenceContext {
    const ctx = ref.context as OpenAPIReferenceContext | undefined;
    return ctx && typeof ctx === "object" ? ctx : {};
}

/** The operation's HTTP method, upper-case (`context.method`, else the type). */
export function operationMethod(ref: Reference): string {
    const method = restContext(ref).method;
    if (typeof method === "string" && method) {
        return method.toUpperCase();
    }
    return String(ref.type || "").replace(/^rest_/, "").toUpperCase();
}

export function operationPath(ref: Reference): string {
    const path = restContext(ref).path;
    return typeof path === "string" ? path : "";
}

/** `context.group` without its empty entries (an untagged operation's `[""]`). */
export function operationGroup(ref: Reference): string[] {
    const group = restContext(ref).group;
    return Array.isArray(group) ? group.filter((g) => typeof g === "string" && g !== "") : [];
}

/** A description as text — a ReactNode (never produced by the converters) reads as "". */
export function text(value: unknown): string {
    return typeof value === "string" ? value : "";
}

export function findMeta(meta: Meta[] | undefined, name: string): Meta | undefined {
    return Array.isArray(meta) ? meta.find((m) => m?.name === name) : undefined;
}

/** A meta flag: the converters write `"true"`; a boolean `true` is honoured too. */
export function metaFlag(meta: Meta[] | undefined, name: string): boolean {
    const m = findMeta(meta, name);
    return m !== undefined && (m.value === "true" || m.value === true);
}

/** Required per its meta. An `anyOf` property carries no meta of its own (the
 *  converter puts it on the alternatives), so a union counts as required when
 *  it has no meta and one of its alternatives is. */
export function isRequired(prop: DefinitionProperty): boolean {
    if (metaFlag(prop.meta, "required")) {
        return true;
    }
    if (prop.type === "$$union" && prop.meta === undefined) {
        return (prop.properties || []).some((alt) => metaFlag(alt.meta, "required"));
    }
    return false;
}

/** The properties of every definition of one REST type, in document order. */
export function definitionProperties(ref: Reference, type: Definition["type"]): DefinitionProperty[] {
    const out: DefinitionProperty[] = [];
    for (const def of ref.definitions || []) {
        if (def?.type === type) {
            out.push(...(def.properties || []));
        }
    }
    return out;
}

export function isJsonContentType(contentType: string): boolean {
    const base = contentType.split(";")[0].trim().toLowerCase();
    return base === "application/json" || base.endsWith("+json");
}

export function variantContentType(variant: DefinitionVariant): string {
    const ct = findMeta(variant.meta, "contentType")?.value;
    return typeof ct === "string" ? ct : "";
}

export interface RequestBody {
    contentType: string;
    description: string;
    properties: DefinitionProperty[];
    rootProperty?: DefinitionProperty;
    required: boolean;
}

/** The request body: its JSON variant, else (`jsonOnly` false) its first one. */
export function requestBody(ref: Reference, jsonOnly: boolean): RequestBody | null {
    const def = (ref.definitions || []).find((d) => d?.type === "$rest.request.body");
    if (!def) {
        return null;
    }
    const required = metaFlag(def.meta, "required");
    const variants = def.variants || [];
    if (variants.length) {
        const json = variants.find((v) => isJsonContentType(variantContentType(v) || v.title || ""));
        const variant = json || (jsonOnly ? undefined : variants[0]);
        if (!variant) {
            return null;
        }
        return {
            contentType: variantContentType(variant) || variant.title || "",
            description: text(variant.description),
            properties: variant.properties || [],
            rootProperty: variant.rootProperty,
            required,
        };
    }
    // A converter that puts the body on the definition itself (no variants).
    const contentType = findMeta(def.meta, "contentType")?.value;
    return {
        contentType: typeof contentType === "string" ? contentType : "application/json",
        description: text(def.description),
        properties: def.properties || [],
        rootProperty: def.rootProperty,
        required,
    };
}

/** The response variants (one per status and content type). */
export function responseVariants(ref: Reference): DefinitionVariant[] {
    const out: DefinitionVariant[] = [];
    for (const def of ref.definitions || []) {
        if (def?.type === "return") {
            out.push(...(def.variants || []));
        }
    }
    return out;
}

const OF_TYPES = new Set(["$$array", "$$union", "$$xor"]);

/** What a plain-typed property with an `ofProperty` stands for: an `allOf`
 *  wrapper around an array or a union reads as that array or union, with the
 *  wrapper's name, its description when it has one, and its meta first. */
export function effectiveProperty(prop: DefinitionProperty): DefinitionProperty {
    let current = prop;
    // Bounded: a malformed tree cannot loop.
    for (let i = 0; i < 16; i++) {
        const of = current.ofProperty;
        if (current.type === "$$enum" || current.type === "$$array" || !of || !OF_TYPES.has(of.type)) {
            return current;
        }
        current = {
            ...of,
            name: current.name,
            description: text(current.description) || text(of.description),
            meta: [...(current.meta || []), ...(of.meta || [])],
        };
    }
    return current;
}

/** An array's item: its `ofProperty`, else a single unnamed child, else an
 *  object of its children (the shape a request/response root array takes). */
export function arrayItem(prop: DefinitionProperty): DefinitionProperty | undefined {
    if (prop.ofProperty) {
        return prop.ofProperty;
    }
    const children = prop.properties || [];
    if (children.length === 1 && !children[0].name) {
        return children[0];
    }
    if (children.length) {
        return { name: "", type: "object", description: "", properties: children };
    }
    return undefined;
}

const JSON_SCHEMA_TYPES = new Set(["string", "integer", "number", "boolean", "object", "array", "null"]);

export function jsonSchemaType(type: unknown): string | undefined {
    return typeof type === "string" && JSON_SCHEMA_TYPES.has(type) ? type : undefined;
}

/** The base type of a `$$enum`: its `ofProperty` type, else its `enum-type` meta. */
export function enumBaseType(prop: DefinitionProperty): string {
    const of = prop.ofProperty?.type;
    if (typeof of === "string" && of) {
        return of;
    }
    const meta = findMeta(prop.meta, "enum-type")?.value;
    return typeof meta === "string" ? meta : "";
}

const NUMERIC = /^-?\d+(\.\d+)?$/;

/** An enum value back from the converter's string child names: numbers and
 *  booleans for numeric and boolean enums, `null` for a non-string one. */
export function enumValue(name: string, baseType: string): unknown {
    if ((baseType === "integer" || baseType === "number") && NUMERIC.test(name)) {
        return Number(name);
    }
    if (baseType === "boolean" && (name === "true" || name === "false")) {
        return name === "true";
    }
    if (baseType !== "" && baseType !== "string" && name === "null") {
        return null;
    }
    return name;
}

/** Whether a property (after `effectiveProperty`) is an object with children. */
export function objectChildren(prop: DefinitionProperty): DefinitionProperty[] {
    if (prop.type === "$$enum" || prop.type === "$$array" || prop.type === "$$union" || prop.type === "$$xor") {
        return [];
    }
    return prop.properties || [];
}

const LIST_ITEM = /^([-*+]|[0-9]+[.)])[ \t]/;

/** Whether a line opens a markdown block of its own: indented, a list item,
 *  a heading, a quote, a table row or a code fence. */
function startsBlock(line: string): boolean {
    const first = line[0];
    return (
        first === " " ||
        first === "\t" ||
        first === "#" ||
        first === ">" ||
        first === "|" ||
        line.startsWith("```") ||
        LIST_ITEM.test(line)
    );
}

/** Joins hard-wrapped lines (a YAML description folded at 80 columns): a
 *  single newline becomes a space, unless a line around it is blank or the
 *  next line opens a block of its own. Paragraph breaks stay. */
export function unwrapSoftBreaks(value: string): string {
    const lines = value.split("\n");
    let out = lines[0];
    for (let i = 1; i < lines.length; i++) {
        const line = lines[i];
        if (lines[i - 1].trim() === "" || line.trim() === "" || startsBlock(line)) {
            out += "\n" + line;
        } else {
            out = out.trimEnd() + " " + line;
        }
    }
    return out;
}
