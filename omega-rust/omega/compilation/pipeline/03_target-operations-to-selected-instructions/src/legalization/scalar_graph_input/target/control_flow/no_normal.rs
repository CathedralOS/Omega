//! Independent entry transport audit for an edge-owned ordinary invocation.
//! The declared result determines ABI placement only; no receiving value or
//! normal completion is manufactured by this audit.
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::{self as input, reference_custody};
use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::{InvocationSite, ValueId};
use target_operations::{
    TargetControlTerminator, TargetOperationPlan, TargetUnitScalarArgumentSource,
};

pub(super) fn validate(
    target: &TargetControlTerminator,
    source: &AbstractOperation,
    available: &[(ValueId, TargetUnitScalarArgumentSource)],
    custody: &reference_custody::Custody,
    caller: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let TargetControlTerminator::InvokeNoNormal {
        psi_edge,
        callee,
        call_plan,
        scalar_arguments,
        arguments,
        placed_bindings,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        declared_result,
    } = target
    else {
        return Err(LegalizationError::custody());
    };
    let Some(invocation) = source.module_invocation() else {
        return Err(LegalizationError::custody());
    };
    let expected = input::callee_plan(*callee, native, plan, unit)?;
    let signature = input::call_signature::signature(*callee, plan)?;
    let declaration = unit.function(*callee).ok_or(LegalizationError::custody())?;
    let site = InvocationSite::InvokeNoNormal(*psi_edge);
    if invocation.site != site
        || invocation.callee != *callee
        || call_plan != &expected
        || declared_result != &declaration.result
        || claim_transfers != invocation.claim_transfers
        || requirement_obligations != invocation.requirement_obligations
        || crash_continuations != invocation.crash_continuations
        || arguments.len() != invocation.structural_arguments.len()
        || arguments.len() != signature.structural_parameter_count()
        || scalar_arguments.len() != invocation.arguments.len()
        || scalar_arguments.len() != signature.scalar_parameter_count()
        || expected.parameters.len() != scalar_arguments.len() + arguments.len()
        || scalar_arguments
            .iter()
            .zip(invocation.arguments)
            .zip(&expected.parameters)
            .enumerate()
            .any(|(position, ((argument, value), placement))| {
                argument.parameter_index != position as u32
                    || argument.placement != *placement
                    || Some(argument.source.scalar_type()) != signature.scalar_parameter(position)
                    || !available
                        .iter()
                        .any(|(identity, source)| identity == value && source == &argument.source)
            })
    {
        return Err(LegalizationError::custody());
    }
    let expected_bindings = reference_custody::placed_call_custody(
        caller,
        declaration,
        &plan.structural_types,
        invocation.structural_arguments,
        custody,
    )?;
    if placed_bindings != &expected_bindings {
        return Err(LegalizationError::custody());
    }
    for (position, (argument, semantic)) in arguments
        .iter()
        .zip(invocation.structural_arguments)
        .enumerate()
    {
        let expected = input::structural_call::argument_at_invocation(
            semantic, position, site, caller, signature, &expected, native, plan, custody,
        )?;
        if argument != &expected {
            return Err(LegalizationError::custody());
        }
    }
    Ok(())
}
