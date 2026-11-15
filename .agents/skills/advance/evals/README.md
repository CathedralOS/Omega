# Evaluating advance

[evals.json](evals.json) holds the behavior scenarios. An edit to this skill
needs checks of its frontmatter, links and agreement with other rules, not a
compiler advance or full workspace run. The current scenarios have not been run
against this revision.

## Review without executing compiler work

Check that an advance request keeps working for its customer (the outcome
the work is for) while there is work to do, and delivers it, not just a helper
step. A paused approach must be rethought before resuming, or the agent picks
other work in the user's scope; a pause is not an automatic stop or a permanent
ban. Scenarios 16 and 17 separate a paused helper approach from an outcome the
agent already owns. Shared files alone are not an ownership conflict. Stopping
without progress needs evidence that nothing in scope is actionable or a
prerequisite blocks it. Also check requests that should not
invoke this skill: a named bug, a code explanation, and an edit to this skill
must not pick board work or authorize publishing. Judge outcomes, not wording.
A rule AGENTS.md already imposes is a control, not evidence the skill helped.

Scenario 24: a compiler layer drops a requirement from a value it produces.
The fix records the requirement list on the value for the next stage to read,
rather than recognizing one production. Scenarios 18–23 test choosing work by
capability: priority versus sample size, continuing after a witness lands, a
missing witness, acceptance that already passes, an explicit sample scope, and
new work from `TASKS.md` rather than another count or width of an existing
example. Agents pick work from `TASKS.md` and failing `samples/` programs;
`samples/cli/facts/` is frozen. Review the next completed assignments: did they
remove requirements from their tasks, keep earlier behavior working, and move
on to what remains? A task ID on unrelated sample work is not success. If that
persists, revise the selection or dispatch guidance; do not add a tracker or
count new samples as closure. This workflow change is an unmeasured experiment.

Scenarios 13 and 14 test whether the agent writes down its reasoning: one
explanation is missing, the other already adequate. Look at the module top and
the decision site, not comment counts: can a new editor find the reason before
making the change? These are definitions, not run results.

## Full behavior trials

For a real session trial, give each arm (variant compared) a fresh
independent clone and a local throwaway bare origin. A worktree alone shares
branches and remotes with the real repository. Check every fetch and push URL
before starting an agent; none may reach the production remote. A local clone
is not an OS sandbox, so also limit the worker's filesystem and tool access.

On Windows and macOS, these Git commands create the isolated pair (use new
absolute paths):

```text
git clone --bare --no-hardlinks <source-checkout> <new-local-origin>
git -C <new-local-origin> remote remove origin
git clone <new-local-origin> <new-trial-checkout>
git -C <new-trial-checkout> remote -v
git -C <new-trial-checkout> remote get-url --all --push origin
```

Use a short path on Windows and record the path or command-line failures you
actually hit; other machines' limits do not apply. Do not reuse or
recursively clear an existing directory as setup. Trial worktrees stay within
the trial checkout under `.agents/worktree/`.

Copy the whole skill directory, including references, to
`.agents/skills/advance` in the trial clone. Record a committed base before the
worker starts. For a no-skill arm, remove only that skill and tell the worker not
to load it through aliases or global discovery; keep AGENTS.md and the other
skills. For before/after trials, use complete snapshots of each revision, not
one SKILL.md with mismatched references.

Give both arms the same prompt and acceptance; only paths and skills differ. Give each worker its own scratch directory. Follow AGENTS.md model routing.
Do not run simultaneous compiler builds on one host. Shared caches and cold
builds distort timing; record how you handled them before comparing times.

Record the worker's revision, host, commands and exit codes, diff, customer
result, publication state, remaining dependencies, and the instruction files it
read. Compare commits against the recorded base, not the moving origin/main.
Rerun the claimed checks on the tested commit before discarding anything. A
full baseline run is an explicit experiment choice under tools/docs/testing.md,
not a routine grading step.

Keep trial results out of the skill's instructions. Say whether you read
the source, checked structure, or ran a behavior trial. Do not infer faster work
from a few dissimilar tasks, or general success from one correct review. Old harness and trial notes are in Git history; the old shell harness no
longer runs.
