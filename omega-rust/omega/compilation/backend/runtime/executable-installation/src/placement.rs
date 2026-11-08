/*
We define the linear placement states (`CodePlacement`, `FrozenPlacement`,
`ValidatedPlacement`), materialized artifact bytes (`MaterializedArtifactBytes`),
and provider-side placement evidence (`ValidatedPlacementEvidence`) that carry
an admitted artifact from a writable W+NX destination to a frozen R+NX
placement ready for installation.
*/

use crate::artifacts::{AdmittedArtifact, Artifact, InstallationAudience};
use crate::authority_digests::{
    AccessPlanId, AdmissionReceiptId, CodePlacementId, FinalBytesDigest, FinalValidationId,
    InstallationScopeId, MachineFootprintId,
};
use extents::{AddressSpaceId, Extent, ExtentProvenanceId, ExtentRights};
use layout_plans::PlacementConstraints;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MaterializedArtifactBytes {
    admission: AdmittedArtifact,
    placement: CodePlacementEvidence,
    access_plan: AccessPlanId,
    base_address: u64,
    bytes: Vec<u8>,
    final_bytes: FinalBytesDigest,
}

impl MaterializedArtifactBytes {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

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

#[derive(Debug)]
pub(crate) struct CodePlacement {
    pub(crate) placement: CodePlacementId,
    pub(crate) scope: InstallationScopeId,
    audience: InstallationAudience,
    pub(crate) constraints: PlacementConstraints,
    pub(crate) extent: Extent, // Linear W+NX placement extent carrying no executable permission.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodePlacementEvidence {
    pub(crate) placement: CodePlacementId,
    scope: InstallationScopeId,
    audience: InstallationAudience,
    constraints: PlacementConstraints,
    extent: CodePlacementExtentEvidence,
}

#[derive(Debug)]
pub(crate) struct FrozenPlacement {
    pub(crate) artifact: AdmittedArtifact,
    pub(crate) placement: CodePlacement,
    pub(crate) materialized: MaterializedArtifactBytes,
    pub(crate) realized_footprint: MachineFootprintId,
}

#[derive(Debug)]
pub struct ValidatedPlacement {
    pub(crate) frozen: FrozenPlacement,
    pub(crate) validation: FinalValidationId,
}

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
