use crate::tests::*;
use omega_abstract_operations_to_target_operations::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
};
use omega_target_operations::{
    ScalarParameterLocation, TargetBooleanExpression, TargetIntegerExpression,
};

mod comparison;
mod direct;
mod immediate;
mod locations;
mod terminal;
mod unary;

use locations::*;
