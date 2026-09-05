use super::super::matchers::MatchedStructuralUnitForm;
use super::super::shared::*;
use super::boundary_settlement::derive_boundary_settlement;
use super::call::derive_structural_call;

pub(super) struct DerivedStructuralOperations {
    pub(super) call: Option<LegalizedCallUnit>,
    pub(super) boundary_settlements: Vec<LegalizedBoundarySettlement>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn derive_structural_operations(
    function: usize,
    matched: &MatchedStructuralUnitForm<'_>,
    parameters: &[LegalizedCallUnitParameter],
    caller_claims: &[terminal_psi::EntryClaim],
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<DerivedStructuralOperations, LegalizationError> {
    let call = match (
        matched.target_call,
        matched.abstract_call,
        matched.optimized_call,
    ) {
        (None, None, None) => None,
        (Some(target_call), Some(abstract_call), Some(optimized_call)) => {
            Some(derive_structural_call(
                function,
                target_call,
                abstract_call,
                optimized_call,
                parameters,
                caller_claims,
                target_plan,
                abstract_plan,
                unit,
            )?)
        }
        _ => return Err(Error::UnsupportedSourceShape { function }),
    };
    let boundary_settlements = matched
        .settlement_rows
        .map(|(target_rows, abstract_rows, optimized_rows)| {
            target_rows
                .iter()
                .zip(abstract_rows)
                .zip(optimized_rows)
                .enumerate()
                .map(|(index, ((target, abstract_row), optimized))| {
                    derive_boundary_settlement(
                        function,
                        index,
                        target,
                        abstract_row,
                        optimized,
                        parameters,
                        caller_claims,
                        abstract_plan,
                    )
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(DerivedStructuralOperations {
        call,
        boundary_settlements,
    })
}
