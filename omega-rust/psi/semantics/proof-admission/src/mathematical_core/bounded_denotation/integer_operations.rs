//! Uninterpreted fixed-integer operations keep applicative denotations.
//!
//! Only addition and subtraction carry arithmetic laws; every other
//! operation is an opaque *function* — one assumption constant per
//! constructor and its machine types — applied to the denoted operands.
//! Keeping operands inside the application rather than interning the whole
//! applied term as one unrelated constant is what lets value-equation
//! transport rewrite a bound witness inside `x * s` or `x & s` the same
//! way it already does inside `add x s`. No arithmetic law is added:
//! the judgment still cannot equate two applications unless the endpoint
//! statements already did.
//!
//! Result carriers differ between the two covered families: the
//! arithmetic operations produce `Int`, while the value-level integer
//! comparisons produce `Two` inhabitants — the proposition-level
//! `Id`/`IntLt`/`IntLe` statements are a separate, already-applicative
//! denotation.

use semantic_vocabulary::IntegerType;

use super::{BoundedDenotationError, Declaration, Denotation, ScalarTerm, Term, TermHandle};

/// One fixed-integer operation's identity: constructor and machine types,
/// so `u16` and `u32` multiplications remain distinct opaque functions.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum IntegerOperation {
    BitwiseNot(IntegerType),
    Widen {
        source: IntegerType,
        target: IntegerType,
    },
    ExactCast {
        source: IntegerType,
        target: IntegerType,
    },
    BitwiseAnd(IntegerType),
    BitwiseOr(IntegerType),
    BitwiseXor(IntegerType),
    WrappingShiftLeft {
        value: IntegerType,
        count: IntegerType,
    },
    WrappingShiftRight {
        value: IntegerType,
        count: IntegerType,
    },
    ExactShiftLeft {
        value: IntegerType,
        count: IntegerType,
    },
    ExactShiftRight {
        value: IntegerType,
        count: IntegerType,
    },
    ExactMultiply(IntegerType),
    ExactDivide(IntegerType),
    ExactRemainder(IntegerType),
    WrappingAdd(IntegerType),
    WrappingSubtract(IntegerType),
    WrappingMultiply(IntegerType),
    WrappingDivide(IntegerType),
    WrappingRemainder(IntegerType),
    SaturatingAdd(IntegerType),
    SaturatingSubtract(IntegerType),
    SaturatingMultiply(IntegerType),
    SaturatingDivide(IntegerType),
    SaturatingRemainder(IntegerType),
    Equal(IntegerType),
    LessThan(IntegerType),
    LessOrEqual(IntegerType),
}

impl IntegerOperation {
    const fn arity(self) -> usize {
        match self {
            Self::BitwiseNot(_) | Self::Widen { .. } | Self::ExactCast { .. } => 1,
            _ => 2,
        }
    }
}

impl Denotation {
    /// `operation : Π(_ : Int). … . R` — the shared assumption constant one
    /// operation denotes, pushed once per operation identity. `R` is `Int`
    /// for the arithmetic operations and `Two` for the value-level
    /// comparisons.
    pub(super) fn integer_operation(
        &mut self,
        operation: IntegerOperation,
    ) -> Result<u32, BoundedDenotationError> {
        if let Some(&position) = self.integer_operations.get(&operation) {
            return Ok(position);
        }
        let result = if matches!(
            operation,
            IntegerOperation::Equal(_)
                | IntegerOperation::LessThan(_)
                | IntegerOperation::LessOrEqual(_)
        ) {
            self.two
        } else {
            self.integer_constant()?
        };
        let domain = self.integer_constant()?;
        let mut ty = result;
        for _ in 0..operation.arity() {
            ty = self.arena.insert(Term::Pi {
                domain,
                codomain: ty,
            });
        }
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        self.integer_operations.insert(operation, position);
        Ok(position)
    }

