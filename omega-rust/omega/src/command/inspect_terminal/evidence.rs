//! Verify inspection input before attempting an optional fixed-work certificate.

use proof_admission::AdmissionProfile;
use terminal_fixed_fuel::{
    FixedEntryFuelCertificate, FixedFuelError, derive_fixed_entry_fuel,
    derive_ranked_countdown_entry_fuel, validate_fixed_entry_fuel,
    validate_ranked_countdown_entry_fuel,
};
use terminal_psi::{ProofBundle, TerminalModule, TerminalRankedScc};
use terminal_verifier::{VerificationError, verify_module, verify_module_for_fixed_fuel};

#[derive(Debug)]
pub(super) enum FixedFuel {
    Available(FixedEntryFuelCertificate),
    Unavailable(FixedFuelError),
}

#[derive(Debug)]
pub(super) enum InspectionError {
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
    // The legacy countdown has a distinct verification entrance. Natural
    // certificates and unranked graphs use ordinary verification; failure
    // never retries through a different authority or drops proof evidence.
    if module.machines.iter().any(|machine| {
        matches!(
            machine.ranked_scc,
            Some(TerminalRankedScc::UnsignedCountdown(_))
        )
    }) {
        let verified = verify_module_for_fixed_fuel(module, proof, &profile)
            .map_err(InspectionError::Verification)?;
        retain_bound(
            derive_ranked_countdown_entry_fuel(&verified, module.entry),
            |certificate| validate_ranked_countdown_entry_fuel(&verified, certificate),
        )
    } else {
        let verified =
            verify_module(module, proof, &profile).map_err(InspectionError::Verification)?;
        retain_bound(
            derive_fixed_entry_fuel(&verified, module.entry),
            |certificate| validate_fixed_entry_fuel(&verified, certificate),
        )
    }
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
            | FixedFuelError::CallCycle(_)
            | FixedFuelError::BranchingNotYetSupported(_)
            | FixedFuelError::NoTerminalPath(_)
            | FixedFuelError::NotRankedCountdown(_)
            | FixedFuelError::BoundOverflow),
        ) => Ok(FixedFuel::Unavailable(error)),
        // Missing identities, malformed semantics, and inconsistent replay are
        // errors, not evidence that a valid program merely lacks a work bound.
        Err(error) => Err(InspectionError::FixedFuel(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{BlockId, MachineId};

    #[test]
    fn only_analysis_limits_become_unavailable_bounds() {
        let block = BlockId::new(1).unwrap();
        let machine = MachineId::new(1).unwrap();
        for reason in [
            FixedFuelError::ControlCycle(block),
            FixedFuelError::CallCycle(machine),
            FixedFuelError::BranchingNotYetSupported(block),
            FixedFuelError::NoTerminalPath(machine),
            FixedFuelError::NotRankedCountdown(machine),
            FixedFuelError::BoundOverflow,
        ] {
            let result = retain_bound(Err(reason.clone()), |_| panic!("no certificate to replay"));
            assert!(matches!(result, Ok(FixedFuel::Unavailable(actual)) if actual == reason));
        }
        for reason in [
            FixedFuelError::UnknownBlock(block),
            FixedFuelError::UnknownEntry(machine),
            FixedFuelError::CertificateMismatch,
        ] {
            let result = retain_bound(Err(reason.clone()), |_| panic!("no certificate to replay"));
            assert!(matches!(result, Err(InspectionError::FixedFuel(actual)) if actual == reason));
        }
    }
}
