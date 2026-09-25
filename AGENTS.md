# AGENTS.md

This is the canonical instruction file for every coding agent working in this
repository. Agent-specific instruction files must point here rather than copy
these rules. Repository skills are canonical in `.agents/skills/`; read and
edit them there. Pi and Codex discover that directory directly. Do not
maintain a second copy. For Claude Code, create an ignored checkout-local
link to the same directory:

- Windows PowerShell: `New-Item -ItemType Directory -Force .claude`, then
  `New-Item -ItemType Junction -Path .claude/skills -Target (Resolve-Path .agents/skills)`.
- macOS shell: `mkdir -p .claude`, then `ln -s ../.agents/skills .claude/skills`.

Create the link only when `.claude/skills` is absent; inspect an existing path
before replacing it. Repeat in a new checkout if using Claude there. The Windows
junction avoids administrator or Developer Mode requirements; it is local, not a
tracked symlink that Git may check out as a plain text file.

Omega is a proof-carrying systems language whose programs are data-oriented
state machines. This repository holds the language, its Rust reference
compiler, its Omega-written product compiler, and the Alpha-to-Omega bootstrap
chain.

- `omega-rust/` — the Rust reference producer: the working development
  compiler and differential comparator. Not canonical, not a language rung.
- `source/psi/` + `source/omega/` — the Omega-written product compiler, split
  along the same firewall. This is the destination.
- `source/library/` — bundled `omega::` packages (`name.omg` or `name/mod.omg`).
- `bootstrap/{0_alpha,…,5_omega}/` — the trust-minimizing bootstrap chain;
  `bootstrap/proofs/` holds Gamma-written derivation checking beside the
  rungs. Bootstrap scripts resolve cross-owner locations through
  `tools/bootstrap/paths.sh` — never hard-code sibling-relative paths — and
  may invoke, stamp, compare, and report but never parse, lower, manufacture
  semantic evidence, or decide trust.

## Commands

The Rust toolchain is pinned in `rust-toolchain.toml`; `rustup` selects it.

- Use `mbx` in place of Cargo for compile and test commands: `cargo check`
  becomes `mbx check`, `cargo test` becomes `mbx nextest run` (needs
  cargo-nextest 0.9.140 or newer). If `mbx` is unavailable, use Cargo without
  asking. Nextest does not run doctests; use `mbx test --doc`.
- Keep using `cargo fmt` and `cargo clean` directly — `mbx clean` differs.
  Whole-workspace formatting is `python tools/fmt.py`: `cargo fmt --all`
  exceeds the Windows command-line limit at this workspace's size.
- Compile a program: `mbx run -p omega -- --check <root.omg>`; run it:
  `mbx run -p omega -- run <root.omg>`. The CLI package is `omega` (binary
  `omega`) — there is no `omega-cli` package. The full flag surface:
  [omega-rust/omega/README.md](omega-rust/omega/README.md#command-surface).
- The compiler's end-to-end test is the corpus gate:
  `python tools/corpus_gate.py --filter <group/name fragment>`; a crate's own
  Rust tests run by package and name filter
  (`mbx nextest run -p <crate> <filter>`).
- Validation policy — what a change must run, baseline gates, corpus and
  bootstrap gates, test selection, slow builds — lives in
  [tools/testing.md](tools/testing.md). Read it before running more than the
  affected crate's check.

## The Psi/Omega firewall

The load-bearing split, and the most common way to put code in the wrong
place:

- **Psi** owns everything target-neutral on Omega source: lexing, parsing,
  resolution, typing, checking, proof, target-neutral optimization, and
  production of **Terminal Psi**.
- **Omega** consumes Terminal Psi and owns provider selection, Omega-side
  optimization, target realization, ABI, native emission, and execution.
- Target backends own only unavoidable ISA, ABI, object-format, and
  relocation detail. Cathedral (the downstream OS) owns OS structures — do
  not model page tables, schedulers, or drivers as compiler-owned types.

Terminal Psi is the only portable boundary. Targets are data, not control
flow: Psi checks every target-scoped machine body in every compilation, and
`--target` narrows what Omega realizes, never what Psi checks. The full
architecture contract — deferred Psi audit policy, crate placement,
discoverability, compositional lowering — lives in
[omega-rust/pipeline.md](omega-rust/pipeline.md).

## Conventions and workflow

The complete contract is [CONTRIBUTING.md](CONTRIBUTING.md): coding
conventions, platform support, boards, landing detail, scope checkpoints,
delegation, commit naming, prose. The rules that touch every edit:

- Real words in code (`character`, `expression`, `arguments` — not `ch`,
  `expr`, `args`).
- Arena-backed data with generational handles over scattered heap objects;
  ZII — the zero handle is the absence state, so do not wrap handles in
  `Optional`; no `RefCell` as an ownership escape.
- Commit subjects are `lane: statement` — pick the lane by changed
  responsibility per [CONTRIBUTING.md](CONTRIBUTING.md#commit-naming) before
  committing.
- `TASKS.md`, `TASKS_BOOTSTRAP.md`, `TASKS_OPTIMIZER.md` are execution
  boards, not changelogs: an item exists only while it names unfinished work
  and is deleted when acceptance passes. Owner decisions go to
  `OWNER_QUESTIONS.md`, not the boards.
- Windows and macOS are both supported development hosts; shared tooling
  needs a documented usable entrypoint on each.
- Read the matching skill before starting: `.agents/skills/` — `advance` for
  board work, `rust-systems-programmer` for Rust implementation and review,
  `architecture-cleanup`, `task-board-cleanup`, `local-swarm`, `cloud-swarm`.

Deliberate configuration — do not "fix" it: `clippy.toml` thresholds are
raised, `debug = 0` and `opt-level = 1` in dev/test profiles (opt back in
per-session with `CARGO_PROFILE_DEV_DEBUG=2`, `CARGO_PROFILE_TEST_DEBUG=2`,
`CARGO_PROFILE_TEST_OPT_LEVEL=0`), and `.gitattributes` forces LF because
canonical source and evidence identities are byte-sensitive.

## Hard rules

- **Never push to `main` directly.** Reserve and publish through
  `tools/landing.py` ([tools/landing.md](tools/landing.md)); history is
  linear — rebase onto `main` and fast-forward, never merge
  (`git config pull.rebase true`).
- Before editing shared paths, check `python tools/claims.py status` for
  live session assignments and claim the work ([tools/claims](tools/claims.md)).
- Keep sample coverage out of the shipped CLI.

## Design references

- [Omega Language Guide](wiki/language_guide/language_guide.md)
- [Compiler ownership and pipeline](omega-rust/pipeline.md)
- [Documentation index](wiki/README.md)
- [Terminal Psi product contract](wiki/spec/terminal-psi/product.md)
- [Optimization phases](wiki/spec/build/optimizations.md#phase-and-product-boundaries)
- [Rust compiler completion](wiki/drafts/reference/rust_compiler_completion.md)
