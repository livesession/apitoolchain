// Reference[] → MCP tool definitions — one tool per REST operation.
//
// A pure JavaScript implementation: an MCP server built on it may run where
// @xyd-js/native is absent, so this never reaches for the addon. The Rust twin
// (crates/apitoolchain_uniform/src/mcp.rs) is pinned to it byte for byte by the
// 5.mcp-tools.* fixtures; a change here is a change there.
import type { DefinitionProperty, Reference } from "./types";
import {
    arrayItem,
    definitionProperties,
    effectiveProperty,
    enumBaseType,
    enumValue,
    findMeta,
    isRequired,
    isRestOperation,
    jsonSchemaType,
    metaFlag,
    objectChildren,
    operationGroup,
    operationMethod,
    operationPath,
    requestBody,
    restContext,
    text,
    unwrapSoftBreaks,
} from "./operations";

/** A JSON Schema node, as emitted (plain JSON). */
export type McpJsonSchema = { [key: string]: unknown };

/** The tool's input schema: always an object. */
export interface McpToolInputSchema extends McpJsonSchema {
    type: "object";
    properties: { [name: string]: McpJsonSchema };
    required?: string[];
    /** `false`, unless a flattened request body declares itself open
     *  (`additionalProperties: true` or a schema), which is carried through. */
    additionalProperties: boolean | McpJsonSchema;
}

/** The input layout of a tool (see `McpToolBinding.layout`). */
export type McpToolLayout = "flat" | "grouped";

/** Where a renamed input goes on the HTTP request, and under which name. */
export interface McpToolInput {
    in: "path" | "query" | "body";
    name: string;
}

/** MCP tool annotations (hints a client may show or act on). */
export interface McpToolAnnotations {
    readOnlyHint: boolean;
    destructiveHint: boolean;
    idempotentHint: boolean;
    openWorldHint: boolean;
}

/** How a call maps back onto the HTTP request. Not part of the MCP tool
 *  definition: a server keeps it and strips it before listing the tool. */
export interface McpToolBinding {
    /** Upper-case HTTP method. */
    method: string;
    /** The operation path with its `{param}` placeholders. */
    path: string;
    /** The first server's URL joined with the path (the path when unknown). */
    fullPath: string;
    /** How the inputs are laid out: `"flat"` (path, query and body inputs at
     *  the top level) or `"grouped"` (under `pathParams` / `queryParams` /
     *  `headers` / `cookies` / `requestBody`) — the `flatten` option. A caller
     *  reads the arguments by it, never by guessing from property names. */
    layout: McpToolLayout;
    /** The input properties that fill the path placeholders (input names —
     *  see `inputs` for one that was renamed). */
    pathParams: string[];
    /** The input properties sent as the query string (input names — see
     *  `inputs` for one that was renamed). */
    queryParams: string[];
    /** `"json"` when the operation takes a JSON request body. */
    body: "json" | null;
    /** The one input property that holds the whole JSON body, when there is
     *  one. A caller sends `arguments[bodyProperty]` as the request body as it
     *  is (an array stays an array). It is set when the body cannot be
     *  flattened — it is not an object with known properties (an array, a
     *  union, a free-form object), or one of its properties shares a
     *  parameter's name — as `body` (`requestBody` if a parameter is called
     *  `body`), and always in the grouped layout (`requestBody`, beside
     *  `pathParams` / `queryParams` / `headers` / `cookies`).
     *
     *  Absent in the flat layout when the body's properties sit at the top
     *  level: the body is then the object of every argument not named in
     *  `pathParams` or `queryParams`. */
    bodyProperty?: string;
    /** The inputs renamed in the flat layout, by input name: a query parameter
     *  or a body property that shares an earlier input's name (path
     *  parameters first, then query parameters, then body properties) is
     *  emitted as `<name>_query` / `<name>_body` (`_2`, `_3`… if taken), and
     *  this maps it back — `{ id_query: { in: "query", name: "id" } }`. Absent
     *  when nothing was renamed; an input not listed goes out under its own name. */
    inputs?: { [input: string]: McpToolInput };
    /** `context.group` without empty entries. */
    group: string[];
    /** The reference's canonical (an OpenAPI operationId, when it has one). */
    canonical: string;
}