    /// `op l' r'` — the applicative denotation of one open arithmetic
    /// operation, or `None` when the term is not such an operation.
    /// Closed operations never reach here: `integer_value` already
    /// evaluated them to their literal denotation.
    pub(super) fn integer_operation_term(
        &mut self,
        term: &ScalarTerm,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let (operation, left, right) = match term {
            ScalarTerm::IntegerBitwiseNot {
                scalar_type,
                operand,
            } => (IntegerOperation::BitwiseNot(*scalar_type), operand, None),
            ScalarTerm::IntegerWiden {
                source_type,
                target_type,
                operand,
            } => (
                IntegerOperation::Widen {
                    source: *source_type,
                    target: *target_type,
                },
                operand,
                None,
            ),
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type,
                operand,
            } => (
                IntegerOperation::ExactCast {
                    source: *source_type,
                    target: *target_type,
                },
                operand,
                None,
            ),
            ScalarTerm::IntegerBitwiseAnd {
                scalar_type,
                left,
                right,
            } => (
                IntegerOperation::BitwiseAnd(*scalar_type),
                left,
                Some(right),
            ),
            ScalarTerm::IntegerBitwiseOr {
                scalar_type,
                left,
                right,
            } => (IntegerOperation::BitwiseOr(*scalar_type), left, Some(right)),
            ScalarTerm::IntegerBitwiseXor {
                scalar_type,
                left,
                right,
            } => (
                IntegerOperation::BitwiseXor(*scalar_type),
                left,
                Some(right),
            ),
            ScalarTerm::WrappingIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            } => (
                IntegerOperation::WrappingShiftLeft {
                    value: *value_type,
                    count: *count_type,
                },
                value,
                Some(count),
            ),
            ScalarTerm::WrappingIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } => (
                IntegerOperation::WrappingShiftRight {
                    value: *value_type,
                    count: *count_type,
                },
                value,
                Some(count),
            ),
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            } => (
                IntegerOperation::ExactShiftLeft {
                    value: *value_type,
                    count: *count_type,
                },
                value,
                Some(count),
            ),
            ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } => (
                IntegerOperation::ExactShiftRight {
                    value: *value_type,
                    count: *count_type,
                },
                value,
                Some(count),
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } => (
                IntegerOperation::ExactMultiply(*scalar_type),
                left,
                Some(right),
            ),
            ScalarTerm::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            } => (
                IntegerOperation::ExactDivide(*scalar_type),
                left,
                Some(right),
            ),
            ScalarTerm::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            } => (
                IntegerOperation::ExactRemainder(*scalar_type),
                left,
                Some(right),
            ),
            ScalarTerm::WrappingIntegerAdd {
                scalar_type,
                left,
                right,
            } => (
                IntegerOperation::WrappingAdd(*scalar_type),
                left,
                Some(right),
            ),
            ScalarTerm::WrappingIntegerSubtract {
                scalar_type,
                left,
                right,
            } => (
                IntegerOperation::WrappingSubtract(*scalar_type),
                left,
                Some(right),
            ),
            ScalarTerm::WrappingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => (
                IntegerOperation::WrappingMultiply(*scalar_type),
                left,
                Some(right),
            ),
            ScalarTerm::WrappingIntegerDivide {
                scalar_type,
                left,
                right,
            } => (
                IntegerOperation::WrappingDivide(*scalar_type),
                left,
                Some(right),
            ),
            ScalarTerm::WrappingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => (
                IntegerOperation::WrappingRemainder(*scalar_type),
                left,
                Some(right),
            ),
            ScalarTerm::SaturatingIntegerAdd {
                scalar_type,
                left,
                right,
            } => (
                IntegerOperation::SaturatingAdd(*scalar_type),
                left,
                Some(right),
            ),
            ScalarTerm::SaturatingIntegerSubtract {
                scalar_type,
                left,
                right,
            } => (
                IntegerOperation::SaturatingSubtract(*scalar_type),
                left,
                Some(right),
            ),
            ScalarTerm::SaturatingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => (
                IntegerOperation::SaturatingMultiply(*scalar_type),
                left,
                Some(right),
            ),
            ScalarTerm::SaturatingIntegerDivide {
                scalar_type,
                left,
                right,
            } => (
                IntegerOperation::SaturatingDivide(*scalar_type),
                left,
                Some(right),
            ),
            ScalarTerm::SaturatingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => (
                IntegerOperation::SaturatingRemainder(*scalar_type),
                left,
                Some(right),
            ),
            _ => return Ok(None),
        };
        let left = self.fixed_scalar_term(left)?;
        let right = match right {
            Some(right) => Some(self.fixed_scalar_term(right)?),
            None => None,
        };
        let position = self.integer_operation(operation)?;
        let mut term = self.constant(position);
        for operand in [left].into_iter().chain(right) {
            term = self.arena.insert(Term::Apply {
                function: term,
                argument: operand,
            });
        }
        Ok(Some(term))
    }

    /// `op l' r'` for the value-level comparisons `IntegerEqual`,
    /// `IntegerLessThan`, `IntegerLessOrEqual` — `Two`-valued operations
    /// over `Int` endpoints, reached from `scalar_term` on Boolean-typed
    /// scalar terms.
    pub(super) fn integer_relation_term(
        &mut self,
        term: &ScalarTerm,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let (operation, left, right) = match term {
            ScalarTerm::IntegerEqual {
                scalar_type,
                left,
                right,
            } => (IntegerOperation::Equal(*scalar_type), left, right),
            ScalarTerm::IntegerLessThan {
                scalar_type,
                left,
                right,
            } => (IntegerOperation::LessThan(*scalar_type), left, right),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            } => (IntegerOperation::LessOrEqual(*scalar_type), left, right),
            _ => return Ok(None),
        };
        let left = self.fixed_scalar_term(left)?;
        let right = self.fixed_scalar_term(right)?;
        let position = self.integer_operation(operation)?;
        let function = self.constant(position);
        let function = self.arena.insert(Term::Apply {
            function,
            argument: left,
        });
        Ok(Some(self.arena.insert(Term::Apply {
            function,
            argument: right,
        })))
    }
}
