// The generic OpenCLI extensions: `x-interactive`, `x-environment` and
// `x-passthrough`. Hand-written (unlike ./types.ts) after the vendored contract
// `opencli-extensions.schema.json`; see the README for its source.
import type { Argument, Option } from './types';

/** What a running interactive session accepts (`x-interactive`). */
export interface XInteractive {
  /** The commands typed inside the session, written without its prefix. */
  commands?: InteractiveCommand[];
  /** The key bindings of the session. */
  keys?: KeyBinding[];
  [k: string]: unknown;
}

/**
 * A command typed inside an interactive session. Its shape is an OpenCLI
 * command's, without extensions of its own. A docs site makes no page for it.
 */
export interface InteractiveCommand {
  name: string;
  aliases?: string[];
  description?: string;
  arguments?: Argument[];
  options?: Option[];
  commands?: InteractiveCommand[];
  hidden?: boolean;
  [k: string]: unknown;
}

/** One key binding of an interactive session. */
export interface KeyBinding {
  /** The keys that trigger it (`ctrl+c`, `esc`), any one of them. */
  keys: string[];
  /** What the keys do. */
  description: string;
  /** Where the binding applies, when not everywhere (a picker, the composer). */
  context?: string;
  [k: string]: unknown;
}

/** One environment variable a command reads (an `x-environment` entry). */
export interface EnvVar {
  name: string;
  description: string;
  /** The values it accepts, when it is not free-form. */
  values?: string[];
  [k: string]: unknown;
}

/**
 * `x-passthrough` on a command: the host hands the arguments after it to
 * another program. The command's arguments, options and subcommands, if any,
 * document that program.
 */
export interface XPassthrough {
  /** The program, when it is not named like the command. */
  program?: string;
  [k: string]: unknown;
}
