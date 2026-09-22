# Native artifact custody

Start at [native_artifact.rs](src/native_artifact.rs). `NativeArtifact` keeps
the canonical Terminal artifact, target object, executable image, selected
provider projections, and replay evidence together.

Its lifecycle is visible there: `from_emitted_parts` derives physical evidence,
`from_replayed_parts` reconstructs retained custody, and `validate` replays the
joins before the artifact can be returned. Receiving-policy checks belong to
explicit ecosystem admission, not a default deny-all gate on emission. Structural
validation alone grants no receiving authority; the mandatory producer gate's
removal is tracked by `TWO-AXIS-TERMINAL-AUTHORITY-REVIEW`.

Follow its subordinate owners for:

- [dynamic_elf.rs](src/native_artifact/dynamic_elf.rs): the distinct,
  non-installable dynamic-ELF product and its own reconstruction and validation.
- [identity.rs](src/native_artifact/identity.rs): exact identity-field
  serialization, including foreign-call custody. This is not a substitute for
  replaying those fields.
- [boundary_applications.rs](src/native_artifact/boundary_applications.rs) and
  [mixed_structural_scalar.rs](src/native_artifact/mixed_structural_scalar.rs):
  focused semantic-to-object checks used by both products.

[callable_entry.rs](src/callable_entry.rs) is the separate entrance for staging
and replaying optimized ordinary callable entries. Its model, reconstruction,
and codec live under `callable_entry/`. Follow
[physical/derivation.rs](src/physical/derivation.rs) for independent physical
evidence reconstruction and [physical/mod.rs](src/physical/mod.rs) for its
retained carriers.

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
