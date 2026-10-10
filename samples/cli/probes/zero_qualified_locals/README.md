# zero_qualified_locals

First sample establishing domain-qualified locals at zero:
`Counter in ReadyAlias` needs no grant for route-/
predicate-free aliases; `f32[0.0..0.5]` floating range
endpoints are IEEE values (zero-initializes);
`zero_value<Choice<T>>() in Choice::Empty` — an unused
generic payload can't invalidate the home case.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
