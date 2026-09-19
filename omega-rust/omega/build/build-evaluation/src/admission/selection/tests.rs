//! Build selection tests.

use super::{
    SelectedCompilerProgramEntry, select_compiler_program_entry, selected_program_entry_machine,
};
use crate::{BuildConfig, RootBinding};

fn config_with_root_bindings(bindings: &[(&str, &str)]) -> BuildConfig {
    BuildConfig {
        root_bindings: bindings
            .iter()
            .map(|(slot, implementation)| RootBinding {
                slot: (*slot).to_owned(),
                implementation: (*implementation).to_owned(),
                implementation_symbol: symbols::SymbolHandle::from_arena_index(1),
            })
            .collect(),
        ..BuildConfig::default()
    }
}

fn source_only_program_entry_settlement() -> SelectedCompilerProgramEntry {
    let source_signature =
        program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            target::TargetProfile::WindowsX64.program_entry_slot(),
            symbols::SymbolHandle::from_arena_index(1),
            symbols::SymbolHandle::from_arena_index(2),
            "Application::start".into(),
            "entry".into(),
            "Application::start::entry() -> Unit".into(),
            program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
            Vec::new(),
        )
        .expect("exact source-only ProgramEntry fixture");
    SelectedCompilerProgramEntry::new(source_signature, None)
}

fn provisioned_program_entry_settlement() -> SelectedCompilerProgramEntry {
    let source_signature =
        program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            target::TargetProfile::WindowsX64.program_entry_slot(),
            symbols::SymbolHandle::from_arena_index(1),
            symbols::SymbolHandle::from_arena_index(2),
            "Application::start".into(),
            "entry".into(),
            "Application::start::entry(&mut self) -> Unit".into(),
            program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                normalized_type_identity: "ref-mut(named(name(Application)))".into(),
            },
            Vec::new(),
        )
        .expect("exact provisioned ProgramEntry fixture");
    SelectedCompilerProgramEntry::new(source_signature, None)
}

fn fused_root_row(
    selected: &SelectedCompilerProgramEntry,
    field: &str,
) -> program_entry_plan::ProgramEntryFusedServiceEstablishment {
    program_entry_plan::ProgramEntryFusedServiceEstablishment::new(
        selected.source_signature().identity(),
        selected.source_signature().target_slot(),
        "ref-mut(named(name(Application)))".into(),
        "named(name(Application))".into(),
        field.into(),
        "named(name(Service<Console>))".into(),
        "named(name(Service<Console>))".into(),
        "Console".into(),
        effects::provider_plan::ServiceSchemaDigest::from_digest([1; 32]),
        effects::provider_plan::ProviderPlanDigest::from_digest([2; 32]),
    )
    .expect("exact Fused root fixture")
}

#[test]
fn compiler_program_entry_absence_needs_no_typed_or_calling_plan_facts() {
    let selected = select_compiler_program_entry(
        &typed_trees::TypedTrees::default(),
        &BuildConfig::default(),
        None,
        &[],
        None,
    )
    .expect("an absent ProgramEntry is a complete settlement");

    assert!(selected.is_none());
}

#[test]
fn compiler_program_entry_consuming_split_preserves_source_only_custody() {
    let selected = source_only_program_entry_settlement();
    let expected_source = selected.source_signature().clone();

    assert_eq!(selected.machine_name(), "Application::start");
    assert!(selected.calling_plans().is_none());
    let (source_signature, calling_plans, establishments) = selected.into_parts();

    assert_eq!(source_signature, expected_source);
    assert!(calling_plans.is_none());
    assert!(establishments.is_empty());
}

#[test]
fn compiler_program_entry_binds_exact_sorted_fused_root_establishments() {
    let mut selected = provisioned_program_entry_settlement();
    let second = fused_root_row(&selected, "#2");
    let first = fused_root_row(&selected, "#1");
    selected
        .bind_fused_service_establishments(vec![second, first.clone()])
        .expect("exact provisioned roots should bind");
    assert_eq!(
        selected
            .fused_service_establishments()
            .iter()
            .map(|row| row.field_identity())
            .collect::<Vec<_>>(),
        ["#1", "#2"],
    );

    assert!(
        selected
            .bind_fused_service_establishments(vec![first.clone(), first])
            .is_err(),
        "duplicate direct receiver fields must reject"
    );
    let mut free = source_only_program_entry_settlement();
    assert!(
        free.bind_fused_service_establishments(vec![fused_root_row(&selected, "#3")])
            .is_err(),
        "a free ProgramEntry cannot acquire receiver establishment"
    );
}

