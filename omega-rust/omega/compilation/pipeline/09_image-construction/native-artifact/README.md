# Native artifact custody

Start at [native_artifact.rs](src/native_artifact.rs). `NativeArtifact` holds
the canonical Terminal artifact, target object, executable image, selected
provider records and replay evidence together.

Its [requested-image construction](src/native_artifact.rs) takes
`NativeArtifactEmissionParts<ExecutableImageEmissionRequest>`, emits the image,
and calls the Direct or the Dynamic-ELF constructor, which carry different
authority. `RequestedNativeArtifact::emit` accepts only records that no longer
reference source, and writes no files.
`NativeSelectedProviderPlan::project_selection` copies the kept provider
selection evidence into those records here in the backend. It keeps each
plan's strong digest and exact closed list of requirements; specialized
requirements replace template identities, as in provider admission. The
compiler passes selection evidence through without building that list itself.
The two image products share the emission-parts shape, not result type or
replay.

The rest of the lifecycle is in `native_artifact.rs`.
`from_emitted_parts` derives physical evidence once and seals the artifact
over the parts this compile produced, without replaying them.
`from_replayed_parts` rebuilds an artifact from kept records, and `validate`
replays the cross-checks before it can be returned. Checks of a receiver's
policy belong to explicit ecosystem admission, not to a default deny-all check
on emission. Structural validation alone grants a receiver no authority;
removing the mandatory producer check is tracked by
`TWO-AXIS-TERMINAL-AUTHORITY-REVIEW`.

The supporting files:

- [dynamic_elf.rs](src/dynamic_elf.rs): the separate dynamic-ELF product,
  which cannot be installed, with its own reconstruction and validation.
- [provider_execution.rs](src/provider_execution.rs): rebuilds the required
  provider executions from Terminal boundaries and foreign calls, checks each
  exact stack contribution, then compares the complete set of selected-plan
  reports. Direct artifacts supply image rows; dynamic ELF supplies object
  rows after replaying its own image. Both use the same requirement checks.
- [identity.rs](src/identity.rs): serializes the exact identity fields,
  including foreign-call records. It does not replace replaying those fields.
- [native_artifact.rs](src/native_artifact.rs) and
  [mixed_structural_scalar.rs](src/mixed_structural_scalar.rs):
  narrow semantic-to-object checks used by both products.

[publication.rs](src/publication.rs) defines the digest domains for
executable publication, checks the producer's executable evidence and writes
self-verifying receipts. `PreparedExecutablePublication` checks the image's
required text and function evidence before installation, then binds its
receipt to the installed path. The compiler chooses products and installs
files; it does not build the native digest chain. Replaying a receipt detects
changed content or file mode but grants no receiving or execution authority.
`DynamicElfPublicationReceipt` gives a dynamically linked ELF product its own
flat-file receipt in a separate digest domain, so dynamic bytes never pass as
a direct image.

[callable_entry.rs](src/callable_entry.rs) is the separate entry point for
staging and replaying optimized ordinary callable entries; its model,
reconstruction and codec are under `callable_entry/`.
[fragment_object_publication.rs](src/fragment_object_publication.rs) (`publish_fragment_object`) binds a
selected physical-entry adapter to its emitted callable and checked entry plan.
The image backend's `physical_entry` module implements the ABI bridge: it
calls that ordinary source adapter and keeps its actual return value. The
bridge does not invent semantic storage roots or replace the adapter's handoff.
[physical/mod.rs](src/physical/mod.rs) rebuilds physical evidence
independently and holds the records it keeps. In
[evidence.rs](src/physical/derivation/evidence.rs), derivation matches the
surviving occurrences back to their sources and sends their boundary evidence
to three files:

- [hosted_builtins.rs](src/physical/derivation/hosted_builtins.rs) holds the
  hosted builtin catalog and the records of each builtin's arguments, result
  and emitted bytes.
- [provider_settlements.rs](src/physical/derivation/provider_settlements.rs)
  matches admitted provider settlements and accounts for their port effects.
- [foreign_calls.rs](src/physical/derivation/foreign_calls.rs) rebuilds
  normalized foreign-call evidence, including argument placement, callbacks
  and import relocations. Its
  [fragment_custody.rs](src/physical/derivation/fragment_custody.rs) matches
  imports published by fragments to selected-call records; the nested
  `floating_control` module checks the saved and restored control state.

Settlement identity encoding and provider-execution records are shared files
beside these. Missing coverage produces an explicit evidence gap;
contradictory kept records are an error.

This crate takes inputs that no longer reference source. It does not select
source providers, install an artifact or publish executable files, and it
grants no publication, installation, provider or runtime authority.

Runtime provision capture reads
[`AdmittedRuntimeInput`](../../../representations/abstract-operations/verified-optimization-input/src/runtime_input.rs):
the authorized artifact, selected entry and target, admitted boundary roster,
and completed scalar applications. The verified input lends those exact rows
without lending its executable abstract graph. The initial lowering stage owns
the [entry correspondence check](../../../pipeline/00_terminal-psi-to-abstract-operations/src/entry_receiver.rs)
before object construction; this crate consumes the resulting entry settlement.

## Physical evidence scope

Physical evidence covers a deliberately narrow set: an unoptimized handoff;
exact D29 records for supported compiler-intrinsic and checked-body operator
applications; supported hosted byte input/output and process-exit settlements
on Linux x86-64/AArch64 and macOS AArch64; and admitted-provider D41 records
for supported normalized foreign calls. Each surviving physical occurrence has
one replayable child bound to its D29 or D41 parent, and the scope is part of
the artifact's identity. Unsupported D29/D41 roles and later optimization
phases produce no D32 evidence, but the native artifact stays usable. A
consumer that needs final-realization evidence must reject an artifact without
it.

`RequestedNativeArtifact::emit_admitted` builds the optimized scope,
`ValidatedOptimizedProjection`, through
`NativePhysicalEvidenceScope::from_fragment_publication` from the published
fragment object and its boundary-application coverage, and refuses an
artifact without that coverage. The scope carries the fragment-publication
records that three boundary cases match against selected-plan rows: an
installed-provider internal call, a structural boundary result, and a kernel
entry without a symbolic import. `NativePhysicalEvidenceScope::Unavailable`
carries no evidence.

Structural results are supported for one affine record with no claims and one
relevant signed 64-bit integer field, under a rooted fixed token. On Linux
x86-64 and AArch64 the compiler handles it exactly through source
construction, one owned Unit-call transfer, structural return with mixed
inputs, mandatory caller discard, native argument materialization,
object/image replay and installation. Wider records, projections, borrows,
services and content evidence are not supported yet.
