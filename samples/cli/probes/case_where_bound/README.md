# case_where_bound

First sample with a case-where ORDERING contribution:
`case Range(lo, hi) where lo <= hi` — the arm contributes
`lo <= hi` as a branch-local fact that proves the target
state's `requires lo <= hi` at arrival, with no other
evidence available.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
