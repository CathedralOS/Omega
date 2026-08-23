# Psi product source

This root is reserved for the eventual Omega-written, target-neutral Psi half
of the production compiler. Its source closure is constrained to `Ωself` and
compiled by `omega-bootstrap`; it is not the Delta implementation of the bridge
frontend.

The current Rust implementation is explicitly transitional and lives at
`bootstrap/onramps/omega-rust/psi/`. Do not place new Rust crates here.
