use crate::artifacts::AdmittedArtifact;
use crate::authority_digests::{FinalBytesDigest, PlacementPlanId};
use crate::installation::InstallationDiagnostic;
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

macro_rules! write_le {
    ($name:ident, $t:ty) => {
        fn $name(bytes: &mut [u8], offset: usize, value: $t) -> Result<(), InstallationDiagnostic> {
            bytes[offset..offset + std::mem::size_of::<$t>()].copy_from_slice(&value.to_le_bytes());
            Ok(())
        }
    };
}
