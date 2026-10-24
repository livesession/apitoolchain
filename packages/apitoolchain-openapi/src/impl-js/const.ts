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

/** The default name of the group the component-schema pages sit in: the flat
 *  top-level group of the schemas with no resource group, and the sub-group
 *  nested under a resource group (`x-docs.group`) for the schemas that have
 *  one. Renamed with `uniformOasOptions.objectsGroup`. */
export const DEFAULT_OBJECTS_GROUP = "Objects"

/** The effective component-schema group name for a set of options. */
export function objectsGroupOf(options: { objectsGroup?: string } | undefined): string {
    return options?.objectsGroup || DEFAULT_OBJECTS_GROUP
}
