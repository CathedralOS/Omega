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

use std::collections::BTreeMap;

use numerics::bignum::BigInt;
use semantic_vocabulary::{IntegerType, IntegerValue};

use super::{BoundedDenotationError, Denotation, ScalarTerm, Term, TermHandle};

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

/// The `ScalarTerm` → `IntegerOperation` projection table: every row names
/// one term variant, the operation its application denotes, and the two
/// operands the application consumes, so the projection is one data table
/// rather than a body per variant.
macro_rules! integer_operation_table {
    ($term:expr; $( $variant:ident { $($field:ident),+ $(,)? } => ($operation:expr, $left:expr, $right:expr) ),+ $(,)?) => {
        match $term {
            $(
                ScalarTerm::$variant { $($field),+ } => ($operation, $left, $right),
            )+
            _ => return Ok(None),
        }
    };
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
        self.interned_keyed_assumption(
            |this| &mut this.integer_operations,
            operation,
            |this| {
                let result = if matches!(
                    operation,
                    IntegerOperation::Equal(_)
                        | IntegerOperation::LessThan(_)
                        | IntegerOperation::LessOrEqual(_)
                ) {
                    this.two
                } else {
                    this.integer_constant()?
                };
                let domain = this.integer_constant()?;
                let mut ty = result;
                for _ in 0..operation.arity() {
                    ty = this.arena.insert(Term::Pi {
                        domain,
                        codomain: ty,
                    });
                }
                Ok(ty)
            },
        )
    }

    /// `op l' r'` — the applicative denotation of one open arithmetic
    /// operation, or `None` when the term is not such an operation.
    /// Closed operations never reach here: `integer_value` already
    /// evaluated them to their literal denotation.
    pub(super) fn integer_operation_term(
        &mut self,
        term: &ScalarTerm,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let (operation, left, right) = integer_operation_table! { term;
            IntegerBitwiseNot { scalar_type, operand } => (IntegerOperation::BitwiseNot(*scalar_type), operand, None),
            IntegerWiden { source_type, target_type, operand } => (IntegerOperation::Widen { source: *source_type, target: *target_type }, operand, None),
            IntegerExactCast { source_type, target_type, operand } => (IntegerOperation::ExactCast { source: *source_type, target: *target_type }, operand, None),
            IntegerBitwiseAnd { scalar_type, left, right } => (IntegerOperation::BitwiseAnd(*scalar_type), left, Some(right)),
            IntegerBitwiseOr { scalar_type, left, right } => (IntegerOperation::BitwiseOr(*scalar_type), left, Some(right)),
            IntegerBitwiseXor { scalar_type, left, right } => (IntegerOperation::BitwiseXor(*scalar_type), left, Some(right)),
            WrappingIntegerShiftLeft { value_type, count_type, value, count } => (IntegerOperation::WrappingShiftLeft { value: *value_type, count: *count_type }, value, Some(count)),
            WrappingIntegerShiftRight { value_type, count_type, value, count } => (IntegerOperation::WrappingShiftRight { value: *value_type, count: *count_type }, value, Some(count)),
            ExactIntegerShiftLeft { value_type, count_type, value, count } => (IntegerOperation::ExactShiftLeft { value: *value_type, count: *count_type }, value, Some(count)),
            ExactIntegerShiftRight { value_type, count_type, value, count } => (IntegerOperation::ExactShiftRight { value: *value_type, count: *count_type }, value, Some(count)),
            ExactIntegerMultiply { scalar_type, left, right } => (IntegerOperation::ExactMultiply(*scalar_type), left, Some(right)),
            ExactIntegerDivide { scalar_type, left, right } => (IntegerOperation::ExactDivide(*scalar_type), left, Some(right)),
            ExactIntegerRemainder { scalar_type, left, right } => (IntegerOperation::ExactRemainder(*scalar_type), left, Some(right)),
            WrappingIntegerAdd { scalar_type, left, right } => (IntegerOperation::WrappingAdd(*scalar_type), left, Some(right)),
            WrappingIntegerSubtract { scalar_type, left, right } => (IntegerOperation::WrappingSubtract(*scalar_type), left, Some(right)),
            WrappingIntegerMultiply { scalar_type, left, right } => (IntegerOperation::WrappingMultiply(*scalar_type), left, Some(right)),
            WrappingIntegerDivide { scalar_type, left, right } => (IntegerOperation::WrappingDivide(*scalar_type), left, Some(right)),
            WrappingIntegerRemainder { scalar_type, left, right } => (IntegerOperation::WrappingRemainder(*scalar_type), left, Some(right)),
            SaturatingIntegerAdd { scalar_type, left, right } => (IntegerOperation::SaturatingAdd(*scalar_type), left, Some(right)),
            SaturatingIntegerSubtract { scalar_type, left, right } => (IntegerOperation::SaturatingSubtract(*scalar_type), left, Some(right)),
            SaturatingIntegerMultiply { scalar_type, left, right } => (IntegerOperation::SaturatingMultiply(*scalar_type), left, Some(right)),
            SaturatingIntegerDivide { scalar_type, left, right } => (IntegerOperation::SaturatingDivide(*scalar_type), left, Some(right)),
            SaturatingIntegerRemainder { scalar_type, left, right } => (IntegerOperation::SaturatingRemainder(*scalar_type), left, Some(right)),
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

    /// One binary operation applied to already-denoted operands —
    /// `op l r` as two `Apply`s over the operation's assumption constant.
    /// `operation` names which fixed operation's constant slot to use, so
    /// `add`/`subtract`/`multiply` keep their distinct functions.
    /// `op : Π(_ : Int). Π(_ : Int). Int` — the shared binary integer
    /// operation each open `IntegerMathTerm` arithmetic denotes, interned
    /// once in the operation family's own table.
    pub(super) fn binary_integer_operation(
        &mut self,
        cache: impl Fn(&mut Self) -> &mut Option<u32>,
    ) -> Result<u32, BoundedDenotationError> {
        self.interned_slot_assumption(cache, |this| {
            let integer = this.integer_constant()?;
            let result = this.arena.insert(Term::Pi {
                domain: integer,
                codomain: integer,
            });
            Ok(this.arena.insert(Term::Pi {
                domain: integer,
                codomain: result,
            }))
        })
    }

    pub(super) fn binary_terms(
        &mut self,
        operation: fn(&mut Self) -> Result<u32, BoundedDenotationError>,
        left: TermHandle,
        right: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = operation(self)?;
        let operation = self.constant(position);
        let function = self.arena.insert(Term::Apply {
            function: operation,
            argument: left,
        });
        Ok(self.arena.insert(Term::Apply {
            function,
            argument: right,
        }))
    }

    /// One checked `op l r = n` bridge between evaluated numerals and the
    /// operation's applied term, interned by its `(l, r, n)` value triple
    /// in the operation's own table. `compute` is the checked numeral
    /// arithmetic (add/sub/mul over machine integers), `terms` the
    /// operation's application builder, `cache` its intern table.
    pub(super) fn numeral_binary_operation(
        &mut self,
        left: IntegerValue,
        right: IntegerValue,
        result: IntegerValue,
        left_term: TermHandle,
        right_term: TermHandle,
        result_term: TermHandle,
        compute: impl Fn(&BigInt, &BigInt) -> BigInt,
        terms: fn(&mut Self, TermHandle, TermHandle) -> Result<TermHandle, BoundedDenotationError>,
        cache: impl Fn(&mut Self) -> &mut BTreeMap<(IntegerValue, IntegerValue, IntegerValue), u32>,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let as_integer = |value: IntegerValue| match value {
            IntegerValue::Signed(value) => BigInt::from_i128(value),
            IntegerValue::Unsigned(value) => BigInt::from_u128(value),
        };
        if compute(&as_integer(left), &as_integer(right)) != as_integer(result) {
            return Ok(None);
        }
        let position = self.interned_keyed_assumption(cache, (left, right, result), |this| {
            let integer = this.integer_constant()?;
            let application = terms(this, left_term, right_term)?;
            Ok(this.arena.insert(Term::Id {
                ty: integer,
                left: application,
                right: result_term,
            }))
        })?;
        Ok(Some(self.constant(position)))
    }
}
