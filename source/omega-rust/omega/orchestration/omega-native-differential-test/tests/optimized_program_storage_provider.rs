use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use omega_effects::provider_plan::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceSchema,
};
use omega_lowering_optimizer::lower_optimized_to_target_operations_with_provider_executions_and_installation;
use omega_native_differential_test::admit_native_provider;
use omega_optimization_core::{Optimization, OptimizationSelections};
use omega_optimization_pipeline::{
    StagedOptimizedFunctionFragmentEmissionSource, compiler_baseline_request_v1,
    optimize_verified_terminal_input, stage_optimized_allocation_legality,
    stage_optimized_function_fragment_emission, stage_optimized_instruction_selection,
    stage_optimized_live_ranges, stage_optimized_liveness, stage_optimized_register_homes,
    stage_optimized_relocation_free_terminal_object_container,
    stage_optimized_relocation_free_text_section,
    stage_optimized_structural_unit_function_relative_realization,
    stage_validated_optimized_terminal_object_artifact,
    validate_optimized_terminal_object_artifact,
};
use omega_optimization_unit::OwnershipEvent;
use omega_program_storage::{
    ProgramEntryPhysicalContractPlan, ProgramEntrySourceReceiverSignature,
    ProgramStorageEntryRootRole, SelectedProgramEntrySourceSignature,
    SelectedProgramStorageEntryPlan, bind_optimized_program_storage_semantic_entry_contract,
    plan_optimized_program_storage_semantic_wrapper,
};
use omega_target::{NativeTarget, ProgramEntryPhysicalContractPackage, TargetProfile};
use omega_terminal_abstract_operations_to_target_operations::AdmittedTerminalBoundarySettlement;
use omega_terminal_native_realization::{
    OptimizedProgramStorageSemanticWrapperObjectSymbolRole, TerminalNativeProgramEntrySettlement,
    TerminalNativeProgramEntrySettlementError,
    select_optimized_program_storage_semantic_wrapper_encoding,
    stage_validated_optimized_program_storage_semantic_wrapper_object,
    validate_optimized_program_storage_semantic_wrapper_object,
    validate_terminal_native_program_entry_settlement,
};
use omega_terminal_psi_to_abstract_operations::{
    SelectedProviderAdapter, admit_provider_installation, lower_artifact_sections_for_optimization,
};
use omega_terminal_target_operations::{
    TerminalBoundaryRealization, TerminalClaimCompletionOnlyRealization, TerminalTargetOperation,
    TerminalTargetUnitOperation,
};
use psi_language_semantics::{CarryPolicy, DomainPredicateBody};
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
    let source_entry = program_storage_source_entry(&typed);
    let checked = lower_typed_trees(typed.clone()).expect("check ProgramStorage provider source");
    let produced = psi_checked_trees_to_terminal::produce_program_entry_terminal_artifact(
        &checked,
        "ProgramLocalProducer::handoff",
        source_entry.identity().bytes(),
    )
    .expect("produce receipt-coupled ProgramStorage artifact");
    let (terminal, checked_entry) = produced.into_parts();
    let semantic = terminal.semantic_bytes();
    let proof = terminal.proof_bytes();
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

    let liveness = stage_optimized_liveness(selected_stage)
        .expect("installed ProgramStorage plan reaches architectural liveness");
    let ranges = stage_optimized_live_ranges(liveness)
        .expect("installed ProgramStorage plan reaches zero-VReg ranges");
    let legality = stage_optimized_allocation_legality(ranges)
        .expect("installed ProgramStorage plan reaches empty allocation legality");
    let homes = stage_optimized_register_homes(legality)
        .expect("installed ProgramStorage plan reaches empty register homes");
    let realization = stage_optimized_structural_unit_function_relative_realization(homes)
        .expect("installed ProgramStorage plan reaches function-relative realization");
    let fragments = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::StructuralUnit(Box::new(realization)),
    )
    .expect("installed ProgramStorage plan reaches function fragments");
    let text = stage_optimized_relocation_free_text_section(fragments)
        .expect("installed ProgramStorage plan resolves its internal call");
    let object = stage_optimized_relocation_free_terminal_object_container(text)
        .expect("installed ProgramStorage plan reaches canonical object custody");
    assert_eq!(object.object().symbols.len(), 2);
    assert_eq!(object.object().text_section.bytes.len(), 91);
    assert_eq!(object.object().relocation_record_count, 0);
    assert!(object.provider_installation().is_some());
    let artifact = stage_validated_optimized_terminal_object_artifact(terminal, object)
        .expect("installed ProgramStorage object rejoins canonical Terminal custody");
    assert_eq!(artifact.artifact().statistics.function_symbols, 2);
    assert_eq!(artifact.artifact().statistics.text_bytes, 91);
    assert_eq!(artifact.artifact().statistics.relocation_records, 0);
    let retained_installation = artifact
        .provider_installation()
        .expect("canonical object retains opaque checked-provider custody");
    assert_eq!(
        retained_installation.terminal_psi(),
        artifact.artifact().terminal_psi
    );
    validate_optimized_terminal_object_artifact(&artifact)
        .expect("installed ProgramStorage object independently replays");

    let semantic_call = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![ValueShape::integer(16, 8); 2],
            result: None,
        },
    )
    .expect("ProgramStorage semantic Microsoft-x64 call plan");
    let storage_entry = program_storage_entry_plan(&source_entry, &candidate, &semantic_call);
    let substituted_source = SelectedProgramEntrySourceSignature::from_checked_typed_entry(
        source_entry.target_slot(),
        source_entry.machine_symbol(),
        source_entry.state_symbol(),
        source_entry.machine_name().into(),
        source_entry.state_name().into(),
        format!(
            "{}::substituted",
            source_entry.normalized_callable_identity()
        ),
        source_entry.receiver().clone(),
        source_entry.visible_parameters().to_vec(),
    )
    .expect("construct a well-shaped but identity-substituted source entry");
    assert_eq!(
        validate_terminal_native_program_entry_settlement(
            artifact.terminal(),
            &checked_entry,
            TerminalNativeProgramEntrySettlement::new(
                &substituted_source,
                Some((semantic_call.plan(), &storage_entry)),
            ),
            NativeTarget::uefi_x64(),
        ),
        Err(TerminalNativeProgramEntrySettlementError::SourceSignatureSubstitution)
    );
    let settlement = validate_terminal_native_program_entry_settlement(
        artifact.terminal(),
        &checked_entry,
        TerminalNativeProgramEntrySettlement::new(
            &source_entry,
            Some((semantic_call.plan(), &storage_entry)),
        ),
        NativeTarget::uefi_x64(),
    )
    .expect("settle checked ProgramStorage entry against canonical object");
    let contract = bind_optimized_program_storage_semantic_entry_contract(
        NativeTarget::uefi_x64(),
        &storage_entry,
        &source_entry,
        semantic_call.plan(),
    )
    .expect("bind exact semantic ProgramStorage contract");
    let wrapper = plan_optimized_program_storage_semantic_wrapper(contract)
        .expect("derive address-free semantic wrapper");
    let encoding = select_optimized_program_storage_semantic_wrapper_encoding(wrapper)
        .expect("select compact Microsoft-x64 wrapper encoding");
    let composite = stage_validated_optimized_program_storage_semantic_wrapper_object(
        settlement, artifact, encoding,
    )
    .expect("compose checked installed continuation with semantic wrapper");
    validate_optimized_program_storage_semantic_wrapper_object(&composite)
        .expect("independently replay checked installed wrapper object");
    assert_eq!(composite.object().text_bytes.len(), 181);
    assert_eq!(composite.object().symbols.len(), 3);
    assert_eq!(
        composite.object().symbols[0].role,
        OptimizedProgramStorageSemanticWrapperObjectSymbolRole::SemanticWrapperV1
    );
    assert!(composite.object().symbols[0].machine.is_none());
}

