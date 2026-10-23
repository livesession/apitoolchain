import {OpenAPIV3} from "openapi-types";

import {
    DEFINED_DEFINITION_PROPERTY_TYPE,
    DefinitionProperty,
    DefinitionPropertyMeta,
    SymbolDef
} from "@xyd-js/uniform";

import {OasJSONSchema} from "./types";
import {BUILT_IN_PROPERTIES} from "./const";
import {componentLink} from "./component-links";

export function schemaObjectToUniformDefinitionProperties(
    schemaObject: OpenAPIV3.ReferenceObject | OpenAPIV3.SchemaObject,
    rootProperty?: boolean,
    visitedRefs?: Map<string, DefinitionProperty>
): DefinitionProperty[] | DefinitionProperty | null {
    if ('$ref' in schemaObject) {
        console.warn("Reference objects are not supported in schemaObjectToUniformDefinitionProperties");

        return null
    }

    const properties: DefinitionProperty[] = [];

    // Process schema properties
    if ('anyOf' in schemaObject && schemaObject.anyOf) {
        const property = schemaObjectToUniformDefinitionProperty("", schemaObject, false, false, visitedRefs)

        if (property) {
            if (rootProperty) {
                return property
            }

            properties.push(property);
        }
    } else if ('oneOf' in schemaObject && schemaObject.oneOf) {
        const property = schemaObjectToUniformDefinitionProperty("", schemaObject, false, false, visitedRefs)

        if (property) {
            if (rootProperty) {
                return property
            }

            properties.push(property);
        }
    } else if ('allOf' in schemaObject && schemaObject.allOf) {
        const componentPaths: string[] = []

        // Handle allOf by merging properties from all schemas
        for (const schema of schemaObject.allOf) {
            if ('$ref' in schema) {
                console.warn("$ref is not supported in allOf schemas");

                continue; // Skip reference objects
            }

            const oasSchema = schema as OasJSONSchema;
            if (oasSchema.__internal_getRefPath) {
                const refPath = oasSchema.__internal_getRefPath();
                if (typeof refPath === 'string') {
                    componentPaths.push(refPath);
                } else if (Array.isArray(refPath)) {
                    componentPaths.push(...refPath);
                } else {
                    console.warn("Invalid refPath type in allOf schema", oasSchema);
                }
            }

            if ('properties' in schema && schema.properties) {
                for (const [propName, propSchema] of Object.entries(schema.properties)) {
                    if (BUILT_IN_PROPERTIES[propName]) {
                        continue;
                    }
                    if ('$ref' in propSchema) {
                        console.warn("$ref is not supported in allOf properties");

                        continue; // Skip reference objects
                    }

                    const property = schemaObjectToUniformDefinitionProperty(
                        propName,
                        propSchema,
                        schema.required?.includes(propName),
                        propSchema?.type === "array" ? true : false,
                        visitedRefs
                    );

                    if (property) {
                        // Check if property already exists and merge if needed
                        const existingPropertyIndex = properties.findIndex(p => p.name === propName);
                        if (existingPropertyIndex >= 0) {
                            // Merge properties if they exist in multiple schemas
                            const existingProperty = properties[existingPropertyIndex];
                            properties[existingPropertyIndex] = {
                                ...existingProperty,
                                ...property,
                                description: property.description || existingProperty.description || "",
                                meta: [...(existingProperty.meta || []), ...(property.meta || [])],
                            };
                        } else {
                            properties.push(property);
                        }
                    }
                }
            }
        }

        const oasSchema = schemaObject as OasJSONSchema;
        oasSchema.__internal_getRefPath = () => componentPaths
    } else if ('properties' in schemaObject && schemaObject.properties) {
        for (const [propName, propSchema] of Object.entries(schemaObject.properties)) {
            if (BUILT_IN_PROPERTIES[propName]) {
                continue;
            }
            if (typeof propSchema === "object" && '$ref' in propSchema) {
                console.warn("$ref is not supported in properties");

                continue; // Skip reference objects
            }

            const property = schemaObjectToUniformDefinitionProperty(
                propName,
                propSchema,
                schemaObject.required?.includes(propName),
                propSchema?.type === "array" ? true : false,
                visitedRefs
            );
            if (property) {
                properties.push(property);
            }
        }
    }

    return properties
}

