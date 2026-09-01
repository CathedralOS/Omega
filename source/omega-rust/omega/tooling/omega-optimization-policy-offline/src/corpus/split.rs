use omega_optimization_core::OptimizationUnitIdentity;
use sha2::{Digest, Sha256};

use super::model::OfflinePolicySplit;

pub fn split_for_source(source: OptimizationUnitIdentity) -> OfflinePolicySplit {
    let mut digest = Sha256::new();
    digest.update(b"omega.offline-policy-source-split.sha256.v1\0");
    digest.update(source.bytes());
    let bytes: [u8; 32] = digest.finalize().into();
    match u64::from_le_bytes(
        bytes[..8]
            .try_into()
            .expect("digest prefix has eight bytes"),
    ) % 10
    {
        0..=6 => OfflinePolicySplit::Training,
        7..=8 => OfflinePolicySplit::Evaluation,
        _ => OfflinePolicySplit::Regression,
    }
}
