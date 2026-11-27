use super::*;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
};
fn machine_id(value: u64) -> MachineId {
    MachineId::new(value).unwrap()
}

fn scalar_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

fn machine(identity: u64, kind: OperationKind) -> TerminalMachine {
    let result = ValueDeclaration {
        id: ValueId::new(1).unwrap(),
        scalar_type: scalar_type(),
        qualifications: Default::default(),
    };
    TerminalMachine {
        id: machine_id(identity),
        attachment: None,
        structural_places: Vec::new(),
        call_formations: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        entry: BlockId::new(1).unwrap(),
        blocks: vec![Block {
            id: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![Operation::unsuspended(
                OperationId::new(1).unwrap(),
                OperationResult::Scalar(result),
                kind,
            )],
            terminator: Terminator::Return {
                edge: EdgeId::new(1).unwrap(),
                value: result.id,
                cleanup_actions: Vec::new(),
            },
        }],
        ranked_scc: None,
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        contract: terminal_psi::MachineContract {
            selected_application: None,
            declaration: None,
            parameters: Vec::new(),
            parameter_order: Vec::new(),
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            content_entry_claims: Vec::new(),
            result: TerminalMachineResult::Scalar(result),
            declared_service_reach: None,
            closed_reach_application: None,
            published_synchronous_entries: Vec::new(),
            published_service_ceiling: Vec::new(),
            ..MachineContract::empty(ContractId::new(identity).unwrap())
        },
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
    }
}

fn call(callee: u64) -> OperationKind {
    OperationKind::Call {
        callee: machine_id(callee),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    }
}

fn graph() -> TerminalModule {
    let mut module = TerminalModule::for_entry(machine_id(1));
    let application = module
        .scalars.scalar_applications
        .insert(terminal_psi::ScalarApplication::Evaluate {
            entry: terminal_psi::TargetMachineCallable::Machine(machine_id(1)),
            scalar_type: scalar_type(),
        })
        .unwrap();
    module.machines = vec![
        machine(
            1,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(7),
            },
        ),
        machine(2, call(3)),
        machine(3, OperationKind::ReadScalarApplication { application }),
    ];
    module
}

fn scope(module: TerminalModule, target: &str) -> Result<TargetReach, NativeExecutionScopeError> {
    let record = terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_parts(module, record, None).unwrap();
    let reloaded =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    let targets = psi_target_layouts::terminal_target_context(reloaded.view(), target);
    let entry = targets
        .entry(
            reloaded.view().identity(),
            reloaded.semantic_module(),
            reloaded.semantic_module().entry.unwrap(),
        )
        .map_err(|reason| NativeExecutionScopeError::TargetContext(reason.to_string()))?;
    TargetReach::derive(ScopeInputs {
        target,
        artifact: reloaded.view(),
        entry,
        entry_call: None,
        adapters: &[],
        callbacks: &[],
    })
}

#[test]
fn assembly_target_is_checked_only_for_reachable_bodies() {
    let mut module = graph();
    module.machines[1].assembly_target_requirement =
        terminal_psi::RegisterTargetApplicability::X86_64;
    assert!(scope(module.clone(), "linux_arm64").is_ok());

    module.machines[0].blocks[0].operations[0].kind = call(2);
    assert!(scope(module.clone(), "linux_x86_64").is_ok());
    let Err(NativeExecutionScopeError::TargetContext(reason)) = scope(module, "linux_arm64") else {
        panic!("the selected ARM64 target reached an x86-64 assembly body");
    };
    assert!(
        reason.contains("requires x86_64") && reason.contains("linux_arm64"),
        "{reason}"
    );
}

#[test]
fn assembly_target_applies_to_a_body_emitted_for_a_dispatch_table() {
    use terminal_psi::{TerminalDynamicInterface, TerminalDynamicSlot, TerminalDynamicTable};

    let mut module = graph();
    let mut identities = module.identities.clone().into_builder();
    let interface = identities.intern("Operation");
    let requirement = identities.intern("Operation::run");
    let implementation = identities.intern("VectorOperation");
    module.identities = identities.build();
    let catalog = &mut module.execution.dynamic_dispatch;
    catalog.interfaces.push(TerminalDynamicInterface {
        id: 0,
        identity: interface,
        slots: vec![TerminalDynamicSlot {
            slot: 0,
            declaring_trait_identity: interface,
            requirement_identity: requirement,
            family_tuple: Vec::new(),
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            entry_claim_paths: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Scalar(scalar_type().into()),
            result_claim_paths: Vec::new(),
            result_reference_regions: Vec::new(),
        }],
    });
    catalog.tables.push(TerminalDynamicTable {
        id: 0,
        interface: 0,
        implementation_identity: implementation,
        rows: vec![Some(machine_id(2))],
    });
    module.machines[1].assembly_target_requirement =
        terminal_psi::RegisterTargetApplicability::X86_64;

    // The entry returns a constant. Only the table's relocation needs machine 2.
    let x86 = scope(module.clone(), "linux_x86_64").unwrap();
    assert!(x86.emitted.contains_machine(machine_id(2)));
    let Err(NativeExecutionScopeError::TargetContext(reason)) = scope(module, "linux_arm64") else {
        panic!("the ARM64 image must refuse its table's x86-64 function");
    };
    assert!(
        reason.contains("requires x86_64") && reason.contains("linux_arm64"),
        "{reason}"
    );
}

