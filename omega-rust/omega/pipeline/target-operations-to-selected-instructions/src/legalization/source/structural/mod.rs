//! Optimizer module role: executable entrance.
mod boundary_settlement;
mod call;
mod callee_contract;
mod contract;
mod operations;

use super::matchers::MatchedStructuralUnitForm;
use super::shared::*;
use crate::legalization::catalog::LegalizationFormRecipe;
use contract::validate_and_derive_parameters;
use operations::derive_structural_operations;

#[allow(clippy::too_many_arguments)]
pub(super) fn derive_source_structural_unit_function(
    function: usize,
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    matched: MatchedStructuralUnitForm<'_>,
) -> Result<SourceStructuralUnitFunction, LegalizationError> {
    let parameters = validate_and_derive_parameters(
        function,
        target,
        abstracted,
        optimized,
        target_plan,
        abstract_plan,
        unit,
        &matched,
    )?;
    let operations = derive_structural_operations(
        function,
        &matched,
        &parameters,
        &abstracted.entry_claims,
        target_plan,
        abstract_plan,
        unit,
    )?;
    let TargetOperation::UnitBody(body) = &target.operation else {
        unreachable!()
    };
    let [optimized_block] = optimized.blocks.as_slice() else {
        unreachable!()
    };
    let LegalizationFormRecipe::StructuralUnit(recipe) = matched.descriptor.recipe else {
        unreachable!()
    };
    let TargetUnitOperation::Return { psi_edge, .. } = matched.target_return else {
        unreachable!()
    };

    Ok(SourceStructuralUnitFunction {
        machine: target.machine,
        attachment: target.attachment,
        provenance: target.provenance.clone(),
        recipe,
        structural_types: body.structural_types.clone(),
        call_plan: body.call_plan.clone(),
        parameters,
        structural_places: synthesized_parameter_places(&abstracted.structural_parameters),
        entry_claims: abstracted.entry_claims.clone(),
        published_service_ceiling: abstracted.published_service_ceiling.clone(),
        entry_block: optimized_block.id,
        boundary_settlements: operations.boundary_settlements,
        call: operations.call,
        return_edge: *psi_edge,
        return_fuel: matched.optimized_return.fuel.clone(),
        return_effect: matched.optimized_return.effect,
        return_ownership: matched.optimized_return.ownership.clone(),
    })
}

fn synthesized_parameter_places(
    parameters: &[terminal_psi::StructuralParameterDeclaration],
) -> Vec<StructuralPlaceDeclaration> {
    parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            },
        })
        .collect()
}
