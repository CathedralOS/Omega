# Psi artifact codec

The artifact codec seals a [Psi program](../../../../../../wiki/spec/terminal-psi/product.md) and packages its sections for another stage or host to reopen. It re-exports [terminal-wire](../wire/README.md), which validates and encodes the program according to the [canonical encoding specification](../wire/encoding.md). The [receiving host](../authorization/authorization.rs) decides whether to trust the producer.

Native code encodes structural domains, access plans and quotient declarations through [declaration encoding](../declaration-encoding/src/lib.rs). Other native consumers use the wire codecs for the records they carry. `mbx xtask check-boundary` keeps representations after stage 02 independent of the artifact codec.

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
