//! Bind a machine's formals to their actual storage and evaluate the checked
//! operations that can affect the private cells or the control leading to a wait.

use super::super::facts::Facts;
use super::super::spin::constant;
use super::storage::{Location, Storage, Stored};
use crate::entry_values::TerminalScalarValue as Scalar;
use semantic_vocabulary::{BlockId, IntegerValue, PlaceId, ScalarType, ValueId};
use std::cell::OnceCell;
use terminal_psi::{
    AtomicAccessEvent, Block, Operation, OperationKind, StructuralAccess, StructuralArgument,
    StructuralParameterDeclaration, StructuralPathSegment, TerminalMachine, TerminalModule,
    ValueDeclaration,
};

#[derive(Default)]
pub(super) struct Bindings {
    pub(super) places: Facts<PlaceId, Location>,
    pub(super) values: Facts<ValueId, Scalar>,
}

impl Bindings {
    pub(super) fn arguments(
        &self,
        memory: &mut Storage,
        formals: &[ValueDeclaration],
        places: &[StructuralParameterDeclaration],
        arguments: &[ValueId],
        actuals: &[StructuralArgument],
    ) -> Option<Self> {
        if places.len() != actuals.len() || formals.len() != arguments.len() {
            return None;
        }
        let mut bindings = Self::default();
        for (formal, actual) in formals.iter().zip(arguments) {
            if let Some(v) = self.values.get(actual) {
                bindings.values.insert(formal.id, *v);
            }
        }
        for (formal, actual) in places.iter().zip(actuals) {
            if formal.access != actual.access {
                return None;
            }
            if let Some(loc) = memory.locate(&self.places, actual.place, &actual.path) {
                let loc = if actual.access == StructuralAccess::Owned {
                    memory.allocate(memory.get(&loc).clone())
                } else {
                    loc
                };
                bindings.places.insert(formal.place, loc);
            }
        }
        Some(bindings)
    }
}

