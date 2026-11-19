# equality_case_subtraction

A case `where lo == hi` equality relates the two payload fields: under the
`Point` arm `hi - lo` cannot underflow — the equality is contributed on the
destructured bindings and survives the edge into `take`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
