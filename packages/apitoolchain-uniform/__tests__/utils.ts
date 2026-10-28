import * as path from "node:path";
import * as fs from "node:fs";

import {expect} from "vitest";

import {uniformToInputJsonSchema} from "../src/converters";
import {pluginJsonView} from "../src/plugins/pluginJsonView";
import {pluginNavigation} from "../src/plugins/pluginNavigation";
import uniform from "../src/index";
import {uniformToMcpTools, type McpToolsOptions} from "../src/mcp";
import {skillFiles, type SkillOptions} from "../src/skill/index";
import type {Reference} from "../src/types";

// Oracle regen is EXPLICIT ONLY (fixture-freeze rule): committed output.json
// files are the parity oracle for the Rust migration (crates/apitoolchain_uniform).
//   UNIFORM_BUILD_FIXTURES=1 pnpm vitest run
const REGEN = process.env.UNIFORM_BUILD_FIXTURES === "1";

function writeOracle(fixtureName: string, result: unknown) {
    fs.writeFileSync(
        path.join(fixturePath(fixtureName), "output.json"),
        JSON.stringify(result, null, 2),
    );
}

export function fixturePath(name: string) {
    return path.join(__dirname, "../__fixtures__", name);
}

export function readFixture(name: string) {
    return JSON.parse(fs.readFileSync(fixturePath(name), "utf8"));
}

export function loadReference(fixtureName: string): Reference {
    return readFixture(`${fixtureName}/reference.json`);
}

export function loadInstance(fixtureName: string): any {
    return readFixture(`${fixtureName}/instance.json`);
}

export function fixture(strings: TemplateStringsArray, ...values: any[]): { reference: Reference; instance: any } {
    const name = String.raw(strings, ...values);
    return {
        reference: loadReference(name),
        instance: loadInstance(name),
    };
}

export async function testConverters(fixtureName: string) {
    const inputData = readFixture(`${fixtureName}/input.json`);
    const expectedOutput = readFixture(`${fixtureName}/output.json`);

    let result;

    if (Array.isArray(inputData)) {
        result = [];
        for (const def of inputData) {
            const schema = uniformToInputJsonSchema(def);
            if (schema) result.push(schema);
        }
    } else {
        result = uniformToInputJsonSchema(inputData);
    }

    if (REGEN) {
        writeOracle(fixtureName, result);
        return;
    }
    expect(result).toEqual(expectedOutput);
}

export async function testPluginJsonView(fixtureName: string) {
    const inputs: Reference[] = readFixture(`${fixtureName}/input.json`);
    const expectedOutput: string[] = readFixture(`${fixtureName}/output.json`);

    const plugin = pluginJsonView();
    const result = uniform(inputs, {plugins: [plugin]});

    if (REGEN) {
        writeOracle(fixtureName, result.out.jsonViews);
        return;
    }
    expect(result.out.jsonViews).toStrictEqual(expectedOutput);
}

/**
 * Fixture-driven pluginNavigation parity case. input.json:
 * `{ settings?, options: {urlPrefix, defaultGroup?}, references: Reference[] }`
 * output.json: `{ pageFrontMatter, sidebar }` (the plugin's deferred output).
 */
export async function testPluginNavigation(fixtureName: string) {
    const input = readFixture(`${fixtureName}/input.json`);

    const plugin = pluginNavigation(input.settings || {}, input.options);
    const result = uniform(input.references as Reference[], {plugins: [plugin]});
    const out = {
        pageFrontMatter: (result.out as any).pageFrontMatter,
        sidebar: (result.out as any).sidebar,
    };

    if (REGEN) {
        writeOracle(fixtureName, out);
        return;
    }
    const expectedOutput = readFixture(`${fixtureName}/output.json`);
    expect(out).toEqual(expectedOutput);
}
// ---------------------------------------------------------------------------
// The agent emitters (uniformToMcpTools, skillFiles). Their fixtures carry an
// options.json whose `options` are passed through as they are, plus a few
// keys this harness turns into the callback options — the Rust parity test
// (crates/apitoolchain_uniform/tests/agent_emitters.rs) builds the same
// callbacks from the same keys:
//   readOnly:  canonicals that are read-only besides every GET
//   exclude:   canonicals left out (`include`)
//   toolNames: tool names by canonical (the rest get the default)
//   extras:    `extras` by canonical
//   header:    the generated-file comment, `{group}` replaced by the group
//   groups:    groups by canonical (the rest get the default)
// The oracles are compared as TEXT: output.json byte for byte, the skill's
// output/ tree file for file.
// ---------------------------------------------------------------------------