export function schemaObjectToUniformDefinitionProperty(
    name: string,
    schema: OpenAPIV3.SchemaObject,
    required?: boolean,
    arrayOf?: boolean,
    visitedRefs?: Map<string, DefinitionProperty>,
    parentProperty?: DefinitionProperty
): DefinitionProperty | null {
    if (name === "__UNSAFE_refPath") {
        return null
    }
    if (!schema) return null;

    if (!visitedRefs) {
        visitedRefs = new Map();
    }
    let refPath = ""
    if ("__UNSAFE_refPath" in schema && typeof schema.__UNSAFE_refPath === 'function') {
        refPath = schema.__UNSAFE_refPath();
        const defProp = visitedRefs.get(refPath);
        if (defProp) {
            const v = JSON.parse(JSON.stringify(defProp))
            v.name = name;
            return v; // Return a deep copy circular json
        }
    }

    if (parentProperty) {
        visitedRefs.set(refPath, parentProperty);
    }

    // A property whose schema IS a component schema links to that schema's
    // page (`objects/<Name>`) — the same link the SDK-type pass emits for
    // nested types. Identity-based (component-links.ts), so a schema that
    // merely contains a `$ref` never links to its member.
    const link = componentLink(schema)

    // Handle anyOf case
    if ('anyOf' in schema && schema.anyOf) {
        const componentPaths: string[] = []
        const properties: DefinitionProperty[] = [];

        for (const variantSchema of schema.anyOf) {
            if ('$ref' in variantSchema) {
                console.warn("$ref is not supported in anyOf schemas");

                continue; // Skip reference objects
            }

            const oasSchema = variantSchema as OasJSONSchema;
            if (oasSchema.__internal_getRefPath) {
                const refPath = oasSchema.__internal_getRefPath();
                if (typeof refPath === 'string') {
                    componentPaths.push(refPath);
                } else if (Array.isArray(refPath)) {
                    componentPaths.push(...refPath);
                } else {
                    console.warn("Invalid refPath type in anyOf schema", oasSchema);
                }
            }

            const property = schemaObjectToUniformDefinitionProperty(name, variantSchema, required, false, visitedRefs);

            if (property) {
                if (isMergeType(property.type)) {
                    properties.push(...property.properties || []);
                } else {
                    properties.push({
                        ...property,
                        name: variantSchema.title || property.name || "",
                    });
                }
            }
        }

        const oasSchema = schema as OasJSONSchema;
        oasSchema.__internal_getRefPath = () => componentPaths

        const prop: DefinitionProperty = {
            name,
            type: DEFINED_DEFINITION_PROPERTY_TYPE.UNION,
            description: schema.description || "",
            properties
        };
        if (link) {
            prop.symbolDef = link
        }

        if (refPath) {
            visitedRefs.set(refPath, prop);
        }
        return prop
    }

    const meta = schemaObjectToUniformDefinitionPropertyMeta({
        ...schema,
        required: required ? [name] : undefined,
    }, name)

    // Handle oneOf case
    if ('oneOf' in schema && schema.oneOf) {
        const componentPaths: string[] = []
        const properties: DefinitionProperty[] = [];

        for (const variantSchema of schema.oneOf) {
            if ('$ref' in variantSchema) {
                console.warn("$ref is not supported in oneOf schemas");

                continue; // Skip reference objects
            }

            const oasSchema = variantSchema as OasJSONSchema;
            if (oasSchema.__internal_getRefPath) {
                const refPath = oasSchema.__internal_getRefPath();
                if (typeof refPath === 'string') {
                    componentPaths.push(refPath);
                } else if (Array.isArray(refPath)) {
                    componentPaths.push(...refPath);
                } else {
                    console.warn("Invalid refPath type in allOf schema", oasSchema);
                }
            }

            const property = schemaObjectToUniformDefinitionProperty(name, variantSchema, required, false, visitedRefs);
            if (property) {
                // if (isOfType(property.type)) { TODO: in the future?
                // }
                properties.push({
                    ...property,
                    name: variantSchema.title || property.name || "",
                });
            }
        }

        const oasSchema = schema as OasJSONSchema;
        oasSchema.__internal_getRefPath = () => componentPaths

        const prop: DefinitionProperty = {
            name,
            type: DEFINED_DEFINITION_PROPERTY_TYPE.XOR,
            description: schema.description || "",
            properties,
            meta,
        };
        if (link) {
            prop.symbolDef = link
        }

        if (refPath) {
            visitedRefs.set(refPath, prop);
        }

        return prop
    }

    // Handle allOf case
    if ('allOf' in schema && schema.allOf) {
        const componentPaths: string[] = []
        // Whether the wrapper declares a `type` of its own; when it does not, a
        // folded scalar member decides the row's type (see `foldMember`).
        const wrapperTyped = typeof schema.type === "string" && !!schema.type
        // The members that are component schemas — ONE such member links the
        // row to that schema's page (`id: allOf: [$ref WidgetId]`); two or more
        // are a composition with no single page to point at.
        const memberLinks: SymbolDef[] = []
        const mergedProperty: DefinitionProperty = {
            name,
            type: schema.type || "",
            description: schema.description || "",
            properties: [],
            meta
        };
        if (link) {
            mergedProperty.symbolDef = link
        }

        for (const variantSchema of schema.allOf) {
            if ('$ref' in variantSchema) {
                console.warn("$ref is not supported in allOf schemas");
                continue; // Skip reference objects
            }

            const memberLink = componentLink(variantSchema)
            if (memberLink) {
                memberLinks.push(memberLink)
            }

            const oasSchema = variantSchema as OasJSONSchema;
            if (oasSchema.__internal_getRefPath) {
                const refPath = oasSchema.__internal_getRefPath();
                if (typeof refPath === 'string') {
                    componentPaths.push(refPath);
                } else if (Array.isArray(refPath)) {
                    componentPaths.push(...refPath);
                } else {
                    console.warn("Invalid refPath type in allOf schema", oasSchema);
                }
            }


            if (!mergedProperty.type) {
                if (typeof variantSchema.type === 'string') {
                    mergedProperty.type = variantSchema.type || ""
                }
            }

            if ('properties' in variantSchema && variantSchema.properties) {
                for (const [propName, propSchema] of Object.entries(variantSchema.properties)) {
                    if (BUILT_IN_PROPERTIES[propName]) {
                        continue;
                    }
                    if ('$ref' in propSchema) {
                        console.warn("$ref is not supported in allOf properties");
                        continue; // Skip reference objects
                    }

                    const property = schemaObjectToUniformDefinitionProperty(
                        propName,
                        propSchema,
                        variantSchema.required?.includes(propName),
                        false,
                        visitedRefs
                    );

                    if (property && mergedProperty.properties) {
                        // Check if property already exists and merge if needed
                        const existingPropertyIndex = mergedProperty.properties.findIndex(p => p.name === propName);
                        if (existingPropertyIndex >= 0) {
                            // Merge properties if they exist in multiple schemas
                            const existingProperty = mergedProperty.properties[existingPropertyIndex];
                            mergedProperty.properties[existingPropertyIndex] = {
                                ...existingProperty,
                                ...property,
                                description: property.description || existingProperty.description || "",
                                meta: [...(existingProperty.meta || []), ...(property.meta || [])],
                            };
                        } else {
                            mergedProperty.properties.push(property);
                        }
                    }
                }
            } else {
                const property = schemaObjectToUniformDefinitionProperty(
                    "",
                    variantSchema,
                    false,
                    false,
                    visitedRefs
                );
                if (property) {
                    if (isOfType(property.type)) {
                        mergedProperty.ofProperty = property
                    } else if (mergedProperty.ofProperty) {
                        mergedProperty.description = property.description || mergedProperty.ofProperty.description || ""
                    } else {
                        foldMember(mergedProperty, property, wrapperTyped)
                    }
                }
            }
        }

        if (!mergedProperty.symbolDef && memberLinks.length === 1) {
            mergedProperty.symbolDef = memberLinks[0]
        }

        const oasSchema = schema as OasJSONSchema;
        oasSchema.__internal_getRefPath = () => componentPaths;

        if (refPath) {
            visitedRefs.set(refPath, mergedProperty);
        }
        return mergedProperty;
    }

    const property: DefinitionProperty = {
        name,
        type: schema.type || "object",
        description: schema.description || "",
        meta,
    };
    if (link) {
        property.symbolDef = link
    }

    // enum → $$enum with value-named child properties. A `const` (OpenAPI 3.1)
    // is a one-value enum: same children, same `enum-type`, plus the `const`
    // meta a single-value enum carries too.
    const enumValues: any[] | undefined = schema.enum
        ?? ("const" in schema ? [(schema as any).const] : undefined)

    if (enumValues) {
        const enumProperties = schemaObjectToUniformDefinitionProperties({
            properties: enumValues.reduce((acc, enumName) => ({
                ...acc,
                [enumName]: {
                    type: schema.type,
                }
            }), {})
        })
        if (!Array.isArray(enumProperties)) {
            return property;
        }

        meta.push({
            name: "enum-type",
            value: schema.type
        })
        // The constant's value: `{ name: "const", value: <literal> }` for a
        // single-value enum or a `const`, beside the `$$enum` children a
        // renderer already knows. Scalars stay what they are; an object or
        // array literal is JSON-stringified, like the `example` meta.
        if (enumValues.length === 1) {
            meta.push({
                name: "const",
                value: literalMetaValue(enumValues[0])
            })
        }

        const enumProperty: DefinitionProperty = {
            name,
            type: DEFINED_DEFINITION_PROPERTY_TYPE.ENUM,
            description: schema.description || "",
            meta,
            properties: enumProperties || [],
            ofProperty: {
                name: "",
                description: "",
                type: schema.type || "",
            }
        };
        if (link) {
            enumProperty.symbolDef = link
        }

        if (refPath) {
            visitedRefs.set(refPath, enumProperty);
        }

        return enumProperty
    } else {
        if ('properties' in schema && schema.properties) {
            property.properties = [];

            for (const [propName, propSchema] of Object.entries(schema.properties)) {
                if (BUILT_IN_PROPERTIES[propName]) {
                    continue;
                }
                if ('$ref' in propSchema) {
                    console.warn("$ref is not supported in properties");

                    continue; // Skip reference objects
                }

                const nestedProperty = schemaObjectToUniformDefinitionProperty(
                    propName,
                    propSchema,
                    schema.required?.includes(propName),
                    propSchema?.type === "array" ? true : false,
                    visitedRefs,
                );

                if (nestedProperty) {
                    property.properties.push(nestedProperty);
                }
            }
        }
        // Handle array items
        else if (schema.type === "array" && schema.items && !('$ref' in schema.items)) {
            const arrayProperty: DefinitionProperty = {
                name,
                type: DEFINED_DEFINITION_PROPERTY_TYPE.ARRAY,
                description: schema.description || "",
                meta,
                properties: []
            };
            if (link) {
                arrayProperty.symbolDef = link
            }

            const itemsProperty = schemaObjectToUniformDefinitionProperty("", schema.items, required, true, visitedRefs, arrayProperty);

            if (itemsProperty) {
                if (arrayOf || isOfType(itemsProperty.type) || itemsProperty.ofProperty?.type) {
                    arrayProperty.ofProperty = {
                        name: "",
                        type: itemsProperty.type,
                        properties: itemsProperty.properties || [],
                        description: itemsProperty.description || "",
                        meta: itemsProperty.meta || [],
                        ofProperty: itemsProperty.ofProperty || undefined,
                        symbolDef: itemsProperty.symbolDef || undefined,
                    }
                } else {
                    arrayProperty.properties = [itemsProperty];
                }
                // An array of a component links to that component — the rule
                // the request/response roots already follow (an array's link is
                // its items').
                if (!arrayProperty.symbolDef && itemsProperty.symbolDef) {
                    arrayProperty.symbolDef = itemsProperty.symbolDef
                }
            }
            if (refPath) {
                visitedRefs.set(refPath, arrayProperty);
            }

            return arrayProperty
        }
    }

    if (arrayOf) {
        const prop = {
            type: property.type,
            name: "",
            description: "",
            ofProperty: property
        }
        if (refPath) {
            visitedRefs.set(refPath, prop);
        }
    }


    if (refPath) {
        visitedRefs.set(refPath, property);
    }
    return property;
}

