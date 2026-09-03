# Direct Beta Delta0 feasibility experiment

This test-owned analysis asks whether concatenative Gamma earns its permanent
place between Beta and the scalar/effect Delta0 bootstrap subset.

## Measured Gamma path

```text
753-line Beta Gamma evaluator
725-line Gamma compiler
193-line Gamma1 lowerer
666-line Gamma1 Delta0 seed
--------------------------------
2,337 authored lines above the common Beta compiler
```

The 666-line seed lowers to 689 lines of Gamma0, then expands to:

```text
3,230 lines / 79,175 bytes of Beta
2,842 Alpha instructions
1,678 calls
374 returns
388 address assertions
22,762-byte Alpha tape
```

Calls alone occupy 15,102 tape bytes, 66% of the artifact. The generated Beta is
regular and reconstructable, but it is not a plausible human-authored form: 59%
of its instructions are calls through tiny Gamma words or the generic value
stack.

## Matched implementation evidence

The same tokenizer structure appears in generated seed Beta and in the
hand-authored Beta Gamma evaluator:

```text
Gamma-generated tokenizer: 193 lines, 168 instructions, 88 calls
hand-written Beta tokenizer: 50 lines, 38 instructions, 0 calls
```

That is a 4.4x instruction expansion on a directly comparable compiler routine.
Two other generated categories are pure abstraction plumbing:

```text
46 named-cell getter/setter words: 230 lines, 184 instructions, 1,380 tape bytes
20 fixed-text emitter words:      370 lines, 350 instructions, 3,120 tape bytes
```

A direct Beta implementation would hold singleton state in persistent registers
or documented memory slots. Fixed text would use one data table and one byte-span
emitter rather than one push/call sequence per character.

Applying less favorable 2.5x-4.4x hand-written/generated ratios to the full seed
suggests roughly 650-1,140 direct Alpha instructions, or about 830-1,460 physical
Beta lines using the repository's authored-Beta line density. This is a
sensitivity range, not an implementation measurement.

## Implemented direct experiment

A direct Beta *compiler* is not necessarily the smallest bootstrap boundary.
The bootstrap only needs to execute a Delta0-authored source transformer that
produces the next-stage compiler. A Beta-authored Delta0 evaluator can therefore
replace all of:

```text
Gamma evaluator + Gamma compiler + Gamma1 lowerer + compiled Delta0 seed
```

The target evaluator profile contains explicitly typed functions, lexical
`let`, `if`, calls, proper tail recursion, integer operators, sealed input,
indexed reads, and byte writes. It need not emit Alpha, implement Gamma, or
support general application profiles. The prototype covers this surface except
for proper-tail execution.

The follow-up experiment is retained under
`tests/gamma/evaluator-development/`. It:

- implements the evaluator in 921 lines of addressed Beta;
- runs the retained 85-line `constant_augmenter.gamma` unchanged;
- requires its exact Delta output and the existing result-42 augmentation loop;
- compiles to a 4,788-byte tape.

The evaluator is materially below the 2,337-line Gamma route and keeps state in
documented registers and memory regions. Gamma therefore fails this initial
earned-rung test. Proper tail execution and static validation remain necessary
before the direct evaluator can replace a conforming compiler edge.
