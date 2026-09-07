# Terminal realization integration

Temporary implementation note owned by Terminal production and native
realization. The [boundary contract](../spec/terminal-psi/boundary_calls.md)
and [byte-view contract](../spec/terminal-psi/byte_views.md) define the intended
behavior; this note does not add a second semantic authority.

The removed Terminal architecture history listed successful narrow cohorts
alongside missing generalization. Do not treat those historical cohorts as a
current support matrix. The remaining integration work is:

- Complete native immutable-view descriptors across calls, block transfers,
  indexing/subslicing, and eventual borrowed returns. Proof and natural-ranked
  interpretation do not establish native placement. See the existing structural
  borrow task and byte-operation lowering in Terminal-to-abstract operations.
- Carry installed-provider structural results and their projected residuals
  through native storage and publication, not merely conformance admission.
  Generalize scalar-provider forwarding only with preserved incoming ABI homes,
  complete call/relocation replay, and a genuinely reachable authored entry.
- Complete retained-product callback/checked-scope composition and Service
  forwarding through the existing callback and entry-root tasks. A passing
  provider-selection or zero-payload layout check is not executable custody.
- Replace the guard-plus-payload compiler-intrinsic conversion in
  [intrinsic_settlements.rs](../../omega-rust/omega/compiler/compiler/src/compiler/intrinsic_settlements.rs)
  with the contract's single exhaustive optional mapping. The present guard and
  wildcard-backed match do not force future planner variants to be classified.

Acceptance: each demanded route independently replays its complete semantics,
arguments/results, resource inputs, and native publication; unsupported routes
reject. Use the owning provider, byte-view, callback, and rooted native controls,
not a metadata-only receipt test. Delete each item when its end-to-end route is
verified, and delete this note once the items are completed or absorbed by the
corresponding implementation task. Do not append a chronology of passing slices.
