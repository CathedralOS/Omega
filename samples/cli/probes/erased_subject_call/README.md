# erased_subject_call

`let p [erased]: P = P::Z {};` + `P::touch(p)` — an erased local may be
the subject of a BY-VALUE proof-machine call: recursive `P` has no
layout, so the call is proof computation. A `&P` subject would be a
runtime consumption attempt and is refused.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
