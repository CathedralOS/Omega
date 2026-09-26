//! Normalized executable-artifact admission and installation.
//!
//! Start at `executable_installation.rs`: it owns the authority-consuming
//! ladder — admit, materialize and freeze, validate, install, seal an entry,
//! retire — and the one-call drives that run a provider operation through the
//! transition consuming its receipt. Container decoding and materialization
//! alone never establish execute authority.
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
mod entry_references;
mod executable_installation;
mod installation;
mod owned_image_provider;
mod placement;
mod post_handoff_writer;
mod retirement;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

pub use artifacts::container::{
    ArtifactRelocationKind, ContainerLimits, ContainerSection, ContainerSectionKind,
    DecodedArtifactContainer, DecodedArtifactRelocation, OMEGA_EXECUTABLE_CONTAINER_MARKER,
    OMEGA_EXECUTABLE_CONTAINER_V1_MARKER, OMEGA_EXECUTABLE_CONTAINER_V2_MARKER,
    ValidatedArtifactContainer, ValidatedContainerAdmissionEvidence, admit_validated_container,
    non_authoritative_decoded_container_fingerprint, normalized_decoded_content_digest,
    normalized_proof_payload_digest, validate_decoded_container,
};
pub use artifacts::container_bytes::{
    OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES, OMEGA_EXECUTABLE_CONTAINER_MAGIC,
    OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES, decode_executable_container,
    encode_executable_container, encode_executable_container_v1_compatibility,
    non_authoritative_informational_section_fingerprint,
};
pub use artifacts::{
    AdmittedArtifact, Artifact, ArtifactAdmissionEvidence, ArtifactEntry, InstallationAudience,
};
pub use authority_digests::{
    AdmissionReceiptId, ArtifactAuthorityCommitments, ArtifactContentDigest, ArtifactId,
    CodePlacementId, DeclaredFootprintDigest, DestinationPreparationReceiptId, EntryContractDigest,
    EntryReferenceFactDigest, EntrySetId, FinalBytesDigest, FinalValidationId,
    ImportedContractSetDigest, InstallationFactDigest, InstallationScopeDigest,
    InstallationScopeId, InstalledCodeId, MachineContractSetId, MachineFootprintId,
    MachineRegimeDigest, MappingQuarantineId, NonAuthoritativeContainerFingerprint64,
    NonAuthoritativeInformationalFingerprint64, NonAuthoritativeWriterContextFingerprint64,
    PlacementPlanId, ProofPayloadDigest, RelocationSetId, ReplacementFactDigest,
    RetirementFactDigest,
};
pub use entry_references::{
    EntryReferenceAuthority, EntryReferenceError, EntryReferenceReceipt, InstalledEntryReference,
};
pub use executable_installation::{
    CallCodeError, InstallCodeError, QuarantineCodeError, ReplaceCodeError, RetireCodeError,
    SealCodeEntryError, UninstallCodeError, admit_executable, install_validated,
    materialize_and_freeze, retire_installed, validate_final_placement,
};
pub use installation::{
    InstallAuthority, InstallationDiagnostic, InstallationError, InstallationReceipt,
    InstallationRegistryAuthority, InstalledCode, InstalledCodeContext, WxEnforcement,
};
pub use owned_image_provider::{
    OWNED_IMAGE_BYTES_STORED, OWNED_IMAGE_FETCH_VISIBILITY_READBACK,
    OWNED_IMAGE_PATCH_FETCH_VISIBILITY, OWNED_IMAGE_PATCH_SITES_WRITTEN,
    OWNED_IMAGE_PATCH_WRITE_RESUMPTION, OWNED_IMAGE_PATCH_WRITE_RESUSPENDED,
    OWNED_IMAGE_RETIRE_EXECUTE_DISABLED, OWNED_IMAGE_RETIRE_EXECUTORS_QUIESCED,
    OWNED_IMAGE_RETIRE_WRITE_AUTHORITY_RESTORED, OWNED_IMAGE_SEAL_COMMITTED_CONTENT,
    OWNED_IMAGE_SEAL_DECLARED_ENTRY, OWNED_IMAGE_SEAL_FETCH_VISIBILITY,
    OWNED_IMAGE_STORE_ORDER_FENCE, OWNED_IMAGE_WRITE_TO_EXECUTE, OwnedImageProvider,
    ResidentEntryCall,
};
pub use placement::materializer::{MaterializedArtifactBytes, materialize_admitted_artifact};
pub use placement::{
    CodePlacement, CodePlacementAuthority, FinalValidationCertificate, FrozenPlacement,
    FrozenPlacementError, MaterializationError, MaterializationReceipt, PlacementClaimError,
    ValidatedPlacement,
};
pub use post_handoff_writer::{
    DestinationClaimError, DestinationPreparationReceipt, DestinationWriteError,
    PreparedPostHandoffWriterDestination, PreparedPostHandoffWriterDestinationValidationError,
    ResolvedPostHandoffEntryWriterContext, ValidatedPreparedPostHandoffWriterDestination,
    ValidatedWrittenPostHandoffWriterDestination, WrittenPostHandoffWriterConsumerValidationError,
    WrittenPostHandoffWriterDestination,
};
pub use retirement::quarantine::{
    MappingQuarantineCause, MappingQuarantineError, MappingQuarantineReceipt,
    QuarantinedInstallation, StaleEntryFault, quarantine_installed,
};
pub use retirement::replacement::{
    ReplacementAuthority, ReplacementError, ReplacementOutcome, ReplacementReceipt,
    replace_installed,
};
pub use retirement::uninstall::{UninstallError, UninstallOutcome, uninstall_installed};
pub use retirement::{
    RetiredInstallation, RetirementAuthority, RetirementError, RetirementReceipt,
};