export interface McpTool {
    name: string;
    title: string;
    description: string;
    inputSchema: McpToolInputSchema;
    annotations: McpToolAnnotations;
    binding: McpToolBinding;
    /** Whatever `options.extras` returned for the reference (omitted when nothing). */
    extras?: { [key: string]: unknown };
}

export interface McpToolsOptions {
    /** The tool name, used as given (an empty one falls back to the default).
     *  Default: the canonical verbatim when it matches `^[A-Za-z0-9_-]{1,128}$`,
     *  else a slug of it (or of `METHOD path`). Every name is made unique in
     *  reference order by a `_2`, `_3`… suffix. */
    toolName?: (ref: Reference) => string;
    /** Path, query and JSON-body properties at the top level (default `true`);
     *  `false` groups them under `pathParams` / `queryParams` / `headers` /
     *  `cookies` / `requestBody`. Header and cookie parameters only appear in
     *  the grouped layout. */
    flatten?: boolean;
    /** Which REST operations become tools. Default: every one whose request
     *  body, if it has one, has a JSON variant — an operation that only takes
     *  a form, multipart or binary body is skipped, since a tool has no way to
     *  send it. A predicate given here is the whole decision: an operation it
     *  includes without a JSON body becomes a tool with no body input and
     *  `binding.body: null`. */
    include?: (ref: Reference) => boolean;
    /** `readOnlyHint` (default: GET, HEAD, OPTIONS, TRACE). */
    readOnly?: (ref: Reference) => boolean;
    /** `destructiveHint` (default: DELETE, PUT, PATCH — they remove or
     *  overwrite; POST only adds). */
    destructive?: (ref: Reference) => boolean;
    /** `idempotentHint` (default: GET, HEAD, OPTIONS, TRACE, PUT, DELETE). */
    idempotent?: (ref: Reference) => boolean;
    /** `openWorldHint` (default: false). */
    openWorld?: (ref: Reference) => boolean;
    /** Caps a parameter or top-level body property named `limit` of type
     *  integer (a whole number, at least 1) unless its own `maximum` is lower:
     *  `maximum` becomes the cap, a larger default comes down to it, and the
     *  description ends "Capped at N by this server." in place of an "at most
     *  N" clause it carried (removed with the comma, semicolon or conjunction
     *  that joined it). */
    limitCap?: number;
    /** The tool description is the first sentence of the reference's
     *  description (hard-wrapped lines joined first) cut to this many
     *  characters (default 240; 0 = no cut), followed by ` — METHOD path`
     *  (just `METHOD path` without one). A sentence ends at `.`, `!` or `?`
     *  followed by the end of the line, or by whitespace and an uppercase
     *  letter, a digit, a quote or a bracket — never at the period of an
     *  abbreviation (`e.g.`, `i.e.`, a single initial, `No.`, `vs.`, `cf.`…). */
    descriptionChars?: number;
    /** How deep objects are spelled out (default 7). A top-level input
     *  property is at depth 1, its properties at depth 2, and so on; an object
     *  deeper than `maxDepth` is emitted as `{ type: "object" }` (with its
     *  description) without its properties. */
    maxDepth?: number;
    /** Extra data carried on each tool as `extras` (e.g. a documentation path). */
    extras?: (ref: Reference) => { [key: string]: unknown } | undefined | null;
}

export const MCP_TOOL_NAME_PATTERN = /^[A-Za-z0-9_-]{1,128}$/;

const MAX_TOOL_NAME = 128;
const SAFE_METHODS = new Set(["GET", "HEAD", "OPTIONS", "TRACE"]);
const DESTRUCTIVE_METHODS = new Set(["DELETE", "PUT", "PATCH"]);

