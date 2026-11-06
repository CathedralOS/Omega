use super::*;
use semantic_vocabulary::{BoundaryCompletion, BoundaryRequirementKind, StructuralDomainId};
use terminal_psi::{
    BoundaryDeclarationRole, BoundaryLenderPreservationDemand, BoundaryMachineResult,
    BoundaryReferenceSource, NominalDeclarationIdentity, OperationResult,
    StructuralDomainRequirement, TerminalIdentityIndex,
};

fn declaration(identity: u64) -> BoundaryMachineDeclaration {
    BoundaryMachineDeclaration {
        id: BoundaryMachineId::new(identity).unwrap(),
        declaration: None,
        numeric_intrinsic: None,
        role: BoundaryDeclarationRole::Callable,
        kind: BoundaryRequirementKind::Machine,
        completion: BoundaryCompletion::NormalReturn,
        unconditional_termination: false,
        identity: TerminalIdentityIndex(1),
        owner: NominalDeclarationIdentity::new(TerminalIdentityIndex(2)),
        attachment: None,
        parameter_order: Vec::new(),
        scalar_parameters: Vec::new(),
        crash_routes: Vec::new(),
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Unit,
        result_reference_regions: Vec::new(),
        requires: Vec::new(),
        normal_qualifications: Vec::new(),
        scalar_domain_requirements: Vec::new(),
        scalar_requires: Vec::new(),
        scalar_ensures: Vec::new(),
        reference_outputs: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        owning_service: None,
        published_service_ceiling: Vec::new(),
    }
}

fn body(identity: u64, operations: Vec<Operation>) -> TerminalMachine {
    let mut body = super::super::tests::unit_machine();
    body.id = MachineId::new(identity).unwrap();
    body.blocks[0].operations = operations;
    body
}

fn call(identity: u64, boundary: u64) -> Operation {
    Operation::unsuspended(
        OperationId::new(identity).unwrap(),
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

fn forwarding(machine: u64, operation: u64, output: u32) -> BoundaryReferenceForwardingDemand {
    BoundaryReferenceForwardingDemand {
        machine: MachineId::new(machine).unwrap(),
        operation: OperationId::new(operation).unwrap(),
        output,
        sources: vec![BoundaryReferenceSource::Reference { argument_index: 0 }],
    }
}

fn lender(machine: u64, operation: u64, domain: u64) -> BoundaryLenderPreservationDemand {
    BoundaryLenderPreservationDemand {
        machine: MachineId::new(machine).unwrap(),
        operation: OperationId::new(operation).unwrap(),
        requirement: StructuralDomainRequirement {
            argument_index: 0,
            path: vec![StructuralPathSegment::Referent],
            domain: StructuralDomainId::new(domain).unwrap(),
        },
    }
}

#[test]
fn an_unrequested_boundary_needs_no_body_but_keeps_the_declaration_gate() {
    let mut module = TerminalModule {
        boundary_machines: vec![declaration(1)],
        machines: vec![body(1, vec![call(1, 1)])],
        ..TerminalModule::declarations()
    };
    let boundary = module.boundary_machines[0].id;
    assert_eq!(
        BoundaryLenderPreservation::new(&module).require(boundary, None),
        Ok(())
    );
    assert_eq!(
        BoundaryLenderPreservation::new(&module).require(BoundaryMachineId::new(2).unwrap(), None),
        Err("lender preservation has no boundary declaration")
    );
    module
        .boundary_reference_forwarding
        .push(forwarding(1, 1, 0));
    assert_eq!(
        BoundaryLenderPreservation::new(&module).require(boundary, None),
        Err("reference forwarding requires an independently checked execution body")
    );
}

#[test]
fn malformed_forwarding_joins_keep_original_request_order_and_refusals() {
    let mut module = TerminalModule {
        machines: vec![body(1, vec![call(1, 2)])],
        boundary_reference_forwarding: vec![forwarding(9, 1, 0), forwarding(1, 8, 0)],
        ..TerminalModule::declarations()
    };
    let requested = BoundaryMachineId::new(1).unwrap();
    let refusal = |module: &TerminalModule| {
        BoundaryLenderPreservation::new(module).require_forwarding(requested, None)
    };
    // Even an unrelated selected boundary retains the old global join refusal.
    assert_eq!(refusal(&module), Err("reference forwarding has no caller"));
    module.boundary_reference_forwarding.swap(0, 1);
    assert_eq!(
        refusal(&module),
        Err("reference forwarding has no operation")
    );
    module.boundary_reference_forwarding = vec![forwarding(1, 1, 0)];
    module.machines.push(module.machines[0].clone());
    assert_eq!(
        refusal(&module),
        Err("reference forwarding has ambiguous callers")
    );
    module.machines.pop();
    module.machines[0].blocks[0].operations.push(call(1, 2));
    assert_eq!(
        refusal(&module),
        Err("reference forwarding has ambiguous operations")
    );
    module.machines[0].blocks[0].operations.truncate(1);
    module.machines[0].blocks[0].operations[0].kind =
        OperationKind::BooleanConstant { value: true };
    assert_eq!(
        refusal(&module),
        Err("reference forwarding does not name a boundary call")
    );
}

#[test]
fn sparse_call_joins_keep_every_duty_in_its_original_order() {
    let machine = u64::MAX - 2;
    let operation = u64::MAX - 3;
    let module = TerminalModule {
        machines: vec![body(machine, vec![call(operation, 8)])],
        boundary_reference_forwarding: vec![
            forwarding(machine, operation, 1),
            forwarding(machine, operation, 0),
        ],
        boundary_lender_preservation: vec![
            lender(machine, operation, 2),
            lender(machine, operation, 1),
        ],
        ..TerminalModule::declarations()
    };
    let prepared = BoundaryLenderPreservation::new(&module);
    assert_eq!(prepared.call_contracts.machines.len(), 1);
    let requests = prepared.forwarding.as_ref().unwrap();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.demand.output)
            .collect::<Vec<_>>(),
        [1, 0]
    );
    assert_eq!(
        prepared
            .call_contracts
            .lenders(
                MachineId::new(machine).unwrap(),
                OperationId::new(operation).unwrap()
            )
            .map(|demand| demand.requirement.domain.get())
            .collect::<Vec<_>>(),
        [2, 1]
    );
    assert_eq!(prepared.lender_calls.len(), 1);
}

