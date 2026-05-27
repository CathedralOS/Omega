# Tasks

Compiler/runtime work surfaced by the `dungeon_crawler_cli` sample, the canary ladder, and the current language-design push.

## High Priority

- [ ] Termination proofs
  Add an opt-in termination proof surface that can be claimed at roots such as `Main::main`, then enforced transitively through the reachable call/state graph.
  Current position:
  - `terminates` and `decreases expr` now parse and lower through syntax/resolved/typed trees
  - checked-tree validation now rejects direct terminating recursive cycles with no `decreases`
  - direct countdown-style self recursion like `remaining > 0` then `self.countdown(remaining - 1)` now proves
  - bounded-distance recursion like `decreases limit - index` with `index < limit` then `index + 1` now proves
  - current canary coverage is compile-proof focused, not full runtime-shape coverage yet
  Initial target:
  - `terminates`
  - `decreases expr`
  - builtin well-founded measures for naturals, bounded distances, and slice lengths
  First useful canaries:
  - terminating index-carrying loop with `decreases limit - index`
  - terminating slice loop with `decreases items.len`
  - eventually shrinking-slice loop with `decreases items`
  Later:
  - lexicographic rankings
  - custom ranking projections for user-defined structs
  - possible sugar such as `increases x -> bound`

- [ ] Domain operators and proof-aware operator resolution
  The executable domain surface is now much healthier; the next step is turning the domain-operator idea into a real compiler feature rather than just documentation.
  Current position:
  - executable domain membership expressions now work for direct and imported positive checks
  - imported false-branch guard execution now works
  - direct and imported domain unions/intersections now run in both guard and value forms
  - contract-side domain unions are now proved for both call `requires` and exit `ensures`
  Next target:
  - define the first concrete operator-resolution surface driven by proved domains
  - keep ambiguity rules strict and compile-time only

- [ ] Proof-checking depth beyond current domain coverage
  We have broad coverage now for domains flowing through calls, exits, mutation invalidation, indexing, and boolean implications.
  Current position:
  - boolean/scalar facts derived from domains are covered on call and exit paths
  - fixed-index and dynamic-index domain-derived facts are covered
  - mutation invalidation for same-place and disjoint-place cases is covered
  Next target:
  - deeper proof shapes that are not just more symmetry
  - quantified/sequence-style facts
  - termination-ranking proof facts
  - custom well-founded projections

- [ ] Persistent machine/state mutation confidence
  Make writes in one state reliably observable in later states and transitions, with regression coverage for room/event flags, counters, and re-entry behavior.
  Current position:
  - direct full-package `enter_room` dispatch works in the sample
  - helper-expanded local slice alias mutation now runs end-to-end
  - direct aliased read/modify/write now runs
  - richer machine-owned indexed nested room copy/readback is covered
  Remaining target:
  - keep extending confidence on broader multi-edge/full-package flows
  - continue strengthening generator-style nested storage updates
  - keep pushing toward the remaining generic dungeon-sample blockers rather than only isolated micro-shapes

- [ ] Slices as a first-class proof/runtime feature
  The basic slice ladder is now in good shape; the next work should move from “individual seams compile/run” toward stronger semantic support.
  Current position:
  - fat slice descriptors survive locals and transition arguments
  - fixed-index and dynamic-index slice reads/copies run across transitions
  - iterative transitioned slice loops run
  - local and alias-based indexed slice writes are covered
  Next target:
  - termination/ranking support for shrinking-slice loops
  - stronger proof vocabulary around slice windows and non-empty views
  - more complex alias/proof interactions over slice-backed structures

- [ ] Runtime text and `read_line` confidence
  Stabilize mutable runtime text/string handling and keep broadening real IO coverage.
  Current position:
  - local string-comparison bool lowering is healthy
  - string concat and indexed string-field writes are covered
  - stdin-driven command branching has runnable pass coverage
  - repeated `read_line` buffering has runnable pass coverage
  Next target:
  - multi-step text flows with richer state transitions
  - more host/runtime confidence around real console interaction paths

