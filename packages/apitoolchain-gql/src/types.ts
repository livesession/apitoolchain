// The PUBLIC option types of @xyd-js/gql.
//
// They live here rather than in `src/impl-js/types.ts` because they outlive it:
// impl-js is the frozen TypeScript converter, deleted once @xyd-js/native ships
// platform binaries, while these types describe the package's API in both modes
// and are referenced by the native branch of `src/index.ts`.
//
// Note `__tests__/utils.ts` has imported `../src/types` since before this file
// existed — the import survived only because it is type-only and therefore
// erased at compile time, so nothing ever resolved it. Creating this module
// fixes that latent break rather than introducing a new dependency.
//
// Deliberately NOT re-exported from `./impl-js/types`: that would point the
// public surface back at the module being deleted.

/** One entry in a sort order — match a node kind, a group path, or a stack index. */
export interface SortItem {
    node?: string;
    group?: string[];
    stack?: number;
}

/** Ordering configuration for the generated reference tree. */
export interface OpenDocsSortConfig {
    sortStack?: string[][];
    sort?: SortItem[];
}

/** Options accepted by `gqlSchemaToReferences`. */
export interface GQLSchemaToReferencesOptions {
    // TODO: support line ranged in the future?
    regions?: string[] // TODO: BETTER API - UNIFY FOR REST API / GRAPHQL ETC

    flat?: boolean;
    sort?: OpenDocsSortConfig;
    route?: string
}
