//! Normalized executable-artifact admission and installation ladder.
//!
//! No operation converts arbitrary bytes into executable memory. Immutable
//! artifacts are admitted once and reused; each installation instead consumes
//! one exact destination authority through frozen and validated states.
//!
//! The transitions below follow lifecycle order. Callers supply the provider
//! evidence between transitions; the state carriers and receipt checks follow.
//! `post_handoff_writer` owns the separate installed-entry writer protocol.
//!
//! This file owns the five lifecycle entry points. `authority_digests.rs`
//! derives the domain-separated authority digests, `artifacts.rs` carries
//! artifacts and their admission, `code_placement.rs` placement claims,
//! materialization and final validation, `entry_references.rs` the
//! control-flow-integrity gate that seals one admitted entry into a
//! requirement-compatible reference, `installation.rs` installed code
//! and its registry, `retirement.rs` retirement receipts, `replacement.rs` the
//! patch-then-drain replacement join, `uninstall.rs` the drain-or-quarantine
//! join; `container.rs`, `container_bytes.rs`,
//! `materializer.rs`, `post_handoff_writer.rs` and `replacement_quarantine.rs`
//! carry the container, writer and quarantine. `owned_image_provider.rs` is a
//! provider that performs the install and patch operations over resident image
//! buffers it owns, minting the receipts the transitions consume.

mod artifacts;
mod authority_digests;
mod code_placement;
mod container;
mod container_bytes;
mod entry_references;
mod installation;
mod materializer;
mod owned_image_provider;
mod post_handoff_writer;
mod replacement;
mod replacement_quarantine;
mod retirement;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
mod uninstall;

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
pub use code_placement::{
    CodePlacement, CodePlacementAuthority, FinalValidationCertificate, FrozenPlacement,
    FrozenPlacementError, MaterializationError, MaterializationReceipt, PlacementClaimError,
    ValidatedPlacement,
};
pub use container::*;
pub use container_bytes::*;
pub use entry_references::{
    EntryReferenceAuthority, EntryReferenceError, EntryReferenceReceipt, InstalledEntryReference,
};
pub use installation::{
    InstallAuthority, InstallationDiagnostic, InstallationError, InstallationReceipt,
    InstallationRegistryAuthority, InstalledCode, InstalledCodeContext, WxEnforcement,
};
pub use materializer::*;
pub use owned_image_provider::*;
pub use post_handoff_writer::*;
pub use replacement::{
    ReplacementAuthority, ReplacementError, ReplacementOutcome, ReplacementReceipt,
    replace_installed,
};
pub use replacement_quarantine::*;
pub use retirement::{
    RetiredInstallation, RetirementAuthority, RetirementError, RetirementReceipt,
};
pub use uninstall::{UninstallError, UninstallOutcome, uninstall_installed};

use crate::executable_installation::code_placement::{
    CodePlacementEvidence, ValidatedPlacementEvidence,
};
use crate::executable_installation::installation::InstalledCodeEvidence;
use layout_plans::{EntryStubId, PlacementConstraints, RelocationTarget};
use sha2::Sha256;
use target::Architecture;

pub fn admit_executable(
    artifact: &Artifact,
    evidence: ArtifactAdmissionEvidence,
) -> Result<AdmittedArtifact, InstallationDiagnostic> {
    if artifact.0.authority_commitments.is_none() {
        return Err(InstallationDiagnostic(
            "container-v1 compatibility candidates lack strong authority commitments and cannot be admitted"
                .into(),
        ));
    }
    if !evidence.accepted {
        return Err(InstallationDiagnostic(
            "artifact validator did not accept executable eligibility".into(),
        ));
    }
    if evidence.artifact != *artifact {
        return Err(InstallationDiagnostic(
            "artifact admission evidence does not match canonical candidate".into(),
        ));
    }
    Ok(AdmittedArtifact {
        artifact: artifact.clone(),
        admission: evidence.receipt,
        container_proof: None,
    })
}

