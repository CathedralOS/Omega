use super::{
    TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact,
    admitted_native_callback_thunks,
};
use crate::callback_placements::tests::{materialized_placement, placement};

fn occurrence(
    placement_index: usize,
    placement: &crate::BoundNominalCallbackPlacement,
) -> TerminalCallbackOccurrenceProposal {
    let entry = semantic_vocabulary::MachineId::new(1).unwrap();
    let mut module = terminal_psi::TerminalModule::for_entry(entry);
    module.machines.push(machine(1, 1));
    let mut lowering_receipt = receipt(&module);
    lowering_receipt.source_machine = placement.selected_machine;
    lowering_receipt.source_entry = placement.selected_entry;
    TerminalCallbackOccurrenceProposal::new(
        placement_index,
        semantic_vocabulary::OperationId::new(7).unwrap(),
        placement
            .private_materialization
            .as_ref()
            .map(|materialization| {
                materialization
                    .destination_parameter_application()
                    .unwrap()
                    .clone()
            }),
        crate::canonical_callback_thunk_identity(placement_index, placement).unwrap(),
        TerminalCallbackThunkArtifact::new(
            crate::canonical_callback_private_symbol(placement),
            thunk_artifact(&module),
            lowering_receipt,
        )
        .unwrap(),
    )
}

fn block(id: u64) -> terminal_psi::Block {
    terminal_psi::Block {
        id: semantic_vocabulary::BlockId::new(id).expect("nonzero block id"),
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: terminal_psi::Terminator::ReturnUnit {
            edge: semantic_vocabulary::EdgeId::new(id).expect("nonzero edge id"),
            cleanup_actions: Vec::new(),
        },
    }
}

fn machine(id: u64, entry_block: u64) -> terminal_psi::TerminalMachine {
    terminal_psi::TerminalMachine {
        id: semantic_vocabulary::MachineId::new(id).expect("nonzero machine id"),
        attachment: None,
        nominal_cleanup_subject: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: terminal_psi::TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: semantic_vocabulary::BlockId::new(entry_block).expect("nonzero block id"),
        blocks: vec![block(entry_block)],
        contract: terminal_psi::MachineContract {
            id: semantic_vocabulary::ContractId::new(id).expect("nonzero contract id"),
            crash_routes: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn thunk_artifact(
    module: &terminal_psi::TerminalModule,
) -> terminal_codec::CanonicalTerminalArtifact {
    let optimization = terminal_codec::build_identity_optimization_execution_record(module)
        .expect("identity optimization execution");
    terminal_codec::CanonicalTerminalArtifact::from_parts(module, &optimization, None)
        .expect("canonical Terminal artifact")
}

fn receipt(module: &terminal_psi::TerminalModule) -> lowered_psi::CallbackTerminalLoweringReceipt {
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("test module contains its entry machine");
    lowered_psi::CallbackTerminalLoweringReceipt {
        source_machine: symbols::SymbolHandle::invalid(),
        source_entry: symbols::SymbolHandle::invalid(),
        terminal_machine: module.entry,
        terminal_entry: entry.entry,
    }
}

#[test]
fn thunk_artifact_binds_its_entry_machine_beside_lowered_callees() {
    // A call-bearing callback body lowers its callees beside the entry
    // machine; the artifact binds the entry join, not a machine count.
    let entry = semantic_vocabulary::MachineId::new(1).expect("nonzero machine id");
    let mut module = terminal_psi::TerminalModule::for_entry(entry);
    module.machines.push(machine(1, 1));
    module.machines.push(machine(2, 2));
    let artifact = thunk_artifact(&module);
    TerminalCallbackThunkArtifact::new(
        std::sync::Arc::from("omega.callback.test"),
        artifact,
        receipt(&module),
    )
    .expect("entry-bound artifact with lowered callees");
}

#[test]
fn thunk_artifact_rejects_a_receipt_drifted_from_its_entry() {
    let entry = semantic_vocabulary::MachineId::new(1).expect("nonzero machine id");
    let mut module = terminal_psi::TerminalModule::for_entry(entry);
    module.machines.push(machine(1, 1));
    module.machines.push(machine(2, 2));
    let artifact = thunk_artifact(&module);
    let mut drifted = receipt(&module);
    drifted.terminal_machine = semantic_vocabulary::MachineId::new(2).expect("nonzero");
    assert!(
        TerminalCallbackThunkArtifact::new(
            std::sync::Arc::from("omega.callback.test"),
            artifact,
            drifted,
        )
        .is_err()
    );
}

#[test]
fn thunk_artifact_rejects_a_receipt_drifted_from_its_entry_block() {
    let entry = semantic_vocabulary::MachineId::new(1).expect("nonzero machine id");
    let mut module = terminal_psi::TerminalModule::for_entry(entry);
    module.machines.push(machine(1, 1));
    let artifact = thunk_artifact(&module);
    let mut drifted = receipt(&module);
    drifted.terminal_entry = semantic_vocabulary::BlockId::new(9).expect("nonzero block id");
    assert!(
        TerminalCallbackThunkArtifact::new(
            std::sync::Arc::from("omega.callback.test"),
            artifact,
            drifted,
        )
        .is_err()
    );
}
