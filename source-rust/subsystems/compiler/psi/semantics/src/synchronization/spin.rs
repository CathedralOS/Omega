//! Derive the condition under which a checked activation must retry. A single
//! cell can change as part of the observation. Read-only waits may combine
//! several observations along different paths to the retry edge.

mod cases;
mod condition;
mod observations;
use condition::Comparison;
pub(super) use condition::SpinCondition;
pub(super) use observations::Observation;

use super::facts::Facts;
use cases::CaseValue;

use crate::entry_values::TerminalScalarValue as Scalar;
use semantic_vocabulary::{BlockId, OperationId, PlaceId, ScalarType, ValueId};
use terminal_psi::{
    AtomicAccessEvent, Operation, OperationKind, OperationResult, StructuralArgument,
    StructuralPathSegment, TargetMachineCallable, TerminalMachine, TerminalModule, Terminator,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpinWait(pub(super) WaitRule);

impl SpinWait {
    /// A retry can release itself even when the caller has no storage facts.
    pub(super) fn can_block_without_inputs(&self) -> bool {
        match &self.0 {
            WaitRule::Cell(cell) => cell.while_value.failure(None, cell.update, &[]).is_some(),
            WaitRule::Observations(predicate) => {
                predicate.evaluate(|_, _| (Observation::Unproved, None::<()>))
                    != Observation::Known(false)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum WaitRule {
    Cell(CellWait),
    Observations(observations::Predicate),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CellWait {
    pub(super) subject: Subject,
    pub(super) while_value: SpinCondition,
    pub(super) update: CellUpdate,
}

/// These transfers are idempotent. Once a private retry observes the result
/// of its own update, all later iterations see those same bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CellUpdate {
    Observe,
    Replace(Scalar),
    BitwiseAnd(Scalar),
    BitwiseOr(Scalar),
    CompareExchange {
        expected: Scalar,
        replacement: Scalar,
    },
}

impl CellUpdate {
    pub(super) fn apply(self, prior: Scalar) -> Option<Scalar> {
        Some(match self {
            Self::Observe => prior,
            Self::Replace(value) => value,
            Self::BitwiseAnd(mask) => condition::masked(prior, Some(mask))?,
            Self::BitwiseOr(mask) => crate::atomic_read_modify_write(
                terminal_psi::AtomicReadModifyWrite::FetchOr,
                prior,
                mask,
            )?,
            Self::CompareExchange {
                expected,
                replacement,
            } => {
                if prior == expected {
                    replacement
                } else {
                    prior
                }
            }
        })
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
    Masked(Scalar),
    Test(SpinCondition),
}

impl Word {
    fn literal(self) -> Option<Scalar> {
        match self {
            Self::Literal(v) => Some(v),
            _ => None,
        }
    }

    fn bitwise_and(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::Observed, Self::Literal(mask)) | (Self::Literal(mask), Self::Observed) => {
                Some(Self::Masked(mask))
            }
            (Self::Masked(prior), Self::Literal(mask))
            | (Self::Literal(mask), Self::Masked(prior)) => {
                Some(Self::Masked(condition::masked(prior, Some(mask))?))
            }
            (Self::Literal(left), Self::Literal(right)) => {
                Some(Self::Literal(condition::masked(left, Some(right))?))
            }
            _ => None,
        }
    }

    fn compare(self, comparison: Comparison, positive: bool) -> Option<Self> {
        let condition = SpinCondition {
            comparison,
            positive,
        };
        match self {
            Self::Observed => Some(Self::Test(condition)),
            Self::Masked(mask) => Some(Self::Test(SpinCondition {
                comparison: Comparison {
                    mask: Some(mask),
                    ..comparison
                },
                positive,
            })),
            Self::Test(mut test) if comparison.order.is_eq() => {
                let Scalar::Boolean(value) = comparison.pivot else {
                    return None;
                };
                test.positive = test.positive == (value == positive);
                Some(Self::Test(test))
            }
            Self::Test(_) => None,
            Self::Literal(value) => Some(Self::Literal(Scalar::Boolean(condition.matches(value)?))),
        }
    }

    fn equal(self, value: Scalar) -> Option<Self> {
        self.compare(Comparison::equal(value), true)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Subject {
    pub(super) parameter: usize,
    pub(super) path: Vec<StructuralPathSegment>,
}

pub(super) struct ForwardedWait {
    pub(super) callee: TargetMachineCallable,
    borrowed: Vec<Subject>,
}

impl SpinWait {
    /// Restate every observed place in the consumer's coordinates. A partial
    /// translation cannot establish the original retry condition.
    pub(super) fn map_subjects(
        mut self,
        mut map: impl FnMut(&mut Subject) -> Option<()>,
    ) -> Option<Self> {
        match &mut self.0 {
            WaitRule::Cell(cell) => map(&mut cell.subject)?,
            WaitRule::Observations(predicate) => {
                for subject in predicate.inputs() {
                    map(subject)?;
                }
            }
        }
        Some(self)
    }
}

impl ForwardedWait {
    /// Substitute the actual borrowed argument for each formal in the predicate.
    pub(super) fn apply(&self, spin: SpinWait) -> Option<SpinWait> {
        spin.map_subjects(|subject| {
            let supplied = self.borrowed.get(subject.parameter)?;
            let mut projected = supplied.path.clone();
            projected.append(&mut subject.path);
            subject.path = projected;
            subject.parameter = supplied.parameter;
            Some(())
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Read {
    site: OperationId,
    cell: Subject,
    update: CellUpdate,
}

#[derive(Default)]
struct Trace {
    cases: Facts<PlaceId, CaseValue>,
    subjects: Subjects,
    forwarded: Option<ForwardedWait>,
    values: Facts<ValueId, Word>,
    read: Option<Read>,
    tested: Option<Comparison>,
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

#[derive(Default)]
struct Subjects {
    places: Facts<PlaceId, Subject>,
    borrows: Facts<PlaceId, Subject>,
}

impl Subjects {
    fn at_entry(machine: &TerminalMachine) -> Self {
        let mut state = Self::default();
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
        state
    }

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
}

/// Only a checked empty unit entry can disappear from a wait predicate.
fn inert_unit_call(module: &TerminalModule, operation: &Operation) -> bool {
    let Some(call) = operation.declared_invocation() else {
        return false;
    };
    if !matches!(operation.result, OperationResult::Unit)
        || !call.claim_transfers.is_empty()
        || !call.completion_receipts.is_empty()
        || !call.crash_continuations.is_empty()
    {
        return false;
    }
    if let TargetMachineCallable::Machine(callee) = call.callee
        && call.structural_arguments.is_empty()
        && call.arguments.is_empty()
        && let Some(callee) = module.machine(callee)
        && let Some(entry) = callee.block(&callee.entry)
        && entry.operations.is_empty()
        && matches!(&entry.terminator, Terminator::ReturnUnit { cleanup_actions, .. } if cleanup_actions.is_empty())
    {
        return true;
    }
    false
}

impl Trace {
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
            if inert_unit_call(module, operation) {
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
                    self.subjects.subject(actual.place, &actual.path)
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
                let supplied = self.subjects.subject(source.place, &source.path)?;
                bind(
                    &mut self.subjects.borrows,
                    operation.result.as_structural()?.place,
                    supplied,
                )?;
                return Some(false);
            }
            OperationKind::ReleaseReference { source, path } if path.is_empty() => {
                self.subjects.borrows.remove(source)?;
                return Some(false);
            }
            _ => {}
        }
        if let Some(result) = operation.result.as_structural() {
            let case = CaseValue::from_operation(&operation.kind, &self.cases)?;
            if !result.is_unrestricted_plain()
                || !case.has_known_fields(|id| self.scalar(id).is_some())
            {
                return None;
            }
            bind(&mut self.cases, result.place, case)?;
            return Some(false);
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
                    let update = match event {
                        AtomicAccessEvent::Load { .. } => CellUpdate::Observe,
                        AtomicAccessEvent::Swap { value, .. } => {
                            CellUpdate::Replace(self.scalar(*value)?.literal()?)
                        }
                        AtomicAccessEvent::ReadModifyWrite {
                            operation, operand, ..
                        } => {
                            let mask = self.scalar(*operand)?.literal()?;
                            match operation {
                                terminal_psi::AtomicReadModifyWrite::FetchAnd => {
                                    CellUpdate::BitwiseAnd(mask)
                                }
                                terminal_psi::AtomicReadModifyWrite::FetchOr => {
                                    CellUpdate::BitwiseOr(mask)
                                }
                                _ => return None,
                            }
                        }
                        AtomicAccessEvent::CompareExchange {
                            expected,
                            replacement,
                            ..
                        } => CellUpdate::CompareExchange {
                            expected: self.scalar(*expected)?.literal()?,
                            replacement: self.scalar(*replacement)?.literal()?,
                        },
                        _ => return None,
                    };
                    let read = Read {
                        site: operation.id,
                        cell: self.subjects.subject(*place, path)?,
                        update,
                    };
                    if self.read.as_ref().is_some_and(|seen| seen != &read) {
                        return None;
                    }
                    self.read = Some(read);
                    reads = true;
                    Word::Observed
                }
                OperationKind::StructuralCaseMembership { source, path, case }
                    if path.is_empty() =>
                {
                    Word::Literal(Scalar::Boolean(self.cases.get(source)?.is(*case)))
                }
                OperationKind::IntegerBitwiseAnd { left, right } => {
                    self.scalar(*left)?.bitwise_and(self.scalar(*right)?)?
                }
                OperationKind::BooleanNot { operand } => {
                    self.scalar(*operand)?.equal(Scalar::Boolean(false))?
                }
                OperationKind::IntegerEqual { left, right }
                | OperationKind::BooleanEqual { left, right }
                | OperationKind::IntegerLessThan { left, right }
                | OperationKind::IntegerLessOrEqual { left, right } => {
                    let (_, _, order, positive) = Comparison::operands(&operation.kind)?;
                    let left = self.scalar(*left)?;
                    let right = self.scalar(*right)?;
                    if let Some(pivot) = right.literal() {
                        left.compare(
                            Comparison {
                                pivot,
                                order,
                                mask: None,
                            },
                            positive,
                        )?
                    } else {
                        right.compare(
                            Comparison {
                                pivot: left.literal()?,
                                order: order.reverse(),
                                mask: None,
                            },
                            positive,
                        )?
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
            Word::Literal(_) | Word::Masked(_) => return None,
            Word::Observed => SpinCondition {
                comparison: Comparison::equal(Scalar::Boolean(true)),
                positive: true,
            },
            Word::Test(condition) => condition,
        };
        if self
            .tested
            .is_some_and(|expected| expected != condition.comparison)
        {
            return None;
        }
        self.tested = Some(condition.comparison);
        Some(equal_case == condition.positive)
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
            if formal.access == terminal_psi::StructuralAccess::Owned
                && actual.access == terminal_psi::StructuralAccess::Owned
                && actual.path.is_empty()
            {
                let supplied = self.cases.get(&actual.place)?.clone();
                bind(&mut self.cases, formal.place, supplied)?;
                continue;
            }
            if !matches!(formal.access, terminal_psi::reading_borrow_pair_pattern!())
                || !matches!(actual.access, terminal_psi::reading_borrow_pair_pattern!())
            {
                return None;
            }
            let supplied = self.subjects.subject(actual.place, &actual.path)?;
            bind(&mut self.subjects.places, formal.place, supplied)?;
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
    let mut state = Trace {
        subjects: Subjects::at_entry(machine),
        ..Trace::default()
    };
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
        let fields: Vec<ValueId>;
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
            Terminator::StructuralCase {
                source,
                path,
                cases,
            } if path.is_empty() => {
                let known = state.cases.get(source)?;
                let edge = cases.iter().find(|edge| known.is(edge.case))?;
                fields = known.payload(&edge.payload_fields)?;
                (
                    &edge.target,
                    &fields,
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
    spin_observation(module, machine).map(|(_, wait)| wait)
}

/// The operation belongs to this defining body; only the resource predicate
/// is substituted into another machine or published provider.
pub(super) fn spin_observation(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Option<(OperationId, SpinWait)> {
    if !machine
        .operations()
        .any(|op| matches!(op.kind, OperationKind::AtomicAccess { .. }))
    {
        return None;
    }
    single_cell_wait(module, machine)
        .map(|(site, cell)| (site, SpinWait(WaitRule::Cell(cell))))
        .or_else(|| {
            observations::derive(module, machine)
                .map(|(site, predicate)| (site, SpinWait(WaitRule::Observations(predicate))))
        })
}

fn single_cell_wait(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Option<(OperationId, CellWait)> {
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
    let comparison = equal_trace.tested?;
    Some((
        read.site,
        CellWait {
            subject: read.cell,
            update: read.update,
            while_value: SpinCondition {
                comparison,
                positive: equal == Completion::Retries,
            },
        },
    ))
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
