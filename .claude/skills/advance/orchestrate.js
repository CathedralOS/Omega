export const meta = {
  name: 'advance',
  description: 'One /advance iteration: rank the canary corpus, assign disjoint lanes, build in isolated worktrees, integrate only green commits onto main',
  whenToUse: 'Invoked by the /advance skill. Runs the whole loop with deterministic overlap and green-gate checks in the script, not in agent judgment.',
  phases: [
    { title: 'Rank', detail: 'run the canary corpus once in the main checkout, cluster failures by cause' },
    { title: 'Plan', detail: 'turn the top causes into N tasks on disjoint crate lanes' },
    { title: 'Build', detail: 'one agent per task in its own worktree; gate and commit there' },
    { title: 'Integrate', detail: 'refuse overlaps and red gates, cherry-pick the rest onto main, re-gate, push' },
  ],
}

// args: { agents?: number (default 2), board?: string (restrict to one board), skipRank?: boolean }
const AGENTS = Math.max(1, Math.min(4, (args && args.agents) || 2))
const BOARD = (args && args.board) || null
const REPO = 'C:/SoftwareDevelopmentKits/Omega'

const RULES = `
Read ${REPO}/AGENTS.md and ${REPO}/.claude/skills/advance/SKILL.md before anything else and follow both.
Use mbx for every compiling command. Never run the unfiltered canary suite to attribute one change; use the
filter variables documented under "Running one test" in AGENTS.md.
Never claim what you did not run: a cause you have not measured is a guess, a test you have not watched fail
is not coverage, and a gate you did not run is reported as not run.
`

const RANK_SCHEMA = {
  type: 'object',
  properties: {
    head: { type: 'string', description: 'git rev-parse HEAD of the main checkout the suite ran on' },
    dirty_entries: { type: 'integer', description: 'git status --porcelain | wc -l before the run' },
    suite: {
      type: 'object',
      properties: { passed: { type: 'integer' }, failed: { type: 'integer' }, seconds: { type: 'number' } },
      required: ['passed', 'failed'],
    },
    causes: {
      type: 'array',
      description: 'failure causes ranked by count, most common first',
      items: {
        type: 'object',
        properties: {
          count: { type: 'integer' },
          message: { type: 'string', description: 'the normalized diagnostic text' },
          board_entry: { type: 'string', description: 'the TASKS*.md entry that already owns this cause, or empty' },
          owning_crates: { type: 'array', items: { type: 'string' }, description: 'crate directories under omega-rust/ the fix lives in' },
        },
        required: ['count', 'message'],
      },
    },
  },
  required: ['head', 'dirty_entries', 'suite', 'causes'],
}

const PLAN_SCHEMA = {
  type: 'object',
  properties: {
    tasks: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          key: { type: 'string', description: 'short kebab-case name' },
          board_entry: { type: 'string' },
          goal: { type: 'string', description: 'what a green result looks like, concretely' },
          acceptance_probe: { type: 'string', description: 'the smallest command or fixture that must fail now for the stated reason and pass after' },
          lane: { type: 'array', items: { type: 'string' }, description: 'path prefixes this task may edit; disjoint from every other task' },
          canaries_unblocked: { type: 'integer' },
        },
        required: ['key', 'goal', 'acceptance_probe', 'lane'],
      },
    },
    rejected: { type: 'array', items: { type: 'string' }, description: 'candidate causes not assigned, and why' },
  },
  required: ['tasks'],
}

const WORKER_SCHEMA = {
  type: 'object',
  properties: {
    sha: { type: 'string', description: 'full 40-char commit SHA created in the worktree, or empty if nothing was committed' },
    worktree: { type: 'string' },
    files_changed: { type: 'array', items: { type: 'string' }, description: 'git diff --name-only <sha>^..<sha>' },
    probe_before: { type: 'string', description: 'what the acceptance probe reported before the change, verbatim' },
    probe_after: { type: 'string', description: 'what it reported after, verbatim' },
    gates: {
      type: 'object',
      description: 'each is the literal result line, or the string NOT RUN',
      properties: {
        fmt: { type: 'string' }, check: { type: 'string' }, clippy: { type: 'string' },
        architecture: { type: 'string' }, lib: { type: 'string' }, filtered_canary: { type: 'string' },
      },
      required: ['fmt', 'check', 'clippy', 'architecture', 'lib'],
    },
    all_green: { type: 'boolean', description: 'true only if every gate above was run and passed' },
    blocked: { type: 'string', description: 'if you stopped short, exactly why; otherwise empty' },
  },
  required: ['sha', 'files_changed', 'gates', 'all_green'],
}

const INTEGRATE_SCHEMA = {
  type: 'object',
  properties: {
    integrated: { type: 'array', items: { type: 'string' }, description: 'SHAs cherry-picked, in order, with their new SHAs on main' },
    pushed: { type: 'boolean' },
    main_head: { type: 'string' },
    gates: { type: 'object', properties: { fmt: { type: 'string' }, check: { type: 'string' }, clippy: { type: 'string' }, architecture: { type: 'string' }, lib: { type: 'string' } } },
    refused: { type: 'string', description: 'anything you declined to do and why' },
  },
  required: ['integrated', 'pushed', 'main_head'],
}

