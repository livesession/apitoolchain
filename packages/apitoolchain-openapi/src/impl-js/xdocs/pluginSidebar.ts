import {OpenAPIV3} from "openapi-types";

import {
    CodeBlockTab,
    DefinitionProperty,
    Example,
    ExampleGroup,
    Reference,
    SymbolDef,
    UniformPluginArgs,
    OpenAPIReferenceContext
} from "@xyd-js/uniform";

import {XDocs, XDocsSidebar, XDocsPage, XDocsMeta, XDocsSchemaMeta} from "./types";
import {objectsGroupOf} from "../const";
import type {uniformOasOptions} from "../types";

interface NavigationGroup {
    id: string
    title: string
    beta: boolean
}

type ComponentMeta = XDocsSchemaMeta

type ExtensionSchema = OpenAPIV3.Document & {
    "x-docs"?: XDocs
}

type NavigationGroupMap = {
    [id: string]: NavigationGroup & {
        index: number
    }
}

interface OperationExample {
    request: string | { [lang: string]: string }
    response: string | { [lang: string]: string }
}

type Examples = string | OperationExample | OperationExample[]

export function uniformPluginXDocsSidebar({
                                              references,
                                              defer,
                                          }: UniformPluginArgs) {
    let schema: ExtensionSchema | undefined
    const refByOperationId: {
        [key: string]: Reference
    } = {}
    const refByComponentSchema: {
        [key: string]: Reference
    } = {}
    // `x-docs.hidden` refs (operations and component schemas alike) — dropped
    // from the GENERATED navigation in the defer below. An `x-docs.sidebar`
    // that lists one explicitly still shows it: the sidebar is a complete
    // enumeration and wins over the per-page flag.
    const hiddenRefs = new Set<Reference>()
    // The converter options travel on the reference list (`__internal_options`,
    // set by the converter and by the native shim alike): `objectsGroup` names
    // the sub-group an object page with a resource group is nested under.
    // @ts-ignore
    const options: uniformOasOptions | undefined = typeof references.__internal_options === "function"
        // @ts-ignore
        ? references.__internal_options()
        : undefined
    const objectsGroup = objectsGroupOf(options)

    defer(() => {
        if (!schema) {
            return {}
        }

        // @ts-ignore
        if (typeof references.__internal_options === "function") {
            // @ts-ignore
            const options = references.__internal_options()

            if (options?.regions?.length) {
                // A page's own rows: the per-page conversion covers one region,
                // so the navigation is not this pass's business — but the links
                // on the page still have to land on pages that exist, and a
                // one-page reference list cannot say which do. The document can.
                if (Array.isArray(references)) {
                    relinkSchemaLinks(references, schemaPageLinks(schema))
                }
                return {}
            }
        }

        const output: Reference[] = []
        const xDocs = schema["x-docs"]

        if (!xDocs?.sidebar) {
            // No sidebar: the generated navigation, minus the hidden refs.
            if (hiddenRefs.size && Array.isArray(references)) {
                const kept = references.filter(ref => !hiddenRefs.has(ref))
                references.length = 0
                references.push(...kept)
            }
            if (Array.isArray(references)) {
                relinkSchemaLinks(references, pageLinksOf(references))
            }
            return {}
        }

        const navigationMap: NavigationGroupMap = {}

        // Process sidebar groups
        for (let i = 0; i < xDocs.sidebar.length; i++) {
            const navGroup: XDocsSidebar = xDocs.sidebar[i]
            if (!navGroup) {
                continue
            }

            const uniqueKey = `${navGroup.group}-${i}`
            navigationMap[uniqueKey] = {
                id: navGroup.group,
                title: navGroup.group,
                beta: false,
                index: i
            }
        }

        // Process each group and its nested pages
        for (let i = 0; i < xDocs.sidebar.length; i++) {
            const group: XDocsSidebar = xDocs.sidebar[i]
            const uniqueKey = `${group.group}-${i}`
            const navGroup = navigationMap[uniqueKey]
            if (!navGroup) {
                console.warn(`No navigation group found for group: ${group.group}`)
                continue
            }

            if (!Array.isArray(group.pages)) {
                continue
            }

            // Process nested groups and pages
            // `?? ""` matches the Rust port (crates/apitoolchain_openapi/src/xdocs.rs:141,
            // `.unwrap_or("")`). Without it, an x-docs sidebar entry with `pages:` but no
            // `group:` put `undefined` into the group path; pluginNavigation uses that value
            // as an OBJECT KEY, and `Object.keys({[undefined]: 1})` is `["undefined"]` — the
            // STRING. A truthy string renders a real header, so the sidebar showed a group
            // literally titled "undefined" where a header-less separator was intended.
            // Reproducer: examples/openapi/todos-api.yaml:25.
            processGroupPages(xDocs, group.pages, [group.group ?? ""], navGroup, output)
        }

        // Clear references and set from output
        if (Array.isArray(references)) {
            references.length = 0
            references.push(...output)
            relinkSchemaLinks(references, pageLinksOf(references))
        } else {
            references = output[0] || references
        }

        return {}
    })

    function processGroupPages(
        xDocs: XDocs,
        pages: (XDocsSidebar | XDocsPage)[],
        groupPath: string[],
        navGroup: NavigationGroup & { index: number },
        output: Reference[],
        parentPath?: string
    ) {
        for (const page of pages) {
            if ('pages' in page && Array.isArray(page.pages)) {
                // This is a nested group
                processGroupPages(xDocs, page.pages, [...groupPath, page.group ?? ""], navGroup, output, page.path)
            } else if ('type' in page && 'key' in page) {
                // This is a page
                processPage(xDocs, page, groupPath, navGroup, output, parentPath)
            }
        }
    }

    function processPage(
        xDocs: XDocs,
        page: XDocsPage,
        groupPath: string[],
        navGroup: NavigationGroup & { index: number },
        output: Reference[],
        parentPath?: string
    ) {
        let uniformRef: Reference | undefined

        switch (page.type) {
            case "endpoint": {
                const operationRef = refByOperationId[page.key]
                if (!operationRef) {
                    console.warn(`No operation found for key: ${page.key} in group ${groupPath.join('/')}`)
                    return
                }
                uniformRef = operationRef
                break
            }

            case "object": {
                const componentRef = refByComponentSchema[page.key]
                if (!componentRef) {
                    console.warn(`No component schema found for key: ${page.key} in group ${groupPath.join('/')}`)
                    return
                }

                const selector = componentRef.__UNSAFE_selector
                if (!selector || typeof selector !== "function") {
                    return
                }

                const component = selector("[component]") as OpenAPIV3.SchemaObject | undefined
                if (!component) {
                    console.warn(`No component schema found for key: ${page.key} in group ${groupPath.join('/')}`)
                    return
                }

                // The same lookup as the generated navigation (on the schema
                // itself, else on the first `allOf` member that carries one),
                // so a listed page reads its `name` from the same place in
                // both modes. A page without any `x-docs` keeps its defaults
                // — the sidebar lists it, so it exists.
                const componentMeta: ComponentMeta | undefined = componentXDocs(component)

                uniformRef = componentRef
                if (!componentMeta) {
                    break
                }

                componentRef.title = componentMeta.name || componentRef.title

                if (componentMeta.example) {
                    const exampleGroups = oasXDocsExamples(componentMeta.example)
                    uniformRef.examples = {
                        groups: exampleGroups,
                    }
                }
                break
            }

            default: {
                console.warn(`Unknown page type: ${page.type} in group ${groupPath.join('/')}`)
                return
            }
        }

        if (!uniformRef) {
            return
        }

        const canonical = sidebarPageCanonical(
            xDocs,
            page,
            parentPath,
            (uniformRef.context as OpenAPIReferenceContext | undefined)?.path
        )
        if (canonical !== undefined) {
            uniformRef.canonical = canonical
        }

        if (!uniformRef.context) {
            uniformRef.context = {}
        }

        uniformRef.context.group = groupPath

        output.push(uniformRef)
    }

    return function pluginXDocsSidebarInner(ref: Reference) {
        // @ts-ignore
        const selector = ref.__UNSAFE_selector
        if (!selector || typeof selector !== "function") {
            return
        }

        const oapSchema = selector("[schema]")
        if (!oapSchema) {
            return
        }
        schema = oapSchema

        const ctx = ref.context as OpenAPIReferenceContext | undefined
        if (ctx?.componentSchema) {
            refByComponentSchema[ctx.componentSchema] = ref
            // The schema's own `x-docs` — honoured here, outside sidebar mode,
            // exactly like an operation's (`name` → title, `group` → sidebar
            // group, `hidden`). The sidebar pass re-reads `name` for the object
            // pages it lists.
            const meta = componentXDocs(selector("[component]") as OpenAPIV3.SchemaObject | undefined)
            if (meta && applyDocsMeta(ref, meta, objectsGroup)) {
                hiddenRefs.add(ref)
            }
        }

        const methodPath = selector("[method] [path]") as OpenAPIV3.OperationObject | undefined
        if (!methodPath) {
            return
        }

        const oapMethod = selector("[method]")
        if (!oapMethod) {
            return
        }

        const operationId = methodPath.operationId
        if (operationId) {
            refByOperationId[operationId] = ref
        }

        const methodId = (oapMethod?.httpMethod?.toUpperCase() + " " + oapMethod?.path || "").trim()
        if (methodId) {
            refByOperationId[methodId] = ref
        }

        const meta = (methodPath as any)["x-docs"]
        if (!meta) {
            return
        }

        if (applyDocsMeta(ref, meta, undefined)) {
            hiddenRefs.add(ref)
        }

        if (!ref.description) {
            ref.description = methodPath.summary || ""
        }

        if (meta.examples) {
            const exampleGroups = oasXDocsExamples(meta.examples)
            ref.examples = {
                groups: exampleGroups,
            }
        }

        if (meta.returns) {
            if (ref.definitions?.length) {
                ref.definitions[ref.definitions.length - 1] = {
                    title: ref.definitions[ref.definitions.length - 1].title,
                    description: meta.returns,
                    properties: []
                }
            } else {
                ref.definitions = [
                    {
                        title: "Response",
                        description: meta.returns,
                        properties: []
                    }
                ]
            }
        }
    }
}

