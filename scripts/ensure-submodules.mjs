#!/usr/bin/env node
// Preflight: the `opensdk` submodule must be checked out.
//
// It is a path-dependency of the Rust build, not an optional extra:
// crates/xyd_openapi path-deps opensdk/crates/oas_doc (the shared spec
// loader/dereferencer), so a missing checkout doesn't degrade gracefully —
// cargo cannot even load the workspace, and the one actionable line ends up
// buried under a wall of cargo output. Fail early and legibly instead.
//
//   strict (default): exit 1 with instructions.
//   --soft:           try to init, and never fail the caller.
import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

// A sentinel is a file that exists ONLY in a populated checkout — an
// uninitialized submodule is an empty DIRECTORY, so testing the directory
// itself would pass while the build still fails.
const SUBMODULES = [
    {
        name: "opensdk",
        sentinel: path.join(REPO, "opensdk", "crates", "oas_doc", "Cargo.toml"),
        what: "the SDK/CLI toolchain; crates/xyd_openapi path-deps its oas_doc",
        url: "github.com/livesession/opensdk",
    },
];

const soft = process.argv.includes("--soft");
const missing = SUBMODULES.filter((s) => !existsSync(s.sentinel));

if (missing.length === 0) process.exit(0);

if (soft) {
    // Best effort: this runs from `prepare`, which also fires in offline and
    // container contexts where failing would break unrelated flows.
    if (existsSync(path.join(REPO, ".git"))) {
        for (const s of missing) {
            spawnSync("git", ["submodule", "update", "--init", s.name], {
                cwd: REPO,
                stdio: "inherit",
            });
        }
    }
    const stillMissing = SUBMODULES.filter((s) => !existsSync(s.sentinel));
    if (stillMissing.length > 0) {
        console.warn(
            `\n  ⚠ Not initialized: ${stillMissing.map((s) => s.name).join(", ")}. Rust builds will fail.\n` +
                `    Run: git submodule update --init ${stillMissing.map((s) => s.name).join(" ")}\n`,
        );
    }
    process.exit(0);
}

console.error(
    `\n${missing.length === 1 ? "A submodule is" : `${missing.length} submodules are`} not initialized:\n\n` +
        missing
            .map((s) => `  ${s.name}  (missing ${path.relative(REPO, s.sentinel)})\n      ${s.what} — ${s.url}`)
            .join("\n") +
        `\n\n  git submodule update --init ${missing.map((s) => s.name).join(" ")}\n`,
);
process.exit(1);
