# Mixed formal callback entry

Prepare this disposable fixture through the normal package review. Accept the
reviewed package decisions in the rendered review before resuming:

```sh
omega update --project tests/omega/pass/calls/callback_mixed_formal_entry --offline
omega update --project tests/omega/pass/calls/callback_mixed_formal_entry --resume
omega run tests/omega/pass/calls/callback_mixed_formal_entry/main.omg
```

On macOS Arm64 the real C qsort_r sorts two 16-byte records. The comparator's
authored formals are a record reference, a scalar direction, then another
record reference. Darwin supplies the context in X0 and the references in
X1/X2. The selected policy deliberately expresses that correspondence without
reordering the authored telescope. Omega's internal scalar-first convention
already has those physical placements, but its ABI rows have a different order.
The ordinary helper reads both records; the callback consumes the direction.
The caller checks ascending values and associated stamps, then invokes the
same callback with a descending direction and checks both complete records
again. It exits 0 (71 on a mismatch). Equal widths cannot establish which
formal a register supplies, and the context is consumed on both branches.

The borrow is call-scoped and qsort_r invokes the comparator synchronously.
This establishes neither durable registration nor a future callback root.

The Terminal inspection releases the source session, then repeats native callback
placement from both the retained artifact and its encoded/decoded form. Canonical
calling declarations supply binder identity, synchronous invocation, and native
argument order (including the function-pointer argument). Controls change the
proposed synchronous contract, pointer width, and argument ordinals; each must
refuse against the retained declaration. Value and inline-slot layouts remain
separate inputs; this does not establish source-free layout planning.
