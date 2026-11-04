//! Equal semantic code from independent invocation owners cannot share admission.

use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId};
use terminal_interpreter::{TerminalInstantiation, TerminalProgram, TerminalRuntimeSupplyRequest};
use terminal_psi::{Block, MachineContract, TerminalMachine, TerminalModule, Terminator};

fn invocation() -> terminal_codec::InvocationTerminalCode {
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    let mut module = TerminalModule::for_entry(machine);
    module.identities = terminal_psi::TerminalIdentityTableBuilder::for_invocation().build();
    module.machines.push(TerminalMachine {
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
        id: machine,
        declaration: None,
        nominal_cleanup_subject: None,
        attachment: None,
        parameters: Vec::new(),
        parameter_order: Vec::new(),
        structural_parameters: Vec::new(),
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
        result: terminal_psi::TerminalMachineResult::Unit,
        entry: block,
        blocks: vec![Block::plain(
            block,
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        )],
        ranked_scc: None,
        contract: MachineContract::empty(ContractId::new(1).unwrap()),
    });
    terminal_codec::InvocationTerminalCode::new(module).unwrap()
}

#[test]
fn equal_semantics_do_not_transfer_invocation_installation_or_intrinsics() {
    let original = invocation();
    let other = invocation();
    assert_eq!(original.view().identity(), other.view().identity());
    assert_ne!(
        original.view().custody_identity(),
        other.view().custody_identity()
    );
    let installed =
        terminal_installation::admit_provider_execution_installation(original.view(), &[], &[])
            .unwrap();
    let selected = Default::default();
    let intrinsics = provider_admission::provider_admission::admit_terminal_intrinsics(
        original.view(),
        &selected,
        std::iter::empty(),
        &Default::default(),
    )
    .unwrap();
    for (code, matches) in [(&original, true), (&other, false)] {
        let program = TerminalProgram::borrow(code.view());
        let supplies = program.admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            installation: Some(&installed),
            host_imports: &[],
            native_code_layouts: None,
            entry: None,
            x86_flags: None,
            memory_ordering: None,
            target_machines: None,
            target_layouts: None,
        });
        assert_eq!(supplies.is_ok(), matches);
        let mut instance = program
            .instantiate(TerminalInstantiation::default())
            .unwrap();
        assert_eq!(
            super::host_imports::bind_admitted_intrinsics(&mut instance, &intrinsics).is_ok(),
            matches
        );
    }
}
