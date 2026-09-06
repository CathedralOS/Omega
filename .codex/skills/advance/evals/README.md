# Evaluating the `advance` skill

`advance` is a loop driver: it picks work off a board, builds it, gates it, and
pushes to `main`. Evaluating it means running real work sessions, so the harness
exists mostly to keep those sessions from touching anything real.

The material below records the historical full-session A/B harness and its
results. Its explicit `baseline` and `regate` commands run full validation;
they are not prerequisites for ordinary advancement or a small skill edit.
Current evaluation criteria live in `evals.json` and assess scoped validation,
result reuse, and refusal to take on unrelated failures. They have not been rerun.

## Worktrees, and how to make them safe

Use worktrees, under `.claude/wt/` — `.gitignore` already reserves `.claude/*`
for exactly this. Do not put eval checkouts elsewhere on the drive.

Two hazards, both measured rather than assumed:

**A worktree pushes to the real remote unless you override `pushurl`.** Setting
`remote.origin.url` in worktree config does NOT override the inherited URL —
remote URLs are multi-valued, so it *adds* a second push target and a push goes
to both. Verified: `git push --dry-run` resolved to the real forge and would
have created the branch there. The mechanism that works is `pushurl`:

```sh
git config extensions.worktreeConfig true
git worktree add -B eval-a .claude/wt/a HEAD
git clone --bare . .claude/wt/oa.git && git -C .claude/wt/oa.git remote remove origin
git -C .claude/wt/a config --worktree remote.origin.pushurl "$(pwd)/.claude/wt/oa.git"
git -C .claude/wt/a push --dry-run origin eval-a    # ALWAYS verify before spawning
```

Never spawn an agent into a worktree without that dry run. The skill under test
pushes; a misconfigured remote ships eval junk to the real repository.

**Two worktrees cannot both hold `main`.** `refs/heads/main` is repo-global and
the primary checkout holds it, so each arm runs on its own branch (`eval-a`,
`eval-b`). Tell the agents this in the prompt or they waste turns on a checkout
that cannot succeed. Consequence for grading: the skill's "work directly on
`main`" instruction is untestable in a worktree, so `stayed_on_main` and
`pushed_to_origin` measure the harness, not the skill. Use clones instead if
those are the assertions you care about.

**`cargo fmt --all -- --check` cannot run from a worktree here.** The path is
~46 characters against the repo root's 32, and gate 1 dies with `os error 206`
— the Windows command-line limit, not a formatting failure. The root barely
clears it, so no subdirectory name is short enough to help. Tell both arms it is
an environment limit so neither burns its budget on it, and grade the remaining
four gates. The per-package form (`cargo fmt -p <crate> -- --check`) does work.

## Run it

```sh
cd .claude/skills/advance/evals
sh harness.sh setup ../SKILL.md none          # arm a = skill, arm b = no skill
```

For a new-vs-old comparison, pass the previous `SKILL.md` as the second argument
instead of `none`. Snapshot it *before* editing, or the comparison is null.

`setup` prewarms the shared mbx action cache from both clones. That is a cold
build of a 111-crate
workspace; wait for `prewarm done` in `$WORK/prewarm-{a,b}.log`. Skipping it means
the agents spend their time box compiling instead of working.

```sh
sh harness.sh baseline                        # which gates are ALREADY red
```

Then, per eval, generate both prompts and diff them before spawning:

```sh
sh harness.sh prompt a "advance the compiler, pick up the next piece of work" > /tmp/pa
sh harness.sh prompt b "advance the compiler, pick up the next piece of work" > /tmp/pb
diff /tmp/pa /tmp/pb                          # only path + skill paragraph may differ
```

Spawn both agents in the same turn, one per clone. When they finish:

```sh
sh harness.sh collect a <run-dir>/outputs
sh harness.sh regate  a <run-dir>/outputs     # ONE SIDE AT A TIME, machine idle
```

Then grade by hand against `evals.json`: the mechanical assertions come from
`_refs.txt`, `_commits.txt`, `_board_diff.txt`, and `_regate.txt` under the
output directory, and the judgement assertions from `report.md`. There is no
grader script; this repository is Rust, Omega, and `sh`, and a grader is not a
reason to add another language.

```sh
sh harness.sh reset a          # between evals; keeps the warm target/ dir
sh harness.sh clean            # deletes the whole work root
```

## Things that cost a run to learn

**Subagents are not sandboxes.** They isolate context, not the filesystem. Every
subagent inherits the *session* scratchpad, so concurrent agents overwrite each
other's scratch files — and one agent will see another's work appear mid-run.
`harness.sh prompt` therefore assigns each arm its own scratch directory and says
so explicitly.

