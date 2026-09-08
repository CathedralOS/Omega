# Final footprint replay

[Machine-state evidence](../../../../../wiki/spec/build/machine_state_evidence.md)
specifies the required final-artifact contract. The typed model and replay live
in [footprint_certificate.rs](src/footprint_certificate.rs); executable-region
inventory lives in [model/executable_regions.rs](src/model/executable_regions.rs).

The certificate retains normalized coverage classes, completeness and missing
sets, final placement, compiler-text derivation, and region inventory in one
checked identity. Current coverage distinguishes compiler instructions and
checked-assembly catalog rows from import thunks and other executable origins.
Absent-by-construction claims apply only to a producer that actually excludes
those origins; they are not exemptions for newly emitted code.

The retained certificate path has no production caller. `FinalFootprintCertificate::current`,
`bind_compiler_entry_footprint`, and `validate_placed_executable_region_inventory`
are exercised by tests, not the shipped compilation route. Their tests and the
architecture test's source-text assertions do not establish production coverage.
Do not describe native output as carrying a complete replayed footprint until
its current route supplies the required rows and consumes the certificate.

Integration belongs to the existing
[translation-validation work](../../../../../TASKS_OPTIMIZER.md#validation-translation-and-publication):
bind current instruction/region production through final placement and exercise
missing, substituted, incomplete, and state-ceiling-exceeding evidence via the
real publication path. An accepted isolated certificate constructor is not that
acceptance condition.
