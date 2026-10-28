import {describe, expect, it} from "vitest";

import {skillFiles} from "../src/skill/index";
import {codeBlock, cell, inlineCode, yamlScalar} from "../src/skill/markdown";
import type {Reference} from "../src/types";
import {readFixture, testSkillFiles} from "./utils";

// The oracles the Rust twin (crates/apitoolchain_uniform::skill) is compared
// with file for file. Regen: UNIFORM_BUILD_FIXTURES=1 pnpm vitest run
const fixtureTests = [
    {name: "6.skill.default", description: "defaults: references/, groups from context.group"},
    {name: "6.skill.options", description: "title, intro, extras, dir, header, groupBy, include"},
    {name: "6.skill.per-operation", description: "one file per operation under its group's directory"},
];

describe("skillFiles fixtures", () => {
    for (const t of fixtureTests) {
        it(t.description, () => {
            testSkillFiles(t.name);
        });
    }
});

describe("skillFiles", () => {
    it("keeps the intro and the extras byte for byte", () => {
        const references: Reference[] = readFixture("6.skill.default/input.json");
        const intro = "Line one  \n\n```js\nconst a = `x`;\n```\n\n| a | b |\n";
        const extra = "# Notes\r\n\nkept as is\t\n";
        const files = skillFiles(references, {
            name: "sample",
            description: "Sample",
            intro,
            extras: {"notes.md": extra},
        });
        expect(files.get("SKILL.md")).toContain(`\n\n${intro}\n## API reference`);
        expect(files.get("notes.md")).toBe(extra);
        expect(files.get("SKILL.md")).toContain("- [notes.md](notes.md)");
    });

    it("never files a group over the index", () => {
        const op = (canonical: string, group: string) =>
            ({
                title: canonical,
                canonical,
                description: "",
                category: "rest",
                type: "rest_get",
                context: {method: "get", path: `/${canonical}`, group: [group]},
                definitions: [],
                examples: {groups: []},
            }) as unknown as Reference;
        const files = skillFiles([op("a", "Index"), op("b", "index!"), op("c", "")], {name: "x", description: "y"});
        expect([...files.keys()]).toEqual([
            "SKILL.md",
            "references/index.md",
            "references/index-2.md",
            "references/index-3.md",
            "references/general.md",
        ]);
    });
});

describe("skillFiles per operation", () => {
    it("keeps the operation's case and never lets two names meet on a case-insensitive disk", () => {
        const op = (canonical: string) =>
            ({
                title: canonical,
                canonical,
                description: "",
                category: "rest",
                type: "rest_get",
                context: {method: "get", path: "/x", group: ["Things & Stuff"]},
                definitions: [],
                examples: {groups: []},
            }) as unknown as Reference;
        const files = skillFiles([op("getThing"), op("GetThing"), op("things/list"), op("../up")], {
            name: "x",
            description: "y",
            references: {perOperation: true},
        });
        expect([...files.keys()]).toEqual([
            "SKILL.md",
            "references/index.md",
            "references/things-stuff/getThing.md",
            "references/things-stuff/GetThing-2.md",
            "references/things-stuff/things-list.md",
            "references/things-stuff/up.md",
        ]);
    });
});

describe("skill markdown helpers", () => {
    it("fences inline code around backticks", () => {
        expect(inlineCode("plain")).toBe("`plain`");
        expect(inlineCode("a `b` c")).toBe("``a `b` c``");
        expect(inlineCode("`x`")).toBe("`` `x` ``");
    });

    it("escapes table cells onto one line", () => {
        expect(cell("a | b\n  c")).toBe("a \\| b c");
    });

    it("fences code blocks longer than any backtick run inside", () => {
        expect(codeBlock("echo ```", "shell")).toBe("````shell\necho ```\n````");
    });

    it("quotes YAML scalars only when a plain one would not read back", () => {
        expect(yamlScalar("pet-store")).toBe("pet-store");
        expect(yamlScalar("Use it: now")).toBe('"Use it: now"');
        expect(yamlScalar("yes")).toBe('"yes"');
        expect(yamlScalar("1.0")).toBe('"1.0"');
        expect(yamlScalar("- item")).toBe('"- item"');
        expect(yamlScalar("ends with:")).toBe('"ends with:"');
        expect(yamlScalar('say "hi"')).toBe('say "hi"');
    });
});
