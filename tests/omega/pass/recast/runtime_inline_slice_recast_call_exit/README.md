# Slice recasts in nested call operands

After ordinary package preparation, run:

```sh
omega run --both tests/omega/pass/recast/runtime_inline_slice_recast_call_exit/main.omg
```

Both engines must return 70. Ordinary helpers receive inline mutable and
shared recasts of the final four bytes of a six-byte array as two `u16`
elements. The writer updates the original backing; the reader is itself an
operand of the Console call. Length and leaf observations require the formed
suffix descriptor rather than its canonical backing place. The mutable
descriptor retires before the later shared formation.
The writer's computed byte offset retains the source's congruence proof.
