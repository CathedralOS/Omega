# Owner Questions

Only unresolved language or architecture decisions belong here. Settled
decisions live in the language guide and design briefs; implementation work
lives in `TASKS.md`. When a question is answered, remove it from this file after
recording the ruling in those authoritative homes.

Last pruned: 2026-07-18.

## Machine and linear surfaces

1. **Accepted proof supply spelling.** Decision 20 distinguishes checked
   bodies, requirements, external providers, and accepted declarations in the
   semantic artifact.

   Needed ruling: whether an accepted theorem remains a bodyless `boundary
   machine`, becomes `boundary fact`, or uses another spelling. Whatever the
   surface, trust expenditure must remain explicit and reportable.

2. **Linear terminal-consumer spelling.** `[linear]` and
   create/transfer/consume conservation are settled.

   Needed ruling: whether `move self` plus result contracts are enough for the
   checker to infer which outcomes discharge an obligation, or terminal
   consumers/outcomes require an explicit declaration. Also settle how an
   authorized `detach` visibly transfers a `Join<T>` obligation out of
   structured scope.

## Resources and components

3. **Resource algebra first customer and proof surface.** Owned splitting and
   merging (`LinBuf<T, n>`), quantitative resources, and attenuation require a
   conservation algebra beyond core multiplicity.

   Needed ruling: choose the first customer and proof surface before promising
   dependent owned-buffer splits or quantitative effect-row members. Whole
   ownership and borrowed views do not wait on this.

4. **Component versioning.** The leading design uses bounded multi-version
   coexistence, per-version activation pools and liveness pins, and import slots
   pinned to normalized machine-contract identities with deterministic
   refinement admission.

   Still needed: outbound-call semantics for old continuations, version
   budgets and eviction, linking mechanics, and the boundary between v1
   coexistence and later continuation migration.
