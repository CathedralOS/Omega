/*
We define the declarative physical-register model (`PhysicalRegisterModel`),
instruction operand constraint catalogs (`RegisterConstraintCatalog`),
active register reservation profiles (`RegisterReservationProfile`),
ABI-preserved storage groupings (`PreservationStorageCatalog`), and their
structural validators and domain-separated SHA-256 identities.
*/

/*
We keep physical-register declarations and their structural validation separate
from register allocation and ISA instruction encoding so every architecture
catalog and downstream stage shares one verified vocabulary of storage units,
views, register classes, and preservation conventions without reaching into a
target-global registry.
*/

/*
This root module owns `PhysicalRegisterModel`, `PhysicalModelBuilder`,
`PreservationConvention`, and `validate_physical_register_model`. Beneath it,
`register_vocabulary.rs` defines units, views, and classes,
`reservation_profiles.rs` validates active reservation overlays,
`constraint_catalog.rs` validates per-instruction operand constraints and ties,
`preservation_storage.rs` groups callee-saved units into frame storage carriers,
and `identities.rs` computes canonical digests across all validated tables.
*/

#![forbid(unsafe_code)]

macro_rules! invalid_display {
    ($type:ty, $message:literal) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, concat!($message, ": {:?}"), self)
            }
        }
    };
}

pub(crate) use optimization_core::codec_bytes;
pub(crate) use optimization_core::digest_encoder;

pub(crate) mod constraint_catalog;
pub(crate) mod identities;
pub(crate) mod preservation_storage;
pub(crate) mod register_vocabulary;
pub(crate) mod reservation_profiles;
#[cfg(feature = "test-support")]
pub mod test_support;
#[cfg(test)]
mod tests;

pub use constraint_catalog::{
    PreparedRegisterConstraintCatalog, RegisterConstraintCatalog,
    RegisterConstraintCatalogValidationError, RegisterConstraintFamily, RegisterConstraintId,
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
    RegisterOperandConstraint, TargetRegisterEnvironmentConstraintKeys, call_constraint_keys,
    encode_constraint_key, find_register_constraint, find_register_constraint_mut,
    produce_register_constraint_catalog, validate_register_constraint_catalog, views_with_access,
};
pub use identities::{
    PhysicalRegisterModelIdentity, RegisterConstraintCatalogIdentity,
    RegisterReservationProfileIdentity, TargetRegisterEnvironmentIdentity,
    infrastructure_environment_agrees, target_register_environment_identity, write_semantics_tag,
};
pub use preservation_storage::{
    FrameAbiPreservationConvention, PreservationStorageCatalog, PreservationStorageCatalogIdentity,
    PreservationStorageCatalogValidationError, PreservationStorageGroup,
    PreservationStorageGroupId, ValidatedPreservationStorageCatalog,
    validate_preservation_storage_catalog,
};
pub use register_vocabulary::{
    RegisterClass, RegisterClassId, RegisterUnit, RegisterUnitId, RegisterUnitKind, RegisterView,
    RegisterViewId, RegisterWriteSemantics, Sha256RegisterDigest, sha256_length,
    sha256_register_units,
};
pub(crate) use reservation_profiles::RegisterReservationOverlay;
pub use reservation_profiles::{
    RegisterReservationProfile, RegisterReservationProfileValidationError, ReservationReason,
    ValidatedRegisterReservationProfile, reservation_overlay, sorted_units,
    validate_register_reservation_profile,
};

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

    pub fn view(&self, id: RegisterViewId) -> Option<&RegisterView> {
        self.views.iter().find(|view| view.id == id)
    }

    pub fn unit(&self, id: RegisterUnitId) -> Option<&RegisterUnit> {
        self.units.iter().find(|unit| unit.id == id)
    }

    pub fn class(&self, id: RegisterClassId) -> Option<&RegisterClass> {
        self.classes.iter().find(|class| class.id == id)
    }

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

pub struct PhysicalModelBuilder {
    units: Vec<RegisterUnit>,
    views: Vec<RegisterView>,
    classes: Vec<RegisterClass>,
}

impl PhysicalModelBuilder {
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

    pub fn unit_ids(&self) -> Vec<RegisterUnitId> {
        self.units.iter().map(|unit| unit.id).collect()
    }

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

pub fn units_for(
    units: &BTreeMap<String, Vec<RegisterUnitId>>,
    names: &[&str],
) -> Vec<RegisterUnitId> {
    crate::sorted_units(names.iter().flat_map(|name| units[*name].iter().copied()))
}

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
