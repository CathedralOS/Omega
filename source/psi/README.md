# Psi product source

This root owns the Omega-written, target-neutral Psi half of the production
compiler. The current live slice contains source/span and token
representations plus the Unicode-aware source-to-token phase. Its
source closure deliberately uses only ordinary-Omega forms accepted by the
published Delta-produced compiler.

This authoring constraint governs features used by the compiler source, not the
Omega programs the resulting compiler accepts. Standalone terminal-Psi
interpreters, proof explorers, and other tools remain outside this closure
unless the compiler executable imports them.

The current Rust implementation is explicitly transitional and lives at
`source/omega-rust/psi/`. It remains a differential comparator; no
Rust implementation belongs here.