#[test]
fn sequence_bounds_read_only_selected_constructors() {
    let mut module = TerminalModule::for_entry(machine_id(1));
    module.machines = vec![machine(
        2,
        OperationKind::EstablishStructuralArray { elements: vec![] },
    )];
    // The fixture has a scalar result, so including this machine must reject it.
    assert_eq!(
        closed_owned_sequence_bounds(&module, |id| id == machine_id(1)),
        Ok(vec![])
    );
    assert_eq!(
        closed_owned_sequence_bounds(&module, |_| true),
        Err("sequence construction has no structural result")
    );
}

#[test]
fn selected_scope_omits_dormant_scalar_work_and_retains_calls_from_all_blocks() {
    assert_eq!(
        scope(graph(), "macos_arm64").unwrap().emitted.machines(),
        &[machine_id(1)]
    );
    let mut called = graph();
    called.machines[0].blocks[0].operations[0].kind = call(2);
    assert_eq!(
        scope(called, "macos_arm64").unwrap().emitted.machines(),
        &[machine_id(1), machine_id(2), machine_id(3)]
    );
    let mut blocks = graph();
    let mut block = blocks.machines[0].blocks[0].clone();
    block.id = BlockId::new(2).unwrap();
    block.operations[0].id = OperationId::new(2).unwrap();
    block.operations[0].result = OperationResult::Scalar(ValueDeclaration {
        id: ValueId::new(2).unwrap(),
        scalar_type: scalar_type(),
        qualifications: Default::default(),
    });
    block.operations[0].kind = call(2);
    block.terminator = Terminator::Return {
        edge: EdgeId::new(2).unwrap(),
        value: ValueId::new(2).unwrap(),
        cleanup_actions: Vec::new(),
    };
    blocks.machines[0].blocks.push(block);
    assert_eq!(
        scope(blocks, "macos_arm64").unwrap().emitted.machines(),
        &[machine_id(1), machine_id(2), machine_id(3)]
    );
}

#[test]
fn boundary_demands_require_one_owner_across_reached_and_dormant_machines() {
    let mut module = graph();
    let operation = OperationId::new(77).unwrap();
    let mut identities = module.identities.clone().into_builder();
    let identity = identities.intern("Fixture::effect");
    module.identities = identities.build();
    let demand = terminal_psi::TerminalBoundaryApplicationDemand::new(
        operation,
        terminal_psi::BoundaryOperatorRequirement::new(
            terminal_psi::NominalDeclarationIdentity::new(identity),
            "effect".into(),
            None,
        )
        .unwrap(),
        terminal_psi::BoundaryApplication::Empty,
    );
    module.interfaces.boundary_application_demands.push(demand.clone());
    let reached = [machine_id(1)];
    assert_eq!(
        module.boundary_application_demands_for_machines(&reached),
        Err("Terminal boundary demand has no operation owner"),
    );
    module.machines[1].blocks[0].operations[0].id = operation;
    assert_eq!(
        module
            .boundary_application_demands_for_machines(&reached)
            .unwrap(),
        [],
    );
    module.machines[0].blocks[0].operations[0].id = operation;
    assert_eq!(
        module.boundary_application_demands_for_machines(&reached),
        Err("Terminal boundary demand has no unique operation owner"),
    );
    module.machines[1].blocks[0].operations[0].id = OperationId::new(2).unwrap();
    assert_eq!(
        module
            .boundary_application_demands_for_machines(&reached)
            .unwrap(),
        [demand],
    );
}

