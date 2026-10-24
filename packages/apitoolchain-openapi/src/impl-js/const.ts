import type {ObjectsLayout} from "./types";

export const SUPPORTED_HTTP_METHODS: string[] = [
    'get',
    'put',
    'patch',
    'post',
    'delete',
    // 'options',
    // 'head',
    // 'trace'
];

export const BUILT_IN_PROPERTIES: { [key: string]: boolean } = {
    "__internal_getRefPath": true,
}

/** The default name of the group the component-schema pages sit in: the
 *  top-level objects group, and under the `per-area` layout the sub-group
 *  nested under a resource group (`x-docs.group`) for the schemas that have
 *  one. Renamed with `uniformOasOptions.objectsGroup`. */
export const DEFAULT_OBJECTS_GROUP = "Objects"

/** The effective component-schema group name for a set of options. */
export function objectsGroupOf(options: { objectsGroup?: string } | undefined): string {
    return options?.objectsGroup || DEFAULT_OBJECTS_GROUP
}

/** The default `uniformOasOptions.objectsLayout`: one top-level objects group,
 *  the last of the navigation, with a sub-group per resource group. */
export const DEFAULT_OBJECTS_LAYOUT: ObjectsLayout = "grouped"

/** The effective objects layout for a set of options — anything but
 *  `"per-area"` is the default. */
export function objectsLayoutOf(options: { objectsLayout?: string } | undefined): ObjectsLayout {
    return options?.objectsLayout === "per-area" ? "per-area" : DEFAULT_OBJECTS_LAYOUT
}