/** The common `x-docs` keys of an operation or a component schema: `name` →
 *  the reference title, `group` → its sidebar group. A component schema's
 *  `group` is a RESOURCE group (the one its endpoints sit in), so its page is
 *  nested under `subGroup` inside it — `[group, "Objects"]` — after the
 *  operations, which the navigation lists before any sub-group; an operation
 *  (`subGroup` undefined) goes straight into `[group]`. Returns whether the ref
 *  is `hidden`. */
function applyDocsMeta(ref: Reference, meta: XDocsMeta, subGroup: string | undefined): boolean {
    if (meta.name) {
        ref.title = meta.name
    }

    if (meta.group) {
        if (ref.context) {
            ref.context.group = subGroup ? [meta.group, subGroup] : [meta.group]
        }
    }

    return !!meta.hidden
}

const COMPONENT_SCHEMA_PREFIX = "#/components/schemas/"

/** The canonical an `x-docs.sidebar` entry gives its page — the entry's
 *  `path` under the enclosing group's `path`; under
 *  `sidebarPathStrategy: inherit` the page's own path (an operation's) comes
 *  first — or `undefined` when the entry leaves the page where the converter
 *  put it. */
function sidebarPageCanonical(
    xDocs: XDocs,
    page: XDocsPage,
    parentPath: string | undefined,
    ownPath: string | undefined
): string | undefined {
    if (xDocs.sidebarPathStrategy === "inherit") {
        return joinPaths(parentPath || ownPath || "", page.path) || undefined
    }
    if (page.path) {
        return joinPaths(parentPath, page.path)
    }
    if (parentPath) {
        return parentPath
    }
    return undefined
}

