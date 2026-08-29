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