#[test]
fn a_requested_caller_still_checks_its_other_calls_to_the_same_boundary() {
    let module = TerminalModule {
        boundary_machines: vec![declaration(1)],
        machines: vec![
            body(30, vec![call(90, 1), call(5, 2), call(7, 1)]),
            body(10, vec![call(3, 1)]),
        ],
        boundary_lender_preservation: vec![lender(30, 7, 1)],
        ..TerminalModule::declarations()
    };
    let prepared = BoundaryLenderPreservation::new(&module);
    assert_eq!(
        prepared
            .lender_calls
            .iter()
            .map(|call| (call.caller_ordinal, call.operation_ordinal))
            .collect::<Vec<_>>(),
        [(0, 0), (0, 2)]
    );
    // The first call is unrequested and malformed. Skipping it would miss its
    // original failure before reaching the later call that selected this caller.
    assert_eq!(
        prepared.require(module.boundary_machines[0].id, None),
        Err("the lender call has no structural argument")
    );
}

#[test]
fn shared_call_catalogs_do_not_share_an_invocations_mutable_proofs() {
    let module = TerminalModule {
        machines: vec![body(1, Vec::new()), body(2, Vec::new())],
        ..TerminalModule::declarations()
    };
    let graph = Vec::new();
    let mut original = NormalPayloadContext::new(&module, &graph);
    original.machine_payloads[0].writing = true;
    original.machine_payloads[0].entry_complete = true;
    original.machine_payloads[0].completing = true;
    let isolated = original.for_complete_entry(module.machines[1].id).unwrap();
    assert!(Arc::ptr_eq(
        &original.call_contracts,
        &isolated.call_contracts
    ));
    assert!(!isolated.machine_payloads[0].writing);
    assert!(!isolated.machine_payloads[0].entry_complete);
    assert!(isolated.machine_payloads[1].entry_complete);
    assert!(isolated.machine_payloads[0].completing);
    assert!(isolated.machine_payloads[0].writes.is_none());
    assert!(isolated.machine_payloads[0].posts.is_empty());
}
