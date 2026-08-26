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

/// Dense, catalog-local identity for an instruction constraint row.
///
/// IDs are canonical only when they match the row's zero-based position in a
/// validated [`RegisterConstraintCatalog`]. The key, rather than this ID, is
/// the stable identity used to join an instruction inventory to its row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterConstraintId(pub u16);

/// The semantic family that owns a register-constraint key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegisterConstraintFamily {
    Call,
    Return,
    SystemCall,
    InlineAssembly,
    Instruction,
}

/// Stable target-owned identity for one constrained instruction form.
///
/// `variant` is assigned by the target owner. It distinguishes calling
/// conventions, syscall ABIs, inline-assembly forms, or instruction forms
/// without relying on display names or declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterConstraintKey {
    pub family: RegisterConstraintFamily,
    pub variant: u32,
}

/// Dataflow access performed by an explicit instruction operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterOperandAccess {
    Use,
    Def,
    UseDef,
}

impl RegisterOperandAccess {
    const fn reads(self) -> bool {
        matches!(self, Self::Use | Self::UseDef)
    }

    const fn writes(self) -> bool {
        matches!(self, Self::Def | Self::UseDef)
    }
}

/// Allocation constraints for one explicit instruction operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterOperandConstraint {
    pub operand: u16,
    pub access: RegisterOperandAccess,
    pub class: RegisterClassId,
    pub fixed_view: Option<RegisterViewId>,
    /// Canonical one-way tie to an earlier operand number.
    pub tied_to: Option<u16>,
    /// The write happens before unrelated input operands have all been read.
    pub early_clobber: bool,
}

/// Complete register effects for one constrained instruction form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterInstructionConstraint {
    pub id: RegisterConstraintId,
    pub key: RegisterConstraintKey,
    pub operands: Vec<RegisterOperandConstraint>,
    pub implicit_uses: Vec<RegisterUnitId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub clobbers: Vec<RegisterUnitId>,
}

/// A target register-constraint inventory and its keyed definitions.
///
/// Both vectors are strictly key-sorted in the validated form. `required`
/// must match the row keys exactly, making omitted and unexpected instruction
/// forms deterministic validation failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterConstraintCatalog {
    pub architecture: Architecture,
    pub required: Vec<RegisterConstraintKey>,
    pub constraints: Vec<RegisterInstructionConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRegisterConstraintCatalog {
    architecture: Architecture,
    catalog: RegisterConstraintCatalog,
}

impl ValidatedRegisterConstraintCatalog {
    pub const fn architecture(&self) -> Architecture {
        self.architecture
    }

    pub const fn catalog(&self) -> &RegisterConstraintCatalog {
        &self.catalog
    }

    pub fn into_catalog(self) -> RegisterConstraintCatalog {
        self.catalog
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterConstraintCatalogValidationError {
    ArchitectureMismatch,
    NonCanonicalConstraintIds,
    NonCanonicalRequiredKeys,
    NonCanonicalConstraintKeys,
    MissingRequiredConstraint(RegisterConstraintKey),
    UnexpectedConstraint(RegisterConstraintKey),
    EmptyConstraint(RegisterConstraintId),
    NonCanonicalOperands(RegisterConstraintId),
    UnknownClass {
        constraint: RegisterConstraintId,
        class: RegisterClassId,
    },
    UnknownFixedView {
        constraint: RegisterConstraintId,
        view: RegisterViewId,
    },
    FixedViewClassMismatch {
        constraint: RegisterConstraintId,
        operand: u16,
    },
    UnallocatableOperandClass {
        constraint: RegisterConstraintId,
        operand: u16,
    },
    InvalidOperandTie {
        constraint: RegisterConstraintId,
        operand: u16,
    },
    IncompatibleOperandTie {
        constraint: RegisterConstraintId,
        operand: u16,
        tied_to: u16,
    },
    InvalidEarlyClobber {
        constraint: RegisterConstraintId,
        operand: u16,
    },
    NonCanonicalImplicitUses(RegisterConstraintId),
    NonCanonicalImplicitDefs(RegisterConstraintId),
    NonCanonicalClobbers(RegisterConstraintId),
    UnknownUnit {
        constraint: RegisterConstraintId,
        unit: RegisterUnitId,
    },
    DefClobberOverlap {
        constraint: RegisterConstraintId,
        unit: RegisterUnitId,
    },
}

impl std::fmt::Display for RegisterConstraintCatalogValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid register constraint catalog: {self:?}")
    }
}

