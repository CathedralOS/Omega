//! Find each requested call once before checking the chosen helpers.
//!
//! Several guarantees can belong to one call. Share only their catalog joins;
//! keep every original request and the independent selected-body proofs. Sparse
//! identities name sorted rows rather than determine an allocation size.

use super::*;
use semantic_vocabulary::{BlockId, BoundaryMachineId, OperationId};
use terminal_psi::{BoundaryMachineDeclaration, BoundaryReferenceForwardingDemand};

pub(super) struct ForwardingRequest<'module> {
    boundary: BoundaryMachineId,
    pub(super) demand: &'module BoundaryReferenceForwardingDemand,
}

pub(super) struct LenderCall<'module> {
    boundary: BoundaryMachineId,
    pub(super) caller_ordinal: usize,
    block_ordinal: usize,
    pub(super) block: BlockId,
    pub(super) operation_ordinal: usize,
    pub(super) structural_arguments: &'module [StructuralArgument],
}

struct RequestedCall<'module> {
    machine: MachineId,
    operation: OperationId,
    matching_operations: usize,
    first_operation: Option<&'module Operation>,
}

/// Check the saved data's requested guarantees against each selected helper.
/// One execution or installation scope shares these immutable call associations;
/// choosing an implementation and proving its body remain independent checks.
pub struct BoundaryLenderPreservation<'module> {
    module: &'module TerminalModule,
    call_contracts: Arc<super::call_contracts::CallContracts<'module>>,
    forwarding: Result<Vec<ForwardingRequest<'module>>, &'static str>,
    lender_calls: Vec<LenderCall<'module>>,
}

impl<'module> BoundaryLenderPreservation<'module> {
    pub fn new(module: &'module TerminalModule) -> Self {
        let call_contracts = Arc::new(super::call_contracts::CallContracts::new(module));
        let mut lender_keys = module
            .boundary_lender_preservation
            .iter()
            .map(|demand| (demand.machine, demand.operation))
            .collect::<Vec<_>>();
        lender_keys.sort_unstable();
        lender_keys.dedup();
        let mut keys = lender_keys.clone();
        keys.extend(
            module
                .boundary_reference_forwarding
                .iter()
                .map(|demand| (demand.machine, demand.operation)),
        );
        keys.sort_unstable();
        keys.dedup();
        let mut requests = keys
            .into_iter()
            .map(|(machine, operation)| RequestedCall {
                machine,
                operation,
                matching_operations: 0,
                first_operation: None,
            })
            .collect::<Vec<_>>();
        let mut lender_callers = Vec::new();
        let mut lender_calls = Vec::new();
        if !requests.is_empty() {
            for (caller_ordinal, body) in module.machines.iter().enumerate() {
                let start = requests.partition_point(|request| request.machine < body.id);
                let end = requests.partition_point(|request| request.machine <= body.id);
                if start == end {
                    continue;
                }
                for (block_ordinal, block) in body.blocks.iter().enumerate() {
                    for (operation_ordinal, operation) in block.operations.iter().enumerate() {
                        if let Ok(index) = requests[start..end]
                            .binary_search_by_key(&operation.id, |request| request.operation)
                        {
                            let request = &mut requests[start + index];
                            request.matching_operations += 1;
                            request.first_operation.get_or_insert(operation);
                        }
                        if let OperationKind::BoundaryCall {
                            boundary,
                            structural_arguments,
                            ..
                        } = &operation.kind
                        {
                            lender_calls.push(LenderCall {
                                boundary: *boundary,
                                caller_ordinal,
                                block_ordinal,
                                block: block.id,
                                operation_ordinal,
                                structural_arguments,
                            });
                            if lender_keys.binary_search(&(body.id, operation.id)).is_ok() {
                                lender_callers.push((*boundary, caller_ordinal));
                            }
                        }
                    }
                }
            }
        }
        lender_callers.sort_unstable();
        lender_callers.dedup();
        // A requested caller historically checks all its calls to this boundary,
        // including another call without its own lender request. Keep that work
        // domain and the module/block/operation error order.
        lender_calls.retain(|call| {
            lender_callers
                .binary_search(&(call.boundary, call.caller_ordinal))
                .is_ok()
        });
        lender_calls.sort_unstable_by_key(|call| {
            (
                call.boundary,
                call.caller_ordinal,
                call.block_ordinal,
                call.operation_ordinal,
            )
        });
        let forwarding = forwarding_requests(module, &call_contracts.machines, &requests);
        Self {
            module,
            call_contracts,
            forwarding,
            lender_calls,
        }
    }

