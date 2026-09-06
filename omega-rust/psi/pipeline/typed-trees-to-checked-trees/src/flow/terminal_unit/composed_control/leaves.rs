//! One exact ordinary or boundary Unit call followed by Unit return.

use super::*;

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    boundaries: &[CheckedBoundaryMachinePlan],
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
) -> Option<CheckedComposedUnitControlStatePlan> {
    let [statement] = program.statement_table.statements(state.statement_nodes) else {
        return None;
    };
    if !matches!(statement, StatementNode::Call(_))
        && super::super::control::tail_call(program, state, 0).is_none()
    {
        return None;
    }
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = super::super::control::outer_calls(
        program,
        facts,
        machine.symbol,
        state,
        facts.flow.control.calls.span(flow.calls)?,
    )?;
    let [call] = calls.as_slice() else {
        return None;
    };
    if call.statement_index != 0 || call.call_ordinal != 0 {
        return None;
    }
    let operation = build_call_operation(
        program,
        facts,
        machine,
        state,
        structural_parameters,
        &[],
        &[],
        entry_claims,
        call,
        false,
        None,
        &[],
    )?;
    match &operation {
        CheckedUnitEffectOperationPlan::BoundaryCall {
            target_machine,
            structural_arguments,
            completion_receipts,
            ..
        } => {
            let matching_boundaries = boundaries
                .iter()
                .filter(|plan| plan.machine == *target_machine)
                .collect::<Vec<_>>();
            let [boundary] = matching_boundaries.as_slice() else {
                return None;
            };
            if !exact_boundary_custody(
                structural_parameters,
                entry_claims,
                structural_arguments,
                completion_receipts,
                &boundary.structural_parameters,
            ) || !boundary.domain_requirements.is_empty()
                || !boundary.result.is_unit()
            {
                return None;
            }
        }
        CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        } if structural_parameters.is_empty()
            && entry_claims.is_empty()
            && structural_arguments.is_empty()
            && claim_transfers.is_empty() => {}
        _ => return None,
    }
    Some(CheckedComposedUnitControlStatePlan {
        state: state.symbol,
        structural_parameters: structural_parameters.to_vec(),
        scalar_parameters: Vec::new(),
        entry_claims: entry_claims.to_vec(),
        bindings: Vec::new(),
        binding_initializers: Vec::new(),
        operations: vec![operation],
        terminator: CheckedComposedUnitControlTerminatorPlan::ReturnUnit,
    })
}

fn exact_boundary_custody(
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    arguments: &[CheckedUnitStructuralArgumentPlan],
    receipts: &[CheckedUnitClaimTransferPlan],
    boundary_parameters: &[CheckedUnitStructuralParameterPlan],
) -> bool {
    if caller_parameters.is_empty() {
        return entry_claims.is_empty()
            && arguments.is_empty()
            && receipts.is_empty()
            && boundary_parameters.is_empty();
    }
    matches!(
        (caller_parameters, entry_claims, arguments, receipts, boundary_parameters),
        ([caller], [claim], [argument], [receipt], [boundary])
            if !caller.is_self
                && caller.multiplicity == Multiplicity::Linear
                && caller.access == CheckedStructuralAccess::Owned
                && caller.qualifications.is_empty()
                && claim.parameter_index == 0
                && claim.path.is_empty()
                && claim.carry == CarryPolicy::STRICT
                && argument.source_parameter_index() == Some(0)
                && argument.path.is_empty()
                && argument.type_identity == caller.type_identity
                && argument.access == CheckedStructuralAccess::Owned
                && argument.byte_sequence_literal().is_none()
                && receipt.claim_identity == claim.claim_identity
                && receipt.argument_index == 0
                && boundary.type_identity == caller.type_identity
                && boundary.multiplicity == Multiplicity::Linear
                && boundary.access == CheckedStructuralAccess::Owned
                && boundary.qualifications.is_empty()
    )
}
