use super::*;
use std::collections::BTreeSet;

use crate::realization::project_selected_provider_adapters_for_requirements;
use omega_effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
};
use omega_psi_to_abstract_operations::SelectedProviderAdapter;
use psi_checked_trees_to_terminal::CheckedProgramEntryTerminalReceipt;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

fn checked(source: &str) -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

const RANKED_COUNTDOWN_SOURCE: &str = r#"
    data Token { value: i32; }
    data Root {}

    machine Root::countdown(token: Token, remaining: u32)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> countdown(token, remaining - 1)
            _ -> done(token)
        }
        state done(token: Token) {}
    }
"#;

#[test]
fn ranked_native_dispatch_emits_exact_machine_body_and_logical_fuel_sites() {
    let checked = checked(RANKED_COUNTDOWN_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::countdown")
        .expect("lower ranked Terminal Psi");
    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode ranked semantics");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode ranked proof");
    let admitted =
        omega_psi_to_abstract_operations::lower_artifact_sections_for_native_realization(
            &semantic,
            &proof,
            &psi_proof_admission::AdmissionProfile::default(),
        )
        .expect("dispatch ranked native custody");
    let omega_psi_to_abstract_operations::NativeArtifactOperationPlan::RankedU32Countdown(ranked) =
        admitted
    else {
        panic!("ranked module must not enter ordinary native lowering")
    };

    for target in [
        omega_target::NativeTarget::linux_x64(),
        omega_target::NativeTarget::linux_arm64(),
    ] {
        let target_plan =
            omega_abstract_operations_to_target_operations::lower_ranked_to_target_operations(
                &ranked, target,
            )
            .expect("lower ranked target operations");
        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                .expect("assign ranked register");
        let emitted = omega_machine_emission::emit_machine_code(&assigned)
            .expect("emit ranked countdown machine code");
        let function = &emitted.functions[0];
        let record = function
            .ranked_u32_countdown
            .as_ref()
            .expect("retain ranked machine custody");
        assert_eq!(record.custody, ranked.countdown);
        assert_eq!(
            function.bytes,
            match target.architecture {
                omega_target::Architecture::X86_64 => {
                    omega_isa_x86_64::encode_ranked_u32_countdown_in_edi().to_vec()
                }
                omega_target::Architecture::Aarch64 => vec![
                    0x01, 0x00, 0x00, 0x14, 0x1f, 0x00, 0x00, 0x71, 0x60, 0x00, 0x00, 0x54, 0x00,
                    0x04, 0x00, 0x51, 0xfd, 0xff, 0xff, 0x17, 0xc0, 0x03, 0x5f, 0xd6,
                ],
            }
        );
        let graph = record.custody.graph;
        let covered = &record.custody.ranked_scc.covered_cyclic_edges[0];
        let psi_terminal::TerminalRankedGuard::UnsignedParameterPositive {
            edge: guard_edge, ..
        } = covered.guard;
        assert_eq!(
            function
                .fuel_attribution
                .iter()
                .map(|row| row.site)
                .collect::<Vec<_>>(),
            vec![
                omega_machine_code::NativeFuelSite::Edge(graph.preheader_edge),
                omega_machine_code::NativeFuelSite::Operation(graph.zero_operation),
                omega_machine_code::NativeFuelSite::Operation(graph.compare_operation),
                omega_machine_code::NativeFuelSite::Edge(guard_edge),
                omega_machine_code::NativeFuelSite::Operation(graph.one_operation),
                omega_machine_code::NativeFuelSite::Operation(graph.subtract_operation),
                omega_machine_code::NativeFuelSite::Edge(covered.edge),
                omega_machine_code::NativeFuelSite::Edge(graph.false_exit_edge),
                omega_machine_code::NativeFuelSite::Edge(graph.return_edge),
            ]
        );
        assert!(function.fuel_attribution.iter().all(|row| row.units == 1));
        assert_eq!(
            record.custody.fixed_fuel.ceiling_units(),
            5 + 6 * u64::from(u32::MAX)
        );

        let object = omega_image_emission::build_object_artifact(&emitted)
            .expect("independently replay ranked machine code into object custody");
        let again = omega_image_emission::build_object_artifact(&emitted)
            .expect("ranked object replay is deterministic");
        assert_eq!(object, again);
        assert_eq!(object.functions().len(), 1);
        assert_eq!(object.functions()[0].bytes(&object), function.bytes);
        assert_eq!(
            object.functions()[0]
                .ranked_u32_countdown
                .as_ref()
                .expect("object retains ranked custody"),
            record
        );
        assert_eq!(object.relocations().record_count(), 0);
        assert_eq!(object.fuel_attribution().len(), 9);
        assert!(
            object
                .fuel_attribution()
                .iter()
                .zip(&function.fuel_attribution)
                .all(|(object, machine)| {
                    object.machine == function.machine
                        && object.attribution == *machine
                        && object.text_offset == machine.code_offset
                })
        );
        let container = omega_image_emission::emit_object_container(&object);
        assert_eq!(container.output.text_bytes, function.bytes.len());
        assert_eq!(container.output.relocations, 0);
        let image_error = omega_image_emission::emit_executable_image(&object, 0)
            .expect_err("ranked final-image replay remains a later boundary");
        assert!(image_error.message.contains("no final-image replay"));

        let assert_invalid = |candidate: &omega_machine_code::MachineCodePlan| {
            assert!(matches!(
                omega_image_emission::build_object_artifact(candidate),
                Err(omega_image_emission::ObjectError::InvalidRankedCountdown(machine))
                    if machine == function.machine
            ));
        };

        let mut corrupted = emitted.clone();
        corrupted.functions[0].bytes[0] ^= 1;
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0].provenance.operations.swap(0, 1);
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0].fuel_attribution[0].units = 2;
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0].fuel_attribution[6].code_offset += 1;
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0]
            .ranked_u32_countdown
            .as_mut()
            .unwrap()
            .call_plan
            .parameters[0]
            .locations
            .clear();
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0]
            .ranked_u32_countdown
            .as_mut()
            .unwrap()
            .cleanup_actions
            .clear();
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0]
            .ranked_u32_countdown
            .as_mut()
            .unwrap()
            .custody
            .graph
            .compare_operation = psi_core::OperationId::new(99).unwrap();
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0]
            .ranked_u32_countdown
            .as_mut()
            .unwrap()
            .custody
            .structural_frontiers
            .machine = psi_core::MachineId::new(2).unwrap();
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0].attachment = None;
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        let structural = &record.structural_parameters[0];
        corrupted.functions[0]
            .unit_parameters
            .push(omega_machine_code::UnitParameterRecord {
                place: structural.place,
                structural_type: structural.structural_type,
                multiplicity: structural.multiplicity,
                shape: structural.shape,
            });
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0].scalar_stack = Some(omega_machine_code::ScalarStackEvidence {
            mutations: Vec::new(),
            control_flow: omega_machine_code::ScalarControlFlowEvidence::Linear,
            stack_alignment: 16,
            cleanup_preservation: None,
        });
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.target = match target.architecture {
            omega_target::Architecture::X86_64 => omega_target::NativeTarget::windows_x64(),
            omega_target::Architecture::Aarch64 => omega_target::NativeTarget::macos_arm64(),
        };
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        let mut extra = corrupted.functions[0].clone();
        extra.machine = psi_core::MachineId::new(2).unwrap();
        extra.attachment = None;
        extra.provenance = omega_target_operations::TerminalPsiProvenance {
            operations: Vec::new(),
            edges: Vec::new(),
        };
        extra.ranked_u32_countdown = None;
        extra.fuel_attribution.clear();
        extra.bytes = match target.architecture {
            omega_target::Architecture::X86_64 => vec![0xc3],
            omega_target::Architecture::Aarch64 => 0xd65f03c0_u32.to_le_bytes().to_vec(),
        };
        corrupted.functions.push(extra);
        assert_invalid(&corrupted);

        let mut stripped = emitted.clone();
        stripped.functions[0].ranked_u32_countdown = None;
        assert!(matches!(
            omega_image_emission::build_object_artifact(&stripped),
            Err(omega_image_emission::ObjectError::MissingRankedCountdownCustody(machine))
                if machine == function.machine
        ));
    }
}

