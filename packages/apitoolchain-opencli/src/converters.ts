import type { EnvVar, InteractiveCommand, KeyBinding, XInteractive } from './extensions';
import { generateUsage } from './generate';
import { nativeOpencliToReferences } from './native';
import type { Argument, Command, ExitCode, OpencliSpecJson, Option } from './types';

// Structural subset of @xyd-js/uniform's `Reference` — kept local so the OpenCLI
// core stays free of the (React-typed) uniform package. The docs engine consumes
// these as uniform References (api.cli → Atlas).
interface OpencliRefProperty {
  name: string;
  type: string;
  description: string;
  meta?: { name: string; value: unknown }[];
  // nested rows: an interactive command's arguments, options and subcommands
  properties?: OpencliRefProperty[];
}
interface OpencliRefDefinition {
  title: string;
  properties: OpencliRefProperty[];
}
interface OpencliRefCodeTab {
  title: string;
  code: string;
  language: string;
}
interface OpencliRefExampleGroup {
  description?: string;
  examples: { description?: string; codeblock: { title?: string; tabs: OpencliRefCodeTab[] } }[];
}
export interface OpencliReference {
  title: string;
  canonical: string;
  description: string;
  // Marks these as CLI references so the docs engine / Atlas can render the
  // usage-formula header (mirrors an OpenAPI page's method + path).
  category: 'cli';
  // `group` drives sidebar placement (the docs engine groups references by it);
  // `path` is the region key round-tripped through the page frontmatter.
  context: { path: string; fullPath: string; group: string[] };
  // a runnable CLI invocation, rendered by Atlas as a code sample (like the
  // request/response samples on an OpenAPI page).
  examples: { groups: OpencliRefExampleGroup[] };
  definitions: OpencliRefDefinition[];
}

export interface OpencliToReferencesOptions {
  /**
   * Restrict output to specific commands. Each region is a command path,
   * space-joined from the CLI root (e.g. `"install"`, `"remote add"`). The
   * root's own page, when there is one, is its id: the CLI's title as a URL
   * path segment (`"acme"`, or `"acme-agent"` for the title `Acme Agent`),
   * suffixed `-1`, `-2`, … while a top-level command or the Global options
   * page answers it.
   */
  regions?: string[];

  /**
   * Render the CLI's global (root recursive) options on every command page.
   * When false (the default), a single "Global options" reference is emitted
   * instead — so the options appear once in the sidebar rather than repeated
   * on every command.
   */
  globalOptionsPerCommand?: boolean;
}

const GLOBAL_OPTIONS_REGION = 'global-options';
const REQUIRED_SUBCOMMAND = '<command>';
const OPTIONAL_SUBCOMMAND = '[command]';

/**
 * Convert an OpenCLI document to uniform `Reference[]` — one Reference per
 * command — so the docs engine can render CLI reference pages the same way it
 * renders OpenAPI/GraphQL (`api.cli`). Mirrors `oapSchemaToReferences`.
 */
export function opencliToReferences(
  spec: OpencliSpecJson | null | undefined,
  options: OpencliToReferencesOptions = {},
): OpencliReference[] {
  if (!spec) return [];

  // Native fast path — byte-identical to the TypeScript below, gated by the
  // same goldens in both globalOptionsPerCommand modes. This function is on the
  // page-compile hot path: one call per CLI docs page.
  const nativeFn = nativeOpencliToReferences();
  if (nativeFn) {
    try {
      return JSON.parse(nativeFn(JSON.stringify(spec), JSON.stringify(options))) as OpencliReference[];
    } catch {
      // A malformed doc the native side rejects falls through to the JS impl,
      // which has always been lenient about partial specs.
    }
  }

  return opencliToReferencesJs(spec, options);
}

