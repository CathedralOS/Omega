# dyn_generic_requirement

`dyn Shape<Item>` selector: `Primary as dyn
Shape<Item>` erases a named implementation;
`Shape::code<16>(selected, &item)` routes calls
through it. A requirement `where Width == 16 || Width
== 32` finite-disjunction materializes every roster
tuple's realization on selection alone.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
