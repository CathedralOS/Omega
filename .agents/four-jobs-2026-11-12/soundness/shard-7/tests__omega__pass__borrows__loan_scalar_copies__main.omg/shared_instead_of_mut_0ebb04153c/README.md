# loan_scalar_copies

Scalar locals initialized from member reads through a `&mut` record loan: copies carry no loan, the loan ends at its last use, the original is read on its own authority.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
