//! One admission for every call a Unit body makes, whether the body is an
//! ordinary machine or one state of a composed graph.
//!
//! Both arrangements emit their calls through
//! `operation_frame::OperationFrame::emit_call`, so both rejoin each call to
//! its checked custody here, before any identity is allocated: the exact
//! authored call site and operands (`call_source_custody::validate_operation`),
//! the reconstructed structural call custody, the target's checked
//! signature, contract and reach, and how each structural operand's producer
//! is consumed. `CallerView` is the one caller both routes build: an ordinary
//! body's whole operation roster, or one state's. Each call shape therefore
//! has exactly one admission, and a shape the emitter lowers is admitted in
//! every arrangement that can hold it.
//!
//! Three decisions stay with the route, outside this file:
//! - What becomes of a structural result after its producing call. An
//!   ordinary body rejoins every result's final use
//!   (`call_closure::validate_unit_operation_sequence`); a composed graph
//!   rejoins it on each edge (`state_graph::result_custody`), because a
//!   state's result may leave on a successor edge.
//! - Whether a scalar callee belongs to the closure. An ordinary body checks
//!   the closure assembled before admission (`operations::validate`); a
//!   composed catalog selects its scalar callees from these same calls.
//! - Provider attachment custody and which non-call operations a body holds.

use super::super::super::{
    CheckedBoundaryMachineResultPlan, CheckedTrees, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, LoweringError, terminal_scalar_type, unsupported,
};
use super::super::bodies::{UnitBody, UnitEntry, UnitPlans};
use super::super::{
    checked_unit_target_reach_matches, primitive_locals, retain_exact_flow_call,
    retain_exact_unit_boundary, scalar_structural_calls, structural_calls, unique_unit_boundary,
};
use crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee;
use checked_trees::{
    CheckedBoundaryMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedComposedUnitControlTerminatorPlan, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitStructuralParameterPlan, CheckedUnitStructuralReturnPlan,
};
use symbols::SymbolHandle;

/// The calling body a call's admission rejoins against.
#[derive(Clone, Copy)]
pub(in crate::unit::attached_unit) struct CallerView<'a> {
    pub(in crate::unit::attached_unit) machine: SymbolHandle,
    pub(in crate::unit::attached_unit) state: SymbolHandle,
    /// The sequence the call belongs to, which holds its operands' producers
    /// and its own cleanup: the whole body, or the one state.
    pub(in crate::unit::attached_unit) operations: &'a [CheckedUnitEffectOperationPlan],
    pub(in crate::unit::attached_unit) structural_parameters:
        &'a [CheckedUnitStructuralParameterPlan],
    /// The claims the body (ordinary) or the state (composed) holds on entry.
    pub(in crate::unit::attached_unit) entry_claims: &'a [CheckedUnitEntryClaimPlan],
    /// The value the sequence returns whole, if it completes with one.
    pub(in crate::unit::attached_unit) structural_result:
        Option<&'a CheckedUnitStructuralReturnPlan>,
    /// An ordinary body's empty-record affine locals; a state declares none.
    pub(in crate::unit::attached_unit) trivial_affine_locals:
        &'a [CheckedTrivialAffineStructuralLocalPlan],
    /// The ordinary body itself. Only an ordinary body establishes primitive
    /// referents (`EstablishPrimitiveLocal`) a call may borrow; a composed
    /// state keeps scalar storage in its binding namespace, and its operation
    /// frame carries no primitive referent, so a state's call cannot name one.
    pub(in crate::unit::attached_unit) ordinary_body: Option<&'a CheckedUnitEffectMachinePlan>,
}

impl<'a> CallerView<'a> {
    pub(in crate::unit::attached_unit) fn ordinary(plan: &'a CheckedUnitEffectMachinePlan) -> Self {
        Self {
            machine: plan.machine,
            state: plan.state,
            operations: &plan.operations,
            structural_parameters: &plan.structural_parameters,
            entry_claims: &plan.entry_claims,
            structural_result: plan.structural_result.as_ref(),
            trivial_affine_locals: &plan.trivial_affine_locals,
            ordinary_body: Some(plan),
        }
    }

    pub(in crate::unit::attached_unit) fn state(
        machine: SymbolHandle,
        state: &'a CheckedComposedUnitControlStatePlan,
    ) -> Self {
        Self {
            machine,
            state: state.state,
            operations: &state.operations,
            structural_parameters: &state.structural_parameters,
            entry_claims: &state.entry_claims,
            structural_result: match &state.terminator {
                CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result } => {
                    Some(result)
                }
                _ => None,
            },
            trivial_affine_locals: &[],
            ordinary_body: None,
        }
    }
}