/**
 * The TypeScript implementation, without the native fast path. It is the twin
 * of `crates/apitoolchain_opencli_uniform`: the goldens pin both. Not part of
 * the package entry; tests import it from here.
 *
 * What a page carries beyond a command's own arguments, options and
 * subcommands (definition order: Arguments, Options, Inherited options,
 * Commands, Global options, Interactive commands, Keys, Environment, Exit codes):
 * - "Inherited options": the recursive options of its ancestor COMMANDS,
 *   outermost first. A name resolves to its nearest declaration (the
 *   command's own, else the nearest ancestor's, recursive or not), and only a
 *   recursive, visible one is inherited. The root's are the global options,
 *   not these; per command, a global option a nearer declaration shadows is
 *   left out.
 * - "Interactive commands" and "Keys", from `x-interactive`: the commands typed
 *   inside the session the command opens (each with its arguments and options
 *   as nested rows; never a page of their own) and its key bindings.
 * - "Environment", from `x-environment`.
 * - "Exit codes": the command's own `exitCodes`, else its nearest ancestor's,
 *   the root's last.
 * - Its `examples`, as an "Examples" sample group after the "CLI Tool" one.
 *
 * A command group with arguments or options of its own also runs without a
 * subcommand: `[command]`, not `<command>`, and its CLI Tool sample leaves the
 * placeholder out. `x-passthrough` changes nothing: the arguments, options and
 * subcommands under it document the program and render like any command's.
 *
 * The root gets a page of its own, ahead of the commands, only when it
 * documents something of its own: arguments, examples, a session or
 * environment variables. Its region and canonical are its id (`rootPageId`),
 * the CLI's title as a URL path segment that no other page answers; its title
 * and usage keep the title itself. Its options that are not global render
 * there too. Options or exit codes alone do not make one.
 */
export function opencliToReferencesJs(
  spec: OpencliSpecJson | null | undefined,
  options: OpencliToReferencesOptions = {},
): OpencliReference[] {
  if (!spec) return [];

  const cliTitle = spec.info?.title || 'cli';
  const regionSet = options.regions?.length ? new Set(options.regions) : null;
  const perCommand = options.globalOptionsPerCommand ?? false;
  const refs: OpencliReference[] = [];

  // Root-level recursive options (e.g. a CLI's global flags) apply to every
  // command. By default they get their own page; with `globalOptionsPerCommand`
  // they're rendered as a "Global options" section on each command instead.
  const globalOptions = (spec.options || []).filter((o) => o.recursive && !o.hidden);
  const perCommandGlobals = perCommand ? globalOptions : [];

  // The CLI's own page, ahead of its commands — only when the root documents
  // something of its own.
  const rootId = rootHasOwnPage(spec) ? rootPageId(spec, cliTitle) : null;
  if (rootId && (!regionSet || regionSet.has(rootId))) {
    refs.push(
      pageReference({ cmd: spec, path: [], ancestors: [], doc: spec, isRoot: true, rootId }, cliTitle, perCommandGlobals),
    );
  }

  const walk = (commands: Command[] | undefined, parentPath: string[], ancestors: Command[]) => {
    for (const cmd of commands || []) {
      if (cmd.hidden) continue;
      const cmdPath = [...parentPath, cmd.name];
      const region = cmdPath.join(' ');
      if (!regionSet || regionSet.has(region)) {
        refs.push(pageReference({ cmd, path: cmdPath, ancestors, doc: spec, isRoot: false }, cliTitle, perCommandGlobals));
      }
      if (cmd.commands?.length) walk(cmd.commands, cmdPath, [...ancestors, cmd]);
    }
  };
  walk(spec.commands, [], []);

  // Default: the global options live on a single dedicated page in the sidebar.
  if (!perCommand && globalOptions.length && (!regionSet || regionSet.has(GLOBAL_OPTIONS_REGION))) {
    refs.push(globalOptionsReference(globalOptions));
  }

  return refs;
}

/**
 * Whether the root documents something of its own, which earns it a page:
 * arguments, examples, a session or environment variables. Options alone do
 * not (a root `--version` or `--help` keeps a document's shape); its non-global
 * ones render on the page once it exists.
 */
