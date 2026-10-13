# saved_case_guarantee

A saved call result keeps its callee's case guarantee: later calls
that require that case accept the saved local by reference and by
value, after an intervening reference-operand call whose contract is
not imported.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
