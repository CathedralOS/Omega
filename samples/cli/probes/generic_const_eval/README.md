# generic_const_eval

First sample instantiating a generic inside const
evaluation: `const Observed: u64 = extent()` calls
`identity<[u8; Count]>` — the substituted T row retains
`Count`'s const-param origin; `bytes.len` reads the
evaluated literal's extent.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