#[test]
fn compiler_program_entry_validates_source_before_calling_plans() {
    let config =
        config_with_root_bindings(&[("windows_x86_64::ProgramEntry", "MissingApplication::start")]);
    let result = select_compiler_program_entry(
        &typed_trees::TypedTrees::default(),
        &config,
        Some(target::TargetProfile::WindowsX64),
        &[],
        None,
    );
    let Err(diagnostics) = result else {
        panic!("missing source entry must reject before calling-plan selection")
    };

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].to_string(),
        "error: build root slot selected entry `MissingApplication::start` is not a declaration in the admitted program"
    );
}

#[test]
fn targetless_check_does_not_select_a_program_entry_from_retained_bindings() {
    let config =
        config_with_root_bindings(&[("windows_x86_64::ProgramEntry", "Application::start")]);

    assert_eq!(
        selected_program_entry_machine(&config, None)
            .expect("targetless checking is entry-agnostic"),
        None
    );
}

#[test]
fn selected_target_ignores_valid_foreign_program_entry_slot_after_its_own() {
    let config = config_with_root_bindings(&[
        ("windows_x86_64::ProgramEntry", "Application::start"),
        ("linux_x86_64::ProgramEntry", "Diagnostics::start"),
    ]);

    let selected = selected_program_entry_machine(&config, Some(target::TargetProfile::WindowsX64))
        .expect("known foreign target roots remain available to their own profiles")
        .expect("selected target has one exact root");

    assert_eq!(selected.machine_name, "Application::start");
    assert_eq!(selected.slot.owner, target::TargetProfile::WindowsX64);
}

#[test]
fn selected_target_ignores_valid_foreign_program_entry_slot_before_its_own() {
    let config = config_with_root_bindings(&[
        ("linux_x86_64::ProgramEntry", "Diagnostics::start"),
        ("windows_x86_64::ProgramEntry", "Application::start"),
    ]);

    let selected = selected_program_entry_machine(&config, Some(target::TargetProfile::WindowsX64))
        .expect("binding order cannot change target-scoped selection")
        .expect("selected target has one exact root");

    assert_eq!(selected.machine_name, "Application::start");
    assert_eq!(selected.slot.owner, target::TargetProfile::WindowsX64);
}

#[test]
fn selected_entry_retains_the_target_owned_slot_schema() {
    let config = config_with_root_bindings(&[("uefi_x86_64::ProgramEntry", "Application::start")]);

    let selected = selected_program_entry_machine(&config, Some(target::TargetProfile::UefiX64))
        .expect("typed root slot selection")
        .expect("one selected entry");

    assert_eq!(selected.machine_name, "Application::start");
    assert_eq!(selected.slot.owner, target::TargetProfile::UefiX64);
    assert_eq!(
        selected.slot.visible_parameters,
        target::ProgramEntryVisibleParameters::ImageAndInitialStorage
    );
}

#[test]
fn root_slot_owner_rejects_legacy_cli_aliases() {
    let config = config_with_root_bindings(&[("windows_x64::ProgramEntry", "Application::start")]);

    let diagnostics =
        selected_program_entry_machine(&config, Some(target::TargetProfile::WindowsX64))
            .expect_err("a noncanonical target owner must reject");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].to_string().contains(
        "root slot `windows_x64::ProgramEntry` belongs to unknown target profile `windows_x64`"
    ));
}

#[test]
fn root_selection_rejects_names_absent_from_the_target_catalog() {
    let config =
        config_with_root_bindings(&[("windows_x86_64::UndeclaredEntry", "Application::start")]);

    let diagnostics =
        selected_program_entry_machine(&config, Some(target::TargetProfile::WindowsX64))
            .expect_err("an undeclared target root cannot enter ProgramEntry lowering");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].to_string().contains(
        "target profile `windows_x86_64` declares no required root slot `windows_x86_64::UndeclaredEntry`"
    ));
}

#[test]
fn selected_target_requires_every_member_of_its_catalog() {
    let config = config_with_root_bindings(&[("linux_x86_64::ProgramEntry", "Diagnostics::start")]);

    let diagnostics =
        selected_program_entry_machine(&config, Some(target::TargetProfile::WindowsX64))
            .expect_err("a foreign target row cannot satisfy the selected catalog");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].to_string().contains(
        "selected target `windows_x86_64` has no bound required root slot `windows_x86_64::ProgramEntry`"
    ));
}

#[test]
fn selected_target_rejects_duplicate_catalog_members() {
    let config = config_with_root_bindings(&[
        ("windows_x86_64::ProgramEntry", "Application::start"),
        ("windows_x86_64::ProgramEntry", "Diagnostics::start"),
    ]);

    let diagnostics =
        selected_program_entry_machine(&config, Some(target::TargetProfile::WindowsX64))
            .expect_err("one required catalog member cannot be bound twice");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].to_string().contains(
        "selected target `windows_x86_64` has more than one bound required root slot `windows_x86_64::ProgramEntry`"
    ));
}
