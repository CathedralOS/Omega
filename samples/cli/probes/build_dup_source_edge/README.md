# build_dup_source_edge

Two observations in one family:

- `pub machine build` is legal — visibility on the activation machine
  does not change the contract (the positive sample).
- `invalid_dup_source`: two UNALIASED `Build::depend` edges onto the
  same source both import under the dependency's declared name —
  "dependency alias `same_kernel` conflicts between occurrences 0 and
  1". The declared name is the default alias, so unnamed duplicates
  collide exactly like repeated explicit aliases.
