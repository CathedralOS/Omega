//! Guarded payloadless call returned through its exhaustive identity arms.
//!
//! Every arm of the caller's case dispatch returns the saved call result
//! unchanged, so the caller's runtime is the call followed by the return; the
//! arms survive only as the selected-evidence rows
//! `proofs::evidence_lowering::guarded_call_evidence` installs on that call.
//! An arm that hands its selected witness to a tail state keeps that state as
//! a proof-only target machine: the verifier replays the tail requirement
//! against it, and no runtime edge reaches it.
//!
//! The callee is an ordinary Unit-effect machine, not a body of this family.
//! It lowers through the shared Unit closure with the caller reserved as the
//! external entry, so the closure numbers the caller machine 1 and the callee
//! machine 2, owns the type catalog, and hands back its identity counters. The
//! caller and the tail target continue from those counters instead of
//! renumbering a separately lowered callee.

use super::{
    Block, CheckedTrees, LoweredPsi, LoweringError, MachineContract, Operation, OperationKind,
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralResultDeclaration, TerminalMachine,
    TerminalMachineResult, Terminator, block_id, contract_id, edge_id, lookup_type_id, machine_id,
    operation_id, place_id, unsupported,
};
use crate::unit::attached_unit::shared_closure::ExternalUnitRoots;
use crate::unit::attached_unit::{UnitClosureRequest, lower_unit_closure};

pub(crate) fn lower_payloadless_guarded_call_return_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedPayloadlessGuardedCallReturnMachinePlan,
) -> Result<LoweredPsi, LoweringError> {
    if plan.call.statement_index != 0
        || plan.call.call_ordinal != 0
        || plan.result.multiplicity != language_semantics::Multiplicity::Unrestricted
        || !plan.result.qualifications.is_empty()
    {
        return unsupported("guarded payloadless call plan is outside the exact checked shape");
    }

    let closure = lower_unit_closure(
        checked,
        &UnitClosureRequest {
            external: Some(ExternalUnitRoots {
                boundary_roots: &[],
                structural_type_roots: &[],
                service_roots: &[],
                scalar_roots: &[],
            }),
            ..UnitClosureRequest::unit(checked, plan.machine, &[plan.target_machine])
        },
    )?;
    let caller_id = machine_id(1);
    let callee_id = machine_id(2);
    let tail_id = machine_id(3);
    // The evidence rows address the caller, callee and tail target by these
    // fixed identities, so the callee must close over no further machine.
    if closure.machine_ids != [(plan.machine, caller_id), (plan.target_machine, callee_id)] {
        return unsupported("guarded payloadless callee does not lower as one closure machine");
    }
    let result_type = lookup_type_id(&closure.type_ids, &plan.result.type_identity)?;
    let attachment = lookup_type_id(&closure.type_ids, &plan.attachment_type_identity)?;
    let mut lowered = closure.lowered;
    let [callee] = lowered.semantic_module.machines.as_slice() else {
        return unsupported("guarded payloadless callee did not lower to one machine");
    };
    if callee.id != callee_id
        || callee.attachment != Some(attachment)
        || !callee.parameters.is_empty()
        || !callee.structural_parameters.is_empty()
        || callee.result.structural().is_none_or(|result| {
            result.structural_type != result_type
                || result.multiplicity != StructuralMultiplicity::Unrestricted
                || !result.qualifications.is_empty()
        })
    {
        return unsupported("guarded payloadless callee signature disagrees with its caller");
    }

    let mut next_place = closure.next_place;
    let mut next_block = closure.next_block;
    let mut next_operation = closure.next_operation;
    let mut next_edge = closure.next_edge;
    let call_result = place_id(take(&mut next_place)?);
    let caller_result = place_id(take(&mut next_place)?);
    let call = operation_id(take(&mut next_operation)?);
    let caller_block = block_id(take(&mut next_block)?);
    let caller = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: caller_id,
        attachment: Some(attachment),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: Vec::new(),
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            reference_sources: Vec::new(),
            place: caller_result,
            structural_type: result_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }),
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: call_result,
                kind: StructuralPlaceKind::OperationResult {
                    producer: call,
                    structural_type: result_type,
                },
            },
            StructuralPlaceDeclaration {
                id: caller_result,
                kind: StructuralPlaceKind::Result,
            },
        ],
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: caller_block,
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: caller_block,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: call,
                result: terminal_psi::OperationResult::Structural(
                    terminal_psi::StructuralOperationResult {
                        qualification_establishments: Vec::new(),
                        place: call_result,
                        structural_type: result_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    },
                ),
                kind: OperationKind::CallStructural {
                    callee: callee_id,
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    returned_claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                    selected_evidence: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnStructural {
                edge: edge_id(take(&mut next_edge)?),
                source: call_result,
                returned_claims: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(caller_id),
    };
    lowered.semantic_module.entry = caller_id;
    lowered.semantic_module.machines.insert(0, caller);
    if plan
        .selected_evidence
        .iter()
        .any(|selection| selection.tail_use.is_some())
    {
        let parameter = place_id(take(&mut next_place)?);
        let tail_result = place_id(take(&mut next_place)?);
        let tail_block = block_id(take(&mut next_block)?);
        lowered.semantic_module.machines.push(TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: tail_id,
            attachment: Some(attachment),
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: parameter,
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
                reference_sources: Vec::new(),
                place: tail_result,
                structural_type: result_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }),
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: parameter,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                },
                StructuralPlaceDeclaration {
                    id: tail_result,
                    kind: StructuralPlaceKind::Result,
                },
            ],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: tail_block,
            blocks: vec![Block {
                structural_parameters: Vec::new(),
                id: tail_block,
                parameters: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnStructural {
                    edge: edge_id(take(&mut next_edge)?),
                    source: parameter,
                    returned_claims: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: empty_contract(tail_id),
        });
    }
    Ok(lowered)
}

fn empty_contract(machine: semantic_vocabulary::MachineId) -> MachineContract {
    MachineContract {
        id: contract_id(machine.get()),
        crash_routes: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

/// The next identity from one of the closure's shared counters.
fn take(next: &mut u64) -> Result<u64, LoweringError> {
    let identity = *next;
    *next = identity.checked_add(1).ok_or(LoweringError::Unsupported(
        "guarded payloadless caller identities overflow",
    ))?;
    Ok(identity)
}
