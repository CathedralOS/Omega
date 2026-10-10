# mixed_len_compare

Mixed-width guard comparisons: `main.i < main.text.len` (i32 vs u64 `.len`); the re-guard `i >= 0 && i < 8` keeps the increment overflow obligation out of the way.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
