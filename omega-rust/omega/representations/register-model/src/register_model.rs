//! The register-model root: the physical register model — units, views,
//! classes and the preservation convention one architecture declares — and
//! the validator that seals it. Beneath it, `register_vocabulary` names
//! units, views and classes, `reservation_profiles` withholds units from
//! allocation, `constraint_catalog` constrains instruction operands,
//! `identities` fingerprints validated declarations and
//! `preservation_storage` describes preserved-register storage.

pub(crate) mod constraint_catalog;
pub(crate) mod identities;
pub(crate) mod preservation_storage;
pub(crate) mod register_vocabulary;
pub(crate) mod reservation_profiles;

use crate::{
    RegisterClass, RegisterClassId, RegisterReservationOverlay, RegisterUnit, RegisterUnitId,
    RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
};
use identities::PhysicalRegisterModelIdentity;
use std::collections::{BTreeMap, BTreeSet};
use target::Architecture;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreservationConvention {
    pub name: String,
    pub argument_views: Vec<RegisterViewId>,
    pub result_views: Vec<RegisterViewId>,
    pub caller_saved: Vec<RegisterUnitId>,
    pub callee_saved: Vec<RegisterUnitId>,
    pub fixed: Vec<RegisterUnitId>,
    pub stack_alignment: u16,
    pub red_zone_bytes: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalRegisterModel {
    pub architecture: Architecture,
    pub units: Vec<RegisterUnit>,
    pub views: Vec<RegisterView>,
    pub classes: Vec<RegisterClass>,
    pub conventions: Vec<PreservationConvention>,
    pub reservations: Vec<RegisterReservationOverlay>,
}

impl PhysicalRegisterModel {
    pub fn view_named(&self, name: &str) -> Option<&RegisterView> {
        self.views.iter().find(|view| view.name == name)
    }

    pub fn aliases(&self, left: RegisterViewId, right: RegisterViewId) -> bool {
        let Some(left) = self.views.iter().find(|view| view.id == left) else {
            return false;
        };
        let Some(right) = self.views.iter().find(|view| view.id == right) else {
            return false;
        };
        left.units.iter().any(|unit| right.units.contains(unit))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPhysicalRegisterModel {
    model: PhysicalRegisterModel,
    identity: PhysicalRegisterModelIdentity,
}

impl ValidatedPhysicalRegisterModel {
    pub const fn model(&self) -> &PhysicalRegisterModel {
        &self.model
    }

    pub const fn identity(&self) -> PhysicalRegisterModelIdentity {
        self.identity
    }

    pub fn into_model(self) -> PhysicalRegisterModel {
        self.model
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterModelValidationError {
    NonCanonicalUnitIds,
    NonCanonicalViewIds,
    NonCanonicalClassIds,
    DuplicateName,
    ZeroWidthUnit,
    ZeroWidthView,
    ViewWidthExceedsUnits(RegisterViewId),
    EmptyViewUnits,
    UnknownUnit(RegisterUnitId),
    UnknownView(RegisterViewId),
    UnknownClass(RegisterClassId),
    NonCanonicalUnitSet,
    NonCanonicalViewSet,
    ClassMembershipMismatch,
    UnitNotCovered(RegisterUnitId),
    WriteFootprintMismatch(RegisterViewId),
    EmptyConvention,
    InvalidStackAlignment,
    ConventionPartitionOverlap(RegisterUnitId),
    ConventionPartitionOmission(RegisterUnitId),
    EmptyReservation,
}

impl std::fmt::Display for RegisterModelValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid physical register model: {self:?}")
    }
}

impl std::error::Error for RegisterModelValidationError {}

pub fn validate_physical_register_model(
    model: PhysicalRegisterModel,
) -> Result<ValidatedPhysicalRegisterModel, RegisterModelValidationError> {
    validate_sequential_ids(
        model.units.iter().map(|unit| unit.id.0),
        RegisterModelValidationError::NonCanonicalUnitIds,
    )?;
    validate_sequential_ids(
        model.views.iter().map(|view| view.id.0),
        RegisterModelValidationError::NonCanonicalViewIds,
    )?;
    validate_sequential_ids(
        model.classes.iter().map(|class| class.id.0),
        RegisterModelValidationError::NonCanonicalClassIds,
    )?;
    if !unique_names(model.units.iter().map(|row| row.name.as_str()))
        || !unique_names(model.views.iter().map(|row| row.name.as_str()))
        || !unique_names(model.classes.iter().map(|row| row.name.as_str()))
        || !unique_names(model.conventions.iter().map(|row| row.name.as_str()))
        || !unique_names(model.reservations.iter().map(|row| row.name.as_str()))
    {
        return Err(RegisterModelValidationError::DuplicateName);
    }

    let units = model
        .units
        .iter()
        .map(|unit| (unit.id, unit))
        .collect::<BTreeMap<_, _>>();
    let views = model
        .views
        .iter()
        .map(|view| (view.id, view))
        .collect::<BTreeMap<_, _>>();
    let classes = model
        .classes
        .iter()
        .map(|class| (class.id, class))
        .collect::<BTreeMap<_, _>>();
    if let Some(unit) = model.units.iter().find(|unit| unit.bits == 0) {
        let _ = unit;
        return Err(RegisterModelValidationError::ZeroWidthUnit);
    }
    let mut covered = BTreeSet::new();
    for view in &model.views {
        if view.bits == 0 {
            return Err(RegisterModelValidationError::ZeroWidthView);
        }
        if view.units.is_empty() {
            return Err(RegisterModelValidationError::EmptyViewUnits);
        }
        validate_unit_set(&view.units, &units)?;
        validate_unit_set(&view.write_units, &units)?;
        let storage_bits = view
            .units
            .iter()
            .map(|unit| u32::from(units[unit].bits))
            .sum::<u32>();
        if u32::from(view.bits) > storage_bits {
            return Err(RegisterModelValidationError::ViewWidthExceedsUnits(view.id));
        }
        if !classes.contains_key(&view.class) {
            return Err(RegisterModelValidationError::UnknownClass(view.class));
        }
        if !view
            .units
            .iter()
            .all(|unit| view.write_units.contains(unit))
        {
            return Err(RegisterModelValidationError::WriteFootprintMismatch(
                view.id,
            ));
        }
        match view.write_semantics {
            RegisterWriteSemantics::ExactView if view.units != view.write_units => {
                return Err(RegisterModelValidationError::WriteFootprintMismatch(
                    view.id,
                ));
            }
            RegisterWriteSemantics::ZeroExtendsParent if view.units == view.write_units => {
                return Err(RegisterModelValidationError::WriteFootprintMismatch(
                    view.id,
                ));
            }
            RegisterWriteSemantics::Discards
                if view
                    .units
                    .iter()
                    .any(|unit| units[unit].kind != RegisterUnitKind::Zero) =>
            {
                return Err(RegisterModelValidationError::WriteFootprintMismatch(
                    view.id,
                ));
            }
            _ => {}
        }
        covered.extend(view.units.iter().copied());
        covered.extend(view.write_units.iter().copied());
    }
    for class in &model.classes {
        validate_view_set(&class.views, &views)?;
        for view in &class.views {
            if views[view].class != class.id {
                return Err(RegisterModelValidationError::ClassMembershipMismatch);
            }
        }
    }
    for view in &model.views {
        if !classes[&view.class].views.contains(&view.id) {
            return Err(RegisterModelValidationError::ClassMembershipMismatch);
        }
    }
    if let Some(unit) = model.units.iter().find(|unit| !covered.contains(&unit.id)) {
        return Err(RegisterModelValidationError::UnitNotCovered(unit.id));
    }

    for convention in &model.conventions {
        validate_convention(convention, &units, &views)?;
    }
    for reservation in &model.reservations {
        if reservation.units.is_empty() {
            return Err(RegisterModelValidationError::EmptyReservation);
        }
        validate_unit_set(&reservation.units, &units)?;
    }
    let identity = identities::physical_register_model_identity(&model);
    Ok(ValidatedPhysicalRegisterModel { model, identity })
}

fn validate_sequential_ids(
    ids: impl Iterator<Item = u16>,
    error: RegisterModelValidationError,
) -> Result<(), RegisterModelValidationError> {
    for (expected, actual) in ids.enumerate() {
        if usize::from(actual) != expected {
            return Err(error);
        }
    }
    Ok(())
}

fn unique_names<'a>(names: impl Iterator<Item = &'a str> + Clone) -> bool {
    names.clone().collect::<BTreeSet<_>>().len() == names.count()
}

fn validate_unit_set(
    set: &[RegisterUnitId],
    known: &BTreeMap<RegisterUnitId, &RegisterUnit>,
) -> Result<(), RegisterModelValidationError> {
    if set.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(RegisterModelValidationError::NonCanonicalUnitSet);
    }
    if let Some(unit) = set.iter().find(|unit| !known.contains_key(unit)) {
        return Err(RegisterModelValidationError::UnknownUnit(*unit));
    }
    Ok(())
}

fn validate_view_set(
    set: &[RegisterViewId],
    known: &BTreeMap<RegisterViewId, &RegisterView>,
) -> Result<(), RegisterModelValidationError> {
    if set.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(RegisterModelValidationError::NonCanonicalViewSet);
    }
    if let Some(view) = set.iter().find(|view| !known.contains_key(view)) {
        return Err(RegisterModelValidationError::UnknownView(*view));
    }
    Ok(())
}

