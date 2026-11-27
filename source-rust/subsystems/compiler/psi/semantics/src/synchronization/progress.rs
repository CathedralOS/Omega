//! A selected wait may need a fact about the scalar written to a private
//! cell. Ordinary scalar arguments preserve the supplying entry's identity
//! across calls; an unrelated machine's local value number is never evidence.

use super::facts::Facts;
use super::spin::{CellUpdate, SpinCondition};
use crate::entry_values::TerminalScalarValue as Scalar;
use semantic_vocabulary::{
    IntegerValue, MachineId, OperationId, Proposition, ScalarTerm, ScalarType,
};
use terminal_psi::ValueDeclaration;

/// An independent export can leave facts about storage received from its
/// caller open. An executable must establish them before entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressScope {
    OpenInterface,
    Executable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressFailure {
    /// Every retry observes a value that keeps the wait blocked.
    Deadlock,
    /// A private cell has no other writer and its first observation could
    /// block. No available fact excludes the retry.
    Unproved,
    /// The activation borrows this resource and has no admitted enabler.
    UnprovedExternal,
    /// This receiving call did not establish the selected supplier's progress condition.
    UnprovedImport,
    /// A preceding opaque call left the inputs of a reachable wait unproved.
    UnprovedContinuation,
}

impl ProgressFailure {
    pub fn detail(self) -> &'static str {
        match self {
            Self::Deadlock => {
                "the wait keeps retrying on unchanged private atomic storage and no other participant can release it"
            }
            Self::Unproved => {
                "the observed values may keep the wait blocked and no other participant can change the deciding private storage"
            }
            Self::UnprovedContinuation => {
                "a preceding provider call leaves the later wait without checked storage or enabling evidence"
            }
            Self::UnprovedImport => {
                "the receiving call has no checked discharge of the selected supplier's progress condition"
            }
            Self::UnprovedExternal => {
                "the wait depends on supplied storage and no checked or admitted action establishes progress"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivateWaitFailure {
    pub machine: MachineId,
    pub operation: OperationId,
    pub cause: ProgressFailure,
}

impl std::fmt::Display for PrivateWaitFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let header = match self.cause {
            ProgressFailure::Deadlock => "synchronization deadlock",
            ProgressFailure::Unproved
            | ProgressFailure::UnprovedExternal
            | ProgressFailure::UnprovedImport
            | ProgressFailure::UnprovedContinuation => "cannot prove synchronization progress",
        };
        write!(
            f,
            "{header} in machine {} at operation {}: {}",
            self.machine.get(),
            self.operation.get(),
            self.cause.detail()
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScalarFact {
    Literal(Scalar),
    EntryInput(ValueDeclaration),
}

impl ScalarFact {
    pub(super) fn literal(self) -> Option<Scalar> {
        match self {
            Self::Literal(value) => Some(value),
            Self::EntryInput(_) => None,
        }
    }

    pub fn ty(self) -> ScalarType {
        match self {
            Self::Literal(value) => value.scalar_type(),
            Self::EntryInput(input) => input.scalar_type,
        }
    }

    pub fn from_integer(ty: ScalarType, value: IntegerValue) -> Option<Self> {
        Some(Self::Literal(match (ty, value) {
            (ScalarType::Boolean, IntegerValue::Unsigned(bit @ 0..=1)) => Scalar::Boolean(bit == 1),
            (ScalarType::Integer(scalar_type), value) => Scalar::Integer { scalar_type, value },
            _ => return None,
        }))
    }

    pub(super) fn index(self) -> Option<u64> {
        match self.literal()? {
            Scalar::Integer {
                value: IntegerValue::Unsigned(raw),
                ..
            } => u64::try_from(raw).ok(),
            Scalar::Integer {
                value: IntegerValue::Signed(raw),
                ..
            } => u64::try_from(raw).ok(),
            _ => None,
        }
    }

    pub fn known_integer(self) -> Option<(ScalarType, IntegerValue)> {
        match self {
            Self::Literal(Scalar::Boolean(v)) => {
                Some((ScalarType::Boolean, IntegerValue::Unsigned(u128::from(v))))
            }
            Self::Literal(Scalar::Integer { scalar_type, value }) => {
                Some((ScalarType::Integer(scalar_type), value))
            }
            _ => None,
        }
    }
}

/// Seed direct equalities on actual entry formals. This also makes ordinary
/// checked scalar evaluation available for computations on constrained inputs.
pub(super) fn entry_scalars(
    machine: &terminal_psi::TerminalMachine,
) -> Facts<semantic_vocabulary::ValueId, ScalarFact> {
    let mut values = Facts::default();
    for formal in &machine.parameters {
        values.insert(formal.id, ScalarFact::EntryInput(*formal));
    }
    let mut pending: Vec<_> = machine.requires.iter().collect();
    while let Some(clause) = pending.pop() {
        match clause {
            Proposition::Conjunction(clauses) => pending.extend(clauses),
            Proposition::Equal(left, right) => {
                for (subject, expression) in [(left, right), (right, left)] {
                    let ScalarTerm::Value { id, scalar_type } = subject else {
                        continue;
                    };
                    if !matches!(values.get(id), Some(ScalarFact::EntryInput(input)) if *scalar_type == input.scalar_type)
                    {
                        continue;
                    }
                    if let Some((ty, value)) =
                        crate::inhabitation::known_scalar_value_with(expression, |_| None)
                        && ty == *scalar_type
                        && let Some(fact) = ScalarFact::from_integer(ty, value)
                    {
                        values.insert(*id, fact);
                    }
                }
            }
            _ => {}
        }
    }
    values
}

impl SpinCondition {
    /// Is the input known to avoid retry under the selected entry contract?
    /// Substituting the retry literal into an ordinary checked requirement
    /// discharges common bounds and inequalities without enumerating inputs.
    fn input_enabled(self, input: ValueDeclaration, requires: &[Proposition]) -> bool {
        if self.excluded_by(input, requires) {
            return true;
        }
        let Some(candidate) = self
            .sole_blocked_value()
            .and_then(|v| ScalarFact::Literal(v).known_integer())
        else {
            return false;
        };
        // When every value except the literal blocks, a checked equality to
        // that literal has already been substituted in entry_scalars.
        crate::inhabitation::predicates_are_false_with(requires, |id| {
            (id == input.id).then_some(candidate)
        })
    }

    pub(super) fn failure(
        self,
        initial: Option<ScalarFact>,
        pivot_input: Option<ValueDeclaration>,
        update: Option<CellUpdate<ScalarFact>>,
        requires: &[Proposition],
    ) -> Option<ProgressFailure> {
        // An entry argument is equal to itself even when its bits are unknown.
        let blocks = |value| match value {
            ScalarFact::Literal(value) => self.matches(value),
            ScalarFact::EntryInput(input)
                if Some(input) == pivot_input && self.comparison.mask.is_none() =>
            {
                Some(self.accepts(std::cmp::Ordering::Equal))
            }
            _ => None,
        };
        if let Some(initial) = initial
            && let Some(first) = blocks(initial)
        {
            if !first {
                return None;
            }
            if let Some(retries) = update.and_then(|u| u.next(initial)).and_then(blocks) {
                return retries.then_some(ProgressFailure::Deadlock);
            }
        }
        // A unique blocked word allows the first retry to determine the next
        // observation even when the resident at entry is unknown.
        let only_blocked = self.sole_blocked_value().map(ScalarFact::Literal);
        if self.excludes_every_word()
            || only_blocked.is_some_and(|prior| {
                update.and_then(|u| u.next(prior)).and_then(blocks) == Some(false)
            })
            || matches!(update, Some(CellUpdate::Replace(value)) if blocks(value) == Some(false))
            || matches!(initial, Some(ScalarFact::EntryInput(input)) if self.input_enabled(input, requires))
        {
            return None;
        }
        Some(ProgressFailure::Unproved)
    }
}
