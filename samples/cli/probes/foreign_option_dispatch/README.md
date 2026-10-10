# foreign_option_dispatch

First sample instantiating a foreign generic sum with
user payloads: `Optional<First>`/`Optional<Second>` via
`use omega::language::core::option` — construction,
`&Optional<T>` borrowed dispatch, `Optional::Some {
value }` destructure, and `Optional::None` typed nullary
case all go through ordinary sum machinery.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
