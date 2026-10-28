// Markdown and YAML text helpers for the skill emitter. Mirrored line for line
// by crates/apitoolchain_uniform/src/skill.rs.

/** Inline code that survives backticks in its text: the fence is one backtick
 *  longer than the longest run inside, padded with a space when the text
 *  starts or ends with a backtick (CommonMark strips one from each side). */
export function inlineCode(value: string): string {
    const longest = longestRun(value, "`");
    if (longest === 0) {
        return "`" + value + "`";
    }
    const fence = "`".repeat(longest + 1);
    const pad = value.startsWith("`") || value.endsWith("`") ? " " : "";
    return fence + pad + value + pad + fence;
}

/** A fenced code block whose fence outlasts any backtick run in the code. */
export function codeBlock(code: string, language: string): string {
    const fence = "`".repeat(Math.max(3, longestRun(code, "`") + 1));
    const info = language.includes("`") ? "" : collapse(language);
    const body = code.endsWith("\n") ? code : code + "\n";
    return `${fence}${info}\n${body}${fence}`;
}

function longestRun(value: string, char: string): number {
    let longest = 0;
    let current = 0;
    for (const c of value) {
        current = c === char ? current + 1 : 0;
        longest = Math.max(longest, current);
    }
    return longest;
}

/** Whitespace runs (newlines included) become one space; trimmed. */
export function collapse(value: string): string {
    return value.split(/\s+/).filter(Boolean).join(" ");
}

/** A table cell: one line, pipes escaped. */
export function cell(value: string): string {
    return collapse(value).replace(/\|/g, "\\|");
}

/** A table row from cells that are already escaped. */
export function row(cells: string[]): string {
    return `| ${cells.join(" | ")} |`;
}

export function tableHeader(names: string[]): string {
    return `${row(names)}\n|${names.map(() => "---").join("|")}|`;
}

/** A link to a relative path: `[path](path)`, the target in angle brackets
 *  when it holds spaces or parentheses. */
export function link(label: string, target: string): string {
    const text = label.replace(/[\[\]]/g, (c) => "\\" + c);
    const href = /[\s()]/.test(target) ? `<${target}>` : target;
    return `[${text}](${href})`;
}

/** An HTML comment whose text cannot close it early. */
export function comment(value: string): string {
    return `<!-- ${collapse(value).replace(/-->/g, "-- >")} -->`;
}

const YAML_INDICATORS = "-?:,[]{}#&*!|>'\"%@`";
const YAML_RESERVED = new Set(["true", "false", "yes", "no", "on", "off", "y", "n", "null", "~"]);

/** A YAML scalar: plain when that reads back as the same string, else
 *  double-quoted (JSON escapes are valid YAML escapes). */
export function yamlScalar(value: string): string {
    return needsQuotes(value) ? JSON.stringify(value) : value;
}

function needsQuotes(value: string): boolean {
    if (value === "") {
        return true;
    }
    const first = value[0];
    const last = value[value.length - 1];
    if (YAML_INDICATORS.includes(first) || /\s/.test(first) || /\s/.test(last) || last === ":") {
        return true;
    }
    if (value.includes(": ") || value.includes(" #")) {
        return true;
    }
    for (const c of value) {
        const code = c.codePointAt(0) as number;
        if (code < 0x20 || code === 0x7f) {
            return true;
        }
    }
    if (YAML_RESERVED.has(value.toLowerCase())) {
        return true;
    }
    // Anything that could read as a number.
    return /^[0-9]/.test(value) || /^[+.][0-9]/.test(value);
}
