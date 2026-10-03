//! Reconstruct return-owned invocations from source custody, not target rows.
//! The selected hook has ordinary ABI and body validation. This join owns only
//! its exact nominal owner, borrowed root, action order, and normal return.
use crate::LegalizationError;
use abstract_operations::{AbstractFunctionResult, AbstractOperation, AbstractOperationPlan};
use calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarCall};
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use target_operations::{CallSiteOwner, TargetOperationPlan, TargetUnitOperation};
use terminal_psi::{StructuralAccess, StructuralArgument, TerminalAffineCleanupAction};

pub(in crate::legalization) fn expected(
    operation: &AbstractOperation,
    caller: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<Vec<(CallSiteOwner, LegalizedScalarCall)>, LegalizationError> {
    let (edge, actions) = match operation {
        AbstractOperation::ReturnUnit {
            psi_edge,
            cleanup_actions,
        }
        | AbstractOperation::Return {
            psi_edge,
            cleanup_actions,
            ..
        } => (*psi_edge, cleanup_actions),
        _ => return Ok(Vec::new()),
    };
    let mut calls = Vec::new();
    for (ordinal, action) in actions.iter().enumerate() {
        let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action else {
            continue;
        };
        let invalid = LegalizationError::custody;
        let callee = unit
            .functions
            .iter()
            .find(|function| function.machine == cleanup.cleanup_machine)
            .ok_or_else(invalid)?;
        if callee.machine == caller.machine
            || callee.attachment != Some(cleanup.structural_type)
            || callee.result != AbstractFunctionResult::Unit
            || !callee.parameters.is_empty()
            || callee.structural_parameters.len() > 1
            || !cleanup.requirement_obligations.is_empty()
        {
            return Err(invalid());
        }
        let owner = CallSiteOwner::CleanupAction {
            edge,
            action_ordinal: ordinal.try_into().map_err(|_| invalid())?,
        };
        let shapes = callee
            .structural_parameters
            .iter()
            .map(|parameter| {
                crate::structural_inputs::structural_reference_input::parameter_shape(
                    parameter,
                    &plan.structural_types,
                )
                .ok_or_else(invalid)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(native.target),
            &CallSignature {
                parameters: shapes,
                result: None,
            },
        )
        .map_err(|_| invalid())?;
        let mut arguments = Vec::new();
        for (position, parameter) in callee.structural_parameters.iter().enumerate() {
            if position != 0
                || parameter.position != 0
                || parameter.structural_type != cleanup.structural_type
                || cleanup.cleanup_receiver != Some(parameter.place)
                || !matches!(
                    parameter.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                )
            {
                return Err(invalid());
            }
            let semantic = StructuralArgument {
                place: cleanup.place,
                path: Vec::new(),
                access: parameter.access,
            };
            let target = super::aggregate_results::borrowed_arguments::reconstruct_at_owner(
                &semantic, position, owner, caller, callee, &call_plan, native, plan,
            )?;
            arguments.push(LegalizedScalarArgument::Structural {
                semantic,
                target,
                indices: Vec::new(),
            });
        }
        if callee.structural_parameters.is_empty() && cleanup.cleanup_receiver.is_some() {
            return Err(invalid());
        }
        calls.push((
            owner,
            LegalizedScalarCall {
                source: target_operations::NativeCallOrigin::Authored,
                callee: callee.machine,
                call_plan,
                arguments,
                result_placement: None,
                structural_result: None,
                descriptor_origins: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        ));
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
            || *callee != call.callee
            || *call_plan != call.call_plan
            || *arguments != expected_arguments
        {
            return Err(LegalizationError::custody());
        }
    }
    Ok(())
}
