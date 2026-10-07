# Psi product source

This sibling package owns the Omega-written, target-neutral Psi half of the
production compiler. `source/omega/` consumes it through the `psi` package
identity; Omega does not own this source tree. The current live slice contains
source/span and token
representations, a UTF-8-framed lexer, and a fail-closed whole-file parser
for ordinary `use path::member;` roots and basic `[pub] data` declarations with
an optional single `[copy]` or `[linear]` multiplicity property and optional
trailing comma, named field types with an optional `in Domain` qualifier, and
payload-free `case Name;` members or structured
`case Name(field: Type, ...);` payloads whose fields accept the same qualifier.
Direct fields and case-payload fields retain separate bounded custody while
mixed declarations preserve exact authored field/case order and shared named
type-reference identity. Empty payload lists and trailing payload commas are
accepted. Unsupported roots, retired inline discriminants, and richer
property/type forms (arrays, multi-domain unions, ranged suffixes) reject
instead of becoming a private bridge tree.

Name resolution follows parsing over the same unit. It rejects any `use`
root, since the unit is the whole source set and supplying more sources is the
compiler request's job. It then rejects builtin-shadowing and duplicate data
names, and duplicate members: fields and cases share one namespace; each
case's payload fields share another with the common fields. Last, it binds
every type reference: an enclosing generic parameter by spelling (never a
lifetime), then a builtin type, then a data declaration of the unit. It also
binds every `in` qualifier to a compiler-known domain (`Wrapping`,
`Saturating`, `Trapping`, `Finite`). The binding tables are indexed by the
syntax table's type-reference handles. Carrier compatibility (`Finite` on
floats only, unbounded `UInt`/`Int` storage) belongs to typing, which does not
exist yet. Its
source closure is being authored against the ordinary-Omega surface that the
Delta-produced compiler must eventually accept; that compiler is not yet
published.

This authoring constraint governs features used by the compiler source, not the
Omega programs the resulting compiler accepts. Standalone terminal-Psi
interpreters, proof explorers, and other tools remain outside this closure
unless the compiler executable imports them.

The maintained Rust implementation lives at `omega-rust/psi/`. It
remains a differential comparator and may continue in parallel; no Rust
implementation belongs in this product subtree.

Each phase has a gate package under `tests/psi/`, driven by
[`tests/psi/gate.sh`](../../tests/psi/gate.sh).
[`tests/psi/parser/test-parser.sh`](../../tests/psi/parser/test-parser.sh)
compiles the gate-owned Omega harness once and runs the parser's
acceptance, rejection, capacity-edge, lexical-handoff, and determinism cases
against that one native artifact. Its Python helper only decodes and compares
versioned parser observations; it implements no compiler semantics. No lexical
wire format or Rust observer is required. Rust's lexer has its own unit tests;
neither implementation must reproduce the other's internal representation.
Set `OMEGA_CLI` to the exact freshly built compiler and `OMEGA_TARGET` to the
selected target profile. Acceptance evidence prints the CLI and Omega artifact
SHA-256 identities beside that target. The gate has no cached-artifact or ambient
`target/debug` lookup; focused iteration may invoke the Python decoder directly
with the explicit Omega executable path. The gate is its own package beside this
root: under it, the gate's lock and build output would be `psi` source. Delete
the gate only when an equal or stronger product-source gate subsumes every
retained failure class, and the harness only when an equal or stronger
semantic-free gate preserves all of its cases.

[`tests/psi/resolver/test-resolver.sh`](../../tests/psi/resolver/test-resolver.sh)
does the same for name resolution. Its harness lexes, parses and resolves, and
observes the diagnostic and every type and domain binding. Lexical or parse
rejection exits without an observation.

On macOS ARM64 both gates build their harnesses natively from a fresh CLI:
all 97 parser cases and all 41 resolver cases pass. Each gate counts each
distinct source it runs as one case. No cached executable may stand in for
that run. Linux and Windows hosts have run neither gate.

The lexer keeps its one canonical mixed `Token` stream; the parser observes it
through a shared borrow forwarded along its state edges.
There is no `TokenObservation`, numeric token array, per-token handoff, raw
parser ordinal, or scalar tag/span cache. Structural parser serialization lives
only in the gate-owned Omega harness; the exact
product entrypoint retains phase driving and exit diagnostics. The same parser
cases and structural observations remain mandatory, and the Python
decoder stays semantic-free. The former 32-case lexical parity matrix and its
numeric token protocol are removed; these parser cases do not replace that
lexical coverage.
Chapter 1 now fixes **LEXICAL-PROFILE-V1**: ASCII
identifiers, space/tab/CR/LF whitespace, byte-preserving literal bodies, and no
codepoint escapes or raw strings. Both maintained lexers reject all retired
XID, `\u{...}`, raw-string, Unicode-whitespace, and raw quoted-newline spellings
through the same profile diagnostic; no compiler source may depend on them.

The parser entrance is [`parse/parser.omg`](parse/parser.omg): it owns `Parser`,
initialization, and whole-file root dispatch. [`parse/input.omg`](parse/input.omg)
owns bounded token selection and trivia traversal;
[`parse/data.omg`](parse/data.omg) owns data-declaration grammar.

## Retention inventory

| Retained child | Product role | Deletion or absorption condition |
| --- | --- | --- |
| `build.omg` | Declares the target-neutral `psi` package consumed by Omega's product build. | Delete only if ordinary package ownership replaces this root atomically. |
| `source/` | Owns bounded source bytes and coordinates shared by the lexer and parser. | Absorb when a replacement representation preserves every live source/coordinate discriminator. |
| `tokens/` | Owns the sole typed lexical token stream the lexer fills and the parser borrows. | Absorb only into a successor representation that preserves the exact typed vocabulary and coordinates without parallel token truth. |
| `syntax/` | Owns the bounded structural syntax retained by the current parser slice, including distinct direct-field and case-payload-field tables. | Absorb into a later Psi representation only with equivalent accepted/rejected observations. |
| `lex/` | Owns the source-to-token implementation for the closed ASCII syntax profile and byte-preserving comment/literal payloads. | Absorb only into a successor that preserves the exact V1 profile, diagnostics, coordinates, and payload bytes. |
| `parse/` | Owns token-to-structural parsing over a token stream the lexer keeps and the parser borrows. | Absorb the parser only into its canonical successor. |
| `resolution/` | Owns the name bindings that annotate the syntax tables by handle, and resolution diagnostics. | Absorb into a later Psi representation only with equivalent bindings and diagnostics. |
| `resolve/` | Owns name resolution from syntax and source bytes to the bindings. | Absorb only into the canonical multi-source resolver. |

Generated data belongs under the semantic phase that consumes it. No retained
lexical contract consumes the current Unicode identifier table, so it must be
deleted rather than moved into a generic `generated/` owner.
