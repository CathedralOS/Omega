use omega_calling_conventions::{CallSignature, ValueShape};
use omega_lowering_optimizer::lower_optimized_to_target_operations_with_provider_executions_and_installation;
use omega_native_differential_test::admit_native_provider;
use omega_optimization_core::{Optimization, OptimizationSelections};
use omega_optimization_pipeline::{
    compiler_baseline_request_v1, optimize_verified_terminal_input,
    stage_optimized_instruction_selection,
};
use omega_optimization_unit::OwnershipEvent;
use omega_target::NativeTarget;
use omega_terminal_abstract_operations_to_target_operations::AdmittedTerminalBoundarySettlement;
use omega_terminal_psi_to_abstract_operations::{
    SelectedProviderAdapter, admit_provider_installation, lower_artifact_sections_for_optimization,
};
use omega_terminal_target_operations::{
    TerminalBoundaryRealization, TerminalClaimCompletionOnlyRealization, TerminalTargetOperation,
    TerminalTargetUnitOperation,
};
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const PROGRAM_STORAGE_PROVIDER_SOURCE: &str = r#"
    pub data Extent [linear] {
        base: addr;
        length: u64;
    }

    pub boundary machine no_wrap(base: addr, length: u64) -> bool;

    pub domain Extent::Granted
    requires
        no_wrap(self.base, self.length)
    established by
        ProgramStorageEntry::enter;

    boundary trait ProgramStorageEntry {
        machine enter(
            image: Extent in Granted,
            initial_storage: Extent in Granted
        );
    }

    boundary machine Extent::settle(self)
    requires
        self in Extent::Granted
    ensures true;

    data ProgramStorageProvider {}
    machine ProgramStorageProvider::enter(
        image: Extent in Granted,
        initial_storage: Extent in Granted
    )
    satisfies ProgramStorageEntry::enter
    {
        image.settle();
        initial_storage.settle();
    }

    data ProgramLocalProducer {}
    machine ProgramLocalProducer::handoff<machine Enter>(
        image: Extent in Granted,
        initial_storage: Extent in Granted
    )
    where machine Enter satisfies ProgramStorageEntry::enter;
    {
        Enter(image, initial_storage);
    }
"#;

