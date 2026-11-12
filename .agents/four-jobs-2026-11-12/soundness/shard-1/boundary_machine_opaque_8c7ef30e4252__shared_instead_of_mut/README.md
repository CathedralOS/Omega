# boundary_machine_opaque

`boundary machine` attached to a `pub boundary data` opaque type: the type carries no fields yet hosts the crossing's contract surface — `WaitSubstrate::park(word) suspends blocks;`, `wake_one`, `wake_all`. Dual-axis `suspends blocks;` marks a blocking wait, not a semantic safe point.

## Learnings

- `boundary machine` declarations ride on type identity alone; `boundary data` needs no representation to own call edges.
- `boundary machine` is callable from checked bodies (`WaitSubstrate::wake_one(main.word)`).

Compile-checked only: run a check target, not native.
