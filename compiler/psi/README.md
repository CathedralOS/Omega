# Psi product source

This root is reserved for the eventual Omega-written, target-neutral Psi half
of the production compiler. Its source closure is constrained to `Ωself` and
compiled by `omega-bootstrap`; it is not the Delta implementation of the bridge
frontend.

The `Ωself` constraint governs features used by this compiler source, not the
Omega programs the resulting compiler accepts. Standalone terminal-Psi
interpreters, proof explorers, and other tools remain outside this closure
unless the compiler executable imports them.

The current Rust implementation is explicitly transitional and lives at
`bootstrap/onramps/omega-rust/psi/`. Do not place new Rust crates here.