/** A request body without a JSON variant (form, multipart, binary only). */
function onlyNonJsonBody(ref: Reference): boolean {
    return requestBody(ref, false) !== null && requestBody(ref, true) === null;
}
const DEFAULT_DESCRIPTION_CHARS = 240;
const DEFAULT_MAX_DEPTH = 7;

/**
 * One MCP tool per REST operation in `references` (component schemas and
 * non-REST references are skipped), in reference order.
 */
export function uniformToMcpTools(references: Reference[], options: McpToolsOptions = {}): McpTool[] {
    const settings: Settings = {
        flatten: options.flatten !== false,
        limitCap:
            typeof options.limitCap === "number" && options.limitCap >= 1 ? Math.floor(options.limitCap) : undefined,
        descriptionChars:
            typeof options.descriptionChars === "number" && options.descriptionChars >= 0
                ? Math.floor(options.descriptionChars)
                : DEFAULT_DESCRIPTION_CHARS,
        maxDepth:
            typeof options.maxDepth === "number" && options.maxDepth >= 0
                ? Math.floor(options.maxDepth)
                : DEFAULT_MAX_DEPTH,
    };

    const used = new Set<string>();
    const tools: McpTool[] = [];

    for (const ref of references || []) {
        if (!isRestOperation(ref)) {
            continue;
        }
        if (options.include ? !options.include(ref) : onlyNonJsonBody(ref)) {
            continue;
        }

        const method = operationMethod(ref);
        const path = operationPath(ref);
        const ctx = restContext(ref);

        const custom = options.toolName ? options.toolName(ref) : "";
        const name = uniqueName(custom || defaultToolName(ref.canonical, method, path), used);

        const { inputSchema, binding } = inputOf(ref, settings);

        const tool: McpTool = {
            name,
            title: ref.title || name,
            description: toolDescription(ref, method, path, settings.descriptionChars),
            inputSchema,
            annotations: {
                readOnlyHint: options.readOnly ? !!options.readOnly(ref) : SAFE_METHODS.has(method),
                destructiveHint: options.destructive ? !!options.destructive(ref) : DESTRUCTIVE_METHODS.has(method),
                idempotentHint: options.idempotent
                    ? !!options.idempotent(ref)
                    : SAFE_METHODS.has(method) || method === "PUT" || method === "DELETE",
                openWorldHint: options.openWorld ? !!options.openWorld(ref) : false,
            },
            binding: {
                method,
                path,
                fullPath: typeof ctx.fullPath === "string" && ctx.fullPath ? ctx.fullPath : path,
                layout: settings.flatten ? "flat" : "grouped",
                pathParams: binding.pathParams,
                queryParams: binding.queryParams,
                body: binding.body,
                ...(binding.bodyProperty !== undefined ? { bodyProperty: binding.bodyProperty } : {}),
                ...(binding.inputs !== undefined ? { inputs: binding.inputs } : {}),
                group: operationGroup(ref),
                canonical: ref.canonical || "",
            },
        };

        const extras = options.extras ? options.extras(ref) : undefined;
        if (extras && typeof extras === "object" && Object.keys(extras).length) {
            tool.extras = extras;
        }

        tools.push(tool);
    }

    return tools;
}

interface Settings {
    flatten: boolean;
    limitCap: number | undefined;
    descriptionChars: number;
    maxDepth: number;
}

// ---------------------------------------------------------------------------
// names and descriptions
// ---------------------------------------------------------------------------

function defaultToolName(canonical: string | undefined, method: string, path: string): string {
    const c = canonical || "";
    if (MCP_TOOL_NAME_PATTERN.test(c)) {
        return c;
    }
    return slugToolName(c) || slugToolName(`${method} ${path}`) || "tool";
}

/** Runs of characters outside `[A-Za-z0-9_-]` become one `_`; `_` trimmed at
 *  both ends; at most 128 characters. */
