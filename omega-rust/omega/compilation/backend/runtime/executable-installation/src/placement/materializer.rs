use crate::artifacts::AdmittedArtifact;
use crate::authority_digests::{FinalBytesDigest, PlacementPlanId};
use crate::placement::CodePlacementEvidence;
/// Inert provider-side result of resolving one admitted artifact at one exact
/// placement. The bytes are not executable authority; only the installation
/// ladder can consume the corresponding placement and establish execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MaterializedArtifactBytes {
    admission: AdmittedArtifact,
    placement: CodePlacementEvidence,
    placement_plan: PlacementPlanId,
    base_address: u64,
    bytes: Vec<u8>,
    final_bytes: FinalBytesDigest,
}

impl MaterializedArtifactBytes {

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

}

