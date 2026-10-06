//! Validate inspection input before attempting an optional fixed-work certificate.

use terminal_fixed_fuel::{
    FixedEntryFuelCertificate, FixedFuelError, derive_fixed_entry_fuel, validate_fixed_entry_fuel,
};
use terminal_psi::TerminalModule;

#[derive(Debug)]
pub enum FixedFuel {
    Available(FixedEntryFuelCertificate),
    Unavailable(FixedFuelError),
}

#[derive(Debug)]
pub enum InspectionError {
    FixedFuel(FixedFuelError),
}

impl std::fmt::Display for InspectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FixedFuel(error) => write!(formatter, "fixed-fuel checking failed: {error}"),
        }
    }
}

pub(super) fn inspect(module: &TerminalModule) -> Result<FixedFuel, InspectionError> {
    retain_bound(
        derive_fixed_entry_fuel(module, module.entry),
        |certificate| validate_fixed_entry_fuel(module, certificate),
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