function slugToolName(value: string): string {
    const slug = value.replace(/[^A-Za-z0-9_-]+/g, "_").replace(/^_+|_+$/g, "");
    return slug.slice(0, MAX_TOOL_NAME).replace(/_+$/, "");
}

function uniqueName(name: string, used: Set<string>): string {
    let candidate = name;
    for (let n = 2; used.has(candidate); n++) {
        const suffix = `_${n}`;
        candidate = Array.from(name).slice(0, Math.max(0, MAX_TOOL_NAME - suffix.length)).join("") + suffix;
    }
    used.add(candidate);
    return candidate;
}

function toolDescription(ref: Reference, method: string, path: string, chars: number): string {
    // No title fallback: a REST reference without a description has no
    // summary either, so its title is the operationId or `method/path`.
    let summary = firstSentence(firstLine(unwrapSoftBreaks(text(ref.description).trim())));
    if (chars > 0) {
        summary = truncate(summary, chars);
    }
    const route = `${method} ${path}`;
    return summary ? `${summary} — ${route}` : route;
}

function firstLine(value: string): string {
    for (const line of value.split("\n")) {
        const trimmed = line.trim();
        if (trimmed) {
            return trimmed;
        }
    }
    return "";
}

/** Up to the first sentence end (see `descriptionChars`), else the whole line. */
function firstSentence(line: string): string {
    const chars = Array.from(line);
    for (let i = 0; i < chars.length; i++) {
        const c = chars[i];
        if ((c === "." || c === "!" || c === "?") && endsSentence(chars, i)) {
            return chars.slice(0, i + 1).join("");
        }
    }
    return line;
}

/** Words whose period is not a sentence end (compared lower-case). */
const ABBREVIATIONS = new Set(["approx", "ca", "cf", "excl", "fig", "incl", "no", "nr", "vs"]);
/** A single letter, or letters joined by periods (`e.g`, `i.e`, `U.S`). */
const INITIALS = /^[A-Za-z](\.[A-Za-z])*$/;
const SENTENCE_OPENERS = "\"'\u201C\u2018([`";

function isBlank(c: string | undefined): boolean {
    return c === " " || c === "\t";
}

/** Whether the terminator at `i` ends a sentence: the end of the line, or
 *  whitespace then the end or a sentence opener — and, for a period, not
 *  after an abbreviation. */
function endsSentence(chars: string[], i: number): boolean {
    let j = i + 1;
    if (j < chars.length) {
        if (!isBlank(chars[j])) {
            return false;
        }
        while (isBlank(chars[j])) {
            j++;
        }
        if (j < chars.length && !opensSentence(chars[j])) {
            return false;
        }
    }
    if (chars[i] !== ".") {
        return true;
    }
    let k = i;
    while (k > 0 && !isBlank(chars[k - 1])) {
        k--;
    }
    const word = chars.slice(k, i).join("").replace(/^[^A-Za-z0-9]+/, "");
    return !(INITIALS.test(word) || ABBREVIATIONS.has(word.toLowerCase()));
}

/** An uppercase letter (one with a lowercase form), a digit, a quote or a bracket. */
function opensSentence(c: string): boolean {
    return (c >= "0" && c <= "9") || SENTENCE_OPENERS.includes(c) || c.toLowerCase() !== c;
}

/** At most `max` characters (code points), the last one an ellipsis when cut. */
function truncate(value: string, max: number): string {
    const chars = Array.from(value);
    if (chars.length <= max) {
        return value;
    }
    return chars.slice(0, Math.max(0, max - 1)).join("").trimEnd() + "…";
}

// ---------------------------------------------------------------------------
// the input schema
// ---------------------------------------------------------------------------

interface InputBinding {
    pathParams: string[];
    queryParams: string[];
    body: "json" | null;
    bodyProperty?: string;
    inputs?: { [input: string]: McpToolInput };
}

