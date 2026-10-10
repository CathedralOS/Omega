# Bounded mutable primitive replacement

A new value outside the mutable formal's declared range must be rejected at
its exact assignment, even though the underlying u32 carrier can represent it.