/// Admit `operation` when it is a call, or the member calls of a structural
/// value construction; every other operation belongs to its route's own
/// checks. Each boundary a call names joins `boundaries` once.
pub(in crate::unit::attached_unit) fn admit<'a>(
    checked: &CheckedTrees,
    plans: UnitPlans<'a>,
    caller: &CallerView<'_>,
    operation: &CheckedUnitEffectOperationPlan,
    boundaries: &mut Vec<(&'a CheckedBoundaryMachinePlan, String)>,
) -> Result<(), LoweringError> {
    match operation {
        // A member call shares its construction's statement custody: the
        // construction's source replay rejoins the member's occurrence and
        // result as part of the value. Only its target is admitted here.
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { calls, .. } => {
            for call in calls {
                unit_target(checked, plans, call.operation())?;
            }
            return Ok(());
        }
        CheckedUnitEffectOperationPlan::CallUnit { .. }
        | CheckedUnitEffectOperationPlan::StructuralCall { .. }
        | CheckedUnitEffectOperationPlan::ScalarCall { .. }
        | CheckedUnitEffectOperationPlan::BoundaryCall { .. }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. } => {}
        _ => return Ok(()),
    }
    crate::emission::call_source_custody::validate_operation(
        checked,
        caller.machine,
        caller.state,
        operation,
        caller.structural_parameters,
    )?;
    structural_calls::validate_custody(checked, caller.machine, caller.state, operation)?;
    match operation {
        // A structural call whose target is not a Unit body is a claim-free
        // affine leaf with its own checked return plan.
        CheckedUnitEffectOperationPlan::StructuralCall { target_machine, .. }
            if !UnitBody::contains(plans, *target_machine) =>
        {
            structural_calls::validate(checked, caller, operation)
        }
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_state,
            ..
        }
        | CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            target_state,
            ..
        } => {
            retain_exact_flow_call(
                checked,
                caller.machine,
                caller.state,
                *coordinate,
                *target_state,
            )?;
            let target = unit_target(checked, plans, operation)?;
            primitive_locals::unit_calls::validate(
                checked,
                caller,
                operation,
                target.structural_parameters,
            )?;
            crate::emission::call_source_custody::projected_receivers::validate(
                checked,
                caller.machine,
                caller.state,
                caller.operations,
                caller.structural_parameters,
                operation,
                target.structural_parameters,
            )?;
            structural_calls::validate_consumer(
                checked,
                caller,
                operation,
                target.structural_parameters,
                target.entry_claims,
            )
        }
        CheckedUnitEffectOperationPlan::ScalarCall { .. } => {
            scalar_call(checked, caller, operation)
        }
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            ..
        } => {
            retain_exact_flow_call(
                checked,
                caller.machine,
                caller.state,
                *coordinate,
                *target_state,
            )?;
            retain_exact_unit_boundary(
                checked,
                plans,
                boundaries,
                *target_machine,
                *target_state,
                *target_contract_report_fingerprint,
                *service_reach,
                CheckedBoundaryMachineResultPlan::Unit,
            )?;
            boundary_consumer(checked, plans, caller, operation, *target_machine)
        }
        CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            result,
            ..
        } => {
            retain_exact_flow_call(
                checked,
                caller.machine,
                caller.state,
                *coordinate,
                *target_state,
            )?;
            retain_exact_unit_boundary(
                checked,
                plans,
                boundaries,
                *target_machine,
                *target_state,
                *target_contract_report_fingerprint,
                *service_reach,
                CheckedBoundaryMachineResultPlan::Scalar(result.primitive_type),
            )?;
            boundary_consumer(checked, plans, caller, operation, *target_machine)
        }
        CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            result,
            ..
        } => {
            retain_exact_flow_call(
                checked,
                caller.machine,
                caller.state,
                *coordinate,
                *target_state,
            )?;
            let target = unique_unit_boundary(plans, *target_machine)?;
            if !matches!(
                &target.result,
                CheckedBoundaryMachineResultPlan::Structural {
                    type_identity,
                    multiplicity,
                    ..
                } if type_identity == &result.type_identity
                    && multiplicity == &result.multiplicity
            ) {
                return unsupported(
                    "Unit structural result drifted from its checked boundary target",
                );
            }
            retain_exact_unit_boundary(
                checked,
                plans,
                boundaries,
                *target_machine,
                *target_state,
                *target_contract_report_fingerprint,
                *service_reach,
                target.result.clone(),
            )?;
            boundary_consumer(checked, plans, caller, operation, *target_machine)
        }
        _ => unreachable!("only call operations reach call admission"),
    }
}