// Some Psi instructions encode their last field separately from the rest
// of the projection. Resolve that field against this machine's actual type.
fn field_location(
    module: &TerminalModule,
    machine: &TerminalMachine,
    bindings: &Bindings,
    memory: &Storage,
    root: PlaceId,
    path: &[StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Option<Location> {
    let base = memory.locate(&bindings.places, root, path)?;
    let Some(field) = field else {
        return Some(base);
    };
    let field = crate::projected_field_declaration(
        module.types.structural_types.iter(),
        machine.structural_place_type(root)?,
        path,
        field,
    )?;
    memory.project(base, &[StructuralPathSegment::Field(field.identity)])
}

pub(super) enum Destination {
    Unit,
    Scalar(ValueId),
    Structural(PlaceId),
}

impl Destination {
    pub(super) fn of(op: &Operation) -> Self {
        match &op.result {
            terminal_psi::OperationResult::Unit => Self::Unit,
            terminal_psi::OperationResult::Scalar(value) => Self::Scalar(value.id),
            terminal_psi::OperationResult::Structural(value) => Self::Structural(value.place),
        }
    }
}

pub(super) struct Frame<'m> {
    pub(super) machine: &'m TerminalMachine,
    pub(super) block: &'m Block,
    pub(super) position: usize,
    pub(super) cleanup_cursor: usize,
    pub(super) bindings: Bindings,
    scalar_types: OnceCell<crate::MachineScalarIndex<'m>>,
    pub(super) return_to: Destination,
}

impl<'m> Frame<'m> {
    pub(super) fn enter(
        machine: &'m TerminalMachine,
        bindings: Bindings,
        return_to: Destination,
    ) -> Option<Self> {
        Some(Self {
            machine,
            block: machine.block(&machine.entry)?,
            position: 0,
            cleanup_cursor: 0,
            bindings,
            return_to,
            scalar_types: OnceCell::new(),
        })
    }
    pub(super) fn step(
        &mut self,
        module: &TerminalModule,
        op: &Operation,
        memory: &mut Storage,
    ) -> Option<()> {
        let bindings = &mut self.bindings;
        if let Some(value) = constant(op) {
            bindings.values.insert(op.result.scalar()?.id, value);
            return Some(());
        }
        match &op.kind {
            OperationKind::EstablishZeroedStructural {}
            | OperationKind::EstablishStructuralLocal {} => {
                let value = if matches!(op.kind, OperationKind::EstablishZeroedStructural {}) {
                    Stored::Zero
                } else {
                    Stored::Unknown
                };
                bindings
                    .places
                    .insert(op.result.structural()?.place, memory.allocate(value));
            }
            OperationKind::EstablishReference {
                source,
                storage: None,
            } => {
                let contents = memory
                    .locate(&bindings.places, source.place, &source.path)
                    .map_or(Stored::Unknown, Stored::Reference);
                bindings
                    .places
                    .insert(op.result.structural()?.place, memory.allocate(contents));
            }
            OperationKind::ReleaseReference { source, path } => {
                if let Some(slot) = memory.locate(&bindings.places, *source, path)
                    && matches!(memory.get(&slot), Stored::Reference(_))
                {
                    memory.set(&slot, Stored::Unknown);
                }
            }
            OperationKind::EstablishRecord { fields } => {
                let result = op.result.structural()?;
                let shape = &module.structural_type(result.structural_type)?.shape;
                let layout = shape.direct_fields()?;
                let record = memory.allocate(Stored::Unknown);
                for field in fields {
                    let key = terminal_psi::live_field_by_id(layout, field.field)?.identity;
                    let destination =
                        memory.project(record.clone(), &[StructuralPathSegment::Field(key)])?;
                    let value = match &field.value {
                        terminal_psi::RecordFieldValue::Scalar { value, .. } => {
                            bindings.values.get(value).copied().into()
                        }
                        terminal_psi::RecordFieldValue::Structural(arg) => memory
                            .locate(&bindings.places, arg.place, &arg.path)
                            .map(|loc| memory.get(&loc).clone())
                            .unwrap_or_default(),
                    };
                    memory.set(&destination, value);
                }
                bindings.places.insert(result.place, record);
            }
            OperationKind::StructuralLeafCopy { source, path } => {
                let location = memory.locate(&bindings.places, *source, path)?;
                match &op.result {
                    terminal_psi::OperationResult::Structural(result) => {
                        let location = memory.allocate(memory.get(&location).clone());
                        bindings.places.insert(result.place, location);
                    }
                    terminal_psi::OperationResult::Scalar(result) => {
                        if let Some(value) = memory.get(&location).scalar(result.scalar_type) {
                            bindings.values.insert(result.id, value);
                        }
                    }
                    _ => return None,
                }
            }
            OperationKind::MoveStructuralValue { source } => {
                let location = memory.locate(&bindings.places, source.place, &source.path)?;
                let moved = memory.allocate(memory.get(&location).clone());
                bindings.places.insert(op.result.structural()?.place, moved);
            }
            OperationKind::StoreStructuralField {
                destination,
                path,
                field,
                value,
                ..
            } => {
                let source = memory.locate(&bindings.places, value.place, &value.path)?;
                let dest = field_location(
                    module,
                    self.machine,
                    bindings,
                    memory,
                    *destination,
                    path,
                    *field,
                )?;
                memory.set(&dest, memory.get(&source).clone());
            }
            OperationKind::MoveStructuralField {
                source,
                path,
                field,
            } => {
                let location = field_location(
                    module,
                    self.machine,
                    bindings,
                    memory,
                    *source,
                    path,
                    *field,
                )?;
                let moved = memory.allocate(memory.get(&location).clone());
                bindings.places.insert(op.result.structural()?.place, moved);
            }
            OperationKind::StructuralScalarFieldStore {
                destination,
                path,
                field,
                value,
                ..
            } => {
                let dest = field_location(
                    module,
                    self.machine,
                    bindings,
                    memory,
                    *destination,
                    path,
                    Some(*field),
                )?;
                memory.set(&dest, bindings.values.get(value).copied().into());
            }
            OperationKind::DecomposeStructuralValue { .. } => {
                // The checked operation transfers custody of the existing fields.
                // Its selected destructor runs only at an explicit cleanup edge.
            }
            OperationKind::AtomicAccess {
                place,
                path,
                event,
                resident_obligation: None,
            } => {
                let location = memory.locate(&bindings.places, *place, path)?;
                match event {
                    AtomicAccessEvent::Load { .. } | AtomicAccessEvent::Swap { .. } => {
                        let result = op.result.scalar()?;
                        if let Some(value) = memory.get(&location).scalar(result.scalar_type) {
                            bindings.values.insert(result.id, value);
                        }
                    }
                    AtomicAccessEvent::Store { .. } => {}
                    _ => return None,
                }
                if let AtomicAccessEvent::Store { value, .. }
                | AtomicAccessEvent::Swap { value, .. } = event
                {
                    memory.set(&location, bindings.values.get(value).copied().into());
                }
            }
            _ => {
                let result = op.result.scalar()?;
                let types = self
                    .scalar_types
                    .get_or_init(|| crate::MachineScalarIndex::new(self.machine));
                let origin = crate::scalar_operation_origin(op, result.id, types)?;
                let crate::ScalarConstantOrigin::Computed(term) = origin else {
                    return None;
                };
                let known =
                    crate::inhabitation::known_scalar_value_with(&term, |id| {
                        match bindings.values.get(&id)? {
                            Scalar::Boolean(v) => {
                                Some((ScalarType::Boolean, IntegerValue::Unsigned(u128::from(*v))))
                            }
                            Scalar::Integer { scalar_type, value } => {
                                Some((ScalarType::Integer(*scalar_type), *value))
                            }
                            _ => None,
                        }
                    });
                match known {
                    Some((ScalarType::Boolean, IntegerValue::Unsigned(v))) => {
                        bindings.values.insert(result.id, Scalar::Boolean(v == 1));
                    }
                    Some((ScalarType::Integer(scalar_type), value)) => {
                        bindings
                            .values
                            .insert(result.id, Scalar::Integer { scalar_type, value });
                    }
                    _ if crate::goal_free_scalar_leaf_semantics(op, types)
                        .ok()?
                        .is_some() => {}
                    _ => return None,
                }
            }
        }
        Some(())
    }

    pub(super) fn jump(
        &mut self,
        target: BlockId,
        scalars: &[ValueId],
        places: &[StructuralArgument],
        memory: &mut Storage,
    ) -> Option<()> {
        let block = self.machine.block(&target)?;
        let binding = self.bindings.arguments(
            memory,
            &block.parameters,
            &block.structural_parameters,
            scalars,
            places,
        )?;
        for formal in &block.parameters {
            self.bindings.values.remove(&formal.id);
        }
        for formal in &block.structural_parameters {
            self.bindings.places.remove(&formal.place);
        }
        self.bindings.values.extend(binding.values);
        self.bindings.places.extend(binding.places);
        self.block = block;
        self.position = 0;
        self.cleanup_cursor = 0;
        Some(())
    }
}
