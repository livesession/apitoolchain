import { defineConfig } from 'vitest/config'

// Mirrors xyd's root vitest config for the five converter shims that moved here,
// so the same 11 test files run the same way on this side of the split.
export default defineConfig({
    test: {
        globals: true,
        environment: 'node',
        include: [
            'packages/**/*.test.ts',
            'packages/**/__tests__/**/*.test.ts'
        ],
        exclude: [
            // Fixture trees contain committed *.test.ts ARTIFACTS (generated SDK
            // goldens). They are data, not repo tests — never collect them.
            '**/__fixtures__/**',
            // The twelve apitoolchain packages and the five apps are standalone
            // bun islands with their own runners and their own node_modules
            // (bun:test, or a local vitest resolving deps this workspace does not
            // install). They were excluded from xyd's root vitest for exactly the
            // same reason; each is run by its own CI job.
            'packages/api-cli/**',
            'packages/apitoolchainapp-api-node/**',
            'packages/apitoolchainapp-auth-design-system/**',
            'packages/apitoolchainapp-design-system/**',
            'packages/apitoolchainapp-dev/**',
            'packages/apitoolchainapp-filters/**',
            'packages/apitoolchainapp-gitprovider-node/**',
            'packages/apitoolchainapp-registry-api-node/**',
            'packages/apitoolchainapp-release-man/**',
            'packages/apitoolchainapp-schemas/**',
            'packages/apitoolchainapp-sdk-chain/**',
            'packages/apitoolchainapp-sdkjson-wizard/**',
            'apps/**',
            '**/node_modules/**',
            '**/dist/**',
            '**/build/**'
        ]
    },
    plugins: [
        {
            name: 'graphql-raw',
            transform(code, id) {
                if (id.endsWith('.graphql')) {
                    return {
                        code: `export default ${JSON.stringify(code)};`,
                        map: null
                    }
                }
            }
        }
    ]
})
