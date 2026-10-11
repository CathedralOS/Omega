# dyn_finite_family

Finite generic method family: `where Width == 16 || Width == 32` disjunctive roster + `dyn Shape<Item>` runtime selection materializes both `Shape::code<16>` and `Shape::code<32>` realizations.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
