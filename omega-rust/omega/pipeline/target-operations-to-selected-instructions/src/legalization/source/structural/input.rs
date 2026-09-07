//! Whole-roster structural classification from borrowed source inputs.

#[cfg(test)]
use super::super::shared::*;
pub(super) use crate::structural_unit_input::Parameter;

pub(super) struct Argument<'a> {
    pub semantic: &'a terminal_psi::StructuralArgument,
    pub target: &'a target_operations::TargetStructuralArgument,
}

#[cfg(test)]
pub(in crate::legalization::source) fn accepts(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> bool {
    if target.target != target::NativeTarget::uefi_x64() || target.functions.is_empty() {
        return false;
    }
    let roster = target
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| {
            matches_function(index, function, target, abstract_plan, unit)
                .map(|callee| (function.machine, callee))
        })
        .collect::<Result<Vec<_>, _>>();
    let Ok(roster) = roster else {
        return false;
    };
    if roster
        .iter()
        .filter(|(machine, _)| *machine == target.entry)
        .count()
        != 1
        || roster.iter().enumerate().any(|(index, (machine, _))| {
            roster[..index]
                .iter()
                .any(|(previous, _)| previous == machine)
        })
    {
        return false;
    }
    // Every retained call names exactly one admitted function. Follow each
    // chain to a leaf; an ordinary acyclic call graph has no roster-size limit.
    roster.iter().all(|(machine, _)| {
        let mut current = *machine;
        for _ in 0..roster.len() {
            let Some((_, callee)) = roster.iter().find(|(candidate, _)| *candidate == current)
            else {
                return false;
            };
            match callee {
                None => return true,
                Some(callee) => current = *callee,
            }
        }
        false
    })
}

#[cfg(test)]
fn matches_function(
    index: usize,
    function: &target_operations::TargetFunction,
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<Option<semantic_vocabulary::MachineId>, LegalizationError> {
    let invalid = || Error::UnsupportedSourceShape { function: index };
    let abstracts = abstract_plan
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
        return Err(invalid());
    };
    let matched =
        super::super::matchers::match_structural_unit_form(function, abstracted, optimized)
            .ok_or_else(invalid)?;
    super::contract::validate_input(
        index,
        function,
        abstracted,
        optimized,
        target,
        abstract_plan,
        unit,
        &matched,
    )?;
    let TargetOperation::UnitBody(body) = &function.operation else {
        unreachable!()
    };
    let parameters = abstracted
        .structural_parameters
        .iter()
        .zip(&body.parameters)
        .map(|(semantic, target)| Parameter { semantic, target })
        .collect::<Vec<_>>();
    if !crate::structural_unit_input::accepts(&body.call_plan, &parameters, &body.structural_types)
    {
        return Err(invalid());
    }
    let callee = match (
        matched.target_call,
        matched.abstract_call,
        matched.optimized_call,
    ) {
        (None, None, None) => None,
        (Some(target_call), Some(abstract_call), Some(optimized_call)) => {
            let call = super::call::match_call(
                index,
                target_call,
                abstract_call,
                optimized_call,
                &parameters,
                &abstracted.entry_claims,
                target,
                abstract_plan,
                unit,
            )?;
            let callee = target
                .functions
                .iter()
                .find(|value| value.machine == call.callee)
                .ok_or_else(invalid)?;
            let TargetOperation::UnitBody(callee_body) = &callee.operation else {
                return Err(invalid());
            };
            // Selection transfers each corresponding whole root, not reordered
            // or repeated aliases. The callee is checked separately by the roster.
            if callee_body.call_plan != body.call_plan
                || call.arguments.len() != parameters.len()
                || call
                    .arguments
                    .iter()
                    .zip(&parameters)
                    .any(|(argument, parameter)| {
                        argument.target.source != parameter.target.placement
                            || argument.target.shape != parameter.target.shape
                            || argument.target.root_structural_type
                                != parameter.semantic.structural_type
                    })
            {
                return Err(invalid());
            }
            Some(call.callee)
        }
        _ => return Err(invalid()),
    };
    if let Some((targets, abstracts, optimized)) = matched.settlement_rows {
        for (position, ((target, abstract_row), optimized)) in
            targets.iter().zip(abstracts).zip(optimized).enumerate()
        {
            super::boundary_settlement::validate_input(
                index,
                position,
                target,
                abstract_row,
                optimized,
                &parameters,
                &abstracted.entry_claims,
                abstract_plan,
            )?;
        }
    }
    Ok(callee)
}
