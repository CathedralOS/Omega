A const alias exported by another module keeps its exporting declaration when
it is a callable's const argument, a premise subject and an implementation's
const argument. The consumer declares a same-spelled `MARKER` with a different
value, so a selection that resolved the alias's initializer in the consumer's
scope would refute `Marker == 7`, return false from `witness`, and leave both
`- 7` subtractions unproved.

Run: `omega run --both main.omg` (exit 0 on both engines).

Reject twins under `tests/omega/fail/generics/cross_module_const_alias_*`
replace one `ALIAS` argument with the local `MARKER` and keep everything else.
