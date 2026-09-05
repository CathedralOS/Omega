# Evaluating the advance skill

Start with the mechanical contract, before spending a work session on an A/B:

```sh
sh .claude/skills/advance/scripts/verify-test.sh
```

These small Git fixtures use fake Cargo/mbx commands. They verify lane overlap,
changed-path containment, rename endpoints, reverted edits, accepted-commit
collisions, clean HEAD checks, and failure/missing/duplicate/stale gate evidence.
They do not establish that Omega builds. The production commands and their
ordering are in [admission.md](../references/admission.md).

## A real session comparison

`harness.sh` gives each arm its own clone and local bare origin. Use clones for
push-to-main evaluation: worktrees share refs and repository configuration.
Use short absolute paths, for example `OMEGA_EVAL_ROOT=/c/omega-eval`, and verify
both clones' remotes and `git push --dry-run` before dispatch. Setup refuses an
existing root; never repurpose another session's checkout.

```sh
export OMEGA_EVAL_ROOT=/c/omega-eval
sh .claude/skills/advance/evals/harness.sh setup .claude/skills/advance/SKILL.md none
sh .claude/skills/advance/evals/harness.sh baseline
```

Setup prewarms both clones sequentially in the foreground and returns nonzero
on a failed compiling command. A red baseline is recorded as red; it is not an
integration exception. Each `regate` uses the admission checker to save full
logs and five SHA-bound command exits under `gates/`; only all-zero completion
writes `gates/GREEN`. A completed process or a printed summary is not success.
Collect the untracked `EVAL_REPORT.md` before regating, so HEAD is clean.

Generate both prompts from the same template and diff them before dispatch:

```sh
sh .claude/skills/advance/evals/harness.sh prompt a 'advance the compiler' > /tmp/advance-a
sh .claude/skills/advance/evals/harness.sh prompt b 'advance the compiler' > /tmp/advance-b
diff /tmp/advance-a /tmp/advance-b
```

Only the arm paths and skill paragraph may differ. Each arm gets its own scratch
directory. Context isolation does not isolate files. Coordinate one build/gate
slot even when agents inspect or edit concurrently.

```sh
sh .claude/skills/advance/evals/harness.sh collect a /c/omega-eval/results-a
sh .claude/skills/advance/evals/harness.sh regate a /c/omega-eval/results-a
```

Collect, regate, then reset between evals. Reset is destructive to the selected
throwaway arm and its branches; inspect its resolved location and saved results
first. It retains ignored `target/`. Use a fresh result directory for each
regate. Do not grade against `origin/main`: a successful push makes that range
empty. Grade against the staged base recorded in `_refs.txt`, with `evals.json`
for behavioral assertions and independent gate logs for actual results.

## What prior measurements establish

- [Fixed-worktree measurement, 2026-09-05](fixed-worktree-2026-09-05.md) supports
  retaining local target artifacts at a stable short path between iterations.
  New worktree paths do not imply reusable workspace action-cache keys.
- On this Windows host, historical roots of 45 characters made the combined
  `cargo fmt --all` command exceed the process argument limit; the 32-character
  primary root passed. This is a path choice to fix, not a formatting-gate waiver.
- Concurrent suites have made the bounded-process CPU-limit test fail. Regate
  sequentially and attribute red tests with isolated probes instead of assuming
  they are unrelated.
- The full library gate must retain `--no-fail-fast`; otherwise one early crate
  failure prevents later crates from running.
- Separate caches per arm when comparing timing. Shared mbx cache entries make
  later arms cheaper and expose paths from other builds.

The historical behavioral comparison had one sample per cell. It distinguished
side-quest refusal and landing behavior, not small timing effects. Near-identical
agent tasks took 16 and 33 minutes; do not present an n=1 wording comparison as a
reliable speed result. The admission fixtures give categorical evidence for the
four mechanical rules without launching another pair of compiler sessions.
