//! The Terminal adapter reads current executable rows once. The shared reader
//! receives normalized data and a current graph, never a callback or old source.
use super::*;

pub(super) fn validate(module: &TerminalModule) -> Result<(), &'static str> {
    for invocation in &module.proof_output_calls {
        if invocation.runtime_call.is_some_and(|call| {
            matches!(
                call.site,
                semantic_vocabulary::InvocationSite::ExternalCompletion(_)
            )
        }) {
            return Err("ordinary evidence invocation names an external completion");
        }
        let no_normal =
            invocation.runtime_result == Some(terminal_psi::ProofOutputRuntimeResult::NoNormal);
        let edge = invocation.runtime_call.is_some_and(|call| {
            matches!(
                call.site,
                semantic_vocabulary::InvocationSite::InvokeNoNormal(_)
            )
        });
        if no_normal || edge {
            if !no_normal || !edge || !invocation.outputs.is_empty() {
                return Err("nonreturning evidence invocation has a normal result or output");
            }
            let retained = invocation
                .runtime_call
                .ok_or("nonreturning evidence invocation has no call")?;
            let caller = module
                .machine(invocation.caller)
                .ok_or("nonreturning evidence invocation has no caller")?;
            let mut calls = caller
                .module_invocations()
                .filter(|call| call.site == retained.site);
            if calls
                .next()
                .is_none_or(|call| call.inputs.callee != retained.callee)
                || calls.next().is_some()
            {
                return Err("nonreturning evidence invocation has no exact executable edge");
            }
        }
    }
    let consumes_snapshots = module
        .proof_output_calls
        .iter()
        .any(|call| call.runtime_call.is_some() && !call.evidence_arguments.is_empty());
    // Empty metadata needs no adapter. Every nonempty declaration roster,
    // including unused Ensures, reaches the common owner below.
    if module.evidence_contract_lanes.is_empty() && !consumes_snapshots {
        return Ok(());
    }
    let checks_calls = consumes_snapshots
        || module
            .evidence_contract_lanes
            .iter()
            .any(|lane| lane.kind == EvidenceContractLaneKind::Requires);
    let graph = if consumes_snapshots {
        descriptor_results::terminal_graph(module)
    } else {
        Vec::new()
    };
    let mut bodies = Vec::with_capacity(module.machines.len());
    for machine in &module.machines {
        let observes = module.proof_output_calls.iter().any(|call| {
            call.caller == machine.id
                && call.runtime_call.is_some()
                && !call.evidence_arguments.is_empty()
        });
        let mut calls = Vec::new();
        for block in machine.blocks.iter().filter(|_| checks_calls) {
            for (index, operation) in block.operations.iter().enumerate() {
                let Some(parts) = terminal_psi::direct_call_parts(&operation.kind) else {
                    continue;
                };
                calls.push(RuntimeEvidenceOperation {
                    site: semantic_vocabulary::InvocationSite::Operation(operation.id),
                    position: (
                        block.id,
                        u32::try_from(index)
                            .map_err(|_| "evidence operation position exceeds u32")?,
                    ),
                    callee: parts.callee,
                    role: parts.role,
                    arguments: parts.arguments.to_vec(),
                    structural_arguments: parts.structural_arguments.to_vec(),
                    result: match &operation.result {
                        terminal_psi::OperationResult::Unit => RuntimeEvidenceResult::Unit,
                        terminal_psi::OperationResult::Scalar(value) => {
                            RuntimeEvidenceResult::Scalar {
                                id: value.id,
                                scalar_type: value.scalar_type,
                            }
                        }
                        terminal_psi::OperationResult::Structural(result) => {
                            RuntimeEvidenceResult::Structural(result.clone())
                        }
                    },
                    selected_evidence: parts.selected_evidence.to_vec(),
                });
            }
            if let Some(invocation) = block.terminator.module_invocation() {
                calls.push(RuntimeEvidenceOperation {
                    site: semantic_vocabulary::InvocationSite::InvokeNoNormal(
                        block.terminator.edge(),
                    ),
                    position: (
                        block.id,
                        u32::try_from(block.operations.len())
                            .map_err(|_| "evidence invocation position exceeds u32")?,
                    ),
                    callee: invocation.callee,
                    role: terminal_psi::DirectCallRole::NoNormal,
                    arguments: invocation.arguments.to_vec(),
                    structural_arguments: invocation.structural_arguments.to_vec(),
                    result: RuntimeEvidenceResult::NoNormal,
                    selected_evidence: Vec::new(),
                });
            }
        }
        let mut values = Vec::new();
        if observes {
            values.extend(machine.parameters.iter().map(|value| value.id));
            for call in &calls {
                values.extend_from_slice(&call.arguments);
                if let Some(result) = call.result.scalar() {
                    values.push(result.0);
                }
            }
            values.sort_unstable();
            values.dedup();
        }
        let scalar_expressions = if observes {
            descriptor_results::terminal_scalar_snapshots(machine, &values)
        } else {
            Vec::new()
        };
        let payload_contracts = if observes {
            descriptor_results::descriptor_machine(&graph, machine.id)
                .ok_or("runtime evidence body has no current descriptor machine")?
                .place_types
                .keys()
                .filter_map(|place| {
                    descriptor_results::terminal_payload_contract(machine, *place)
                        .map(|contract| (*place, contract))
                })
                .collect()
        } else {
            Vec::new()
        };
        bodies.push(RuntimeEvidenceBody {
            id: machine.id,
            entry: machine.entry,
            declaration: machine.declaration.clone(),
            parameters: machine
                .parameters
                .iter()
                .map(|parameter| parameter.id)
                .collect(),
            structural_parameters: machine.structural_parameters.clone(),
            outcome_specific_ensures: machine.contract.outcome_specific_ensures.clone(),
            calls,
            scalar_expressions,
            payload_contracts,
        });
    }
    validate_runtime_evidence_inputs_in(&RuntimeEvidenceInputs {
        identities: &module.identities,
        structural_types: &module.structural_types,
        structural_domains: &module.structural_domains,
        proposition_applications: &module.proposition_applications,
        evidence_terms: &module.evidence_terms,
        evidence_contract_lanes: &module.evidence_contract_lanes,
        proof_output_calls: &module.proof_output_calls,
        machines: &bodies,
        graph: &graph,
    })
}