function inputOf(ref: Reference, settings: Settings): { inputSchema: McpToolInputSchema; binding: InputBinding } {
    const pathProps = definitionProperties(ref, "$rest.param.path");
    const queryProps = definitionProperties(ref, "$rest.param.query");
    const body = requestBody(ref, true);

    const binding: InputBinding = { pathParams: [], queryParams: [], body: body ? "json" : null };
    const properties: { [name: string]: McpJsonSchema } = {};
    const required: string[] = [];
    let additionalProperties: boolean | McpJsonSchema = false;

    if (settings.flatten) {
        const inputs: { [input: string]: McpToolInput } = {};
        addParameters(pathProps, properties, required, binding.pathParams, settings, "path", inputs);
        addParameters(queryProps, properties, required, binding.queryParams, settings, "query", inputs);

        if (body) {
            const names = body.properties.map((p) => p.name);
            const flattenable =
                !body.rootProperty &&
                body.properties.length > 0 &&
                names.every((n) => !!n) &&
                new Set(names).size === names.length;

            if (flattenable) {
                // An optional body's required properties are required only
                // when the body is sent, so none is a required input.
                addParameters(
                    body.properties,
                    properties,
                    body.required ? required : [],
                    null,
                    settings,
                    "body",
                    inputs
                );
                if (body.additionalProperties !== undefined) {
                    additionalProperties = body.additionalProperties as boolean | McpJsonSchema;
                }
            } else {
                const key = bodyKey(properties);
                properties[key] = bodySchema(
                    body.rootProperty,
                    body.properties,
                    body.description,
                    settings,
                    body.additionalProperties
                );
                if (body.required) {
                    required.push(key);
                }
                binding.bodyProperty = key;
            }
        }
        if (Object.keys(inputs).length) {
            binding.inputs = inputs;
        }
    } else {
        const groups: [string, DefinitionProperty[], string[] | null][] = [
            ["pathParams", pathProps, binding.pathParams],
            ["queryParams", queryProps, binding.queryParams],
            ["headers", definitionProperties(ref, "$rest.param.header"), null],
            ["cookies", definitionProperties(ref, "$rest.param.cookie"), null],
        ];
        for (const [key, props, names] of groups) {
            if (!props.length) {
                continue;
            }
            const groupProperties: { [name: string]: McpJsonSchema } = {};
            const groupRequired: string[] = [];
            addParameters(props, groupProperties, groupRequired, names, settings);
            properties[key] = closedObject(groupProperties, groupRequired);
            if (groupRequired.length) {
                required.push(key);
            }
        }
        if (body) {
            properties.requestBody = bodySchema(
                body.rootProperty,
                body.properties,
                body.description,
                settings,
                body.additionalProperties
            );
            if (body.required) {
                required.push("requestBody");
            }
            binding.bodyProperty = "requestBody";
        }
    }

    return {
        inputSchema: closedObject(properties, required, additionalProperties) as McpToolInputSchema,
        binding,
    };
}

/** `body`, else `requestBody`, else `body_2`, `body_3`… — whichever no
 *  parameter has taken. */
function bodyKey(properties: { [name: string]: McpJsonSchema }): string {
    if (!has(properties, "body")) {
        return "body";
    }
    if (!has(properties, "requestBody")) {
        return "requestBody";
    }
    let n = 2;
    while (has(properties, `body_${n}`)) {
        n++;
    }
    return `body_${n}`;
}

/**
 * Adds one location's properties as inputs. A name repeated within the
 * location is skipped; a name an earlier location already took is renamed
 * `<name>_<location>` (`_2`, `_3`… if that is taken too) and recorded in
 * `inputs` — only when `location` and `inputs` are given (the flat layout).
 */