/** What a nested link to a component schema becomes: the canonical of the
 *  schema's page, `null` when the schema has no page, `undefined` to leave the
 *  converter's link as it is. */
type PageLink = string | null | undefined

/** The page links once the reference list is FINAL (whole-document mode): a
 *  schema's surviving reference gives its canonical — so a sidebar that moves
 *  an object page under another path is followed — and a schema without one
 *  has no page any more (`x-docs.hidden`, or not listed by an
 *  `x-docs.sidebar`). */
function pageLinksOf(references: Reference[]): (name: string) => PageLink {
    const canonicalBySchema = new Map<string, string>()
    for (const ref of references) {
        const name = (ref.context as OpenAPIReferenceContext | undefined)?.componentSchema
        if (name && ref.canonical && !canonicalBySchema.has(name)) {
            canonicalBySchema.set(name, ref.canonical)
        }
    }
    return name => canonicalBySchema.get(name) ?? null
}

/** The page links read from the DOCUMENT (regions mode, where the reference
 *  list is one page long and cannot say which pages exist): without an
 *  `x-docs.sidebar` a schema has a page unless it is `x-docs.hidden`; with one,
 *  only the schemas it lists have a page, where the entry puts it. The same
 *  rules the whole-document pass applies through the surviving references. */
function schemaPageLinks(schema: ExtensionSchema): (name: string) => PageLink {
    const schemas = (schema.components?.schemas || {}) as Record<string, OpenAPIV3.SchemaObject>
    const xDocs = schema["x-docs"]
    const links = new Map<string, PageLink>()

    if (!xDocs?.sidebar) {
        for (const name of Object.keys(schemas)) {
            links.set(name, componentXDocs(schemas[name])?.hidden ? null : undefined)
        }
        return name => links.get(name)
    }

    for (const name of Object.keys(schemas)) {
        links.set(name, null)
    }
    const walk = (pages: (XDocsSidebar | XDocsPage)[], parentPath?: string) => {
        for (const page of pages) {
            if ('pages' in page && Array.isArray(page.pages)) {
                walk(page.pages, page.path)
            } else if ('type' in page && 'key' in page && page.type === "object" && page.key in schemas) {
                links.set(page.key, sidebarPageCanonical(xDocs, page, parentPath, undefined))
            }
        }
    }
    for (const group of xDocs.sidebar) {
        if (group && Array.isArray(group.pages)) {
            walk(group.pages)
        }
    }
    return name => links.get(name)
}

