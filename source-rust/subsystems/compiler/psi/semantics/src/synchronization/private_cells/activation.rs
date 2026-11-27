//! Bind a machine's formals to their actual storage and evaluate the checked
//! operations that can affect the private cells or the control leading to a wait.

use super::super::facts::Facts;
use super::super::progress::ScalarFact;
use super::super::spin::{CellUpdate, constant};
use crate::synchronization::progress::condition::Assumption;
use super::storage::{Location, Storage, Stored};
use crate::entry_values::TerminalScalarValue as Scalar;
use semantic_vocabulary::{BlockId, EdgeId, PlaceId, ValueId};
use std::cell::OnceCell;
use std::rc::Rc;
use terminal_psi::{
    AtomicAccessEvent, Block, Operation, OperationKind, StructuralAccess, StructuralArgument,
    StructuralParameterDeclaration, StructuralPathSegment, TerminalAffineCleanupAction,
    TerminalMachine, TerminalModule, ValueDeclaration,
};

#[derive(Clone)]
pub(super) struct Bindings<'m> {
    pub(super) module: &'m TerminalModule,
    pub(super) machine: &'m TerminalMachine,
    pub(super) places: Facts<PlaceId, Location>,
    pub(super) values: Facts<ValueId, ScalarFact>,
    pub(super) predicates: Facts<ValueId, Assumption>,
}

pub(super) enum ScalarArguments<'a> {
    Values(&'a [ValueId]),
    Payload(Vec<Option<ScalarFact>>),
}

impl<'m> Bindings<'m> {
    pub(super) fn new(module: &'m TerminalModule, machine: &'m TerminalMachine) -> Self {
        Self {
            module,
            machine,
            places: Facts::default(),
            values: Facts::default(),
            predicates: Facts::default(),
        }
    }

    pub(super) fn predicate(&self, value: ValueId) -> Option<Assumption> {
        self.predicates
            .get(&value)
            .copied()
            .or_else(|| Assumption::boolean(*self.values.get(&value)?))
    }

    pub(super) fn join(&mut self, other: &Self, arrivals: &mut super::storage::Arrivals<'_>) {
        self.values.retain_equal(&other.values);
        self.predicates.retain_equal(&other.predicates);
        self.places.retain(|key, place| {
            if let Some(incoming) = other.places.get(key) {
                arrivals.place(place, incoming);
                true
            } else {
                false
            }
        });
    }

