//! Publication eligibility is input classification, not attempted legalization.
//! Independent construction/replay still checks source, proof and physical custody.

use super::shared::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum OrdinaryInputKind {
    Unit,
    Leaf,
    SharedReturn,
    Conditional,
}

pub(super) fn kind(
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
) -> OrdinaryInputKind {
    if matches!(target.operation, TargetOperation::UnitBody(_)) {
        OrdinaryInputKind::Unit
    } else if crate::legalization::scalar_leaf::control(target).is_some() {
        OrdinaryInputKind::Leaf
    } else if abstracted.block_entries.len() == 4 {
        OrdinaryInputKind::SharedReturn
    } else {
        OrdinaryInputKind::Conditional
    }
}

pub(crate) fn is_fragment_publication_program(
    target: &abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
) -> bool {
    let native = target.target_operations();
    let plan = target.optimized().plan();
    let unit = target.optimized().unit();
    accepts(native, plan, unit)
}

pub(crate) fn accepts(
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> bool {
    if native.functions.is_empty()
        || super::custody::validate_source_custody(native, plan, unit).is_err()
    {
        return false;
    }
    native
        .functions
        .iter()
        .enumerate()
        .all(|(index, function)| {
            let abstracts = plan
                .functions
                .iter()
                .filter(|value| value.machine == function.machine)
                .collect::<Vec<_>>();
            let optimized = unit
                .functions
                .iter()
                .filter(|value| value.machine == function.machine)
                .collect::<Vec<_>>();
            let ([abstracted], [optimized]) = (abstracts.as_slice(), optimized.as_slice()) else {
                return false;
            };
            eligible_function(index, native, function, abstracted, optimized, plan, unit)
        })
}

#[allow(clippy::too_many_arguments)]
fn eligible_function(
    index: usize,
    native: &TargetOperationPlan,
    function: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> bool {
    if function.mixed_structural_scalar_abi.is_some()
        || !abstracted.structural_parameters.is_empty()
        || !optimized.structural_parameters.is_empty()
        || !abstracted.entry_claims.is_empty()
        || !optimized.entry_claims.is_empty()
        || !optimized.entry_claim_declarations.is_empty()
        || !optimized.declared_places.is_empty()
        || !abstracted.published_service_ceiling.is_empty()
        || !optimized.published_service_ceiling.is_empty()
    {
        return false;
    }
    if kind(function, abstracted) == OrdinaryInputKind::Unit {
        return super::matchers::match_unit_form(function, abstracted, optimized).is_some()
            || super::scalar_call_unit::matches_input(
                index, function, abstracted, optimized, native, plan, unit,
            );
    }
    // Boolean-parameter catalog forms remain ineligible until the ordinary
    // scalar ABI representation retains Boolean without laundering it as U8.
    if function.attachment.is_some() || function.fixed_integer_scalar_abi.is_none() {
        return false;
    }
    match kind(function, abstracted) {
        OrdinaryInputKind::Leaf => crate::legalization::scalar_leaf::validate_input(
            index,
            native.target,
            function,
            abstracted,
            optimized,
        )
        .is_ok(),
        OrdinaryInputKind::SharedReturn => {
            super::shared_return::match_input(index, native.target, function, abstracted, optimized)
                .is_ok()
        }
        OrdinaryInputKind::Conditional => {
            super::conditional_input::match_input(index, function, abstracted, optimized).is_ok()
        }
        OrdinaryInputKind::Unit => unreachable!("handled Unit input"),
    }
}
