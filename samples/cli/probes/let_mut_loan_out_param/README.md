# let_mut_loan_out_param

First sample lending `&mut` from a plain `let` binding:
`mut` gates rebinding of the NAME, not loans from owned
storage — `find(2, &mut found)` writes through the
out-param and the caller's binding observes both writes.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