impl std::error::Error for RegisterConstraintCatalogValidationError {}

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
    Ok(ValidatedPhysicalRegisterModel(model))
}

/// Validates a closed register-constraint inventory against an independently
/// validated physical register model.
pub fn validate_register_constraint_catalog(
    catalog: RegisterConstraintCatalog,
    model: &ValidatedPhysicalRegisterModel,
) -> Result<ValidatedRegisterConstraintCatalog, RegisterConstraintCatalogValidationError> {
    let physical = model.model();
    if catalog.architecture != physical.architecture {
        return Err(RegisterConstraintCatalogValidationError::ArchitectureMismatch);
    }
    if catalog
        .constraints
        .iter()
        .enumerate()
        .any(|(expected, constraint)| usize::from(constraint.id.0) != expected)
    {
        return Err(RegisterConstraintCatalogValidationError::NonCanonicalConstraintIds);
    }
    if catalog.required.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(RegisterConstraintCatalogValidationError::NonCanonicalRequiredKeys);
    }
    if catalog
        .constraints
        .windows(2)
        .any(|pair| pair[0].key >= pair[1].key)
    {
        return Err(RegisterConstraintCatalogValidationError::NonCanonicalConstraintKeys);
    }

    validate_required_constraint_inventory(&catalog)?;

    let units = physical
        .units
        .iter()
        .map(|unit| (unit.id, unit))
        .collect::<BTreeMap<_, _>>();
    let views = physical
        .views
        .iter()
        .map(|view| (view.id, view))
        .collect::<BTreeMap<_, _>>();
    let classes = physical
        .classes
        .iter()
        .map(|class| (class.id, class))
        .collect::<BTreeMap<_, _>>();

    for constraint in &catalog.constraints {
        if constraint.operands.is_empty()
            && constraint.implicit_uses.is_empty()
            && constraint.implicit_defs.is_empty()
            && constraint.clobbers.is_empty()
        {
            return Err(RegisterConstraintCatalogValidationError::EmptyConstraint(
                constraint.id,
            ));
        }
        if constraint
            .operands
            .windows(2)
            .any(|pair| pair[0].operand >= pair[1].operand)
        {
            return Err(
                RegisterConstraintCatalogValidationError::NonCanonicalOperands(constraint.id),
            );
        }

        for operand in &constraint.operands {
            let Some(class) = classes.get(&operand.class) else {
                return Err(RegisterConstraintCatalogValidationError::UnknownClass {
                    constraint: constraint.id,
                    class: operand.class,
                });
            };
            if let Some(fixed_view) = operand.fixed_view {
                let Some(view) = views.get(&fixed_view) else {
                    return Err(RegisterConstraintCatalogValidationError::UnknownFixedView {
                        constraint: constraint.id,
                        view: fixed_view,
                    });
                };
                if view.class != operand.class {
                    return Err(
                        RegisterConstraintCatalogValidationError::FixedViewClassMismatch {
                            constraint: constraint.id,
                            operand: operand.operand,
                        },
                    );
                }
            } else if !class
                .views
                .iter()
                .any(|view| views.get(view).is_some_and(|view| view.allocatable))
            {
                return Err(
                    RegisterConstraintCatalogValidationError::UnallocatableOperandClass {
                        constraint: constraint.id,
                        operand: operand.operand,
                    },
                );
            }
            if operand.early_clobber && !operand.access.writes() {
                return Err(
                    RegisterConstraintCatalogValidationError::InvalidEarlyClobber {
                        constraint: constraint.id,
                        operand: operand.operand,
                    },
                );
            }
        }
        validate_operand_ties(constraint)?;

        validate_constraint_unit_set(
            constraint.id,
            &constraint.implicit_uses,
            &units,
            ConstraintUnitSetKind::ImplicitUses,
        )?;
        validate_constraint_unit_set(
            constraint.id,
            &constraint.implicit_defs,
            &units,
            ConstraintUnitSetKind::ImplicitDefs,
        )?;
        validate_constraint_unit_set(
            constraint.id,
            &constraint.clobbers,
            &units,
            ConstraintUnitSetKind::Clobbers,
        )?;
        if let Some(unit) = constraint
            .implicit_defs
            .iter()
            .find(|unit| constraint.clobbers.binary_search(unit).is_ok())
        {
            return Err(
                RegisterConstraintCatalogValidationError::DefClobberOverlap {
                    constraint: constraint.id,
                    unit: *unit,
                },
            );
        }
    }

    Ok(ValidatedRegisterConstraintCatalog {
        architecture: catalog.architecture,
        catalog,
    })
}

