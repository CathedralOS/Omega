use super::{validate_machine, validate_module};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId, OperationId, PlaceId, StructuralCaseId,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralMultiplicity, StructuralParameterDeclaration, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator,
};

fn machine(id: u64) -> TerminalMachine {
    let block = BlockId::new(1).unwrap();
    TerminalMachine {
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
        id: MachineId::new(id).unwrap(),
        attachment: None,
        ranked_scc: None,
        structural_places: Vec::new(),
        call_formations: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block,
        blocks: vec![Block::plain(
            block,
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        )],
        contract: terminal_psi::MachineContract {
            selected_application: None,
            declaration: None,
            parameters: Vec::new(),
            parameter_order: Vec::new(),
            structural_parameters: Vec::new(),
            result: TerminalMachineResult::Unit,
            entry_claims: Vec::new(),
            declared_service_reach: None,
            closed_reach_application: None,
            published_synchronous_entries: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            ..terminal_psi::MachineContract::empty(ContractId::new(1).unwrap())
        },
    }
}

#[test]
fn module_activation_batch_preserves_global_error_priority_and_machine_order() {
    let mut module = TerminalModule::declarations();
    assert_eq!(validate_module(&module), Ok(()));
    module.machines = vec![machine(1), machine(2)];
    assert_eq!(validate_module(&module), Ok(()));
    for machine in &module.machines {
        assert_eq!(validate_machine(&module, machine), Ok(0));
    }

    module.machines[1].blocks.clear();
    assert_eq!(
        validate_module(&module),
        Err("activation control target is absent")
    );
    module.machines[1].id = module.machines[0].id;
    assert_eq!(
        validate_module(&module),
        Err("activation control target is absent")
    );
    let parameter = |ty| StructuralParameterDeclaration {
        place: PlaceId::new(1).unwrap(),
        position: 0,
        structural_type: StructuralTypeId::new(ty).unwrap(),
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    };
    module.machines[0].structural_parameters = vec![parameter(1), parameter(2)];
    let roots = Err("activation carrier root has inconsistent structural types");
    let absent = Err("activation control target is absent");
    assert_eq!(validate_module(&module), roots);
    module.machines.swap(0, 1);
    assert_eq!(validate_module(&module), absent);

    module.machines[1].blocks[0]
        .operations
        .push(Operation::unsuspended(
            OperationId::new(1).unwrap(),
            OperationResult::Unit,
            OperationKind::EstablishActivationClaim {
                bound: 1,
                committed: ValueId::new(1).unwrap(),
                backing_type: StructuralTypeId::new(1).unwrap(),
                established_case: StructuralCaseId::new(1).unwrap(),
                rejected_case: StructuralCaseId::new(2).unwrap(),
            },
        ));
    let global = Err("activation establishment requires a structural result");
    assert_eq!(validate_module(&module), global);
    assert_eq!(
        validate_machine(&module, &module.machines[0]).map(|_| ()),
        global
    );
    module.machines[1].blocks[0].operations.clear();
    assert_eq!(validate_module(&module), absent);

    module.machines = vec![machine(1), machine(2)];
    assert_eq!(validate_module(&module), Ok(()));
    let mut external = machine(3);
    external.blocks.clear();
    assert_eq!(validate_machine(&module, &external).map(|_| ()), absent);
}