fn program_storage_source_entry(
    typed: &psi_typed_trees::TypedTrees,
) -> SelectedProgramEntrySourceSignature {
    let slot = TargetProfile::UefiX64.program_entry_slot();
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "ProgramLocalProducer::handoff")
        .expect("checked ProgramStorage source machine");
    let entry = typed
        .machine_states(machine)
        .first()
        .expect("checked ProgramStorage source entry state");
    let visible_parameters = typed
        .state_parameters(entry)
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let role = match index {
                0 => ProgramStorageEntryRootRole::Image,
                1 => ProgramStorageEntryRootRole::InitialStorage,
                _ => panic!("ProgramStorage entry has exactly two visible roots"),
            };
            let extent = omega_provider_planning::calling_policy_plans::selected_program_storage_source_extent_value_layout(
                typed,
                slot,
                parameter.type_reference,
            )
            .expect("derive checked Extent source layout");
            SelectedProgramEntrySourceSignature::visible_parameter(
                role,
                index,
                typed
                    .normalized_type_identity(parameter.type_reference)
                    .into_string(),
                extent.shape(),
                extent,
                parameter.is_const,
                parameter.is_mutable,
            )
        })
        .collect();
    SelectedProgramEntrySourceSignature::from_checked_typed_entry(
        slot,
        machine.symbol,
        entry.symbol,
        machine.name.as_str().into(),
        entry.name.as_str().into(),
        typed
            .normalized_machine_overload_identity(machine)
            .expect("one checked ProgramStorage entry overload")
            .identity(),
        ProgramEntrySourceReceiverSignature::Free,
        visible_parameters,
    )
    .expect("retain exact checked ProgramStorage source signature")
}

