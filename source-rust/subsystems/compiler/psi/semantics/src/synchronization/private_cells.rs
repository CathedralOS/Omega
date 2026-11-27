//! Instantiate derived wait predicates on fresh, unescaped storage. Borrow
//! arguments identify the same allocation across checked calls and jumps.

use super::facts::Facts;
use super::progress::{PrivateWaitFailure, ProgressFailure, ScalarFact, entry_scalars};
use super::selected::Summaries;
use super::spin::{SpinWait, constant};
use crate::entry_values::TerminalScalarValue as Scalar;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, MachineId, OperationId, PlaceId, ScalarType, ValueId,
};
use std::cell::OnceCell;
use terminal_psi::{
    AtomicAccessEvent, Block, Operation, OperationKind, StructuralAccess, StructuralArgument,
    StructuralParameterDeclaration, StructuralPathSegment, TargetMachineCallable,
    TerminalAffineCleanupAction, TerminalMachine, TerminalModule, Terminator, ValueDeclaration,
};

/// Independent source fields and fixed indices retain their last proven bits.
struct Contents {
    zeroed: bool,
    writes: Vec<(Vec<StructuralPathSegment>, Option<ScalarFact>)>,
}

#[derive(Clone)]
struct Location {
    allocation: usize,
    path: Vec<StructuralPathSegment>,
}

impl Location {
    fn read(&self, allocations: &[Contents], ty: ScalarType) -> Option<ScalarFact> {
        let memory = &allocations[self.allocation];
        for (path, value) in memory.writes.iter().rev() {
            if path == &self.path {
                return value.filter(|bits| bits.ty() == ty);
            }
            if crate::structural_paths_may_overlap(path, &self.path) {
                return None;
            }
        }
        memory
            .zeroed
            .then(|| Scalar::zero(ty))
            .flatten()
            .map(ScalarFact::Literal)
    }

    fn write(self, allocations: &mut [Contents], value: Option<ScalarFact>) {
        let writes = &mut allocations[self.allocation].writes;
        writes.retain(|(path, _)| path != &self.path);
        writes.push((self.path, value));
    }
}

#[derive(Default)]
struct Bindings {
    places: Facts<PlaceId, Location>,
    references: Facts<PlaceId, Location>,
    values: Facts<ValueId, ScalarFact>,
}

impl Bindings {
    fn locate(&self, root: PlaceId, path: &[StructuralPathSegment]) -> Option<Location> {
        let (mut loc, path) = match path {
            [StructuralPathSegment::Referent, rest @ ..] => {
                (self.references.get(&root)?.clone(), rest)
            }
            _ => (self.places.get(&root)?.clone(), path),
        };
        if !path.iter().all(|part| {
            matches!(
                part,
                StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
            )
        }) {
            return None;
        }
        loc.path.extend_from_slice(path);
        Some(loc)
    }

    // Only read and write borrow formals forward the caller's actual storage.
    // Owned transfers need a separate copy/move and alias argument.
    fn arguments(
        &self,
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
            if actual.access == StructuralAccess::Owned || formal.access != actual.access {
                return None;
            }
            if let Some(loc) = self.locate(actual.place, &actual.path) {
                bindings.places.insert(formal.place, loc);
            }
        }
        Some(bindings)
    }
}

struct Frame<'m> {
    machine: &'m TerminalMachine,
    block: &'m Block,
    position: usize,
    bindings: Bindings,
    scalar_types: OnceCell<crate::MachineScalarIndex<'m>>,
    return_to: Option<ValueId>,
}

