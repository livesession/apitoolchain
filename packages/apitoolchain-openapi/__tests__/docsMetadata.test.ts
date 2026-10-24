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

        // A typed wrapper (`type: string`) around the enum is the same enum row.
        const shade = row(rows, "shade");
        expect(shade.type).toBe("$$enum");
        expect(shade.description).toBe("The shade of the widget.");
        expect(shade.properties?.map(p => p.name)).toEqual(["red", "green", "blue"]);
        expect(shade.ofProperty?.type).toBe("string");
        expect(shade.symbolDef?.canonical).toBe("objects/Color");
    });

    it("allOf: a composed member keeps its rows whatever its position among the members", async () => {
        const doc = await deferencedOpenAPI(fixture("9.schema-links"));
        const refs = oapSchemaToReferences(doc!);
        const rows = byCanonical(refs, "objects/Widget").definitions[0].properties;

        // `allOf: [{type: object, properties: {label}}, $ref Assembly]` where
        // Assembly is itself `allOf: [$ref AssemblyBase, {properties: {sku}}]`.
        const assembly = row(rows, "assembly");
        expect(assembly.type).toBe("object");
        expect(assembly.description).toBe("How the widget is assembled.");
        expect(assembly.properties?.map(p => p.name)).toEqual(["label", "id", "sku"]);
        expect(assembly.symbolDef?.canonical).toBe("objects/Assembly");

        // The other order, on an inline document (an inline `allOf` member has
        // no `properties` key of its own either, so it takes the same fold
        // path as a `$ref` to a composition): the composed member's rows
        // first, the wrapper's own after them.
        const composed = () => ({
            allOf: [
                {type: "object", properties: {id: {type: "string"}}},
                {type: "object", properties: {sku: {type: "string"}}},
            ],
        });
        const inline: any = {
            openapi: "3.0.3",
            info: {title: "Widgets", version: "1.0.0"},
            paths: {},
            components: {
                schemas: {
                    Widget: {
                        type: "object",
                        properties: {
                            composed_first: {allOf: [composed(), {type: "object", properties: {name: {type: "string"}}}]},
                            overlap: {allOf: [{type: "object", properties: {id: {type: "string", description: "the wrapper's id"}}}, composed()]},
                        },
                    },
                },
            },
        };
        const inlineRows = byCanonical(oapSchemaToReferences(inline), "objects/Widget").definitions[0].properties;
        expect(row(inlineRows, "composed_first").properties?.map(p => p.name)).toEqual(["id", "sku", "name"]);
        // A row both sides declare is merged by name, the description kept.
        const overlap = row(inlineRows, "overlap");
        expect(overlap.properties?.map(p => p.name)).toEqual(["id", "sku"]);
        expect(row(overlap.properties, "id").description).toBe("the wrapper's id");
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
        ]);
        const widget = byCanonical(refs, "objects/Widget");
        expect(widget.title).toBe("The widget object");
        expect(widget.context?.group).toEqual(["widgets"]);
        const part = byCanonical(refs, "objects/Part");
        expect(part.context?.group).toEqual(["parts"]);
        expect(byCanonical(refs, "objects/PartBase").context?.group).toEqual(["Objects"]);
        // hidden: the operation and the three schemas are gone.
        expect(refs.some(r => r.canonical === "health")).toBe(false);
        expect(refs.some(r => r.canonical === "objects/WidgetList")).toBe(false);
        expect(refs.some(r => r.canonical === "objects/Error")).toBe(false);
        expect(refs.some(r => r.canonical === "objects/Tag")).toBe(false);

        // A link to a hidden schema keeps its id and loses its canonical — on
        // the object page and on every endpoint that embeds the schema.
        const tags = row(widget.definitions[0].properties, "tags");
        expect(tags.symbolDef).toEqual({id: "#/components/schemas/Tag"});
        expect(tags.ofProperty?.symbolDef).toEqual({id: "#/components/schemas/Tag"});
        const create = byCanonical(refs, "createWidget");
        const body = create.definitions.find(d => d.title === "Request body")!.variants![0].properties;
        expect(row(body, "tags").symbolDef).toEqual({id: "#/components/schemas/Tag"});
        // A link to a page that exists stays whole: listWidgets → WidgetList
        // (hidden, inlined) → data: array of Widget.
        const list = byCanonical(refs, "listWidgets");
        const links: string[] = [];
        const walk = (p: DefinitionProperty | undefined) => {
            if (!p) return;
            if (typeof p.symbolDef?.canonical === "string") links.push(p.symbolDef.canonical);
            p.properties?.forEach(walk);
            walk(p.ofProperty);
        };
        for (const d of list.definitions) {
            d.properties?.forEach(walk);
            d.variants?.forEach(v => v.properties?.forEach(walk));
        }
        expect(links).toContain("objects/Widget");
        expect(links).not.toContain("objects/Tag");
    });

    it("an x-docs.sidebar: links follow the listed page, a listed composition reads its own x-docs", () => {
        // The shape a dereferenced document has: ONE object per component,
        // shared between `components.schemas` and every place that used it.
        const Tag: any = {type: "object", properties: {name: {type: "string"}}};
        const Widget: any = {
            "x-docs": {name: "The widget object"},
            type: "object",
            properties: {id: {type: "string"}, tags: {type: "array", items: Tag}},
        };
        const WidgetList: any = {type: "object", properties: {data: {type: "array", items: Widget}}};
        const PartBase: any = {type: "object", properties: {id: {type: "string"}}};
        const doc: any = {
            openapi: "3.0.3",
            info: {title: "Widgets", version: "1.0.0"},
            paths: {
                "/widgets": {
                    get: {
                        operationId: "listWidgets", summary: "List widgets",
                        responses: {"200": {description: "ok", content: {"application/json": {schema: WidgetList}}}},
                    },
                },
            },
            components: {
                schemas: {
                    Widget,
                    WidgetList,
                    Tag,
                    PartBase,
                    Bundle: {
                        "x-docs": {name: "The bundle object"},
                        allOf: [PartBase, {type: "object", properties: {count: {type: "integer"}}}],
                    },
                    Kit: {allOf: [PartBase]},
                },
            },
            "x-docs": {
                sidebar: [{
                    group: "Reference",
                    pages: [
                        {type: "endpoint", key: "listWidgets"},
                        {type: "object", key: "Widget", path: "the-widget"},
                        {type: "object", key: "Bundle"},
                        {type: "object", key: "Kit"},
                    ],
                }],
            },
        };
        const refs = uniform(oapSchemaToReferences(doc), {plugins: [uniformPluginXDocsSidebar]}).references;
        // The composition's own x-docs is read (not only a member's); one
        // without any x-docs keeps its page and its default title.
        expect(refs.map(r => r.title)).toEqual(["List widgets", "The widget object", "The bundle object", "Kit"]);

        const widget = refs[1];
        expect(widget.canonical).not.toBe("objects/Widget");
        // listWidgets → data: array of Widget follows the moved page, on the
        // row and on its items.
        const data = row(refs[0].definitions[0].variants![0].properties, "data");
        expect(data.symbolDef?.canonical).toBe(widget.canonical);
        expect(data.ofProperty?.symbolDef?.canonical).toBe(widget.canonical);
        // Widget.tags → Tag is not listed: no page, so no canonical.
        const tags = row(widget.definitions[0].properties, "tags");
        expect(tags.symbolDef).toEqual({id: "#/components/schemas/Tag"});
        expect(tags.ofProperty?.symbolDef).toEqual({id: "#/components/schemas/Tag"});
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
