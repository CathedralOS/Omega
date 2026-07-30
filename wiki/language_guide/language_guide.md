# Omega Language Guide

This guide is the current semantic authority for Omega's language direction.
Syntax may still change, but each chapter should describe one present model—not
the sequence of arguments that produced it. When a ruling changes, rewrite the
affected chapter in place and move any still-useful rationale to a design
brief; do not retain contradictory “proposal / owner answer / amendment” layers
in the guide.

Documentation roles:

- language-guide chapters state current language behavior;
- frozen design briefs supply laws, rationale, and acceptance cases;
- architecture documents state compiler representation and ownership;
- `TASKS.md` contains engineering status and remaining work, not language
  decision transcripts; and
- `OWNER_QUESTIONS.md` contains unresolved owner decisions only.

Implementation-status claims name the exact landed layer: source surface,
normalization, validation, lowering, runtime realization, or proof. Landing one
layer never licenses describing the complete language model as implemented.
Generated origin and checked trust, historical schema and migration coverage,
and selected representation and authored representation law remain similarly
distinct.

Compiler architecture notes live in [Architecture](../architecture/architecture.md).

Suggested reading path:

Core language:

- [Chapter 1: Data, Values, And Literals](chapter_1_data_values_literals.md)
- [Chapter 2: Ownership, Borrowing, And Moves](chapter_2_ownership_borrowing_moves.md)
- [Chapter 3: Machines](chapter_3_machines.md)
- [Chapter 4: States And Transitions](chapter_4_states_transitions.md)
- [Chapter 5: Expressions And Evaluation](chapter_5_expressions_evaluation.md)
- [Chapter 6: Pattern Matching And Dispatch](chapter_6_pattern_matching_dispatch.md)

Proof and semantic model:

- [Chapter 7: Contracts And Flow Facts](chapter_7_types_constraints_invariants.md)
- [Chapter 8: Domains](chapter_8_domains.md)
- [Chapter 9: Proof Obligations](chapter_9_proof_obligations.md)
- [Chapter 10: Compile-Time Proofs](chapter_10_compile_time_proofs.md)
- [Chapter 11: Invariant Windows](chapter_11_invariant_windows.md)
- [Chapter 12: Dependent Types](chapter_12_dependent_types.md)

Language-building features:

- [Chapter 13: Generics](chapter_13_generics.md)
- [Chapter 14: Traits And Runtime Dispatch](chapter_14_traits.md)
- [Chapter 15: Modules, Imports, And Visibility](chapter_15_modules_imports_visibility.md)
- [Chapter 16: Errors, Traps, And Failure](chapter_16_errors_traps_failure.md)
- [Chapter 17: Drops And Cleanup](chapter_17_drops_and_cleanup.md)
- [Chapter 18: Concurrency](chapter_18_concurrency.md)

Boundary and low-level topics:

- [Chapter 19: Capabilities, Reach, And Boundaries](chapter_19_capabilities_effects_boundaries.md)
- [Chapter 20: Memory Layout And ABI](chapter_20_memory_layout_abi.md)
- [Chapter 21: Wire Protocols](chapter_21_wire_protocols.md)
- [Chapter 22: Historical Data And Component Replacement](chapter_22_versioned_data.md)
- [Chapter 23: Inline Assembly](chapter_23_inline_assembly.md)
- [Appendix: Open Questions](appendix_open_questions.md)
