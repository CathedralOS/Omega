//! Exact descriptor rows for the two local roles of the projected structural call/return closure.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::model::TranslationFamilyDescriptor;
use crate::validation::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamily,
    AbstractToTargetTranslationFamilyError, structural_call_return,
};

pub(in crate::validation::catalog) const CALLER: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StructuralCallReturnCaller,
        structural_call_return::is_caller_candidate,
        validate_caller,
    );

pub(in crate::validation::catalog) const CALLEE: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StructuralParameterReturnCallee,
        structural_call_return::is_callee_candidate,
        validate_callee,
    );

fn validate_caller(
    source: &AbstractFunction,
    _target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    structural_call_return::validate_caller(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StructuralCallReturnCaller)
        .map_err(AbstractToTargetTranslationFamilyError::StructuralCallReturnCaller)
}

fn validate_callee(
    source: &AbstractFunction,
    _target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    structural_call_return::validate_callee(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StructuralParameterReturnCallee)
        .map_err(AbstractToTargetTranslationFamilyError::StructuralParameterReturnCallee)
}
