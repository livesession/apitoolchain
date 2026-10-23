// Golden generator for the FUSED uniform endpoint (S6+ W3 tail).
// Replicates the slice of plugin-docs' uniformResolver that the Rust
// `apitoolchain_openapi::fused::uniform_oas_pages` fuses: JS conversion →
// x-docs sidebar plugin → pluginNavigation → per-ref {pagePath, region}.
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

if (process.env.XYD_NATIVE !== "0") {
    console.error("run with XYD_NATIVE=0 — goldens must come from the JS impls");
    process.exit(1);
}

// JS impls: the openapi shim's loader sees XYD_NATIVE=0 → the frozen converter
// and the JS x-docs plugin. (It imports @xyd-js/uniform's DIST, a separate
// module instance from the src imported below.)
const { deferencedOpenAPI, oapSchemaToReferences, uniformPluginXDocsSidebar } = await import("../src/index.ts");
// Native: pluginNavigation's only implementation. The uniform package's loader
// (`src/native.ts`) is first imported by `src/index.ts` and evaluates once, so
// the flag is lifted before that import — it then affects this half alone.
delete process.env.XYD_NATIVE;
const uniformMod = await import("../../apitoolchain-uniform/src/index.ts");
const { pluginNavigation } = await import("../../apitoolchain-uniform/src/plugins/pluginNavigation.ts");
const uniform = uniformMod.default;

const FIXTURES = path.join(import.meta.dirname, "..", "__fixtures__");
const URL_PREFIX = "docs/api"; // the fixed test prefix the Rust test mirrors

const cases = ["1.basic", "2.more", "3.multiple-responses", "5.xdocs.sidebar", "5.xdocs.objects", "8.enums"];

for (const name of cases) {
    const input = path.join(FIXTURES, name, "input.yaml");
    const doc = await deferencedOpenAPI(input);
    const refs = oapSchemaToReferences(doc);

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

    const golden = {
        urlPrefix: URL_PREFIX,
        sidebar: res.out.sidebar,
        pageFrontMatter: res.out.pageFrontMatter,
        pages,
    };
    fs.writeFileSync(
        path.join(FIXTURES, name, "fused.golden.json"),
        JSON.stringify(golden, null, 2),
    );
    console.log(name, "→ fused.golden.json", `(${pages.length} pages)`);
}
