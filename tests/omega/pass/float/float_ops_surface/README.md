# float_ops_surface

`use omega::language::core::float_operations` makes primitive f32/f64
spellings resolve to named Float requirements: `+ - * /` and
`== != <` both participate; f32 fields carry the narrower format.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
