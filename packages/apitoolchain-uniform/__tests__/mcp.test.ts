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
        expect(references.length).toBe(22);
        expect(tools.map((t) => t.name)).toEqual([
            "listPets",
            "createPet",
            "searchPets",
            "getPet",
            "updatePet",
            "deletePet",
            "replacePetTags",
            "store-inventory",
            "placeOrder",
            "getOrder",
            "replaceOrder",
            "cancelOrder",
            "get_health",
        ]);
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

    it("nests a body that cannot be flattened under one property", () => {
        const [collides, array] = uniformToMcpTools([
            operation({
                method: "put",
                definitions: [
                    {
                        title: "Path parameters",
                        type: "$rest.param.path",
                        properties: [{name: "id", type: "string", description: "", meta: [{name: "required", value: "true"}]}],
                    },
                    body([{name: "id", type: "string", description: ""}]),
                ],
            } as any),
            operation({
                method: "post",
                definitions: [body([], {rootProperty: {type: "$$array", ofProperty: {name: "", type: "string"}}})],
            } as any),
        ]);
        expect(collides.binding.bodyProperty).toBe("body");
        expect(collides.inputSchema.required).toEqual(["id", "body"]);
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
        expect(tool.inputSchema.properties.l3).toEqual({
            type: "object",
            properties: {l2: {type: "object"}},
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
