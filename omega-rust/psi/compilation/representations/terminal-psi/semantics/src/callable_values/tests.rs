//! Controls for evaluated code values, using the production Terminal projection.

use super::*;
use semantic_vocabulary::{ContractId, ScalarQualificationSetId, ServiceId, StructuralPlaceKind};
use terminal_psi::{
    Block, BoundaryMachineResult, BoundaryParameterKind, MachineContract, Operation,
    OperationKind as O, OperationResult, RegisterTargetApplicability, StructuralAccess,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalCallableContract,
    TerminalCallableEnvelope, TerminalCallableTermination, TerminalIdentityIndex, TerminalMachine,
    TerminalMachineDescriptor, TerminalMachineDescriptorOrigin, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration,
};

fn machine_id(id: u64) -> MachineId {
    MachineId::new(id).unwrap()
}
fn operation_id(id: u64) -> OperationId {
    OperationId::new(id).unwrap()
}
fn descriptor_id(id: u64) -> MachineDescriptorId {
    MachineDescriptorId::new(id).unwrap()
}
fn contract_id() -> CallableContractId {
    CallableContractId::new(1).unwrap()
}
fn value(id: u64) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(id).unwrap(),
        scalar_type: ScalarType::Callable(contract_id()),
        qualifications: ScalarQualificationSetId::ZERO,
    }
}
fn machine(id: u64) -> TerminalMachine {
    let entry = BlockId::new(1).unwrap();
    TerminalMachine {
        assembly_target_requirement: RegisterTargetApplicability::Any,
        id: machine_id(id),
        attachment: None,
        nominal_cleanup_subject: None,
        ranked_scc: None,
        structural_places: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![Block::plain(
            entry,
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        )],
        contract: MachineContract {
            declaration: None,
            parameters: Vec::new(),
            parameter_order: Vec::new(),
            structural_parameters: Vec::new(),
            result: TerminalMachineResult::Unit,
            entry_claims: Vec::new(),
            declared_service_reach: None,
            closed_reach_application: None,
            published_service_ceiling: Vec::new(),
            published_synchronous_entries: Vec::new(),
            content_entry_claims: Vec::new(),
            ..MachineContract::empty(ContractId::new(1).unwrap())
        },
    }
}
fn module() -> TerminalModule {
    let mut module = TerminalModule::for_entry(machine_id(1));
    module.callables.contracts.push(TerminalCallableContract {
        id: contract_id(),
        identity: TerminalIdentityIndex(1),
        scalar_parameters: Vec::new(),
        parameter_order: Vec::new(),
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Unit,
        proof_result: TerminalMachineResult::Unit,
        entry_claim_paths: Vec::new(),
        result_claim_paths: Vec::new(),
        result_reference_regions: Vec::new(),
        proof: MachineContract::empty(ContractId::new(1).unwrap()),
        evidence_contract: Vec::new(),
        envelope: TerminalCallableEnvelope {
            service_reach: Vec::new(),
            invocations: Vec::new(),
            suspends: false,
            blocks: false,
            termination: TerminalCallableTermination::NotGuaranteed,
        },
    });
    for (descriptor, selected) in [(1, 3), (2, 4), (3, 5)] {
        module
            .callables
            .descriptors
            .push(TerminalMachineDescriptor {
                id: descriptor_id(descriptor),
                contract: contract_id(),
                machine: machine_id(selected),
                origin: TerminalMachineDescriptorOrigin {
                    application: TerminalIdentityIndex(descriptor as u32 + 1),
                    conformance: None,
                    requirement: None,
                },
                refinement_obligations: Vec::new(),
                refinement_evidence: Vec::new(),
            });
    }
    module
}
fn select(operation: u64, result: u64, descriptor: u64) -> Operation {
    Operation::unsuspended(
        operation_id(operation),
        OperationResult::Scalar(value(result)),
        O::SelectMachine {
            descriptor: descriptor_id(descriptor),
        },
    )
}
fn direct_call(operation: u64, callee: u64, arguments: Vec<ValueId>) -> Operation {
    Operation::unsuspended(
        operation_id(operation),
        OperationResult::Unit,
        O::CallUnit {
            callee: machine_id(callee),
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    )
}
fn scalar_call(operation: u64, result: u64, callee: u64, arguments: Vec<ValueId>) -> Operation {
    Operation::unsuspended(
        operation_id(operation),
        OperationResult::Scalar(value(result)),
        O::Call {
            callee: machine_id(callee),
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    )
}
fn indirect_call(operation: u64, callee: ValueId) -> Operation {
    Operation::unsuspended(
        operation_id(operation),
        OperationResult::Unit,
        O::CallMachine {
            callee,
            contract: contract_id(),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    )
}
fn query(machine: u64, id: u64) -> CallableValueQuery {
    CallableValueQuery::Scalar {
        machine: machine_id(machine),
        value: value(id).id,
        contract: contract_id(),
    }
}

fn boundary(id: u64, returns_callable: bool) -> terminal_psi::BoundaryMachineDeclaration {
    terminal_psi::BoundaryMachineDeclaration {
        id: BoundaryMachineId::new(id).unwrap(),
        declaration: None,
        numeric_intrinsic: None,
        role: terminal_psi::BoundaryDeclarationRole::Callable,
        kind: semantic_vocabulary::BoundaryRequirementKind::Machine,
        completion: if returns_callable {
            semantic_vocabulary::BoundaryCompletion::NormalReturn
        } else {
            semantic_vocabulary::BoundaryCompletion::External(
                semantic_vocabulary::ExternalCompletionKind::ProcessExit,
            )
        },
        unconditional_termination: false,
        identity: TerminalIdentityIndex(id as u32),
        owner: terminal_psi::NominalDeclarationIdentity::new(TerminalIdentityIndex(id as u32)),
        attachment: None,
        parameter_order: if returns_callable {
            vec![BoundaryParameterKind::Scalar]
        } else {
            Vec::new()
        },
        scalar_parameters: if returns_callable {
            vec![value(1).scalar_type]
        } else {
            Vec::new()
        },
        crash_routes: Vec::new(),
        structural_parameters: Vec::new(),
        result: if returns_callable {
            BoundaryMachineResult::Scalar(value(1).value_type())
        } else {
            BoundaryMachineResult::Unit
        },
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
        published_synchronous_entries: Vec::new(),
    }
}

#[test]
fn entry_calls_bind_two_selections_through_one_formal_without_invoking_unused_code() {
    let mut module = module();
    let mut entry = machine(1);
    let mut identity = machine(6);
    identity.parameters.push(value(1));
    identity.parameter_order.push(BoundaryParameterKind::Scalar);
    identity.result = TerminalMachineResult::Scalar(value(1));
    identity.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(1).unwrap(),
        value: value(1).id,
        cleanup_actions: Vec::new(),
    };
    let result_call = scalar_call(4, 4, 6, vec![value(1).id]);
    entry.blocks[0].operations = vec![
        select(1, 1, 1),
        select(2, 2, 2),
        select(3, 3, 3),
        result_call,
        direct_call(5, 2, vec![value(4).id]),
        direct_call(6, 2, vec![value(2).id]),
    ];
    let mut shared = machine(2);
    shared.parameters.push(value(1));
    shared.parameter_order.push(BoundaryParameterKind::Scalar);
    shared.blocks[0]
        .operations
        .push(indirect_call(1, value(1).id));
    let mut unreachable = Block::plain(
        BlockId::new(2).unwrap(),
        Vec::new(),
        Terminator::ReturnUnit {
            edge: EdgeId::new(2).unwrap(),
            cleanup_actions: Vec::new(),
        },
    );
    unreachable.operations.push(indirect_call(2, value(1).id));
    shared.blocks.push(unreachable);
    module.machines = vec![entry, shared, machine(3), machine(4), machine(5), identity];

    let projected = terminal_callable_values(&module).unwrap();
    let returned = resolve_callable_value(&projected, query(1, 4)).unwrap();
    assert_eq!(returned.descriptors, BTreeSet::from([descriptor_id(1)]));
    assert!(returned.is_resolved());
    let reached = reachable_callable_invocations(&projected, [machine_id(1)]).unwrap();
    let invocation = reached
        .operation(machine_id(2), operation_id(1))
        .unwrap()
        .unwrap();
    let ReachedCallableTarget::Callable(target) = &invocation.target else {
        panic!("shared formal lost its evaluated callable target");
    };
    assert_eq!(
        target.descriptors,
        BTreeSet::from([descriptor_id(1), descriptor_id(2)])
    );
    assert!(invocation.targets_are_resolved());
    assert_eq!(
        invocation.selected_machines,
        BTreeSet::from([machine_id(3), machine_id(4)])
    );
    assert_eq!(
        reached.reached_machines,
        BTreeSet::from([
            machine_id(1),
            machine_id(2),
            machine_id(3),
            machine_id(4),
            machine_id(6)
        ])
    );
    assert!(!reached.reached_machines.contains(&machine_id(5)));
    assert!(
        reached
            .checked_operation(&projected, machine_id(2), operation_id(2))
            .unwrap()
            .is_none()
    );
    let mut missing = reached.clone();
    missing.invocations.remove(&(
        machine_id(2),
        CallableInvocationSite::Invocation(InvocationSite::Operation(operation_id(1))),
    ));
    assert!(
        missing
            .checked_operation(&projected, machine_id(2), operation_id(1))
            .is_err()
    );
    assert!(
        reached
            .operation(machine_id(1), operation_id(3))
            .unwrap()
            .is_none()
    );

    // The same returned value and formal substitution must survive a chosen
    // provider. Its terminal call is reached; an unused descriptor's is not.
    module.boundary_machines = vec![boundary(1, true), boundary(2, false), boundary(3, false)];
    module.machines[0].blocks[0].operations[3].kind = O::BoundaryCall {
        boundary: BoundaryMachineId::new(1).unwrap(),
        arguments: vec![value(1).id],
        structural_arguments: Vec::new(),
        completion_receipts: Vec::new(),
        requirement_obligations: Vec::new(),
    };
    for (machine, boundary) in [(2, 2), (4, 3)] {
        module.machines[machine].blocks[0].terminator = Terminator::ExternalCompletion {
            edge: EdgeId::new(1).unwrap(),
            boundary: BoundaryMachineId::new(boundary).unwrap(),
            kind: semantic_vocabulary::ExternalCompletionKind::ProcessExit,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            requirement_obligations: Vec::new(),
            frontier_lower_bound: Vec::new(),
        };
    }
    let binding = CallableBoundaryImplementation {
        boundary: BoundaryMachineId::new(1).unwrap(),
        machine: machine_id(6),
    };
    let projected =
        terminal_callable_values_with_boundary_implementations(&module, &[binding]).unwrap();
    assert_eq!(
        resolve_callable_value(&projected, query(1, 4))
            .unwrap()
            .descriptors,
        BTreeSet::from([descriptor_id(1)])
    );
    let reached = reachable_callable_invocations(&projected, [machine_id(1)]).unwrap();
    assert_eq!(reached.roots, BTreeSet::from([machine_id(1)]));
    assert_eq!(
        reached
            .site(
                machine_id(2),
                CallableInvocationSite::Invocation(InvocationSite::Operation(operation_id(1)))
            )
            .unwrap()
            .selected_machines,
        BTreeSet::from([machine_id(3), machine_id(4)])
    );
    assert_eq!(
        reached_boundary_calls(&module, machine_id(1), &[binding]).unwrap(),
        vec![
            BoundaryMachineId::new(1).unwrap(),
            BoundaryMachineId::new(2).unwrap()
        ]
    );
    assert!(
        terminal_callable_values_with_boundary_implementations(&module, &[binding, binding])
            .is_err()
    );
    assert!(
        reached_boundary_calls(
            &module,
            machine_id(1),
            &[CallableBoundaryImplementation {
                machine: machine_id(5),
                ..binding
            }]
        )
        .is_err()
    );
}

#[test]
fn saved_callee_uses_its_read_cut_after_an_overlapping_write() {
    let mut module = module();
    let carrier = StructuralTypeId::new(1).unwrap();
    let home = PlaceId::new(1).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: carrier,
        identity: TerminalIdentityIndex(10),
        requires: Vec::new(),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: true,
        descriptor_cleanup: true,
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Callable(contract_id())),
        scalar_restriction: None,
    });
    let mut entry = machine(1);
    entry
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: home,
            position: 0,
            structural_type: carrier,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
        });
    entry
        .parameter_order
        .push(BoundaryParameterKind::Structural);
    entry.structural_places.push(StructuralPlaceDeclaration {
        id: home,
        kind: StructuralPlaceKind::Parameter { position: 0 },
    });
    let store = |id, source| {
        Operation::unsuspended(
            operation_id(id),
            OperationResult::Unit,
            O::WriteOnlyPrimitiveStore {
                destination: home,
                path: Vec::new(),
                value: value(source).id,
                range_obligation: None,
            },
        )
    };
    let read = |id, result| {
        Operation::unsuspended(
            operation_id(id),
            OperationResult::Scalar(value(result)),
            O::PrimitiveScalarRead {
                source: home,
                path: Vec::new(),
            },
        )
    };
    entry.blocks[0].operations = vec![
        select(1, 1, 1),
        select(2, 2, 2),
        store(3, 1),
        read(4, 3),
        store(5, 2),
        indirect_call(6, value(3).id),
        read(7, 4),
        indirect_call(8, value(4).id),
    ];
    module.machines = vec![entry, machine(3), machine(4), machine(5)];
    let projected = terminal_callable_values(&module).unwrap();
    for (result, descriptor) in [(3, 1), (4, 2)] {
        let possibilities = resolve_callable_value(&projected, query(1, result)).unwrap();
        assert!(possibilities.is_resolved());
        assert_eq!(
            possibilities.descriptors,
            BTreeSet::from([descriptor_id(descriptor)])
        );
    }
    let reached = reachable_callable_invocations(&projected, [machine_id(1)]).unwrap();
    for (call, selected) in [(6, 3), (8, 4)] {
        let invocation = reached
            .operation(machine_id(1), operation_id(call))
            .unwrap()
            .unwrap();
        assert!(invocation.targets_are_resolved());
        assert_eq!(
            invocation.selected_machines,
            BTreeSet::from([machine_id(selected)])
        );
    }
}