#[test]
fn emission_records_a_callback_only_when_its_registrar_runs() {
    use calling_conventions::{
        CallSignature, CallingPolicy, evaluate_ordinary_boundary_entry_plan,
    };
    use terminal_psi::{
        CheckedMachineDeclaration, NominalDeclarationIdentity, TerminalCallbackApplication,
    };

    let mut module = graph();
    let mut identities = module.identities.clone().into_builder();
    let name = identities.intern("Fixture::register");
    let callback = identities.intern("Fixture::callback");
    let unit = identities.intern("()");
    module.identities = identities.build();
    let declaration = CheckedMachineDeclaration {
        machine: NominalDeclarationIdentity::new(callback),
        state: NominalDeclarationIdentity::new(callback),
        semantic_declaration: callback,
        parameter_types: Vec::new(),
        callable: callback,
        result_type: unit,
        contract: [9; 32],
        published_may_suspend: false,
        published_may_block: false,
        specialization: None,
    };
    let boundary = BoundaryMachineId::new(1).unwrap();
    module
        .interfaces.boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            id: boundary,
            identity: name,
            owner: terminal_psi::NominalDeclarationIdentity::new(name),
            role: terminal_psi::BoundaryDeclarationRole::Callable,
            kind: terminal_psi::BoundaryRequirementKind::Machine,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            unconditional_termination: true,
            declaration: None,
            numeric_intrinsic: None,
            attachment: None,
            membership_requires: Vec::new(),
            membership_ensures: Vec::new(),
            normal_qualifications: Vec::new(),
            scalar_domain_requirements: Vec::new(),
            parameter_order: Vec::new(),
            scalar_parameters: Vec::new(),
            crash_routes: Vec::new(),
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Scalar(scalar_type().into()),
            requires: Vec::new(),
            scalar_requires: Vec::new(),
            scalar_ensures: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            result_reference_regions: Vec::new(),
            result_lifetimes: None,
            reference_outputs: Vec::new(),
            fixed_service_reach: Vec::new(),
            service_reach_is_installation_bound: false,
            owning_service: None,
            published_synchronous_entries: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    let inbound_boundary = BoundaryMachineId::new(2).unwrap();
    let mut inbound_requirement = module.interfaces.boundary_machines[0].clone();
    inbound_requirement.id = inbound_boundary;
    inbound_requirement.identity = callback;
    inbound_requirement.result = terminal_psi::BoundaryMachineResult::Unit;
    module.interfaces.boundary_machines.push(inbound_requirement);
    let registrar = OperationId::new(2).unwrap();
    let demand = terminal_psi::TerminalBoundaryApplicationDemand::new(
        registrar,
        terminal_psi::BoundaryOperatorRequirement::new(
            NominalDeclarationIdentity::new(name),
            "Fixture::register".into(),
            None,
        )
        .unwrap(),
        terminal_psi::BoundaryApplication::Empty,
    );
    module.interfaces.boundary_application_demands.push(demand.clone());
    module.machines[1].blocks[0].operations[0].id = registrar;
    module.machines[1].blocks[0].operations[0].kind = OperationKind::BoundaryCall {
        boundary,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        completion_receipts: Vec::new(),
        requirement_obligations: Vec::new(),
    };
    module.machines[2].blocks[0].operations.clear();
    module.machines[2].blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(3).unwrap(),
        cleanup_actions: Vec::new(),
    };
    module.machines[2].contract.result = TerminalMachineResult::Unit;
    module.machines[2].contract.declaration = Some(declaration.clone());
    module
        .interfaces.callback_applications
        .push(TerminalCallbackApplication {
            application_index: 0,
            registrar_machine: machine_id(2),
            registrar,
            static_machine_ordinal: 0,
            requirement: "Fixture::callback".into(),
            calling_owner_application: "Fixture".into(),
            boundary: Some(inbound_boundary),
            published_contract: declaration.contract,
            entry_admitted: true,
            body: declaration,
        });
    let record = terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
    let artifact = std::sync::Arc::new(
        terminal_codec::CanonicalTerminalArtifact::from_parts(module, record, None).unwrap(),
    );
    let thunk = provider_correspondence::callbacks::TerminalCallbackThunkArtifact::for_application(
        artifact.clone(),
        0,
    )
    .unwrap();
    let identity =
        function_identity::MachineFunctionIdentity::callback_thunk(artifact.view().identity(), 0)
            .unwrap();
    let inbound = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: Vec::new(),
            result: None,
        },
    )
    .unwrap();
    let occurrences = [
        provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal::new(
            0,
            0,
            registrar,
            identity,
            thunk,
            inbound.plan().clone(),
            Vec::new(),
        ),
    ];
    for (entry, reached) in [(machine_id(1), false), (machine_id(2), true)] {
        let scope = TargetReach::derive(ScopeInputs {
            target: "windows_x86_64",
            artifact: artifact.view(),
            entry,
            entry_call: None,
            adapters: &[],
            callbacks: &occurrences,
        })
        .unwrap();
        let expected = if reached {
            std::slice::from_ref(&demand)
        } else {
            &[]
        };
        let demands = artifact
            .semantic_module()
            .boundary_application_demands_for_machines(scope.emitted.machines())
            .unwrap();
        assert_eq!(demands, expected);
        assert_eq!(scope.emitted.contains_machine(machine_id(3)), reached);
        assert_eq!(scope.emitted.callbacks().len(), usize::from(reached));
        if reached {
            assert_eq!(scope.emitted.callbacks()[0].application_index, 0);
            assert_eq!(scope.emitted.callbacks()[0].terminal_operation, registrar);
        }
    }
}

#[test]
fn target_context_does_not_select_source_bodies() {
    let module = graph();
    assert_eq!(
        scope(module.clone(), "macos_arm64")
            .unwrap()
            .emitted
            .machines(),
        scope(module, "windows_x86_64").unwrap().emitted.machines()
    );
}
