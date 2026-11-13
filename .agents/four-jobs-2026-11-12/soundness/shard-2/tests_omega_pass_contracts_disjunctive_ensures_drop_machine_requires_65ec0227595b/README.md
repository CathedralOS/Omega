# disjunctive_ensures

`ensures result == input || result < input` disjunctive guarantee + vacuous `requires true==true`/`ensures true==true` + modulo actual into requires-bearing callee in a state guard.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