#[test]
fn external_callable_input_retains_an_explicit_unresolved_ceiling() {
    let mut module = module();
    let mut entry = machine(1);
    entry.parameters.push(value(1));
    entry.parameter_order.push(BoundaryParameterKind::Scalar);
    entry.blocks[0]
        .operations
        .push(indirect_call(1, value(1).id));
    module.machines = vec![entry, machine(3), machine(4), machine(5)];
    let projected = terminal_callable_values(&module).unwrap();
    let reached = reachable_callable_invocations(&projected, [machine_id(1)]).unwrap();
    let invocation = reached
        .operation(machine_id(1), operation_id(1))
        .unwrap()
        .unwrap();
    let ReachedCallableTarget::Callable(target) = &invocation.target else {
        panic!("unknown callable ingress disappeared as a call");
    };
    assert!(target.descriptors.is_empty());
    assert_eq!(target.unresolved_ceilings, BTreeSet::from([contract_id()]));
    assert!(!invocation.targets_are_resolved());
    assert_eq!(reached.reached_machines, BTreeSet::from([machine_id(1)]));
}

#[test]
fn ungrounded_recursive_callable_result_is_a_ceiling_instead_of_an_empty_selection() {
    let mut module = module();
    let mut recursive = machine(1);
    recursive.result = TerminalMachineResult::Scalar(value(1));
    recursive.blocks[0]
        .operations
        .push(scalar_call(1, 1, 1, Vec::new()));
    recursive.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(1).unwrap(),
        value: value(1).id,
        cleanup_actions: Vec::new(),
    };
    module.machines = vec![recursive, machine(3), machine(4), machine(5)];
    let projected = terminal_callable_values(&module).unwrap();
    let selected = resolve_callable_value(&projected, query(1, 1)).unwrap();
    assert!(selected.descriptors.is_empty());
    assert!(selected.inputs.is_empty());
    assert_eq!(
        selected.unresolved_ceilings,
        BTreeSet::from([contract_id()])
    );
    assert!(!selected.is_resolved());
}

