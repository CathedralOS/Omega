# interval_member_lit

An interval-domained payload member: `case Lit(amount: i32 in Ten)` under
`domain i32::Ten(value) requires 0 <= value && value <= 10` — the
construction proves the member's range obligation, and the dispatched
payload discharges `scale`'s `requires` through its domain alone.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
