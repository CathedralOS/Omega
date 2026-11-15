//! Repository developer, testing, and verification tooling in Rust (`cargo xtask`).
//!
//! Subcommands:
//! - `cargo xtask fmt [--check] [--verbose] [--repository <dir>] [-- <rustfmt-args>...]`
//! - `cargo xtask check-boundary`
//! - `cargo xtask rust-producer-omission --manifest <path> [--steps <path>] [--root <dir>]`
//! - `cargo xtask check-rules [--base <rev>] [--title <title>] [--body-file <path>]`
//! - `cargo xtask sample-gate [--filter <fragments>] [--facts]`
//! - `cargo xtask qualify --samples <paths> --output <new-directory> [--target <host>]`
//! - `cargo xtask doctor [--json <path>]`
//! - `cargo xtask claims <status|claim|release|note> [options]`
//! - `cargo xtask provenance [--agent <name>] [--model <model>] [--effort <effort>]`
//! - `cargo xtask worktree <status|acquire|begin|release|clean> [options]`
//! - `cargo xtask managed-cache-retention [--repository <dir>]`
//! - `cargo xtask retire-attached-machines [--write] <path>...`
//! - `cargo xtask alpha-forge LISTING OUTPUT` or `alpha-forge LISTING --check COMMITTED`
//! - `cargo xtask gamma-invoke --evaluator PATH --source PATH --output PATH [--input PATH] [--timeout SECONDS]`
//! - `cargo xtask source-closure MANIFEST OUTPUT [--prefix ENTRY]`
//! - `cargo xtask sweep [--quick | --cause <text> --previous <sweep>] [--target <name>] [--shard K/N] [--out <file>]`
//! - `cargo xtask sweep merge --out <file.json> <shard.json>...`
//! - `cargo xtask sweep fleet <start|status|collect|restart>`

mod alpha_forge;
mod attached_machines;
mod boundary;
mod claims;
mod doctor;
mod execution;
mod fmt;
mod gamma_invoke;
mod managed_cache_retention;
mod producer;
mod provenance;
mod qualification;
mod repository;
mod rules;
mod rust_producer_omission;
mod samples;
mod source_closure;
mod sweep;
mod work_interval;
mod worktree;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

/// A status consumer may stop after reading a prefix of the output.
fn write_status(render: impl FnOnce(&mut dyn Write) -> io::Result<()>) -> Result<(), u8> {
    let mut output = io::stdout().lock();
    match render(&mut output).and_then(|()| output.flush()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => {
            let _ = writeln!(io::stderr(), "xtask: could not write status: {error}");
            Err(1)
        }
    }
}

pub(crate) fn repo_root() -> Result<PathBuf, u8> {
    repository::current().map_err(|error| {
        eprintln!("xtask: {error}");
        1
    })
}

fn usage() -> &'static str {
    "\
usage: cargo xtask <command> [options]

verification and gates:
  check-rules               Verify commit lane, PR provenance, ratchets, and board hygiene
  check-boundary            Verify native/runtime production dependency and Psi/Omega boundaries
  sample-gate               Run differential native + Psi sample verification
  qualify                   Run exact sample selections with retained producer evidence
  sweep                     Run omega --check over the programs and group where each one stops;
                            `sweep fleet` splits one target's sweep across machines
  rust-producer-omission    Audit declared bootstrap dependencies (developer-only)

formatting and migration:
  fmt                       Format all workspace targets in bounded rustfmt chunks
  retire-attached-machines  Rewrite retired `machine Data::member` declarations as free machines

coordination and workspace:
  doctor                    Run host toolchain and repository preflight checks
  claims                    Inspect or update the cross-session work-claim registry
  worktree                  List, acquire/recycle, release, or clean isolated `.agents/worktree/` checkouts
  managed-cache-retention   Retain the managed session's shared worker pool in mbx
  provenance                Emit the canonical model-audit footer from the active session transcript

bootstrap tools:
  alpha-forge               Resolve an Alpha Windows .hex listing into the seed image
  gamma-invoke              Frame Gamma source and run one evaluator
  source-closure            Check and concatenate an ordered source-closure manifest"
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help" | "help") {
        println!("{}", usage());
        return ExitCode::SUCCESS;
    }
    let subcommand = args.remove(0);
    let result = match subcommand.as_str() {
        "fmt" => fmt::run(&args),
        "check-boundary" => boundary::run(&args),
        "check-rules" => rules::run(&args),
        "rust-producer-omission" => rust_producer_omission::run(&args),
        "sample-gate" => samples::run(&args),
        "qualify" => qualification::run(&args),
        "doctor" => doctor::run(&args),
        "claims" => claims::run(&args),
        "provenance" => provenance::run(&args),
        "worktree" => worktree::run(&args),
        "managed-cache-retention" => managed_cache_retention::run(&args),
        "retire-attached-machines" => attached_machines::run(&args),
        "alpha-forge" => alpha_forge::run(&args),
        "gamma-invoke" => gamma_invoke::run(&args),
        "source-closure" => source_closure::run(&args),
        "sweep" => sweep::run(&args),
        other => {
            eprintln!("xtask: unknown command `{other}`\n\n{}", usage());
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

#[cfg(test)]
mod tests {
    use super::usage;

    #[test]
    fn usage_uses_public_psi_name_and_lists_all_subcommands() {
        let help = usage();
        assert!(
            !help.contains("Terminal Psi"),
            "user-facing usage must use Psi, not Terminal Psi"
        );
        for cmd in [
            "fmt",
            "check-boundary",
            "check-rules",
            "rust-producer-omission",
            "sample-gate",
            "qualify",
            "doctor",
            "claims",
            "provenance",
            "worktree",
            "managed-cache-retention",
            "retire-attached-machines",
            "alpha-forge",
            "gamma-invoke",
            "source-closure",
            "sweep",
        ] {
            assert!(help.contains(cmd), "usage missing subcommand `{cmd}`");
        }
    }
}