fn validate_convention(
    convention: &PreservationConvention,
    units: &BTreeMap<RegisterUnitId, &RegisterUnit>,
    views: &BTreeMap<RegisterViewId, &RegisterView>,
) -> Result<(), RegisterModelValidationError> {
    if convention.argument_views.is_empty() || convention.result_views.is_empty() {
        return Err(RegisterModelValidationError::EmptyConvention);
    }
    if convention.stack_alignment == 0 || !convention.stack_alignment.is_power_of_two() {
        return Err(RegisterModelValidationError::InvalidStackAlignment);
    }
    validate_ordered_views(&convention.argument_views, views)?;
    validate_ordered_views(&convention.result_views, views)?;
    validate_unit_set(&convention.caller_saved, units)?;
    validate_unit_set(&convention.callee_saved, units)?;
    validate_unit_set(&convention.fixed, units)?;
    let mut partition = BTreeSet::new();
    for unit in convention
        .caller_saved
        .iter()
        .chain(&convention.callee_saved)
        .chain(&convention.fixed)
    {
        if !partition.insert(*unit) {
            return Err(RegisterModelValidationError::ConventionPartitionOverlap(
                *unit,
            ));
        }
    }
    if let Some(unit) = units.keys().find(|unit| !partition.contains(unit)) {
        return Err(RegisterModelValidationError::ConventionPartitionOmission(
            *unit,
        ));
    }
    Ok(())
}

fn validate_ordered_views(
    ordered: &[RegisterViewId],
    known: &BTreeMap<RegisterViewId, &RegisterView>,
) -> Result<(), RegisterModelValidationError> {
    if ordered.iter().copied().collect::<BTreeSet<_>>().len() != ordered.len() {
        return Err(RegisterModelValidationError::NonCanonicalViewSet);
    }
    if let Some(view) = ordered.iter().find(|view| !known.contains_key(view)) {
        return Err(RegisterModelValidationError::UnknownView(*view));
    }
    Ok(())
}
