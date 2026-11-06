//! A program checked without running still needs its saved-reference checks.
//! Resolve each requested call through the same implementation and target choices
//! as execution. Missing or opaque supplies cannot prove that its data stays valid.

use crate::{
    AdmittedProviderSelections, AdmittedTargetMachineSelections, admitted_boundary_member,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};
use terminal_codec::TerminalArtifactView;
use terminal_psi::{OperationKind, TargetMachineCallable, TerminalModule};

/// Check only explicitly requested duties. The requests select original
/// subjects at their own call cuts; neither a declaration row nor a selected
/// body's input qualification supplies their normal-return proof.
pub fn require_selected_lender_preservation(
    artifact: TerminalArtifactView<'_>,
    providers: &AdmittedProviderSelections,
    targets: &AdmittedTargetMachineSelections,
) -> Result<(), String> {
    if providers.terminal_psi() != artifact.identity()
        || targets.terminal_psi() != artifact.identity()
    {
        return Err("lender preservation choices belong to another artifact".into());
    }
    let module = artifact.module();
    for boundary in requested_boundaries(module)? {
        let assumption = providers
            .assumed_bindings()
            .iter()
            .find(|binding| binding.required == boundary);
        let installed = |required| {
            providers
                .providers()
                .find_map(|(found, body)| (found == required).then_some(body))
        };
        let installed = assumption
            .map(|binding| binding.implementation)
            .or_else(|| installed(boundary));
        let member = admitted_boundary_member(module, Some(targets), boundary)?;
        // Final native call selection refuses competing installed and family
        // supplies. Retain that refusal here instead of proving one body's
        // writes and relying on a different dispatch order at execution.
        if installed.is_some() && member.is_some() {
            return Err(
                "lender preservation has competing installed and target-family supplies".into(),
            );
        }
        let implementation = match member {
            Some(TargetMachineCallable::Machine(body)) => Some(body),
            _ => installed,
        };
        terminal_semantics::descriptor_results::require_boundary_lender_preservation(
            module,
            boundary,
            implementation,
        )
        .map_err(|reason| format!("lender preservation at {boundary:?}: {reason}"))?;
    }
    Ok(())
}

fn requested_calls(module: &TerminalModule) -> impl Iterator<Item = (MachineId, OperationId)> + '_ {
    module
        .boundary_lender_preservation
        .iter()
        .map(|demand| (demand.machine, demand.operation))
        .chain(
            module
                .boundary_reference_forwarding
                .iter()
                .map(|demand| (demand.machine, demand.operation)),
        )
}

struct RequestedCall {
    machine: MachineId,
    operation: OperationId,
    caller_found: bool,
    matching_operations: usize,
    boundary: Option<BoundaryMachineId>,
}

