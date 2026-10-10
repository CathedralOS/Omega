# Retained local reference at a suspension crossing

```sh
omega run --both tests/omega/pass/concurrency/runtime_retained_reference_boundary_exit/main.omg
```

The program captures its original `backing` storage, invokes a possibly
suspending boundary machine, then observes the retained reference. A generic
carrier retains two borrowed leaves beside a non-borrowed marker: one reborrows
the retained reference, and the other borrows a separate owned root.
A local retains write-only access and writes its original backing after
the call, without acquiring read permission.
Its authored exit is 0 only when the call returns 42, the reference still
observes 40, both shared carrier leaves and the marker retain their original
values, and the write-only reference stores 99 into its original backing.

This is an inline-completion control, not evidence of parking or resumption.
Admission must use the same per-value loan, backing and lifetime join as other
suspension crossings. The selected provider's inline body cannot waive that
obligation.

Source admission closes the exact retained loan and backing dependencies before
judging carry. Each admitted root keeps its checked enclosing-invocation
lifetime; the canonical Terminal graph preserves formation, access, parent and
supplier relationships. Activation-wide CPU/thread preservation stays strict
rather than becoming portable just because a descriptor can survive the call.
Incoming referents and incomplete captured-actual leaf coverage still reject.
Real start/park/resume/cancellation execution remains the separate TR3-TR8
frontier. The conflicting-backing-write failure fixture pins access exclusion.
