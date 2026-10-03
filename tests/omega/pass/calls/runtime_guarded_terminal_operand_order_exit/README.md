# Guarded terminal operand evaluation order

Run `omega run --both main.omg`; both engines must exit 0.

The unselected arm leaves the counter untouched. A later selected arm calls two
operands that each increment and observe the same counter. The outer call must
receive 1 followed by 2, so the program checks left-to-right, exactly-once
execution as well as arm selection. Returning from that terminal call completes
the caller through the ordinary invocation contract.
