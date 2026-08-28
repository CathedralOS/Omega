# Psi product source

This root owns the Omega-written, target-neutral Psi half of the production
compiler. The current live slice contains source/span and token
representations, the Unicode-aware lexer, and a fail-closed whole-file parser
for ordinary `use path::member;` roots. The parser retains ordered source spans
in bounded owner-local syntax tables and rejects unsupported roots instead of
inventing an opaque bootstrap tree. Its source closure is being authored
against the ordinary-Omega surface that the Delta-produced compiler must
eventually accept; that compiler is not yet published.

This authoring constraint governs features used by the compiler source, not the
Omega programs the resulting compiler accepts. Standalone terminal-Psi
interpreters, proof explorers, and other tools remain outside this closure
unless the compiler executable imports them.

The current Rust implementation is explicitly transitional and lives at
`source/omega-rust/psi/`. It remains a differential comparator; no
Rust implementation belongs here.

`test-parser.sh` compiles the product entrypoint once and runs the parser's
acceptance, rejection, capacity-edge, lexical-handoff, and determinism cases
against that one native artifact. Its Python helper only decodes the versioned
black-box observation; it implements no compiler semantics. Set
`OMEGA_PRODUCT_PROGRAM` to reuse an already-built product executable during a
focused iteration.
