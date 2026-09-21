
import {defineConfig} from 'tsup';

export default defineConfig({
    entry: ['index.ts'],
    format: ['esm'], // Output both ESM and CJS formats
    target: 'node16', // Ensure compatibility with Node.js 16
    dts: {
        entry: 'index.ts', // Specify the entry for DTS
        resolve: true, // Resolve external types
    },
    splitting: false, // Disable code splitting
    sourcemap: true, // Generate source maps
    clean: true, // Clean the output directory before each build
    esbuildOptions: (options) => {
        options.platform = 'node'; // Ensure the platform is set to Node.js
        options.external = ['node:fs/promises']; // Mark 'node:fs/promises' as external
        options.loader = {
            '.js': 'jsx', // Ensure proper handling of .js files
            // No '.graphql' loader: the only .graphql import lived in
            // src/impl-js/schema.ts, deleted with the frozen implementation.
            // The Rust core carries its own copy (crates/apitoolchain_gql/src/opendocs.rs).
        };
    },
    // No onSuccess hook. It used to copy src/impl-js/opendocs.graphql to
    // dist/opendocs.graphql — an output nothing in either repo ever read. It
    // and the `.graphql` loader above went together: the loader's only consumer
    // was src/impl-js/schema.ts, which is gone, so this package now has no
    // .graphql scaffolding at all. The schema itself did not disappear — the
    // Rust core carries its own copy.
});