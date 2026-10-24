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
            label: "WidgetDefinition",
        });
        // Through allOf: [$ref] as well.
        expect(row(widget, "id").symbolDef).toEqual({
            id: "#/components/schemas/WidgetId",
            canonical: "objects/WidgetId",
            label: "WidgetId",
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

        // The default objects layout (`grouped`): every object page in ONE
        // top-level "Objects" group after the operations — the page without a
        // resource group first, then one sub-group per resource group in the
        // order the groups appear above (`widgets` has operations, `parts`
        // has none, so it follows).
        expect(refs.map(r => r.title)).toEqual([
            "List widgets",
            "Create a widget",
            "Retrieve widget",
            "PartBase",
            "The widget object",
            "The part object",
        ]);
        const widget = byCanonical(refs, "objects/Widget");
        expect(widget.title).toBe("The widget object");
        expect(widget.context?.group).toEqual(["Objects", "widgets"]);
        const part = byCanonical(refs, "objects/Part");
        expect(part.context?.group).toEqual(["Objects", "parts"]);
        expect(byCanonical(refs, "objects/PartBase").context?.group).toEqual(["Objects"]);
        // An operation's own `x-docs.group` is never nested.
        expect(byCanonical(refs, "listWidgets").context?.group).toEqual(["widgets"]);
        // hidden: the operation and the three schemas are gone.
        expect(refs.some(r => r.canonical === "health")).toBe(false);
        expect(refs.some(r => r.canonical === "objects/WidgetList")).toBe(false);
        expect(refs.some(r => r.canonical === "objects/Error")).toBe(false);
        expect(refs.some(r => r.canonical === "objects/Tag")).toBe(false);

        // A link to a hidden schema keeps its id and loses its canonical — on
        // the object page and on every endpoint that embeds the schema.
        const tags = row(widget.definitions[0].properties, "tags");
        expect(tags.symbolDef).toEqual({id: "#/components/schemas/Tag", label: "Tag"});
        expect(tags.ofProperty?.symbolDef).toEqual({id: "#/components/schemas/Tag", label: "Tag"});
        const create = byCanonical(refs, "createWidget");
        const body = create.definitions.find(d => d.title === "Request body")!.variants![0].properties;
        expect(row(body, "tags").symbolDef).toEqual({id: "#/components/schemas/Tag", label: "Tag"});
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
        // The response ROOT links to the hidden envelope: the variant's own
        // symbolDef loses its canonical too, and keeps its id and label (the
        // envelope's constant `object` value).
        const root = list.definitions.find(d => d.title === "Response")!.variants![0].symbolDef;
        expect(root).toEqual({id: "#/components/schemas/WidgetList", label: "list"});
    });

    it("the objectsGroup option names the top-level group and the sub-group", async () => {
        const doc = await deferencedOpenAPI(fixture("5.xdocs.objects"));
        const refs = uniform(oapSchemaToReferences(doc!, {objectsGroup: "Types"}), {plugins: [uniformPluginXDocsSidebar]}).references;
        expect(byCanonical(refs, "objects/Widget").context?.group).toEqual(["Types", "widgets"]);
        expect(byCanonical(refs, "objects/Part").context?.group).toEqual(["Types", "parts"]);
        expect(byCanonical(refs, "objects/PartBase").context?.group).toEqual(["Types"]);
        expect(byCanonical(refs, "listWidgets").context?.group).toEqual(["widgets"]);
        // Under the per-area layout the renamed group is the sub-group.
        const perArea = uniform(oapSchemaToReferences(doc!, {objectsGroup: "Types", objectsLayout: "per-area"}), {plugins: [uniformPluginXDocsSidebar]}).references;
        expect(byCanonical(perArea, "objects/Widget").context?.group).toEqual(["widgets", "Types"]);
        expect(byCanonical(perArea, "objects/PartBase").context?.group).toEqual(["Types"]);
        // An empty name means the default.
        const plain = uniform(oapSchemaToReferences(doc!, {objectsGroup: ""}), {plugins: [uniformPluginXDocsSidebar]}).references;
        expect(byCanonical(plain, "objects/Widget").context?.group).toEqual(["Objects", "widgets"]);
    });

    it("objectsLayout: per-area nests an object page under its resource group's Objects sub-group", async () => {
        const doc = await deferencedOpenAPI(fixture("5.xdocs.objects"));
        const refs = uniform(oapSchemaToReferences(doc!, {objectsLayout: "per-area"}), {plugins: [uniformPluginXDocsSidebar]}).references;
        // The object pages keep the schema order; the navigation lists each
        // sub-group after its group's operations.
        expect(refs.map(r => r.title)).toEqual([
            "List widgets",
            "Create a widget",
            "Retrieve widget",
            "The widget object",
            "The part object",
            "PartBase",
        ]);
        expect(byCanonical(refs, "objects/Widget").context?.group).toEqual(["widgets", "Objects"]);
        expect(byCanonical(refs, "objects/Part").context?.group).toEqual(["parts", "Objects"]);
        expect(byCanonical(refs, "objects/PartBase").context?.group).toEqual(["Objects"]);
        expect(byCanonical(refs, "listWidgets").context?.group).toEqual(["widgets"]);
        // An unknown layout name means the default.
        const fallback = uniform(oapSchemaToReferences(doc!, {objectsLayout: "sideways" as any}), {plugins: [uniformPluginXDocsSidebar]}).references;
        expect(byCanonical(fallback, "objects/Widget").context?.group).toEqual(["Objects", "widgets"]);
    });

    it("grouped layout: the Objects group is last, its pages ordered by the groups above", async () => {
        const doc = await deferencedOpenAPI(fixture("11.objects-layouts"));
        const refs = uniform(oapSchemaToReferences(doc!), {plugins: [uniformPluginXDocsSidebar]}).references;
        // Operations in tag order (declared `parts`, `widgets`; the undeclared
        // `assemblies`; the untagged `getStatus`), then EVERY object page: the
        // two without a resource group first (schema order), then the groups
        // in the order they appear above — `parts`, `widgets`, `assemblies` —
        // then `fasteners`, which has no operations.
        expect(refs.map(r => r.canonical)).toEqual([
            "listParts",
            "listWidgets",
            "listAudit",
            "createAssembly",
            "getStatus",
            "objects/Status",
            "objects/Address",
            "objects/Part",
            "objects/Widget",
            "objects/Assembly",
            "objects/Fastener",
        ]);
        expect(refs.map(r => r.context?.group)).toEqual([
            ["parts"],
            ["widgets"],
            ["audit"],
            ["assemblies"],
            [""],
            ["Objects"],
            ["Objects"],
            ["Objects", "parts"],
            ["Objects", "widgets"],
            ["Objects", "assemblies"],
            ["Objects", "fasteners"],
        ]);
        // The hidden schemas have no page.
        expect(refs.some(r => r.canonical === "objects/WidgetList")).toBe(false);
        expect(refs.some(r => r.canonical === "objects/Error")).toBe(false);
    });

    it("object pages sort after every operation, even one tagged with the objects group's name", () => {
        const doc: any = {
            openapi: "3.0.3",
            info: {title: "Widgets", version: "1.0.0"},
            tags: [{name: "Objects"}, {name: "widgets"}],
            paths: {
                "/objects": {get: {operationId: "listObjects", summary: "List objects", tags: ["Objects"], responses: {"200": {description: "ok"}}}},
                "/widgets": {get: {operationId: "listWidgets", summary: "List widgets", tags: ["widgets"], responses: {"200": {description: "ok"}}}},
            },
            components: {schemas: {Widget: {type: "object", properties: {id: {type: "string"}}}}},
        };
        const refs = oapSchemaToReferences(doc);
        expect(refs.map(r => r.canonical)).toEqual(["listObjects", "listWidgets", "objects/Widget"]);
    });

    it("schema links carry the schema's label: its constant `object` value, else its name", async () => {
        const doc = await deferencedOpenAPI(fixture("10.schema-labels"));
        const refs = oapSchemaToReferences(doc!);

        // The component pages' own links.
        const widget = byCanonical(refs, "objects/Widget");
        expect(widget.definitions[0].symbolDef).toEqual({
            id: "#/components/schemas/Widget",
            canonical: "objects/Widget",
            label: "widget",
        });
        expect(byCanonical(refs, "objects/Material").definitions[0].symbolDef?.label).toBe("Material");
        expect(byCanonical(refs, "objects/Part").definitions[0].symbolDef?.label).toBe("part");
        expect(byCanonical(refs, "objects/Assembly").definitions[0].symbolDef?.label).toBe("assembly");

        // Nested rows: a schema without an `object` property, a multi-value
        // `object` enum (not a constant), a `const`, a tag on an allOf member.
        const rows = widget.definitions[0].properties;
        expect(row(rows, "material").symbolDef?.label).toBe("Material");
        expect(row(rows, "shape").symbolDef?.label).toBe("Shape");
        expect(row(rows, "assembly").symbolDef?.label).toBe("assembly");
        const parts = row(rows, "parts");
        expect(parts.symbolDef?.label).toBe("part");
        expect(parts.ofProperty?.symbolDef?.label).toBe("part");
        expect(row(rows, "id").symbolDef).toBeUndefined();

        // An array of widgets: the row and its items.
        const list = byCanonical(refs, "objects/WidgetList");
        expect(list.definitions[0].symbolDef?.label).toBe("list");
        const data = row(list.definitions[0].properties, "data");
        expect(data.type).toBe("$$array");
        expect(data.symbolDef?.label).toBe("widget");
        expect(data.ofProperty?.symbolDef?.label).toBe("widget");

        // A nullable widget (allOf + nullable) and a union with a null branch.
        const slot = byCanonical(refs, "objects/Slot").definitions[0].properties;
        const nullable = row(slot, "widget");
        expect(nullable.type).toBe("object");
        expect(metaValue(nullable, "nullable")).toBe("true");
        expect(nullable.symbolDef?.label).toBe("widget");
        const spare = row(slot, "spare");
        expect(spare.type).toBe("$$union");
        expect(spare.symbolDef).toBeUndefined();
        expect(spare.properties?.[0].symbolDef?.label).toBe("widget");
        expect(spare.properties?.[1].symbolDef).toBeUndefined();

        // The request/response roots: a `$ref` root, an array root (its
        // items'), a request body; an inline root has no link.
        const response = (canonical: string, index = 0) =>
            byCanonical(refs, canonical).definitions.find(d => d.title === "Response")!.variants![index].symbolDef;
        expect(response("getWidget")).toEqual({
            id: "#/components/schemas/Widget",
            canonical: "objects/Widget",
            label: "widget",
        });
        expect(response("listWidgets")?.label).toBe("list");
        expect(response("listMaterials")?.label).toBe("Material");
        expect(response("getSlot", 1)).toBeUndefined();
        const body = byCanonical(refs, "createWidget").definitions.find(d => d.title === "Request body")!.variants![0];
        expect(body.symbolDef?.label).toBe("widget");
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
        expect(tags.symbolDef).toEqual({id: "#/components/schemas/Tag", label: "Tag"});
        expect(tags.ofProperty?.symbolDef).toEqual({id: "#/components/schemas/Tag", label: "Tag"});
    });

    it("regions mode (one page at a time): the links follow the document, not the one-page list", async () => {
        // A generated page converts ONE region, then runs the plugin: the
        // sidebar pass is not its business, but its links still have to land
        // on pages that exist — hidden schemas have none.
        const doc = await deferencedOpenAPI(fixture("5.xdocs.objects"));
        const page = (region: string) =>
            uniform(oapSchemaToReferences(doc!, {regions: [region]}), {plugins: [uniformPluginXDocsSidebar]}).references;

        const widget = page("/components/schemas/Widget");
        expect(widget.map(r => r.canonical)).toEqual(["objects/Widget"]);
        const tags = row(widget[0].definitions[0].properties, "tags");
        expect(tags.symbolDef).toEqual({id: "#/components/schemas/Tag", label: "Tag"});
        expect(tags.ofProperty?.symbolDef).toEqual({id: "#/components/schemas/Tag", label: "Tag"});

        const list = page("GET /widgets");
        expect(list.map(r => r.canonical)).toEqual(["listWidgets"]);
        const data = row(list[0].definitions[0].variants![0].properties, "data");
        expect(data.symbolDef?.canonical).toBe("objects/Widget");
        expect(data.ofProperty?.symbolDef?.canonical).toBe("objects/Widget");

        const create = page("POST /widgets");
        const body = create[0].definitions.find(d => d.title === "Request body")!.variants![0].properties;
        expect(row(body, "tags").symbolDef).toEqual({id: "#/components/schemas/Tag", label: "Tag"});
    });

    it("regions mode under an x-docs.sidebar: the links follow the listed page", () => {
        const Tag: any = {type: "object", properties: {name: {type: "string"}}};
        const Widget: any = {
            "x-docs": {name: "The widget object"},
            type: "object",
            properties: {id: {type: "string"}, tags: {type: "array", items: Tag}},
        };
        const WidgetList: any = {type: "object", properties: {data: {type: "array", items: Widget}}};
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
            components: {schemas: {Widget, WidgetList, Tag}},
            "x-docs": {
                sidebar: [{
                    group: "Reference",
                    pages: [
                        {type: "endpoint", key: "listWidgets"},
                        {type: "object", key: "Widget", path: "the-widget"},
                    ],
                }],
            },
        };
        // The whole-document pass says where the page went.
        const whole = uniform(oapSchemaToReferences(doc), {plugins: [uniformPluginXDocsSidebar]}).references;
        const widgetCanonical = whole[1].canonical;
        expect(widgetCanonical).not.toBe("objects/Widget");

        const page = (region: string) =>
            uniform(oapSchemaToReferences(doc, {regions: [region]}), {plugins: [uniformPluginXDocsSidebar]}).references;
        const data = row(page("GET /widgets")[0].definitions[0].variants![0].properties, "data");
        expect(data.symbolDef?.canonical).toBe(widgetCanonical);
        expect(data.ofProperty?.symbolDef?.canonical).toBe(widgetCanonical);
        // Tag is not listed: no page, so no canonical.
        const tags = row(page("/components/schemas/Widget")[0].definitions[0].properties, "tags");
        expect(tags.symbolDef).toEqual({id: "#/components/schemas/Tag", label: "Tag"});
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
