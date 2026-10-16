# Native artifact custody

Start at [native_artifact.rs](src/native_artifact.rs). `NativeArtifact` keeps
the canonical Terminal artifact, target object, executable image, selected
provider projections, and replay evidence together.

Its [requested-image construction](src/native_artifact/emission.rs) consumes
`NativeArtifactEmissionParts<ExecutableImageEmissionRequest>`, emits the image,
and dispatches to the authority-distinct Direct or Dynamic-ELF constructor.
`RequestedNativeArtifact::emit` accepts only source-free custody and writes no
files. `NativeSelectedProviderPlan::project_selection` projects the retained
selection evidence into those source-free records at this backend owner. It
preserves each plan's strong digest and exact closed requirement roster;
specialized requirements replace template identities as in provider admission.
The compiler forwards selection evidence without assembling that roster itself.
Both image products share the emission-parts shape without sharing their
result type or independent replay.

The remaining lifecycle is visible in `native_artifact.rs`:
`from_emitted_parts` derives physical evidence once and seals the artifact
over the parts the same compile produced, without replaying them;
`from_replayed_parts` reconstructs retained custody, and `validate` replays the
joins before such an artifact can be returned. Receiving-policy checks belong to
explicit ecosystem admission, not a default deny-all gate on emission. Structural
validation alone grants no receiving authority; the mandatory producer gate's
removal is tracked by `TWO-AXIS-TERMINAL-AUTHORITY-REVIEW`.

Follow its subordinate owners for:

- [dynamic_elf.rs](src/native_artifact/dynamic_elf.rs): the distinct,
  non-installable dynamic-ELF product and its own reconstruction and validation.
- [provider_execution.rs](src/native_artifact/provider_execution.rs): reconstruct
  required provider executions from Terminal boundaries and foreign calls,
  validate exact stack contributions, then compare the complete selected-plan
  report set. Direct artifacts supply image rows; dynamic ELF supplies object
  rows after its own image replay. Both use the same requirement checks.
- [identity.rs](src/native_artifact/identity.rs): exact identity-field
  serialization, including foreign-call custody. This is not a substitute for
  replaying those fields.
- [boundary_applications.rs](src/native_artifact/boundary_applications.rs) and
  [mixed_structural_scalar.rs](src/native_artifact/mixed_structural_scalar.rs):
  focused semantic-to-object checks used by both products.

[publication.rs](src/publication.rs) owns the executable publication digest
domains, producer-side executable evidence validation and self-verifying receipts.
`PreparedExecutablePublication` admits the image's required text/function
evidence before installation, then binds its receipt to the installed path.
The compiler chooses products and installs files; it does not assemble the
native digest chain. Receipt replay detects content and mode drift without
granting receiving or execution authority. Its
[dynamic_elf.rs](src/publication/dynamic_elf.rs) gives a dynamically linked
ELF product its own flat-file receipt in a separate digest domain, so dynamic
bytes never satisfy direct-image publication custody.

[callable_entry.rs](src/callable_entry.rs) is the separate entrance for staging
and replaying optimized ordinary callable entries. Its model, reconstruction,
and codec live under `callable_entry/`.
[fragment_object_publication.rs](src/fragment_object_publication.rs) binds a
selected physical-entry adapter to its emitted callable and checked entry plan.
The image backend's `physical_entry` owner implements the ABI bridge; it calls
that ordinary source adapter and preserves its actual return value. The bridge
does not synthesize semantic storage roots or replace the adapter's handoff.
Follow
[physical/derivation.rs](src/physical/derivation.rs) for independent physical
evidence reconstruction and [physical/mod.rs](src/physical/mod.rs) for its
retained carriers. The derivation coordinator in
[evidence.rs](src/physical/derivation/evidence.rs) rejoins surviving occurrences
and dispatches their boundary evidence to three owners:

- [hosted_builtins.rs](src/physical/derivation/hosted_builtins.rs) owns the hosted
  builtin catalog and its argument, result, and emitted-byte custody.
- [provider_settlements.rs](src/physical/derivation/provider_settlements.rs)
  rejoins admitted provider settlements and accounts for their port effects.
- [foreign_calls.rs](src/physical/derivation/foreign_calls.rs) reconstructs
  normalized foreign-call evidence, including argument placement, callbacks,
  and import relocations. Its
  [fragment_custody.rs](src/physical/derivation/foreign_calls/fragment_custody.rs)
  rejoins fragment-published imports and selected-call records; the sibling
  `floating_control.rs` checks the saved and restored control-state envelope.

Settlement identity encoding and provider-execution custody remain shared,
named owners beside these derivations. Missing coverage still produces an
explicit evidence gap; contradictory retained custody remains an error.

This crate consumes source-free inputs. It does not select source providers,
install an artifact, or publish executable files.
It grants no publication, installation, provider, or runtime authority.

## Physical evidence scope

The current physical lane is deliberately narrow: an unoptimized handoff or a
verified Psi-phase survivor projection, exact D29 custody for supported
compiler-intrinsic and checked-body operator applications, supported hosted byte
input/output and process-exit settlements on Linux x86-64/AArch64 and macOS AArch64,
and admitted-provider D41
custody for supported normalized foreign calls. Each surviving physical
occurrence has one replayable child bound to its D29 or D41 parent, and that
scope is bound into artifact identity. Unsupported D29/D41 roles and later
optimization phases yield no D32 evidence while the underlying native artifact
remains usable. Consumers requiring final-realization evidence must reject that
absence.

The rooted fixed-token structural-result cohort includes one claim-free affine
record with one relevant signed 64-bit integer field. Source construction,
single owned Unit-call transfer, mixed-input structural return, mandatory caller
discard, native argument materialization, object/image replay, and installation
transport are exact on Linux x86-64 and AArch64. Wider records, projections,
borrows, services, and content evidence remain outside this cohort.
