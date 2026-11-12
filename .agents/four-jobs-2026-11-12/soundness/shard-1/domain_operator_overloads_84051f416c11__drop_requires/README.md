# domain_operator_overloads

`operator Domain::name(params) -> T;` — domain-scoped operator
signatures, overloaded per operand types (`combine` on i32 and u64) and
across result shapes (`scale` returning the domained record).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
