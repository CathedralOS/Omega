//! Code placement claims, materialization receipts, frozen placements and
//! final validation certificates.
//!
//! `materializer.rs` resolves one admitted artifact at one claimed placement
//! into the inert `MaterializedArtifactBytes` the freeze transition binds.

pub(crate) mod materializer;

use crate::artifacts::{AdmittedArtifact, Artifact, InstallationAudience};
use crate::authority_digests::{
    AdmissionReceiptId, CodePlacementId, FinalBytesDigest, FinalValidationId, InstallationScopeId,
    MachineFootprintId,
};
use crate::installation::InstallationDiagnostic;
use crate::placement::materializer::MaterializedArtifactBytes;
use extents::{AddressSpaceId, Extent, ExtentProvenanceId, ExtentRights};
use layout_plans::{PlacementConstraints, PlacementSite};

/// One-shot authority over an exact W+NX destination.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CodePlacementExtentEvidence {
    base: u64,
    length: u64,
    address_space: AddressSpaceId,
    rights: ExtentRights,
    provenance: ExtentProvenanceId,
    era: extents::MappingEraId,
    lineage: extents::ExtentLineageId,
}

impl CodePlacementExtentEvidence {
    fn from_extent(extent: &Extent) -> Self {
        Self {
            base: extent.base(),
            length: extent.length(),
            address_space: extent.address_space(),
            rights: extent.rights().clone(),
            provenance: extent.provenance(),
            era: extent.era(),
            lineage: extent.lineage_root(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct CodePlacementAuthority {
    placement: CodePlacementId,
    scope: InstallationScopeId,
    audience: InstallationAudience,
    extent: CodePlacementExtentEvidence,
    required_rights: ExtentRights,
    constraints: PlacementConstraints,
    site: PlacementSite,
}

impl CodePlacementAuthority {
    #[allow(clippy::too_many_arguments)]
    pub fn from_admitted_provider(
        placement: CodePlacementId,
        scope: InstallationScopeId,
        audience: InstallationAudience,
        extent: &Extent,
        required_rights: ExtentRights,
        constraints: PlacementConstraints,
        site: PlacementSite,
    ) -> Self {
        Self {
            placement,
            scope,
            audience,
            extent: CodePlacementExtentEvidence::from_extent(extent),
            required_rights,
            constraints,
            site,
        }
    }

    pub fn claim(self, extent: Extent) -> Result<CodePlacement, Box<PlacementClaimError>> {
        let mismatch = if CodePlacementExtentEvidence::from_extent(&extent) != self.extent {
            Some(
                "extent does not match the exact range, space, rights, provenance, era, and lineage bound by code-placement authority"
                    .into(),
            )
        } else if !extent.rights().contains(&self.required_rights) {
            Some("extent lacks rights required by code-placement authority".into())
        } else if self.site.base_address != extent.base() {
            Some("placement site base does not match destination Extent".into())
        } else if self
            .site
            .installation_scope
            .is_none_or(|scope| scope.normalized_identity() != self.scope.normalized_identity())
        {
            Some("placement site does not carry the exact installation scope".into())
        } else {
            match usize::try_from(extent.length()) {
                Ok(length) => self
                    .constraints
                    .validate_site(length, self.site)
                    .err()
                    .map(|diagnostic| diagnostic.0),
                Err(_) => {
                    Some("placement length cannot be represented by the host validator".into())
                }
            }
        };
        if let Some(message) = mismatch {
            return Err(Box::new(PlacementClaimError {
                authority: self,
                extent,
                diagnostic: InstallationDiagnostic(message),
            }));
        }
        Ok(CodePlacement {
            placement: self.placement,
            scope: self.scope,
            audience: self.audience,
            constraints: self.constraints,
            extent,
        })
    }
}

#[derive(Debug)]
pub struct PlacementClaimError {
    authority: CodePlacementAuthority,
    extent: Extent,
    diagnostic: InstallationDiagnostic,
}

impl PlacementClaimError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (CodePlacementAuthority, Extent) {
        (self.authority, self.extent)
    }
}

/// Linear W+NX placement state. It carries no executable permission.
#[derive(Debug)]
pub struct CodePlacement {
    pub(crate) placement: CodePlacementId,
    pub(crate) scope: InstallationScopeId,
    audience: InstallationAudience,
    pub(crate) constraints: PlacementConstraints,
    pub(crate) extent: Extent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodePlacementEvidence {
    pub(crate) placement: CodePlacementId,
    scope: InstallationScopeId,
    audience: InstallationAudience,
    constraints: PlacementConstraints,
    extent: CodePlacementExtentEvidence,
}

impl CodePlacementEvidence {
    pub(crate) fn from_placement(placement: &CodePlacement) -> Self {
        Self {
            placement: placement.placement,
            scope: placement.scope,
            audience: placement.audience,
            constraints: placement.constraints,
            extent: CodePlacementExtentEvidence::from_extent(&placement.extent),
        }
    }
}

impl CodePlacement {
    pub const fn identity(&self) -> CodePlacementId {
        self.placement
    }

    pub const fn base(&self) -> u64 {
        self.extent.base()
    }

    pub const fn length(&self) -> u64 {
        self.extent.length()
    }
}

/// Provider result after declared sections/relocations were written and all
/// write authority over the final bytes was frozen.
#[derive(Debug, PartialEq, Eq)]
pub struct MaterializationReceipt {
    pub(crate) materialized: MaterializedArtifactBytes,
    pub(crate) realized_footprint: MachineFootprintId,
    pub(crate) writes_frozen: bool,
}

impl MaterializationReceipt {
    /// Bind a provider's write/freeze receipt to the exact output of the
    /// canonical materializer instead of restating its artifact/placement
    /// identities independently.
    pub fn from_materialized(
        materialized: &MaterializedArtifactBytes,
        realized_footprint: MachineFootprintId,
        writes_frozen: bool,
    ) -> Self {
        Self {
            materialized: materialized.clone(),
            realized_footprint,
            writes_frozen,
        }
    }
}

#[derive(Debug)]
pub struct MaterializationError {
    pub(crate) placement: CodePlacement,
    pub(crate) materialized: MaterializedArtifactBytes,
    pub(crate) receipt: MaterializationReceipt,
    pub(crate) diagnostic: InstallationDiagnostic,
}

impl MaterializationError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        CodePlacement,
        MaterializedArtifactBytes,
        MaterializationReceipt,
    ) {
        (self.placement, self.materialized, self.receipt)
    }
}

/// Linear R+NX placement whose exact bytes can no longer change.
#[derive(Debug)]
pub struct FrozenPlacement {
    pub(crate) artifact: AdmittedArtifact,
    pub(crate) placement: CodePlacement,
    pub(crate) materialized: MaterializedArtifactBytes,
    pub(crate) realized_footprint: MachineFootprintId,
}

impl FrozenPlacement {
    /// Exact immutable byte snapshot whose write authority the provider froze.
    /// Final footprint/PCC validators inspect this provider-side view; it is
    /// not an Omega source-visible byte-to-code escape hatch.
    pub fn bytes(&self) -> &[u8] {
        self.materialized.bytes()
    }

    pub const fn final_bytes(&self) -> FinalBytesDigest {
        self.materialized.final_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalValidationCertificate {
    pub(crate) identity: FinalValidationId,
    pub(crate) artifact: Artifact,
    pub(crate) admission: AdmissionReceiptId,
    pub(crate) placement: CodePlacementEvidence,
    pub(crate) final_bytes_digest: FinalBytesDigest,
    pub(crate) final_bytes: Vec<u8>,
    pub(crate) realized_footprint: MachineFootprintId,
    pub(crate) accepted: bool,
}

impl FinalValidationCertificate {
    /// Construct validation evidence from the exact frozen carrier inspected
    /// by the validator. Compact normalized IDs remain useful report keys but
    /// are not accepted as collision-resistant content evidence.
    pub fn from_validator(
        identity: FinalValidationId,
        frozen: &FrozenPlacement,
        accepted: bool,
    ) -> Self {
        Self {
            identity,
            artifact: frozen.artifact.artifact.clone(),
            admission: frozen.artifact.admission,
            placement: CodePlacementEvidence::from_placement(&frozen.placement),
            final_bytes_digest: frozen.materialized.final_bytes(),
            final_bytes: frozen.materialized.bytes().to_vec(),
            realized_footprint: frozen.realized_footprint,
            accepted,
        }
    }
}

#[derive(Debug)]
pub struct FrozenPlacementError {
    pub(crate) frozen: FrozenPlacement,
    pub(crate) diagnostic: InstallationDiagnostic,
}

impl FrozenPlacementError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_frozen(self) -> FrozenPlacement {
        self.frozen
    }
}

/// Linear R+NX placement bound to checked final bytes and footprint.
#[derive(Debug)]
pub struct ValidatedPlacement {
    pub(crate) frozen: FrozenPlacement,
    pub(crate) validation: FinalValidationId,
}

/// Exact provider-side evidence for one validated placement. Normalized IDs
/// remain report keys; authorization compares the canonical artifact, frozen
/// bytes, placement geometry, authority lineage, and validation result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedPlacementEvidence {
    pub(crate) admission_evidence: AdmittedArtifact,
    artifact: Artifact,
    admission: AdmissionReceiptId,
    pub(crate) placement: CodePlacementId,
    pub(crate) scope: InstallationScopeId,
    pub(crate) audience: InstallationAudience,
    pub(crate) constraints: PlacementConstraints,
    pub(crate) base: u64,
    pub(crate) length: u64,
    pub(crate) address_space: AddressSpaceId,
    pub(crate) rights: ExtentRights,
    pub(crate) provenance: ExtentProvenanceId,
    pub(crate) era: extents::MappingEraId,
    pub(crate) lineage: extents::ExtentLineageId,
    pub(crate) final_bytes: Vec<u8>,
    pub(crate) realized_footprint: MachineFootprintId,
    pub(crate) validation: FinalValidationId,
}

impl ValidatedPlacementEvidence {
    pub(crate) fn from_validated(validated: &ValidatedPlacement) -> Self {
        let frozen = &validated.frozen;
        let extent = &frozen.placement.extent;
        Self {
            admission_evidence: frozen.artifact.clone(),
            artifact: frozen.artifact.artifact.clone(),
            admission: frozen.artifact.admission,
            placement: frozen.placement.placement,
            scope: frozen.placement.scope,
            audience: frozen.placement.audience,
            constraints: frozen.placement.constraints,
            base: extent.base(),
            length: extent.length(),
            address_space: extent.address_space(),
            rights: extent.rights().clone(),
            provenance: extent.provenance(),
            era: extent.era(),
            lineage: extent.lineage_root(),
            final_bytes: frozen.materialized.bytes().to_vec(),
            realized_footprint: frozen.realized_footprint,
            validation: validated.validation,
        }
    }
}