#[test]
fn stale_descriptor_contract_and_missing_callee_ssa_are_errors() {
    let mut module = module();
    let mut entry = machine(1);
    entry.blocks[0].operations = vec![select(1, 1, 1), indirect_call(2, value(1).id)];
    module.machines = vec![entry, machine(3), machine(4), machine(5)];
    module.callables.descriptors[0].contract = CallableContractId::new(2).unwrap();
    let projected = terminal_callable_values(&module).unwrap();
    assert_eq!(
        resolve_callable_value(&projected, query(1, 1)),
        Err("constructed callable descriptor has the wrong exact contract")
    );
    module.callables.descriptors[0].contract = contract_id();
    module.machines[0].blocks[0].operations[1] = indirect_call(2, value(99).id);
    let projected = terminal_callable_values(&module).unwrap();
    assert_eq!(
        reachable_callable_invocations(&projected, [machine_id(1)]),
        Err("callable provenance SSA type disagrees with its exact contract")
    );
}

fn service_summary_module() -> TerminalModule {
    let mut module = module();
    module.services = vec![
        terminal_psi::ServiceDeclaration {
            id: ServiceId::new(1).unwrap(),
            identity: TerminalIdentityIndex(1),
            parents: Vec::new(),
        },
        terminal_psi::ServiceDeclaration {
            id: ServiceId::new(2).unwrap(),
            identity: TerminalIdentityIndex(2),
            parents: vec![ServiceId::new(1).unwrap()],
        },
        terminal_psi::ServiceDeclaration {
            id: ServiceId::new(3).unwrap(),
            identity: TerminalIdentityIndex(3),
            parents: Vec::new(),
        },
    ];
    module.machines = (1..=5).map(machine).collect();
    module
}

