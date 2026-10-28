// Input generator for the uniform package's agent-emitter fixtures
// (packages/apitoolchain-uniform/__fixtures__/{5.mcp-tools.*,6.skill.*}):
// each case's input.json is the shared sample document converted by THIS
// package, so the emitters are pinned against real converter output rather
// than hand-written references. Regen is explicit only (oracle-freeze rule):
//   XYD_NATIVE=0 bun scripts/build-agent-fixture-inputs.ts
// XYD_NATIVE=0 selects the JS converter, whose endpoint code samples are what
// the skill emitter's "Example request" sections read.
import path from "node:path";
import fs from "node:fs";

if (process.env.XYD_NATIVE !== "0") {
    console.error("run with XYD_NATIVE=0 — inputs must come from the JS converter");
    process.exit(1);
}

const { deferencedOpenAPI, oapSchemaToReferences } = await import("../src/index.ts");

const FIXTURES = path.join(import.meta.dirname, "..", "..", "apitoolchain-uniform", "__fixtures__");
const SAMPLE = path.join(FIXTURES, "agent-sample.openapi.yaml");

const CASES = [
    "5.mcp-tools.default",
    "5.mcp-tools.options",
    "5.mcp-tools.grouped",
    "5.mcp-tools.open-body",
    "6.skill.default",
    "6.skill.options",
    "6.skill.per-operation",
];

const doc = await deferencedOpenAPI(SAMPLE);
// JSON round-trip: drops the __UNSAFE_selector thunks, as every committed
// Reference[] fixture does.
const references = JSON.parse(JSON.stringify(oapSchemaToReferences(doc)));
const text = JSON.stringify(references, null, 2) + "\n";

for (const name of CASES) {
    const dir = path.join(FIXTURES, name);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, "input.json"), text);
    console.log(`wrote ${name}/input.json (${references.length} references)`);
}
