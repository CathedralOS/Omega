# Tasks

Compiler/runtime work surfaced by the `dungeon_crawler_cli` sample.

## High Priority

- [ ] Runtime guard lowering for common state expressions
  Support runtime lowering for simple state-field comparisons and conjunctions like `room == 1 && used == 0` without forcing sample-level staging hacks.
  Progress:
  - conjunction guard emission now works in the full dungeon package `enter_room` dispatch path
  - the direct `enter_room` form reaches `Ambush Hall` correctly in native execution and now has a non-ignored regression in `canary_suite`
  - executable domain membership expressions now work for direct and imported positive checks
    - `canaries/pass/domains/executable_domain_membership_expression_exit`
    - `canaries/pass/domains/executable_imported_domain_membership_exit`
    - imported-domain false-branch guards now run too:
      - `canaries/pass/domains/executable_imported_domain_membership_guard_exit`
  - boolean `or` guard dispatch now runs:
    - `canaries/pass/control_flow/runtime_boolean_or_guard_exit`
  - computed local boolean conjunction values now lower and run:
    - `canaries/pass/control_flow/runtime_local_boolean_conjunction_value_exit`
  - scalar local comparison values now lower and run too:
    - `canaries/pass/control_flow/runtime_local_scalar_comparison_value_exit`
  - local boolean values initialized from string equality now lower and run:
    - `canaries/pass/control_flow/runtime_local_string_comparison_value_exit`
  Remaining visible gap:
  - broader conjunction-heavy runtime-dispatch states still need continued coverage beyond this fixed dungeon path
  - keep extending coverage for more complex boolean normalization shapes beyond the now-covered plain `or` family
    - simple negated guards are covered and work:
      - `canaries/pass/control_flow/runtime_negated_boolean_place_guard_exit`
      - `canaries/pass/control_flow/runtime_negated_comparison_guard_exit`
  - keep extending coverage for computed boolean values that cross more complex transition boundaries and helper-call shapes

- [ ] Persistent machine/state mutation confidence
  Make writes in one state reliably observable in later states and transitions, with regression coverage for room/event flags, counters, and re-entry behavior.
  Progress:
  - executable re-entry coverage now exists for spent-fountain and cleared-ambush flows
  - full-package direct `enter_room` dispatch now works without staged helper states in `dungeon_crawler_cli`
  - local mutable slice alias writes now compile in both straight-line and state-body forms
  - runnable exit-code probes now cover straight-line and state-body mutable slice alias writes too
  - runnable slice-index probes now cover dynamic reads and dynamic local copies in both straight-line and state-body forms
  - slice descriptors for `&[T]` now survive local storage and transition-argument materialization as full fat descriptors
    - `canaries/pass/slices/runtime_slice_len_transition_exit`
  - alias-indexed machine-owned string writes through slice views now run too
    - `canaries/pass/text/runtime_slice_alias_indexed_string_field_concat_exit`
  - indexed borrow overlap checks now distinguish obviously disjoint fixed indices from potentially aliasing ones
    - `canaries/pass/borrows/borrow_disjoint_fixed_index_call_mut`
    - `canaries/pass/borrows/borrow_disjoint_fixed_index_mut`
    - `canaries/fail/borrows/borrow_same_fixed_index_call_mut`
    - `canaries/fail/borrows/borrow_same_fixed_index_mut`
    - `canaries/fail/borrows/borrow_same_fixed_index_slice_alias_mut`
    - `canaries/fail/borrows/borrow_unknown_index_pair_mut`
  - indexed domain invalidation now has parser-safe canary coverage
    - `canaries/pass/domains/indexed_domain_requires_preserved_across_disjoint_field_mutation`
    - `canaries/fail/domains/indexed_domain_requires_invalidated_by_same_index_mutation`
    - `canaries/fail/domains/indexed_domain_requires_invalidated_by_unknown_index_mutation`
  Remaining gap:
  - keep extending confidence on other multi-edge/full-package dispatch shapes
  - continue strengthening richer nested storage updates for generator-style code
  - keep pushing beyond the now-covered indexed write/copy seams into the remaining generic dungeon sample blockers
  - fixed-index slice element reads through transitioned slice parameters are still not semantically correct at runtime
    - direct fixed-index guard probe still fails:
      - `canaries/run/runtime_slice_fixed_index_guard_probe`
  - `canaries/run/runtime_slice_index_transition_probe`
  - `canaries/run/runtime_slice_iteration_probe`
    - fat slice descriptors are now preserved, but `entries[0]` / `entries[0].field` still misread after transition in both direct-guard and local-initializer shapes
  - direct machine-owned nested fixed-array writes under dynamic indexing are covered, but richer room-style struct copy/readback still needs a dedicated follow-up before it can be promoted into the active pass suite
  - mutable state-call parameters and caller-local indexed writes are now covered at compile time:
    - machine-owned targets:
      - `canaries/pass/calls/runtime_mutable_machine_owned_parameter_write_exit`
    - caller-local frame targets:
      - `canaries/pass/calls/runtime_mutable_local_parameter_write_compile`
      - `canaries/pass/storage/runtime_dispatch_helper_local_alias_add_compile`
      - `canaries/pass/storage/runtime_dispatch_local_index_binary_write_exit`
  - remaining runtime follow-up:
    - `canaries/run/runtime_mutable_local_parameter_write_probe`
    - `canaries/run/runtime_dispatch_helper_local_alias_add_probe`