function rootHasOwnPage(doc: OpencliSpecJson): boolean {
  const session = doc['x-interactive'] as XInteractive | undefined;
  return (
    visible(doc.arguments).length > 0 ||
    exampleLines(doc).length > 0 ||
    visible(session?.commands).length > 0 ||
    visible(session?.keys).length > 0 ||
    visible(doc['x-environment'] as EnvVar[] | undefined).length > 0
  );
}

/**
 * The id of the root's page: its region and its canonical, so a URL path
 * segment. The CLI's title is free text (`Acme Agent`, `@acme/agent`), and a
 * page's canonical becomes its route, its `.md` twin and its links, so the id
 * is the title with ASCII letters lowercased and every run of anything but
 * `a-z`, `0-9`, `_` and `-` one `-`, trimmed of `-`; `cli` when nothing is left.
 *
 * No other page may answer it, or a page asking for it would render both and
 * the sidebar would lose one. A one-segment id can only meet a top-level
 * command (its region and canonical are its name) or the Global options page,
 * so while one of those has it, the id takes the first free `-1`, `-2`, …
 * suffix. The Global options id is kept free in either mode, so the root's id
 * does not depend on `globalOptionsPerCommand`.
 */
function rootPageId(doc: OpencliSpecJson, cliTitle: string): string {
  const base = pathSegment(String(cliTitle)) || 'cli';
  const taken = new Set<string>([GLOBAL_OPTIONS_REGION]);
  for (const cmd of doc.commands || []) {
    if (!cmd.hidden) taken.add(str(cmd.name));
  }

  let id = base;
  for (let n = 1; taken.has(id); n++) id = `${base}-${n}`;
  return id;
}