pub fn materialize_and_freeze(
    artifact: &AdmittedArtifact,
    placement: CodePlacement,
    materialized: MaterializedArtifactBytes,
    receipt: MaterializationReceipt,
) -> Result<FrozenPlacement, Box<MaterializationError>> {
    let mismatch = if materialized.admission_evidence() != artifact {
        Some("canonical materializer output does not retain the exact admitted artifact")
    } else if materialized.placement_evidence()
        != &CodePlacementEvidence::from_placement(&placement)
    {
        Some("canonical materializer output does not retain the exact code placement")
    } else if materialized.placement_plan() != artifact.artifact.0.placement_plan {
        Some("canonical materializer output did not use the admitted placement plan")
    } else if materialized.bytes().len() as u64 != artifact.artifact.0.byte_length {
        Some("canonical materializer output has the wrong executable byte length")
    } else if receipt.materialized != materialized {
        Some("materialization receipt does not bind the exact canonical output")
    } else if placement.extent.length() < artifact.artifact.0.byte_length {
        Some("code placement is smaller than the admitted artifact")
    } else if placement.constraints != artifact.artifact.0.placement_constraints {
        Some("code placement constraints do not match the admitted artifact")
    } else if !receipt.writes_frozen {
        Some("materialization did not freeze write authority over final bytes")
    } else {
        None
    };
    if let Some(message) = mismatch {
        return Err(Box::new(MaterializationError {
            placement,
            materialized,
            receipt,
            diagnostic: InstallationDiagnostic(message.into()),
        }));
    }
    Ok(FrozenPlacement {
        artifact: artifact.clone(),
        placement,
        materialized,
        realized_footprint: receipt.realized_footprint,
    })
}

pub fn validate_final_placement(
    frozen: FrozenPlacement,
    certificate: &FinalValidationCertificate,
) -> Result<ValidatedPlacement, Box<FrozenPlacementError>> {
    let matches = certificate.accepted
        && certificate.artifact == frozen.artifact.artifact
        && certificate.admission == frozen.artifact.admission
        && certificate.placement == CodePlacementEvidence::from_placement(&frozen.placement)
        && certificate.final_bytes_digest == frozen.materialized.final_bytes()
        && certificate.final_bytes == frozen.materialized.bytes()
        && certificate.realized_footprint == frozen.realized_footprint;
    if !matches {
        return Err(Box::new(FrozenPlacementError {
            frozen,
            diagnostic: InstallationDiagnostic(
                "final validation certificate does not match frozen placement".into(),
            ),
        }));
    }
    Ok(ValidatedPlacement {
        frozen,
        validation: certificate.identity,
    })
}

pub fn install_validated(
    validated: ValidatedPlacement,
    authority: InstallAuthority,
    receipt: InstallationReceipt,
) -> Result<InstalledCode, Box<InstallationError>> {
    let evidence = ValidatedPlacementEvidence::from_validated(&validated);
    let mismatch = if authority.validated != evidence {
        Some("install authority is not scoped to this validated placement")
    } else if receipt.validated != evidence {
        Some("installation receipt does not match validated placement")
    } else if !receipt.visibility_complete {
        Some("installation did not complete instruction-fetch visibility")
    } else if receipt.wx == WxEnforcement::Unsupported {
        Some("provider does not support executable installation")
    } else if !authority
        .required_facts
        .is_subset(&receipt.established_facts)
    {
        Some("installation receipt lacks required completion facts")
    } else {
        None
    };
    if let Some(message) = mismatch {
        return Err(Box::new(InstallationError {
            validated,
            authority,
            receipt,
            diagnostic: InstallationDiagnostic(message.into()),
        }));
    }

    Ok(InstalledCode {
        identity: receipt.installed,
        validated,
        wx: receipt.wx,
        installation_registry_claimed: false,
    })
}

pub fn retire_installed(
    installed: InstalledCode,
    authority: RetirementAuthority,
    receipt: RetirementReceipt,
) -> Result<RetiredInstallation, Box<RetirementError>> {
    let evidence = InstalledCodeEvidence::from_installed(&installed);
    let mismatch = if authority.installed != evidence {
        Some("retirement authority is not scoped to this installed code")
    } else if receipt.installed != evidence {
        Some("retirement receipt does not match installed code")
    } else if !receipt.executors_quiesced {
        Some("retirement receipt does not establish executor quiescence")
    } else if !receipt.execute_disabled {
        Some("retirement receipt does not establish execute removal")
    } else if !receipt.write_authority_restored {
        Some("retirement receipt does not restore placement write authority")
    } else if !authority
        .required_facts
        .is_subset(&receipt.established_facts)
    {
        Some("retirement receipt lacks required completion facts")
    } else {
        None
    };
    if let Some(message) = mismatch {
        return Err(Box::new(RetirementError {
            installed,
            authority,
            receipt,
            diagnostic: InstallationDiagnostic(message.into()),
        }));
    }

    let ValidatedPlacement { frozen, .. } = installed.validated;
    Ok(RetiredInstallation {
        previous_artifact: frozen.artifact,
        placement: frozen.placement,
    })
}
