// Was `@graphql-markdown/types` — a package declared in no package.json or
// lockfile in either repo. It resolved only because the import is type-only and
// therefore erased before any resolver ran. `GraphQLScalarType` is a standard
// export of `graphql`, which IS a declared dependency.
import type {GraphQLScalarType} from "graphql";

import type {Reference} from "@xyd-js/uniform";

import {uniformify} from "../gql-core";
import {Context} from "../context";

// gqlScalarToUniformRef is a helper function to convert a GraphQL scalar type into a 'uniform' reference.
export function gqlScalarToUniformRef(ctx: Context, gqlType: GraphQLScalarType): Reference {
    return uniformify(
        ctx,
        gqlType,
        [],
        []
    )
}
