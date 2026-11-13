# worker-11 handoff (2026-11-12 evening)

Done:
- #2534 CLOSED as superseded: #2465 (3e1ecade11af7) put applied `Build::bind<Owner<Args>::member>` on main with its own carrier. Measured on main b57f2b5: #2534's positive/mismatch/conflict fixtures all behave.
- #2802 MERGED: generic boundary's Calling policy is evaluated for a natively bound application (typed-trees boundary_policy_instances + provider-planning attach by plan application identity + fail-closed planning refusal). 3 fixtures: fail/build/applied_native_calling_policy_rejection, pass/calls/applied_callback_private_materialization, pass/capabilities/windows_applied_requirement_import_exit.

Not established:
- #2802 validated on main b57f2b5, not rerun after #2792 (lowered-form checker only verdict).
- compiler --test access_view_calls: 7 of 8 time out at 900 s under idle priority; exclusive_backing_aliases_refuse_before_effects fails with TargetLayoutSelectionMissing(LayoutApplicationId(1)); not compared with main.
- product_boundary::selected_callback_custody_carries_declarations_not_source_symbols fails (source-text assert on terminal-psi); not compared with main.
- Dropped fixtures (mismatch, conflict) kept in scratchpad w11probes/dropped_fixtures; they pass on main but are not in the corpus.
- Stale remote branches: work/compiler-build-bind-applied-requirement, work/compiler-native-application-calling-instance, work/compiler-build-bind-applied-merged-wip.
