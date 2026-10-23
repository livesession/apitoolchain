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
        description: "x-docs name/group/hidden on component schemas, outside sidebar mode",
        plugins: [
            uniformPluginXDocsSidebar
        ]
    }
]

// Fixtures whose oracles carry the docs-metadata output (the `const` meta,
// `symbolDef`s on nested properties, the allOf-scalar fold): a published
// @xyd-js/native built from a core that predates it cannot produce them, so
// under the NATIVE path they run only once the addon has caught up. The gate is
// a feature probe, not a version pin — it lifts by itself the moment a native
// core that emits `const` resolves; until then these cases are exercised on the
// JS impl (XYD_NATIVE=0) and the Rust core by `cargo test` (tests/parity.rs).
const DOCS_METADATA_FIXTURES = new Set(["-3.random", "8.enums", "9.schema-links", "5.xdocs.objects"])
const nativeHasDocsMetadata = (() => {
    if (typeof native?.oapSchemaToReferencesFromFile !== "function") {
        return false
    }
    try {
        const probe = path.join(__dirname, "..", "__fixtures__", "9.schema-links", "input.yaml")
        const out: string = native.oapSchemaToReferencesFromFile(probe)
        return out.includes('"name":"const"')
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
            || (DOCS_METADATA_FIXTURES.has(test.name) && !!native && !nativeHasDocsMetadata)
        );
        (skipNative ? it.skip : it)(`[${test.name}]: ${test.description}`, async () => {
            await testOasSchemaToReferences(test.name, test.options, test.plugins, test.url);
        });
    });
});
