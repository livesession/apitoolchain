import {OpenAPIV3} from "openapi-types";
import {JSONSchema} from "@apidevtools/json-schema-ref-parser";

export interface uniformOasOptions {
    regions?: string[] // Format: 'METHOD /path' e.g. 'GET /users'

    /** The name of the component-schema pages' group — the flat top-level
     *  group of the schemas with no resource group AND the sub-group an object
     *  page with an `x-docs.group` is nested under inside that group. Default
     *  `"Objects"`. */
    objectsGroup?: string
}

export interface SelectorMethod {
    oapPath: OpenAPIV3.PathItemObject;

    path: string;

    httpMethod: "get" | "put" | "post" | "delete" | "patch";
}

export type OasJSONSchema = JSONSchema & {
    __internal_getRefPath?: () => string | string[];
}