fn program_storage_entry_plan(
    source: &SelectedProgramEntrySourceSignature,
    candidate: &psi_terminal::ProviderCandidateConformance,
    semantic_call: &omega_calling_conventions::ValidatedBoundaryEntryPlan,
) -> SelectedProgramStorageEntryPlan {
    let slot = TargetProfile::UefiX64.program_entry_slot();
    let claim = |parameter_index| ServiceEntryClaim {
        parameter_index,
        carrier_identity: "named(name(Extent))".into(),
        domain: "Extent::Granted".into(),
        predicate_body: DomainPredicateBody::Present,
        effective_carry: CarryPolicy::STRICT,
        authority_flow: ServiceEntryAuthorityFlow::Accepts,
    };
    let storage = SelectedProgramStorageEntryPlan::from_target_slot(
        slot,
        ServiceSchema {
            trait_name: slot.boundary_schema.expect("UEFI boundary schema").into(),
            methods: vec![ServiceMethod {
                name: "enter".into(),
                requirement_owner: "ProgramStorageEntry".into(),
                requirement_identity: candidate.requirement_identity.clone(),
                parameter_count: 2,
                parameter_type_identities: source
                    .visible_parameters()
                    .iter()
                    .map(|parameter| parameter.normalized_type_identity().into())
                    .collect(),
                entry_claims: vec![claim(0), claim(1)],
                calling_plan_fingerprint: Some(semantic_call.contract_fingerprint()),
                ..Default::default()
            }],
            ..Default::default()
        },
        candidate.requirement_identity.clone(),
    )
    .expect("select exact semantic ProgramStorage entry");
    let pointer = ValueShape::integer(8, 8);
    let physical = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![pointer; 2],
            result: Some(pointer),
        },
    )
    .expect("UEFI physical Microsoft-x64 call plan");
    storage
        .with_physical_contract(
            ProgramEntryPhysicalContractPlan::new(
                slot,
                "UefiPhysicalEntry::enter".into(),
                ProgramEntryPhysicalContractPackage::UefiX64,
                1,
                vec!["EfiImageHandle".into(), "&EfiSystemTable".into()],
                "EfiStatus".into(),
                physical.contract_fingerprint(),
                physical.plan().clone(),
            )
            .expect("retain non-invoked UEFI physical contract"),
        )
        .expect("pair ProgramStorage semantic and physical plans")
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
