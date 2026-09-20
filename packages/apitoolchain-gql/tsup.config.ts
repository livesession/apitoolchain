
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
            '.graphql': 'text' // Load .graphql files as text
        };
    },
    // No onSuccess hook. It used to copy src/impl-js/opendocs.graphql to
    // dist/opendocs.graphql, but NOTHING in either repo reads that output — the
    // file is consumed at BUILD time by the `.graphql: text` loader above, via
    // `import openDocsSchemaRaw from './opendocs.graphql'` (src/impl-js/schema.ts:33),
    // so its contents are already inlined into the bundle. The loader stays; the
    // copy was dead weight that also pinned a path inside impl-js.
});