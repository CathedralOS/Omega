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
mod comparison;
mod direct;
mod immediate;
mod locations;
mod shift;
mod terminal;
mod unary;
mod unit;

use locations::*;
