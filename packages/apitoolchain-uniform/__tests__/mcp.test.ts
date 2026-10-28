import * as fs from "node:fs";
import * as path from "node:path";

import {describe, expect, it} from "vitest";

import {stripAtMost, uniformToMcpTools} from "../src/mcp";
import {unwrapSoftBreaks} from "../src/operations";
import type {Reference} from "../src/types";
import {readFixture, testMcpTools} from "./utils";

// The oracles the Rust twin (crates/apitoolchain_uniform::mcp) is compared
// with byte for byte. Regen: UNIFORM_BUILD_FIXTURES=1 pnpm vitest run
const fixtureTests = [
    {name: "5.mcp-tools.default", description: "defaults: flat inputs, method-derived annotations"},
    {name: "5.mcp-tools.options", description: "limit cap, short descriptions, depth cap, read-only POST, names, extras"},
    {name: "5.mcp-tools.grouped", description: "flatten: false groups the inputs"},
    {name: "5.mcp-tools.open-body", description: "an open request body opens the input object"},
];

describe("uniformToMcpTools fixtures", () => {
    for (const t of fixtureTests) {
        it(t.description, () => {
            testMcpTools(t.name);
        });
    }
});

function operation(overrides: Partial<Reference> & {method?: string; path?: string}): Reference {
    const {method = "get", path: opPath = "/things", ...rest} = overrides;
    return {
        title: "Do a thing",
        canonical: "doThing",
        description: "Does a thing.",
        category: "rest",
        type: `rest_${method}`,
        context: {method, path: opPath, group: ["things"]},
        definitions: [],
        examples: {groups: []},
        ...rest,
    } as unknown as Reference;
}

function body(properties: any[], extra: any = {}) {
    return {
        title: "Request body",
        type: "$rest.request.body",
        properties: [],
        meta: [{name: "required", value: "true"}],
        variants: [
            {
                title: "application/json",
                properties,
                meta: [{name: "contentType", value: "application/json"}],
                ...extra,
            },
        ],
    };
}

