# Checked-program manifests

Start with the report you are investigating:

- [Machine contracts](src/machine_contract_manifest.rs): public contracts,
  specializations, and retained implementation evidence.
- [Qualifications](src/qualification_manifest.rs): establishment origins,
  receipts, and exact program points.
- [Claim outcomes](src/claim_outcome_manifest.rs): ownership outcomes, content
  projections, conservation, and partition relationships.
- [Capabilities](src/capability_manifest.rs): the selected entry's service reach
  and component composition.
- [Carry](src/carry_manifest.rs) and [task activation](src/task_activation_manifest.rs):
  checked carry policies and provider-independent activation demands.
- [Index compatibility](src/index_compatibility_manifest.rs): named conditions
  and their discharge routes.
- [Executable TCB](src/executable_tcb_manifest.rs): selected provider realizations
  and executable trust boundaries.

Each manifest owns its validation and rendering. Shared coordinate labels live
in `manifest_coordinates.rs`; shared JSON value encoders live in
`manifest_values.rs`. Tests sit beneath their manifest, with test-only program
builders in `test_support.rs`.

The [compiler observation owner](../../compiler/compiler/src/pipeline/artifacts.rs)
chooses report filenames and sequences writes through `ArtifactWriter`. These
writers return strings, perform no filesystem I/O, and grant no authority.
Inconsistent input currently panics rather than producing partial evidence;
the negative tests preserve those rejection conditions. Changing that error
contract is separate from how the reports are organized.
