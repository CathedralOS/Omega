# Evaluating advance

Use [evals.json](evals.json) for behavior scenarios. A skill edit needs metadata,
link, and policy consistency checks; it does not require a compiler advance or
full workspace run. Current scenarios have not been executed against this revision.

## Review without executing compiler work

Check that an advance request retains its customer and produces one bounded
improvement or a justified pause. Check nearby non-invocations too: a named bug,
a code explanation, and an edit to this skill must not independently select board
work or authorize publication. Use the assertions as outcome criteria, not required
wording. Existing AGENTS.md requirements are controls, not evidence of a skill win.

## Full behavior trials

When a real session trial is warranted, use a fresh independent clone and a local
throwaway bare origin for each arm. Worktrees alone share references and remote
configuration with the real repository. Inspect every fetch and push URL before
starting an agent; none may lead to the production remote. A local clone is not
an OS sandbox: also bound the worker's filesystem and tool access.

On Windows and macOS, Git can create the isolated pair with the same commands
(replace the placeholders with new absolute paths):

```text
git clone --bare --no-hardlinks <source-checkout> <new-local-origin>
git -C <new-local-origin> remote remove origin
git clone <new-local-origin> <new-trial-checkout>
git -C <new-trial-checkout> remote -v
git -C <new-trial-checkout> remote get-url --all --push origin
```

Use a short path on Windows and measure actual path or command-line failures;
historical checkout-length thresholds are not portable limits. Do not reuse or
recursively clear an existing directory as setup. Trial worktrees stay within
the trial checkout under `.codex/worktrees/`.

Stage the entire skill directory, including references, at
`.agents/skills/advance` in the trial clone. Record a committed base before the
worker starts. For a no-skill arm, remove only that skill and tell the worker not
to load it through aliases or global discovery. Preserve the same AGENTS.md and
other skills. For before/after trials, use complete snapshots from the respective
revisions, not one SKILL.md with mismatched references.

Use the same prompt and acceptance in both arms; only paths and skill selection
differ. Give each worker its own scratch directory. Follow AGENTS.md model routing.
Do not launch simultaneous compiler builds on one host. Shared caches and cold
builds confound timing; record their treatment before comparing elapsed time.

Capture the worker's revision, host, commands and exits, diff, customer result,
publication state, remaining dependencies, and actual instruction paths. Compare
commits against the recorded base, not the moving origin/main. Verify the claimed
checks at the tested candidate before discarding anything. Full baseline work is
an explicit experiment choice under AGENTS.md, not a routine grading prerequisite.

Keep run evidence outside the skill's reusable instructions. Distinguish source
review, structural validation, and executed behavior trials. Do not infer a timing
improvement from a small sample of dissimilar tasks, or general success from one
correct review. Historical harness and trial notes remain available in Git history;
the obsolete shell harness is no longer an executable workflow.