impl<'m> Frame<'m> {
    fn enter(
        machine: &'m TerminalMachine,
        bindings: Bindings,
        return_to: Option<ValueId>,
    ) -> Option<Self> {
        Some(Self {
            machine,
            block: machine.block(&machine.entry)?,
            position: 0,
            bindings,
            return_to,
            scalar_types: OnceCell::new(),
        })
    }
    fn step(&mut self, op: &Operation, allocations: &mut Vec<Contents>) -> Option<()> {
        let bindings = &mut self.bindings;
        if let Some(value) = constant(op) {
            bindings
                .values
                .insert(op.result.scalar()?.id, ScalarFact::Literal(value));
            return Some(());
        }
        match &op.kind {
            OperationKind::EstablishZeroedStructural {}
            | OperationKind::EstablishStructuralLocal {} => {
                let place = op.result.as_structural()?.place;
                bindings.places.insert(
                    place,
                    Location {
                        allocation: allocations.len(),
                        path: Vec::new(),
                    },
                );
                allocations.push(Contents {
                    zeroed: matches!(op.kind, OperationKind::EstablishZeroedStructural {}),
                    writes: Vec::new(),
                });
            }
            OperationKind::EstablishReference {
                source,
                storage: None,
            } => {
                if let Some(loc) = bindings.locate(source.place, &source.path) {
                    bindings
                        .references
                        .insert(op.result.as_structural()?.place, loc);
                }
            }
            OperationKind::ReleaseReference { source, path } if path.is_empty() => {
                bindings.references.remove(source);
            }
            OperationKind::AtomicAccess {
                place,
                path,
                event,
                resident_obligation: None,
            } => {
                let location = bindings.locate(*place, path)?;
                match event {
                    AtomicAccessEvent::Load { .. } | AtomicAccessEvent::Swap { .. } => {
                        let result = op.result.scalar()?;
                        if let Some(value) = location.read(allocations, result.scalar_type) {
                            bindings.values.insert(result.id, value);
                        }
                    }
                    AtomicAccessEvent::Store { .. } => {}
                    _ => return None,
                }
                if let AtomicAccessEvent::Store { value, .. }
                | AtomicAccessEvent::Swap { value, .. } = event
                {
                    location.write(allocations, bindings.values.get(value).copied());
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

    fn jump(
        &mut self,
        target: BlockId,
        scalars: &[ValueId],
        places: &[StructuralArgument],
        cleanup: &[TerminalAffineCleanupAction],
    ) -> Option<()> {
        if !cleanup.is_empty() {
            return None;
        }
        let block = self.machine.block(&target)?;
        let binding = self.bindings.arguments(
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
            self.bindings.references.remove(&formal.place);
        }
        self.bindings.values.extend(binding.values);
        self.bindings.places.extend(binding.places);
        self.block = block;
        self.position = 0;
        Some(())
    }
}

/// Find the first wait on private storage whose progress is violated or unproved.
/// The caller supplies summaries only for its exact admitted boundary bodies.
/// Stop upon unknown effects, control or aliasing; repeat visits to ordinary
/// blocks also stop, so expanding acyclic helper calls stays bounded by the IR.
pub fn private_wait_failure(
    module: &TerminalModule,
    entry: MachineId,
    boundary: impl Fn(BoundaryMachineId) -> Option<SpinWait>,
) -> Option<PrivateWaitFailure> {
    if !module.types.structural_types.iter().any(|ty| {
        matches!(ty.shape, terminal_psi::StructuralTypeShape::AtomicCell(_))
            || ty
                .shape
                .fields()
                .any(|field| field.field_type.atomic_cell_type().is_some())
    }) {
        return None;
    }
    let root = module.machine(entry)?;
    let parameters = Bindings {
        values: entry_scalars(root),
        ..Default::default()
    };
    if crate::inhabitation::predicates_are_false_with(&root.requires, |id| {
        parameters.values.get(&id)?.known_integer()
    }) {
        return None;
    }
    let mut stack = vec![Frame::enter(root, parameters, None)?];
    let mut allocations = Vec::new();
    let mut seen = Facts::default();
    let mut summaries = Summaries::default();
    while let Some(frame) = stack.last_mut() {
        if frame.position == 0
            && seen
                .insert((frame.machine.id, frame.block.id), ())
                .is_some()
        {
            return None;
        }
        if let Some(op) = frame.block.operations.get(frame.position) {
            frame.position += 1;
            // A suspending call can return immediately. Its selected body or
            // admitted boundary summary determines which effects reach this frame.
            let Some(call) = op.declared_invocation() else {
                frame.step(op, &mut allocations)?;
                continue;
            };
            let spin = match call.callee {
                TargetMachineCallable::Machine(id) => {
                    if let Some(summary) = summaries.get(module, id, &boundary) {
                        summary
                    } else {
                        let callee = module.machine(id)?;
                        if op.result.as_structural().is_some() {
                            return None;
                        }
                        let args = frame.bindings.arguments(
                            &callee.parameters,
                            &callee.structural_parameters,
                            call.arguments,
                            call.structural_arguments,
                        )?;
                        stack.push(Frame::enter(
                            callee,
                            args,
                            op.result.scalar().map(|s| s.id),
                        )?);
                        continue;
                    }
                }
                TargetMachineCallable::Boundary(id) => boundary(id)?,
            };
            let actual = call.structural_arguments.get(spin.parameter)?;
            if actual.access != StructuralAccess::SharedBorrow {
                return None;
            }
            let mut location = frame.bindings.locate(actual.place, &actual.path)?;
            location.path.extend(spin.path);
            let initial = location.read(&allocations, spin.while_value.value.scalar_type());
            if let Some(cause) = spin
                .while_value
                .failure(initial, spin.writes, &root.requires)
            {
                return Some(PrivateWaitFailure {
                    machine: frame.machine.id,
                    operation: op.id,
                    cause,
                });
            }
            if let Some(value) = spin.writes {
                location.write(&mut allocations, Some(ScalarFact::Literal(value)));
            }
            continue;
        }
        match &frame.block.terminator {
            Terminator::Jump {
                target,
                arguments,
                structural_arguments,
                cleanup_actions,
                ..
            } => frame.jump(*target, arguments, structural_arguments, cleanup_actions)?,
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let ScalarFact::Literal(Scalar::Boolean(test)) =
                    frame.bindings.values.get(condition)?
                else {
                    return None;
                };
                let edge = if *test { when_true } else { when_false };
                frame.jump(
                    edge.target,
                    &edge.arguments,
                    &edge.structural_arguments,
                    &edge.cleanup_actions,
                )?;
            }
            Terminator::ReturnUnit {
                cleanup_actions, ..
            }
            | Terminator::Return {
                cleanup_actions, ..
            } if cleanup_actions
                .iter()
                .all(|action| action.invoked_machine().is_none()) =>
            {
                let returned = if let Terminator::Return { value, .. } = &frame.block.terminator {
                    frame
                        .return_to
                        .zip(frame.bindings.values.get(value).copied())
                } else {
                    None
                };
                stack.pop();
                if let Some((dest, v)) = returned {
                    stack.last_mut()?.bindings.values.insert(dest, v);
                }
            }
            _ => return None,
        }
    }
    None
}

/// Only positively established violations, for callers consuming that judgment.
pub fn stuck_private_wait(
    module: &TerminalModule,
    entry: MachineId,
    boundary: impl Fn(BoundaryMachineId) -> Option<SpinWait>,
) -> Option<(MachineId, OperationId)> {
    let failure = private_wait_failure(module, entry, boundary)?;
    (failure.cause == ProgressFailure::Deadlock).then_some((failure.machine, failure.operation))
}
