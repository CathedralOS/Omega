# machine_row_satisfaction

The parameter in `register<const Selected: Handler::call>` requires a
machine that satisfies `Handler::call`. The caller selects `chosen` with
`register::<chosen>(4)`, and the body invokes `Selected(value)`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