/// A call can carry several duties. Find its operation once while keeping every
/// original duty for the selected body's proof. Sparse IDs name sorted rows;
/// they never determine an allocation size.
fn requested_boundaries(module: &TerminalModule) -> Result<Vec<BoundaryMachineId>, String> {
    let mut keys = requested_calls(module).collect::<Vec<_>>();
    keys.sort_unstable();
    keys.dedup();
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let mut calls = keys
        .into_iter()
        .map(|(machine, operation)| RequestedCall {
            machine,
            operation,
            caller_found: false,
            matching_operations: 0,
            boundary: None,
        })
        .collect::<Vec<_>>();
    for caller in &module.machines {
        let start = calls.partition_point(|call| call.machine < caller.id);
        let end = calls.partition_point(|call| call.machine <= caller.id);
        if start == end || calls[start].caller_found {
            continue;
        }
        for call in &mut calls[start..end] {
            call.caller_found = true;
        }
        for operation in caller.operations() {
            if let Ok(index) =
                calls[start..end].binary_search_by_key(&operation.id, |call| call.operation)
            {
                let call = &mut calls[start + index];
                call.matching_operations += 1;
                if let OperationKind::BoundaryCall { boundary, .. } = operation.kind {
                    call.boundary = Some(boundary);
                }
            }
        }
    }
    let mut boundaries = Vec::new();
    // Check the original request order so a malformed earlier duty keeps its
    // existing refusal, including when the other request list has a lower ID.
    for key in requested_calls(module) {
        let index = calls
            .binary_search_by_key(&key, |call| (call.machine, call.operation))
            .map_err(|_| "a lender request has no operation")?;
        let call = &calls[index];
        if !call.caller_found {
            return Err("a lender request has no caller".into());
        }
        if call.matching_operations == 0 {
            return Err("a lender request has no operation".into());
        }
        if call.matching_operations > 1 {
            return Err("a lender request has ambiguous operations".into());
        }
        boundaries.push(
            call.boundary
                .ok_or("a lender request does not name a boundary call")?,
        );
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    Ok(boundaries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{BlockId, ContractId, EdgeId, ServiceId, StructuralDomainId};
    use terminal_psi::{
        Block, BoundaryLenderPreservationDemand, BoundaryReferenceForwardingDemand,
        BoundaryReferenceSource, MachineContract, Operation, OperationResult,
        StructuralDomainRequirement, TerminalMachine, TerminalMachineResult, Terminator,
    };

    fn machine(id: u64, operations: Vec<Operation>) -> TerminalMachine {
        let block = BlockId::new(1).unwrap();
        let mut entry_block = Block::plain(
            block,
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        );
        entry_block.operations = operations;
        TerminalMachine {
            assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
            id: MachineId::new(id).unwrap(),
            declaration: None,
            attachment: None,
            nominal_cleanup_subject: None,
            parameters: Vec::new(),
            parameter_order: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            declared_service_reach: Vec::new(),
            closed_reach_application: None,
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![entry_block],
            contract: MachineContract::empty(ContractId::new(id).unwrap()),
        }
    }

    fn call(operation: u64, boundary: u64) -> Operation {
        Operation::unsuspended(
            OperationId::new(operation).unwrap(),
            OperationResult::Unit,
            OperationKind::BoundaryCall {
                boundary: BoundaryMachineId::new(boundary).unwrap(),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
                requirement_obligations: Vec::new(),
            },
        )
    }

    fn lender(machine: u64, operation: u64, domain: u64) -> BoundaryLenderPreservationDemand {
        BoundaryLenderPreservationDemand {
            machine: MachineId::new(machine).unwrap(),
            operation: OperationId::new(operation).unwrap(),
            requirement: StructuralDomainRequirement {
                argument_index: 0,
                path: Vec::new(),
                domain: StructuralDomainId::new(domain).unwrap(),
            },
        }
    }

    fn forwarding(machine: u64, operation: u64) -> BoundaryReferenceForwardingDemand {
        BoundaryReferenceForwardingDemand {
            machine: MachineId::new(machine).unwrap(),
            operation: OperationId::new(operation).unwrap(),
            output: 0,
            sources: vec![BoundaryReferenceSource::Reference { argument_index: 0 }],
        }
    }

    #[test]
    fn requested_calls_keep_all_duties_and_sparse_identities() {
        let mut module = TerminalModule::for_entry(MachineId::new(17).unwrap());
        module.machines = vec![
            machine(17, vec![call(5, 1), call(6, 2)]),
            machine(u64::MAX - 1, vec![call(u64::MAX, 3)]),
        ];
        module.boundary_lender_preservation = vec![lender(17, 5, 1), lender(17, 5, 2)];
        module.boundary_reference_forwarding = vec![
            forwarding(17, 5),
            forwarding(17, 6),
            forwarding(u64::MAX - 1, u64::MAX),
        ];
        let original_lenders = module.boundary_lender_preservation.clone();
        let original_forwarding = module.boundary_reference_forwarding.clone();
        assert_eq!(
            requested_boundaries(&module).unwrap(),
            [1, 2, 3]
                .map(|id| BoundaryMachineId::new(id).unwrap())
                .to_vec()
        );
        assert_eq!(module.boundary_lender_preservation, original_lenders);
        assert_eq!(module.boundary_reference_forwarding, original_forwarding);
    }

    #[test]
    fn requested_calls_still_refuse_missing_ambiguous_and_noncall_operations() {
        let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
        module.machines.push(machine(1, vec![call(1, 1)]));
        module.boundary_lender_preservation.push(lender(2, 1, 1));
        assert_eq!(
            requested_boundaries(&module).unwrap_err(),
            "a lender request has no caller"
        );
        module.boundary_lender_preservation[0] = lender(1, 2, 1);
        assert_eq!(
            requested_boundaries(&module).unwrap_err(),
            "a lender request has no operation"
        );
        module.boundary_lender_preservation[0] = lender(1, 1, 1);
        module.machines[0].blocks[0].operations.push(call(1, 1));
        assert_eq!(
            requested_boundaries(&module).unwrap_err(),
            "a lender request has ambiguous operations"
        );
        module.machines[0].blocks[0].operations.pop();
        module.machines[0].blocks[0].operations[0].kind = OperationKind::ReadTaskRegister {
            service: ServiceId::new(1).unwrap(),
        };
        assert_eq!(
            requested_boundaries(&module).unwrap_err(),
            "a lender request does not name a boundary call"
        );
    }

    #[test]
    fn requested_calls_keep_the_original_refusal_order() {
        let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
        let mut not_a_call = call(1, 1);
        not_a_call.kind = OperationKind::ReadTaskRegister {
            service: ServiceId::new(1).unwrap(),
        };
        module.machines.push(machine(1, vec![not_a_call]));
        module.boundary_lender_preservation.push(lender(9, 1, 1));
        module.boundary_reference_forwarding.push(forwarding(1, 1));
        assert_eq!(
            requested_boundaries(&module).unwrap_err(),
            "a lender request has no caller"
        );
    }
}