/** Schema links. A `symbolDef` whose `id` is a component schema
 *  (`#/components/schemas/<Name>`) — a property's, a request/response root's
 *  (the variant's) or a component page's own — takes that schema's page link:
 *  the page's canonical, or no `canonical` at all when the schema has no page —
 *  a link must never land on a page that does not exist. The `id` and the
 *  `label` stay either way. */
function relinkSchemaLinks(references: Reference[], pageLink: (name: string) => PageLink) {
    const relink = (symbolDef: SymbolDef | undefined) => {
        const id = symbolDef?.id
        if (symbolDef && typeof id === "string" && id.startsWith(COMPONENT_SCHEMA_PREFIX)) {
            const target = pageLink(id.slice(COMPONENT_SCHEMA_PREFIX.length))
            if (typeof target === "string") {
                symbolDef.canonical = target
            } else if (target === null) {
                delete symbolDef.canonical
            }
        }
    }
    const seen = new Set<DefinitionProperty>()
    const visit = (property: DefinitionProperty | undefined) => {
        if (!property || typeof property !== "object" || seen.has(property)) {
            return
        }
        seen.add(property)
        relink(property.symbolDef)
        property.properties?.forEach(visit)
        visit(property.ofProperty)
    }

    for (const ref of references) {
        for (const definition of ref.definitions || []) {
            relink(definition.symbolDef)
            definition.properties?.forEach(visit)
            visit(definition.rootProperty)
            for (const variant of definition.variants || []) {
                relink(variant.symbolDef)
                variant.properties?.forEach(visit)
                visit(variant.rootProperty)
            }
        }
    }
}

/** A component schema's `x-docs`: on the schema itself, else on the first
 *  `allOf` member that carries one (a composed model may document itself on a
 *  member). */
function componentXDocs(component: OpenAPIV3.SchemaObject | undefined): XDocsSchemaMeta | undefined {
    if (!component) {
        return undefined
    }
    const own = (component as any)["x-docs"] as XDocsSchemaMeta | undefined
    if (own) {
        return own
    }
    for (const item of component.allOf || []) {
        const docsMeta = (item as any)["x-docs"] as XDocsSchemaMeta | undefined
        if (docsMeta) {
            return docsMeta
        }
    }
    return undefined
}

