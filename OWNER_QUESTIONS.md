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

## Surface semantics

5. **Aggregate field defaults: support or reject.** Parked repro
   `canaries/pending/arithmetic/array_field_default_silent` (2026-07-05):
   an inline aggregate-literal FIELD DEFAULT (`xs: [i32;3] = [1,2,3]`, and
   presumably `Foo {..}`) is silently DROPPED at emission (reads see ZII),
   and its length/element class are UNVALIDATED (`[i32;2] = [1,2,3,4]`
   compiles). Scalar literal defaults and nested-record defaults both work.

   Needed ruling: (a) SUPPORT aggregate field defaults — emit inline
   aggregate literals + wire `validate_array_literal_elements` into
   data.rs's field loop; or (b) REJECT non-scalar field defaults with a
   clear diagnostic ("aggregate field defaults are not emitted; initialize
   in a machine body"). Per "no silent anything", the current
   silently-dropped-and-unvalidated state is the one indefensible option;
   engineering is ready to build either ruling.

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