## Backend Quality

- [ ] Strengthen assigned-target allocation
  Evolve the current assigned-home model into a more mature register/stack allocation story with clearer register classes, spill behavior, and post-assignment cleanup.

- [ ] Reduce host/runtime special-case lowering
  Keep shrinking the bring-up-era special handling around stdin/stdout/process calls so host/runtime lowering feels like a real subsystem instead of a narrow happy path.

## Language Guide

- [ ] Continue guide reorganization past the front half
  The front-half sequencing is much cleaner now.
  Next target:
  - traits/modules/host-boundaries sequence
  - keeping advanced chapters from assuming concepts that have not been introduced yet
  - pulling speculative topics into clearer “working direction” sections when needed

## Useful Landed Checkpoints

- [x] Executable domain membership in direct/imported guard and value forms
  Key canaries:
  - `canaries/pass/domains/executable_domain_membership_expression_exit`
  - `canaries/pass/domains/executable_imported_domain_membership_exit`
  - `canaries/pass/domains/executable_imported_domain_membership_guard_exit`
  - `canaries/pass/domains/executable_domain_membership_union_guard_exit`
  - `canaries/pass/domains/executable_domain_membership_intersection_guard_exit`
  - `canaries/pass/domains/executable_domain_membership_union_value_exit`
  - `canaries/pass/domains/executable_domain_membership_intersection_value_exit`
  - `canaries/pass/domains/executable_imported_domain_membership_union_guard_exit`
  - `canaries/pass/domains/executable_imported_domain_membership_union_value_exit`
  - `canaries/pass/domains/executable_imported_domain_membership_intersection_guard_exit`
  - `canaries/pass/domains/executable_imported_domain_membership_intersection_value_exit`

- [x] Slice runtime ladder
  Key canaries:
  - `canaries/pass/slices/runtime_slice_len_transition_exit`
  - `canaries/pass/slices/runtime_slice_fixed_index_guard_exit`
  - `canaries/pass/slices/runtime_slice_index_transition_exit`
  - `canaries/pass/slices/runtime_local_slice_len_comparison_value_exit`
  - `canaries/pass/slices/runtime_slice_iteration_exit`

- [x] Indexed mutation/call-write ladder
  Key canaries:
  - `canaries/pass/calls/runtime_mutable_machine_owned_parameter_write_exit`
  - `canaries/pass/calls/runtime_mutable_machine_owned_local_indexed_parameter_write_exit`
  - `canaries/pass/calls/runtime_mutable_dynamic_indexed_machine_owned_parameter_write_exit`
  - `canaries/pass/calls/runtime_mutable_local_parameter_write_exit`
  - `canaries/pass/calls/runtime_mutable_local_indexed_parameter_write_exit`
  - `canaries/pass/calls/runtime_mutable_parameter_read_modify_write_exit`
  - `canaries/pass/storage/runtime_dispatch_helper_local_alias_add_exit`
  - `canaries/pass/storage/runtime_slice_alias_indexed_field_write_exit`

- [x] Runtime boolean/string transition-value fixes
  Key canaries:
  - `canaries/pass/control_flow/runtime_local_boolean_conjunction_value_exit`
  - `canaries/pass/control_flow/runtime_local_boolean_or_value_exit`
  - `canaries/pass/control_flow/runtime_local_scalar_comparison_value_exit`
  - `canaries/pass/control_flow/runtime_local_string_comparison_value_exit`
  - `canaries/pass/control_flow/runtime_boolean_transition_argument_after_string_guard_exit`

- [x] Docs cleanup for dead invariant syntax and front-half guide structure
  Landed:
  - dead `Type[...]`/`range<...>` cleanup across samples/canaries/docs
  - guide front-half restructure
  - termination-proof design note in the proof chapters
