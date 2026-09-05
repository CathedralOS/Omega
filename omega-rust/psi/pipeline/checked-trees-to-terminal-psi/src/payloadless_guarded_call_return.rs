//! Exact zero-input payloadless guarded call returned through identity arms.

use super::*;

pub(super) fn lower_payloadless_guarded_call_return_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedPayloadlessGuardedCallReturnMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let target_plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .payloadless_case_for_machine(plan.target_machine)
        .ok_or(LoweringError::Unsupported(
            "guarded payloadless call target has no exact constructor plan",
        ))?;
    if target_plan.state != plan.target_state
        || plan.call.statement_index != 0
        || plan.call.call_ordinal != 0
        || plan.result != target_plan.result
        || plan.result.multiplicity != language_semantics::Multiplicity::Unrestricted
        || !plan.result.qualifications.is_empty()
        || plan.attachment_type_identity != target_plan.attachment_type_identity
    {
        return unsupported("guarded payloadless call plan is outside the exact checked shape");
    }

    let mut lowered = lower_payloadless_case_return_machine(checked, target_plan)?;
    let module = &mut lowered.semantic_module;
    let [callee] = module.machines.as_mut_slice() else {
        return unsupported("guarded payloadless call target did not lower to one machine");
    };
    callee.id = machine_id(2);
    callee.entry = block_id(2);
    callee.blocks[0].id = block_id(2);
    callee.contract.id = contract_id(2);
    for place in &mut callee.structural_places {
        match &mut place.kind {
            StructuralPlaceKind::OperationResult { producer, .. } => {
                place.id = place_id(3);
                *producer = operation_id(2);
            }
            StructuralPlaceKind::Result => place.id = place_id(4),
            _ => {
                return unsupported(
                    "guarded payloadless callee retained an unexpected structural place",
                );
            }
        }
    }
    let [operation] = callee.blocks[0].operations.as_mut_slice() else {
        return unsupported("guarded payloadless callee has no exact constructor");
    };
    operation.id = operation_id(2);
    let terminal_psi::OperationResult::Structural(result) = &mut operation.result else {
        return unsupported("guarded payloadless constructor result is not structural");
    };
    result.place = place_id(3);
    let Terminator::ReturnStructural { edge, source, .. } = &mut callee.blocks[0].terminator else {
        return unsupported("guarded payloadless callee has no exact return");
    };
    *edge = edge_id(2);
    *source = place_id(3);
    let TerminalMachineResult::Structural(callee_result) = &mut callee.result else {
        return unsupported("guarded payloadless callee result is not structural");
    };
    callee_result.place = place_id(4);

    let result_type = callee_result.structural_type;
    let attachment = callee.attachment.ok_or(LoweringError::Unsupported(
        "guarded payloadless callee attachment is absent",
    ))?;
    let caller = TerminalMachine {
        id: machine_id(1),
        attachment: Some(attachment),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: Vec::new(),
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: place_id(2),
            structural_type: result_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }),
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: place_id(1),
                kind: StructuralPlaceKind::OperationResult {
                    producer: operation_id(1),
                    structural_type: result_type,
                },
            },
            StructuralPlaceDeclaration {
                id: place_id(2),
                kind: StructuralPlaceKind::Result,
            },
        ],
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation_id(1),
                result: terminal_psi::OperationResult::Structural(
                    terminal_psi::StructuralOperationResult {
                        place: place_id(1),
                        structural_type: result_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    },
                ),
                kind: OperationKind::CallStructural {
                    callee: machine_id(2),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    returned_claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                    selected_evidence: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnStructural {
                edge: edge_id(1),
                source: place_id(1),
                returned_claims: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    module.entry = caller.id;
    module.machines.insert(0, caller);
    if plan
        .selected_evidence
        .iter()
        .any(|selection| selection.tail_use.is_some())
    {
        module.machines.push(TerminalMachine {
            id: machine_id(3),
            attachment: Some(attachment),
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: place_id(5),
                position: 0,
                is_self: false,
                structural_type: result_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                place: place_id(6),
                structural_type: result_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }),
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: place_id(5),
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                },
                StructuralPlaceDeclaration {
                    id: place_id(6),
                    kind: StructuralPlaceKind::Result,
                },
            ],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(3),
            blocks: vec![Block {
                id: block_id(3),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnStructural {
                    edge: edge_id(3),
                    source: place_id(5),
                    returned_claims: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: contract_id(3),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        });
    }
    Ok(lowered)
}