- [ ] Runtime text and `read_line` cleanup
  Stabilize mutable runtime text/string handling and remove the fragile feel around input/output buffer lowering, especially on macOS.

## Backend Quality

- [ ] Strengthen assigned-target allocation
  Evolve the current assigned-home model into a more mature register/stack allocation story with clearer register classes, spill behavior, and post-assignment cleanup.

- [ ] Reduce host/runtime special-case lowering
  Keep shrinking the bring-up-era special handling around stdin/stdout/process calls so host/runtime lowering feels like a real subsystem instead of a narrow happy path.

## Suggested First Crunch

- [x] Add focused canaries for runtime guard/state persistence
  Cover:
  - room/event consumed flags surviving leave/re-enter flows
  - simple numeric/boolean machine fields used in later transition guards
  - conjunction/disjunction guard cases that currently force staged dispatch
  Landed:
  - `canaries/pass/dungeon/runtime_room_use_reentry_guard`
  - `canaries/pass/dungeon/runtime_room_use_reentry_exit`
  - `canaries/pass/dungeon/runtime_enemy_clear_reentry_guard`
  - `canaries/pass/dungeon/runtime_enemy_clear_reentry_exit`
  - `canaries/pass/dungeon/runtime_boolean_helper_guard_dispatch`
  - `canaries/pass/dungeon/runtime_direct_boolean_conjunction_dispatch`
  - `canaries/pass/dungeon/runtime_direct_boolean_conjunction_exit`
  - `canaries/pass/control_flow/runtime_local_boolean_conjunction_value_exit`
  - `canaries/pass/control_flow/runtime_local_scalar_comparison_value_exit`
  - `canaries/pass/slices/runtime_mutable_slice_element_write_compile`
  - `canaries/pass/slices/runtime_mutable_slice_element_write_exit`
  - `canaries/pass/slices/runtime_dispatch_mutable_slice_element_write_compile`
  - `canaries/pass/slices/runtime_dispatch_mutable_slice_element_write_exit`
  - `canaries/pass/slices/runtime_slice_index_read_exit`
  - `canaries/pass/slices/runtime_slice_index_read_dispatch_exit`
  - `canaries/pass/slices/runtime_slice_index_copy_exit`
  - `canaries/pass/slices/runtime_slice_index_copy_dispatch_exit`
  - `canaries/pass/dungeon/runtime_ordered_room_dispatch_after_call_exit`
  - `canaries/pass/dungeon/runtime_ordered_room_dispatch_exit`
  - `canaries/pass/dungeon/runtime_ordered_room_dispatch_game_shape_exit`
  - `canaries/pass/dungeon/runtime_ordered_room_dispatch_large_machine_exit`
  - `canaries/pass/dungeon/runtime_ordered_room_dispatch_loop_exit`
  - `canaries/pass/dungeon/runtime_ordered_room_dispatch_real_show_states_exit`
  - `canaries/pass/dungeon/runtime_multi_room_reentry_exit`
  - native scripted dungeon smoke in `compiler/orchestration/omega-compiler/tests/canary_suite.rs`

- [ ] Replace sample workarounds once compiler support lands
  Progress:
  - removed dungeon-crawler pseudo-room-history indices
  - sample now uses real room ids plus persisted boolean consumed/defeated flags
  - removed staged `enter_room` helper-view states now that the direct full-package path is proven
  - removed additional helper-view staging in `look`, `help`, `use`, and `fight` where direct guarded dispatch now works cleanly
  Remaining gap:
  - continue trimming remaining sample-side staging only where it is clearly masking real compiler gaps
