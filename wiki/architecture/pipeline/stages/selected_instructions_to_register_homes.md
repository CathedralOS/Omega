# Selected Instructions To Register Homes

[Pipeline](../pipeline.md) | [Optimization phases](../optimization_phases.md)

`omega-selected-instructions-to-register-homes` owns selected-program analyses,
explicit pre-allocation rewrites, mandatory reanalysis, and register assignment.
These are internal parts of one allocation phase, not a crate per optimization.

Its implementation areas are `analyses/`, `rewrites/`, `assignment/`, and
`output/`. The existing algorithms and independent replay remain separate;
consolidation does not replace validation with producer receipts.

The common downstream boundary is `AllocationOutput`: a temporary immutable
view of the current validated selected program, liveness, ranges, legality,
homes, manifest, target environment, and exact retained policy. A sealed
`AllocationSource` reconstructs the evidence before creating this view.
`AllocationEvidence` separately retains the role-specific replay receipt.
An existing view can be reused while its immutable input borrows remain live.

Machine-plan construction and machine optimization accept this same boundary
after baseline allocation, fixed-view copies, literal folding, or pressure
rematerialization. Neither consumer walks upstream stage objects, looks for the
last rewrite step, or chooses an `after_*` entrypoint. Rule catalog selection
and target applicability still govern which machine rewrites may run.

Retained histories inside allocation and in the remaining layout/realization
adapters are transitional. Consolidation does not make those histories a new
canonical program representation or complete the outer pipeline convergence.

## Internal analysis and rewrite contracts

- [Liveness](selected_instructions_to_liveness.md)
- [Live ranges](liveness_to_live_ranges.md)
- [Allocation legality](live_ranges_to_allocation_legality.md)
- [Fixed-view copies](allocation_legality_to_fixed_view_copies.md)
- [Reanalysis](fixed_view_copies_to_reanalyzed_legality.md)
- [Home assignment](allocation_legality_to_register_homes.md)