function addParameters(
    props: DefinitionProperty[],
    properties: { [name: string]: McpJsonSchema },
    required: string[],
    names: string[] | null,
    settings: Settings,
    location?: McpToolInput["in"],
    inputs?: { [input: string]: McpToolInput }
) {
    const seen = new Set<string>();
    for (const prop of props) {
        const name = prop.name;
        if (!name || seen.has(name)) {
            continue;
        }
        seen.add(name);
        let input = name;
        if (has(properties, name)) {
            if (!location || !inputs) {
                continue;
            }
            input = `${name}_${location}`;
            for (let n = 2; has(properties, input); n++) {
                input = `${name}_${location}_${n}`;
            }
            inputs[input] = { in: location, name };
        }
        properties[input] = propertySchema(prop, 1, settings, true);
        names?.push(input);
        if (isRequired(prop)) {
            required.push(input);
        }
    }
}

function closedObject(
    properties: { [name: string]: McpJsonSchema },
    required: string[],
    additionalProperties: boolean | McpJsonSchema = false
): McpJsonSchema {
    const out: McpJsonSchema = { type: "object", properties };
    if (required.length) {
        out.required = required;
    }
    out.additionalProperties = additionalProperties;
    return out;
}

/** A body that is one input property: its root property (an array, a union),
 *  or an object of its properties (open when it has none). */
function bodySchema(
    rootProperty: DefinitionProperty | undefined,
    props: DefinitionProperty[],
    description: string,
    settings: Settings,
    additionalProperties?: unknown
): McpJsonSchema {
    description = unwrapSoftBreaks(description);
    if (rootProperty) {
        return propertySchema(rootProperty, 0, settings, false, description);
    }
    const fields: Fields = { type: "object" };
    if (description) {
        fields.description = description;
    }
    if (props.length) {
        const properties: { [name: string]: McpJsonSchema } = {};
        const required: string[] = [];
        for (const prop of props) {
            if (!prop.name || has(properties, prop.name)) {
                continue;
            }
            properties[prop.name] = propertySchema(prop, 1, settings, true);
            if (isRequired(prop)) {
                required.push(prop.name);
            }
        }
        fields.properties = properties;
        if (required.length) {
            fields.required = required;
        }
    }
    if (additionalProperties !== undefined) {
        fields.additionalProperties = additionalProperties;
    }
    return emit(fields);
}

function has(object: object, key: string): boolean {
    return Object.prototype.hasOwnProperty.call(object, key);
}

/** The fields of a JSON Schema node, emitted in this fixed order. */
interface Fields {
    type?: string | string[];
    title?: string;
    description?: string;
    enum?: unknown[];
    const?: { value: unknown };
    items?: McpJsonSchema;
    properties?: { [name: string]: McpJsonSchema };
    required?: string[];
    additionalProperties?: unknown;
    anyOf?: McpJsonSchema[];
    oneOf?: McpJsonSchema[];
    default?: { value: unknown };
    minimum?: unknown;
    maximum?: unknown;
    examples?: unknown[];
    deprecated?: boolean;
}

function emit(f: Fields): McpJsonSchema {
    const out: McpJsonSchema = {};
    if (f.type !== undefined) out.type = f.type;
    if (f.title !== undefined) out.title = f.title;
    if (f.description !== undefined) out.description = f.description;
    if (f.enum !== undefined) out.enum = f.enum;
    if (f.const !== undefined) out.const = f.const.value;
    if (f.items !== undefined) out.items = f.items;
    if (f.properties !== undefined) out.properties = f.properties;
    if (f.required !== undefined) out.required = f.required;
    if (f.additionalProperties !== undefined) out.additionalProperties = f.additionalProperties;
    if (f.anyOf !== undefined) out.anyOf = f.anyOf;
    if (f.oneOf !== undefined) out.oneOf = f.oneOf;
    if (f.default !== undefined) out.default = f.default.value;
    if (f.minimum !== undefined) out.minimum = f.minimum;
    if (f.maximum !== undefined) out.maximum = f.maximum;
    if (f.examples !== undefined) out.examples = f.examples;
    if (f.deprecated !== undefined) out.deprecated = f.deprecated;
    return out;
}