/// The Unit body a Unit or structural call names: the callee's normal result
/// and its exact entry state, contract and reach. Caller result use cannot
/// change the callee's result contract.
fn unit_target<'a>(
    checked: &CheckedTrees,
    plans: UnitPlans<'a>,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<UnitEntry<'a>, LoweringError> {
    let (CheckedUnitEffectOperationPlan::CallUnit {
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        ..
    }
    | CheckedUnitEffectOperationPlan::StructuralCall {
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        ..
    }) = operation
    else {
        return unsupported("Unit call admission requires a Unit or structural call");
    };
    let body = UnitBody::find(plans, *target_machine)?;
    structural_calls::validate_body_result(checked, operation, body.result()?)?;
    let target = body.entry()?;
    if target.state != *target_state
        || target.contract_report_fingerprint != *target_contract_report_fingerprint
        || !checked_unit_target_reach_matches(*service_reach, target.contract_service_reach)
    {
        return unsupported(
            "Unit call does not match the exact checked target state, contract, and reach",
        );
    }
    Ok(target)
}

/// A scalar call rejoins its exact flow occurrence, a callee the shared
/// scalar-call catalog resolves, and that callee's exact contract, signature
/// and reach. Structural operands and claim transfers rejoin their authored
/// sources through `scalar_structural_calls`.
fn scalar_call(
    checked: &CheckedTrees,
    caller: &CallerView<'_>,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::ScalarCall {
        coordinate,
        result,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        target_contract_commitment,
        service_reach,
        scalar_arguments,
        structural_arguments,
        claim_transfers,
        ..
    } = operation
    else {
        return unsupported("scalar call admission requires a scalar call");
    };
    let source_call = retain_exact_flow_call(
        checked,
        caller.machine,
        caller.state,
        *coordinate,
        *target_state,
    )?;
    let target = CheckedScalarCallee::find_for_unit_call(checked, *target_machine)?;
    if matches!(
        target,
        CheckedScalarCallee::Boundary(_) | CheckedScalarCallee::Structural(_)
    ) || !structural_arguments.is_empty()
        || !claim_transfers.is_empty()
    {
        scalar_structural_calls::validate_call_source(checked, caller, operation, &target)?;
    }
    let contract = checked
        .facts
        .contract_plans
        .for_machine(*target_machine)
        .ok_or(LoweringError::Unsupported(
            "ordinary Unit scalar call target has no checked contract",
        ))?;
    let target_reaches = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| {
            state.machine_symbol == *target_machine && state.state_symbol == *target_state
        })
        .map(|(_, state)| state.service_reach)
        .collect::<Vec<_>>();
    let reach_matches = match &target {
        CheckedScalarCallee::Graph(_) | CheckedScalarCallee::Structural(_) => {
            target_reaches.as_slice() == [*service_reach]
        }
        CheckedScalarCallee::Operations(body) => {
            let contract_service_reach = body.entry()?.contract_service_reach;
            // The body owns its direct effects; an ordinary call contributes
            // the published callee ceiling transitively. Rejoin each subject
            // instead of equating their summaries; the caller still retains
            // its exact source occurrence row.
            source_call.service_reach == *service_reach
                && body.retains_checked_reach(checked, &target_reaches)
                && checked
                    .facts
                    .service_reaches
                    .plan_for_machine(*target_machine)
                    == Some(contract_service_reach)
                && checked_unit_target_reach_matches(*service_reach, contract_service_reach)
        }
        CheckedScalarCallee::Boundary(plan) => {
            target_reaches.as_slice() == [plan.service_reach]
                && checked_unit_target_reach_matches(*service_reach, plan.contract_service_reach)
        }
    };
    if target.entry_state()? != *target_state
        || target.parameter_types()?.len() != scalar_arguments.len()
        || target.result_type()? != terminal_scalar_type(result.primitive_type)?
        || contract.report_fingerprint != *target_contract_report_fingerprint
        || contract.commitment != *target_contract_commitment
        || !reach_matches
    {
        return unsupported(
            "ordinary Unit scalar call disagrees with its checked target signature, contract, or reach",
        );
    }
    if matches!(
        target,
        CheckedScalarCallee::Graph(_) | CheckedScalarCallee::Structural(_)
    ) && (!checked
        .facts
        .service_reaches
        .rows
        .services(service_reach.direct)
        .is_empty()
        || !checked
            .facts
            .service_reaches
            .rows
            .services(service_reach.transitive)
            .is_empty())
    {
        return unsupported(
            "ordinary Unit scalar call with services requires scalar service lowering",
        );
    }
    Ok(())
}

/// A boundary call rejoins how its structural operands' producers are
/// consumed, as a Unit call does. Boundary targets declare no entry claims.
fn boundary_consumer(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    caller: &CallerView<'_>,
    operation: &CheckedUnitEffectOperationPlan,
    target_machine: SymbolHandle,
) -> Result<(), LoweringError> {
    structural_calls::validate_consumer(
        checked,
        caller,
        operation,
        &unique_unit_boundary(plans, target_machine)?.structural_parameters,
        &[],
    )
}
