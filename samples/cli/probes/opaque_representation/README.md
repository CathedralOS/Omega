# opaque_representation

First sample with an opaque representation pair: `boundary
data Token` (sibling `token.omg` via `use token;`) +
`TokenRepresentation satisfies
OpaqueRepresentation<Token, TokenBytes>` — the opaque
carrier travels as a machine param and carrier field while
its byte view stays an implementation choice.

## Learnings

- `use token;` exposes the sibling file's names
  unqualified in the importing file — `Token`, not
  `token::Token`.
- `boundary data Token [copy]` rejects in a package:
  "[copy] without an admitted property receipt" — the
  corpus fixture's `[copy]` claim rides hermetic
  provenance; a package must claim copy properties
  through the receipt route (unfiled, edge noted).

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
