# erased_formal_borrowed

An erased proof formal occupies no structural position: `relay` forwards
its borrowed `values` parameter to `first`, and the argument names
`values` — not a slot counted past the erased `witness`; a trailing
erased formal does not disturb the forwarding either.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