    pub fn require(
        &self,
        boundary: BoundaryMachineId,
        implementation: Option<MachineId>,
    ) -> Result<(), &'static str> {
        self.declaration(boundary)
            .ok_or("lender preservation has no boundary declaration")?;
        self.require_forwarding(boundary, implementation)?;
        let start = self
            .lender_calls
            .partition_point(|call| call.boundary < boundary);
        let end = self
            .lender_calls
            .partition_point(|call| call.boundary <= boundary);
        super::provider_observations::require_callers(
            self.module,
            implementation,
            &self.lender_calls[start..end],
            Arc::clone(&self.call_contracts),
        )
    }

    pub(super) fn require_forwarding(
        &self,
        boundary: BoundaryMachineId,
        implementation: Option<MachineId>,
    ) -> Result<(), &'static str> {
        let requests = self.forwarding.as_ref().map_err(|reason| *reason)?;
        let start = requests.partition_point(|request| request.boundary < boundary);
        let end = requests.partition_point(|request| request.boundary <= boundary);
        if start == end {
            return Ok(());
        }
        let implementation = implementation
            .ok_or("reference forwarding requires an independently checked execution body")?;
        let declaration = self
            .declaration(boundary)
            .ok_or("reference forwarding has no boundary declaration")?;
        let bodies = self.call_contracts.machines(implementation);
        let (_, ordinal) = bodies
            .first()
            .ok_or("reference forwarding has no execution body")?;
        if bodies.len() != 1 {
            return Err("reference forwarding has ambiguous execution bodies");
        }
        super::reference_forwarding::require_selected(
            self.module,
            boundary,
            declaration,
            &self.module.machines[*ordinal],
            &requests[start..end],
            Arc::clone(&self.call_contracts),
        )
    }

    fn declaration(
        &self,
        identity: BoundaryMachineId,
    ) -> Option<&'module BoundaryMachineDeclaration> {
        self.call_contracts.first_declaration(identity)
    }
}

fn matching_machines(rows: &[(MachineId, usize)], identity: MachineId) -> &[(MachineId, usize)] {
    let start = rows.partition_point(|(found, _)| *found < identity);
    let end = rows.partition_point(|(found, _)| *found <= identity);
    &rows[start..end]
}

fn forwarding_requests<'module>(
    module: &'module TerminalModule,
    machines: &[(MachineId, usize)],
    calls: &[RequestedCall<'module>],
) -> Result<Vec<ForwardingRequest<'module>>, &'static str> {
    let mut output = Vec::new();
    for (ordinal, demand) in module.boundary_reference_forwarding.iter().enumerate() {
        let callers = matching_machines(machines, demand.machine);
        if callers.is_empty() {
            return Err("reference forwarding has no caller");
        }
        if callers.len() != 1 {
            return Err("reference forwarding has ambiguous callers");
        }
        let call = calls
            .binary_search_by_key(&(demand.machine, demand.operation), |call| {
                (call.machine, call.operation)
            })
            .ok()
            .and_then(|index| calls.get(index))
            .ok_or("reference forwarding has no operation")?;
        let operation = call
            .first_operation
            .ok_or("reference forwarding has no operation")?;
        if call.matching_operations != 1 {
            return Err("reference forwarding has ambiguous operations");
        }
        let OperationKind::BoundaryCall { boundary, .. } = &operation.kind else {
            return Err("reference forwarding does not name a boundary call");
        };
        output.push((*boundary, ordinal, demand));
    }
    output.sort_unstable_by_key(|(boundary, ordinal, _)| (*boundary, *ordinal));
    Ok(output
        .into_iter()
        .map(|(boundary, _, demand)| ForwardingRequest { boundary, demand })
        .collect())
}

/// A single query uses the same prepared owner as bulk installation/execution.
pub fn require_boundary_lender_preservation(
    module: &TerminalModule,
    boundary: BoundaryMachineId,
    implementation: Option<MachineId>,
) -> Result<(), &'static str> {
    BoundaryLenderPreservation::new(module).require(boundary, implementation)
}

#[cfg(test)]
mod tests;
