use clap::parser::ValueSource;
use clap::{ArgAction, ArgMatches, Command};

fn emit_named(arg: &clap::Arg, matches: &ArgMatches, out: &mut Vec<String>) {
    let id = arg.get_id().as_str();
    let long = arg.get_long().unwrap_or(id);
    match arg.get_action() {
        ArgAction::SetTrue => out.push(format!("--{long}")),
        ArgAction::SetFalse | ArgAction::Help | ArgAction::Version => {}
        _ => {
            if let Some(vals) = matches.get_raw(id) {
                for v in vals {
                    out.push(format!("--{long}"));
                    out.push(v.to_string_lossy().into_owned());
                }
            }
        }
    }
}

fn should_emit(
    arg: &clap::Arg,
    matches: &ArgMatches,
    exclude: &[&str],
) -> bool {
    let id = arg.get_id().as_str();
    !exclude.contains(&id)
        && matches.value_source(id) == Some(ValueSource::CommandLine)
}

fn emit_positional(
    arg: &clap::Arg,
    matches: &ArgMatches,
    out: &mut Vec<String>,
) {
    if let Some(vals) = matches.get_raw(arg.get_id().as_str()) {
        out.extend(vals.map(|v| v.to_string_lossy().into_owned()));
    }
}

/// Walk clap's argument metadata and reconstruct a canonical argv from the
/// parsed `matches`. `exclude` is a list of arg IDs to omit (the caller
/// decides what is sensitive — this function makes no policy choices).
///
/// Any flag registered with clap is picked up automatically — there is no
/// per-field maintenance burden when new flags are added. Short forms,
/// `--name=value`, env-var-only sources, and defaults are all normalized:
///  - emitted in long form (`--chars` not `-C`)
///  - emitted as separate tokens (`--chars 500`, not `--chars=500`)
///  - omitted when the user did not set the value on the command line
#[allow(
    dead_code,
    reason = "utility for upcoming query-log canonicalization; see deferred task"
)]
pub(crate) fn canonical_argv(
    matches: &ArgMatches,
    cmd: &Command,
    exclude: &[&str],
) -> Vec<String> {
    let mut named = vec![cmd.get_name().to_string()];
    let mut positional: Vec<String> = Vec::new();
    for arg in cmd
        .get_arguments()
        .filter(|a| should_emit(a, matches, exclude))
    {
        if arg.is_positional() {
            emit_positional(arg, matches, &mut positional);
        } else {
            emit_named(arg, matches, &mut named);
        }
    }
    named.extend(positional);
    named
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};
    use serial_test::serial;

    #[derive(Parser)]
    #[command(name = "t")]
    struct T {
        paths: Vec<String>,
        #[arg(short = 'C', long)]
        chars: Option<u64>,
        #[arg(short, long, default_value = "auto")]
        format: String,
        #[arg(short = 'g', long)]
        grep: Vec<String>,
        #[arg(long)]
        flag: bool,
        #[arg(long)]
        secret: Option<String>,
        #[arg(long, env = "T_FROM_ENV")]
        from_env: Option<String>,
    }

    fn canon(args: &[&str], exclude: &[&str]) -> Vec<String> {
        let cmd = T::command();
        let matches = cmd.clone().get_matches_from(args);
        canonical_argv(&matches, &cmd, exclude)
    }

    #[test]
    fn short_form_normalized_to_long_form() {
        let v = canon(&["t", "-C", "500", "file"], &[]);
        assert_eq!(v, vec!["t", "--chars", "500", "file"]);
    }

    #[test]
    fn equals_form_normalized_to_space_separated() {
        let v = canon(&["t", "--chars=200"], &[]);
        assert_eq!(v, vec!["t", "--chars", "200"]);
    }

    #[test]
    fn vec_args_repeat_per_value() {
        let v = canon(&["t", "-g", "a", "--grep", "b"], &[]);
        assert_eq!(v, vec!["t", "--grep", "a", "--grep", "b"]);
    }

    #[test]
    fn exclude_list_drops_named_args() {
        let v = canon(&["t", "--secret", "shh", "-C", "10"], &["secret"]);
        assert_eq!(v, vec!["t", "--chars", "10"]);
    }

    #[test]
    fn defaults_and_unset_args_omitted() {
        let v = canon(&["t", "file"], &[]);
        assert_eq!(v, vec!["t", "file"]);
    }

    #[test]
    fn bool_flag_emits_no_value() {
        let v = canon(&["t", "--flag"], &[]);
        assert_eq!(v, vec!["t", "--flag"]);
    }

    #[test]
    fn positional_args_appear_at_end() {
        let v = canon(&["t", "-C", "100", "file1", "file2"], &[]);
        assert_eq!(v, vec!["t", "--chars", "100", "file1", "file2"]);
    }

    #[test]
    #[serial]
    fn env_set_args_omitted() {
        let prev = std::env::var("T_FROM_ENV").ok();
        // SAFETY: tests touching env vars must run serially; this test
        // restores the previous value before returning.
        unsafe {
            std::env::set_var("T_FROM_ENV", "from-env-value");
        }
        let v = canon(&["t", "file"], &[]);
        unsafe {
            match prev {
                Some(p) => std::env::set_var("T_FROM_ENV", p),
                None => std::env::remove_var("T_FROM_ENV"),
            }
        }
        assert_eq!(
            v,
            vec!["t", "file"],
            "env-only values must not appear in canonical argv"
        );
    }
}
