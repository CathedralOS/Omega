# every_width_index

Every integer width indexes array and slice geometry once its bounds are
proved: ranged params `u8 [0..=3]` … `i64 [0..=3]` each select an
element (`^` folds them); `read_slice` keeps a requires-bearing
`position < values.len` contract on `&[u64]`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
