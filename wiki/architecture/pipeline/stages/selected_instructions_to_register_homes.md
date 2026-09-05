# Selected Instructions To Register Homes

[Pipeline](../pipeline.md) | [Optimization phases](../optimization_phases.md)

`selected-instructions-to-register-homes` owns selected-program analyses,
explicit pre-allocation rewrites, mandatory reanalysis, and register assignment.
These are internal parts of one allocation phase, not a crate per optimization.

`stage_register_allocation` executes ordinary assignment, selected lowering,
fixed-view recovery, or active-resident rematerialization and publishes the same
admitted allocation output. The owner retains the rules' exact availability,
budget, reanalysis, and assignment policies. Unsupported rule combinations
reject before execution. The outer coordinator no longer carries an
`after_selected_lowering` flag into machine optimization, and recovery followed
by a machine rule uses that same downstream path.

Its implementation areas are `analyses/`, `rewrites/`, `assignment/`, and
`output/`. The existing algorithms and independent replay remain separate;
consolidation does not replace validation with producer receipts.

The common downstream boundary is `AllocationOutput`: a temporary immutable
view of the current validated selected program, liveness, ranges, legality,
homes, manifest, target environment, and exact retained policy. A sealed
`AllocationSource` reconstructs the evidence before creating this view.
`AllocationEvidence` separately retains the role-specific replay receipt.
An existing view can be reused while its immutable input borrows remain live.

`RetainedAllocation` owns the admitted replay inputs when a later stage needs
to retain them. Its fallible construction replays the input before taking
custody; its private immutable history cannot be modified by consumers.
`current()` projects the same checked facts without rerunning analysis on each
access. Independent replay remains available at subsequent proof gates. The
retained target/proof input is separate from the current selected program.
The owner also reconstructs the exercised allocation-recovery selection from
replayed evidence and compares it with build policy; a copied selection alone
cannot establish completion.

Machine-plan construction and machine optimization accept this same boundary
after baseline allocation, fixed-view copies, literal folding, or pressure
rematerialization. Neither consumer walks upstream stage objects, looks for the
last rewrite step, or chooses an `after_*` entrypoint. Rule catalog selection
and target applicability still govern which machine rewrites may run.

Post-allocation function realization and fragment emission use this same
boundary. They do not carry a baseline/selected-lowering/recovery source enum,
or construct separate manifests according to allocation history. The manifest
binds the current allocation, selected policy, and actual optional completion.

Retained histories inside allocation and in the remaining layout/realization
adapters are transitional. Consolidation does not make those histories a new
canonical program representation or complete the outer pipeline convergence.

Plain recovery realization also owns `RetainedAllocation`, not a second
fixed-view/rematerialization source taxonomy. Its encoding, layout, exit, and
emission consume current facts; independent replay rechecks the exact recovery
role and selected transformations. A non-recovery allocation or an unexecuted
later selection cannot enter this plain realization. There is one coordinator
entrance for both recovery rules, with their choice owned by allocation.

## Internal analysis and rewrite contracts

- [Liveness](selected_instructions_to_liveness.md)
- [Live ranges](liveness_to_live_ranges.md)
- [Allocation legality](live_ranges_to_allocation_legality.md)
- [Fixed-view copies](allocation_legality_to_fixed_view_copies.md)
- [Reanalysis](fixed_view_copies_to_reanalyzed_legality.md)
- [Home assignment](allocation_legality_to_register_homes.md)
