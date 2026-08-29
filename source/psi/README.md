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
roots, case payloads, explicit discriminants, and richer property/type forms
reject instead of becoming a private bridge tree. Its source closure is
being authored against the ordinary-Omega surface that the Delta-produced
compiler must eventually accept; that compiler is not yet published.

This authoring constraint governs features used by the compiler source, not the
Omega programs the resulting compiler accepts. Standalone terminal-Psi
interpreters, proof explorers, and other tools remain outside this closure
unless the compiler executable imports them.

The maintained Rust implementation lives at `source/omega-rust/psi/`. It
remains a differential comparator and may continue in parallel; no Rust
implementation belongs in this product subtree.

`test-parser.sh` compiles the product entrypoint once and runs the parser's
acceptance, rejection, capacity-edge, lexical-handoff, and determinism cases
against that one native artifact. Its Python helper only decodes the versioned
black-box observation; it implements no compiler semantics. Set `OMEGA_CLI` to
the exact freshly built comparator CLI and `OMEGA_TARGET` to the exact selected
target profile that should compile the product source, or set
`OMEGA_PRODUCT_PROGRAM` to reuse one exact product executable during a focused
iteration. The gate deliberately selects neither an arbitrary existing
`target/debug/omega` nor an ambient host target as acceptance evidence.

## Retention inventory

| Retained child | Product role | Deletion or absorption condition |
| --- | --- | --- |
| `build.omg` | Declares the target-neutral `psi` package consumed by Omega's product build. | Delete only if ordinary package ownership replaces this root atomically. |
| `source/` | Owns bounded source bytes and coordinates shared by the lexer and parser. | Absorb when a replacement representation preserves every live source/coordinate discriminator. |
| `tokens/` | Owns the lexical token stream and its observation surface. | Absorb when the next canonical representation subsumes the lexer/parser handoff and its gates. |
| `syntax/` | Owns the bounded structural syntax retained by the current parser slice. | Absorb into a later Psi representation only with equivalent accepted/rejected observations. |
| `lex/` | Owns the Unicode tables and source-to-token implementation used directly by `omega/main.omg`. | Delete or absorb only when the canonical lexer and all lexical boundary cases move together. |
| `parse/` | Owns the token-to-structural-parse implementation and black-box observation decoder. | Delete the decoder when a cheaper product gate covers the same observation; absorb the parser only into its canonical successor. |
| `test-parser.sh` | Builds one exact product executable and exercises the live lexical/parser boundary. | Delete when an equal or stronger product-source gate subsumes every retained failure class. |

Generated data is retained under the semantic phase that consumes it. The
Unicode tables therefore live in `lex/`; there is no generic `generated/`
source owner or generator runtime in the compiler closure.
