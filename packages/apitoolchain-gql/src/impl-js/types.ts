import type {GraphQLInputObjectType, GraphQLInterfaceType, GraphQLNamedType, GraphQLObjectType} from "graphql";
import {GraphQLField, OperationTypeNode} from "graphql";

import type {DefinitionProperty} from "@xyd-js/uniform";

// The three public option types now live in ../types.ts — the module that
// outlives this one when impl-js is deleted. Re-exported here so the frozen
// converter keeps compiling against a SINGLE definition rather than a copy that
// could drift from the one the public API advertises.
export type { SortItem, OpenDocsSortConfig, GQLSchemaToReferencesOptions } from "../types";
import type { SortItem } from "../types";

export interface SortStack {
    sortStack?: string[][];
    sort?: SortItem[];
}

export const DEFAULT_SORT_ORDER: SortItem[] = [
    { node: "query" },
    { node: "mutation" },
    { node: "subscription" },
    { node: "object" },
    { node: "interface" },
    { node: "union" },
    { node: "input" },
    { node: "enum" },
    { node: "scalar" },
];

export type NestedGraphqlType = {
    __definitionProperties?: DefinitionProperty[];
} & (GraphQLObjectType | GraphQLInputObjectType | GraphQLInterfaceType)

// needed cuz GraphQLField does not have operation info?
export class GQLOperation implements GraphQLField<any, any> {
    public _operationType!: OperationTypeNode
    field: GraphQLField<any, any>;

    constructor(field: GraphQLField<any, any>) {
        this.field = field;
    }

    get name() {
        return this.field.name;
    }

    get description() {
        return this.field.description;
    }

    get type() {
        return this.field.type;
    }

    get args() {
        return this.field.args;
    }

    get deprecationReason() {
        return this.field.deprecationReason;
    }

    get extensions() {
        return this.field.extensions;
    }

    get astNode() {
        return this.field.astNode;
    }

    set __operationType(operationType: OperationTypeNode) {
        this._operationType = operationType;
    }
}

export interface GQLTypeInfo {
    typeFlat?: GraphQLNamedType
}

export interface FieldMetadata {
    path?: string;
    groups?: string[];
}

export interface GQLSchemaMetadata {
    fields: Map<string, FieldMetadata>;
    rootGroups?: string[];
}

