# Shared foundations

These independent crates provide infrastructure and vocabulary used across
compiler phases and product subsystems:

- [bounded-process](bounded-process/README.md): bounded child execution,
  stream capture, termination and cleanup on supported hosts.
- [platform-custody](platform-custody/src/lib.rs): filesystem object identity,
  access-control observations and retained file operations.
- [function-identity](function-identity/src/lib.rs): native function and state
  identities shared across lowering and realization.
- [optimization-core](optimization-core/src/lib.rs): phase selections,
  optimization contracts, identities and canonical evidence vocabulary.

Dependencies point to host libraries and Psi foundations, never to program IRs,
producing stages or product coordinators. Source storage, arenas, symbols,
numerics and target-neutral optimization selections remain in
[Psi foundation](../../psi/foundation/).

Compiler artifact records and output writing belong to the independent
[artifacts package](../compiler/artifacts/README.md) under compiler ownership.
