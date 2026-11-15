import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, it } from 'vitest';

import { type OpencliSpecJson, opencliToReferences } from '../src';
import { opencliToReferencesJs } from '../src/converters';
import { nativeOpencliToReferences } from '../src/native';

// The `opencliToReferences` oracle.
//
// This converter turns an OpenCLI document into uniform `Reference[]` and runs
// on the PAGE-COMPILE HOT PATH — `uniformProcessor.ts` and
// `presets/cli/index.ts` call it for every CLI docs page. It has two
// implementations: the TypeScript in src/converters.ts (`opencliToReferencesJs`)
// and its Rust twin, `crates/apitoolchain_opencli_uniform`. The goldens below
// pin both: this file the TypeScript, the crate's tests/references.rs the Rust,
// over the same files.
//
// The corpus is deliberately SELF-CONTAINED (inputs copied in, not referenced
// where they live). `xyd_opencli_uniform` stays in xyd while
// `xyd-openapi2opencli` and `xyd-opencli2go` move to the opensdk repo — reading
// their fixtures from here would turn into a cross-REPO path the moment the
// split lands.
//
// Inputs:
//   1-5    openapi2opencli-derived docs: arguments, subcommands, body flatten,
//          and `x-openapi.responses` (the "Example response" group)
//   6      the real hand-authored xyd CLI: 6 recursive root options (the global
//          -options branch) plus nested subcommands
//   7.*    a deterministic slice of the real OpenAI-derived corpus
//   8      a host CLI ("acme") with an agent component mounted as `agent` by
//          `opensdk opencli --mount agent=<doc> --mount-drop agent:profile`:
//          exit codes (own, inherited, an empty list falling back), examples,
//          x-interactive, x-environment, x-passthrough, a runnable command
//          group (`[command]`), and recursive options of a non-root command
//          reaching its descendants
//   9      that component's own document: the root's own page
//
// Both `globalOptionsPerCommand` modes are captured because the flag materially
// reshapes the result: false (default) appends ONE extra `global-options`
// reference; true instead adds a "Global options" definition to every command
// and emits no extra reference.
//
// Regenerate with `XYD_NATIVE=0 O2S_BUILD_DOCS=1`: the goldens come from the
// TypeScript twin, never from a native addon.

const FIXTURES = path.join(__dirname, '../__fixtures__/references');
const BUILD = process.env.O2S_BUILD_DOCS === '1';

const MODES = [
  { file: 'references.json', options: {} },
  { file: 'references.per-command.json', options: { globalOptionsPerCommand: true } },
] as const;

const cases = fs.existsSync(FIXTURES)
  ? fs
      .readdirSync(FIXTURES)
      .sort()
      .filter((d) => fs.existsSync(path.join(FIXTURES, d, 'input.json')))
  : [];

function input(name: string): OpencliSpecJson {
  return JSON.parse(fs.readFileSync(path.join(FIXTURES, name, 'input.json'), 'utf8')) as OpencliSpecJson;
}

describe.runIf(BUILD && cases.length > 0)('generate opencliToReferences goldens', () => {
  it('writes references.json + references.per-command.json for every case', () => {
    for (const name of cases) {
      for (const mode of MODES) {
        const refs = opencliToReferencesJs(input(name), mode.options);
        fs.writeFileSync(path.join(FIXTURES, name, mode.file), `${JSON.stringify(refs, null, 2)}\n`);
      }
    }
    expect(cases.length).toBeGreaterThan(0);
  });
});

// The corpus size is ASSERTED, not merely enumerated. `skipIf(cases.length === 0)`
// meant a missing or relocated __fixtures__/references made the whole suite
// vanish and still report green — the failure mode this file exists to prevent.
// Rust's crates/apitoolchain_opencli_uniform/tests/references.rs asserts the same 19
// against the same corpus; keep the two numbers in step.
const EXPECTED_CASES = 19;

describe('opencliToReferences corpus', () => {
  it(`discovers all ${EXPECTED_CASES} committed fixture cases`, () => {
    expect({ count: cases.length, dir: FIXTURES }).toEqual({ count: EXPECTED_CASES, dir: FIXTURES });
  });
});

function golden(name: string, file: string): unknown {
  const goldenPath = path.join(FIXTURES, name, file);
  // Was `it.skipIf(!fs.existsSync(goldenPath))`: a golden that failed to land
  // skipped its own case silently. Every golden is committed, so a missing one
  // is damage, not a legitimate gap — assert it.
  expect(fs.existsSync(goldenPath), `missing golden ${goldenPath}`).toBe(true);
  return JSON.parse(fs.readFileSync(goldenPath, 'utf8'));
}

// Tier 1: the TypeScript twin itself, whatever native addon resolves.
describe.skipIf(BUILD)('opencliToReferences — the TypeScript twin', () => {
  for (const name of cases) {
    for (const mode of MODES) {
      it(`${name} — ${mode.file}`, () => {
        expect(opencliToReferencesJs(input(name), mode.options)).toEqual(golden(name, mode.file));
      });
    }
  }
});

// The cases whose features (exit codes, examples, sessions, environment,
// inherited options, a runnable group, the root's page) a published
// @xyd-js/native built from an older core cannot produce.
const SECTIONS_CASES = new Set(['8.acme-mounted-agent', '9.acme-agent-standalone']);

// A feature probe, not a version pin: it lifts by itself the moment an addon
// that renders every one of those features resolves (one carrying only some of
// them stays gated). Without an addon the entry runs the TypeScript twin, so
// nothing is gated.
const nativeHasSections = (() => {
  const nativeFn = nativeOpencliToReferences();
  if (!nativeFn) return true;
  try {
    const doc = {
      opencli: '1.0.0',
      info: { title: 'demo', version: '1.0.0' },
      arguments: [{ name: 'prompt' }],
      exitCodes: [{ code: 3, description: 'x' }],
      commands: [
        {
          name: 'group',
          options: [{ name: 'model', recursive: true }],
          examples: ['demo group'],
          'x-interactive': { commands: [{ name: 'clear' }], keys: [{ keys: ['esc'], description: 'x' }] },
          'x-environment': [{ name: 'DEMO_HOME', description: 'x' }],
          commands: [{ name: 'leaf' }],
        },
      ],
    };
    const refs: any[] = JSON.parse(nativeFn(JSON.stringify(doc), '{}'));
    const titles = (r: any) => (r?.definitions ?? []).map((d: any) => d.title);
    const root = refs.find((r) => r.canonical === 'demo');
    const group = refs.find((r) => r.canonical === 'group');
    const leaf = refs.find((r) => r.canonical === 'group/leaf');
    return (
      !!root &&
      group?.title === 'group [command]' &&
      ['Interactive commands', 'Keys', 'Environment', 'Exit codes'].every((t) => titles(group).includes(t)) &&
      group?.examples?.groups?.[1]?.description === 'Examples' &&
      titles(leaf).includes('Inherited options') &&
      titles(leaf).includes('Exit codes')
    );
  } catch {
    return false;
  }
})();

// Tier 2: the package entry, which prefers the native addon.
describe.skipIf(BUILD)('opencliToReferences — the package entry', () => {
  for (const name of cases) {
    for (const mode of MODES) {
      const gated = SECTIONS_CASES.has(name) && !nativeHasSections;
      (gated ? it.skip : it)(`${name} — ${mode.file}`, () => {
        expect(opencliToReferences(input(name), mode.options)).toEqual(golden(name, mode.file));
      });
    }
  }
});
