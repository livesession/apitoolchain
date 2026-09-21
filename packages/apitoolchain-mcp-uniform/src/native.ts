// Loader for the Rust core (S6+ W3 rider). Same bundler-invisible pattern as
// @xyd-js/uniform's loader (no static node:module import — see that file's
// header): browser → null; XYD_NATIVE=0 → __xydNativeCore (compiled binary) →
// @xyd-js/native → null.
//
// A null result used to mean "use the frozen JS impl". It now means the
// conversion entry THROWS: src/impl-js was deleted once @xyd-js/native shipped
// platform binaries. XYD_NATIVE=0 is therefore no longer a working hatch for
// this package. What survives in JS is src/transport.ts — the JSON-RPC, auth
// and manifest IO — which is live production code on the native path, not a
// fallback.

function load(): any | null {
    if (typeof process === "undefined" || !process.versions) return null;
    if (process.env?.XYD_NATIVE === "0") return null;
    const embedded = (globalThis as any).__xydNativeCore;
    if (embedded?.mcpToReferences) return embedded;
    let mod: any = null;
    try {
        const nodeModule = (process as any).getBuiltinModule?.("node:module");
        const require = nodeModule?.createRequire?.(import.meta.url);
        mod = require?.("@xyd-js/native") ?? null;
    } catch {
        mod = null;
    }
    // See the note in @xyd-js/gql's loader: XYD_REQUIRE_NATIVE=1 makes a failed
    // load fatal, so a "native" CI leg can't silently be a second JS run.
    if (process.env?.XYD_REQUIRE_NATIVE === "1" && typeof mod?.mcpToReferences !== "function") {
        throw new Error("XYD_REQUIRE_NATIVE=1 but @xyd-js/native.mcpToReferences is unavailable");
    }
    return mod;
}

export const native = load();
