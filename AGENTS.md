# AGENTS.md

Omega is a proof-checked systems language. The goal is a complete compiler
that ships. You own your task until it is on `main`.

## Where things are

- `wiki/spec/` is the language. It decides; drafts decide nothing.
- `omega-rust/psi/` turns source into Terminal Psi and interprets it. It is
  target-neutral and never imports `omega/`.
- `omega-rust/omega/` is build, packages, native backends and the CLI.
- `source/` is the compiler and library written in Omega; `bootstrap/` is the
  bootstrap chain.
- `TASKS.md` is the remaining work. `OWNER_QUESTIONS.md` holds questions only
  the owner can answer.
- Skills are in `.agents/skills/`. More detail: [pipeline](omega-rust/pipeline.md),
  [testing](tools/testing.md), [release gates](wiki/drafts/reference/rust_compiler_completion.md).

## How to work

1. **Pick work.** What the user asked for; otherwise a break on `main`;
   otherwise the top of `TASKS.md`.
2. **Read the spec page, then build what it says.** If the spec does not
   decide it, add the question to `OWNER_QUESTIONS.md` and take other work.
3. **Fix the cause.** Every program that shares the cause should pass, from
   checking through native execution (`omega run --both`). Keep the checker
   sound: make the program right, or the compiler right, never the check
   weaker.
4. **Check what you changed.** Build, run the fixtures and samples you
   touched, lint the crates you touched.
5. **Land it.** Open a pull request and squash-merge it yourself when
   `admission` passes. Delete the finished item from `TASKS.md`. `main` is
   checked after every merge; fix what that finds.

## Commands

Use `mbx` for every build, test and lint (`bash tools/agent_setup.sh` installs
it). Close stdin on `omega` and corpus runs with `< /dev/null`. Work in your
own worktree (`mbx xtask worktree acquire <lane>-<topic>`), not the primary
checkout.

```bash
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null
mbx xtask corpus --filter <fixtures> < /dev/null
mbx xtask sample-gate --filter <sample_path> < /dev/null
mbx clippy -p <crate> --all-targets -- -D warnings
mbx xtask fmt --check
mbx xtask check-rules --base origin/main --title "<lane>: <subject>" --body-file <body>
```

`check-rules` holds the mechanical limits (for example new `.omg` files per
pull request); run it and follow what it says. The corpus ledger
`tests/omega/corpus_outcomes.txt` is written on `main`, not in pull requests.

## Pull request text

Say in the first sentence or two, in plain words, what was wrong, what changed
and why it matters. Then the detail, then what you ran. End the description
with the line `mbx xtask provenance` prints. A compiler-lane pull request adds
`Gate: <release-gate> | red rows: <change>` (with `| unlocks: <what>` when no
row turns green).

## Commit naming

Title: `lane: statement`, lowercase, at most 85 characters, no trailing period.

| Lane | Covers |
| --- | --- |
| `alpha`, `beta`, `gamma`, `delta`, `epsilon` | That bootstrap rung in `bootstrap/<rung>/`. |
| `bootstrap-omega` | The first Omega compiler in `bootstrap/5_omega/`. |
| `bootstrap` | Cross-rung bootstrap work, `tests/bootstrap/`, `tools/bootstrap/`. |
| `psi` | Source semantics through Terminal Psi: `omega-rust/psi/`, `source/psi/`. |
| `omega` | The Terminal-Psi-consuming stage: `omega-rust/omega/compilation/`, `source/omega/`. |
| `backend` | Targets, ABI, encoding: `omega-rust/omega/compilation/backend/`. |
| `compiler` | Compilation coordination, or a change spanning Psi and Omega. |
| `build` | Build system and `build.omg`. |
| `packages` | Package acquisition, graphs, review. |
| `cli` | Command parsing, flags, help. |
| `tooling` | Shipped developer tools in `omega-rust/omega/foundation/`. |
| `library` | Bundled packages in `source/library/`. |
| `samples` | Sample content in `samples/`. |
| `tests` | Shared test infrastructure and corpus registration. |
| `tools` | Repository utilities in `tools/`. |
| `repo` | Workflow, `AGENTS.md`, skills, CI. |
| `docs` | Documentation in `wiki/`, READMEs, guides. |
