# CI rebuild plan

On 2026-11-12 nothing merged for more than four hours. Every pull request and
every merge-queue entry rebuilt the compiler several times and re-ran up to
eight corpus shards on a handful of 2-CPU hosted runners, two bots dispatched a
four-target smoke run every few minutes, and entries timed out waiting for
runners. The old workflows were removed; this is the plan for what replaces
them. The aim is that CI never blocks landing for more than about a minute,
and that full verification happens after landing, not before.

## Principles

1. **One required check, about a minute.** It reads the diff and runs only
   checks that need no compiler build. Nothing else may block a merge.
2. **Everything heavy runs after merge.** Lint, tests, corpus, samples and
   native targets run on main and file a repair PR for what they find. Agents
   follow `AGENTS.md`, "Spec first, land first".
3. **Build once.** A post-merge run compiles once and shares the binaries.
4. **Bounded load.** Every workflow has a concurrency group and a
   `max-parallel`; no account can start unbounded runs.

## Stages

### 1. Required: `admission` (about one minute)

One job on `ubuntu-24.04`, no Rust build of the compiler:

- `cargo xtask check-rules` (title, body, provenance line, path and
  file-count rules), with the `xtask` binary restored from cache;
- `cargo xtask fmt --check` when Rust files changed (formatting needs no
  build).

That is the whole required set. Clippy, unit tests and `check-boundary` move
after merge.

### 2. After merge to main: `main-verify`

Triggered on push to main, one run at a time (`concurrency` with
`cancel-in-progress: true`, so a busy main measures only its newest head):

- a `build` job compiles `omega` and the corpus runner once, runs Clippy,
  unit tests and `check-boundary`, and uploads the binaries;
- shards (`max-parallel: 4`) run the corpus rows the merged diffs can reach and
  the sample gate, using the uploaded binaries;
- a report job opens or updates one ledger PR with moved rows, accepted
  `fail/` rows listed first, and publishes `observed.txt` for anyone who wants
  a baseline.

### 3. Nightly: `full-sweep`

Same build-once shape over every corpus row and sample, plus native runs on the
four hosted targets. Off-peak, `max-parallel: 4`.

### 4. On demand: `qualify`

`workflow_dispatch` with an explicit target list, defaulting to
`linux_x86_64`, one run per dispatching account at a time. Replaces the
qualification mode of Post-merge smoke.

### 5. Kept as they were

`warm-build.yml` (publishes the warm build cloud sessions start from) and
`pages.yml` (documentation site) are not checks and were kept.

## Repository settings (owner action)

- `main` ruleset, "Require status checks to pass": only `admission`.
- No merge queue: with a one-minute check, merge directly. If one is kept,
  build concurrency 4, group size up to 8, "require all queue entries to pass"
  off.

## Order of work

1. Land `admission` and make it the only required check (one PR, one settings
   change).
2. Land `main-verify` with build-once, including Clippy, unit tests and
   `check-boundary`.
3. Land `full-sweep` and `qualify`.
