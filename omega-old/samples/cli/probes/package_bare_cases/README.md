# package_bare_cases

Package aliases select the same exact payload-free case as braces:
`Value::Empty` and `Value::Empty {}` denote one constructor — both
return through the dependency's module.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
