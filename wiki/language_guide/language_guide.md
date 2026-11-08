# Omega language guide

Start with data and machines, then add ownership, contracts, and the services
your program needs. Examples introduce one feature at a time; protocol sketches
name the surrounding library operations they assume.

The [specification](../README.md) is the reference for complete rules. This guide
describes the accepted language. Compiler support and remaining work are tracked
in [implementation notes](../../omega-rust/README.md) and [TASKS.md](../../TASKS.md).

Suggested reading path:

Core language:

- [Chapter 1: Data, values, and literals](chapter_1_data_values_literals.md)
- [Chapter 2: Ownership, borrowing, and moves](chapter_2_ownership_borrowing_moves.md)
- [Chapter 3: Machines](chapter_3_machines.md)
- [Chapter 4: Nested machines and transitions](chapter_4_nested_machines_transitions.md)
- [Chapter 5: Expressions and evaluation](chapter_5_expressions_evaluation.md)
- [Chapter 6: Pattern matching and dispatch](chapter_6_pattern_matching_dispatch.md)

Proof and semantic model:

- [Chapter 7: Contracts and flow facts](chapter_7_types_constraints_invariants.md)
- [Chapter 8: Domains](chapter_8_domains.md)
- [Chapter 9: Proof obligations](chapter_9_proof_obligations.md)
- [Chapter 10: Compile-time proofs](chapter_10_compile_time_proofs.md)
- [Chapter 11: Invariant windows](chapter_11_invariant_windows.md)
- [Chapter 12: Dependent types](chapter_12_dependent_types.md)

Language-building features:

- [Chapter 13: Generics](chapter_13_generics.md)
- [Chapter 14: Traits and runtime machine values](chapter_14_traits.md)
- [Chapter 15: Modules, imports, and visibility](chapter_15_modules_imports_visibility.md)
- [Chapter 16: Errors, traps, and failure](chapter_16_errors_traps_failure.md)
- [Chapter 17: Drops and cleanup](chapter_17_drops_and_cleanup.md)
- [Chapter 18: Concurrency](chapter_18_concurrency.md)

Boundary and low-level topics:

- [Chapter 19: Capabilities, reach, and boundaries](chapter_19_capabilities_effects_boundaries.md)
- [Chapter 20: Memory layout and ABI](chapter_20_memory_layout_abi.md)
- [Chapter 21: Reflection](chapter_21_reflection.md)
- [Chapter 22: Inline assembly](chapter_22_inline_assembly.md)

Building, testing, and embedding:

- [Chapter 23: Testing](chapter_23_testing.md)
- [Chapter 24: Embedding and scripting](chapter_24_embedding.md)
- [Using source packages](packages.md)