fn hosted_custody() -> (
    psi_terminal_codec::CanonicalTerminalArtifact,
    CheckedProgramEntryTerminalReceipt,
    omega_program_entry_plan::SelectedProgramEntrySourceSignature,
) {
    let checked = checked(
        r#"
            data Main {}
            machine Main::launch() {}
        "#,
    );
    let selection = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|machine| machine.name == "Main::launch")
        .expect("terminal selection");
    let source =
        omega_program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            omega_target::TargetProfile::WindowsX64.program_entry_slot(),
            selection.machine,
            selection.machine,
            selection.name.clone(),
            "entry".into(),
            "test::Main::launch() -> Unit".into(),
            omega_program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
            Vec::new(),
        )
        .expect("hosted source signature");
    let produced = psi_checked_trees_to_terminal::produce_program_entry_terminal_artifact(
        &checked,
        "Main::launch",
        source.identity().bytes(),
    )
    .expect("ProgramEntry Terminal artifact");
    let (artifact, receipt) = produced.into_parts();
    (artifact, receipt, source)
}

#[test]
fn ordinary_and_explicit_optimizer_lowering_share_the_verified_entry() {
    let (artifact, _, _) = hosted_custody();
    let ordinary = omega_psi_to_abstract_operations::lower_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("ordinary native lowering produces a bare abstract plan");
    let explicit = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("an explicit optimizer request retains verified context");

    assert_eq!(ordinary.entry, explicit.plan().entry);
    assert_eq!(explicit.context().module().entry, explicit.plan().entry);
}

