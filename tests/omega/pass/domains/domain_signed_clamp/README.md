# domain_signed_clamp

`i8 in Saturating` signed-clamps positive overflow to 127; `u8 in Trapping` compiles the same operation class (overflow is a runtime abort, not a check failure).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
