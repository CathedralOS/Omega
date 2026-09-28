//! Optimizer module role: derivation leaf. Custody-derived reducible-region projection.

use super::{
    CountedLoopAnalysisError, LoopRegion, MachineId, OptimizerCycleComponent,
    OptimizerUnsignedCountdownRankingCertificate,
};

/// Project a certificate-bearing component into the reducible `LoopRegion`
/// the counted summary carries.
///
/// Member, internal-edge, entry, and exit identity are validated Terminal-SCC
/// custody facts pinned to this exact unit revision, so no edge or
/// reachability reconstruction happens here. What remains countdown-specific
/// is the certificate's binding to that custody: the certified header is a
/// member and the component's single entry edge lands on it — the
/// single-entry shape that makes the region reducible under that header in a
/// unit whose blocks are all reachable from the function entry.
pub(super) fn derive(
    machine: MachineId,
    component: &OptimizerCycleComponent,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
) -> Result<LoopRegion, CountedLoopAnalysisError> {
    let [entry] = component.entries.as_slice() else {
        return Err(shape(machine));
    };
    if component.members.is_empty()
        || !component.members.windows(2).all(|pair| pair[0] < pair[1])
        || !component.members.contains(&certificate.header)
        || entry.target != certificate.header
    {
        return Err(shape(machine));
    }
    Ok(LoopRegion {
        header: Some(certificate.header),
        blocks: component.members.clone(),
        irreducible: false,
    })
}

fn shape(machine: MachineId) -> CountedLoopAnalysisError {
    CountedLoopAnalysisError::UnsupportedCountdownShape { machine }
}
