# qualified_declarations

A data type and a machine imported from the same module by qualified
path (`use combat::Damage;`, `use combat::damage;`) stay distinct
declarations at the use site rather than colliding on their shared
leaf — qualified record, case, and empty-case constructors work through
the module path too.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
