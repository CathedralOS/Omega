# short_circuit_transfer

First sample with affine-owner transfer inside `&&`/`||`
operands — `enabled && consume(Wrapped { token })` moves the
owner only on the taken path; the skipped side settles its
retained custody before reconverging, and a disjoint sibling
survives either way.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