function oasXDocsExamples(examples: Examples) {
    const groups: ExampleGroup[] = []

    if (examples) {
        if (Array.isArray(examples)) {
            // Create request group
            const requestExamples: Example[] = []
            examples.forEach((example: {
                title?: string;
                request?: string | Record<string, string>;
                response?: string | Record<string, string>;
            }) => {
                if (example.request) {
                    const tabs: CodeBlockTab[] = []
                    if (typeof example.request === "string") {
                        tabs.push({
                            title: "",
                            language: "json",
                            code: example.request
                        })
                    } else {
                        for (let lang of Object.keys(example.request)) {
                            const code = example.request[lang] || ""
                            const language = lang === "curl" ? "bash" :
                                lang === "node.js" ? "js" : lang

                            tabs.push({
                                title: lang,
                                language,
                                code
                            })
                        }
                    }
                    if (tabs.length > 0) {
                        requestExamples.push({
                            description: example.title || "",
                            codeblock: {
                                title: example.title || "",
                                tabs
                            }
                        })
                    }
                }
            })
            if (requestExamples.length > 0) {
                groups.push({
                    description: "Example request",
                    examples: requestExamples
                })
            }

            // Create response group
            const responseExamples: Example[] = []
            examples.forEach((example: {
                title?: string;
                request?: string | Record<string, string>;
                response?: string | Record<string, string>;
            }) => {
                if (example.response) {
                    const tabs: CodeBlockTab[] = []
                    if (typeof example.response === "string") {
                        tabs.push({
                            title: "",
                            language: "json",
                            code: example.response
                        })
                    } else {
                        for (let lang of Object.keys(example.response)) {
                            const code = example.response[lang] || ""
                            const language = lang === "curl" ? "bash" :
                                lang === "node.js" ? "js" : lang

                            tabs.push({
                                title: lang,
                                language,
                                code
                            })
                        }
                    }
                    if (tabs.length > 0) {
                        responseExamples.push({
                            description: example.title || "",
                            codeblock: {
                                title: example.title || "",
                                tabs
                            }
                        })
                    }
                }
            })
            if (responseExamples.length > 0) {
                groups.push({
                    description: "Example response",
                    examples: responseExamples
                })
            }
        } else {
            if (typeof examples === "string") {
                groups.push({
                    description: "Example",
                    examples: [
                        {
                            description: "",
                            codeblock: {
                                tabs: [
                                    {
                                        title: "",
                                        language: "json",
                                        code: examples
                                    }
                                ]
                            }
                        }
                    ]
                })
            } else {
                if (examples.request) {
                    const tabs: CodeBlockTab[] = []

                    if (typeof examples.request === "string") {
                        tabs.push({
                            title: "",
                            language: "json",
                            code: examples.request || "",
                        })
                    } else {
                        for (let lang of Object.keys(examples.request)) {
                            const code = examples.request[lang] || ""

                            switch (lang) {
                                case "curl":
                                    lang = "bash"
                                    break
                                case "node.js":
                                    lang = "js"
                                    break
                                default:
                                    break
                            }

                            tabs.push({
                                title: lang,
                                language: lang,
                                code,
                            })
                        }
                    }

                    groups.push({
                        description: "Example request",
                        examples: [
                            {
                                description: "",
                                codeblock: {
                                    tabs
                                }
                            }
                        ]
                    })
                }

                if (examples.response) {
                    const tabs: CodeBlockTab[] = []
                    if (typeof examples.response === "string") {
                        tabs.push({
                            title: "",
                            language: "json",
                            code: examples.response || "",
                        })
                    } else {
                        for (let lang of Object.keys(examples.response)) {
                            const code = examples.response[lang] || ""

                            switch (lang) {
                                case "curl":
                                    lang = "bash"
                                    break
                                case "node.js":
                                    lang = "js"
                                    break
                                default:
                                    break
                            }

                            tabs.push({
                                title: lang,
                                language: lang,
                                code,
                            })
                        }
                    }

                    groups.push({
                        description: "Example response",
                        examples: [
                            {
                                description: "",
                                codeblock: {
                                    tabs
                                }
                            }
                        ]
                    })
                }
            }
        }
    }

    return groups
}

function sanitizePath(path: string): string {
    // Remove path parameters like {userId} or :userId
    return path.replace(/\/\{[^}]+\}/g, '').replace(/\/:[^/]+/g, '')
}

function joinPaths(...paths: (string | undefined)[]): string {
    return paths
        .filter(Boolean)
        .map(path => {
            // Remove leading and trailing slashes
            path = path!.replace(/^\/+|\/+$/g, '')
            // Ensure path starts with / if it's not empty
            return path ? `/${path}` : ''
        })
        .join('')
        .replace(/\/+/g, '/') // Replace multiple slashes with single slash
        .replace(/\/\{[^}]+\}/g, '') // Remove {param} segments
        .replace(/\/:[^/]+/g, '') // Remove :param segments
}