/**
 * A uniform property as JSON Schema, at nesting `level` (a top-level input
 * property is level 1). `top` marks a parameter or top-level body property —
 * the only place the `limit` cap applies. `fallbackDescription` stands in for
 * an empty description (a root body takes its schema's); `title` names a
 * union alternative.
 */
function propertySchema(
    raw: DefinitionProperty,
    level: number,
    settings: Settings,
    top: boolean,
    fallbackDescription = "",
    title?: string
): McpJsonSchema {
    const prop = effectiveProperty(raw);
    const f: Fields = {};
    if (title) f.title = title;
    const description = unwrapSoftBreaks(text(prop.description)) || fallbackDescription;

    switch (prop.type) {
        case "$$enum": {
            const base = enumBaseType(prop);
            const type = jsonSchemaType(base);
            if (type) f.type = type;
            if (description) f.description = description;
            // A nullable enum gets a real `null` below; the converter's
            // `"null"` child (a null member) would read as the string.
            const nullable = metaFlag(prop.meta, "nullable");
            f.enum = (prop.properties || [])
                .filter((value) => !(nullable && value.name === "null"))
                .map((value) => enumValue(String(value.name ?? ""), base));
            break;
        }
        case "$$array": {
            f.type = "array";
            if (description) f.description = description;
            const item = arrayItem(prop);
            if (item) {
                f.items = propertySchema(item, level + 1, settings, false);
            }
            break;
        }
        case "$$union":
        case "$$xor": {
            if (description) f.description = description;
            // An alternative named apart from its union (a oneOf member's
            // schema title) keeps the name as its `title`.
            const alternatives = (prop.properties || []).map((alt) =>
                propertySchema(alt, level, settings, false, "", alt.name && alt.name !== prop.name ? alt.name : undefined)
            );
            if (prop.type === "$$union") {
                f.anyOf = alternatives;
            } else {
                f.oneOf = alternatives;
            }
            break;
        }
        default: {
            const children = objectChildren(prop);
            const type = jsonSchemaType(prop.type) || (children.length ? "object" : undefined);
            if (type) f.type = type;
            if (description) f.description = description;
            if (children.length && level <= settings.maxDepth) {
                f.properties = {};
                const required: string[] = [];
                for (const child of children) {
                    if (!child.name || has(f.properties, child.name)) {
                        continue;
                    }
                    f.properties[child.name] = propertySchema(child, level + 1, settings, false);
                    if (isRequired(child)) {
                        required.push(child.name);
                    }
                }
                if (required.length) {
                    f.required = required;
                }
            }
        }
    }

    const constant = findMeta(prop.meta, "const");
    if (constant && "value" in constant) {
        f.const = { value: constant.value };
    }
    const defaults = findMeta(prop.meta, "defaults");
    if (defaults && "value" in defaults) {
        f.default = { value: defaults.value };
    }
    const minimum = findMeta(prop.meta, "minimum")?.value;
    if (minimum !== undefined) f.minimum = minimum;
    const maximum = findMeta(prop.meta, "maximum")?.value;
    if (maximum !== undefined) f.maximum = maximum;

    const examples = examplesOf(prop);
    if (examples) f.examples = examples;

    if (metaFlag(prop.meta, "deprecated")) {
        f.deprecated = true;
    }

    if (metaFlag(prop.meta, "nullable")) {
        makeNullable(f);
    }

    if (top && settings.limitCap !== undefined && prop.name === "limit" && isIntegerType(f.type)) {
        capLimit(f, settings.limitCap);
    }

    return emit(f);
}

function examplesOf(prop: DefinitionProperty): unknown[] | undefined {
    const many = findMeta(prop.meta, "examples");
    if (many && "value" in many) {
        return Array.isArray(many.value) ? many.value : [many.value];
    }
    const one = findMeta(prop.meta, "example");
    if (one && "value" in one) {
        return [one.value];
    }
    if (Array.isArray(prop.examples)) {
        return prop.examples.length ? prop.examples : undefined;
    }
    if (typeof prop.examples === "string" && prop.examples) {
        return [prop.examples];
    }
    return undefined;
}

