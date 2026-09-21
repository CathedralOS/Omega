# Terminal operation-level trap crash site — profile row proposal

Design record for **NEW-APR-TRAP-CRASH-SITE-PROFILE-PROPOSAL** (TASKS.md,
planner-scoped draft). Feeds OWNER_QUESTIONS.md question 3
(`terminal-operation-level-trap-crash-site`), which the DESIGN-BLOCKED
Terminal Trapping family legs wait on. Audited at `12ea4941eb` on linux
x86-64.

## Problem

The language side is settled: an executable `Trapping` operation owns a
crash site — `wiki/spec/terminal-psi/structural_predicates.md:71` requires
it to carry "its primitive denotation and path-conditioned crash site",
and `wiki/spec/language/effects.md:261` states the body operation creates
the crash site under its compiler-defined denotation. What is not settled
is the Terminal form of that site.

`omega.terminal.observation-profile.v1` enumerates the reconstructed
profile as a closed, ordered row list (`observations.md`), and only two
groups carry a crash:

- group 3 — crash sites ordered by machine, block, and **edge**, with
  exact closed cause (`terminal_trace_v1.rs:232` keys `(MachineId,
  BlockId, EdgeId)`).
- group 4 — boundary crash sites ordered by machine, block, operation,
  and cause, scoped to every declared route of every `BoundaryCall` and
  keyed by the boundary's exact public identity and route bucket
  (`terminal_trace_v1.rs:236`).

A trapping `a + b` has neither an edge nor a boundary identity, so no
admitted encoding exists for its site — and `observations.md:59-60`
forbids observing it on a fabricated terminator edge. Until a row shape
is chosen, `expression_preparation` keeps refusing `TrappingShiftLeft` /
`TrappingShiftRight` ("checked trapping operation requires runtime policy
realization") and the five remaining Trapping arithmetic operators keep
their no-fact boundary.

## Constraints the chosen shape must satisfy

1. The row list is **closed and ordered** — any new shape is a profile
   version move (`omega.terminal.observation-profile.v1`), not a silent
   field add. Consumers decode by group tag; unknown groups reject.
2. No fabricated terminator edge (observations.md:59-60).
3. The site must be path-conditioned: the same Trapping operation under a
   different guard path is a different crash site (structural_predicates
   :71). A plain `(machine, block, operation)` triple is only sufficient
   if operation identity already fixes the path context — see option A's
   caveat.
4. Cause stays exact and closed, as in groups 3 and 4.

## Candidate row shapes

**A. Widen group 3 to a machine/block/site key.** Re-key group 3 from
`(MachineId, BlockId, EdgeId)` to `(MachineId, BlockId, SiteId)` where
`SiteId` is a sum of edge coordinate or operation coordinate.
- Pro: one crash-site table; edge-vs-operation distinction stays
  structural rather than a second list.
- Con: changes an existing group's key — a heavier version move than an
  append; every consumer of group 3 re-reads a new encoding. A
  path-conditioned operation site also needs the *path* recorded (the
  guarding branch identities), so `SiteId` cannot be a bare operation id
  unless the path context rides alongside.

**B. Widen group 4 beyond `BoundaryCall`.** Admit non-boundary crash rows
into group 4, dropping its boundary-identity/route-bucket payload for
them.
- Pro: reuses the operation-level keying (machine, block, operation,
  cause) the group already proves.
- Con: group 4's definition is "every declared route of every
  BoundaryCall" — widening it rewrites the group's meaning rather than
  extending it, and loses the boundary-identity uniformity consumers
  rely on. Weakest provenance of the options.

**C. Append a fifth crash-bearing group.** A new ordered row group —
operation crash sites ordered by machine, block, operation, path context,
and cause — scoped to executable operations that own a crash site without
an edge or boundary (today: Trapping arithmetic).
- Pro: groups 3 and 4 keep their exact shapes; the append is the least
  invasive version move — existing decoders reject the unknown group
  cleanly rather than misreading a re-keyed one. Path context gets an
  explicit field instead of being folded into a sum key.
- Con: a second operation-keyed table beside group 4; the schema grows
  one group for what is currently a narrow family.

**D. No new site — observe at the aborting state.** An operation-level
trap already terminates under the machine's `crashes` domain; the crash
could be observed through the existing state/exit surface with no profile
row.
- Pro: zero schema movement.
- Con: does not satisfy the structural-predicates requirement — the spec
  requires the operation's *own* crash site with primitive denotation and
  path conditioning, which a state-level observation cannot carry. Listed
  for completeness; it is not a conforming answer.

## Recommendation space (owner decision — not chosen here)

The row's own text says resuming means "choosing a profile row shape and
moving `omega.terminal.observation-profile.v1` with it". On profile
mechanics alone, **C** is the least-disruptive conforming shape: closed
ordered lists gain a trailing group, groups 3–4 are untouched, and the
path context is explicit. **A** is viable if the owner prefers one
crash-site table and accepts the wider key change. **B** weakens group
4's contract; **D** fails the spec.

## What unblocks after a choice

1. `terminal_trace_v1` profile rows + codec encode/decode + interpreter
   support for the chosen shape (the group-4 leg's tagged-row machinery
   is the model).
2. `checked-trees-to-lowered-psi/expression_preparation` admits
   `TrappingShiftLeft`/`TrappingShiftRight` once their crash site has an
   admitted encoding; the five remaining Trapping arithmetic operators
   follow the same path (their check-stage refusal pin is
   `argument_cast_policies_follow_the_callee_parameter_domain`).
3. Package-evidence vocabulary tags already exist for the shift kinds
   (22–23); the remaining Trapping family needs tags in the same style.

## Out of scope

- Signed/mixed modular and saturating conversion spellings (separate
  Lowered Psi vocabulary gaps on the same row).
- Any choice of the named decision itself — this draft enumerates the
  conforming space for the owner.
