# call_local_reassign

A call-initialized mutable local can be overwritten by a literal and
reassigned from a second call in a multi-state machine; the composed
body rejoins both whole stores to the local the machine established.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
