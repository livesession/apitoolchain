// FROZEN JS implementation of the MCP surface -> Reference[] conversion.
//
// Mirrors crates/apitoolchain_mcp_uniform; deleted once @xyd-js/native ships
// platform binaries. Bugfix-only.
//
// Transport (resolveMcpSurface and friends) moved to ../transport.ts — it runs
// in both modes and therefore outlives this file.

import { schemaObjectToUniformDefinitionProperty } from "@xyd-js/openapi";
import {
    type Reference,
    type DefinitionProperty,
    type MCPReferenceContext,
    ReferenceCategory,
    ReferenceType,
} from "@xyd-js/uniform";

import type { JsonSchemaObject, McpResource, McpTool } from "../types";

import { resolveMcpSurface, type McpUrlToReferencesOptions } from "../transport";

export async function mcpUrlToReferences(
    source: string,
    options: McpUrlToReferencesOptions = {},
): Promise<Reference[]> {
    if (!source) {
        return [];
    }

    const { tools, resources, serverUrl, transport } = await resolveMcpSurface(source, options);

    const references: Reference[] = [];

    for (const tool of tools) {
        references.push(toolToReference(tool, serverUrl, transport));
    }
    for (const resource of resources) {
        references.push(resourceToReference(resource, serverUrl, transport));
    }

    return references;
}

interface McpManifest {
    serverUrl?: string;
    tools?: McpTool[];
    resources?: McpResource[];
}

function toolToReference(
    tool: McpTool,
    serverUrl: string,
    transport: MCPReferenceContext["transport"],
): Reference<MCPReferenceContext> {
    const canonical = slug(tool.name);
    const properties = jsonSchemaPropertiesToDefinitionProperties(tool.inputSchema);

    return {
        title: tool.name,
        description: tool.description || "",
        canonical,
        category: ReferenceCategory.MCP,
        type: ReferenceType.MCP_TOOL,
        context: {
            serverUrl,
            transport,
            toolName: tool.name,
            // pluginNavigation reads context.group to organise the generated
            // sidebar. Without it, no entries are emitted and the tool pages
            // never get routed. Tools live under "Tools".
            group: ["Tools"],
        },
        definitions: [
            {
                title: "Input",
                properties,
            },
        ],
        examples: { groups: [] },
    };
}

function resourceToReference(
    resource: McpResource,
    serverUrl: string,
    transport: MCPReferenceContext["transport"],
): Reference<MCPReferenceContext> {
    const canonical = slug(resource.name || resource.uri);
    return {
        title: resource.name || resource.uri,
        description: resource.description || "",
        canonical,
        category: ReferenceCategory.MCP,
        type: ReferenceType.MCP_RESOURCE,
        context: {
            serverUrl,
            transport,
            resourceUri: resource.uri,
            mimeType: resource.mimeType,
            group: ["Resources"],
        },
        definitions: [
            {
                title: "Resource",
                properties: [
                    {
                        // Atlas only renders `name`, `type`, `description` and
                        // metadata badges visibly in the property card — the
                        // `examples` field doesn't appear in the rendered tree.
                        // For resources the URI and mimeType ARE the
                        // information the page needs to show, so put them in
                        // the description text.
                        name: "uri",
                        type: "string",
                        description: `Resource URI \`${resource.uri}\`.`,
                    },
                    ...(resource.mimeType
                        ? [
                              {
                                  name: "mimeType",
                                  type: "string",
                                  description: `MIME type \`${resource.mimeType}\`.`,
                              } satisfies DefinitionProperty,
                          ]
                        : []),
                ],
            },
        ],
        examples: { groups: [] },
    };
}

/**
 * MCP tool `inputSchema` is JSON Schema with `type: "object"` and a `properties` map.
 * Walk those top-level properties and reuse the OpenAPI JSON-Schema converter for the
 * actual type/recursion handling — `inputSchema.properties[name]` is a SchemaObject.
 */
function jsonSchemaPropertiesToDefinitionProperties(
    schema?: JsonSchemaObject,
): DefinitionProperty[] {
    if (!schema || !schema.properties) {
        return [];
    }
    const required = new Set<string>(
        Array.isArray(schema.required) ? schema.required : [],
    );

    const out: DefinitionProperty[] = [];
    for (const [name, propSchema] of Object.entries(schema.properties)) {
        // `schemaObjectToUniformDefinitionProperty` is typed against OpenAPI's
        // SchemaObject — JSON Schema is a structural subset for the fields we touch,
        // so the cast is intentional and safe.
        const prop = schemaObjectToUniformDefinitionProperty(
            name,
            propSchema as never,
            required.has(name),
        );
        if (prop) {
            out.push(prop);
        }
    }
    return out;
}

function slug(input: string): string {
    return input
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-+|-+$/g, "");
}
