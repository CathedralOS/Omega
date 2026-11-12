# Omega language guide

Omega is a systems language in which the compiler checks, before the program
runs, that arithmetic does not overflow, indexes are in bounds, invariants
hold and a program touches only what it declared. This guide shows how to
write it. Each chapter is short and starts from code.

The [specification](../README.md) has the complete rules; every chapter links
to the part it summarizes. The guide describes the language as designed. What
the compiler supports today is tracked in the
[implementation notes](../../omega-rust/README.md) and [TASKS.md](../../TASKS.md).

## Start here

Read chapters 1 to 4 in order. They take about fifteen minutes and are enough
to read most Omega code. Then read 5 to 7, and after that pick what you need.

```omega
data Counter
requires count <= limit
{
    count: u32;
    limit: u32;
}

machine bump(counter: &mut Counter) -> bool {
    transition counter.count < counter.limit {
        true -> advance(counter)
        false -> false
    }

    machine advance(counter: &mut Counter) -> bool
    requires counter.count < counter.limit
    {
        counter.count = counter.count + 1;
        true
    }
}
```

That is a record with an invariant, a machine that takes it by mutable
reference, a branch, and a nested machine whose `requires` is what makes the
addition provably safe. Nothing here is checked at run time.

## If you know another language

| You expect | In Omega |
| --- | --- |
| Functions and methods | `machine`. No methods; the value is an ordinary parameter. |
| `struct` and `enum` | `data` with fields, or `data` with `case`s. |
| `for` and `while` | A machine that calls itself as its last action. |
| `if` and `match` | `transition` and `match`. |
| Exceptions or `?` | Return a sum and match on it. |
| `impl Trait for Type` | A named implementation, `Name satisfies Trait<Type>`, passed explicitly. |
| Closures and trait objects | A record holding machines, with `some S` for hidden state. |
| Runtime bounds and overflow checks | A compile-time proof, or an operator such as `+!` that says what to do. |
| Private fields | Public fields, with a domain controlling who may create and change values. |
| `async` and `await` | `suspends` on the machine and `suspend` at the call. |
| Global state, ambient I/O | Boundary traits chosen by the build. |

## Chapters

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
