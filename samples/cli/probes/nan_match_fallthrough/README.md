# nan_match_fallthrough

Float match equality is semantic IEEE equality: `-0.0f32` matches `0.0f32`; NaN matches NOTHING — a NaN subject falls to `_` even when every pattern is NaN itself.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
