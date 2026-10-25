import {OpenAPIV3} from "openapi-types";
import {JSONSchema} from "@apidevtools/json-schema-ref-parser";

/** Where the object pages (the component schemas) sit in the generated
 *  navigation — `uniformOasOptions.objectsLayout`. */
export type ObjectsLayout = "grouped" | "per-area"

/** How a page without an explicit `x-docs.path` gets its canonical —
 *  `uniformOasOptions.operationPaths`. */
export type OperationPaths = "operation-id" | "title"

export interface uniformOasOptions {
    regions?: string[] // Format: 'METHOD /path' e.g. 'GET /users'

    /** The name of the component-schema pages' group: the top-level objects
     *  group, and under `objectsLayout: "per-area"` the sub-group an object
     *  page with an `x-docs.group` is nested under inside that group. Default
     *  `"Objects"`. */
    objectsGroup?: string

    /** The layout of the object pages in the generated navigation (no
     *  `x-docs.sidebar`):
     *  - `"grouped"` (the default): ONE top-level objects group, the last group
     *    of the navigation — the pages without a resource group first, then
     *    one sub-group per resource group (`x-docs.group`) in the order the
     *    groups appear above it (`context.group = ["Objects", "<group>"]`; a
     *    renderer that starts nested groups collapsed shows them folded);
     *  - `"per-area"`: an object page with a resource group is nested under
     *    that group's own objects sub-group, after its operations
     *    (`context.group = ["<group>", "Objects"]`); the pages without one
     *    keep the top-level objects group.
     *  Anything else means the default. */
    objectsLayout?: ObjectsLayout

    /** How a page without an `x-docs.path` gets its canonical (applied by the
     *  x-docs pass, like the objects layout):
     *  - `"operation-id"` (the default): the converter's canonical — the
     *    `operationId` (else a slug of the summary, else `METHOD/path`) for an
     *    operation, `objects/<Name>` for a component schema;
     *  - `"title"`: an operation derives `<group>/[<sub-resource>/]<verb>` from
     *    its group and a verb-first summary ("Create widget" in `widgets` →
     *    `widgets/create`, "List widget parts" → `widgets/parts/list`), falling
     *    back to the default when the summary is not verb-first; a component
     *    page derives `<group>/<label>` (the label its constant `object` value
     *    verbatim, else its name in kebab-case) or `objects/<label>` without a
     *    group. See xdocs/pagePaths.ts for the grammar.
     *  Anything else means the default. */
    operationPaths?: OperationPaths
}

export interface SelectorMethod {
    oapPath: OpenAPIV3.PathItemObject;

    path: string;

    httpMethod: "get" | "put" | "post" | "delete" | "patch";
}

export type OasJSONSchema = JSONSchema & {
    __internal_getRefPath?: () => string | string[];
}