export function schemaObjectToUniformDefinitionPropertyMeta(objProp: OpenAPIV3.SchemaObject | OpenAPIV3.ParameterObject, name: string) {
    const meta: DefinitionPropertyMeta[] = []
    if (!objProp) {
        return meta
    }

    if (typeof objProp.required === "boolean" && objProp.required) {
        meta.push({
            name: "required",
            value: "true"
        })
    } else if (Array.isArray(objProp.required)) {
        for (const req of objProp.required) {
            if (req === name) {
                meta.push({
                    name: "required",
                    value: "true"
                })
            }
        }
    }
    if (objProp.deprecated) {
        meta.push({
            name: "deprecated",
            value: "true"
        })
    }
    if ("default" in objProp) {
        meta.push({
            name: "defaults",
            value: objProp.default
        })
    }
    if ("nullable" in objProp) {
        meta.push({
            name: "nullable",
            value: "true"
        })
    }
    if ("example" in objProp) {
        const example = typeof objProp.example === "object" ? JSON.stringify(objProp.example) : objProp.example;
        meta.push({
            name: "example",
            value: example
        })
    }
    if ("examples" in objProp) {
        meta.push({
            name: "examples",
            value: objProp.examples
        })
    }
    if ("maximum" in objProp) {
        meta.push({
            name: "maximum",
            value: objProp.maximum
        })
    }
    if ("minimum" in objProp) {
        meta.push({
            name: "minimum",
            value: objProp.minimum
        })
    }

    return meta
}

