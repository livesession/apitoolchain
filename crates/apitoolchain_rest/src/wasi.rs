//! The WASI environment `api rest` hands to the guest.
//!
//! Split out as a pure function so the policy — argv, env, preopens — is
//! testable without instantiating a 65 MB module.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The program name the guest sees as `os.Args[0]`.
///
/// Upstream's cobra root is `&cobra.Command{Use: "openapi", …}` and cobra takes
/// its command path from `Use`, falling back to `os.Args[0]` only when `Use` is
/// empty. So this does NOT make help text read `api rest …` — that is not
/// reachable from the host at all. It is set so `os.Args[0]` is honest.
pub(crate) const ARGV0: &str = "openapi";

/// What `run` will configure the guest with.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Plan {
    /// argv, including argv[0].
    pub argv: Vec<String>,
    /// Environment pairs, with `PWD` forced to the host's current directory.
    pub env: Vec<(String, String)>,
    /// `(host_path, guest_path)` for the single preopen.
    pub preopen: (PathBuf, PathBuf),
}

/// Build the guest environment.
///
/// ## Why the preopen is `/` → `/` and not the working directory
///
/// Go resolves paths IN THE GUEST before WASI ever sees them
/// (`syscall/fs_wasip1.go`): it joins a relative path against `$PWD`, cleans
/// `..` — clamping it at root — then picks the longest matching preopen prefix
/// and strips it.
///
/// So the instinctive "preopen the CWD as guest `/`" is actively dangerous:
///
/// * `api rest spec validate /etc/foo.yaml` → the guest joins to `/etc/foo.yaml`,
///   matches preopen `/`, strips it, and opens `etc/foo.yaml` RELATIVE TO THE
///   CWD. Usually a baffling ENOENT; occasionally a file that exists, and the
///   wrong document is silently validated.
/// * `../shared/api.yaml` → `..` is clamped at root, giving `/shared/api.yaml`
///   → `$CWD/shared/api.yaml`. Again silently wrong, with no error.
///
/// Narrowing to "the CWD plus the parent of every path-looking argument" is not
/// implementable by a passthrough: it cannot know which arguments are paths
/// (`-o out.yaml`, `bundle <in> <out>`, `localize <in> <dir>`, flags that vary
/// per subcommand, commands upstream has not shipped yet), and it would still
/// miss a `$ref: ../common/schemas.yaml` that appears on no command line.
///
/// Mapping host `/` to guest `/` makes the two namespaces byte-identical, so
/// there is no translation left to get wrong. Absolute paths, `..`, symlinks
/// and relative `$ref` chains all behave exactly as they do for a native
/// install.
///
/// ## On the security posture
///
/// The wasm sandbox buys PORTABILITY here, not confinement. A natively
/// installed `openapi` has precisely this authority: it reads the specs the
/// user names and writes the files the user asks for. Narrowing the preopen
/// does not make it safer — it makes it open the wrong file. And wasmtime-wasi's
/// preopen permission checks have had real bypasses (CVE-2026-47261), so this
/// must never become load-bearing as a trust boundary.
pub(crate) fn plan<I>(args: &[String], cwd: &Path, host_env: I) -> Plan
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let mut argv = Vec::with_capacity(args.len() + 1);
    argv.push(ARGV0.to_string());
    argv.extend(args.iter().cloned());

    // `PWD` is ours: Go reads it to resolve relative paths and falls back to
    // "whatever the first preopen happens to be" when it is unset, which is not
    // something to leave to chance. Any inherited PWD is dropped, not merged.
    //
    // Non-UTF-8 pairs are skipped rather than panicked on — WasiCtxBuilder only
    // accepts `str`, and one odd variable in the environment must not take down
    // the command.
    let mut env: Vec<(String, String)> = host_env
        .into_iter()
        .filter(|(key, _)| key != "PWD")
        .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
        .collect();
    env.push(("PWD".to_string(), cwd.to_string_lossy().into_owned()));

    Plan {
        argv,
        env,
        preopen: (PathBuf::from("/"), PathBuf::from("/")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs
            .iter()
            .map(|(k, v)| (OsString::from(k), OsString::from(v)))
            .collect()
    }

    #[test]
    fn argv0_is_injected_ahead_of_the_users_arguments() {
        // Without this the guest loses its first real argument: cobra reads
        // os.Args[1..], so `spec` would be consumed as the program name.
        let p = plan(
            &["spec".into(), "validate".into(), "x.yaml".into()],
            Path::new("/work"),
            env(&[]),
        );
        assert_eq!(p.argv, ["openapi", "spec", "validate", "x.yaml"]);
    }

    #[test]
    fn pwd_is_the_host_cwd_and_never_the_inherited_one() {
        let p = plan(
            &[],
            Path::new("/work/project"),
            env(&[("PWD", "/somewhere/else")]),
        );
        let pwds: Vec<&str> = p
            .env
            .iter()
            .filter(|(k, _)| k == "PWD")
            .map(|(_, v)| v.as_str())
            .collect();
        assert_eq!(
            pwds,
            ["/work/project"],
            "exactly one PWD, and it is the cwd"
        );
    }

    #[test]
    fn other_environment_variables_pass_through() {
        // HOME in particular: `spec lint` fails outright with
        // "Error: $HOME is not defined" when it is missing.
        let p = plan(
            &[],
            Path::new("/w"),
            env(&[("HOME", "/home/x"), ("NO_COLOR", "1")]),
        );
        assert!(p.env.contains(&("HOME".to_string(), "/home/x".to_string())));
        assert!(p.env.contains(&("NO_COLOR".to_string(), "1".to_string())));
    }

    #[test]
    fn non_utf8_environment_pairs_are_dropped_not_fatal() {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let bad = OsString::from_vec(vec![0x66, 0xff, 0x66]);
            let p = plan(&[], Path::new("/w"), vec![(bad, OsString::from("v"))]);
            assert!(p.env.iter().all(|(k, _)| k == "PWD"));
        }
    }

    #[test]
    fn the_preopen_does_not_remap() {
        // The whole point: guest namespace == host namespace, so an absolute
        // path cannot resolve somewhere else. See this module's doc comment.
        let p = plan(&[], Path::new("/w"), env(&[]));
        assert_eq!(p.preopen, (PathBuf::from("/"), PathBuf::from("/")));
    }
}
