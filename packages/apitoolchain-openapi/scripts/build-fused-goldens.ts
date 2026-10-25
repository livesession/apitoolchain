// Golden generator for the FUSED uniform endpoint (S6+ W3 tail).
// Replicates the slice of plugin-docs' uniformResolver that the Rust
// `apitoolchain_openapi::fused::uniform_oas_pages` fuses: JS conversion →
// x-docs sidebar plugin → pluginNavigation → the navigation's finishing pass
// (the objects group's `collapsed` flag) → per-ref {pagePath, region}.
// Regen is explicit only (oracle-freeze rule):
//   XYD_NATIVE=0 bun scripts/build-fused-goldens.ts
// XYD_NATIVE=0 is REQUIRED for the halves that still have a JS impl: the
// conversion and the x-docs plugin come from `src/impl-js`. pluginNavigation
// has NO JS impl any more (reaped with @xyd-js/native@0.1.0), so that one step
// is supplied by the addon — its group-tree semantics are pinned by the
// uniform package's own fixtures, and a golden only carries its output shape.
// The env flag is therefore flipped between the two imports below: each
// shim's loader reads it at import time.
import path from "node:path";
import fs from "node:fs";

import type { uniformOasOptions } from "../src/impl-js/types";

if (process.env.XYD_NATIVE !== "0") {
    console.error("run with XYD_NATIVE=0 — goldens must come from the JS impls");
    process.exit(1);
}

// JS impls: the openapi shim's loader sees XYD_NATIVE=0 → the frozen converter
// and the JS x-docs plugin. (It imports @xyd-js/uniform's DIST, a separate
// module instance from the src imported below.)
const { deferencedOpenAPI, oapSchemaToReferences, uniformPluginXDocsSidebar, collapseObjectsGroup } = await import("../src/index.ts");
// Native: pluginNavigation's only implementation. The uniform package's loader
// (`src/native.ts`) is first imported by `src/index.ts` and evaluates once, so
// the flag is lifted before that import — it then affects this half alone.
delete process.env.XYD_NATIVE;
const uniformMod = await import("../../apitoolchain-uniform/src/index.ts");
const { pluginNavigation } = await import("../../apitoolchain-uniform/src/plugins/pluginNavigation.ts");
const uniform = uniformMod.default;

const FIXTURES = path.join(import.meta.dirname, "..", "__fixtures__");
const URL_PREFIX = "docs/api"; // the fixed test prefix the Rust test mirrors

// A case's `options` are the converter options the Rust test passes through
// `FusedInput` for it (tests/fused.rs). 5.xdocs.objects pins the per-area
// objects layout explicitly — the default is `grouped`, which 11.objects-layouts
// pins; 12.page-paths pins the title-derived page paths (the default,
// `operation-id`, is what every other case runs under).
const cases: { name: string; options?: uniformOasOptions }[] = [
    { name: "1.basic" },
    { name: "2.more" },
    { name: "3.multiple-responses" },
    { name: "5.xdocs.sidebar" },
    { name: "5.xdocs.objects", options: { objectsLayout: "per-area" } },
    { name: "8.enums" },
    { name: "11.objects-layouts" },
    { name: "12.page-paths", options: { operationPaths: "title" } },
];

for (const { name, options } of cases) {
    const input = path.join(FIXTURES, name, "input.yaml");
    const doc = await deferencedOpenAPI(input);
    const refs = oapSchemaToReferences(doc, options);

    // uniformResolver plugin order for openapi: [xdocs, navigation].
    const res = uniform(refs, {
        plugins: [
            uniformPluginXDocsSidebar,
            pluginNavigation({} as any, { urlPrefix: URL_PREFIX }),
        ],
    }) as any;

    const pages = res.references.map((ref: any) => {
        const byCanonical = path.join(URL_PREFIX, ref.canonical);
        const ctx = ref.context;
        const method = (ctx?.method || "").toUpperCase();
        let region = "";
        if (method && ctx?.path) {
            region = `${method} ${ctx?.path}`;
        } else if (ctx.componentSchema) {
            region = "/components/schemas/" + ctx.componentSchema;
        }
        return { pagePath: byCanonical, region };
    });

    // The finishing pass over the generated navigation (the JS twin of the
    // Rust `navigation::collapse_objects_group`, run by the fused endpoint).
    const sidebar = collapseObjectsGroup(res.out.sidebar, doc, res.references, URL_PREFIX, options);

    const golden = {
        urlPrefix: URL_PREFIX,
        sidebar,
        pageFrontMatter: res.out.pageFrontMatter,
        pages,
    };
    fs.writeFileSync(
        path.join(FIXTURES, name, "fused.golden.json"),
        JSON.stringify(golden, null, 2),
    );
    console.log(name, "→ fused.golden.json", `(${pages.length} pages)`);
}
