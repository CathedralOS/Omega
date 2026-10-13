# borrowed_sum_dispatch

`transition duo { Duo::Left { a } -> ... }` on `&Duo`/`&mut Duo` —
borrowed sum dispatch binds case payloads per arm and forwards them to
successor states; the caller keeps its value (dispatch disposes
nothing).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
