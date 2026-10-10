# Bounded mutable primitive storage

A mutable machine parameter, a mutable local, and a mutable named-state
parameter retain their closed range while storing and observing new values.
The authored result checks all three values and crashes on disagreement.

Run `omega run --both main.omg` to check native and Terminal execution.
