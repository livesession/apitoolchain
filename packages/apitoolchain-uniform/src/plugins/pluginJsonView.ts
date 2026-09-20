// Dispatcher (S6+ W3): the view-building core runs in Rust
// (crates/apitoolchain_uniform::plugins::plugin_json_view) when the native core is
// present. The UniformPlugin closure contract stays JS — the factory defers
// one native call over the full Reference[].
import type { UniformPluginArgs, UniformPlugin } from "../index";
import type { Reference } from "../types";
import { native } from "../native";

export interface pluginJsonViewOptions {
}

type pluginJsonViewOutput = {
    jsonViews: string;
};

export function pluginJsonView(
    options?: pluginJsonViewOptions
): UniformPlugin<pluginJsonViewOutput> {
    if (!native?.pluginJsonView) {
        throw new Error(
            "@xyd-js/uniform.pluginJsonView requires @xyd-js/native, which did not load. " +
                "Install it (it is an optionalDependency, so a failed install is silent) " +
                "or build it locally with `pnpm --filter @xyd-js/native build:native`. " +
                "There is no JavaScript fallback: it was removed in favour of a single implementation."
        );
    }

    return function pluginJsonViewInner({ references, defer }: UniformPluginArgs) {
        defer(() => ({
            jsonViews: JSON.parse(
                native.pluginJsonView(
                    JSON.stringify(Array.isArray(references) ? references : [references])
                )
            ),
        }));

        return (_ref: Reference) => {};
    };
}
