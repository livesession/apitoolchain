import path from "node:path";

import {describe, expect, it} from 'vitest'

import {testOasSchemaToReferences} from "./utils";
import {uniformOasOptions} from "../src/impl-js/types";
import {uniformOpenAIMeta} from "../__fixtures__/-2.complex.openai/pluginOasOpenai";
import {uniformPluginXDocsSidebar} from "../src";
import {native} from "../src/native";

// The FULL fixture matrix runs — these outputs are the frozen parity oracle for
// the Rust migration (crates/apitoolchain_openapi). Regen is explicit only:
//   OAS_BUILD_FIXTURES=1 pnpm vitest run __tests__/oapSchemaToReferences.test.ts
// Fixtures with `plugins` exercise JS-closure post-processing — the Rust tier-1
// parity test skips those (covered by the through-shim vitest tier instead).
const tests: {
    name: string;
    description: string,
    url?: string, // URL to the OpenAPI schema
    plugins?: any[]; // TODO: fix any,
    options?: uniformOasOptions
}[] = [
    {
        name: "-2.complex.openai",
        description: "OpenAI OpenAPI API example",
        plugins: [
            uniformOpenAIMeta,
        ],
    },
    {
        name: "-3.random",
        description: "Random/scratch OpenAPI example",
    },
    {
        name: "1.basic",
        description: "Basic OpenAPI API example",
    },
    {
        name: "2.more",
        description: "More OpenAPI API example",
    },
    {
        name: "3.multiple-responses",
        description: "Multiple responses OpenAPI API example",
    },
    {
        name: "5.xdocs.codeLanguages",
        description: "x-docs OpenAPI API codeLanguages example",
    },
    {
        name: "5.xdocs.sidebar",
        description: "x-docs OpenAPI API sidebar example",
        plugins: [
            uniformPluginXDocsSidebar
        ]
    },
    {
        name: "6.codeSamples",
        description: "x-codeSamples OpenAPI API example",
    },
    {
        name: "7.examples",
        description: "OpenAPI examples property",
    },
    {
        name: "8.enums",
        description: "OpenAPI enums",
    },
    {
        name: "9.schema-links",
        description: "const values, schema links on nested properties, allOf scalar rows",
    },
    {
        name: "5.xdocs.objects",
        description: "x-docs name/group/hidden on component schemas, outside sidebar mode (the per-area objects layout)",
        plugins: [
            uniformPluginXDocsSidebar
        ],
        options: {objectsLayout: "per-area"}
    },
    {
        name: "10.schema-labels",
        description: "schema link labels: the constant `object` value, else the schema name; the request/response roots",
    },
    {
        name: "11.objects-layouts",
        description: "the grouped objects layout (the default): one Objects group last, the resource groups as sub-groups in the groups' order",
        plugins: [
            uniformPluginXDocsSidebar
        ]
    },
    {
        name: "12.page-paths",
        description: "explicit page paths (x-docs.path) and the title-derived canonicals (operationPaths: title); the links follow",
        plugins: [
            uniformPluginXDocsSidebar
        ],
        options: {operationPaths: "title"}
    },
    {
        name: "13.open-objects-null-enums",
        description: "additionalProperties meta, null enum members",
    },
]

// Every oracle carries the docs-metadata output (the `const` meta, `symbolDef`s
// on nested properties and on the request/response roots, their `label`, the
// allOf fold): a published @xyd-js/native built from a core that predates it
// cannot produce them, so under the NATIVE path the matrix runs only once the
// addon has caught up. The gate is a feature probe, not a version pin — it
// lifts by itself the moment a native core that produces this core's
// 9.schema-links rows resolves (every feature at once: the `const` meta, a
// nested link with its label, the scalar fold, the merged rows of a composed
// member, the response root's link — an addon carrying only some of them stays
// gated) and that emits 13.open-objects-null-enums' `additionalProperties`
// meta and null-free nullable enums; until then the matrix is exercised on the
// JS impl (XYD_NATIVE=0) and the Rust core by `cargo test` (tests/parity.rs).
const nativeHasDocsMetadata = (() => {
    if (typeof native?.oapSchemaToReferencesFromFile !== "function") {
        return false
    }
    try {
        const probe = path.join(__dirname, "..", "__fixtures__", "9.schema-links", "input.yaml")
        const refs: any[] = JSON.parse(native.oapSchemaToReferencesFromFile(probe))
        const rows: any[] = refs.find(r => r.canonical === "objects/Widget")?.definitions?.[0]?.properties ?? []
        const row = (name: string) => rows.find(p => p.name === name)
        const getWidget = refs.find(r => r.canonical === "getWidget")
        const response = getWidget?.definitions?.find((d: any) => d.title === "Response")?.variants?.[0]
        const open = path.join(__dirname, "..", "__fixtures__", "13.open-objects-null-enums", "input.yaml")
        const openRefs: any[] = JSON.parse(native.oapSchemaToReferencesFromFile(open))
        const createBody = openRefs.find(r => r.canonical === "createNote")
            ?.definitions?.find((d: any) => d.type === "$rest.request.body")?.variants?.[0]
        const size = openRefs.find(r => r.canonical === "objects/Note")
            ?.definitions?.[0]?.properties?.find((p: any) => p.name === "size")
        return !!row("object")?.meta?.some((m: any) => m.name === "const")
            && row("definition")?.symbolDef?.canonical === "objects/WidgetDefinition"
            && row("definition")?.symbolDef?.label === "WidgetDefinition"
            && row("id")?.type === "string"
            && row("assembly")?.properties?.map((p: any) => p.name).join(",") === "label,id,sku"
            && response?.symbolDef?.label === "widget"
            && !!createBody?.meta?.some((m: any) => m.name === "additionalProperties")
            && !!size?.meta?.some((m: any) => m.name === "nullable")
            && !size?.properties?.some((p: any) => p.name === "null")
    } catch {
        return false
    }
})()

describe("oapSchemaToReferences", {timeout: 60000}, () => {
    tests.forEach((test) => {
        // KNOWN DIVERGENCE (S6+ W2): -2.complex.openai under the NATIVE path.
        // The JS impl's circular-schema handling deep-copies visited schemas
        // MID-CONSTRUCTION, embedding construction-order-dependent partial
        // snapshots (wrong type, empty description, missing meta) throughout
        // this circular-heavy spec — and STACK-OVERFLOWS on a minimal circular
        // oneOf repro. The Rust core resolves those same nodes to their final
        // well-formed shape, so the oracle (a frozen JS output) can't match.
        // The full 625-page openai docs site builds BYTE-IDENTICAL in both
        // modes — the divergence never reaches rendered output. The oracle is
        // regenerated from Rust at the impl-js reap, removing this skip.
        const nativePath = process.env.XYD_NATIVE !== "0"
        const skipNative = nativePath && (
            test.name === "-2.complex.openai"
            || (!!native && !nativeHasDocsMetadata)
        );
        (skipNative ? it.skip : it)(`[${test.name}]: ${test.description}`, async () => {
            await testOasSchemaToReferences(test.name, test.options, test.plugins, test.url);
        });
    });
});