function readOptionsFixture(fixtureName: string): any {
    const file = path.join(fixturePath(fixtureName), "options.json");
    return fs.existsSync(file) ? JSON.parse(fs.readFileSync(file, "utf8")) : {};
}

function excluded(fixture: any) {
    const list: string[] | undefined = fixture.exclude;
    return list ? (ref: Reference) => !list.includes(ref.canonical) : undefined;
}

export function mcpToolsOptionsFromFixture(fixture: any): McpToolsOptions {
    const options: McpToolsOptions = { ...(fixture.options || {}) };
    if (fixture.readOnly) {
        const list: string[] = fixture.readOnly;
        options.readOnly = (ref) =>
            (ref.context as any)?.method === "get" || list.includes(ref.canonical);
    }
    options.include = excluded(fixture);
    if (fixture.toolNames) {
        options.toolName = (ref) => fixture.toolNames[ref.canonical] ?? "";
    }
    if (fixture.extras) {
        options.extras = (ref) => fixture.extras[ref.canonical];
    }
    return options;
}

export function skillOptionsFromFixture(fixture: any): SkillOptions {
    const options: SkillOptions = { ...fixture.options };
    if (fixture.header) {
        options.references = {
            ...(options.references || {}),
            header: (group) => fixture.header.replace("{group}", group),
        };
    }
    options.include = excluded(fixture);
    if (fixture.groups) {
        options.groupBy = (ref) => fixture.groups[ref.canonical] ?? ((ref.context as any)?.group?.[0] || "");
    }
    return options;
}

export function testMcpTools(fixtureName: string) {
    const references: Reference[] = readFixture(`${fixtureName}/input.json`);
    const tools = uniformToMcpTools(references, mcpToolsOptionsFromFixture(readOptionsFixture(fixtureName)));
    const actual = JSON.stringify(tools, null, 2) + "\n";
    const oracle = path.join(fixturePath(fixtureName), "output.json");
    if (REGEN) {
        fs.writeFileSync(oracle, actual);
        return;
    }
    expect(actual).toBe(fs.readFileSync(oracle, "utf8"));
}

function listFiles(dir: string, prefix = ""): string[] {
    const out: string[] = [];
    for (const entry of fs.readdirSync(dir, {withFileTypes: true})) {
        const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
        if (entry.isDirectory()) {
            out.push(...listFiles(path.join(dir, entry.name), rel));
        } else {
            out.push(rel);
        }
    }
    return out.sort();
}

export function testSkillFiles(fixtureName: string) {
    const references: Reference[] = readFixture(`${fixtureName}/input.json`);
    const files = skillFiles(references, skillOptionsFromFixture(readOptionsFixture(fixtureName)));
    const outputDir = path.join(fixturePath(fixtureName), "output");
    if (REGEN) {
        fs.rmSync(outputDir, {recursive: true, force: true});
        for (const [file, contents] of files) {
            fs.mkdirSync(path.dirname(path.join(outputDir, file)), {recursive: true});
            fs.writeFileSync(path.join(outputDir, file), contents);
        }
        return;
    }
    expect([...files.keys()].sort()).toEqual(listFiles(outputDir));
    for (const [file, contents] of files) {
        expect(contents, file).toBe(fs.readFileSync(path.join(outputDir, file), "utf8"));
    }
}
