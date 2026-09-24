//! Exact payloadless structural calls and case-return exits.

use crate::validation::{
    BTreeMap, BlockId, MachineId, OperationKind, StructuralMultiplicity, StructuralPlaceKind,
    TerminalMachine, TerminalModule, Terminator, propositions,
};

pub(crate) fn exact_payloadless_case_return_exits(
    machine: &TerminalMachine,
) -> Option<BTreeMap<BlockId, terminal_psi::OutcomeSpecificGuard>> {
    let result = machine.result.structural()?;
    if !crate::validation::structural::result_contracts::has_empty_qualification_rosters(
        &result.qualifications,
        &result.projected_qualifications,
    ) || result.multiplicity != StructuralMultiplicity::Unrestricted
        || machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Call { .. }
                        | OperationKind::CallUnit { .. }
                        | OperationKind::CallStructuralScalar { .. }
                        | OperationKind::CallDynamicScalar { .. }
                        | OperationKind::CallDynamicParameterScalar { .. }
                        | OperationKind::CallStructural { .. }
                        | OperationKind::CallStructuralWithScalarArguments { .. }
                        | OperationKind::BoundaryCall { .. }
                )
            })
    {
        return None;
    }
    let mut exits = BTreeMap::new();
    for block in &machine.blocks {
        let Terminator::ReturnStructural {
            source,
            returned_claims,
            ..
        } = &block.terminator
        else {
            continue;
        };
        if !returned_claims.is_empty() {
            return None;
        }
        let producer = machine.structural_places.iter().find_map(|place| {
            (place.id == *source)
                .then_some(place.kind)
                .and_then(|kind| match kind {
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } if structural_type == result.structural_type => Some(producer),
                    _ => None,
                })
        })?;
        let operation = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| operation.id == producer)?;
        let operation_result = operation.result.structural()?;
        let OperationKind::EstablishScalarCase {
            result_case,
            ref fields,
        } = operation.kind
        else {
            return None;
        };
        if !fields.is_empty() {
            return None;
        }
        if operation_result.place != *source
            || operation_result.structural_type != result.structural_type
            || operation_result.multiplicity != StructuralMultiplicity::Unrestricted
            || !operation_result.claims.is_empty()
            || !crate::validation::structural::result_contracts::has_empty_qualification_rosters(
                &operation_result.qualifications,
                &operation_result.projected_qualifications,
            )
        {
            return None;
        }
        exits.insert(
            block.id,
            terminal_psi::OutcomeSpecificGuard {
                result_type: result.structural_type,
                result_case,
            },
        );
    }
    (!exits.is_empty()).then_some(exits)
}

pub(crate) fn is_exact_payloadless_structural_call(
    module: &TerminalModule,
    operation: &terminal_psi::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> bool {
    let OperationKind::CallStructural {
        callee,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        selected_evidence: _,
    } = &operation.kind
    else {
        return false;
    };
    let Some(result) = operation.result.structural() else {
        return false;
    };
    let Some(callee) = machines.get(callee).copied() else {
        return false;
    };
    let Some(callee_result) = callee.result.structural() else {
        return false;
    };
    callee.parameters.is_empty()
        && callee.structural_parameters.is_empty()
        && callee.entry_claims.is_empty()
        && callee.content_entry_claims.is_empty()
        && callee.contract.requires.is_empty()
        && callee.contract.ensures.is_empty()
        && callee.contract.crash_routes.is_empty()
        && module
            .evidence_contract_lanes
            .iter()
            .all(|lane| lane.machine != callee.id)
        && structural_arguments.is_empty()
        && claim_transfers.is_empty()
        && returned_claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && result.structural_type == callee_result.structural_type
        && result.multiplicity == StructuralMultiplicity::Unrestricted
        && result.multiplicity == callee_result.multiplicity
        && crate::validation::structural::result_contracts::has_empty_qualification_rosters(
            &result.qualifications,
            &result.projected_qualifications,
        )
        && crate::validation::structural::result_contracts::call_result_matches(
            result,
            callee_result,
        )
        && result.claims.is_empty()
        && callee.contract.outcome_specific_ensures.iter().all(|row| {
            propositions::proposition_boolean_field_roots(&row.proposition)
                .into_iter()
                .chain(propositions::proposition_content_roots(&row.proposition))
                .all(|root| root == callee_result.place)
        })
        && exact_payloadless_case_return_exits(callee).is_some()
}

pub(crate) fn exact_payloadless_structural_call(
    module: &TerminalModule,
    operation: &terminal_psi::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> bool {
    is_exact_payloadless_structural_call(module, operation, machines)
}
