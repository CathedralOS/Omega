#![forbid(unsafe_code)]

//! Declarative physical-register facts and their independent structural
//! validator. This crate deliberately performs no allocation yet: ISA owners
//! construct models, while future allocators consume validated values passed
//! in by orchestration rather than reaching into target globals.

use std::collections::{BTreeMap, BTreeSet};

use omega_target::Architecture;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterUnitId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterViewId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterClassId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegisterUnitKind {
    IntegerLane,
    VectorLane,
    Flags,
    StackPointer,
    InstructionPointer,
    Zero,
    FloatingControl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterUnit {
    pub id: RegisterUnitId,
    pub name: String,
    pub bits: u16,
    pub kind: RegisterUnitKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterWriteSemantics {
    ExactView,
    PreservesUnwritten,
    ZeroExtendsParent,
    ZeroExtendsWithinUnit,
    Discards,
    InstructionDefined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterView {
    pub id: RegisterViewId,
    pub name: String,
    pub class: RegisterClassId,
    /// Storage units occupied by a live value in this view.
    pub units: Vec<RegisterUnitId>,
    /// Storage units modified by the view's canonical write behavior.
    pub write_units: Vec<RegisterUnitId>,
    pub bits: u16,
    pub write_semantics: RegisterWriteSemantics,
    pub allocatable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterClass {
    pub id: RegisterClassId,
    pub name: String,
    pub views: Vec<RegisterViewId>,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixedRegisterOperand {
    pub operand: u16,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterConstraint {
    pub name: String,
    pub fixed_inputs: Vec<FixedRegisterOperand>,
    pub fixed_outputs: Vec<FixedRegisterOperand>,
    pub early_clobbers: Vec<RegisterUnitId>,
    pub clobbers: Vec<RegisterUnitId>,
}

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
    pub constraints: Vec<RegisterConstraint>,
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
pub struct ValidatedPhysicalRegisterModel(PhysicalRegisterModel);

impl ValidatedPhysicalRegisterModel {
    pub const fn model(&self) -> &PhysicalRegisterModel {
        &self.0
    }

    pub fn into_model(self) -> PhysicalRegisterModel {
        self.0
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
    EmptyConstraint,
    NonCanonicalOperandSet,
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
        || !unique_names(model.constraints.iter().map(|row| row.name.as_str()))
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
    for constraint in &model.constraints {
        if constraint.fixed_inputs.is_empty()
            && constraint.fixed_outputs.is_empty()
            && constraint.early_clobbers.is_empty()
            && constraint.clobbers.is_empty()
        {
            return Err(RegisterModelValidationError::EmptyConstraint);
        }
        validate_operands(&constraint.fixed_inputs, &views)?;
        validate_operands(&constraint.fixed_outputs, &views)?;
        validate_unit_set(&constraint.early_clobbers, &units)?;
        validate_unit_set(&constraint.clobbers, &units)?;
    }
    Ok(ValidatedPhysicalRegisterModel(model))
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

fn validate_operands(
    operands: &[FixedRegisterOperand],
    views: &BTreeMap<RegisterViewId, &RegisterView>,
) -> Result<(), RegisterModelValidationError> {
    if operands.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(RegisterModelValidationError::NonCanonicalOperandSet);
    }
    if let Some(operand) = operands
        .iter()
        .find(|operand| !views.contains_key(&operand.view))
    {
        return Err(RegisterModelValidationError::UnknownView(operand.view));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn miniature_model() -> PhysicalRegisterModel {
        PhysicalRegisterModel {
            architecture: Architecture::X86_64,
            units: vec![RegisterUnit {
                id: RegisterUnitId(0),
                name: "r0.storage".into(),
                bits: 64,
                kind: RegisterUnitKind::IntegerLane,
            }],
            views: vec![RegisterView {
                id: RegisterViewId(0),
                name: "r0".into(),
                class: RegisterClassId(0),
                units: vec![RegisterUnitId(0)],
                write_units: vec![RegisterUnitId(0)],
                bits: 64,
                write_semantics: RegisterWriteSemantics::ExactView,
                allocatable: true,
            }],
            classes: vec![RegisterClass {
                id: RegisterClassId(0),
                name: "integer".into(),
                views: vec![RegisterViewId(0)],
            }],
            conventions: vec![PreservationConvention {
                name: "test-call".into(),
                argument_views: vec![RegisterViewId(0)],
                result_views: vec![RegisterViewId(0)],
                caller_saved: vec![RegisterUnitId(0)],
                callee_saved: Vec::new(),
                fixed: Vec::new(),
                stack_alignment: 16,
                red_zone_bytes: 0,
            }],
            reservations: Vec::new(),
            constraints: Vec::new(),
        }
    }

    #[test]
    fn validator_accepts_a_closed_model_and_rejects_noncanonical_units() {
        assert!(validate_physical_register_model(miniature_model()).is_ok());
        let mut duplicate = miniature_model();
        duplicate.views[0].units.push(RegisterUnitId(0));
        assert_eq!(
            validate_physical_register_model(duplicate),
            Err(RegisterModelValidationError::NonCanonicalUnitSet)
        );
    }

    #[test]
    fn validator_rejects_a_false_zero_extension_footprint() {
        let mut false_zero_extension = miniature_model();
        false_zero_extension.views[0].write_semantics = RegisterWriteSemantics::ZeroExtendsParent;
        assert_eq!(
            validate_physical_register_model(false_zero_extension),
            Err(RegisterModelValidationError::WriteFootprintMismatch(
                RegisterViewId(0)
            ))
        );
    }
}
