# core_drop_generic_chain

Contextual cleanup crosses an explicit core `drop` and an unrelated
generic call chain: the exact ready owner supplies each hook's
prerequisite, and the identity-generic forward `id<Guard>` does not
disturb the obligation.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
