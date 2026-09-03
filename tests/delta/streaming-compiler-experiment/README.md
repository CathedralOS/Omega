# Streaming Functional Delta compiler experiment

This test-owned experiment asks whether the selected Delta compiler is hard to
audit primarily because it retains a universal expression-row representation,
or because current Gamma lacks the abstractions needed by compiler workloads.
It does not propose a new admitted rung.

The compiler accepts the scalar Functional Delta surface: typed functions,
forward calls, typed lexical `let`, `if`, seven scalar operators, arbitrary
nested calls, proper direct tail calls, sealed input length/indexing, and byte
output. It deliberately omits nominal data,
`Bytes`, checked overflow, application profiles, and exact production outcomes.
Those omissions make it an architecture discriminator rather than an alternate
Delta implementation.

## Architecture

The source is read three times:

1. Collect only function names and arities, rejecting duplicates and identifying
   the nullary `main`.
2. Reparse and validate every body, retaining only the active lexical
   environment and final frame width.
3. Reparse and emit bottom-up Gamma expression words. Tail position is passed
   into recursive emission, so no expression table or tail-analysis pass exists.

Only four-cell function rows survive between passes. Variable rows disappear as
lexical scopes close. Four-cell expression contexts exist only for active
recursive parser calls. Generated-call argument IDs use a bounded context array.

The authored compiler uses the test-owned augmentation of former concatenative
Gamma for named cells, layout constants, and exact fixed-token emitters. That
layer lowers to the downgraded concatenative compiler and changes no runtime
semantics. This experiment led to the current functional Gamma selection.

## Measurements

```text
666-line / 27,081-byte Gamma1 experimental compiler
  -> 689-line / 29,899-byte generated Gamma0
  -> 22,762-byte Alpha tape

193-line / 6,254-byte reusable Gamma1 lowerer
  -> 6,259-byte Alpha tape

9-line / 202-byte recursive Delta
  -> 26-line / 1,459-byte Gamma
  -> 2,554-byte Alpha tape -> byte 15

32-line / 951-byte scalar-surface Delta
  -> 84-line / 4,888-byte Gamma
  -> 6,108-byte Alpha tape -> byte 21
```

For comparison, the older scalar direct-to-Alpha Functional experiment is about
565 Gamma lines and produces a 22,214-byte compiler tape. It retains function,
variable, expression, and call-argument rows plus separate sizing contexts. The
streaming experiment has no retained expression IR and emits readable Gamma
rather than Alpha opcodes. Its Gamma1 source has 23 named-cell declarations, 8
named layout constants, 20 exact-text declarations, and no authored
`output-word` sites.

## Result

The experiment supports both hypotheses:

- The selected Delta compiler's universal row model is avoidable. Declaration
  collection, local validation, direct emission, and inherited tail position
  work for the complete scalar surface.
- A tiny source augmentation removes manually allocated singleton cells, names
  table layouts, and confines ASCII encoding to one reusable lowerer. It does
  not remove recursive context arithmetic or the hand-managed frame protocol.

Extending this exact compiler through ADTs and `Bytes` would demonstrate known
expressiveness but would not distinguish the hypotheses further. Gamma1 improves
local readability, but counting its 193-line lowerer raises the current
cold-start authored total to 859 lines. The augmentation therefore needs reuse or
self-augmentation before it can justify permanent trust. Verified stack effects
remain outside this tiny layer because checking them is semantic analysis, not
textual lowering.

Run `tests/delta/streaming-compiler-experiment/run.sh` to check interpreted/native
agreement, rejection before output, exact receipts, forward calls, maximum
arity, lexical scoping, and tail-recursive execution.