// ---------------------------------------------------------------- Rank
phase('Rank')
let rank = null
if (!(args && args.skipRank)) {
  rank = await agent(`${RULES}
You are the ranking step. Work in the MAIN checkout at ${REPO}. Change no files.

1. Report git rev-parse HEAD and git status --porcelain | wc -l. If the porcelain count is nonzero, list the
   dirty paths - those lanes belong to another session and the planner must avoid them.
2. Run the canary corpus once, capturing everything:
     mbx test -p omega-compiler --test canary_suite 2>&1 | tee /tmp/advance-rank.txt
   It takes roughly 8 minutes. Let it finish. Report the test result line.
3. Cluster the failures by cause with the pipeline from SKILL.md "Pick the work":
     grep -oE 'message: "[^"]{0,90}' /tmp/advance-rank.txt | sed 's/^message: "//' \\
       | sed -E 's/\`[^\`]*\`/X/g; s/[0-9]+/N/g; s/: .*$//' | sort | uniq -c | sort -rn | head -12
   For the top cause, also extract the detail it wraps (e.g. the Lowering(Unsupported(...)) text).
4. For each of the top causes, find the TASKS*.md entry that already owns it (grep the boards for the fence
   or diagnostic text) and name the crate directories under omega-rust/ where the fix lives.
Return the structured result. Counts must come from the run you just did, not from memory or commit bodies.`,
  { label: 'rank', phase: 'Rank', schema: RANK_SCHEMA })
  if (!rank) throw new Error('ranking agent returned nothing')
  log(`Rank: ${rank.suite.passed} passed / ${rank.suite.failed} failed at ${rank.head.slice(0, 10)}; top cause x${rank.causes[0] ? rank.causes[0].count : 0}`)
  if (rank.dirty_entries > 0) log(`Main checkout has ${rank.dirty_entries} dirty entries - another session is active; planner will avoid their paths.`)
}

// ---------------------------------------------------------------- Plan
phase('Plan')
const plan = await agent(`${RULES}
You are the planner. Work in the MAIN checkout at ${REPO}. Change no files.

Ranking result:
${rank ? JSON.stringify(rank, null, 2) : '(ranking skipped - derive tasks from the boards alone, and say so)'}

${BOARD ? `Scope is restricted to ${BOARD}; do not draw tasks from the other boards.` : 'All four boards are in scope.'}

Produce at most ${AGENTS} tasks. Rules:
- Prefer the cause that unblocks the most canaries. A task worth one canary is not the same size as one worth hundreds.
- Every task gets a lane: the path prefixes it may edit. Lanes MUST be pairwise disjoint. If two good tasks
  share a crate, keep the bigger one and reject the other with a reason. Never assign a lane containing a
  path that is dirty in the main checkout.
- Every task gets an acceptance probe: the smallest filtered canary or fixture compile that fails NOW for the
  reason the board gives. Write the exact command. If you cannot write one, the task is not ready - reject it.
- Read the owning board entry for each task and quote its acceptance condition in the goal.
- If a task exists on a board but the ranking shows its cause is already gone, say so in rejected.`,
  { label: 'plan', phase: 'Plan', schema: PLAN_SCHEMA })
if (!plan || !plan.tasks.length) throw new Error('planner produced no tasks')

// Deterministic disjointness check - do not trust the planner's word for it.
for (let i = 0; i < plan.tasks.length; i++) for (let j = i + 1; j < plan.tasks.length; j++) {
  for (const a of plan.tasks[i].lane) for (const b of plan.tasks[j].lane) {
    if (a.startsWith(b) || b.startsWith(a)) throw new Error(`lanes overlap: ${plan.tasks[i].key}:${a} vs ${plan.tasks[j].key}:${b}`)
  }
}
log(`Plan: ${plan.tasks.map(t => `${t.key} (${t.canaries_unblocked || '?'} canaries, lane ${t.lane.join(',')})`).join('; ')}`)
if (plan.rejected && plan.rejected.length) log(`Rejected: ${plan.rejected.join(' | ')}`)

