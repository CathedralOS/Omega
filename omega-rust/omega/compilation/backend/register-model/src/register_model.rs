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
#[cfg(feature = "test-support")]
pub mod test_support;

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

    /// The register view keyed `id`, when the model declares it.
    pub fn view(&self, id: RegisterViewId) -> Option<&RegisterView> {
        self.views.iter().find(|view| view.id == id)
    }

    /// The register unit keyed `id`, when the model declares it.
    pub fn unit(&self, id: RegisterUnitId) -> Option<&RegisterUnit> {
        self.units.iter().find(|unit| unit.id == id)
    }

    /// The register class keyed `id`, when the model declares it.
    pub fn class(&self, id: RegisterClassId) -> Option<&RegisterClass> {
        self.classes.iter().find(|class| class.id == id)
    }

    /// The preservation convention named `name`, when the model declares it.
    pub fn convention_named(&self, name: &str) -> Option<&PreservationConvention> {
        self.conventions
            .iter()
            .find(|convention| convention.name == name)
    }

    pub fn aliases(&self, left: RegisterViewId, right: RegisterViewId) -> bool {
        let Some(left) = self.view(left) else {
            return false;
        };
        let Some(right) = self.view(right) else {
            return false;
        };
        left.units.iter().any(|unit| right.units.contains(unit))
    }
}

/// Appends register classes, units and views in declaration order and seals
/// them into a [`PhysicalRegisterModel`]. The class roster, the layout and the
/// conventions and reservations stay with the architecture that declares them;
/// the builder only owns the id assignment every architecture repeats.
pub struct PhysicalModelBuilder {
    units: Vec<RegisterUnit>,
    views: Vec<RegisterView>,
    classes: Vec<RegisterClass>,
}

impl PhysicalModelBuilder {
    /// Start a model whose register classes are `class_names`, in id order.
    pub fn with_classes(class_names: &[&str]) -> Self {
        Self {
            units: Vec::new(),
            views: Vec::new(),
            classes: class_names
                .iter()
                .enumerate()
                .map(|(id, name)| RegisterClass {
                    id: RegisterClassId(u16::try_from(id).expect("class id fits u16")),
                    name: (*name).into(),
                    views: Vec::new(),
                })
                .collect(),
        }
    }

    /// Append one unit and return its declaration-order id.
    pub fn unit(&mut self, name: String, bits: u16, kind: RegisterUnitKind) -> RegisterUnitId {
        let id = RegisterUnitId(u16::try_from(self.units.len()).expect("unit id fits u16"));
        self.units.push(RegisterUnit {
            id,
            name,
            bits,
            kind,
        });
        id
    }

    /// Append one view and register it under its class.
    #[allow(clippy::too_many_arguments)]
    pub fn view(
        &mut self,
        name: String,
        class: RegisterClassId,
        units: Vec<RegisterUnitId>,
        write_units: Vec<RegisterUnitId>,
        bits: u16,
        write_semantics: RegisterWriteSemantics,
        allocatable: bool,
    ) -> RegisterViewId {
        let id = RegisterViewId(u16::try_from(self.views.len()).expect("view id fits u16"));
        self.views.push(RegisterView {
            id,
            name,
            class,
            units,
            write_units,
            bits,
            write_semantics,
            allocatable,
        });
        self.classes[usize::from(class.0)].views.push(id);
        id
    }

    /// Every unit declared so far, in declaration order.
    pub fn unit_ids(&self) -> Vec<RegisterUnitId> {
        self.units.iter().map(|unit| unit.id).collect()
    }

    /// Seal the table with the architecture's conventions and reservations.
    pub fn finish(
        self,
        architecture: Architecture,
        conventions: Vec<PreservationConvention>,
        reservations: Vec<RegisterReservationOverlay>,
    ) -> PhysicalRegisterModel {
        PhysicalRegisterModel {
            architecture,
            units: self.units,
            views: self.views,
            classes: self.classes,
            conventions,
            reservations,
        }
    }
}

/// The units `names` select from `units`, sorted.
pub fn units_for(
    units: &BTreeMap<String, Vec<RegisterUnitId>>,
    names: &[&str],
) -> Vec<RegisterUnitId> {
    crate::sorted_units(names.iter().flat_map(|name| units[*name].iter().copied()))
}

/// Everything in `all` that is neither fixed nor callee-saved.
pub fn complement(
    all: &[RegisterUnitId],
    fixed: &[RegisterUnitId],
    callee: &[RegisterUnitId],
) -> Vec<RegisterUnitId> {
    all.iter()
        .copied()
        .filter(|unit| !fixed.contains(unit) && !callee.contains(unit))
        .collect()
}

