# Psi product source

This sibling package owns the Omega-written, target-neutral Psi half of the
production compiler. `source/omega/` consumes it through the `psi` package
identity; Omega does not own this source tree. The current live slice contains
source/span and token
representations, the Unicode-aware lexer, and a fail-closed whole-file parser
for ordinary `use path::member;` roots and basic `[pub] data` declarations with
an optional `[copy]` property, bare named field types, and payload-free
`case Name;` members. Mixed declarations retain the exact authored field/case
order in bounded owner-local syntax and type-reference tables. Unsupported
roots, case payloads, retired inline discriminants, and richer property/type
forms reject instead of becoming a private bridge tree. Its source closure is
being authored against the ordinary-Omega surface that the Delta-produced
compiler must eventually accept; that compiler is not yet published.

This authoring constraint governs features used by the compiler source, not the
Omega programs the resulting compiler accepts. Standalone terminal-Psi
interpreters, proof explorers, and other tools remain outside this closure
unless the compiler executable imports them.

The maintained Rust implementation lives at `source/omega-rust/psi/`. It
remains a differential comparator and may continue in parallel; no Rust
implementation belongs in this product subtree.

`test-parser.sh` compiles the gate-owned Omega harness once and runs the parser's
acceptance, rejection, capacity-edge, lexical-handoff, and determinism cases
against that one native artifact. Its Python helper only decodes the versioned
black-box observation; it implements no compiler semantics. Set `OMEGA_CLI` to
the exact freshly built comparator CLI and `OMEGA_TARGET` to the exact selected
target profile that should compile the current product source. Acceptance
evidence prints the SHA-256 identity of that CLI and freshly emitted artifact
beside the explicit target. The canonical gate has no cached-artifact bypass;
focused iteration may invoke the semantic-free Python decoder directly. The
gate deliberately selects neither an arbitrary existing `target/debug/omega`
nor an ambient host target.

The source closure and harness currently pass checked-source compilation, but
fresh native publication remains fail-closed at the attached Unit transitive
machine-plan boundary. Until that dependency lands, the required 45-case run is
not acceptance evidence and no cached executable may stand in for it.

The lexer now transfers one canonical mixed `Token` stream to the parser as a
whole ownership move.
There is no `TokenObservation`, numeric token array, per-token handoff, raw
parser ordinal, or scalar tag/span cache. Numeric protocol projection and
lex/parse serialization live only in the gate-owned Omega harness; the exact
product entrypoint retains phase driving and exit diagnostics. The same 45
black-box cases and structural observations remain mandatory, and the Python
decoder stays semantic-free. Unicode identifiers, codepoint escapes, and raw
strings are design-blocked pending an owner ruling; do not expand those
surfaces or make new compiler-source code depend on them meanwhile.

## Retention inventory

| Retained child | Product role | Deletion or absorption condition |
| --- | --- | --- |
| `build.omg` | Declares the target-neutral `psi` package consumed by Omega's product build. | Delete only if ordinary package ownership replaces this root atomically. |
| `source/` | Owns bounded source bytes and coordinates shared by the lexer and parser. | Absorb when a replacement representation preserves every live source/coordinate discriminator. |
| `tokens/` | Owns the sole typed lexical token stream transferred whole from lexer to parser. | Absorb only into a successor representation that preserves the exact typed vocabulary and coordinates without parallel token truth. |
| `syntax/` | Owns the bounded structural syntax retained by the current parser slice. | Absorb into a later Psi representation only with equivalent accepted/rejected observations. |
| `lex/` | Owns the source-to-token implementation and the currently unsettled Unicode tables. | Freeze disputed lexical expansion pending the owner ruling; afterward delete or retain the tables with the chosen contract and move every affected boundary case atomically. |
| `parse/` | Owns token-to-structural parsing; `harness.omg` is gate-only black-box serialization and is absent from the product closure. | Absorb the parser only into its canonical successor; delete the harness when an equal or stronger semantic-free gate preserves all 45 cases. |
| `gates/parser/`, `test-parser.sh` | Builds one fresh explicit-target harness artifact, prints exact identities, and exercises the live lexical/parser boundary. Its four empty target declarations are temporary compiler-discovery scaffolding. | Delete the declarations when immutable CLI target activation supplies the selected profile; delete the gate only when an equal or stronger product-source gate subsumes every retained failure class. |

Generated data is retained under the semantic phase that consumes it. The
Unicode tables therefore live in `lex/`; there is no generic `generated/`
source owner or generator runtime in the compiler closure.
