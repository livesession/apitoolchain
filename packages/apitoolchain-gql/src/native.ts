// Loader for the Rust core (S6+ W1). Resolution order:
//   1. XYD_NATIVE=0 → null
//   2. globalThis.__xydNativeCore — the embedded core.node inside the
//      bun-compiled binary (set by xyd-cli's native-boot)
//   3. @xyd-js/native — the napi package (platform .node via optionalDependencies)
//   4. null → the shim THROWS
//
// Steps 1 and 4 used to read "force the frozen JS impl — test/incident hatch"
// and "falls back to src/impl-js". That directory is gone: the frozen
// TypeScript converter was deleted once @xyd-js/native shipped platform
// binaries, so there is one implementation rather than two. XYD_NATIVE=0 is
// consequently no longer a usable hatch FOR THIS PACKAGE — it now produces the
// shim's "requires @xyd-js/native" error rather than a working JS run.
import { createRequire } from "node:module";

function load(): any | null {
    if (process.env.XYD_NATIVE === "0") return null;
    const embedded = (globalThis as any).__xydNativeCore;
    if (embedded?.gqlSchemaToReferences) return embedded;
    let mod: any = null;
    try {
        const require = createRequire(import.meta.url);
        mod = require("@xyd-js/native");
    } catch {
        mod = null;
    }
    // XYD_REQUIRE_NATIVE=1 turns a failed load into a HARD failure at LOAD time.
    // It mattered originally because a failed load silently downgraded to
    // impl-js, making the two CI legs indistinguishable: hiding the .node and
    // running `XYD_NATIVE=1 vitest run` here reported "14 passed",
    // byte-identical to the real native run — a native leg that never touched
    // native code, still green. With impl-js gone the call sites throw anyway,
    // so this now buys precision rather than detection: it fails HERE, naming
    // the addon, instead of at the first conversion with a message about a
    // missing dependency. Assert
    // the SYMBOL, not just the module, so "loaded but missing an export" (a
    // dropped #[napi] fn) fails here too rather than at the `native?.fn` call site.
    if (process.env.XYD_REQUIRE_NATIVE === "1" && typeof mod?.gqlSchemaToReferences !== "function") {
        throw new Error("XYD_REQUIRE_NATIVE=1 but @xyd-js/native.gqlSchemaToReferences is unavailable");
    }
    return mod;
}

export const native = load();
