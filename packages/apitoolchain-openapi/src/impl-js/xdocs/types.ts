export interface XDocs {
    route?: string
    
    codeLanguages?: string[]

    sidebarPathStrategy?: "inherit"

    sidebar?: XDocsSidebar[]
}

// TODO: move to core settings like SidebarLite<XDocsSidebar>
export interface XDocsSidebar {
    group: string

    path?: string

    pages: XDocsSidebar[] | XDocsPage[]
}

export interface XDocsPage {
    type: "endpoint" | "object"

    key: string

    path?: string
}

/** `x-docs` on an operation or on a component schema — honoured with or
 *  without an `x-docs.sidebar`. */
export interface XDocsMeta {
    /** The reference title (the page title and sidebar label) instead of the
     *  operation summary / the schema name, e.g. `"The widget object"`. */
    name?: string

    /** The sidebar group instead of the operation's first tag / `"Objects"` for
     *  a schema — what puts an object page beside its resource's endpoints. */
    group?: string

    /** Drop the page from the generated navigation (envelopes, errors, ids).
     *  An `x-docs.sidebar` that lists the page explicitly still shows it. */
    hidden?: boolean
}

/** `x-docs` on an operation. */
export interface XDocsOperationMeta extends XDocsMeta {
    /** Replaces the response definition with this description. */
    returns?: string

    /** Request / response code samples. */
    examples?: unknown
}

/** `x-docs` on a component schema — on the schema itself, or (for an `allOf`
 *  composition) on one of its members. */
export interface XDocsSchemaMeta extends XDocsMeta {
    /** A JSON example shown on the object page (sidebar mode). */
    example?: string
}