function pathSegment(text: string): string {
  return text
    .replace(/[A-Z]/g, (c) => c.toLowerCase())
    .replace(/[^a-z0-9_-]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

function globalOptionsReference(globalOptions: Option[]): OpencliReference {
  return {
    title: 'Global options',
    canonical: GLOBAL_OPTIONS_REGION,
    description: 'Options available on every command.',
    category: 'cli',
    context: { path: GLOBAL_OPTIONS_REGION, fullPath: '', group: ['Global options'] },
    examples: { groups: [] },
    definitions: [{ title: 'Global options', properties: globalOptions.map(optionToProperty) }],
  };
}

/** What one page documents: a command, or the document root. */
interface Page {
  /** The command, or the document root. */
  cmd: Command | OpencliSpecJson;
  /** Its path from the CLI root; empty for the root. */
  path: string[];
  /** Its ancestor commands, outermost first; the root is not one. */
  ancestors: Command[];
  /** The document, where exit codes are looked up last. */
  doc: OpencliSpecJson;
  isRoot: boolean;
  /** The root's page id (`rootPageId`), its region and canonical; unset for a command. */
  rootId?: string;
}

function pageReference(page: Page, cliTitle: string, globalOptions: Option[] = []): OpencliReference {
  const { cmd, path: cmdPath } = page;
  // A command's page is named and answers by its path. The root's is named by
  // the CLI's title and answers its id, a URL path segment of that title.
  const name = page.isRoot ? cliTitle : cmdPath.join(' ');
  const region = page.isRoot && page.rootId ? page.rootId : name;
  const displayPath = page.isRoot ? cliTitle : `${cliTitle} ${name}`.trim();

  const args = visible(cmd.arguments);
  // The root's recursive options are the global options, documented apart.
  const opts = visible(cmd.options).filter((o) => !page.isRoot || !o.recursive);
  const subs = visible(cmd.commands);
  const hasSubcommands = subs.length > 0;
  // A group with arguments or options of its own also runs on its own.
  const placeholder = commandPlaceholder(hasSubcommands, args.length > 0 || opts.length > 0);
  const session = cmd['x-interactive'] as XInteractive | undefined;

  const definitions: OpencliRefDefinition[] = [];
  pushDefinition(definitions, 'Arguments', args.map(argumentToProperty));
  pushDefinition(definitions, 'Options', opts.map(optionToProperty));
  pushDefinition(definitions, 'Inherited options', inheritedOptions(cmd, page.ancestors).map(optionToProperty));
  pushDefinition(
    definitions,
    'Commands',
    subs.map((s) => ({ name: s.name, type: 'command', description: str(s.description) })),
  );
  // Global flags (root-level recursive options) available on every command.
  pushDefinition(definitions, 'Global options', pageGlobalOptions(page, globalOptions).map(optionToProperty));
  pushDefinition(definitions, 'Interactive commands', visible(session?.commands).map(interactiveCommandProperty));
  pushDefinition(definitions, 'Keys', visible(session?.keys).map(keyProperty));
  pushDefinition(definitions, 'Environment', visible(cmd['x-environment'] as EnvVar[] | undefined).map(environmentProperty));
  pushDefinition(
    definitions,
    'Exit codes',
    exitCodes(page)
      .filter((e) => typeof e?.code === 'number')
      .map(exitCodeProperty),
  );

  // Sidebar grouping: a leaf top-level command sits directly under "Commands".
  // A command that owns subcommands becomes a nested group (named after itself)
  // under "Commands", and each of its subcommands sits inside that same group —
  // e.g. `components` + `components install` → "Commands" › "components". The
  // root's page (an empty path) sits directly under "Commands".
  const group =
    cmdPath.length === 1 && !hasSubcommands ? ['Commands'] : ['Commands', ...(hasSubcommands ? cmdPath : cmdPath.slice(0, -1))];

  // The runnable invocation ("CLI Tool") at groups[0], then the command's own
  // examples, plus — when the OpenAPI binding carried one — an "Example
  // response" group rendered as a JSON sample. This mirrors an OpenAPI page's
  // request + response code samples; Atlas stacks the groups vertically. Keep
  // the CLI Tool group first.
  const groups: OpencliRefExampleGroup[] = [
    { examples: cliToolExamples(cliTitle, cmd, cmdPath, placeholder === REQUIRED_SUBCOMMAND) },
  ];
  const examplesGroup = ownExamplesGroup(cmd);
  if (examplesGroup) groups.push(examplesGroup);
  const responseGroup = responseExampleGroup(cmd as Command);
  if (responseGroup) groups.push(responseGroup);

  const description = page.isRoot
    ? str(page.doc.info?.description) || str(page.doc.info?.summary)
    : str((cmd as Command).description);

  return {
    // A command that owns subcommands reads as "<path> <command>" (or
    // "<path> [command]" when it also runs on its own) so it's distinct from
    // its group and shows it takes a subcommand.
    title: (placeholder ? `${name} ${placeholder}` : name) || cliTitle,
    canonical: (page.isRoot ? region : cmdPath.join('/')) || cliTitle,
    description,
    category: 'cli',
    // context.path is the region key the docs engine writes into the page
    // frontmatter (`cli: <spec>#<path>`) and reads back to re-resolve the command.
    // api.cli shows the more specific form — a command group carries its placeholder.
    context: {
      path: region,
      fullPath: generateUsage(page.doc, cmd as Command, displayPath, { commandPlaceholder: placeholder ?? false }),
      group,
    },
    examples: { groups },
    definitions,
  };
}

/**
 * The subcommand placeholder of a page: `<command>` for a group that needs a
 * subcommand, `[command]` for one that also runs on its own, none for a leaf.
 */
function commandPlaceholder(hasSubcommands: boolean, runsOnItsOwn: boolean): string | null {
  if (!hasSubcommands) return null;
  return runsOnItsOwn ? OPTIONAL_SUBCOMMAND : REQUIRED_SUBCOMMAND;
}

/** A `{title, properties}` definition, left out when it has no rows. */
function pushDefinition(definitions: OpencliRefDefinition[], title: string, properties: OpencliRefProperty[]) {
  if (properties.length) definitions.push({ title, properties });
}

/**
 * The options a command inherits from its ancestor commands, outermost first.
 *
 * Each name resolves to its nearest declaration, the way a CLI library
 * propagates a recursive option: the command's own, else the nearest ancestor
 * that declares the name, recursive or not. Only a recursive, visible nearest
 * declaration is inherited; a plain one ends the chain, and a redeclaration
 * hides the outer one. The root's are the global options, not these.
 */
function inheritedOptions(cmd: Command | OpencliSpecJson, ancestors: Command[]): Option[] {
  const taken = new Set(optionNames(cmd));
  const nearestFirst: Option[][] = [];
  for (const ancestor of [...ancestors].reverse()) {
    const here: Option[] = [];
    for (const opt of arr<Option>(ancestor.options)) {
      const name = str(opt?.name);
      if (taken.has(name)) continue;
      taken.add(name);
      if (opt?.recursive && !opt.hidden) here.push(opt);
    }
    nearestFirst.push(here);
  }
  return nearestFirst.reverse().flat();
}

/**
 * The global options one page lists with `globalOptionsPerCommand`: the
 * root's, less every name the command or one of its ancestor commands
 * declares. That nearer declaration is the one the command gets (listed under
 * Options or Inherited options), or, when it is not recursive, the end of the
 * chain. The root's own page lists them all.
 */
function pageGlobalOptions(page: Page, globalOptions: Option[]): Option[] {
  if (page.isRoot) return globalOptions;
  const taken = new Set([page.cmd, ...page.ancestors].flatMap(optionNames));
  return globalOptions.filter((o) => !taken.has(str(o?.name)));
}

/** The names of every option a command declares, hidden ones included. */
function optionNames(cmd: Command | OpencliSpecJson): string[] {
  return arr<Option>(cmd.options).map((o) => str(o?.name));
}

/** The exit codes a page shows: the command's own, else its nearest ancestor's, the root's last. */
function exitCodes(page: Page): ExitCode[] {
  const chain: (Command | OpencliSpecJson)[] = [page.cmd, ...[...page.ancestors].reverse(), page.doc];
  return chain.map((c) => arr<ExitCode>(c?.exitCodes)).find((codes) => codes.length) ?? [];
}

/** One "Exit codes" row; the caller keeps only entries whose `code` is a number. */
function exitCodeProperty(entry: ExitCode): OpencliRefProperty {
  return { name: String(entry.code), type: 'integer', description: str(entry.description) };
}

/**
 * One "Interactive commands" row: a command typed inside the session, with its
 * arguments, options and subcommands as nested rows. Its aliases are words,
 * not flags.
 */
function interactiveCommandProperty(c: InteractiveCommand): OpencliRefProperty {
  const aliases = arr<unknown>(c.aliases);
  const aliasNote = aliases.length ? ` (alias: ${aliases.map(jsString).join(', ')})` : '';
  const nested = [
    ...visible(c.arguments).map(argumentToProperty),
    ...visible(c.options).map(optionToProperty),
    ...visible(c.commands).map(interactiveCommandProperty),
  ];
  return {
    name: c.name,
    type: 'command',
    description: `${str(c.description)}${aliasNote}`,
    ...(nested.length ? { properties: nested } : {}),
  };
}

/** One "Keys" row: the keys (any one of them), what they do, and where, when the binding does not apply everywhere. */
function keyProperty(binding: KeyBinding): OpencliRefProperty {
  const context = typeof binding.context === 'string' && binding.context ? ` (context: ${binding.context})` : '';
  return {
    name: arr<unknown>(binding.keys).map(jsString).join(', '),
    type: 'key',
    description: `${str(binding.description)}${context}`,
  };
}

/** One "Environment" row. The values a variable accepts surface like an argument's accepted values: an `examples` meta. */
function environmentProperty(variable: EnvVar): OpencliRefProperty {
  const values = arr<unknown>(variable.values);
  return {
    name: variable.name,
    type: 'string',
    description: str(variable.description),
    ...(values.length ? { meta: [{ name: 'examples', value: values }] } : {}),
  };
}

/** The non-empty string entries of a command's `examples`. */
function exampleLines(cmd: Command | OpencliSpecJson): string[] {
  return arr<unknown>(cmd.examples).filter((e): e is string => typeof e === 'string' && e !== '');
}

/** The "Examples" group: the command's own examples, one per line, in one shell sample. Null when it has none. */
function ownExamplesGroup(cmd: Command | OpencliSpecJson): OpencliRefExampleGroup | null {
  const lines = exampleLines(cmd);
  if (!lines.length) return null;
  return {
    description: 'Examples',
    examples: [{ codeblock: { tabs: [{ title: 'CLI Tool', language: 'shell', code: lines.join('\n') }] } }],
  };
}

// JS-semantics helpers that match the Rust twin on any input, malformed ones
// included (`array`, `visible`, `str_or_empty`, `js_string` there).

/** An array-valued field, else []. */
function arr<T>(v: unknown): T[] {
  return Array.isArray(v) ? (v as T[]) : [];
}

/** The entries of an array-valued field that are not hidden. */
function visible<T>(v: readonly T[] | null | undefined): T[] {
  return arr<T>(v).filter((x) => !(x as { hidden?: unknown } | null)?.hidden);
}

/** A string-valued field, else ''. */
function str(v: unknown): string {
  return typeof v === 'string' ? v : '';
}

/** `String(value)` as the Rust twin renders a JSON value: an array's null entries are empty. */
function jsString(v: unknown): string {
  return Array.isArray(v) ? v.map((i) => (i === null ? '' : jsString(i))).join(',') : String(v);
}

/** Map a response content type to a code-sample language. */
function responseLanguage(contentType: string | undefined): string {
  if (!contentType) return 'json';
  if (/json/i.test(contentType)) return 'json';
  if (/xml/i.test(contentType)) return 'xml';
  return 'text';
}

/**
 * The "Example response" group built from the command's `x-openapi.responses`
 * binding (emitted by openapi2opencli) — one code sample per captured response,
 * rendered alongside the "CLI Tool" invocation like an OpenAPI page's
 * request/response samples. Returns null when no response example is available.
 */
function responseExampleGroup(cmd: Command): OpencliRefExampleGroup | null {
  const responses = cmd['x-openapi']?.responses;
  if (!responses?.length) return null;

  const examples: OpencliRefExampleGroup['examples'] = [];
  for (const res of responses) {
    if (res.example == null) continue; // skip missing / null bodies (no useful sample)
    const code = typeof res.example === 'string' ? res.example : JSON.stringify(res.example, null, 2);
    examples.push({
      codeblock: {
        title: res.status || '200',
        tabs: [{ title: res.contentType || 'application/json', language: responseLanguage(res.contentType), code }],
      },
    });
  }
  if (!examples.length) return null;
  return { description: 'Example response', examples };
}

/** Shell-quote a value only when it needs it (spaces / special chars). */
function shellQuote(value: string): string {
  return /[\s'"$`\\<>|&;()]/.test(value) ? `'${value.replace(/'/g, "'\\''")}'` : value;
}

/** A representative value for an argument: an explicit example, an accepted/enum value, else a placeholder. */
function exampleValue(arg: Argument | undefined): string {
  const example = arg?.metadata?.find((m) => m.name === 'example')?.value;
  if (typeof example === 'string' && example) return example;
  if (arg?.acceptedValues?.length) return String(arg.acceptedValues[0]);
  return 'Example data';
}

/**
 * The "CLI Tool" example(s) for a command. When a required argument enumerates
 * its accepted values (e.g. `completion <zsh|fish>`), each value becomes its own
 * example (`codeblock.title` = the value) — so Atlas renders an example-level
 * switcher (`[zsh] [fish]`) rather than language tabs inside a single code
 * block. Otherwise a single "CLI Tool" example.
 */
function cliToolExamples(
  cliTitle: string,
  cmd: Command | OpencliSpecJson,
  cmdPath: string[],
  needsSubcommand: boolean,
): OpencliRefExampleGroup['examples'] {
  const reqArgs = (cmd.arguments || []).filter((a) => a.required && !a.hidden);
  // The argument whose representative value(s) label the example(s): an enum
  // (acceptedValues) gives one tab per value; a single example value gives one.
  const variantArg = [...reqArgs].reverse().find((a) => a.acceptedValues?.length || argExampleValue(a) != null);
  const values = variantArg?.acceptedValues?.length
    ? variantArg.acceptedValues.map(String)
    : variantArg
      ? [String(argExampleValue(variantArg))]
      : null;

  if (variantArg && values) {
    // Each value is its own example (`codeblock.title` = the value) so Atlas
    // renders an example switcher — `[zsh] [fish]`, `[diagrams]`, etc.
    return values.map((value) => ({
      codeblock: {
        title: value,
        tabs: [
          {
            title: 'CLI Tool',
            language: 'shell',
            code: generateCliExample(cliTitle, cmd, cmdPath, needsSubcommand, { [variantArg.name]: value }),
          },
        ],
      },
    }));
  }

  return [
    { codeblock: { tabs: [{ title: 'CLI Tool', language: 'shell', code: generateCliExample(cliTitle, cmd, cmdPath, needsSubcommand) }] } },
  ];
}

/** An argument's explicit example value (from metadata), or null. */
function argExampleValue(arg: Argument): string | null {
  const example = arg.metadata?.find((m) => m.name === 'example')?.value;
  return typeof example === 'string' && example ? example : null;
}

/**
 * A runnable, concrete CLI invocation for a command — the command path plus its
 * required arguments and options filled with example values, one option per line
 * (the shape shown on an OpenAPI page's request sample). `needsSubcommand`
 * adds the `<command>` placeholder of a group that does not run on its own.
 * `overrides` pins a specific value for a named argument (used to render one
 * tab per accepted value).
 */
function generateCliExample(
  cliTitle: string,
  cmd: Command | OpencliSpecJson,
  cmdPath: string[],
  needsSubcommand: boolean,
  overrides: Record<string, string> = {},
): string {
  let head = [cliTitle, ...cmdPath].join(' ');

  // A command group that needs a subcommand shows a placeholder (unquoted, like
  // the usage formula), e.g. `xyd components <command>`. One that also runs on
  // its own is invoked without one.
  if (needsSubcommand) {
    head += ` ${REQUIRED_SUBCOMMAND}`;
  }

  for (const arg of (cmd.arguments || []).filter((a) => a.required && !a.hidden)) {
    head += ` ${shellQuote(overrides[arg.name] ?? exampleValue(arg))}`;
  }

  const lines = [head];
  for (const opt of (cmd.options || []).filter((o) => o.required && !o.hidden)) {
    if (!opt.arguments?.length) {
      lines.push(`--${opt.name}`); // boolean flag
    } else {
      lines.push(`--${opt.name} ${shellQuote(exampleValue(opt.arguments[0]))}`);
    }
  }

  return lines.join(' \\\n  ');
}

function argumentToProperty(arg: Argument): OpencliRefProperty {
  const meta: { name: string; value: unknown }[] = [];
  if (arg.required) meta.push({ name: 'required', value: 'true' });
  // Surface the argument's accepted values / example in the Arguments section
  // (e.g. `component` → diagrams, `shell` → zsh, fish).
  if (arg.acceptedValues?.length) {
    meta.push({ name: 'examples', value: arg.acceptedValues });
  } else {
    const example = arg.metadata?.find((m) => m.name === 'example')?.value;
    if (example != null && example !== '') meta.push({ name: 'example', value: example });
  }
  return {
    name: arg.name,
    type: arg.arity ? 'array' : 'string',
    description: arg.description || '',
    ...(meta.length ? { meta } : {}),
  };
}

function optionToProperty(opt: Option): OpencliRefProperty {
  const aliasNote = opt.aliases?.length
    ? ` (alias: ${opt.aliases.map((a) => (a.length === 1 ? `-${a}` : `--${a}`)).join(', ')})`
    : '';
  const meta = opt.required ? [{ name: 'required', value: 'true' }] : undefined;
  return {
    name: `--${opt.name}`,
    // an option that takes a value vs. a boolean flag
    type: opt.arguments?.length ? 'string' : 'boolean',
    description: `${opt.description || ''}${aliasNote}`,
    ...(meta ? { meta } : {}),
  };
}
