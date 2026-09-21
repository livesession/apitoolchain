//! Yours — scaffolded once by opencli2rust, never overwritten by regeneration.
//!
//! Two extension seams, both optional:
//!  * `CliOverrides` on [`Custom`] — hook/override every generated command
//!    (see src/gen/runtime/overrides.rs for the available methods).
//!  * [`register`] — add fully custom commands anywhere in the tree; a command
//!    registered on an existing path takes over that command's behavior.

use std::process::ExitCode;

use clap::{Arg, Command};

use crate::gen::runtime::{CliOverrides, CustomCommands, Error};

/// Override only what you need — every trait method has a default.
pub struct Custom;

impl CliOverrides for Custom {
    /// `api rest` speaks for itself.
    ///
    /// The embedded CLI has already printed its own diagnostics by the time it
    /// exits, so a non-zero status must become OUR exit status and nothing
    /// else — the default `error: {err}` line would be a second, redundant
    /// report of a failure the user has already read about.
    ///
    /// The seam offers only `Ok`/`Err`, so the status travels on
    /// `Error::Invalid` behind a marker no generated message can produce.
    fn print_error(&self, cmd_path: &[String], err: &Error) -> ExitCode {
        if let Some(code) = rest_exit_status(cmd_path, err) {
            return code;
        }
        eprintln!("error: {err}");
        ExitCode::FAILURE
    }
}

/// U+0001 — no real message begins with it.
const REST_EXIT: &str = "\u{1}rest-exit:";

/// Decode a `rest` exit status, if that is what this error carries.
///
/// Flat rather than nested: `clippy::collapsible_if` is on by default and CI
/// runs at `-D warnings`. Pure, so the whole exit-code contract is testable
/// without spawning a process.
fn rest_exit_status(cmd_path: &[String], err: &Error) -> Option<ExitCode> {
    if cmd_path.first().map(String::as_str) != Some("rest") {
        return None;
    }
    let Error::Invalid(message) = err else {
        return None;
    };
    let code: i32 = message.strip_prefix(REST_EXIT)?.parse().ok()?;
    // A process exit status keeps only the low 8 bits. A non-zero code that
    // truncates to zero would report success for a failure, so floor it.
    Some(match code as u8 {
        0 if code != 0 => ExitCode::FAILURE,
        byte => ExitCode::from(byte),
    })
}

/// `api rest <anything>` — argv forwarded verbatim to the embedded OpenAPI CLI.
///
/// ## Why this command owns no subcommands
///
/// The shape is forced by the dispatcher, not chosen. `CustomCommands::find`
/// matches the EXACT invoked depth (`under.len() + 1 == path.len()`) and
/// `descend` walks to the DEEPEST invoked subcommand. Give `rest` any
/// subcommand — including via `allow_external_subcommands`, which synthesises a
/// nested match — and `api rest spec validate` makes `descend` report
/// `["rest", "spec"]`, which `find` does not match, and dispatch falls through
/// to `error: unknown command`. A single trailing var-arg keeps the path at
/// `["rest"]`, and the arguments arrive as plain values.
///
/// ## Why the help and version flags are disabled
///
/// Load-bearing, not tidiness. clap resolves `--help` against the command
/// keymap BEFORE the positional gets a chance, so with the automatic help flag
/// registered `api rest --help` prints OUR help and upstream's never runs. The
/// same applies to `-h`: the parser's hyphen-passthrough branch is guarded by
/// `short_arg.any(|c| !cmd.contains_short(c))`, which is false once `-h` exists.
fn rest_command() -> Command {
    Command::new("rest")
        .about("Run the embedded OpenAPI CLI (upstream `openapi`), verbatim")
        .long_about(
            "Forwards every argument to an embedded build of \
             github.com/speakeasy-api/openapi, so `api rest spec validate x.yaml` \
             is `openapi spec validate x.yaml`.\n\n\
             Its help and usage text name `openapi`, not `api rest`: the command \
             names come from upstream and are deliberately not rewritten.\n\n\
             Remote $ref URLs are NOT fetched — the embedded build has no network \
             access of any kind. Vendor those documents locally first.",
        )
        .disable_help_flag(true)
        .disable_help_subcommand(true)
        .disable_version_flag(true)
        .arg(
            Arg::new("args")
                .value_name("ARGS")
                .num_args(0..)
                .allow_hyphen_values(true)
                .trailing_var_arg(true),
        )
}

/// Register hand-written commands. Called by the generated `main`.
pub fn register(commands: &mut CustomCommands) {
    commands.add(&[], rest_command(), |_ctx, matches| async move {
        let args: Vec<String> = matches
            .get_many::<String>("args")
            .map(|values| values.cloned().collect())
            .unwrap_or_default();

        match apitoolchain_rest::run(&args) {
            Ok(0) => Ok(()),
            Ok(code) => Err(Error::Invalid(format!("{REST_EXIT}{code}"))),
            Err(err) => Err(Error::Invalid(format!("rest: {err}"))),
        }
    });
}

