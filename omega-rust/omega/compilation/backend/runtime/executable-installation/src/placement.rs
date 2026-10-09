//! Code placement claims, materialization receipts, frozen placements and
//! final validation certificates.
//!
//! `materializer.rs` resolves one admitted artifact at one claimed placement
//! into the inert `MaterializedArtifactBytes` the freeze transition binds.

pub(crate) mod materializer;

use crate::artifacts::{AdmittedArtifact, Artifact, InstallationAudience};
use crate::authority_digests::{
    AdmissionReceiptId, CodePlacementId, FinalValidationId, InstallationScopeId,
    MachineFootprintId,
};
use crate::placement::materializer::MaterializedArtifactBytes;
use extents::{AddressSpaceId, Extent, ExtentProvenanceId, ExtentRights};
use layout_plans::PlacementConstraints;

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

/// Linear W+NX placement state. It carries no executable permission.
#[derive(Debug)]
pub(crate) struct CodePlacement {
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

/// Linear R+NX placement whose exact bytes can no longer change.
#[derive(Debug)]
pub(crate) struct FrozenPlacement {
    pub(crate) artifact: AdmittedArtifact,
    pub(crate) placement: CodePlacement,
    pub(crate) materialized: MaterializedArtifactBytes,
    pub(crate) realized_footprint: MachineFootprintId,
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
