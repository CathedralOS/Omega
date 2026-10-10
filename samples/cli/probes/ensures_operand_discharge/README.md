# ensures_operand_discharge

Effectful call as a `==` operand: the boundary `requires right >= 0`
discharges via the sibling operand's own `ensures value >= 0`; `mut`
on a machine param lets the param be loaned `&mut`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
