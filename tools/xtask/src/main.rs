//! Repository developer, testing, and verification tooling in Rust (`cargo xtask`).
//!
//! Subcommands:
//! - `cargo xtask fmt [--check] [--verbose] [--repository <dir>] [-- <rustfmt-args>...]`
//! - `cargo xtask check-boundary`
//! - `cargo xtask rust-producer-omission --manifest <path> [--steps <path>] [--root <dir>]`
//! - `cargo xtask check-rules [--base <rev>] [--title <title>] [--body-file <path>]`
//! - `cargo xtask corpus [--filter <fragments>] [--record] [--native] [--list-pinned-regressions]`
//! - `cargo xtask corpus-gate <plan|build|check|sweep-shard|sweep-report> [options]`
//! - `cargo xtask sample-gate [--filter <fragments>] [--facts]`
//! - `cargo xtask stage-coverage [--list <file>] [--json <path>] <program>... [-- <omega-args>...]`
//! - `cargo xtask doctor [--json <path>]`
//! - `cargo xtask focused-test [--validate | --prepare]`
//! - `cargo xtask land <status|enqueue|wait|land> [pr] [--expected-head <sha>]`
//! - `cargo xtask claims <status|claim|release|note> [options]`
//! - `cargo xtask provenance [--agent <name>] [--model <model>] [--effort <effort>]`
//! - `cargo xtask worktree <status|acquire|clean> [options]`
//! - `cargo xtask retire-attached-machines [--write] <path>...`

mod attached_machines;
mod boundary;
mod claims;
mod corpus;
mod corpus_gate;
mod doctor;
mod execution;
#[path = "../../../omega-rust/omega/compilation/compiler/tests/support/fixture_selection.rs"]
mod fixture_selection;
mod fmt;
mod focused_test;
mod landing;
mod managed_cache_retention;
mod producer;
mod provenance;
mod qualification;
mod rules;
#[path = "../../../omega-rust/omega/compilation/compiler/tests/support/runtime_oracles.rs"]
mod runtime_oracles;
mod rust_producer_omission;
mod samples;
mod stage_coverage;
mod worktree;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub(crate) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("tools/xtask sits two levels below repository root")
        .to_path_buf()
}

fn usage() -> &'static str {
    "\
usage: cargo xtask <command> [options]

commands:
  fmt              Format all workspace targets in bounded rustfmt chunks
  check-boundary   Verify native/runtime production dependency and Psi/Omega boundaries
  check-rules      Verify commit lane, PR provenance, ratchets, and corpus/board hygiene
  rust-producer-omission  Audit declared bootstrap dependencies (developer-only)
  corpus           Run focused corpus verification, recording, or regression listing
  corpus-gate      Plan and check a pull request's corpus rows, or run the nightly ledger sweep
  sample-gate      Run differential native + Terminal Psi sample verification
  stage-coverage   Count the Psi 05, validation and Psi 06 lines a program runs
  qualify          Execute exact corpus and CLI selections with retained producer evidence
  doctor           Run host toolchain and repository preflight checks
  focused-test     Validate, prepare, or run a focused CI Rust test selection
  land             Inspect, enqueue, or land a pull request via the GitHub merge queue
  claims           Inspect or update the cross-session work-claim registry
  provenance       Emit the canonical model-audit footer from the active session transcript
  worktree         List, acquire/recycle, or clean isolated `.agents/worktree/` checkouts
  managed-cache-retention  Retain the managed session's shared worker pool in mbx
  retire-attached-machines  Rewrite retired `machine Data::member` declarations as free machines"
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
        "corpus" => corpus::run(&args),
        "corpus-gate" => corpus_gate::run(&args),
        "sample-gate" => samples::run(&args),
        "stage-coverage" => stage_coverage::run(&args),
        "qualify" => qualification::run(&args),
        "doctor" => doctor::run(&args),
        "focused-test" => focused_test::run(&args),
        "land" => landing::run(&args),
        "claims" => claims::run(&args),
        "provenance" => provenance::run(&args),
        "worktree" => worktree::run(&args),
        "managed-cache-retention" => managed_cache_retention::run(&args),
        "retire-attached-machines" => attached_machines::run(&args),
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
