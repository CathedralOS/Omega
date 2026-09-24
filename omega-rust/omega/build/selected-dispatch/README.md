# Selected execution dispatch

Start at [selected_dispatch.rs](src/selected_dispatch.rs) for the atomic execution
settlement used by the compiler. It plans operator and float rewrites, builds
and validates their Unit applications together, applies them to a staged
program, then publishes the program and source-query journal. Failures leave
the caller's program unchanged. A no-rewrite path updates comparison facts
only when needed, without an unconditional scratch clone.

The subordinate [operator](src/selected_dispatch/operator_adapter.rs),
[float](src/selected_dispatch/float_intrinsic.rs), and
[comparison](src/selected_dispatch/float_comparisons.rs) implementations live
beneath that owner. The two settlement entrances deliberately differ:
compiler publication retains source edits for later semantic queries; the
transformation-only entrance does not provide that evidence.

Related operations have separate entrances because they do different work:

- [Boundary dispatch](src/boundary_dispatch.rs) associates exact checked calls
  with selected adapters without rewriting their source meaning.
- [Intrinsic review](src/intrinsic_review.rs) retains accepted semantic bindings;
  [intrinsic classification](src/compiler_intrinsic.rs) checks exact selected rows.
- [Source edits](src/source_edits.rs) validates and reconstructs the source view
  from the settled tree. It is not an unchecked undo log.
- [Service custody](src/service_custody.rs) checks Fused service relationships
  and derives program-entry establishments.

The [compiler phase transition](../../compiler/checked-compilation/src/checking/phase_transitions.rs)
sequences settlement with review and publication. This crate does not choose
providers or acquire authority from imports.
