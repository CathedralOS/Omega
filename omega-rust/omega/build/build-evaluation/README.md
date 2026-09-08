# Build evaluation

[src/lib.rs](src/lib.rs) owns checked Build output, filesystem observation
records, and the conversion to concrete build intent. Language rules belong to
the [build specification](../../../../wiki/spec/build/execution.md) and
[observation contract](../../../../wiki/spec/build/observations.md).

- [Observation custody](observation_custody.md): operand preparation, handles,
  observed carriers, and deterministic resource accounts.
- [Replay](replay.md): current supported sequences and exact failure models.
- [src/replay_record.rs](src/replay_record.rs): bounded canonical records and
  conversion to interpreter replay inputs.
- [src/observation_identity.rs](src/observation_identity.rs): commitment encoding.

These notes describe the current Rust implementation, not a history of schema
versions. Supported replay is deliberately narrower than all admitted build
operations. Extending it needs a concrete customer and complete observation
custody, not a goal of filling every operation combination.
