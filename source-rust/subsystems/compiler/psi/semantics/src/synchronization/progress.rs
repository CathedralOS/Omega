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
}

impl ProgressFailure {
    pub fn detail(self) -> &'static str {
        match self {
            Self::Deadlock => {
                "the wait retries on an unchanged private atomic cell and no other participant can release it"
            }
            Self::Unproved => {
                "the initial value may keep the wait blocked and no other participant can change this private cell"
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
            ProgressFailure::Unproved | ProgressFailure::UnprovedExternal => {
                "cannot prove synchronization progress"
            }
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
        let Some(candidate) = ScalarFact::Literal(self.value).known_integer() else {
            return false;
        };
        // When every value except the literal blocks, a checked equality to
        // that literal has already been substituted in entry_scalars.
        self.matches(self.value)
            && crate::inhabitation::predicates_are_false_with(requires, |id| {
                (id == input.id).then_some(candidate)
            })
    }

    pub(super) fn failure(
        self,
        initial: Option<ScalarFact>,
        update: CellUpdate,
        requires: &[Proposition],
    ) -> Option<ProgressFailure> {
        if let Some(ScalarFact::Literal(value)) = initial {
            return (self.matches(value) && self.matches(update.apply(value)))
                .then_some(ProgressFailure::Deadlock);
        }
        // If the test blocks only on one word, the first update determines
        // the next observation even when the starting value is unknown.
        let only_blocked = if self.matches(self.value) {
            Some(self.value)
        } else if let Scalar::Boolean(value) = self.value {
            Some(Scalar::Boolean(!value))
        } else {
            None
        };
        if only_blocked.is_some_and(|value| !self.matches(update.apply(value)))
            || matches!(update, CellUpdate::Replace(value) if !self.matches(value))
            || matches!(initial, Some(ScalarFact::EntryInput(input)) if self.input_enabled(input, requires))
        {
            return None;
        }
        Some(ProgressFailure::Unproved)
    }
}