/// The view ids `names` select from `views`, in order.
pub fn view_ids(views: &BTreeMap<String, RegisterViewId>, names: &[&str]) -> Vec<RegisterViewId> {
    names.iter().map(|name| views[*name]).collect()
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

    /// Unwrap for tests that mutate the validated model and validate it again.
    #[cfg(test)]
    pub(crate) fn into_model(self) -> PhysicalRegisterModel {
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

invalid_display!(
    RegisterModelValidationError,
    "invalid physical register model"
);

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

    if model.units.iter().any(|unit| unit.bits == 0) {
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
        validate_unit_set(&view.units, &model)?;
        validate_unit_set(&view.write_units, &model)?;
        let storage_bits = view
            .units
            .iter()
            .map(|unit| u32::from(model.unit(*unit).expect("view unit validated above").bits))
            .sum::<u32>();
        if u32::from(view.bits) > storage_bits {
            return Err(RegisterModelValidationError::ViewWidthExceedsUnits(view.id));
        }
        if model.class(view.class).is_none() {
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
                if view.units.iter().any(|unit| {
                    model.unit(*unit).expect("view unit validated above").kind
                        != RegisterUnitKind::Zero
                }) =>
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
        validate_view_set(&class.views, &model)?;
        for view in &class.views {
            if model.view(*view).expect("class view validated above").class != class.id {
                return Err(RegisterModelValidationError::ClassMembershipMismatch);
            }
        }
    }
    for view in &model.views {
        if !model
            .class(view.class)
            .expect("view class validated above")
            .views
            .contains(&view.id)
        {
            return Err(RegisterModelValidationError::ClassMembershipMismatch);
        }
    }
    if let Some(unit) = model.units.iter().find(|unit| !covered.contains(&unit.id)) {
        return Err(RegisterModelValidationError::UnitNotCovered(unit.id));
    }

    for convention in &model.conventions {
        validate_convention(convention, &model)?;
    }
    for reservation in &model.reservations {
        if reservation.units.is_empty() {
            return Err(RegisterModelValidationError::EmptyReservation);
        }
        validate_unit_set(&reservation.units, &model)?;
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
    model: &PhysicalRegisterModel,
) -> Result<(), RegisterModelValidationError> {
    if set.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(RegisterModelValidationError::NonCanonicalUnitSet);
    }
    if let Some(unit) = set.iter().find(|unit| model.unit(**unit).is_none()) {
        return Err(RegisterModelValidationError::UnknownUnit(*unit));
    }
    Ok(())
}

fn validate_view_set(
    set: &[RegisterViewId],
    model: &PhysicalRegisterModel,
) -> Result<(), RegisterModelValidationError> {
    if set.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(RegisterModelValidationError::NonCanonicalViewSet);
    }
    if let Some(view) = set.iter().find(|view| model.view(**view).is_none()) {
        return Err(RegisterModelValidationError::UnknownView(*view));
    }
    Ok(())
}

fn validate_convention(
    convention: &PreservationConvention,
    model: &PhysicalRegisterModel,
) -> Result<(), RegisterModelValidationError> {
    if convention.argument_views.is_empty() || convention.result_views.is_empty() {
        return Err(RegisterModelValidationError::EmptyConvention);
    }
    if convention.stack_alignment == 0 || !convention.stack_alignment.is_power_of_two() {
        return Err(RegisterModelValidationError::InvalidStackAlignment);
    }
    validate_ordered_views(&convention.argument_views, model)?;
    validate_ordered_views(&convention.result_views, model)?;
    validate_unit_set(&convention.caller_saved, model)?;
    validate_unit_set(&convention.callee_saved, model)?;
    validate_unit_set(&convention.fixed, model)?;
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
    if let Some(unit) = model
        .units
        .iter()
        .map(|unit| unit.id)
        .find(|unit| !partition.contains(unit))
    {
        return Err(RegisterModelValidationError::ConventionPartitionOmission(
            unit,
        ));
    }
    Ok(())
}

fn validate_ordered_views(
    ordered: &[RegisterViewId],
    model: &PhysicalRegisterModel,
) -> Result<(), RegisterModelValidationError> {
    if ordered.iter().copied().collect::<BTreeSet<_>>().len() != ordered.len() {
        return Err(RegisterModelValidationError::NonCanonicalViewSet);
    }
    if let Some(view) = ordered.iter().find(|view| model.view(**view).is_none()) {
        return Err(RegisterModelValidationError::UnknownView(*view));
    }
    Ok(())
}
