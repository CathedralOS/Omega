# CI rebuild plan

On 2026-11-12 nothing merged for more than four hours, so the old workflows
and branch rules were removed. This is the plan for what replaces them. The
goal: a pull request merges in about a minute, main is verified continuously
after merge, and a break on main is found, attributed and reverted without a
person or a session spending time on it.

## What went wrong, measured

- **Every job rebuilt the compiler** (about 8.5 minutes cold). A pull request
  built it in `compiler-check`, `workspace-clippy` and each of eight corpus
  shards, then all of it again in the merge queue.
- **The repository gets only a few hosted runners at a time**; 100 queued runs
  behind 2 to 6 running was normal that night.
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

### 2. After merge: `post-merge-check`, continuous, one run at a time

Triggered by pushes to main; a new push while one is running waits and the
newest pending run replaces older ones, so a burst of merges is verified as
one batch.

1. **Build once**: compile `omega`, run Clippy (`--workspace --all-targets`),
   unit tests and `check-boundary`. Upload the binaries.
2. **Verify the batch**: shards download the binaries and run the tests and
   samples the batch's changed paths can reach, with a fixed smoke set.
3. **Attribute and repair**: when a batch breaks the build or a check, the
   post-merge check bisects the batch with the already-shared build cache,
   names the pull request, and opens a revert PR that `admission` passes in a
   minute. Fix-forward is the author's choice; the default is that main is
   green again within one post-merge run.

## Speed work, in order of payoff

1. **Self-hosted runners on the machines we already own** (`omega-linux`,
   the Windows and macOS test hosts). Persistent `target/` and warm caches
   make the post-merge check's build incremental (about a minute, not 8.5), and they do
   not share GitHub's hosted-runner limit. Hosted runners stay for
   `admission` only.
2. **Build once per run**, shared as an artifact with a receipt (revision,
   tree, host, digest); a shard refuses a binary whose receipt does not match.
3. **`sccache`** keyed on `Cargo.lock` for any hosted build that remains.
4. **Every workflow has a concurrency group and `max-parallel`.**

## Repository settings (owner action, once)

- Pull requests: squash merging only.
- One ruleset on `main`: require `admission`; no merge queue.
- Register the owned machines as self-hosted runners.

## Targets

| Measure | Target |
| --- | --- |
| Pull request ready to merged | under 2 minutes |
| Merge to post-merge verdict | under 20 minutes |
| Main red to revert PR open | one post-merge run |
| Hosted-runner minutes per pull request | under 2 |

## Order of work

1. `admission` workflow and the ruleset that requires it.
2. `post-merge-check` with build-once, verification and attribution; hosted
   runners first, self-hosted when registered.

Each step is one pull request and is useful without the later ones.
