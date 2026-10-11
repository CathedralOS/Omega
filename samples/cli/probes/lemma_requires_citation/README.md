# lemma_requires_citation

Citing a REQUIRES-bearing lemma: `cite_with_requires` carries `(x)==(add(y,Zero))`, the site discharge proves the instantiated requires, and `minus_swap`'s injected ensures settles the goal.

## Gaps found while authoring (filed)
- `core/nat`'s proof lemmas (`add_zero_right` et al.) are package-private — "selects private machine" on citation; axiom-style boundary machines can be declared locally but the shipped proof library can't be cited cross-package.
- `pub boundary data T [copy]` (opaque representation) refuses "[copy] without an admitted property receipt" in user packages — the corpus fixture fails identically; property receipts are gate-admitted (corpus-only surface).

Compile-checked only: run a check target, not native.
