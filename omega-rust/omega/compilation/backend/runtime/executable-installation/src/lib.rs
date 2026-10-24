//! Normalized executable-artifact admission and installation.
//!
//! Start at `executable_installation.rs`: it owns the authority-consuming
//! ladder — admit, materialize and freeze, install, seal an entry,
//! retire — and the one-call drives that run a provider operation through the
//! transition consuming its receipt. There is no final-certificate replay;
//! installation tests construct producer output directly. Container decoding
//! and materialization alone never establish execute authority.
//!
//! The domains beside it: `authority_digests` (domain-separated digests and
//! report-only fingerprints), `artifacts` (artifacts, their container wire
//! format and admission), `placement` (placement claims, materialization,
//! frozen and validated placements), `installation` (install authorities,
//! receipts, installed code and its registry), `entry_references` (the
//! control-flow-integrity gate), `retirement` (retirement, uninstall,
//! replacement and quarantine endings), `post_handoff_writer` (the installed
//! entry writer protocol) and `owned_image_provider` (the resident-buffer
//! provider that performs the operations and mints the receipts).

mod artifacts;
mod authority_digests;
mod fnv1a;
mod installation;
mod placement;
mod post_handoff_writer;
mod retirement;

pub use artifacts::Artifact;
pub use authority_digests::{
    AdmissionReceiptId, ArtifactContentDigest, ArtifactId, CodePlacementId,
    DeclaredFootprintDigest, DestinationPreparationReceiptId, EntryContractDigest,
    EntryReferenceFactDigest, EntrySetId, FinalBytesDigest, FinalValidationId,
    ImportedContractSetDigest, InstallationFactDigest, InstallationScopeDigest,
    InstallationScopeId, InstalledCodeId, MachineContractSetId, MachineFootprintId,
    MachineRegimeDigest, MappingQuarantineId, NonAuthoritativeContainerFingerprint64,
    NonAuthoritativeInformationalFingerprint64, NonAuthoritativeWriterContextFingerprint64,
    PlacementPlanId, RelocationSetId, ReplacementFactDigest, RetirementFactDigest,
};
pub use installation::{InstallationError, InstalledCode};
pub use post_handoff_writer::{
    ResolvedPostHandoffEntryWriterContext, ValidatedPreparedPostHandoffWriterDestination,
};