fn validate_required_constraint_inventory(
    catalog: &RegisterConstraintCatalog,
) -> Result<(), RegisterConstraintCatalogValidationError> {
    let mut required = catalog.required.iter().copied().peekable();
    let mut actual = catalog
        .constraints
        .iter()
        .map(|constraint| constraint.key)
        .peekable();
    loop {
        match (required.peek().copied(), actual.peek().copied()) {
            (Some(expected), Some(found)) if expected == found => {
                required.next();
                actual.next();
            }
            (Some(expected), Some(found)) if expected < found => {
                return Err(
                    RegisterConstraintCatalogValidationError::MissingRequiredConstraint(expected),
                );
            }
            (Some(_), Some(found)) => {
                return Err(RegisterConstraintCatalogValidationError::UnexpectedConstraint(found));
            }
            (Some(expected), None) => {
                return Err(
                    RegisterConstraintCatalogValidationError::MissingRequiredConstraint(expected),
                );
            }
            (None, Some(found)) => {
                return Err(RegisterConstraintCatalogValidationError::UnexpectedConstraint(found));
            }
            (None, None) => return Ok(()),
        }
    }
}

fn validate_operand_ties(
    constraint: &RegisterInstructionConstraint,
) -> Result<(), RegisterConstraintCatalogValidationError> {
    for operand in &constraint.operands {
        let Some(tied_to) = operand.tied_to else {
            continue;
        };
        if tied_to >= operand.operand {
            return Err(
                RegisterConstraintCatalogValidationError::InvalidOperandTie {
                    constraint: constraint.id,
                    operand: operand.operand,
                },
            );
        }
        let Ok(tied_index) = constraint
            .operands
            .binary_search_by_key(&tied_to, |candidate| candidate.operand)
        else {
            return Err(
                RegisterConstraintCatalogValidationError::InvalidOperandTie {
                    constraint: constraint.id,
                    operand: operand.operand,
                },
            );
        };
        let tied = &constraint.operands[tied_index];
        if tied.tied_to.is_some() {
            return Err(
                RegisterConstraintCatalogValidationError::InvalidOperandTie {
                    constraint: constraint.id,
                    operand: operand.operand,
                },
            );
        }
        let fixed_views_compatible = match (operand.fixed_view, tied.fixed_view) {
            (Some(left), Some(right)) => left == right,
            _ => true,
        };
        let pair_reads = operand.access.reads() || tied.access.reads();
        let pair_writes = operand.access.writes() || tied.access.writes();
        if operand.class != tied.class || !fixed_views_compatible || !pair_reads || !pair_writes {
            return Err(
                RegisterConstraintCatalogValidationError::IncompatibleOperandTie {
                    constraint: constraint.id,
                    operand: operand.operand,
                    tied_to,
                },
            );
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum ConstraintUnitSetKind {
    ImplicitUses,
    ImplicitDefs,
    Clobbers,
}

fn validate_constraint_unit_set(
    constraint: RegisterConstraintId,
    set: &[RegisterUnitId],
    known: &BTreeMap<RegisterUnitId, &RegisterUnit>,
    kind: ConstraintUnitSetKind,
) -> Result<(), RegisterConstraintCatalogValidationError> {
    if set.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(match kind {
            ConstraintUnitSetKind::ImplicitUses => {
                RegisterConstraintCatalogValidationError::NonCanonicalImplicitUses(constraint)
            }
            ConstraintUnitSetKind::ImplicitDefs => {
                RegisterConstraintCatalogValidationError::NonCanonicalImplicitDefs(constraint)
            }
            ConstraintUnitSetKind::Clobbers => {
                RegisterConstraintCatalogValidationError::NonCanonicalClobbers(constraint)
            }
        });
    }
    if let Some(unit) = set.iter().find(|unit| !known.contains_key(unit)) {
        return Err(RegisterConstraintCatalogValidationError::UnknownUnit {
            constraint,
            unit: *unit,
        });
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn miniature_model() -> PhysicalRegisterModel {
        PhysicalRegisterModel {
            architecture: Architecture::X86_64,
            units: vec![
                RegisterUnit {
                    id: RegisterUnitId(0),
                    name: "r0.storage".into(),
                    bits: 64,
                    kind: RegisterUnitKind::IntegerLane,
                },
                RegisterUnit {
                    id: RegisterUnitId(1),
                    name: "v0.storage".into(),
                    bits: 128,
                    kind: RegisterUnitKind::VectorLane,
                },
            ],
            views: vec![
                RegisterView {
                    id: RegisterViewId(0),
                    name: "r0".into(),
                    class: RegisterClassId(0),
                    units: vec![RegisterUnitId(0)],
                    write_units: vec![RegisterUnitId(0)],
                    bits: 64,
                    write_semantics: RegisterWriteSemantics::ExactView,
                    allocatable: true,
                },
                RegisterView {
                    id: RegisterViewId(1),
                    name: "v0".into(),
                    class: RegisterClassId(1),
                    units: vec![RegisterUnitId(1)],
                    write_units: vec![RegisterUnitId(1)],
                    bits: 128,
                    write_semantics: RegisterWriteSemantics::ExactView,
                    allocatable: true,
                },
            ],
            classes: vec![
                RegisterClass {
                    id: RegisterClassId(0),
                    name: "integer".into(),
                    views: vec![RegisterViewId(0)],
                },
                RegisterClass {
                    id: RegisterClassId(1),
                    name: "vector".into(),
                    views: vec![RegisterViewId(1)],
                },
            ],
            conventions: vec![PreservationConvention {
                name: "test-call".into(),
                argument_views: vec![RegisterViewId(0)],
                result_views: vec![RegisterViewId(0)],
                caller_saved: vec![RegisterUnitId(0)],
                callee_saved: vec![RegisterUnitId(1)],
                fixed: Vec::new(),
                stack_alignment: 16,
                red_zone_bytes: 0,
            }],
            reservations: Vec::new(),
        }
    }

    fn instruction_key(variant: u32) -> RegisterConstraintKey {
        RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant,
        }
    }

    fn miniature_catalog() -> RegisterConstraintCatalog {
        let key = instruction_key(7);
        RegisterConstraintCatalog {
            architecture: Architecture::X86_64,
            required: vec![key],
            constraints: vec![RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: vec![
                    RegisterOperandConstraint {
                        operand: 0,
                        access: RegisterOperandAccess::Use,
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: false,
                    },
                    RegisterOperandConstraint {
                        operand: 1,
                        access: RegisterOperandAccess::Def,
                        class: RegisterClassId(0),
                        fixed_view: Some(RegisterViewId(0)),
                        tied_to: Some(0),
                        early_clobber: true,
                    },
                ],
                implicit_uses: vec![RegisterUnitId(0)],
                implicit_defs: Vec::new(),
                clobbers: vec![RegisterUnitId(1)],
            }],
        }
    }

    fn validated_miniature_model() -> ValidatedPhysicalRegisterModel {
        validate_physical_register_model(miniature_model()).expect("miniature model must validate")
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

    #[test]
    fn constraint_catalog_accepts_a_closed_required_inventory() {
        let validated =
            validate_register_constraint_catalog(miniature_catalog(), &validated_miniature_model())
                .expect("closed catalog must validate");

        assert_eq!(validated.architecture(), Architecture::X86_64);
        assert_eq!(validated.catalog().required, vec![instruction_key(7)]);
    }

    #[test]
    fn constraint_catalog_rejects_missing_and_unexpected_inventory_rows() {
        let model = validated_miniature_model();
        let mut missing = miniature_catalog();
        missing.required.push(instruction_key(8));
        assert_eq!(
            validate_register_constraint_catalog(missing, &model),
            Err(
                RegisterConstraintCatalogValidationError::MissingRequiredConstraint(
                    instruction_key(8)
                )
            )
        );

        let mut unexpected = miniature_catalog();
        let mut row = unexpected.constraints[0].clone();
        row.id = RegisterConstraintId(1);
        row.key = instruction_key(8);
        unexpected.constraints.push(row);
        assert_eq!(
            validate_register_constraint_catalog(unexpected, &model),
            Err(RegisterConstraintCatalogValidationError::UnexpectedConstraint(instruction_key(8)))
        );
    }

    #[test]
    fn constraint_catalog_rejects_noncanonical_ids_keys_and_operands() {
        let model = validated_miniature_model();
        let mut bad_id = miniature_catalog();
        bad_id.constraints[0].id = RegisterConstraintId(1);
        assert_eq!(
            validate_register_constraint_catalog(bad_id, &model),
            Err(RegisterConstraintCatalogValidationError::NonCanonicalConstraintIds)
        );

        let mut duplicate_required = miniature_catalog();
        duplicate_required.required.push(instruction_key(7));
        assert_eq!(
            validate_register_constraint_catalog(duplicate_required, &model),
            Err(RegisterConstraintCatalogValidationError::NonCanonicalRequiredKeys)
        );

        let mut duplicate_key = miniature_catalog();
        let mut second_row = duplicate_key.constraints[0].clone();
        second_row.id = RegisterConstraintId(1);
        duplicate_key.constraints.push(second_row);
        assert_eq!(
            validate_register_constraint_catalog(duplicate_key, &model),
            Err(RegisterConstraintCatalogValidationError::NonCanonicalConstraintKeys)
        );

        let mut duplicate_operand = miniature_catalog();
        duplicate_operand.constraints[0].operands[1].operand = 0;
        assert_eq!(
            validate_register_constraint_catalog(duplicate_operand, &model),
            Err(
                RegisterConstraintCatalogValidationError::NonCanonicalOperands(
                    RegisterConstraintId(0)
                )
            )
        );
    }

    #[test]
    fn constraint_catalog_rejects_fixed_view_class_corruption() {
        let model = validated_miniature_model();
        let mut corrupted = miniature_catalog();
        corrupted.constraints[0].operands[1].class = RegisterClassId(1);

        assert_eq!(
            validate_register_constraint_catalog(corrupted, &model),
            Err(
                RegisterConstraintCatalogValidationError::FixedViewClassMismatch {
                    constraint: RegisterConstraintId(0),
                    operand: 1,
                }
            )
        );
    }

    #[test]
    fn constraint_catalog_rejects_unknown_or_unallocatable_operand_domains() {
        let model = validated_miniature_model();
        let mut unknown_class = miniature_catalog();
        unknown_class.constraints[0].operands[0].class = RegisterClassId(u16::MAX);
        assert_eq!(
            validate_register_constraint_catalog(unknown_class, &model),
            Err(RegisterConstraintCatalogValidationError::UnknownClass {
                constraint: RegisterConstraintId(0),
                class: RegisterClassId(u16::MAX),
            })
        );

        let mut unknown_view = miniature_catalog();
        unknown_view.constraints[0].operands[1].fixed_view = Some(RegisterViewId(u16::MAX));
        assert_eq!(
            validate_register_constraint_catalog(unknown_view, &model),
            Err(RegisterConstraintCatalogValidationError::UnknownFixedView {
                constraint: RegisterConstraintId(0),
                view: RegisterViewId(u16::MAX),
            })
        );

        let mut physical = miniature_model();
        physical.views[1].allocatable = false;
        let physical = validate_physical_register_model(physical).unwrap();
        let mut unallocatable = miniature_catalog();
        unallocatable.constraints[0].operands[0].class = RegisterClassId(1);
        assert_eq!(
            validate_register_constraint_catalog(unallocatable, &physical),
            Err(
                RegisterConstraintCatalogValidationError::UnallocatableOperandClass {
                    constraint: RegisterConstraintId(0),
                    operand: 0,
                }
            )
        );

        let mut empty = miniature_catalog();
        let row = &mut empty.constraints[0];
        row.operands.clear();
        row.implicit_uses.clear();
        row.implicit_defs.clear();
        row.clobbers.clear();
        assert_eq!(
            validate_register_constraint_catalog(empty, &model),
            Err(RegisterConstraintCatalogValidationError::EmptyConstraint(
                RegisterConstraintId(0)
            ))
        );
    }

    #[test]
    fn constraint_catalog_rejects_malformed_ties_and_early_clobbers() {
        let model = validated_miniature_model();
        let mut self_tie = miniature_catalog();
        self_tie.constraints[0].operands[1].tied_to = Some(1);
        assert_eq!(
            validate_register_constraint_catalog(self_tie, &model),
            Err(
                RegisterConstraintCatalogValidationError::InvalidOperandTie {
                    constraint: RegisterConstraintId(0),
                    operand: 1,
                }
            )
        );

        let mut dangling_tie = miniature_catalog();
        dangling_tie.constraints[0].operands[1].operand = 2;
        dangling_tie.constraints[0].operands[1].tied_to = Some(1);
        assert_eq!(
            validate_register_constraint_catalog(dangling_tie, &model),
            Err(
                RegisterConstraintCatalogValidationError::InvalidOperandTie {
                    constraint: RegisterConstraintId(0),
                    operand: 2,
                }
            )
        );

        let mut incompatible_tie = miniature_catalog();
        incompatible_tie.constraints[0].operands[1].class = RegisterClassId(1);
        incompatible_tie.constraints[0].operands[1].fixed_view = None;
        assert_eq!(
            validate_register_constraint_catalog(incompatible_tie, &model),
            Err(
                RegisterConstraintCatalogValidationError::IncompatibleOperandTie {
                    constraint: RegisterConstraintId(0),
                    operand: 1,
                    tied_to: 0,
                }
            )
        );

        let mut early_use = miniature_catalog();
        early_use.constraints[0].operands[0].early_clobber = true;
        assert_eq!(
            validate_register_constraint_catalog(early_use, &model),
            Err(
                RegisterConstraintCatalogValidationError::InvalidEarlyClobber {
                    constraint: RegisterConstraintId(0),
                    operand: 0,
                }
            )
        );
    }

    #[test]
    fn constraint_catalog_rejects_implicit_effect_corruption() {
        let model = validated_miniature_model();
        let mut duplicate_use = miniature_catalog();
        duplicate_use.constraints[0]
            .implicit_uses
            .push(RegisterUnitId(0));
        assert_eq!(
            validate_register_constraint_catalog(duplicate_use, &model),
            Err(
                RegisterConstraintCatalogValidationError::NonCanonicalImplicitUses(
                    RegisterConstraintId(0)
                )
            )
        );

        let mut unknown_clobber = miniature_catalog();
        unknown_clobber.constraints[0].clobbers = vec![RegisterUnitId(2)];
        assert_eq!(
            validate_register_constraint_catalog(unknown_clobber, &model),
            Err(RegisterConstraintCatalogValidationError::UnknownUnit {
                constraint: RegisterConstraintId(0),
                unit: RegisterUnitId(2),
            })
        );

        let mut contradictory_write = miniature_catalog();
        contradictory_write.constraints[0].implicit_defs = vec![RegisterUnitId(1)];
        assert_eq!(
            validate_register_constraint_catalog(contradictory_write, &model),
            Err(
                RegisterConstraintCatalogValidationError::DefClobberOverlap {
                    constraint: RegisterConstraintId(0),
                    unit: RegisterUnitId(1),
                }
            )
        );
    }

    #[test]
    fn constraint_catalog_is_bound_to_the_validated_model_architecture() {
        let mut wrong_architecture = miniature_catalog();
        wrong_architecture.architecture = Architecture::Aarch64;

        assert_eq!(
            validate_register_constraint_catalog(wrong_architecture, &validated_miniature_model()),
            Err(RegisterConstraintCatalogValidationError::ArchitectureMismatch)
        );
    }
}
