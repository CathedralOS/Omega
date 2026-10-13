# Restricted field mutable call

A shared call may observe the original restricted field through its scalar
carrier. A mutable scalar loan must preserve the owning record's standing
invariant at handback. This callee can assign 2 and offers no sufficient
normal-return guarantee, so source checking rejects the loan before lowering.
