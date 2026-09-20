// Dispatcher (S6+ W3): the group-tree/sidebar core runs in Rust
// (crates/apitoolchain_uniform::plugins::plugin_navigation) when the native core is
// present. Only the `engine.uniform.store` flag from settings is forwarded —
// live Settings objects can carry non-serializable values (docs.tsx), and the
// Rust core reads nothing else.
import type { Settings, Sidebar, MetadataMap } from "@xyd-js/core";

import type { UniformPluginArgs, UniformPlugin } from "../index";
import type { Reference } from "../types";
import { native } from "../native";
// The options type outlived the frozen implementation it used to live beside,
// so it moves here rather than dying with it — it describes this package's
// public API in the one mode that remains.
export interface pluginNavigationOptions {
    urlPrefix: string
    defaultGroup?: string
}

type pluginNavigationOutput = {
    pageFrontMatter: MetadataMap;
    sidebar: Sidebar[];
};

export function pluginNavigation(
    settings: Settings,
    options: pluginNavigationOptions
): UniformPlugin<pluginNavigationOutput> {
    if (!native?.pluginNavigation) {
        throw new Error(
            "@xyd-js/uniform.pluginNavigation requires @xyd-js/native, which did not load. " +
                "Install it (it is an optionalDependency, so a failed install is silent) " +
                "or build it locally with `pnpm --filter @xyd-js/native build:native`. " +
                "There is no JavaScript fallback: it was removed in favour of a single implementation."
        );
    }

    return function pluginNavigationInner({ references, defer }: UniformPluginArgs) {
        defer(() =>
            JSON.parse(
                native.pluginNavigation(
                    JSON.stringify({
                        settings: {
                            engine: {
                                uniform: {
                                    store: !!settings?.engine?.uniform?.store,
                                },
                            },
                        },
                        urlPrefix: options.urlPrefix,
                        references: Array.isArray(references) ? references : [references],
                    })
                )
            )
        );

        return (_ref: Reference) => {};
    };
}
