//! Reservation profiles: which units an environment withholds from
//! allocation, why, and the validator that binds a profile to its model.

use crate::register_model::identities;
use crate::register_model::identities::{
    PhysicalRegisterModelIdentity, RegisterReservationProfileIdentity,
};
use crate::{RegisterUnitId, ValidatedPhysicalRegisterModel};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservationReason {
    Architectural,
    StackPointer,
    FramePointer,
    Platform,
    Dispatch,
    Metering,
    Syscall,
    InlineAssembly,
    Backend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterReservationOverlay {
    pub name: String,
    pub reason: ReservationReason,
    pub units: Vec<RegisterUnitId>,
}

/// Exact, named subset of the model's reservation-overlay catalog which is
/// active for one allocator environment.
///
/// Names are strictly sorted in validated form. Keeping selection distinct
/// from the overlay catalog prevents declarations such as frame-pointer or
/// metering reservations from silently becoming active policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterReservationProfile {
    pub name: String,
    pub active_overlays: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRegisterReservationProfile {
    profile: RegisterReservationProfile,
    target: target::NativeTarget,
    physical: PhysicalRegisterModelIdentity,
    reserved_units: Vec<RegisterUnitId>,
    identity: RegisterReservationProfileIdentity,
}

impl ValidatedRegisterReservationProfile {
    pub const fn profile(&self) -> &RegisterReservationProfile {
        &self.profile
    }

    pub const fn target(&self) -> target::NativeTarget {
        self.target
    }

    pub const fn physical_identity(&self) -> PhysicalRegisterModelIdentity {
        self.physical
    }

    pub fn reserved_units(&self) -> &[RegisterUnitId] {
        &self.reserved_units
    }

    pub const fn identity(&self) -> RegisterReservationProfileIdentity {
        self.identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterReservationProfileValidationError {
    EmptyName,
    TargetArchitectureMismatch,
    NonCanonicalOverlayNames,
    UnknownOverlay(String),
}

impl std::fmt::Display for RegisterReservationProfileValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid register reservation profile: {self:?}")
    }
}

impl std::error::Error for RegisterReservationProfileValidationError {}

/// Validate one explicit active reservation selection and derive its exact
/// reserved-unit union. An empty overlay list is valid and means precisely
/// that no optional overlay is active.
pub fn validate_register_reservation_profile(
    profile: RegisterReservationProfile,
    target: target::NativeTarget,
    model: &ValidatedPhysicalRegisterModel,
) -> Result<ValidatedRegisterReservationProfile, RegisterReservationProfileValidationError> {
    if profile.name.is_empty() {
        return Err(RegisterReservationProfileValidationError::EmptyName);
    }
    if target.architecture != model.model().architecture {
        return Err(RegisterReservationProfileValidationError::TargetArchitectureMismatch);
    }
    if profile
        .active_overlays
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(RegisterReservationProfileValidationError::NonCanonicalOverlayNames);
    }
    let overlays = model
        .model()
        .reservations
        .iter()
        .map(|overlay| (overlay.name.as_str(), overlay))
        .collect::<BTreeMap<_, _>>();
    let mut units = BTreeSet::new();
    for name in &profile.active_overlays {
        let Some(overlay) = overlays.get(name.as_str()) else {
            return Err(RegisterReservationProfileValidationError::UnknownOverlay(
                name.clone(),
            ));
        };
        units.extend(overlay.units.iter().copied());
    }
    let reserved_units = units.into_iter().collect::<Vec<_>>();
    let identity = identities::register_reservation_profile_identity(
        target,
        model.identity(),
        &profile,
        &reserved_units,
    );
    Ok(ValidatedRegisterReservationProfile {
        profile,
        target,
        physical: model.identity(),
        reserved_units,
        identity,
    })
}