fn checked_adapter_plan(
    name: &str,
    provider: &str,
    requirement_owner: &str,
    requirement: &str,
    machine: &str,
) -> ProviderPlan {
    ProviderPlan {
        name: name.into(),
        provider_type: provider.into(),
        provider_type_package_identity: None,
        target: "uefi_x64".into(),
        schema: ServiceSchema {
            trait_name: requirement_owner.into(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                name: "enter".into(),
                requirement_owner: requirement_owner.into(),
                requirement_owner_package_identity: None,
                requirement_identity: requirement.into(),
                parameter_count: 0,
                parameter_type_identities: Vec::new(),
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: vec![requirement_owner.into()],
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            }],
        },
        rows: vec![ProviderPlanRow {
            method: "enter".into(),
            requirement_identity: requirement.into(),
            binding: ProviderBinding::CheckedAdapter {
                machine_identity: machine.into(),
                machine_package_identity: None,
            },
        }],
        origin_package_identity: None,
        origin_package: "test".into(),
    }
}

#[test]
fn selected_checked_adapter_projection_is_exact_and_fail_closed() {
    let selected =
        omega_effects::SelectedProviderPlanFacts::from_selected_plans(vec![checked_adapter_plan(
            "program-storage",
            "ProgramStorageProvider",
            "ProgramStorageEntry",
            "ProgramStorageEntry::enter",
            "ProgramStorageProvider::enter(Extent, Extent) -> Unit",
        )])
        .expect("valid selected checked-adapter plan");
    assert_eq!(
        project_selected_provider_adapters_for_requirements(
            &selected,
            &BTreeSet::from(["ProgramStorageEntry::enter"]),
        )
        .unwrap(),
        vec![SelectedProviderAdapter {
            requirement_identity: "ProgramStorageEntry::enter".into(),
            provider_identity: "ProgramStorageProvider".into(),
            machine_identity: "ProgramStorageProvider::enter(Extent, Extent) -> Unit".into(),
        }]
    );

    let mut external = checked_adapter_plan(
        "external-program-storage",
        "ProgramStorageProvider",
        "ProgramStorageEntry",
        "ProgramStorageEntry::enter",
        "ProgramStorageProvider::enter(Extent, Extent) -> Unit",
    );
    external.rows[0].binding = ProviderBinding::CompilerIntrinsic {
        machine: "TargetProgramStorage::enter(Extent, Extent) -> Unit".into(),
    };
    let external = omega_effects::SelectedProviderPlanFacts::from_selected_plans(vec![external])
        .expect("valid non-checked selected provider plan");
    assert!(
        project_selected_provider_adapters_for_requirements(
            &external,
            &BTreeSet::from(["ProgramStorageEntry::enter"]),
        )
        .expect("non-checked provider selection is not an installation")
        .is_empty()
    );

    let duplicate = omega_effects::SelectedProviderPlanFacts::from_selected_plans(vec![
        checked_adapter_plan(
            "first",
            "FirstProvider",
            "FirstBoundary",
            "Shared::enter",
            "FirstProvider::enter() -> Unit",
        ),
        checked_adapter_plan(
            "second",
            "SecondProvider",
            "SecondBoundary",
            "Shared::enter",
            "SecondProvider::enter() -> Unit",
        ),
    ])
    .expect("the selected closure itself permits distinct slots");
    assert!(
        project_selected_provider_adapters_for_requirements(
            &duplicate,
            &BTreeSet::from(["Shared::enter"]),
        )
        .expect_err("one Terminal requirement cannot acquire two checked adapters")
        .contains("more than one checked adapter")
    );

    let unrelated_duplicates = omega_effects::SelectedProviderPlanFacts::from_selected_plans(vec![
        checked_adapter_plan(
            "program-storage",
            "ProgramStorageProvider",
            "ProgramStorageEntry",
            "ProgramStorageEntry::enter",
            "ProgramStorageProvider::enter(Extent, Extent) -> Unit",
        ),
        checked_adapter_plan(
            "first-unrelated",
            "FirstProvider",
            "FirstBoundary",
            "Unrelated::enter",
            "FirstProvider::enter() -> Unit",
        ),
        checked_adapter_plan(
            "second-unrelated",
            "SecondProvider",
            "SecondBoundary",
            "Unrelated::enter",
            "SecondProvider::enter() -> Unit",
        ),
    ])
    .expect("selected closure may contain unrelated boundary slots");
    assert_eq!(
        project_selected_provider_adapters_for_requirements(
            &unrelated_duplicates,
            &BTreeSet::from(["ProgramStorageEntry::enter"]),
        )
        .expect("projection ignores checked rows outside the Terminal closure")
        .len(),
        1
    );

    let package =
        psi_core::PackageKeyIdentity::from_digest([0x5a; 32]).expect("nonzero package identity");
    let mut drifted = checked_adapter_plan(
        "drifted",
        "Provider",
        "Boundary",
        "Boundary::enter",
        "Provider::enter() -> Unit",
    );
    let ProviderBinding::CheckedAdapter {
        machine_package_identity,
        ..
    } = &mut drifted.rows[0].binding
    else {
        unreachable!()
    };
    *machine_package_identity = Some(package);
    assert!(
        omega_effects::SelectedProviderPlanFacts::from_selected_plans(vec![drifted])
            .expect_err("sealed selected facts must reject checked-adapter package drift")
            .contains("package identity")
    );
}

