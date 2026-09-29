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

/// The shared call and callee shape every payloadless structural call must
/// have: a bare `CallStructural` operation against a contract-free, parameter-
/// free callee, with an Unrestricted structural result that mirrors the
/// callee's declaration. The callee's own exits stay unexamined; each
/// admission family (case returns, literal returns) supplies its own exit
/// predicate.
fn exact_payloadless_call_shape<'a>(
    module: &TerminalModule,
    operation: &terminal_psi::Operation,
    machines: &'a BTreeMap<MachineId, &'a TerminalMachine>,
) -> Option<&'a TerminalMachine> {
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
        return None;
    };
    let result = operation.result.structural()?;
    let callee = machines.get(callee).copied()?;
    let callee_result = callee.result.structural()?;
    (callee.parameters.is_empty()
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
        }))
    .then_some(callee)
}

pub(crate) fn is_exact_payloadless_structural_call(
    module: &TerminalModule,
    operation: &terminal_psi::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> bool {
    exact_payloadless_call_shape(module, operation, machines)
        .is_some_and(|callee| exact_payloadless_case_return_exits(callee).is_some())
}

/// A payloadless callee whose exits return byte-sequence literal views: every
/// `ReturnStructural` block sources a literal-established place rather than a
/// payloadless case. Literal destinations are module-static storage, so the
/// returned Unrestricted view leaves nothing on loan, exactly like the case
/// family.
pub(crate) fn is_exact_payloadless_literal_call(
    module: &TerminalModule,
    operation: &terminal_psi::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> bool {
    exact_payloadless_call_shape(module, operation, machines).is_some_and(literal_return_exits)
}

fn literal_return_exits(machine: &TerminalMachine) -> bool {
    let Some(result) = machine.result.structural() else {
        return false;
    };
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
        return false;
    }
    let mut exits = 0usize;
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
            return false;
        }
        let Some(place) = machine
            .structural_places
            .iter()
            .find(|place| place.id == *source)
        else {
            return false;
        };
        let StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        } = place.kind
        else {
            return false;
        };
        if structural_type != result.structural_type {
            return false;
        }
        exits += 1;
    }
    exits != 0
}

pub(crate) fn exact_payloadless_structural_call(
    module: &TerminalModule,
    operation: &terminal_psi::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> bool {
    is_exact_payloadless_structural_call(module, operation, machines)
}