describe("uniformToMcpTools", () => {
    it("emits one tool per REST operation and skips everything else", () => {
        const references: Reference[] = readFixture("5.mcp-tools.default/input.json");
        const tools = uniformToMcpTools(references);
        expect(references.length).toBe(25);
        // uploadPetPhoto takes a multipart body only: skipped by default
        expect(tools.map((t) => t.name)).toEqual([
            "listPets",
            "createPet",
            "searchPets",
            "getPet",
            "updatePet",
            "deletePet",
            "replacePetTags",
            "listOwnerPets",
            "store-inventory",
            "placeOrder",
            "shipOrder",
            "getOrder",
            "replaceOrder",
            "cancelOrder",
            "get_health",
        ]);
    });

    it("lets include opt a non-JSON-body operation in, without a body input", () => {
        const references: Reference[] = readFixture("5.mcp-tools.default/input.json");
        const [photo] = uniformToMcpTools(references, {include: (ref) => ref.canonical === "uploadPetPhoto"});
        expect(photo.binding).toMatchObject({body: null, pathParams: ["petId"]});
        expect(Object.keys(photo.inputSchema.properties)).toEqual(["petId"]);
    });

    it("derives annotations from the method", () => {
        const methods = ["get", "head", "options", "trace", "post", "put", "patch", "delete"];
        const tools = uniformToMcpTools(methods.map((method) => operation({method, canonical: method})));
        const hints = Object.fromEntries(
            tools.map((t) => [t.name, [t.annotations.readOnlyHint, t.annotations.destructiveHint, t.annotations.idempotentHint]])
        );
        expect(hints).toEqual({
            get: [true, false, true],
            head: [true, false, true],
            options: [true, false, true],
            trace: [true, false, true],
            post: [false, false, false],
            put: [false, true, true],
            patch: [false, true, false],
            delete: [false, true, true],
        });
    });

    it("renames an input that shares an earlier input's name and maps it back", () => {
        const references: Reference[] = readFixture("5.mcp-tools.default/input.json");
        const tools = uniformToMcpTools(references);
        const owner = tools.find((t) => t.name === "listOwnerPets")!;
        expect(Object.keys(owner.inputSchema.properties)).toEqual(["name", "name_query"]);
        expect(owner.binding).toMatchObject({
            pathParams: ["name"],
            queryParams: ["name_query"],
            inputs: {name_query: {in: "query", name: "name"}},
        });
        const order = tools.find((t) => t.name === "replaceOrder")!;
        expect(order.binding.bodyProperty).toBeUndefined();
        expect(order.binding.inputs).toEqual({id_body: {in: "body", name: "id"}});
        expect(order.inputSchema.required).toEqual(["id", "id_body", "pet_id", "status"]);
        // no rename, no map
        expect(tools.find((t) => t.name === "getOrder")!.binding.inputs).toBeUndefined();
    });

    it("keeps an optional body's properties optional", () => {
        const references: Reference[] = readFixture("5.mcp-tools.default/input.json");
        const ship = uniformToMcpTools(references).find((t) => t.name === "shipOrder")!;
        expect(Object.keys(ship.inputSchema.properties)).toEqual(["id", "carrier", "tracking"]);
        expect(ship.inputSchema.required).toEqual(["id"]);
    });

    it("drops the string null of a nullable enum for a real null", () => {
        const references: Reference[] = readFixture("5.mcp-tools.default/input.json");
        const create = uniformToMcpTools(references).find((t) => t.name === "createPet")!;
        expect(create.inputSchema.properties.size).toEqual({
            type: ["string", "null"],
            description: "The pet's size; null when unknown.",
            enum: ["small", "medium", "large", null],
        });
    });

    it("never ends the first sentence at an abbreviation", () => {
        const cases: [string, string][] = [
            ["Returns pets, e.g. the ones on sale. More.", "Returns pets, e.g. the ones on sale."],
            ["Uses carrier No. 5 by default. More.", "Uses carrier No. 5 by default."],
            ["Signed by J. Doe. More.", "Signed by J. Doe."],
            ["Compares a vs. b and stops. More.", "Compares a vs. b and stops."],
            ["Returns 3 items. 2 are new.", "Returns 3 items."],
            ["Returns the item.It has no space.", "Returns the item.It has no space."],
            ["Is it there? \"Yes\" it is.", "Is it there?"],
            ["Ends here.", "Ends here."],
        ];
        const tools = uniformToMcpTools(cases.map(([description]) => operation({description})), {descriptionChars: 0});
        expect(tools.map((t) => t.description)).toEqual(cases.map(([, sentence]) => `${sentence} — GET /things`));
    });

    it("slugs a canonical that is not a valid tool name and keeps names unique", () => {
        const tools = uniformToMcpTools([
            operation({canonical: "things/list"}),
            operation({canonical: "things list"}),
            operation({canonical: "", method: "delete", path: "/things/{id}"}),
            operation({canonical: "x".repeat(200)}),
            operation({canonical: "x".repeat(200)}),
        ]);
        expect(tools.map((t) => t.name)).toEqual([
            "things_list",
            "things_list_2",
            "DELETE_things_id",
            "x".repeat(128),
            "x".repeat(126) + "_2",
        ]);
    });

    it("cuts the description to its first sentence and the character budget", () => {
        const [short, cut, none] = uniformToMcpTools(
            [
                operation({description: "First sentence. Second one.\n\nMore."}),
                operation({description: "A long first sentence that goes on and on"}),
                operation({description: ""}),
            ],
            {descriptionChars: 12}
        );
        expect(short.description).toBe("First sente… — GET /things");
        expect(cut.description).toBe("A long firs… — GET /things");
        expect(none.description).toBe("GET /things");
    });

    it("caps limit: maximum, default and description", () => {
        const [tool] = uniformToMcpTools(
            [
                operation({
                    definitions: [
                        {
                            title: "Query parameters",
                            type: "$rest.param.query",
                            properties: [
                                {name: "limit", type: "integer", description: "Page size", meta: [{name: "defaults", value: 100}]},
                                {name: "small", type: "integer", description: "", meta: [{name: "maximum", value: 10}]},
                            ],
                        },
                    ],
                } as any),
            ],
            {limitCap: 25}
        );
        expect(tool.inputSchema.properties.limit).toEqual({
            type: "integer",
            description: "Page size. Capped at 25 by this server.",
            default: 25,
            maximum: 25,
        });
        // only `limit` is capped
        expect(tool.inputSchema.properties.small).toEqual({type: "integer", maximum: 10});
    });

    it("leaves a limit alone when its own maximum is within the cap", () => {
        const [tool] = uniformToMcpTools(
            [
                operation({
                    definitions: [
                        {
                            title: "Query parameters",
                            type: "$rest.param.query",
                            properties: [
                                {name: "limit", type: "integer", description: "Page size, at most 20.", meta: [{name: "maximum", value: 20}]},
                            ],
                        },
                    ],
                } as any),
            ],
            {limitCap: 25}
        );
        expect(tool.inputSchema.properties.limit).toEqual({
            type: "integer",
            description: "Page size, at most 20.",
            maximum: 20,
        });
    });

    it("replaces an at-most clause the cap overrides", () => {
        expect(stripAtMost("Page size, at most 200.")).toBe("Page size.");
        expect(stripAtMost("Page size (at most 200).")).toBe("Page size.");
        expect(stripAtMost("Page size; At Most 200")).toBe("Page size");
        expect(stripAtMost("At most 200.")).toBe("");
        // with what joins it on
        expect(stripAtMost("10 by default and at most 200")).toBe("10 by default");
        expect(stripAtMost("Defaults to 10, and at most 200.")).toBe("Defaults to 10.");
        expect(stripAtMost("Up to 10 or at most 20; more is rejected.")).toBe("Up to 10; more is rejected.");
        expect(stripAtMost("Brand at most 2.")).toBe("Brand.");
        // not a clause of its own: kept
        expect(stripAtMost("At most 200 items are returned.")).toBe("At most 200 items are returned.");
        expect(stripAtMost("Whatnot most 2.")).toBe("Whatnot most 2.");
    });

    it("joins hard-wrapped description lines before taking the first sentence", () => {
        const [tool] = uniformToMcpTools([
            operation({
                description: "Returns the things that match\nthe filters, newest first. More\nfollows.\n\nA second paragraph.",
                definitions: [
                    {
                        title: "Query parameters",
                        type: "$rest.param.query",
                        properties: [{name: "q", type: "string", description: "Text to\nmatch.\n\n- one\n- two"}],
                    },
                ],
            } as any),
        ]);
        expect(tool.description).toBe("Returns the things that match the filters, newest first. — GET /things");
        expect(tool.inputSchema.properties.q.description).toBe("Text to match.\n\n- one\n- two");
        expect(unwrapSoftBreaks("a\n  indented\n1. item\n# heading")).toBe("a\n  indented\n1. item\n# heading");
    });

    it("keeps fenced code, headings and rules line for line", () => {
        expect(unwrapSoftBreaks("Sample\n```json\n{\n  \"a\": 1\n}\n```\nAfter\nthat.")).toBe(
            "Sample\n```json\n{\n  \"a\": 1\n}\n```\nAfter that."
        );
        expect(unwrapSoftBreaks("Title\n=====\nText\nmore")).toBe("Title\n=====\nText more");
        expect(unwrapSoftBreaks("Title\n---\nText")).toBe("Title\n---\nText");
        expect(unwrapSoftBreaks("# Head\nnext\nline")).toBe("# Head\nnext line");
        expect(unwrapSoftBreaks("a\n***\nb")).toBe("a\n***\nb");
        // only an item numbered 1 interrupts a paragraph; any number continues an open list
        expect(unwrapSoftBreaks("Up to 10 and at most\n5000. Larger is rejected.")).toBe(
            "Up to 10 and at most 5000. Larger is rejected."
        );
        expect(unwrapSoftBreaks("Steps:\n1. one\n2. two\nlazy\n3. three")).toBe("Steps:\n1. one\n2. two lazy\n3. three");
        expect(unwrapSoftBreaks("5. five\n6. six")).toBe("5. five\n6. six");
        expect(unwrapSoftBreaks("Text\n\n7. seven\n8. eight")).toBe("Text\n\n7. seven\n8. eight");
        expect(unwrapSoftBreaks("1. one\n\nText\n2. two")).toBe("1. one\n\nText 2. two");
        // a longer fence is closed only by one at least as long
        expect(unwrapSoftBreaks("~~~~\nx\n~~~\ny\n~~~~\nz\nw")).toBe("~~~~\nx\n~~~\ny\n~~~~\nz w");
        const references: Reference[] = readFixture("5.mcp-tools.default/input.json");
        const create = uniformToMcpTools(references).find((t) => t.name === "createPet")!;
        expect(create.inputSchema.properties.attributes.description).toBe(
            'Free-form attributes\n====================\nStored as given, for example:\n```json\n{\n  "color": "brown",\n  "legs": 4\n}\n```\nKeys are case-sensitive.'
        );
    });

    it("nests a body that cannot be flattened under one property", () => {
        const [named, array] = uniformToMcpTools([
            operation({
                method: "put",
                definitions: [
                    {
                        title: "Path parameters",
                        type: "$rest.param.path",
                        properties: [{name: "body", type: "string", description: "", meta: [{name: "required", value: "true"}]}],
                    },
                    body([], {rootProperty: {type: "$$array", ofProperty: {name: "", type: "string"}}}),
                ],
            } as any),
            operation({
                method: "post",
                definitions: [body([], {rootProperty: {type: "$$array", ofProperty: {name: "", type: "string"}}})],
            } as any),
        ]);
        // a parameter took `body`
        expect(named.binding.bodyProperty).toBe("requestBody");
        expect(named.inputSchema.required).toEqual(["body", "requestBody"]);
        expect(array.inputSchema.properties.body).toEqual({type: "array", items: {type: "string"}});
        expect(array.binding).toMatchObject({body: "json", bodyProperty: "body"});
    });

    it("spells objects out down to maxDepth only", () => {
        const deep = (level: number): any =>
            level === 0
                ? {name: "leaf", type: "string", description: ""}
                : {name: `l${level}`, type: "object", description: "", properties: [deep(level - 1)]};
        const [tool] = uniformToMcpTools(
            [operation({method: "post", definitions: [body([deep(3)])]} as any)],
            {maxDepth: 2}
        );
        // l3 at depth 1, l2 at depth 2 — both spelled out; l1 at depth 3 is not
        expect(tool.inputSchema.properties.l3).toEqual({
            type: "object",
            properties: {l2: {type: "object", properties: {l1: {type: "object"}}}},
        });
    });

    it("counts an anyOf property as required when its alternatives are", () => {
        const [tool] = uniformToMcpTools([
            operation({
                method: "post",
                definitions: [
                    body([
                        {
                            name: "note",
                            type: "$$union",
                            description: "",
                            properties: [
                                {name: "note", type: "string", description: "", meta: [{name: "required", value: "true"}]},
                                {name: "note", type: "null", description: "", meta: [{name: "required", value: "true"}]},
                            ],
                        },
                    ]),
                ],
            } as any),
        ]);
        expect(tool.inputSchema.required).toEqual(["note"]);
        expect(tool.inputSchema.properties.note).toEqual({anyOf: [{type: "string"}, {type: "null"}]});
    });

    it("stays isomorphic: no native addon, no Node builtins", () => {
        for (const file of ["../src/mcp.ts", "../src/operations.ts", "../src/skill/index.ts", "../src/skill/markdown.ts"]) {
            const source = fs.readFileSync(path.join(__dirname, file), "utf8");
            const imports = [...source.matchAll(/from\s+["']([^"']+)["']/g)].map((m) => m[1]);
            for (const specifier of imports) {
                expect(["./types", "./operations", "../types", "../operations", "./markdown"], `${file}: ${specifier}`).toContain(
                    specifier
                );
            }
        }
    });
});
