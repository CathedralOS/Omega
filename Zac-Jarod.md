# Jarod's questions for Zac

## Should a paused task stop an entire `advance` invocation?

An `advance` invocation selected an already-paused bootstrap task, reread its
known evidence, and stopped to ask me whether to continue or defer that strategy.
There was other compiler work on the boards. No implementation was delivered.

My expectation is that `advance` advances the compiler. A blocked or paused task
is not a reason to stop an unrestricted invocation: the agent must select other
actionable work. If a task needs an owner decision, the question belongs in
`OWNER_QUESTIONS.md`, the task references that dependency, and the agent continues
elsewhere. Ordinary engineering choices should not become requests for me to
prioritize implementation details. Reconfirming a known pause does not satisfy
`advance`.

The instructions at the time permitted a different outcome:

- `.agents/skills/advance/SKILL.md`, opening contract: “One invocation delivers a
  bounded compiler improvement through publication, or an evidence-backed scope
  pause.” This makes a pause an alternative deliverable.
- `AGENTS.md`, **Scope checkpoints**: after two supporting-machinery-only
  milestones, “pause implementation” and propose continuing, simplifying, or
  deferring “for the user's direction.” It does not explicitly limit the pause
  to that task or require selection of other actionable work.
- `AGENTS.md`, **Scope checkpoints**: “Scope/prioritization questions belong in
  the conversation; only actual owner language/architecture decisions belong in
  `OWNER_QUESTIONS.md`.” This creates a separate conversational escalation route.
- `.agents/skills/advance/SKILL.md`, **When the slice cannot close**: a scope pause
  requires asking for prioritization, with a warning not to evade the pause by
  changing helpers. It does not clearly distinguish continuing the paused
  strategy from selecting independent work.

The skill also says that reconfirming a known blocker is verification-only, not
a completed advance. The agent's behavior failed that distinction. The missing
selection rule nevertheless makes this failure easier: a fresh invocation is
not explicitly required to skip already-blocked or paused tasks.

Questions:

1. Should scope pauses apply only to the affected task or strategy, rather than
   end an unrestricted `advance` invocation?
2. Should a fresh `advance` skip already-paused or blocked tasks and continue
   selecting work until it finds an actionable slice?
3. Do we want conversational scope/prioritization questions as a separate
   escalation category? If so, when should they actually interrupt work rather
   than leave that task deferred while the agent continues elsewhere?
4. Should an unrestricted invocation stop without implementation only when no
   actionable work remains, while a user-restricted assignment may stop on its
   specific missing dependency or decision?
5. Can we preserve the prohibition on repeatedly adding unjustified helper
   machinery while explicitly allowing independent work on another task?

Jarod's requested direction: keep the anti-scaffolding safeguard, make pauses
task-local, route actual owner decisions to `OWNER_QUESTIONS.md`, and require
continued selection elsewhere. `advance` is a request for progress, not an
invitation to select a known blocker and return it to the user. Stopping an
unrestricted invocation requires evidence that no actionable work remains, not
merely that one selected task cannot proceed. The questions above are about
aligning the instructions with that expectation; `AGENTS.md` and the `advance` skill now implement task-local pauses and continued
selection. These questions retain the context for discussion with Zac.
