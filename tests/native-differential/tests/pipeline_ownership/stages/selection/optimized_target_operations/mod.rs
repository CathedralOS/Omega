//! Optimizer module role: stage group.
use crate::tests::*;
use abstract_operations_to_target_operations::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    IntegerIeeeFloatLiteralSequenceMember,
};
use target_operations::{ScalarParameterLocation, TargetUnitOperation};

mod direct;
mod locations;
mod structural_projected;
mod unit;

use locations::*;