#[test]
fn body_service_summary_closes_cycles_without_counting_unused_constructed_code() {
    let mut module = service_summary_module();
    module.machines[0].published_service_ceiling =
        module.services.iter().map(|row| row.id).collect();
    module.machines[0].blocks[0].operations = vec![select(1, 1, 1), direct_call(2, 2, Vec::new())];
    module.machines[1].blocks[0].operations = vec![
        direct_call(1, 1, Vec::new()),
        Operation::unsuspended(
            operation_id(2),
            OperationResult::Unit,
            O::PortWrite {
                service: ServiceId::new(2).unwrap(),
                port: 1,
                value: 1,
            },
        ),
    ];
    module.machines[2].blocks[0].operations = vec![Operation::unsuspended(
        operation_id(1),
        OperationResult::Unit,
        O::PortWrite {
            service: ServiceId::new(3).unwrap(),
            port: 1,
            value: 1,
        },
    )];
    let summaries = crate::body_services::body_service_reaches(&module).unwrap();
    assert_eq!(
        summaries.get(machine_id(1)).unwrap(),
        &[ServiceId::new(1).unwrap(), ServiceId::new(2).unwrap()]
    );
    assert_eq!(
        summaries.get(machine_id(3)).unwrap(),
        &[ServiceId::new(3).unwrap()]
    );
}

