//! Derive the condition under which a checked activation must retry. A single
//! cell can change as part of the observation. Read-only waits may combine
//! several observations along different paths to the retry edge.

mod cases;
mod condition;
mod observations;
mod orbit;
pub(super) use orbit::{Orbit, Trip};
mod subject;
use subject::Subjects;
pub(super) use subject::{Subject, fixed_extent, type_path};
mod update;
pub(super) use condition::Comparison;
pub(super) use condition::Operand;
pub(super) use condition::SpinCondition;
pub(super) use observations::Observation;
pub(super) use update::{AtomicWordUpdate, CellUpdate};

use super::facts::Facts;
use cases::CaseValue;
use language_core::atomic::AtomicCompareExchangeOutcomeCase as Attempt;

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
            WaitRule::Cell(cell) => {
                let update = cell
                    .update
                    .try_map(|operand| operand.literal().map(super::progress::ScalarFact::Literal));
                cell.while_value
                    .failure(None, None, update, &Default::default())
                    .is_some()
            }
            WaitRule::Periodic(cell) => cell
                .while_value
                .failure(None, None, Some(CellUpdate::Observe), &Default::default())
                .is_some(),
            WaitRule::Observations(predicate) => {
                predicate.evaluate(
                    |_, _| (Observation::Unproved, None::<((), SpinCondition)>),
                    |_, _| Err(Observation::Unproved),
                    Operand::literal,
                ) != Observation::Known(false)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum WaitRule {
    Cell(CellWait),
    Periodic(CellWait<Orbit<Operand>>),
    Observations(observations::Predicate),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CellWait<U = CellUpdate<Operand>> {
    pub(super) subject: Subject,
    pub(super) while_value: SpinCondition,
    pub(super) update: U,
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
    Masked(Operand),
    Argument(Operand),
    Test(SpinCondition),
}

impl From<Operand> for Word {
    fn from(operand: Operand) -> Self {
        match operand {
            Operand::Literal(value) => Self::Literal(value),
            Operand::Parameter { .. } => Self::Argument(operand),
        }
    }
}

impl Word {
    fn operand(self) -> Option<Operand> {
        match self {
            Self::Literal(value) => Some(Operand::Literal(value)),
            Self::Argument(value) => Some(value),
            _ => None,
        }
    }

    fn bitwise_and(self, other: Self) -> Option<Self> {
        let (word, mask) = if let Some(operand) = other.operand() {
            (self, operand)
        } else {
            (other, self.operand()?)
        };
        match word {
            Self::Observed => Some(Self::Masked(mask)),
            Self::Masked(prior) => Some(Self::Masked(mask.intersect(Some(prior))?)),
            Self::Literal(left) => Some(Self::Literal(condition::masked(
                left,
                Some(mask.literal()?),
            )?)),
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
                let Scalar::Boolean(value) = comparison.pivot.literal()? else {
                    return None;
                };
                test.positive = test.positive == (value == positive);
                Some(Self::Test(test))
            }
            Self::Test(_) | Self::Argument(_) => None,
            Self::Literal(value) => Some(Self::Literal(Scalar::Boolean(condition.matches(value)?))),
        }
    }

    fn equal(self, value: Scalar) -> Option<Self> {
        self.compare(Comparison::equal(value), true)
    }
}

pub(super) struct ForwardedWait {
    pub(super) callee: TargetMachineCallable,
    borrowed: Vec<Subject>,
    scalars: Vec<Option<Operand>>,
}

impl SpinWait {
    pub(super) fn single_cell(&self) -> Option<&CellWait> {
        match &self.0 {
            WaitRule::Cell(cell) => Some(cell),
            _ => None,
        }
    }

    /// Restate every observed place in the consumer's coordinates. A partial
    /// translation cannot establish the original retry condition.
    pub(super) fn map_subjects(
        mut self,
        mut map: impl FnMut(&mut Subject) -> Option<()>,
    ) -> Option<Self> {
        match &mut self.0 {
            WaitRule::Cell(cell) => map(&mut cell.subject)?,
            WaitRule::Periodic(cell) => map(&mut cell.subject)?,
            WaitRule::Observations(predicate) => {
                for subject in predicate.subjects() {
                    map(subject)?;
                }
            }
        }
        Some(self)
    }
}

impl ForwardedWait {
    /// Substitute the actual borrowed argument for each formal in the predicate.
    pub(super) fn apply(&self, mut spin: SpinWait) -> Option<SpinWait> {
        let remap = |operand| self.scalar(operand);
        match &mut spin.0 {
            WaitRule::Cell(cell) => {
                cell.while_value.comparison.substitute(remap)?;
                cell.update = cell.update.try_map(remap)?;
            }
            WaitRule::Periodic(cell) => {
                cell.while_value.comparison.substitute(remap)?;
                cell.update = cell.update.try_map(remap)?;
            }
            WaitRule::Observations(predicate) => {
                predicate.substitute(remap)?;
            }
        }
        spin.map_subjects(|subject| self.apply_subject(subject))
    }

    pub(super) fn apply_subject(&self, subject: &mut Subject) -> Option<()> {
        let supplied = self.borrowed.get(subject.parameter)?;
        subject.forwarded(supplied, |operand| self.scalar(operand))
    }

    fn scalar(&self, operand: Operand) -> Option<Operand> {
        match operand {
            Operand::Literal(_) => Some(operand),
            Operand::Parameter {
                position,
                scalar_type,
            } => self
                .scalars
                .get(position)
                .copied()
                .flatten()
                .filter(|actual| actual.scalar_type() == scalar_type),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Read {
    site: OperationId,
    cell: Subject,
    update: AtomicWordUpdate,
    single_attempt: bool,
}

#[derive(Default)]
struct Trace {
    cases: Facts<PlaceId, CaseValue>,
    vacant: Facts<PlaceId, ()>,
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

    fn subject(
        &self,
        module: &TerminalModule,
        machine: &TerminalMachine,
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Option<Subject> {
        self.subjects.subject(module, machine, place, path, |id| {
            self.scalar(id)?.operand()
        })
    }

    fn operation(
        &mut self,
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
        outcome: Attempt,
    ) -> Option<bool> {
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
                    self.subject(module, machine, actual.place, &actual.path)
                })
                .collect::<Option<_>>()?;
            self.forwarded = Some(ForwardedWait {
                callee: call.callee,
                borrowed,
                scalars: call
                    .arguments
                    .iter()
                    .map(|id| self.scalar(*id).and_then(Word::operand))
                    .collect(),
            });
            return Some(false);
        }
        match &operation.kind {
            OperationKind::EstablishReference {
                source,
                storage: None,
            } => {
                let supplied = self.subject(module, machine, source.place, &source.path)?;
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
        if let OperationKind::StoreStructuralField {
            disposition: terminal_psi::StructuralStoreDisposition::Vacant,
            destination,
            path,
            field: None,
            value,
        } = &operation.kind
        {
            // A move into a fresh local is a change of home for this exact
            // case. It cannot write through the caller's borrowed resource.
            if !path.is_empty()
                || !value.path.is_empty()
                || value.access != terminal_psi::StructuralAccess::Owned
            {
                return None;
            }
            self.vacant.remove(destination)?;
            let case = self.cases.remove(&value.place)?;
            bind(&mut self.cases, *destination, case)?;
            return Some(false);
        }
        if let Some(result) = operation.result.as_structural() {
            return self.case_result(module, machine, operation, result, outcome);
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
                    let update =
                        AtomicWordUpdate::from_observation(event, |id| self.scalar(id)?.operand())?;
                    self.observe(
                        operation.id,
                        self.subject(module, machine, *place, path)?,
                        update,
                        false,
                    )?;
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
                    if let Some(pivot) = right.operand() {
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
                                pivot: left.operand()?,
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

    fn observe(
        &mut self,
        site: OperationId,
        cell: Subject,
        update: AtomicWordUpdate,
        single_attempt: bool,
    ) -> Option<()> {
        let read = Read {
            site,
            cell,
            update,
            single_attempt,
        };
        if self.read.as_ref().is_some_and(|seen| *seen != read) {
            return None;
        }
        self.read = Some(read);
        Some(())
    }

    fn test(&mut self, value: Comparison) -> Option<()> {
        if self.tested.is_some_and(|prior| prior != value) {
            return None;
        }
        self.tested = Some(value);
        Some(())
    }

    fn branch(&mut self, condition: ValueId, equal_case: bool) -> Option<bool> {
        let condition = match self.scalar(condition)? {
            Word::Literal(Scalar::Boolean(known)) => return Some(known),
            Word::Literal(_) | Word::Masked(_) | Word::Argument(_) => return None,
            Word::Observed => SpinCondition {
                comparison: Comparison::equal(Scalar::Boolean(true)),
                positive: true,
            },
            Word::Test(condition) => condition,
        };
        self.test(condition.comparison)?;
        Some(equal_case == condition.positive)
    }

    fn transfer(
        &mut self,
        module: &TerminalModule,
        machine: &TerminalMachine,
        dest: &terminal_psi::Block,
        arguments: Arguments<'_>,
        places: &[StructuralArgument],
    ) -> Option<()> {
        if arguments.len() != dest.parameters.len()
            || places.len() != dest.structural_parameters.len()
        {
            return None;
        }
        for (index, formal) in dest.parameters.iter().enumerate() {
            let supplied = match arguments {
                Arguments::Values(ids) => self.scalar(ids[index])?,
                Arguments::Case(place, fields) => self.cases.get(&place)?.payload(fields[index])?,
            };
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
            let supplied = self.subject(module, machine, actual.place, &actual.path)?;
            bind(&mut self.subjects.places, formal.place, supplied)?;
        }
        Some(())
    }
}

enum Arguments<'a> {
    Values(&'a [ValueId]),
    Case(PlaceId, &'a [semantic_vocabulary::StructuralFieldId]),
}

impl Arguments<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Values(args) => args.len(),
            Self::Case(_, fields) => fields.len(),
        }
    }
}

#[derive(PartialEq, Eq)]
enum Completion {
    Returns,
    Retries,
}

/// Follow one equality outcome. On a single-attempt operation the matched
/// observation has two distinct paths, committed and uncommitted.
fn trace(
    module: &TerminalModule,
    machine: &TerminalMachine,
    outcome: Attempt,
) -> Option<(Trace, Completion)> {
    let mut state = Trace {
        subjects: Subjects::at_entry(machine),
        values: Operand::formals(machine, Word::Argument),
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
            observations += usize::from(state.operation(module, machine, operation, outcome)?);
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
            } => (
                target,
                Arguments::Values(arguments),
                structural_arguments,
                cleanup_actions,
            ),
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let edge = if state.branch(*condition, outcome != Attempt::Mismatched)? {
                    when_true
                } else {
                    when_false
                };
                (
                    &edge.target,
                    Arguments::Values(&edge.arguments),
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
                (
                    &edge.target,
                    Arguments::Case(*source, &edge.payload_fields),
                    &edge.structural_arguments,
                    &edge.cleanup_actions,
                )
            }
            _ => return None,
        };
        // A case edge captures scalar payloads, then discards the matched
        // affine temporary. Its construction established a scalar-only value
        // with no nominal cleanup or retained claim.
        let discard = match cleanup.as_slice() {
            [] => None,
            [terminal_psi::TerminalAffineCleanupAction::DiscardRoot(root)]
                if matches!(arguments, Arguments::Case(source, _) if source == *root)
                    && places.is_empty() =>
            {
                Some(*root)
            }
            _ => return None,
        };
        block = machine.block(next)?;
        state.transfer(module, machine, block, arguments, places)?;
        if let Some(root) = discard {
            state.cases.remove(&root)?;
        }
    };
    Some((state, completion))
}

pub fn spin_wait(module: &TerminalModule, machine: &TerminalMachine) -> Option<SpinWait> {
    spin_observation(module, machine).map(|(_, wait)| wait)
}

/// A local atomic retry both observes a cell and traverses a control cycle.
/// Every cycle contains an edge to an equal or lower block ID, regardless of
/// allocation order. A backwards edge alone does not establish a wait.
pub(super) fn may_spin(machine: &TerminalMachine) -> bool {
    machine.blocks.iter().any(|block| {
        block
            .terminator
            .successor_edges()
            .iter()
            .any(|(_, target)| *target <= block.id)
    }) && machine.operations().any(|op| {
        matches!(op.kind, OperationKind::AtomicAccess { event, .. }
            if !matches!(event, AtomicAccessEvent::Store { .. }))
    })
}

/// The operation belongs to this defining body; only the resource predicate
/// is substituted into another machine or published provider.
pub(super) fn spin_observation(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Option<(OperationId, SpinWait)> {
    if !may_spin(machine) {
        return None;
    }
    single_cell_wait(module, machine)
        .map(|(site, rule)| (site, SpinWait(rule)))
        .or_else(|| {
            observations::derive(module, machine)
                .map(|(site, predicate)| (site, SpinWait(WaitRule::Observations(predicate))))
        })
}

fn single_cell_wait(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Option<(OperationId, WaitRule)> {
    let (equal_trace, equal) = trace(module, machine, Attempt::Exchanged)?;
    let (different_trace, different) = trace(module, machine, Attempt::Mismatched)?;
    if equal_trace.forwarded.is_some()
        || different_trace.forwarded.is_some()
        || equal_trace.read != different_trace.read
        || equal_trace.tested != different_trace.tested
        || equal == different
    {
        return None;
    }
    let read = equal_trace.read?;
    if read.single_attempt {
        // Failure to commit may retry without another participant. It cannot
        // change the private resident, and a matching cell still permits a
        // commit. Only the mismatching resident can permanently block this loop.
        let (without_commit, completion) = trace(module, machine, Attempt::Uncommitted)?;
        if equal != Completion::Returns
            || completion != Completion::Retries
            || without_commit.read.as_ref() != Some(&read)
            || without_commit.tested != equal_trace.tested
            || without_commit.forwarded.is_some()
        {
            return None;
        }
    }
    let comparison = equal_trace.tested?;
    let subject = read.cell;
    let while_value = SpinCondition {
        comparison,
        positive: equal == Completion::Retries,
    };
    let rule = match read.update {
        AtomicWordUpdate::Stable(update) => WaitRule::Cell(CellWait {
            subject,
            while_value,
            update,
        }),
        AtomicWordUpdate::Periodic(update) => WaitRule::Periodic(CellWait {
            subject,
            while_value,
            update,
        }),
    };
    Some((read.site, rule))
}

/// A transparent unit wrapper lends its own arguments to one named machine and
/// returns. Its wait depends on the exact implementation selected for that call.
pub(super) fn forwarded_wait(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Option<ForwardedWait> {
    let (state, completion) = trace(module, machine, Attempt::Exchanged)?;
    if completion != Completion::Returns || state.read.is_some() {
        return None;
    }
    state.forwarded
}
