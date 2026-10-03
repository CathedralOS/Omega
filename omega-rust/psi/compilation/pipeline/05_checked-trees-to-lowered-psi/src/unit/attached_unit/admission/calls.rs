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
    CheckedUnitCallResult, CheckedUnitCallRoute, CheckedUnitEntryClaimPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralReturnPlan,
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
    // A member call shares its construction's statement custody: the
    // construction's source replay rejoins the member's occurrence and
    // result as part of the value. Only its target is admitted here, and an
    // operand naming one of the caller's earlier result bindings (a `let`
    // local the call reads or takes) rejoins its authored source through the
    // shared result-use check, so it cannot stand in for another local.
    if let CheckedUnitEffectOperationPlan::EstablishStructuralValue { calls, .. } = operation {
        for call in calls {
            let member = call.operation();
            super::super::argument_schedule::validate_operand_preparation(
                checked,
                caller.machine,
                caller.state,
                call.operands(),
                member,
            )?;
            let mut operations = caller.operations.to_vec();
            operations.extend(call.operands().iter().cloned());
            operations.push(member.clone());
            let caller = CallerView {
                operations: &operations,
                ..*caller
            };
            for operand in call.operands().iter() {
                match operand {
                    CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. } => {
                        crate::expression_preparation::source_custody::structural::validate(
                            checked,
                            caller.machine,
                            caller.state,
                            operand,
                        )?;
                    }
                    CheckedUnitEffectOperationPlan::EstablishScalarArray {
                        source,
                        result,
                        elements,
                    } => {
                        super::super::scalar_arrays::validate(
                            checked,
                            caller.machine,
                            caller.state,
                            *source,
                            result,
                            elements,
                        )?;
                    }
                    CheckedUnitEffectOperationPlan::Call(_) => {}
                    _ => {
                        return unsupported(
                            "structural call preparation contains another operation",
                        );
                    }
                }
                admit(checked, plans, &caller, operand, boundaries)?;
            }
            let target = unit_target(checked, plans, member)?;
            if member.call_structural_arguments().iter().any(|argument| {
                argument
                    .source_structural_result_binding_ordinal()
                    .is_some()
            }) {
                structural_calls::validate_consumer(
                    checked,
                    &caller,
                    member,
                    target.structural_parameters,
                    target.entry_claims,
                )?;
            }
        }
        return Ok(());
    }
    let Some(planned) = operation.call() else {
        return Ok(());
    };
    crate::emission::call_source_custody::validate_operation(
        checked,
        caller.machine,
        caller.state,
        operation,
        caller.structural_parameters,
    )?;
    structural_calls::validate_custody(checked, caller.machine, caller.state, operation)?;
    match (planned.route(), planned.result()) {
        // A structural call whose target is not a Unit body is a claim-free
        // affine leaf with its own checked return plan.
        (CheckedUnitCallRoute::Module { .. }, CheckedUnitCallResult::Structural { .. })
            if !UnitBody::contains(plans, planned.target.machine) =>
        {
            structural_calls::validate(checked, caller, operation)
        }
        (
            CheckedUnitCallRoute::Module { .. },
            CheckedUnitCallResult::Unit | CheckedUnitCallResult::Structural { .. },
        ) => {
            retain_exact_flow_call(
                checked,
                caller.machine,
                caller.state,
                planned.coordinate,
                planned.target.state,
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
        (CheckedUnitCallRoute::Module { .. }, CheckedUnitCallResult::Scalar(_)) => {
            scalar_call(checked, caller, operation)
        }
        // The three boundary calls differ only in the result their boundary
        // must declare.
        (CheckedUnitCallRoute::Boundary, _) => {
            retain_exact_flow_call(
                checked,
                caller.machine,
                caller.state,
                planned.coordinate,
                planned.target.state,
            )?;
            let expected = match planned.result() {
                CheckedUnitCallResult::Unit => CheckedBoundaryMachineResultPlan::Unit,
                CheckedUnitCallResult::Scalar(result) => {
                    CheckedBoundaryMachineResultPlan::Scalar(result.primitive_type)
                }
                CheckedUnitCallResult::Structural {
                    binding: result, ..
                } => {
                    let target = unique_unit_boundary(plans, planned.target.machine)?;
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
                    target.result.clone()
                }
            };
            retain_exact_unit_boundary(
                checked,
                plans,
                boundaries,
                planned.target.machine,
                planned.target.state,
                planned.target.contract_report_fingerprint,
                planned.target.service_reach,
                expected,
            )?;
            boundary_consumer(checked, plans, caller, operation, planned.target.machine)
        }
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
    // An ordinary call with a Unit or structural result.
    let Some(call) = operation
        .call()
        .filter(|call| !call.is_boundary() && call.scalar_result().is_none())
    else {
        return unsupported("Unit call admission requires a Unit or structural call");
    };
    let body = UnitBody::find(plans, call.target.machine)?;
    structural_calls::validate_body_result(checked, operation, body.result()?)?;
    let target = body.entry()?;
    if target.state != call.target.state
        || target.contract_report_fingerprint != call.target.contract_report_fingerprint
        || !checked_unit_target_reach_matches(
            call.target.service_reach,
            target.contract_service_reach,
        )
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
    let Some((call, result)) = operation
        .call()
        .filter(|call| !call.is_boundary())
        .and_then(|call| Some((call, call.scalar_result()?)))
    else {
        return unsupported("scalar call admission requires a scalar call");
    };
    let coordinate = &call.coordinate;
    let target_machine = &call.target.machine;
    let target_state = &call.target.state;
    let target_contract_report_fingerprint = &call.target.contract_report_fingerprint;
    let service_reach = &call.target.service_reach;
    let scalar_arguments = &call.scalar_arguments;
    let structural_arguments = &call.structural_arguments;
    let claim_transfers = call.claim_transfers();
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
        || Some(contract.commitment) != call.contract_commitment()
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
