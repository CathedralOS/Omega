# layout_intrinsics

First sample calling the builtin layout queries as ordinary
machines: `size_of<Pair>()` in const position and
`align_of<Pair>()` in value position — `Pair{u64; u32}`
lays out size 16 / align 8. Retracts the earlier report
that these reject in packages: they resolve at
`--check` fine.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
