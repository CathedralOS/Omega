# Atomic read-and-clear across loop arrivals

The same shared atomic cell is read and cleared eight times. The first iteration must return 1; every repeated iteration must return 0. The final load must return 0 and both neighboring fields must retain 37 and 91. A mismatch traps; the expected exit is 0.

Atomic lowering must retain the complete incoming histories: the initial residency on the first arrival and the prior atomic write on repeated arrivals. It cannot choose one history for every iteration.

This source and Build file are byte-identical to the frozen ordinary reproducer. The isolated published layout-repair producer refused it with `UnsupportedAtomicCoherence(OperationId(368))`; the repaired producer runs it with native exit 0 and Terminal Psi exit 0. This is an ordinary program, with no hardware interrupt claim.

Focused ordering rejection: [a load using Publish](../../../fail/atomics/atomic_load_publish_ordering_rejected/main.omg) must still reject its unsupported ordering.
