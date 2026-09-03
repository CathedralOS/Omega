# Direct Beta Gamma evaluator development

This test-owned development source implements typed scalar/effect Gamma
directly in addressed Beta, without Gamma:

```text
Beta compiler -> Gamma evaluator tape
Gamma evaluator + Gamma-authored augmenter -> richer Gamma source
Gamma evaluator + expanded source -> result 42
```

The evaluator accepts a little-endian u32 source length, exact source bytes, and
remaining sealed input. It censuses function names, parameter spans, body spans,
and arities, then evaluates expressions directly from source. It implements
explicitly typed scalar functions, lexical `let`, `if`, forward calls, integer
operators, sealed input length/indexing, and byte output. It retains no AST and
emits no Gamma or Alpha code.

## Measurements

```text
1,410-line / 32,096-byte symbolic addressed-Beta source
81-line / 2,070-byte test-only label resolver
1,410-line / 39,423-byte canonical addressed Beta with named control targets
7,690-byte evaluator tape
```

The resolver only computes numeric addresses and the gate pins the resulting
Beta and tape hashes. A promoted implementation would retain the addressed Beta
directly and would not trust the Python resolver.

For comparison, the current Gamma route above the common Beta compiler contains:

```text
753-line Beta Gamma evaluator
725-line Gamma compiler
193-line Gamma1 lowerer
666-line former concatenative seed for the same scalar/effect language
--------------------------------
2,337 authored lines and four semantic/build layers
```

The direct evaluator runs the unchanged 85-line Gamma `const` augmenter,
requires its exact 51-byte Gamma receipt, and evaluates that receipt to byte 42.
It also covers literals, lexical bindings, every scalar operator, true/false
branches, forward and parameterized calls, recursion, compiler I/O, and quiet
invalid/trap outcomes.

## Limitations

This is not yet an admitted Gamma evaluator:

- Non-tail calls and source nesting retain explicit bounds.
- Integer parsing and arithmetic are deliberately wrapping Gamma operations;
  Delta owns checked arithmetic.
- Resource partitions and detailed profile outcomes are experimental.

The remaining capacity proof requires real work, but the measured margin remains:
the direct candidate is 1,410 Beta lines versus 2,337 lines across the former
concatenative route. More importantly, its state is held in documented
registers and five explicit memory regions rather than hidden behind a generic
stack-machine expansion.

## Finding

The former concatenative Gamma is not earned as a permanent bootstrap rung.
Direct Beta already executes the high-level Gamma-authored augmentation workflow
at less than half the authored line surface and about one fifth the Alpha tape
size of the compiled seed alone. The remaining decision is whether proper tail
execution and static validation preserve the evaluator's local auditability.
