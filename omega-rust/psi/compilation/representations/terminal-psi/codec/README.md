# Terminal codec

This crate owns the sealed Terminal Psi module and the artifact envelope that
carries it between stages. The canonical encoding, validation and vocabulary
encoders live in [terminal-wire](../wire/README.md); this crate re-exports all
of it, so its users name one crate. A native form that needs only a vocabulary
encoder depends on `terminal-wire` instead, and `mbx xtask check-boundary`
refuses any path from a form after stage 02's preparation to this crate.

Public contract: [Terminal product](../../../../../../wiki/spec/terminal-psi/product.md).
The [canonical encoding contract](../../../../../../wiki/spec/terminal-psi/encoding.md)
defines canonical form and section identity.

- [closed_module.rs](src/closed_module.rs) seals a module: it keeps the
  module, its closed type catalog, canonical bytes and identity together, and
  binds obligation evidence to that exact identity.
- [canonical_artifact.rs](src/canonical_artifact.rs) packs semantic,
  optimization and optional debug bytes into the envelope passed between
  stages. A producer hands over its module; an artifact read from outside
  is decoded once and kept. Views borrow that storage without decoding,
  failure or replay caches. The manifest is rebuilt from the sections.
- [entry_receipt.rs](src/entry_receipt.rs) joins an exact source declaration
  to one canonical artifact.

Sealing shows valid encoding and structure, not that a checker accepted the
program or that its producer can be trusted.
