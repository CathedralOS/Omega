# lexical_const_shadow

First sample exercising const names in lexical scoping:
parameters and `let` bindings named `KEY` shadow the const
from their binding point onward; a `let selected: Key =
KEY;` read before the shadow still snapshots the const.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