function makeNullable(f: Fields) {
    if (typeof f.type === "string") {
        if (f.type !== "null") {
            f.type = [f.type, "null"];
        }
    } else if (f.anyOf && !f.anyOf.some((alt) => alt.type === "null")) {
        f.anyOf.push({ type: "null" });
    } else if (f.oneOf && !f.oneOf.some((alt) => alt.type === "null")) {
        f.oneOf.push({ type: "null" });
    }
    if (f.enum && !f.enum.includes(null)) {
        f.enum.push(null);
    }
}

function isIntegerType(type: Fields["type"]): boolean {
    return type === "integer" || (Array.isArray(type) && type[0] === "integer");
}

/** The cap binds unless the API's own `maximum` is already within it: the
 *  maximum and a larger default come down to the cap, and the description
 *  says so in place of the "at most N" clause it may carry. */
function capLimit(f: Fields, cap: number) {
    if (typeof f.maximum === "number" && f.maximum <= cap) {
        return;
    }
    f.maximum = cap;
    if (f.default !== undefined && typeof f.default.value === "number" && f.default.value > cap) {
        f.default = { value: cap };
    }
    f.description = withSentence(stripAtMost(f.description || ""), `Capped at ${cap} by this server.`);
}

const CLAUSE_END = new Set([".", ",", ";", ")"]);
const CONJUNCTIONS = new Set(["and", "or", "but"]);

/** Drops an "at most N" clause (any case) that ends a clause — `…, at most 200.`,
 *  `… and at most 200`, `…; at most 200` or `… (at most 200).` — with what joins
 *  it to the sentence: the comma or semicolon, a conjunction (`and`, `or`,
 *  `but`), the parentheses. Nothing but punctuation left reads as no
 *  description. */
export function stripAtMost(description: string): string {
    const chars = Array.from(description);
    const pattern = Array.from("at most");
    const lower = (c: string | undefined) => (c !== undefined && c >= "A" && c <= "Z" ? c.toLowerCase() : c);
    const alnum = (c: string | undefined) => c !== undefined && /^[A-Za-z0-9]$/.test(c);
    const digit = (c: string | undefined) => c !== undefined && c >= "0" && c <= "9";
    const space = (c: string | undefined) => c === " " || c === "\t";
    let i = 0;
    while (i < chars.length) {
        if (!alnum(chars[i - 1]) && pattern.every((p, k) => lower(chars[i + k]) === p)) {
            let end = i + pattern.length;
            const spaced = space(chars[end]);
            while (space(chars[end])) end++;
            const numbered = digit(chars[end]);
            while (digit(chars[end])) end++;
            let start = i;
            if (chars[start - 1] === "(" && chars[end] === ")") {
                start--;
                end++;
            }
            if (spaced && numbered && (end === chars.length || CLAUSE_END.has(chars[end]))) {
                while (start > 0 && space(chars[start - 1])) start--;
                // a conjunction joining the clause on: " and at most 200"
                let word = start;
                while (word > 0 && /^[A-Za-z]$/.test(chars[word - 1])) word--;
                if (word < start && !alnum(chars[word - 1]) && CONJUNCTIONS.has(chars.slice(word, start).join("").toLowerCase())) {
                    start = word;
                    while (start > 0 && space(chars[start - 1])) start--;
                }
                if (start > 0 && (chars[start - 1] === "," || chars[start - 1] === ";")) start--;
                chars.splice(start, end - start);
                i = start;
                continue;
            }
        }
        i++;
    }
    const out = chars.join("");
    return Array.from(out).every((c) => " \t.,;:()".includes(c)) ? "" : out;
}

function withSentence(description: string | undefined, sentence: string): string {
    const d = (description || "").trimEnd();
    if (!d) {
        return sentence;
    }
    const last = d[d.length - 1];
    return last === "." || last === "!" || last === "?" ? `${d} ${sentence}` : `${d}. ${sentence}`;
}