#[test]
fn independently_settles_exact_hosted_source_and_entry() {
    let (artifact, receipt, source) = hosted_custody();
    let settlement = validate_native_program_entry_settlement(
        &artifact,
        &receipt,
        NativeProgramEntrySettlement::new(&source, None),
        omega_target::NativeTarget::windows_x64(),
    )
    .expect("independent ProgramEntry settlement");

    assert_eq!(settlement.source(), &source);
    assert_eq!(settlement.checked_entry(), &receipt);
    assert_eq!(
        settlement.target(),
        omega_target::NativeTarget::windows_x64()
    );
    assert!(settlement.semantic_boundary_entry_plan().is_none());
    assert!(settlement.storage_entry().is_none());
}

#[test]
fn rejects_source_signature_target_and_artifact_substitution() {
    let (artifact, receipt, source) = hosted_custody();
    let substituted =
        omega_program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            source.target_slot(),
            source.machine_symbol(),
            source.state_symbol(),
            source.machine_name().into(),
            source.state_name().into(),
            "test::substituted::launch() -> Unit".into(),
            omega_program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
            Vec::new(),
        )
        .expect("substituted source signature");
    assert!(matches!(
        validate_native_program_entry_settlement(
            &artifact,
            &receipt,
            NativeProgramEntrySettlement::new(&substituted, None),
            omega_target::NativeTarget::windows_x64(),
        ),
        Err(NativeProgramEntrySettlementError::SourceSignatureSubstitution)
    ));
    assert!(matches!(
        validate_native_program_entry_settlement(
            &artifact,
            &receipt,
            NativeProgramEntrySettlement::new(&source, None),
            omega_target::NativeTarget::linux_x64(),
        ),
        Err(NativeProgramEntrySettlementError::TargetDrift)
    ));

    let scalar = checked(
        r#"
            data Helper {}
            machine Helper::touch() {}
            data Token { value: u64; }
            machine Token::drop(&mut self) { Helper::touch(); }
            data Main {}
            machine Main::launch(token: Token) -> u64 { 7u64 }
        "#,
    );
    let substituted_artifact =
        psi_checked_trees_to_terminal::produce_terminal_artifact(&scalar, "Main::launch")
            .expect("different canonical artifact");
    assert!(matches!(
        validate_native_program_entry_settlement(
            &substituted_artifact,
            &receipt,
            NativeProgramEntrySettlement::new(&source, None),
            omega_target::NativeTarget::windows_x64(),
        ),
        Err(NativeProgramEntrySettlementError::TerminalPsiSubstitution)
    ));
}