    pub(super) fn arguments(
        &self,
        memory: &mut Storage,
        formals: &[ValueDeclaration],
        places: &[StructuralParameterDeclaration],
        arguments: ScalarArguments<'_>,
        actuals: &[StructuralArgument],
    ) -> Option<Self> {
        let count = match &arguments {
            ScalarArguments::Values(values) => values.len(),
            ScalarArguments::Payload(fields) => fields.len(),
        };
        if places.len() != actuals.len() || formals.len() != count {
            return None;
        }
        let mut bindings = Self::new(self.module, self.machine);
        for (index, formal) in formals.iter().enumerate() {
            let value = match &arguments {
                ScalarArguments::Values(values) => self.values.get(&values[index]).copied(),
                ScalarArguments::Payload(fields) => fields[index],
            };
            if let ScalarArguments::Values(values) = &arguments
                && let Some(test) = self.predicates.get(&values[index])
            {
                bindings.predicates.insert(formal.id, *test);
            }
            if let Some(value) = value {
                bindings.values.insert(formal.id, value);
            }
        }
        for (formal, actual) in places.iter().zip(actuals) {
            if formal.access != actual.access {
                return None;
            }
            if let Some(loc) = memory.locate(self, actual.place, &actual.path) {
                let loc = if actual.access == StructuralAccess::Owned {
                    memory.allocate(memory.get(&loc))
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
    let base = memory.locate(bindings, root, path)?;
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

#[derive(Clone, Copy)]
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

/// The interpreter selects an edge and captures all block arguments before
/// invoking any cleanup. Its referenced storage remains live until arrival.
#[derive(Clone)]
pub(super) struct Arrival<'m> {
    pub(super) edge: EdgeId,
    pub(super) bindings: Bindings<'m>,
    pub(super) cleanup: &'m [TerminalAffineCleanupAction],
    block: &'m Block,
}

#[derive(Clone)]
pub(super) struct Frame<'m> {
    pub(super) machine: &'m TerminalMachine,
    pub(super) block: &'m Block,
    pub(super) position: usize,
    pub(super) cleanup_cursor: usize,
    pub(super) selected_arm: Option<usize>,
    pub(super) arrival: Option<Arrival<'m>>,
    visited: Facts<BlockId, ()>,
    pub(super) bindings: Bindings<'m>,
    scalar_types: Rc<OnceCell<crate::MachineScalarIndex<'m>>>,
    pub(super) return_to: Destination,
}

impl<'m> Frame<'m> {
    pub(super) fn enter(
        machine: &'m TerminalMachine,
        mut bindings: Bindings<'m>,
        return_to: Destination,
    ) -> Option<Self> {
        bindings.machine = machine;
        Some(Self {
            machine,
            block: machine.block(&machine.entry)?,
            position: 0,
            cleanup_cursor: 0,
            selected_arm: None,
            arrival: None,
            visited: Facts::default(),
            bindings,
            return_to,
            scalar_types: Rc::default(),
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
            bindings
                .values
                .insert(op.result.scalar()?.id, ScalarFact::Literal(value));
            return Some(());
        }
        if let Some(result) = op.result.scalar()
            && let Some(test) =
                Assumption::of_operation(&op.kind, &bindings.values, &bindings.predicates)
        {
            bindings.predicates.insert(result.id, test);
        }
        match &op.kind {
            // Selecting code alone cannot touch any of this activation's cells.
            // The callable flow supplies the selected body at its invocation.
            OperationKind::SelectMachine { .. } => {}
            OperationKind::EstablishZeroedStructural {}
            | OperationKind::EstablishStructuralLocal {} => {
                let value = if matches!(op.kind, OperationKind::EstablishZeroedStructural {}) {
                    let shape = &module
                        .structural_type(op.result.structural()?.structural_type)?
                        .shape;
                    match shape.cases().first() {
                        Some(first) => Stored::ZERO.select_case(first.id),
                        None => Stored::ZERO,
                    }
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
                    .locate(bindings, source.place, &source.path)
                    .map_or(Stored::Unknown, Stored::Reference);
                bindings
                    .places
                    .insert(op.result.structural()?.place, memory.allocate(contents));
            }
            OperationKind::ReleaseReference { source, path } => {
                if let Some(slot) = memory.locate(bindings, *source, path)
                    && matches!(memory.get(&slot), Stored::Reference(_))
                {
                    memory.set(&slot, Stored::Unknown);
                }
            }
            OperationKind::EstablishRecord { .. }
            | OperationKind::EstablishScalarCase { .. }
            | OperationKind::EstablishStructuralCase { .. } => {
                let result = op.result.structural()?;
                let shape = &module.structural_type(result.structural_type)?.shape;
                let value = super::aggregate::establish(shape, &op.kind, bindings, memory)?;
                bindings.places.insert(result.place, value);
            }
            OperationKind::StructuralCaseMembership { source, path, case } => {
                let target = memory.locate(bindings, *source, path);
                let contents = target.as_ref().map_or(Stored::Unknown, |v| memory.get(v));
                let tags = contents.tags();
                let membership = match tags {
                    [_] => Some(tags[0] == *case),
                    [] => None,
                    tags if !tags.contains(case) => Some(false),
                    _ => None,
                };
                if let Some(answer) = membership {
                    bindings.values.insert(
                        op.result.scalar()?.id,
                        ScalarFact::Literal(Scalar::Boolean(answer)),
                    );
                }
            }
            OperationKind::StructuralLeafCopy { source, path } => {
                // Copying an input of unknown value still creates the checked
                // result. Its possible cases can be visited independently.
                let location = memory.locate(bindings, *source, path);
                let value = location.as_ref().map_or(Stored::Unknown, |v| memory.get(v));
                match &op.result {
                    terminal_psi::OperationResult::Structural(result) => {
                        let location = memory.allocate(value.clone());
                        bindings.places.insert(result.place, location);
                    }
                    terminal_psi::OperationResult::Scalar(result) => {
                        if let Some(value) = value.scalar(result.scalar_type) {
                            bindings.values.insert(result.id, value);
                        }
                    }
                    _ => return None,
                }
            }
            OperationKind::MoveStructuralValue { source } => {
                let location = memory.locate(bindings, source.place, &source.path)?;
                let moved = memory.allocate(memory.get(&location));
                bindings.places.insert(op.result.structural()?.place, moved);
            }
            OperationKind::StoreStructuralField {
                destination,
                path,
                field,
                value,
                ..
            } => {
                let source = memory.locate(bindings, value.place, &value.path)?;
                let dest = field_location(
                    module,
                    self.machine,
                    bindings,
                    memory,
                    *destination,
                    path,
                    *field,
                )?;
                memory.set(&dest, memory.get(&source));
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
                let moved = memory.allocate(memory.get(&location));
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
                let location = memory.locate(bindings, *place, path)?;
                let observed = if event.observes_resident() {
                    let result = op.result.scalar()?;
                    let prior = memory.get(&location).scalar(result.scalar_type);
                    if let Some(value) = prior {
                        bindings.values.insert(result.id, value);
                    }
                    prior
                } else {
                    None
                };
                let next = match event {
                    AtomicAccessEvent::Load { .. } => return Some(()),
                    AtomicAccessEvent::Store { value, .. }
                    | AtomicAccessEvent::Swap { value, .. } => {
                        bindings.values.get(value).copied().into()
                    }
                    AtomicAccessEvent::ReadModifyWrite {
                        operation, operand, ..
                    } => {
                        let ty = op.result.scalar()?.scalar_type;
                        let operand = bindings.values.get(operand).copied();
                        memory.get(&location).map_scalars(ty, |prior| {
                            let (ScalarFact::Literal(prior), Some(ScalarFact::Literal(operand))) =
                                (prior, operand)
                            else {
                                return None;
                            };
                            crate::atomic_read_modify_write(*operation, prior, operand)
                                .map(ScalarFact::Literal)
                        })
                    }
                    AtomicAccessEvent::CompareExchange {
                        expected,
                        replacement,
                        ..
                    } => {
                        let expect = bindings.values.get(expected).copied();
                        let proposal = bindings.values.get(replacement).copied();
                        match (expect, proposal) {
                            (Some(expected), _) if observed == Some(expected) => proposal.into(),
                            (Some(expected), _) if proposal == Some(expected) => return Some(()),
                            (Some(expected), Some(replacement)) => memory
                                .get(&location)
                                .after_update(CellUpdate::CompareExchange {
                                    expected,
                                    replacement,
                                }),
                            (Some(ScalarFact::Literal(_)), _)
                                if matches!(observed, Some(ScalarFact::Literal(_))) =>
                            {
                                return Some(());
                            }
                            _ => Stored::Unknown,
                        }
                    }
                    _ => return None,
                };
                memory.set(&location, next);
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
                let known = crate::inhabitation::known_scalar_value_with(&term, |id| {
                    bindings.values.get(&id)?.known_integer()
                });
                if let Some(fact) = known.and_then(|(ty, bits)| ScalarFact::from_integer(ty, bits))
                {
                    bindings.values.insert(result.id, fact);
                } else if crate::goal_free_scalar_leaf_semantics(op, types)
                    .ok()?
                    .is_none()
                {
                    return None;
                }
            }
        }
        Some(())
    }

    pub(super) fn prepare_arrival(
        &mut self,
        edge: EdgeId,
        target: BlockId,
        scalars: ScalarArguments<'_>,
        places: &[StructuralArgument],
        cleanup: &'m [TerminalAffineCleanupAction],
        memory: &mut Storage,
    ) -> Option<()> {
        let block = self.machine.block(&target)?;
        let bindings = self.bindings.arguments(
            memory,
            &block.parameters,
            &block.structural_parameters,
            scalars,
            places,
        )?;
        self.arrival = Some(Arrival {
            edge,
            block,
            bindings,
            cleanup,
        });
        Some(())
    }

    pub(super) fn arrive(&mut self) -> Option<()> {
        let Arrival {
            block,
            bindings: binding,
            ..
        } = self.arrival.take()?;
        if block.id == self.machine.entry || self.visited.insert(block.id, ()).is_some() {
            return None;
        }
        for formal in &block.parameters {
            self.bindings.values.remove(&formal.id);
            self.bindings.predicates.remove(&formal.id);
        }
        for formal in &block.structural_parameters {
            self.bindings.places.remove(&formal.place);
        }
        self.bindings.values.extend(binding.values);
        self.bindings.predicates.extend(binding.predicates);
        self.bindings.places.extend(binding.places);
        self.block = block;
        self.position = 0;
        self.cleanup_cursor = 0;
        self.selected_arm = None;
        Some(())
    }
}
