# Layout plans

Normalized programmable-layout plans and symbolic materialization. Layout
policies describe geometry; a materializer consumes validated geometry plus
compiler-issued symbolic values. Source programs never receive numeric code
addresses or a byte-patching primitive.

[lib.rs](src/lib.rs) is the entry map. Each file owns one concept:

- [layout_reports.rs](src/layout_reports.rs): validated geometry reports,
  conventional sum layouts, replay fingerprints and matching.
- [materialization.rs](src/materialization.rs): ordinary scalar and aggregate values
  supplied to a materializer (`field_values`), field identity keys (`field_identities`),
  stored-integer fit validation and container reads/writes (`stored_integer_writes`),
  scalar/aggregate codecs (`field_codec`), and materialization plan application.
- [lib.rs](src/lib.rs): entry stub and data symbol identities, relocation targets,
  symbolic field paths, interior layouts (`symbolic_values`), placement constraints
  and portable materialization actions (`placement`), and default sum packing
  (`aggregate_geometry`).
- [symbolic_materialization.rs](src/symbolic_materialization.rs): derivation
  of a phase-aware plan from a report, symbolic values, and resolved targets.

Everything above is portable description and semantic validity. The
post-handoff writer consumers live on the Omega side:
`backend/plans/post-handoff-writer-plan` derives writer programs and reusable
fragments from these actions, `backend/plans/program-entry-plan` lowers and
validates them against ISA encoders, and
`backend/runtime/executable-installation` executes them against a concrete
destination. The interpreter consumes the same portable actions through its
own materialization path, never through native fragment lowering.

Run the crate's tests with `cargo nextest run -p layout-plans --no-fail-fast`.
