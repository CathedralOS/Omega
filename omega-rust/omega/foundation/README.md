# Shared host foundations

These independent crates provide host infrastructure used across compilation,
package management and application workflows:

- [bounded-process](bounded-process/README.md): bounded child execution,
  stream capture, termination and cleanup on supported hosts.
- [platform-custody](platform-custody/src/lib.rs): filesystem object identity,
  access-control observations and retained file operations.

Their dependencies point to host libraries, never to compiler stages,
representations or subsystem policy. Source storage, arenas, symbols and numeric
primitives remain in [Psi foundation](../../psi/foundation/).

Compiler artifact records and output writing belong to the independent
[artifacts package](../compiler/artifacts/README.md) under compiler ownership.
