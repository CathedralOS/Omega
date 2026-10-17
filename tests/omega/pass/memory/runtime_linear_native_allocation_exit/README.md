# Native allocation with routed release custody

The selected Linux provider calls the real libc malloc and free leaves. Allocation
is linear; Held authorizes release of exactly that result and does not confer
storage-reference or extent authority. Two different non-null live allocations
must be observed. Both normal and failed checks release both claims; a failed
check traps instead of returning success.

Run the Linux native corpus gate for memory/runtime_linear_native_allocation_exit.
