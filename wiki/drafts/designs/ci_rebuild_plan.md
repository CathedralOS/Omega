# CI rebuild plan

On 2026-11-12 nothing merged for more than four hours, so the old workflows
and branch rules were removed. This is the plan for what replaces them. The
goal: a pull request merges in about a minute, main is verified continuously
after merge, and a break on main is found, attributed and reverted without a
person or a session spending time on it.

## What went wrong, measured

- **Every fixture re-checks the bundled std library**, 55 to 117 seconds on a
  2-CPU hosted runner (comment in the old `compiler-regression.yml`). This is
  why the full corpus costs about 26 runner-hours and why fixtures time out.
- **Every job rebuilt the compiler** (about 8.5 minutes cold). A pull request
  built it in `compiler-check`, `workspace-clippy` and each of eight corpus
  shards, then all of it again in the merge queue.
- **The repository gets only a few hosted runners at a time**; 100 queued runs
  behind 2 to 6 running was normal that night.
- **Every pull request edits one file**, `tests/omega/corpus_outcomes.txt`, so
  a slow queue turns into ledger merge conflicts for whoever waits.
- **Nothing bounded load**: bots dispatched a four-target smoke run every five
  minutes, each in its own concurrency group.

## Design

### 1. Before merge: `admission`, about one minute, the only required check

No compiler build. A prebuilt `xtask` binary (restored from cache, rebuilt
only when `tools/xtask` changes) runs:

- `check-rules`: title, body, provenance line, file-count and path rules;
- `fmt --check` when Rust files changed.

Squash merge only. No merge queue: with a one-minute check there is nothing
to queue for.

### 2. After merge: `main-keeper`, continuous, one run at a time

Triggered by pushes to main; a new push while one is running waits and the
newest pending run replaces older ones, so a burst of merges is verified as
one batch.

1. **Build once**: compile `omega` and the corpus runner, run Clippy
   (`--workspace --all-targets`), unit tests and `check-boundary`. Upload the
   binaries.
2. **Verify the batch**: shards download the binaries and run the corpus rows
   and samples the batch's changed paths can reach (the selection logic from
   the old `corpus-gate plan` is kept), with a fixed smoke set.
3. **Record**: the keeper, not the pull request, writes the ledger. It commits
   moved rows to `corpus_outcomes.txt` directly, so authors stop recording and
   stop conflicting on that file.
4. **Attribute and repair**: when a batch makes rows worse or breaks the
   build, the keeper bisects the batch with the already-shared build cache,
   names the pull request, and opens a revert PR that `admission` passes in a
   minute. Fix-forward is the author's choice; the default is that main is
   green again within one keeper cycle.

### 3. Nightly: `full-sweep`

The whole corpus, all samples, and native runs on the four hosted targets,
build-once, off-peak. Its ledger commit is the baseline the keeper compares
with.

### 4. On demand: `qualify`

`workflow_dispatch` with an explicit target list, default `linux_x86_64`,
one run per dispatching account, newest pending replaces older.

## Speed work, in order of payoff

1. **Check std once per run, not once per fixture.** The corpus runner checks
   the bundled library once, keeps the checked artifact, and every fixture
   reuses it. This removes most of the 26 runner-hours and the fixture
   timeouts. It is the largest single win and is compiler-harness work, not
   workflow work.
2. **Self-hosted runners on the machines we already own** (`omega-linux`,
   the Windows and macOS test hosts). Persistent `target/` and warm caches
   make the keeper's build incremental (about a minute, not 8.5), and they do
   not share GitHub's hosted-runner limit. Hosted runners stay for
   `admission` only.
3. **Build once per run**, shared as an artifact with a receipt (revision,
   tree, host, digest); a shard refuses a binary whose receipt does not match.
4. **`sccache`** keyed on `Cargo.lock` for any hosted build that remains.
5. **Every workflow has a concurrency group and `max-parallel`.**

## Repository settings (owner action, once)

- Pull requests: squash merging only.
- One ruleset on `main`: require `admission`; no merge queue.
- Register the owned machines as self-hosted runners.

## Changes to agent rules

- Authors do not run `corpus --record` or edit `corpus_outcomes.txt`; the
  keeper records. The `Gate: ... | red rows:` line states the expected change
  and the keeper checks it after merge.
- `AGENTS.md`, "Spec first, land first" already covers the rest.

## Targets

| Measure | Target |
| --- | --- |
| Pull request ready to merged | under 2 minutes |
| Merge to keeper verdict | under 20 minutes |
| Main red to revert PR open | one keeper cycle |
| Full corpus sweep | under 2 hours |
| Hosted-runner minutes per pull request | under 2 |

## Order of work

1. `admission` workflow and the ruleset that requires it.
2. `main-keeper` with build-once, verification and attribution; hosted
   runners first, self-hosted when registered.
3. Keeper-owned ledger; remove author recording from `AGENTS.md`, the
   `advance` skill and `check-rules`.
4. Std-once in the corpus runner.
5. `full-sweep` and `qualify`.

Each step is one pull request and is useful without the later ones.
