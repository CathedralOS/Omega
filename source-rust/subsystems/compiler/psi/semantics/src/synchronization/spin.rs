//! Derive a retry contract from one observed atomic cell. Follow both outcomes
//! of its test in the lowered control flow. The waiting path must revisit the
//! access, the other must return, and neither may have any unaccounted effect.

use super::facts::Facts;

use crate::entry_values::TerminalScalarValue as Scalar;
use semantic_vocabulary::{BlockId, OperationId, PlaceId, ScalarType, ValueId};
use terminal_psi::{
    AtomicAccessEvent, Operation, OperationKind, OperationResult, StructuralArgument,
    StructuralPathSegment, TargetMachineCallable, TerminalMachine, TerminalModule, Terminator,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpinWait {
    pub(super) parameter: usize,
    pub(super) path: Vec<StructuralPathSegment>,
    pub(super) while_value: SpinCondition,
    pub(super) writes: Option<Scalar>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SpinCondition {
    pub(super) value: Scalar,
    equal: bool,
}

impl SpinCondition {
    pub(super) fn matches(self, observed: Scalar) -> bool {
        (observed == self.value) == self.equal
    }
}

pub(super) fn constant(operation: &Operation) -> Option<Scalar> {
    match operation.kind {
        OperationKind::IntegerConstant { value } => {
            let ScalarType::Integer(scalar_type) = operation.result.scalar()?.scalar_type else {
                return None;
            };
            Some(Scalar::Integer { scalar_type, value })
        }
        OperationKind::BooleanConstant { value } => Some(Scalar::Boolean(value)),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Word {
    Literal(Scalar),
    Observed,
    Test(SpinCondition),
}

impl Word {
    fn literal(self) -> Option<Scalar> {
        match self {
            Self::Literal(v) => Some(v),
            _ => None,
        }
    }

    fn equal(self, value: Scalar) -> Option<Self> {
        match self {
            Self::Observed => Some(Self::Test(SpinCondition { value, equal: true })),
            Self::Test(mut condition) => {
                let Scalar::Boolean(value) = value else {
                    return None;
                };
                condition.equal = condition.equal == value;
                Some(Self::Test(condition))
            }
            Self::Literal(left) => Some(Self::Literal(Scalar::Boolean(left == value))),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Subject {
    parameter: usize,
    path: Vec<StructuralPathSegment>,
}

pub(super) struct ForwardedWait {
    pub(super) callee: TargetMachineCallable,
    borrowed: Vec<Subject>,
}

impl ForwardedWait {
    /// Restate the callee's cell over the caller's exact borrowed argument.
    pub(super) fn apply(&self, mut spin: SpinWait) -> Option<SpinWait> {
        let supplied = self.borrowed.get(spin.parameter)?;
        let mut path = supplied.path.clone();
        path.append(&mut spin.path);
        spin.parameter = supplied.parameter;
        spin.path = path;
        Some(spin)
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Read {
    site: OperationId,
    cell: Subject,
    writes: Option<Scalar>,
}

#[derive(Default)]
struct Trace {
    places: Facts<PlaceId, Subject>,
    borrows: Facts<PlaceId, Subject>,
    forwarded: Option<ForwardedWait>,
    values: Facts<ValueId, Word>,
    read: Option<Read>,
    tested: Option<Scalar>,
}

/// A new trip through a block must agree on the original resource and values
/// of its formals. Merely seeing the same instruction twice is insufficient.
fn bind<K: Ord, V: Eq>(bindings: &mut Facts<K, V>, key: K, value: V) -> Option<()> {
    if let Some(prior) = bindings.get(&key) {
        return (*prior == value).then_some(());
    }
    bindings.insert(key, value);
    Some(())
}

impl Trace {
    fn subject(&self, place: PlaceId, path: &[StructuralPathSegment]) -> Option<Subject> {
        let (mut subject, path) = if let Some(borrow) = self.borrows.get(&place) {
            let [StructuralPathSegment::Referent, rest @ ..] = path else {
                return None;
            };
            (borrow.clone(), rest)
        } else {
            (self.places.get(&place)?.clone(), path)
        };
        // Runtime projections and dereferencing a stored pointer can change
        // what the same syntax denotes on successive trips around the loop.
        if !path.iter().all(|part| {
            matches!(
                part,
                StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
            )
        }) {
            return None;
        }
        subject.path.extend_from_slice(path);
        Some(subject)
    }

    fn scalar(&self, value: ValueId) -> Option<Word> {
        self.values.get(&value).copied()
    }

    fn operation(&mut self, module: &TerminalModule, operation: &Operation) -> Option<bool> {
        if let Some(call) = operation.declared_invocation() {
            if !matches!(operation.result, OperationResult::Unit)
                || !call.claim_transfers.is_empty()
                || !call.completion_receipts.is_empty()
                || !call.crash_continuations.is_empty()
            {
                return None;
            }
            // A completion arm can end by calling a local unit machine. Inspect
            // its selected executable entry before treating this call as inert.
            if let TargetMachineCallable::Machine(callee) = call.callee
                && call.structural_arguments.is_empty()
                && call.arguments.is_empty()
                && let Some(callee) = module.machine(callee)
                && let Some(entry) = callee.block(&callee.entry)
                && entry.operations.is_empty()
                && matches!(&entry.terminator, Terminator::ReturnUnit { cleanup_actions, .. } if cleanup_actions.is_empty())
            {
                return Some(false);
            }
            if self.forwarded.is_some() {
                return None;
            }
            let borrowed = call
                .structural_arguments
                .iter()
                .map(|actual| {
                    if !matches!(actual.access, terminal_psi::reading_borrow_pair_pattern!()) {
                        return None;
                    }
                    self.subject(actual.place, &actual.path)
                })
                .collect::<Option<_>>()?;
            self.forwarded = Some(ForwardedWait {
                callee: call.callee,
                borrowed,
            });
            return Some(false);
        }
        match &operation.kind {
            OperationKind::EstablishReference {
                source,
                storage: None,
            } => {
                let supplied = self.subject(source.place, &source.path)?;
                bind(
                    &mut self.borrows,
                    operation.result.as_structural()?.place,
                    supplied,
                )?;
                return Some(false);
            }
            OperationKind::ReleaseReference { source, path } if path.is_empty() => {
                self.borrows.remove(source)?;
                return Some(false);
            }
            _ => {}
        }
        let result = operation.result.scalar()?.id;
        let mut reads = false;
        let value = if let Some(constant) = constant(operation) {
            Word::Literal(constant)
        } else {
            match &operation.kind {
                OperationKind::AtomicAccess {
                    place,
                    path,
                    event,
                    resident_obligation: None,
                } => {
                    let writes = match event {
                        AtomicAccessEvent::Load { .. } => None,
                        AtomicAccessEvent::Swap { value, .. } => {
                            Some(self.scalar(*value)?.literal()?)
                        }
                        _ => return None,
                    };
                    let read = Read {
                        site: operation.id,
                        cell: self.subject(*place, path)?,
                        writes,
                    };
                    if self.read.as_ref().is_some_and(|seen| seen != &read) {
                        return None;
                    }
                    self.read = Some(read);
                    reads = true;
                    Word::Observed
                }
                OperationKind::BooleanNot { operand } => {
                    self.scalar(*operand)?.equal(Scalar::Boolean(false))?
                }
                OperationKind::IntegerEqual { left, right }
                | OperationKind::BooleanEqual { left, right } => {
                    let left = self.scalar(*left)?;
                    let right = self.scalar(*right)?;
                    if let Some(value) = right.literal() {
                        left.equal(value)?
                    } else {
                        right.equal(left.literal()?)?
                    }
                }
                _ => return None,
            }
        };
        bind(&mut self.values, result, value)?;
        Some(reads)
    }

    fn branch(&mut self, condition: ValueId, equal_case: bool) -> Option<bool> {
        let condition = match self.scalar(condition)? {
            Word::Literal(Scalar::Boolean(known)) => return Some(known),
            Word::Literal(_) => return None,
            Word::Observed => SpinCondition {
                value: Scalar::Boolean(true),
                equal: true,
            },
            Word::Test(condition) => condition,
        };
        if self
            .tested
            .is_some_and(|expected| expected != condition.value)
        {
            return None;
        }
        self.tested = Some(condition.value);
        Some(equal_case == condition.equal)
    }

    fn transfer(
        &mut self,
        dest: &terminal_psi::Block,
        arguments: &[ValueId],
        places: &[StructuralArgument],
    ) -> Option<()> {
        if arguments.len() != dest.parameters.len()
            || places.len() != dest.structural_parameters.len()
        {
            return None;
        }
        for (formal, actual) in dest.parameters.iter().zip(arguments) {
            let supplied = self.scalar(*actual)?;
            bind(&mut self.values, formal.id, supplied)?;
        }
        for (formal, actual) in dest.structural_parameters.iter().zip(places) {
            if !matches!(formal.access, terminal_psi::reading_borrow_pair_pattern!())
                || !matches!(actual.access, terminal_psi::reading_borrow_pair_pattern!())
            {
                return None;
            }
            let supplied = self.subject(actual.place, &actual.path)?;
            bind(&mut self.places, formal.place, supplied)?;
        }
        Some(())
    }
}

#[derive(PartialEq, Eq)]
enum Completion {
    Returns,
    Retries,
}

fn trace(
    module: &TerminalModule,
    machine: &TerminalMachine,
    equal_case: bool,
) -> Option<(Trace, Completion)> {
    let mut state = Trace::default();
    for (parameter, formal) in machine.structural_parameters.iter().enumerate() {
        // A borrowed machine formal already denotes the caller's referent.
        // Its first Field projects that storage; a Referent would follow a
        // pointer stored inside it.
        if !matches!(formal.access, terminal_psi::reading_borrow_pair_pattern!()) {
            continue;
        }
        state.places.insert(
            formal.place,
            Subject {
                parameter,
                path: Vec::new(),
            },
        );
    }
    let mut seen = Facts::<BlockId, usize>::default();
    let mut block = machine.block(&machine.entry)?;
    let mut observations = 0;
    let completion = loop {
        if let Some(prior) = seen.insert(block.id, observations) {
            // A permanently spinning branch that never rereads the cell has a
            // different enabling condition.
            if prior == observations {
                return None;
            }
            break Completion::Retries;
        }
        for operation in &block.operations {
            observations += usize::from(state.operation(module, operation)?);
        }
        let (next, arguments, places, cleanup) = match &block.terminator {
            Terminator::ReturnUnit {
                cleanup_actions, ..
            } if cleanup_actions.is_empty() => break Completion::Returns,
            Terminator::Jump {
                target,
                arguments,
                structural_arguments,
                cleanup_actions,
                ..
            } => (target, arguments, structural_arguments, cleanup_actions),
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let edge = if state.branch(*condition, equal_case)? {
                    when_true
                } else {
                    when_false
                };
                (
                    &edge.target,
                    &edge.arguments,
                    &edge.structural_arguments,
                    &edge.cleanup_actions,
                )
            }
            _ => return None,
        };
        if !cleanup.is_empty() {
            return None;
        }
        block = machine.block(next)?;
        state.transfer(block, arguments, places)?;
    };
    Some((state, completion))
}

pub fn spin_wait(module: &TerminalModule, machine: &TerminalMachine) -> Option<SpinWait> {
    if !machine
        .operations()
        .any(|op| matches!(op.kind, OperationKind::AtomicAccess { .. }))
    {
        return None;
    }
    let (equal_trace, equal) = trace(module, machine, true)?;
    let (different_trace, different) = trace(module, machine, false)?;
    if equal_trace.forwarded.is_some()
        || different_trace.forwarded.is_some()
        || equal_trace.read != different_trace.read
        || equal_trace.tested != different_trace.tested
        || equal == different
    {
        return None;
    }
    let read = equal_trace.read?;
    let value = equal_trace.tested?;
    Some(SpinWait {
        parameter: read.cell.parameter,
        path: read.cell.path,
        writes: read.writes,
        while_value: SpinCondition {
            value,
            equal: equal == Completion::Retries,
        },
    })
}

/// A transparent unit wrapper lends its own arguments to one named machine and
/// returns. Its wait depends on the exact implementation selected for that call.
pub(super) fn forwarded_wait(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Option<ForwardedWait> {
    let (state, completion) = trace(module, machine, true)?;
    if completion != Completion::Returns || state.read.is_some() {
        return None;
    }
    state.forwarded
}
