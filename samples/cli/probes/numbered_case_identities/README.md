# numbered_case_identities

Stable sum-case and structured payload identities as data metadata: `case
#1 Found(#1 value: T, retired #2)` numbers both the case and its payload
member, `retired #2` retires a payload slot, and `retired #3` retires a
data-level number — all without altering the runtime discriminant.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
