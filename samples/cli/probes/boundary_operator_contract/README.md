# boundary_operator_contract

A `boundary operator` declares a checked operator contract with its own
requires/ensures (`ensures result == value + 0`); the satisfying provider
restates both sides of the contract, and the caller discharges the
requires by literal.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
