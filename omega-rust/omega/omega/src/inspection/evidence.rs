//! Verify inspection input before attempting an optional fixed-work certificate.

use proof_admission::AdmissionProfile;
use terminal_fixed_fuel::{
    FixedEntryFuelCertificate, FixedFuelError, derive_fixed_entry_fuel, validate_fixed_entry_fuel,
};
use terminal_psi::{ProofBundle, TerminalModule};
use terminal_verifier::{VerificationError, verify_module};

#[derive(Debug)]
pub enum FixedFuel {
    Available(FixedEntryFuelCertificate),
    Unavailable(FixedFuelError),
}

#[derive(Debug)]
pub enum InspectionError {
    Verification(VerificationError),
    FixedFuel(FixedFuelError),
}

impl std::fmt::Display for InspectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Verification(error) => write!(formatter, "verification failed: {error}"),
            Self::FixedFuel(error) => write!(formatter, "fixed-fuel checking failed: {error}"),
        }
    }
}

pub(super) fn inspect(
    module: &TerminalModule,
    proof: &ProofBundle,
) -> Result<FixedFuel, InspectionError> {
    let profile = AdmissionProfile::default();
    // Natural certificates and unranked graphs share the ordinary verification
    // entrance; failure never retries through a different authority or drops
    // proof evidence.
    let verified = verify_module(module, proof, &profile).map_err(InspectionError::Verification)?;
    retain_bound(
        derive_fixed_entry_fuel(&verified, module.entry),
        |certificate| validate_fixed_entry_fuel(&verified, certificate),
    )
}

fn retain_bound(
    derived: Result<FixedEntryFuelCertificate, FixedFuelError>,
    replay: impl FnOnce(&FixedEntryFuelCertificate) -> Result<(), FixedFuelError>,
) -> Result<FixedFuel, InspectionError> {
    match derived {
        Ok(certificate) => {
            replay(&certificate).map_err(InspectionError::FixedFuel)?;
            Ok(FixedFuel::Available(certificate))
        }
        Err(
            error @ (FixedFuelError::ControlCycle(_)
            | FixedFuelError::UnboundedCycleComponent { .. }
            | FixedFuelError::CallCycle(_)
            | FixedFuelError::BranchingNotYetSupported(_)
            | FixedFuelError::NoTerminalPath(_)
            | FixedFuelError::BoundOverflow),
        ) => Ok(FixedFuel::Unavailable(error)),
        // Missing identities, malformed semantics, and inconsistent replay are
        // errors, not evidence that a valid program merely lacks a work bound.
        Err(error) => Err(InspectionError::FixedFuel(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::{FixedFuel, FixedFuelError, InspectionError, retain_bound};
    use semantic_vocabulary::{BlockId, MachineId};

    #[test]
    fn only_analysis_limits_become_unavailable_bounds() {
        let block = BlockId::new(1).unwrap();
        let machine = MachineId::new(1).unwrap();
        for reason in [
            FixedFuelError::ControlCycle(block),
            FixedFuelError::UnboundedCycleComponent {
                component: semantic_vocabulary::CycleComponentId::new(1).unwrap(),
                cause: terminal_fixed_fuel::UnboundedCycleCause::Unranked,
            },
            FixedFuelError::CallCycle(machine),
            FixedFuelError::BranchingNotYetSupported(block),
            FixedFuelError::NoTerminalPath(machine),
            FixedFuelError::BoundOverflow,
        ] {
            let result = retain_bound(Err(reason.clone()), |_| panic!("no certificate to replay"));
            assert!(matches!(result, Ok(FixedFuel::Unavailable(actual)) if actual == reason));
        }
        for reason in [
            FixedFuelError::UnknownBlock(block),
            FixedFuelError::UnknownEntry(machine),
            FixedFuelError::InvalidRankedScc(machine),
            FixedFuelError::CertificateMismatch,
        ] {
            let result = retain_bound(Err(reason.clone()), |_| panic!("no certificate to replay"));
            assert!(matches!(result, Err(InspectionError::FixedFuel(actual)) if actual == reason));
        }
    }
}
