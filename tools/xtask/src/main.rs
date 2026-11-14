//! Repository developer, testing, and verification tooling in Rust (`cargo xtask`).
//!
//! Subcommands:
//! - `cargo xtask fmt [--check] [--verbose] [--repository <dir>] [-- <rustfmt-args>...]`
//! - `cargo xtask check-boundary`
//! - `cargo xtask rust-producer-omission --manifest <path> [--steps <path>] [--root <dir>]`
//! - `cargo xtask check-rules [--base <rev>] [--title <title>] [--body-file <path>]`
//! - `cargo xtask sample-gate [--filter <fragments>] [--facts]`
//! - `cargo xtask doctor [--json <path>]`
//! - `cargo xtask focused-test [--validate | --prepare]`
//! - `cargo xtask land <status|enqueue|wait|land> [pr] [--expected-head <sha>]`
//! - `cargo xtask claims <status|claim|release|note> [options]`
//! - `cargo xtask provenance [--agent <name>] [--model <model>] [--effort <effort>]`
//! - `cargo xtask worktree <status|acquire|clean> [options]`
//! - `cargo xtask retire-attached-machines [--write] <path>...`
//! - `cargo xtask alpha-forge LISTING OUTPUT` or `alpha-forge LISTING --check COMMITTED`
//! - `cargo xtask gamma-invoke --evaluator PATH --source PATH --output PATH [--input PATH] [--timeout SECONDS]`
//! - `cargo xtask source-closure MANIFEST OUTPUT [--prefix ENTRY]`

mod alpha_forge;
mod attached_machines;
mod boundary;
mod claims;
mod doctor;
mod execution;
mod fmt;
mod focused_test;
mod gamma_invoke;
mod landing;
mod managed_cache_retention;
mod producer;
mod provenance;
mod qualification;
mod repository;
mod rules;
mod rust_producer_omission;
mod samples;
mod source_closure;
mod worktree;

use std::path::PathBuf;
use std::process::ExitCode;

pub(crate) fn repo_root() -> Result<PathBuf, u8> {
    repository::current().map_err(|error| {
        eprintln!("xtask: {error}");
        1
    })
}

fn usage() -> &'static str {
    "\
usage: cargo xtask <command> [options]

commands:
  fmt              Format all workspace targets in bounded rustfmt chunks
  check-boundary   Verify native/runtime production dependency and Psi/Omega boundaries
  check-rules      Verify commit lane, PR provenance, ratchets, and board hygiene
  rust-producer-omission  Audit declared bootstrap dependencies (developer-only)
  sample-gate      Run differential native + Terminal Psi sample verification
  qualify          Run exact sample selections with retained producer evidence
  doctor           Run host toolchain and repository preflight checks
  focused-test     Validate, prepare, or run a focused CI Rust test selection
  land             Inspect, enqueue, or land a pull request via the GitHub merge queue
  claims           Inspect or update the cross-session work-claim registry
  provenance       Emit the canonical model-audit footer from the active session transcript
  worktree         List, acquire/recycle, or clean isolated `.agents/worktree/` checkouts
  managed-cache-retention  Retain the managed session's shared worker pool in mbx
  retire-attached-machines  Rewrite retired `machine Data::member` declarations as free machines
  alpha-forge      Resolve an Alpha Windows .hex listing into the seed image
  gamma-invoke     Frame Gamma source and run one evaluator
  source-closure   Check and concatenate an ordered source-closure manifest"
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
        "focused-test" => focused_test::run(&args),
        "land" => landing::run(&args),
        "claims" => claims::run(&args),
        "provenance" => provenance::run(&args),
        "worktree" => worktree::run(&args),
        "managed-cache-retention" => managed_cache_retention::run(&args),
        "retire-attached-machines" => attached_machines::run(&args),
        "alpha-forge" => alpha_forge::run(&args),
        "gamma-invoke" => gamma_invoke::run(&args),
        "source-closure" => source_closure::run(&args),
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