**Worktrees share the build cache, and agents can see each other through it.**
Both arms run against one `mbx` action cache, so a concurrent arm's artifacts are
visible to the other. Observed: an agent reported cache warnings naming source
files that did not exist in its own worktree — they were the other arm's, working
the same board task — and correctly inferred "another checkout is mid-flight on
this entrance." Harmless for gate timing, but it is a real channel between arms
that are supposed to be independent, and it also means a shared cache can make
one arm's build cheaper than the other's. Separate caches per arm if the measure
is timing.

**Both arms must come from one template.** A stray adjective decides your result.
Saying pushing is "safe and expected" in the skill arm and merely "safe" in the
baseline confounded `pushed_to_origin` for a whole iteration. `harness.sh prompt`
emits both from one function; always diff before spawning.

**Never compare against `origin/main`.** The skill pushes, so after a successful
run `origin/main == HEAD` and a `origin/main..HEAD` range is empty — which reads
as "landed no commits". Everything anchors on the staged base commit instead,
found by its marker subject.

**Re-run gates one side at a time, machine idle.** Two concurrent `mbx test`
suites make load-sensitive tests fail spuriously. Doing this produced a
`job_aggregate_cpu_limit_terminates_the_job` failure that was purely a
measurement artifact.

**The lib gate used to fail fast**, stopping at the first failing crate and
hiding the rest — 7 of 110 lib targets on this host, with every `psi_*` crate
among the 103 that never ran. `AGENTS.md` now carries `--no-fail-fast`, so the
gate matches the coverage it always claimed; keep it on any command you add
here. On a Windows host without symlink privilege,
`cache_lock_open_does_not_follow_a_preexisting_symlink` fails deterministically
(OS error 1314) — the repo is red before any agent touches it, and grading has to
account for that or "gate honesty" is meaningless.

**Agents write their report inside the clone** (`EVAL_REPORT.md`), and `collect`
moves it out. Writing directly to a path outside the working directory gets
refused by the permission classifier, and the report is then stranded in the
agent's final message.

**Re-gate before you reset.** `regate` runs against whatever `HEAD` the clone is
sitting on, so resetting for the next eval destroys the tree you meant to verify.
The commits survive in the clone's object store, so a missed re-gate can be
recovered with `git checkout <sha>` — but only if you noticed. Collect, re-gate,
*then* reset.

**`pgrep` does not exist in git-bash.** Waiting on `! pgrep -f ...` is vacuously
true and your wait returns immediately. `regate` writes a `REGATE_COMPLETE`
sentinel; wait on that.

**Keep the work root short — two different limits bite.** Windows caps paths at
260 characters and this repo's deepest source paths are ~120, so a long root
fails the clone with "Filename too long". Separately, `cargo fmt --all -- --check`
expands to one rustfmt invocation naming every file in 111 crates; past roughly a
40-character root that command line exceeds the 32 KB process limit and the gate
dies with `os error 206`, which reads like a formatting failure and is not one.
Measured: the repo at a 32-character root passes, a clone at 45 characters fails.
The default `%LOCALAPPDATA%\Temp\omega-eval` is already over that line — set
`OMEGA_EVAL_ROOT` to something short if you need gate 1 to actually run. Note the
consequence for grading: a work root that breaks a gate the real repo passes is a
confound you introduced, not a property of the code under test.

## What discriminates

From iteration 1 (2 evals, skill vs no skill, n=1 per cell), the skill earned:
pushing rather than leaving work local, staying on `main`, running the whole gate
list, and explaining board decisions rather than silently leaving the board alone.
The sharpest single result was the side-quest eval: with the skill, an agent told
"the diagnostic wording is inconsistent, might be worth a pass" declined it, found
the canary suite actually red with 26 drifted cases, and filed a board line;
without it, the agent rewrote emphasis in five `expected.txt` files.

Both arms got these right with `AGENTS.md` alone, so they measure nothing: commit
subject style, board changelog hygiene, orienting before editing, honest gate
reporting, and task provenance. Do not read a skill win from them.

## Know what your sample size can resolve

An A/B on two wordings of the same paragraph was attempted and abandoned, and the
reason generalises. Run-to-run variance here is large: two agents given the same
prompt on the same tree picked the same task and produced near-identical designs,
but took 16 and 33 minutes. Any effect smaller than that spread — and most
single-paragraph wording changes are — cannot be separated from noise at n=3.

Before building a comparison, ask what effect size you expect and whether the
runs you can afford could detect it. If the answer is no, do not run the
comparison; either accept the change on its face when it is not plausibly worse,
or find an assertion where the arms behave categorically differently. The
side-quest eval discriminated because one arm edited five files the other did not
touch at all. Gate *timing* did not, because both arms reached correct
attribution by different routes.