#[test]
fn body_service_summary_retains_the_full_unknown_callable_ceiling() {
    let mut module = service_summary_module();
    module.callables.contracts[0].envelope.service_reach =
        vec![ServiceId::new(1).unwrap(), ServiceId::new(2).unwrap()];
    module.machines[0].parameters.push(value(1));
    module.machines[0]
        .parameter_order
        .push(BoundaryParameterKind::Scalar);
    module.machines[0].blocks[0]
        .operations
        .push(indirect_call(1, value(1).id));
    let summaries = crate::body_services::body_service_reaches(&module).unwrap();
    assert_eq!(
        summaries.get(machine_id(1)).unwrap(),
        &[ServiceId::new(1).unwrap(), ServiceId::new(2).unwrap()]
    );
    module.callables.contracts.clear();
    assert!(crate::body_services::body_service_reaches(&module).is_err());
}

#[test]
fn body_service_summary_refuses_an_unknown_instruction_service() {
    let mut module = service_summary_module();
    module.machines[0].blocks[0]
        .operations
        .push(Operation::unsuspended(
            operation_id(1),
            OperationResult::Unit,
            O::PortWrite {
                service: ServiceId::new(99).unwrap(),
                port: 1,
                value: 1,
            },
        ));
    assert!(crate::body_services::body_service_reaches(&module).is_err());
}