pub fn overrides() -> Custom {
    Custom
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::ArgMatches;

    /// Parse `argv` through the real grafted tree and replicate `descend`.
    ///
    /// This mirrors `gen::cli::run` exactly, because the thing being asserted
    /// is the interaction between grafting, parsing and depth-matching — the
    /// one place where a plausible-looking registration silently produces
    /// "unknown command" for the user instead of running.
    fn invoke(argv: &[&str]) -> Result<(Vec<String>, Vec<String>), clap::Error> {
        let mut commands = CustomCommands::new();
        register(&mut commands);
        let matches = commands
            .graft(crate::gen::cli::root_command())
            .try_get_matches_from(argv)?;

        let mut path = Vec::new();
        let mut current: &ArgMatches = &matches;
        while let Some((name, sub)) = current.subcommand() {
            path.push(name.to_string());
            current = sub;
        }
        // try_get_many, not get_many: `args` exists only on `rest`, and
        // get_many PANICS on an id the matched command never defined.
        let forwarded = current
            .try_get_many::<String>("args")
            .ok()
            .flatten()
            .map(|v| v.cloned().collect())
            .unwrap_or_default();
        Ok((path, forwarded))
    }

    #[test]
    fn a_nested_invocation_still_dispatches_at_depth_one() {
        // The regression this design exists to prevent. If `rest` ever grows a
        // subcommand, `path` becomes ["rest", "spec"], `find` misses, and the
        // user gets "error: unknown command: rest spec validate".
        let (path, args) = invoke(&["api", "rest", "spec", "validate", "./f.yaml"]).unwrap();
        assert_eq!(path, ["rest"]);
        assert_eq!(args, ["spec", "validate", "./f.yaml"]);
    }

    #[test]
    fn flags_meant_for_the_guest_are_not_eaten() {
        let (_, args) =
            invoke(&["api", "rest", "spec", "lint", "--fix", "-w", "../a.yaml"]).unwrap();
        assert_eq!(args, ["spec", "lint", "--fix", "-w", "../a.yaml"]);
    }

    #[test]
    fn help_reaches_the_guest_rather_than_printing_ours() {
        // Both spellings, and both at the top level and nested. Without
        // disable_help_flag these return Err(DisplayHelp) and upstream's own
        // help — the only help that knows what the commands are — never runs.
        for argv in [
            &["api", "rest", "--help"][..],
            &["api", "rest", "-h"][..],
            &["api", "rest", "spec", "validate", "--help"][..],
        ] {
            let (path, args) = invoke(argv).unwrap_or_else(|e| {
                panic!("clap intercepted {argv:?}: {:?}", e.kind());
            });
            assert_eq!(path, ["rest"]);
            assert!(
                args.iter().any(|a| a == "--help" || a == "-h"),
                "the help flag was consumed before reaching the guest: {args:?}",
            );
        }
    }

    #[test]
    fn version_reaches_the_guest_so_upstream_reports_its_own() {
        let (_, args) = invoke(&["api", "rest", "--version"]).unwrap();
        assert_eq!(args, ["--version"]);
    }

    #[test]
    fn bare_rest_forwards_nothing_and_lets_the_guest_print_its_help() {
        let (path, args) = invoke(&["api", "rest"]).unwrap();
        assert_eq!(path, ["rest"]);
        assert!(args.is_empty());
    }

    #[test]
    fn generated_commands_are_untouched() {
        // Grafting `rest` must not disturb the generated tree.
        let (path, _) = invoke(&["api", "get", "sdks"]).unwrap();
        assert_eq!(path, ["get", "sdk"], "plural resolves through the alias");
    }

    #[test]
    fn exit_status_round_trips_through_the_error_seam() {
        let err = Error::Invalid(format!("{REST_EXIT}3"));
        let path = vec!["rest".to_string()];
        assert!(rest_exit_status(&path, &err).is_some());
    }

    #[test]
    fn a_status_that_truncates_to_zero_is_floored_at_failure() {
        // 256 & 0xff == 0. Reporting success for a failure is the one outcome
        // worth special-casing.
        let err = Error::Invalid(format!("{REST_EXIT}256"));
        let path = vec!["rest".to_string()];
        let code = rest_exit_status(&path, &err).expect("decoded");
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::FAILURE));
    }

    #[test]
    fn ordinary_errors_are_left_alone() {
        let path = vec!["rest".to_string()];
        assert!(rest_exit_status(&path, &Error::Invalid("boom".into())).is_none());

        // And a marker under a different command is not ours to decode.
        let other = vec!["get".to_string()];
        let err = Error::Invalid(format!("{REST_EXIT}3"));
        assert!(rest_exit_status(&other, &err).is_none());
    }
}