#[test]
fn checked_program_storage_provider_reaches_optimized_selected_claim_completion() {
    let tokens = Lexer::new(PROGRAM_STORAGE_PROVIDER_SOURCE)
        .tokenize()
        .expect("tokenize ProgramStorage provider source");
    let syntax = parse_syntax_trees(&tokens).expect("parse ProgramStorage provider source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve ProgramStorage provider source");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type ProgramStorage provider source");
    let checked = lower_typed_trees(typed).expect("check ProgramStorage provider source");
    let produced = psi_checked_trees_to_terminal::produce_program_entry_terminal_artifact(
        &checked,
        "ProgramLocalProducer::handoff",
        [0xa5; 32],
    )
    .expect("produce receipt-coupled ProgramStorage artifact");
    let semantic = produced.artifact().semantic_bytes();
    let proof = produced.artifact().proof_bytes();
    let profile = AdmissionProfile::default();
    let verified = lower_artifact_sections_for_optimization(semantic, proof, &profile)
        .expect("admit verified optimizer input");

    let [candidate] = verified.plan().provider_candidates.as_slice() else {
        panic!("one exact checked ProgramStorage provider candidate")
    };
    let candidate = candidate.clone();
    let settlement_boundary = verified
        .plan()
        .boundary_machines
        .iter()
        .find(|boundary| boundary.identity == "Extent::settle")
        .expect("Extent::settle boundary declaration");
    let settlement_boundary_id = settlement_boundary.id;
    let settlement_requirement = settlement_boundary.identity.clone();
    let installation = admit_provider_installation(
        verified.plan(),
        semantic,
        proof,
        &profile,
        &[SelectedProviderAdapter {
            requirement_identity: candidate.requirement_identity.clone(),
            provider_identity: candidate.provider_identity.clone(),
            machine_identity: candidate.candidate_identity.clone(),
        }],
    )
    .expect("admit exact checked ProgramStorage provider installation");

    let selections =
        OptimizationSelections::new([Optimization::CopyPropagation]).expect("named optimization");
    let request = compiler_baseline_request_v1(&selections).expect("bounded optimizer request");
    let optimized = optimize_verified_terminal_input(verified, request)
        .expect("optimize verified ProgramStorage plan");
    let root_machine = optimized.plan().entry;
    let provider_machine = candidate.candidate;
    let provider_claims = optimized
        .plan()
        .functions
        .iter()
        .find(|function| function.machine == provider_machine)
        .expect("installed provider function remains in the optimized plan")
        .entry_claims
        .iter()
        .map(|claim| claim.claim)
        .collect::<Vec<_>>();
    assert_eq!(provider_claims.len(), 2);
    let provider_execution = admit_native_provider(
        NativeTarget::uefi_x64(),
        &settlement_requirement,
        0x5e77_1e,
        CallSignature {
            parameters: vec![ValueShape::integer(16, 8)],
            result: None,
        },
    );
    let settlements = [AdmittedTerminalBoundarySettlement {
        boundary: settlement_boundary_id,
        provider_execution: &provider_execution,
        realization: TerminalClaimCompletionOnlyRealization.into(),
    }];
    let optimized_target =
        lower_optimized_to_target_operations_with_provider_executions_and_installation(
            optimized,
            NativeTarget::uefi_x64(),
            &settlements,
            installation,
        )
        .expect("lower optimized plan with checked installation and admitted settlement");
    let retained_installation = optimized_target
        .provider_installation()
        .expect("optimized target retains opaque checked-provider custody");
    assert_eq!(
        retained_installation.terminal_psi(),
        optimized_target.target_operations().terminal_psi
    );

    let target_root = target_unit_body(optimized_target.target_operations(), root_machine);
    let [
        TerminalTargetUnitOperation::InstalledProviderCall {
            boundary,
            provider,
            completion_receipts,
            ..
        },
        TerminalTargetUnitOperation::Return { .. },
    ] = target_root.operations.as_slice()
    else {
        panic!("root must be one installed provider call followed by ReturnUnit")
    };
    assert_eq!(*boundary, candidate.boundary);
    assert_eq!(provider, &candidate);
    assert_eq!(completion_receipts.len(), 2);

    let target_provider = target_unit_body(optimized_target.target_operations(), provider_machine);
    let [first, second, TerminalTargetUnitOperation::Return { .. }] =
        target_provider.operations.as_slice()
    else {
        panic!("provider must retain two ordered settlements followed by ReturnUnit")
    };
    let target_settlement_claims = [first, second].map(|operation| match operation {
        TerminalTargetUnitOperation::BoundarySettlement {
            boundary,
            realization: TerminalBoundaryRealization::ClaimCompletionOnly(_),
            scalar_arguments,
            arguments,
            byte_sequence_arguments,
            completion_receipts,
            ..
        } => {
            assert_eq!(*boundary, settlement_boundary_id);
            assert!(scalar_arguments.is_empty());
            assert!(byte_sequence_arguments.is_empty());
            assert_eq!(arguments.len(), 1);
            let [receipt] = completion_receipts.as_slice() else {
                panic!("each Extent::settle completes exactly one claim")
            };
            receipt.claim
        }
        _ => panic!("provider operation must be ClaimCompletionOnly settlement"),
    });
    assert_eq!(
        target_settlement_claims.as_slice(),
        provider_claims.as_slice()
    );

    let selected_stage = stage_optimized_instruction_selection(optimized_target)
        .expect("legalize and select installed ProgramStorage plan");
    let legalized = selected_stage.legalized().plan();
    let legalized_root = legalized
        .structural_unit_functions
        .iter()
        .find(|function| function.machine == root_machine)
        .expect("legalized root structural function");
    let legalized_root_call = legalized_root
        .call
        .as_ref()
        .expect("legalized installed call");
    assert_eq!(
        legalized_root_call.ownership,
        [OwnershipEvent::ClaimCompletion(
            legalized_root
                .entry_claims
                .iter()
                .map(|claim| claim.claim)
                .collect()
        )]
    );
    let legalized_provider = legalized
        .structural_unit_functions
        .iter()
        .find(|function| function.machine == provider_machine)
        .expect("legalized provider structural function");
    assert_eq!(legalized_provider.boundary_settlements.len(), 2);
    for (settlement, claim) in legalized_provider
        .boundary_settlements
        .iter()
        .zip(&provider_claims)
    {
        assert_eq!(
            settlement.ownership,
            [OwnershipEvent::ClaimCompletion(vec![*claim])]
        );
    }

    let selected = selected_stage.selected();
    assert!(selected.plan().functions.is_empty());
    assert_eq!(selected.plan().structural_unit_functions.len(), 2);
    let selected_root = selected
        .plan()
        .structural_unit_functions
        .iter()
        .find(|function| function.machine == root_machine)
        .expect("selected root structural function");
    let selected_root_call = selected_root
        .call
        .as_ref()
        .expect("selected installed call");
    assert_eq!(selected_root_call.ownership, legalized_root_call.ownership);
    assert!(selected_root.boundary_settlements.is_empty());
    assert_eq!(selected_root.terminator.instruction.id.0, 1);
    let selected_provider = selected
        .plan()
        .structural_unit_functions
        .iter()
        .find(|function| function.machine == provider_machine)
        .expect("selected provider structural function");
    assert!(selected_provider.call.is_none());
    assert_eq!(selected_provider.boundary_settlements.len(), 2);
    assert_eq!(
        selected_provider.boundary_settlements,
        legalized_provider.boundary_settlements
    );
    assert_eq!(selected_provider.terminator.instruction.id.0, 0);
    assert_eq!(selected.receipt().virtual_register_count(), 0);
    assert_eq!(selected.receipt().instruction_count(), 3);
}

fn target_unit_body(
    plan: &omega_terminal_target_operations::TerminalTargetOperationPlan,
    machine: psi_core::MachineId,
) -> &omega_terminal_target_operations::TerminalTargetUnitBody {
    let function = plan
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("target Unit function");
    let TerminalTargetOperation::UnitBody(body) = &function.operation else {
        panic!("ProgramStorage function must lower as structural Unit body")
    };
    body
}
