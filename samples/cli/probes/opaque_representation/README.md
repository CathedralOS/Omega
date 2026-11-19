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

- A representation is inert until the build selects it
  (`wiki/spec/build/opaque_representations.md`): without
  `Build::select_representation<Token, TokenRepresentation>(builder)`
  the by-value `Token` parameter and field have no carrier, and native
  realization refused the state signature. The build now selects it.

No standard-library dependency and no `Main::exit`: the program has no
exit oracle, and `hits` records the reached arm. With the selection the
probe realizes natively on macOS arm64; a check target still needs no
selection, since only an actual by-value realization closes the demand.
