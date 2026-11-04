# Return safely from an Omega interrupt body

The compiled body emits `T` to port `0xE9`. The [CPU harness](../../../../../tools/native_entry_cpu/README.md) delivers eight interrupts to its exact generated entry and requires `BT.T.T.T.T.T.T.T.P` plus intact registers, flags and stack canaries after every return. The hosted application only demands the private callback; it is never run.

The entry has no parameters and creates no interrupt acknowledgement. Its split private descriptor and ordinary borrowed backing use the normal compiler path. The frozen pre-repair compiler rejected native entry transport with `IncompatiblePlan`; the repaired source-produced image passes the CPU oracle. This is interrupt transport evidence, not a live timer or installed-root result.

The sibling helper witness carries the same marker through a real ordinary call. Reject controls remove GPR preservation, use unsaved vector state, or enable interrupts despite a Masked plan.
