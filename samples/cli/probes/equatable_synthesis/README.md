# equatable_synthesis

Empty `PointEquality satisfies Equatable<Point> { }`
receives synthesized structural `equals`: `==`/`!=`
compare Point field-by-field, `PointEquality::equals`
selects the same row. An authored `equals` member
overrides synthesis — `==` calls it (returns `false`
on identical values).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
