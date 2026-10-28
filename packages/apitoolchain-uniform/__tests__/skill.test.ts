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

    it("links code labels without escaping inside the code span", () => {
        const op = {
            title: "t",
            canonical: "things[0]",
            description: "",
            category: "rest",
            type: "rest_get",
            context: {method: "get", path: "/x", group: ["g"]},
            definitions: [],
            examples: {groups: []},
        } as unknown as Reference;
        const files = skillFiles([op], {name: "x", description: "y", references: {perOperation: true}});
        expect(files.get("references/index.md")).toContain("| [`things[0]`](g/things-0.md) |");
    });
});

describe("skillFiles operation details", () => {
    const op = (definitions: any[], examples: any = {groups: []}) =>
        ({
            title: "Search things",
            canonical: "searchThings",
            description: "",
            category: "rest",
            type: "rest_post",
            context: {method: "post", path: "/things/{id}/search", fullPath: "https://api.example.com/things/{id}/search", group: ["things"]},
            definitions,
            examples,
        }) as unknown as Reference;
    const required = [{name: "required", value: "true"}];
    const definitions = [
        {title: "Path parameters", type: "$rest.param.path", properties: [{name: "id", type: "string", description: "", meta: required}]},
        {
            title: "Query parameters",
            type: "$rest.param.query",
            properties: [
                {name: "limit", type: "integer", description: "", meta: [{name: "defaults", value: 20}]},
                {name: "q", type: "string", description: "", meta: [...required, {name: "example", value: "rex's"}]},
                {name: "sort", type: "string", description: "", meta: [{name: "defaults", value: ""}]},
            ],
        },
        {
            title: "Request body",
            type: "$rest.request.body",
            properties: [],
            meta: required,
            variants: [
                {
                    title: "application/json",
                    meta: [{name: "contentType", value: "application/json"}],
                    properties: [
                        {name: "thing", type: "string", description: "", meta: required},
                        {name: "thing_id", type: "string", description: "Alias of thing."},
                        {name: "size", type: "$$enum", description: "", meta: required, properties: [{name: "s"}, {name: "m"}], ofProperty: {type: "string"}},
                        {name: "filter", type: "object", description: "", meta: required, properties: [{name: "min", type: "integer", description: "", meta: required}, {name: "max", type: "integer", description: ""}]},
                    ],
                },
            ],
        },
        {
            title: "Response",
            type: "return",
            properties: [],
            variants: [
                {
                    title: "200",
                    description: "The matches.",
                    meta: [{name: "status", value: "200"}, {name: "contentType", value: "application/json"}],
                    properties: [
                        {name: "data", type: "$$array", description: "", ofProperty: {name: "", type: "object", properties: [{name: "id", type: "string", description: "The ID."}]}},
                        {name: "total", type: "integer", description: "How many."},
                    ],
                },
                {title: "204", description: "Nothing.", meta: [{name: "status", value: "204"}], properties: []},
            ],
        },
    ];

    it("renders an empty default as a quoted empty string", () => {
        const md = skillFiles([op(definitions)], {name: "x", description: "y"}).get("references/things.md")!;
        expect(md).toContain("| `sort` | string | no | Default: `\"\"`. |");
        expect(md).not.toContain("``.");
    });

    it("lists each response's fields one level deep", () => {
        const md = skillFiles([op(definitions)], {name: "x", description: "y"}).get("references/things.md")!;
        expect(md).toContain(
            "#### 200 — `application/json`\n\n| Name | Type | Description |\n|---|---|---|\n| `data` | array of object |  |\n| `data[].id` | string | The ID. |\n| `total` | integer | How many. |"
        );
        expect(md).not.toContain("#### 204");
    });

    it("tabulates the 2xx responses' fields by default, every response's or none on request", () => {
        const withError = definitions.map((d: any) =>
            d.type === "return"
                ? {
                      ...d,
                      variants: [
                          ...d.variants,
                          {
                              title: "404",
                              description: "Not found.",
                              meta: [{name: "status", value: "404"}, {name: "contentType", value: "application/json"}],
                              properties: [{name: "code", type: "string", description: "An error code."}],
                          },
                      ],
                  }
                : d
        );
        const render = (responseFields?: "all" | "success" | "none") =>
            skillFiles([op(withError)], {name: "x", description: "y", responseFields}).get("references/things.md")!;
        expect(render()).toContain("#### 200 — `application/json`");
        expect(render()).not.toContain("#### 404");
        expect(render()).toContain("| 404 | `application/json` | object | Not found. |");
        expect(render("all")).toContain("#### 404 — `application/json`");
        expect(render("none")).not.toContain("#### ");
    });

    it("synthesizes a request from the required inputs plus limit when none is given", () => {
        const md = skillFiles([op(definitions)], {name: "x", description: "y"}).get("references/things.md")!;
        expect(md).toContain(
            [
                "```shell",
                "curl --request POST \\",
                "     --url 'https://api.example.com/things/<id>/search?limit=20&q=rex%27s' \\",
                "     --header 'content-type: application/json' \\",
                "     --data '{",
                '  "thing": "<thing>",',
                '  "size": "s",',
                '  "filter": {',
                '    "min": 0',
                "  }",
                "}'",
                "```",
            ].join("\n")
        );
    });

    it("prefers the references' own example, or leaves it out on request", () => {
        const given = {groups: [{kind: "request", examples: [{codeblock: {tabs: [{title: "shell", language: "shell", code: "curl given"}]}}]}]};
        const auto = skillFiles([op(definitions, given)], {name: "x", description: "y"}).get("references/things.md")!;
        expect(auto).toContain("```shell\ncurl given\n```");
        const synthesized = skillFiles([op(definitions, given)], {name: "x", description: "y", exampleRequest: "synthesized"}).get(
            "references/things.md"
        )!;
        expect(synthesized).not.toContain("curl given");
        const none = skillFiles([op(definitions, given)], {name: "x", description: "y", exampleRequest: "none"}).get(
            "references/things.md"
        )!;
        expect(none).not.toContain("Example request");
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
