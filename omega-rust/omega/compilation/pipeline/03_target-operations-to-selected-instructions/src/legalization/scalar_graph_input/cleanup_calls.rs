//! Reconstruct edge-owned invocations from source custody, not target rows.
//! The selected hook has ordinary ABI and body validation. This join owns only
//! its exact nominal owner, borrowed root, action order, and normal return.
use crate::LegalizationError;
use abstract_operations::{AbstractFunctionResult, AbstractOperation, AbstractOperationPlan};
use calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarCall};
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use target_operations::{CallSiteOwner, TargetOperationPlan, TargetUnitOperation};
use terminal_psi::{StructuralArgument, TerminalAffineCleanupAction};

pub(in crate::legalization) fn expected(
    operation: &AbstractOperation,
    caller: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<Vec<(CallSiteOwner, LegalizedScalarCall)>, LegalizationError> {
    let mut calls = Vec::new();
    for (edge, actions) in operation.cleanup_rosters() {
        for (ordinal, action) in actions.iter().enumerate() {
            let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action else {
                continue;
            };
            let callee = unit
                .function(cleanup.cleanup_machine)
                .ok_or(LegalizationError::custody())?;
            if callee.machine == caller.machine
                || callee.attachment != Some(cleanup.structural_type)
                || callee.nominal_cleanup_subject != Some(cleanup.structural_type)
                || callee.result != AbstractFunctionResult::Unit
                || !callee.parameters.is_empty()
                || callee.structural_parameters.len() > 1
                || !cleanup.requirement_obligations.is_empty()
            {
                return Err(LegalizationError::custody());
            }
            let owner = CallSiteOwner::CleanupAction {
                edge,
                action_ordinal: ordinal
                    .try_into()
                    .map_err(|_| LegalizationError::custody())?,
            };
            let shapes = callee
                .structural_parameters
                .iter()
                .map(|parameter| {
                    crate::structural_inputs::structural_reference_input::parameter_shape(
                        parameter,
                        &plan.structural_types,
                    )
                    .ok_or(LegalizationError::custody())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(native.target),
                &CallSignature {
                    parameters: shapes,
                    result: None,
                },
            )
            .map_err(|_| LegalizationError::custody())?;
            let mut arguments = Vec::new();
            for (position, parameter) in callee.structural_parameters.iter().enumerate() {
                if position != 0 || !parameter.is_cleanup_receiver(cleanup) {
                    return Err(LegalizationError::custody());
                }
                let semantic = StructuralArgument {
                    place: cleanup.place,
                    path: cleanup.path.clone(),
                    access: parameter.access,
                };
                let target = super::aggregate_results::borrowed_arguments::reconstruct_at_owner(
                    &semantic,
                    position,
                    owner,
                    caller,
                    super::call_signature::signature(callee.machine, plan)?,
                    &call_plan,
                    native,
                    plan,
                )?;
                let indices = crate::legalization::runtime_indices::projection(
                    caller,
                    unit,
                    optimization_unit::PsiProvenance::Edge(edge),
                    target.root_structural_type,
                    &target.path,
                    &unit.structural_types,
                )?;
                arguments.push(LegalizedScalarArgument::Structural {
                    semantic,
                    target,
                    indices,
                });
            }
            if callee.structural_parameters.is_empty() && cleanup.cleanup_receiver.is_some() {
                return Err(LegalizationError::custody());
            }
            calls.push((
                owner,
                LegalizedScalarCall {
                    source: target_operations::NativeCallOrigin::Authored,
                    target: target_operations::NativeCallTarget::Machine(callee.machine),
                    call_plan,
                    arguments,
                    result_placement: None,
                    structural_result: None,
                    descriptor_origins: Vec::new(),
                    placed_bindings: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            ));
        }
    }
    Ok(calls)
}

pub(in crate::legalization) fn validate_target(
    rows: &[TargetUnitOperation],
    operation: &AbstractOperation,
    caller: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let expected = expected(operation, caller, native, plan, unit)?;
    if rows.len() != expected.len() {
        return Err(LegalizationError::custody());
    }
    for (row, (owner, call)) in rows.iter().zip(expected) {
        let TargetUnitOperation::CleanupCall {
            owner: actual,
            callee,
            call_plan,
            arguments,
        } = row
        else {
            return Err(LegalizationError::custody());
        };
        let expected_arguments = call
            .arguments
            .iter()
            .map(|argument| match argument {
                LegalizedScalarArgument::Structural { target, .. } => target.clone(),
                _ => unreachable!("cleanup has no scalar parameters"),
            })
            .collect::<Vec<_>>();
        if *actual != owner
            || Some(*callee) != call.target.machine()
            || *call_plan != call.call_plan
            || *arguments != expected_arguments
        {
            return Err(LegalizationError::custody());
        }
    }
    Ok(())
}
