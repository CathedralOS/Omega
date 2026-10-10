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
The incomplete call also receives a fresh borrow, a forwarded reference, a
stored generic carrier and a directly constructed generic carrier. Its fresh
root and one stored leaf's root have no later caller use. Each actual's completed
evaluation, not its former descriptor home, must retain the original backing.
Closed constructor fields use the same declaration correspondence as ordinary
structural values; template `T` fields cannot erase their borrowed leaves.
Its authored exit is 0 only when the call returns 241, the reference still
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
The canonical graph names each evaluated actual and exact borrowed leaf beside
the retained formation captures. Incoming referents and incomplete actual
frontiers still reject; descriptor permission alone establishes no lifetime.
Real start/park/resume/cancellation execution remains the separate TR3-TR8
frontier. The conflicting-backing-write failure fixture pins access exclusion.
