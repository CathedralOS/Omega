# Shared foundations

These separate crates hold infrastructure and types shared by compiler
stages and product subsystems:

- [bounded-process](bounded-process/README.md): bounded child processes:
  output capture, termination and cleanup on supported hosts.
- [platform-custody](platform-custody/src/lib.rs): filesystem object identity,
  access-control observations and retained file operations.
- [function-identity](function-identity/src/lib.rs): native function and state
  identities shared by lowering and realization.
- [optimization-core](optimization-core/src/lib.rs): phase selections,
  optimization contracts, identities and canonical evidence types.
- [artifacts](artifacts/README.md): per-file output writing and opt-in
  allocation/timing measurements, without semantic records or backend dependencies.

They depend on host libraries and Psi foundations, never on program IRs,
producing stages or product coordinators. Source storage, arenas, symbols,
numerics and target-neutral optimization selections stay in
[Psi foundation](../../psi/foundation).

Semantic trust records and their validation belong to
[realization admission](../compilation/realization/trust-model/README.md).
