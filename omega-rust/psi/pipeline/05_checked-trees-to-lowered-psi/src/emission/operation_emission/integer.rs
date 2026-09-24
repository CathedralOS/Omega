//! Integer operation selection and its exact formation-obligation identity.

use crate::terminal_identities::obligation_id;
use numerics::{
    arithmetic::ArithmeticDomain,
    integer_policy::{IntegerPolicyPrimitive, integer_policy_bridge},
};
use semantic_vocabulary::{ObligationId, OperationId, ValueId};
use terminal_psi::{OperationKind, TrappingIntegerOperation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoweredIntegerComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}

impl LoweredIntegerComparisonKind {
    pub(crate) const fn operation(self, left: ValueId, right: ValueId) -> OperationKind {
        match self {
            Self::Equal => OperationKind::IntegerEqual { left, right },
            Self::LessThan => OperationKind::IntegerLessThan { left, right },
            Self::LessOrEqual => OperationKind::IntegerLessOrEqual { left, right },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoweredIntegerBinaryKind {
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    WrappingShiftLeft,
    WrappingShiftRight,
    ExactShiftLeft,
    ExactShiftRight,
    ExactAdd,
    ExactSubtract,
    ExactMultiply,
    ExactDivide,
    ExactRemainder,
    WrappingDivide,
    WrappingRemainder,
    SaturatingDivide,
    SaturatingRemainder,
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
    TrappingAdd,
    TrappingSubtract,
    TrappingMultiply,
    TrappingDivide,
    TrappingRemainder,
    TrappingShiftLeft,
    TrappingShiftRight,
}

impl LoweredIntegerBinaryKind {
    pub(crate) const fn integer_policy_binding(
        self,
    ) -> Option<(IntegerPolicyPrimitive, ArithmeticDomain)> {
        match self {
            Self::WrappingShiftLeft => Some((
                IntegerPolicyPrimitive::ShiftLeft,
                ArithmeticDomain::Wrapping,
            )),
            Self::WrappingShiftRight => Some((
                IntegerPolicyPrimitive::ShiftRight,
                ArithmeticDomain::Wrapping,
            )),
            Self::ExactShiftLeft => {
                Some((IntegerPolicyPrimitive::ShiftLeft, ArithmeticDomain::Exact))
            }
            Self::ExactShiftRight => {
                Some((IntegerPolicyPrimitive::ShiftRight, ArithmeticDomain::Exact))
            }
            Self::ExactAdd => Some((IntegerPolicyPrimitive::Add, ArithmeticDomain::Exact)),
            Self::ExactSubtract => {
                Some((IntegerPolicyPrimitive::Subtract, ArithmeticDomain::Exact))
            }
            Self::ExactMultiply => {
                Some((IntegerPolicyPrimitive::Multiply, ArithmeticDomain::Exact))
            }
            Self::ExactDivide => Some((IntegerPolicyPrimitive::Divide, ArithmeticDomain::Exact)),
            Self::WrappingDivide => {
                Some((IntegerPolicyPrimitive::Divide, ArithmeticDomain::Wrapping))
            }
            Self::SaturatingDivide => {
                Some((IntegerPolicyPrimitive::Divide, ArithmeticDomain::Saturating))
            }
            Self::ExactRemainder => {
                Some((IntegerPolicyPrimitive::Remainder, ArithmeticDomain::Exact))
            }
            Self::WrappingRemainder => Some((
                IntegerPolicyPrimitive::Remainder,
                ArithmeticDomain::Wrapping,
            )),
            Self::SaturatingRemainder => Some((
                IntegerPolicyPrimitive::Remainder,
                ArithmeticDomain::Saturating,
            )),
            Self::WrappingAdd => Some((IntegerPolicyPrimitive::Add, ArithmeticDomain::Wrapping)),
            Self::SaturatingAdd => {
                Some((IntegerPolicyPrimitive::Add, ArithmeticDomain::Saturating))
            }
            Self::WrappingSubtract => {
                Some((IntegerPolicyPrimitive::Subtract, ArithmeticDomain::Wrapping))
            }
            Self::SaturatingSubtract => Some((
                IntegerPolicyPrimitive::Subtract,
                ArithmeticDomain::Saturating,
            )),
            Self::WrappingMultiply => {
                Some((IntegerPolicyPrimitive::Multiply, ArithmeticDomain::Wrapping))
            }
            Self::SaturatingMultiply => Some((
                IntegerPolicyPrimitive::Multiply,
                ArithmeticDomain::Saturating,
            )),
            Self::TrappingAdd => Some((IntegerPolicyPrimitive::Add, ArithmeticDomain::Trapping)),
            Self::TrappingSubtract => {
                Some((IntegerPolicyPrimitive::Subtract, ArithmeticDomain::Trapping))
            }
            Self::TrappingMultiply => {
                Some((IntegerPolicyPrimitive::Multiply, ArithmeticDomain::Trapping))
            }
            Self::TrappingDivide => {
                Some((IntegerPolicyPrimitive::Divide, ArithmeticDomain::Trapping))
            }
            Self::TrappingRemainder => Some((
                IntegerPolicyPrimitive::Remainder,
                ArithmeticDomain::Trapping,
            )),
            Self::TrappingShiftLeft => Some((
                IntegerPolicyPrimitive::ShiftLeft,
                ArithmeticDomain::Trapping,
            )),
            Self::TrappingShiftRight => Some((
                IntegerPolicyPrimitive::ShiftRight,
                ArithmeticDomain::Trapping,
            )),
            Self::BitwiseAnd | Self::BitwiseOr | Self::BitwiseXor => None,
        }
    }

    pub(crate) fn formation_obligation(self, operation: OperationId) -> Option<ObligationId> {
        let catalog_requires_formation =
            self.integer_policy_binding()
                .is_some_and(|(primitive, policy)| {
                    !integer_policy_bridge(primitive, policy)
                        .formation_conditions
                        .is_empty()
                });
        catalog_requires_formation.then(|| {
            obligation_id(
                operation
                    .get()
                    .checked_add(1)
                    .expect("integer formation obligation follows its operation identity"),
            )
        })
    }

    pub(crate) fn operation(
        self,
        operation: OperationId,
        left: ValueId,
        right: ValueId,
    ) -> OperationKind {
        let formation_obligation = self.formation_obligation(operation);
        match self {
            Self::BitwiseAnd => OperationKind::IntegerBitwiseAnd { left, right },
            Self::BitwiseOr => OperationKind::IntegerBitwiseOr { left, right },
            Self::BitwiseXor => OperationKind::IntegerBitwiseXor { left, right },
            Self::WrappingShiftLeft => OperationKind::WrappingIntegerShiftLeft {
                value: left,
                count: right,
            },
            Self::WrappingShiftRight => OperationKind::WrappingIntegerShiftRight {
                value: left,
                count: right,
            },
            Self::ExactShiftLeft => OperationKind::ExactIntegerShiftLeft {
                value: left,
                count: right,
                obligation: formation_obligation.expect("exact shift has formation conditions"),
            },
            Self::ExactShiftRight => OperationKind::ExactIntegerShiftRight {
                value: left,
                count: right,
                obligation: formation_obligation.expect("exact shift has formation conditions"),
            },
            Self::ExactAdd => OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: formation_obligation.expect("exact add has formation conditions"),
            },
            Self::ExactSubtract => OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: formation_obligation.expect("exact subtract has formation conditions"),
            },
            Self::ExactMultiply => OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation: formation_obligation.expect("exact multiply has formation conditions"),
            },
            Self::ExactDivide => OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation: formation_obligation.expect("exact divide has formation conditions"),
            },
            Self::ExactRemainder => OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation: formation_obligation.expect("exact remainder retains its obligation"),
            },
            Self::WrappingDivide => OperationKind::WrappingIntegerDivide {
                left,
                right,
                obligation: formation_obligation.expect("wrapping divide has formation conditions"),
            },
            Self::WrappingRemainder => OperationKind::WrappingIntegerRemainder {
                left,
                right,
                obligation: formation_obligation
                    .expect("wrapping remainder retains its obligation"),
            },
            Self::SaturatingDivide => OperationKind::SaturatingIntegerDivide {
                left,
                right,
                obligation: formation_obligation
                    .expect("saturating divide has formation conditions"),
            },
            Self::SaturatingRemainder => OperationKind::SaturatingIntegerRemainder {
                left,
                right,
                obligation: formation_obligation
                    .expect("saturating remainder retains its obligation"),
            },
            Self::WrappingAdd => OperationKind::WrappingIntegerAdd { left, right },
            Self::SaturatingAdd => OperationKind::SaturatingIntegerAdd { left, right },
            Self::WrappingSubtract => OperationKind::WrappingIntegerSubtract { left, right },
            Self::SaturatingSubtract => OperationKind::SaturatingIntegerSubtract { left, right },
            Self::WrappingMultiply => OperationKind::WrappingIntegerMultiply { left, right },
            Self::SaturatingMultiply => OperationKind::SaturatingIntegerMultiply { left, right },
            Self::TrappingAdd => trapping(TrappingIntegerOperation::Add { left, right }),
            Self::TrappingSubtract => trapping(TrappingIntegerOperation::Subtract { left, right }),
            Self::TrappingMultiply => trapping(TrappingIntegerOperation::Multiply { left, right }),
            Self::TrappingDivide => trapping(TrappingIntegerOperation::Divide { left, right }),
            Self::TrappingRemainder => {
                trapping(TrappingIntegerOperation::Remainder { left, right })
            }
            Self::TrappingShiftLeft => trapping(TrappingIntegerOperation::ShiftLeft {
                value: left,
                count: right,
            }),
            Self::TrappingShiftRight => trapping(TrappingIntegerOperation::ShiftRight {
                value: left,
                count: right,
            }),
        }
    }
}

/// The settled catalog gives every Trapping row an empty formation list, so
/// no obligation identity is allocated: the operation is its own crash site.
const fn trapping(operation: TrappingIntegerOperation) -> OperationKind {
    OperationKind::TrappingInteger { operation }
}
