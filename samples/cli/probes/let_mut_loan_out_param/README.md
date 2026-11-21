# let_mut_loan_out_param

First sample lending `@mutable` from a plain `let` binding:
`mutable` gates rebinding of the NAME, not loans from owned
storage — `find(2, @mutable found)` writes through the
out-param and the caller's binding observes both writes.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
