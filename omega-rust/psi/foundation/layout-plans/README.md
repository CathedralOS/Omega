# Layout plans

Normalized programmable-layout plans and symbolic materialization. Layout
policies describe geometry; a materializer consumes validated geometry plus
compiler-issued symbolic values. Source programs never receive numeric code
addresses or a byte-patching primitive.

[lib.rs](src/lib.rs) is the entry map. Each file owns one concept:

- [layout_reports.rs](src/layout_reports/mod.rs): validated geometry reports,
  conventional sum layouts, replay fingerprints and matching.
- [field_values.rs](src/materialization/field_values.rs): ordinary scalar and aggregate values
  supplied to a materializer, with their validating schemas.
- [symbolic_values.rs](src/symbolic_values/mod.rs): entry stub and data symbol
  identities, relocation targets, symbolic field paths, interior layouts.
- [placement.rs](src/placement/mod.rs): byte order, consumption instants, phases,
  address ranges, regimes, scopes, and placement constraints.
- [post_handoff_writer.rs](src/post_handoff_writer/mod.rs): the reusable writer
  fragment ABI and the plans a provider replays after handoff.
- [materialization.rs](src/materialization/mod.rs): materialization plans, their
  application to bytes, scalar decoding, and the diagnostic type.
- [symbolic_materialization.rs](src/symbolic_materialization.rs): derivation
  of a phase-aware plan from a report, symbolic values, and resolved targets.
- [stored_integer_writes.rs](src/materialization/stored_integer_writes.rs): scalar fragments,
  stored-integer fit validation, and container reads and writes.
- [materialization_field_identities.rs](src/materialization/field_identities.rs):
  field identity keys shared by report validation and materialization.

Run the crate's tests with `cargo nextest run -p layout-plans --no-fail-fast`.
