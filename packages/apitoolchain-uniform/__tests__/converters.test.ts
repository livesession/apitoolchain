import {describe, it, expect} from "vitest";

import {uniformPropertiesToJsonSchema} from "../src/converters";
import {testConverters} from "./utils";

const tests = [
    {name: "1.converters.basic", description: "Basic REST endpoint (Update user)"},
    {name: "1.converters.advanced", description: "Advanced union/enum types (OpenAI)"},
    {name: "1.converters.advanced-livesession", description: "Advanced LiveSession API"},
];

describe("uniformToInputJsonSchema", () => {
    for (const t of tests) {
        it(t.description, async () => {
            await testConverters(t.name);
        });
    }
});

// `uniformPropertiesToJsonSchema` is re-exported from this package's root, so
// it is public API — and until now NOTHING exercised it. That is how it came to
// be the one entry with no native binding: the frozen JS impl backed it, no
// test noticed, and deleting that impl would have dropped a public symbol
// silently. These cases pin both overloads so the binding cannot regress the
// same quiet way.
describe("uniformPropertiesToJsonSchema", () => {
    const prop = (name: string, type: string, required = false) => ({
        name,
        type,
        description: "",
        ...(required ? {meta: [{name: "required", value: "true"}]} : {}),
    });

    it("array overload: properties become an object schema", () => {
        const schema = uniformPropertiesToJsonSchema([
            prop("id", "string", true),
            prop("age", "number"),
        ] as any);

        expect(schema).toMatchObject({
            type: "object",
            properties: {id: {type: "string"}, age: {type: "number"}},
            required: ["id"],
        });
    });

    it("array overload: `required` is omitted entirely when nothing is required", () => {
        const schema = uniformPropertiesToJsonSchema([prop("age", "number")] as any);
        expect(schema).not.toHaveProperty("required");
    });

    it("array overload: `id` becomes $id", () => {
        const schema = uniformPropertiesToJsonSchema([prop("id", "string")] as any, "urn:x");
        expect(schema).toMatchObject({$id: "urn:x"});
    });

    // The JS source wrote `$id: id || undefined` — an empty string is falsy
    // there, so it must NOT emit `$id: ""`. The Rust side reproduces this with
    // a `truthy()` gate; this is the case that proves the two agree, and the
    // reason the binding passes `id` through untouched instead of pre-filtering.
    it("array overload: an empty `id` is falsy, not an empty $id", () => {
        const schema = uniformPropertiesToJsonSchema([prop("id", "string")] as any, "");
        expect(schema).not.toHaveProperty("$id");
    });

    it("single-property overload: a lone property converts on its own", () => {
        expect(uniformPropertiesToJsonSchema(prop("age", "number") as any))
            .toMatchObject({type: "number"});
    });
});