// ---------------------------------------------------------------- Build
phase('Build')
const workers = await parallel(plan.tasks.map(task => () => agent(`${RULES}
You are building ONE task in an isolated worktree. Commit here. Do NOT push. Do NOT rebase. Do NOT touch
files outside your lane.

TASK ${task.key}${task.board_entry ? ` (board entry ${task.board_entry})` : ''}
GOAL: ${task.goal}
LANE (the only paths you may edit): ${task.lane.join(', ')}
ACCEPTANCE PROBE: ${task.acceptance_probe}

Procedure:
1. Run the acceptance probe FIRST and record its output verbatim. It must fail for the reason the board gives.
   If it passes, or fails for a different reason, stop: report that in "blocked" and commit nothing.
2. Do the work. Stay inside the lane. Prefer the smallest change that makes the probe pass; register any
   temporary state on the board in the same commit per SKILL.md "Debt gets registered on the way in".
3. Run the acceptance probe again and record its output verbatim.
4. If you added a fixture under tests/omega, add its roster entry in canary_suite.rs in the same commit, then
   prove the case runs AND fails with your change reverted (git stash or git show <sha>:<path>) - never claim
   coverage you have not watched fail.
5. Gate on YOUR worktree, every one of these, and record each literal result line:
     cargo fmt --all -- --check   (if that cannot spawn on this host, cargo fmt -p <each touched package> -- --check)
     mbx check --workspace --all-targets
     mbx clippy --workspace --all-targets -- -D warnings
     mbx test -p omega-architecture-test --all-targets
     mbx test --workspace --lib --no-fail-fast
     plus the filtered canary for what you changed.
   all_green is true ONLY if every one ran and passed. A gate you did not run is reported as NOT RUN.
6. If all green: commit with a message per AGENTS.md "Workflow" (lane: statement; body with prior behavior,
   gates with counts, known red). Delete or update the board entry in the same commit if its acceptance
   condition now passes. Report the full SHA and git diff --name-only <sha>^..<sha>.
   If not all green: commit nothing, report sha as empty, and say what is red.`,
  { label: `build:${task.key}`, phase: 'Build', isolation: 'worktree', schema: WORKER_SCHEMA })))

const results = workers.map((w, i) => ({ task: plan.tasks[i], w })).filter(r => r.w)
for (const r of results) {
  if (r.w.blocked) log(`${r.task.key}: blocked - ${r.w.blocked}`)
  else log(`${r.task.key}: sha=${(r.w.sha || '').slice(0, 10) || 'none'} green=${r.w.all_green} files=${r.w.files_changed.length}`)
}

// ---------------------------------------------------------------- Deterministic admission
const admitted = []
const refused = []
for (const r of results) {
  if (!r.w.sha || r.w.sha.length < 40) { refused.push(`${r.task.key}: no commit`); continue }
  if (!r.w.all_green) { refused.push(`${r.task.key}: gates not all green`); continue }
  const outside = r.w.files_changed.filter(f => !r.task.lane.some(p => f.startsWith(p)) && !f.startsWith('TASKS'))
  if (outside.length) { refused.push(`${r.task.key}: edited outside its lane: ${outside.join(', ')}`); continue }
  admitted.push(r)
}
// File-level overlap across admitted commits - the last line of defence before main.
for (let i = 0; i < admitted.length; i++) for (let j = i + 1; j < admitted.length; j++) {
  const shared = admitted[i].w.files_changed.filter(f => admitted[j].w.files_changed.includes(f) && !f.startsWith('TASKS'))
  if (shared.length) {
    refused.push(`${admitted[j].task.key}: overlaps ${admitted[i].task.key} on ${shared.join(', ')}`)
    admitted.splice(j, 1); j--
  }
}
log(`Admitted ${admitted.length}/${results.length}${refused.length ? `; refused: ${refused.join(' | ')}` : ''}`)
if (!admitted.length) return { rank, plan, results, integrated: null, refused }

// ---------------------------------------------------------------- Integrate
phase('Integrate')
const integrated = await agent(`${RULES}
You are the integrator. Work in the MAIN checkout at ${REPO}. You are the only thing that lands on main.

Commits to integrate, in this order (each was gated green in its own worktree and verified non-overlapping):
${admitted.map(r => `  ${r.w.sha}  ${r.task.key}  files: ${r.w.files_changed.join(', ')}`).join('\n')}

Procedure:
1. git status --porcelain. If it is nonzero on any TRACKED file, stop: another session is mid-edit here.
   Report refused and do nothing else.
2. git fetch, then git pull --rebase. If it refuses, stop and report why.
3. git cherry-pick each SHA in order. Worktrees share the object store, so the SHAs are reachable. If a
   cherry-pick conflicts, git cherry-pick --abort, report which one, and continue with the rest.
4. Re-run the full gate list on the integrated result and record each literal result line:
     cargo fmt --all -- --check (or per-package), mbx check --workspace --all-targets,
     mbx clippy --workspace --all-targets -- -D warnings, mbx test -p omega-architecture-test --all-targets,
     mbx test --workspace --lib --no-fail-fast
   If anything is red that was green in the worktrees, do NOT push; report it.
5. git push. If rejected non-fast-forward, git pull --rebase once and push again; if still rejected, report.
Report the SHAs as they now appear on main and git rev-parse HEAD.`,
  { label: 'integrate', phase: 'Integrate', schema: INTEGRATE_SCHEMA })

log(integrated ? `Integrated ${integrated.integrated.length}, pushed=${integrated.pushed}, main=${(integrated.main_head || '').slice(0, 10)}` : 'integrator returned nothing')
return { rank, plan, results, admitted: admitted.map(r => r.w.sha), refused, integrated }
