# computed_arrival_state

A named state reached only through a computed arrival: the state's
auxiliary payload `base + step` combines two entry inputs — the rank
and its endpoints stay exact.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