// `allOf: [<a scalar, enum or otherwise typed member>]` — the member is folded
// onto the merged row: its type (when the wrapper declares none), its
// description (when the wrapper has none), its meta (the wrapper's win by
// name), and an enum's value children + base type. Before this, such a member
// was pushed as a NAMELESS CHILD, so a described `id: allOf: [$ref Uuid]`
// rendered as a parent with one empty-named `string` row.
function foldMember(merged: DefinitionProperty, member: DefinitionProperty, wrapperTyped: boolean) {
    if (!wrapperTyped) {
        merged.type = member.type
    }
    if (!merged.description) {
        merged.description = member.description || ""
    }
    if (!merged.meta) {
        merged.meta = []
    }
    for (const m of member.meta || []) {
        if (!merged.meta.some(existing => existing.name === m.name)) {
            merged.meta.push(m)
        }
    }
    if (!merged.properties?.length && member.properties?.length) {
        merged.properties = member.properties
    }
    if (!merged.ofProperty && member.ofProperty) {
        merged.ofProperty = member.ofProperty
    }
}

// The `const` meta value: scalars as they are, object/array/null literals
// JSON-stringified (the `example` meta rule).
function literalMetaValue(v: unknown) {
    return typeof v === "object" ? JSON.stringify(v) : v
}

function isMergeType(type: string) {
    return type === DEFINED_DEFINITION_PROPERTY_TYPE.XOR
        || type === DEFINED_DEFINITION_PROPERTY_TYPE.UNION
}

function isOfType(type: string) {
    return type === DEFINED_DEFINITION_PROPERTY_TYPE.XOR
        || type === DEFINED_DEFINITION_PROPERTY_TYPE.UNION
        || type === DEFINED_DEFINITION_PROPERTY_TYPE.ARRAY
}