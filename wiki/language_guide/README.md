# Omega Language Guide

This guide is a chaptered sketch of Omega's language direction.

The syntax is not final. These chapters exist so language ideas can be organized, challenged, and eventually turned into parser/compiler work.

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
- [Chapter 11: Relax Scopes](chapter_11_relax_scopes.md)

Language-building features:

- [Chapter 12: Generics](chapter_12_generics.md)
- [Chapter 13: Traits And Runtime Dispatch](chapter_13_traits.md)
- [Chapter 14: Modules, Imports, And Visibility](chapter_14_modules_imports_visibility.md)
- [Chapter 15: Errors, Traps, And Failure](chapter_15_errors_traps_failure.md)
- [Chapter 16: Drops And Cleanup](chapter_16_drops_and_cleanup.md)
- [Chapter 17: Concurrency](chapter_17_concurrency.md)

Boundary and low-level topics:

- [Chapter 18: Host Libraries And Trust Boundaries](chapter_18_host_trust_boundaries.md)
- [Chapter 19: Memory Layout And ABI](chapter_19_memory_layout_abi.md)
- [Chapter 20: Wire Protocols](chapter_20_wire_protocols.md)
- [Chapter 21: Versioned Data And Machine Replacement](chapter_21_versioned_data.md)
- [Chapter 22: Inline Assembly](chapter_22_inline_assembly.md)
- [Appendix: Open Questions](appendix_open_questions.md)
