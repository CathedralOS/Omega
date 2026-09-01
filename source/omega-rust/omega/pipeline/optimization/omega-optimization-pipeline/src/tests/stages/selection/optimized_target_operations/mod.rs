//! Optimizer module role: stage group.
use crate::tests::*;
use omega_abstract_operations_to_target_operations::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    IntegerIeeeFloatLiteralSequenceMember,
};
use omega_target_operations::{
    ScalarParameterLocation, TargetBooleanExpression, TargetIntegerExpression, TargetUnitOperation,
};

mod arithmetic;
mod bitwise;
mod boolean_equal_immediate;
mod comparison;
mod direct;
mod immediate;
mod integer_equal_immediate;
mod locations;
mod shift;
mod structural_projected;
mod terminal;
mod unary;
mod unit;

use locations::*;
