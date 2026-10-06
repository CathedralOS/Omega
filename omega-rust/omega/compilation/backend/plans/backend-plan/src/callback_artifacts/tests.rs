use super::{
    TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact,
    admitted_native_callback_thunks, validate_callback_occurrence_placements,
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

#[test]
fn native_thunks_borrow_exact_artifacts_and_preserve_occurrence_order() {
    let first = placement();
    let mut second = first.clone();
    second.static_machine_ordinal += 1;
    let placements = [first, second];
    let occurrences = [occurrence(1, &placements[1]), occurrence(0, &placements[0])];
    validate_callback_occurrence_placements(&placements, &occurrences).unwrap();
    let admitted = admitted_native_callback_thunks(&placements, &occurrences).unwrap();
    for (settlement, occurrence) in admitted.iter().zip(&occurrences) {
        assert_eq!(settlement.placement_index, occurrence.placement_index());
        assert_eq!(
            settlement.terminal_operation,
            occurrence.terminal_operation()
        );
        assert_eq!(
            settlement.callback_function,
            occurrence.callback_thunk_identity()
        );
        assert_eq!(
            settlement.private_symbol,
            occurrence
                .callback_thunk_artifact()
                .private_symbol()
                .as_ref()
        );
        assert!(std::ptr::eq(
            settlement.artifact,
            occurrence.callback_thunk_artifact().artifact()
        ));
        assert!(std::ptr::eq(
            settlement.boundary_entry_plan,
            &placements[occurrence.placement_index()].boundary_entry_plan
        ));
    }
    validate_callback_occurrence_placements(&[], &[]).unwrap();
    assert!(
        admitted_native_callback_thunks(&[], &[])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn retained_occurrences_require_exact_placement_coverage() {
    let placements = [placement(), placement()];
    let first = occurrence(0, &placements[0]);
    assert_eq!(
        validate_callback_occurrence_placements(&placements, std::slice::from_ref(&first)),
        Err("Terminal native proposal does not cover every retained callback placement"),
    );
    assert_eq!(
        validate_callback_occurrence_placements(&placements, &[first.clone(), first.clone()]),
        Err("Terminal native proposal does not uniquely bind a retained callback placement"),
    );
    assert_eq!(
        admitted_native_callback_thunks(&[], &[first]).unwrap_err(),
        "callback thunk occurrence names an absent retained placement",
    );
}

#[test]
fn source_symbol_and_function_substitutions_reject_before_native_projection() {
    let placement = placement();
    let baseline = occurrence(0, &placement);
    let mut wrong_machine = baseline.clone();
    wrong_machine
        .callback_thunk_artifact
        .lowering_receipt
        .source_machine = symbols::SymbolHandle::from_parts(4, 99);
    let mut wrong_entry = baseline.clone();
    wrong_entry
        .callback_thunk_artifact
        .lowering_receipt
        .source_entry = symbols::SymbolHandle::from_parts(5, 99);
    let mut wrong_symbol = baseline.clone();
    wrong_symbol.callback_thunk_artifact.private_symbol = std::sync::Arc::from("substituted");
    let mut wrong_function = baseline;
    wrong_function.callback_thunk_identity =
        crate::canonical_callback_thunk_identity(1, &placement).unwrap();
    for (substitution, expected) in [
        (
            wrong_machine,
            "Terminal callback occurrence lowering receipt drifted from its retained placement",
        ),
        (
            wrong_entry,
            "Terminal callback occurrence lowering receipt drifted from its retained placement",
        ),
        (
            wrong_symbol,
            "Terminal callback occurrence private symbol drifted from its retained placement",
        ),
        (
            wrong_function,
            "Terminal callback occurrence thunk continuation drifted from its retained placement",
        ),
    ] {
        let occurrences = [substitution];
        assert_eq!(
            validate_callback_occurrence_placements(std::slice::from_ref(&placement), &occurrences),
            Err(expected)
        );
        assert_eq!(
            admitted_native_callback_thunks(std::slice::from_ref(&placement), &occurrences)
                .unwrap_err(),
            "callback thunk body, symbol, or function identity drifted from its retained placement",
        );
    }
}

#[test]
fn retained_occurrences_replay_direct_and_field_destinations() {
    let parameter = calling_conventions::NativeParameterId::new(107).unwrap();
    for destination in [
        calling_conventions::NativePlace::Parameter(parameter),
        calling_conventions::NativePlace::Field {
            parameter,
            layout: calling_conventions::LayoutPlanId::new(109).unwrap(),
            field_path: vec![calling_conventions::LayoutSlotId::new(113).unwrap()],
        },
    ] {
        let placement = materialized_placement(destination);
        let mut occurrence = occurrence(0, &placement);
        validate_callback_occurrence_placements(
            std::slice::from_ref(&placement),
            std::slice::from_ref(&occurrence),
        )
        .unwrap();
        occurrence
            .destination_parameter_application
            .as_mut()
            .unwrap()
            .native_ordinal += 1;
        assert_eq!(
            validate_callback_occurrence_placements(&[placement], &[occurrence]),
            Err(
                "Terminal callback occurrence native parameter application drifted from its retained placement"
            ),
        );
    }
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
            trivial_affine_discards: Vec::new(),
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
    let proof = terminal_psi::ProofBundle::default();
    let optimization = terminal_codec::build_identity_optimization_execution_record(module, &proof)
        .expect("identity optimization execution");
    terminal_codec::CanonicalTerminalArtifact::from_parts(module, &proof, &optimization, None)
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
