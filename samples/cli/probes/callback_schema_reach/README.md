# callback_schema_reach

Machine params bound by signature schemas:
`requires machine Schema<const Number: u64>(value: u64)
-> u64 reaches Console` — const binder + `reaches`
effect in the bound; `Schema<2>(value)` calls with
static args; `apply<selected, unused>(value)` passes
machine names as arguments.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
