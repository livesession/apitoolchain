import path from "node:path";

import {describe, expect, it} from "vitest";

import uniform, {DefinitionProperty, Reference} from "@xyd-js/uniform";

// The JS impl directly — these pin the JS twin's behaviour whatever addon is
// installed (the fixture matrix covers the dispatching entry point).
import {deferencedOpenAPI, oapSchemaToReferences, uniformPluginXDocsSidebar} from "../src/impl-js";

function fixture(name: string) {
    return path.join(__dirname, "..", "__fixtures__", name, "input.yaml");
}

function row(rows: DefinitionProperty[] | undefined, name: string): DefinitionProperty {
    const found = rows?.find(p => p.name === name);
    if (!found) {
        throw new Error(`no row ${name} in ${JSON.stringify(rows?.map(p => p.name))}`);
    }
    return found;
}

function metaValue(p: DefinitionProperty, name: string) {
    return p.meta?.find(m => m.name === name)?.value;
}

function byCanonical(refs: Reference[], canonical: string): Reference {
    const found = refs.find(r => r.canonical === canonical);
    if (!found) {
        throw new Error(`no reference ${canonical}`);
    }
    return found;
}

describe("docs metadata (JS impl)", () => {
    it("a constant's value: single-value enums and `const` carry the `const` meta", async () => {
        const doc = await deferencedOpenAPI(fixture("9.schema-links"));
        const refs = oapSchemaToReferences(doc!);
        const rows = byCanonical(refs, "objects/Widget").definitions[0].properties;

        const object = row(rows, "object");
        expect(object.type).toBe("$$enum");
        expect(metaValue(object, "const")).toBe("widget");
        expect(metaValue(object, "enum-type")).toBe("string");
        expect(object.properties?.map(p => p.name)).toEqual(["widget"]);

        const kind = row(rows, "kind");
        expect(kind.type).toBe("$$enum");
        expect(metaValue(kind, "const")).toBe("standard");
        expect(kind.properties?.map(p => p.name)).toEqual(["standard"]);

        const status = row(rows, "status");
        expect(status.type).toBe("$$enum");
        expect(metaValue(status, "const")).toBeUndefined();
        expect(status.properties?.length).toBe(2);
    });

    it("allOf: [$ref <scalar>] + description is a scalar row, never a parent with a nameless child", async () => {
        const doc = await deferencedOpenAPI(fixture("9.schema-links"));
        const refs = oapSchemaToReferences(doc!);
        const rows = byCanonical(refs, "objects/Widget").definitions[0].properties;

        const id = row(rows, "id");
        expect(id.type).toBe("string");
        expect(id.description).toBe("Unique identifier for the widget.");
        expect(id.properties).toEqual([]);
        expect(metaValue(id, "required")).toBe("true");
        expect(metaValue(id, "example")).toBe("0d4f1e2a-6b2c-4f33-9d7e-1a2b3c4d5e6f");

        const color = row(rows, "color");
        expect(color.type).toBe("$$enum");
        expect(color.properties?.map(p => p.name)).toEqual(["red", "green", "blue"]);
        expect(color.ofProperty?.type).toBe("string");
        expect(metaValue(color, "enum-type")).toBe("string");
    });

    it("nested properties that are a component schema link to its page", async () => {
        const doc = await deferencedOpenAPI(fixture("9.schema-links"));
        const refs = oapSchemaToReferences(doc!);
        const widget = byCanonical(refs, "objects/Widget").definitions[0].properties;

        expect(row(widget, "definition").symbolDef).toEqual({
            id: "#/components/schemas/WidgetDefinition",
            canonical: "objects/WidgetDefinition",
        });
        // Through allOf: [$ref] as well.
        expect(row(widget, "id").symbolDef).toEqual({
            id: "#/components/schemas/WidgetId",
            canonical: "objects/WidgetId",
        });
        // An array of a component links to the component, on the row and its items.
        const parts = row(widget, "parts");
        expect(parts.symbolDef?.canonical).toBe("objects/Part");
        expect(parts.ofProperty?.symbolDef?.canonical).toBe("objects/Part");
        // Inline schemas do not link.
        expect(row(widget, "created_at").symbolDef).toBeUndefined();
        expect(row(widget, "status").symbolDef).toBeUndefined();

        // A request body's described allOf: [$ref] member and a parameter's $ref schema.
        const create = byCanonical(refs, "createWidget");
        const body = create.definitions.find(d => d.title === "Request body")!.variants![0].properties;
        expect(row(body, "definition").symbolDef?.canonical).toBe("objects/WidgetDefinition");
        const get = byCanonical(refs, "getWidget");
        const params = get.definitions.find(d => d.title === "Path parameters")!.properties;
        expect(row(params, "widgetId").symbolDef?.canonical).toBe("objects/WidgetId");
    });

    it("x-docs on component schemas: name, group and hidden outside sidebar mode", async () => {
        const doc = await deferencedOpenAPI(fixture("5.xdocs.objects"));
        const refs = uniform(oapSchemaToReferences(doc!), {plugins: [uniformPluginXDocsSidebar]}).references;

        expect(refs.map(r => r.title)).toEqual([
            "List widgets",
            "Create a widget",
            "Retrieve widget",
            "The widget object",
            "The part object",
            "PartBase",
            "Tag",
        ]);
        const widget = byCanonical(refs, "objects/Widget");
        expect(widget.title).toBe("The widget object");
        expect(widget.context?.group).toEqual(["widgets"]);
        const part = byCanonical(refs, "objects/Part");
        expect(part.context?.group).toEqual(["parts"]);
        expect(byCanonical(refs, "objects/Tag").context?.group).toEqual(["Objects"]);
        // hidden: the operation and both schemas are gone.
        expect(refs.some(r => r.canonical === "health")).toBe(false);
        expect(refs.some(r => r.canonical === "objects/WidgetList")).toBe(false);
        expect(refs.some(r => r.canonical === "objects/Error")).toBe(false);
    });

    it("an x-docs.sidebar that lists a hidden page still shows it", () => {
        const doc: any = {
            openapi: "3.0.3",
            info: {title: "Widgets", version: "1.0.0"},
            paths: {
                "/widgets": {
                    get: {operationId: "listWidgets", summary: "List widgets", responses: {"200": {description: "ok"}}},
                },
                "/internal/health": {
                    get: {operationId: "health", summary: "Health", "x-docs": {hidden: true}, responses: {"200": {description: "ok"}}},
                },
            },
            components: {
                schemas: {
                    Widget: {"x-docs": {name: "The widget object", hidden: true}, type: "object", properties: {id: {type: "string"}}},
                },
            },
            "x-docs": {
                sidebar: [{
                    group: "Reference",
                    pages: [
                        {type: "endpoint", key: "health"},
                        {type: "object", key: "Widget"},
                    ],
                }],
            },
        };
        const refs = uniform(oapSchemaToReferences(doc), {plugins: [uniformPluginXDocsSidebar]}).references;
        expect(refs.map(r => r.title)).toEqual(["Health", "The widget object"]);
        expect(refs.map(r => r.context?.group)).toEqual([["Reference"], ["Reference"]]);
    });
});
