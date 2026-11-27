//! Follow an authored UEFI provider choice through saved Psi into its native plan.

use native_intrinsics::CompilerIntrinsicExecutionIdentity;
use provider_realization::target_supply::{
    TargetSupplyExecutionInputs, TargetSupplyRealizationRequest, realize_target_supply,
};
use target::TargetProfile;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_semantics::logical_providers::ClosedLogicalProviders;

#[test]
fn fixed_image_supplies_only_its_authenticated_member_on_uefi() {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "fixed-image-uefi",
        false,
    )
    .unwrap();
    let path = temporary.path().join("build.omg");
    std::fs::write(
        &path,
        r#"use {omega}/language/core/program_image.omg;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Package { name: "firmware_provider" };
    Build::select_provider::<|program_image|::ProgramImage, |program_image|::ProgramImageNativeProvider>(builder, uefi_x86_64);
}"#,
    )
    .unwrap();
    std::fs::write(
        temporary.path().join("main.omg"),
        r"use {omega}/language/core/program_image.omg;
public machine image_tail() -> address reaches |program_image|::ProgramImage crashes none {
    |program_image|::ProgramImage::retained_suffix_start()
}",
    )
    .unwrap();
    let target = TargetProfile::UefiX64;
    let (returned, inputs) = super::run_project_build(&path, &[target]);
    let product = returned
        .settle_dependency(inputs.clone())
        .unwrap()
        .open_product(inputs)
        .unwrap();
    let stage = product.product_stage(&[target]).unwrap();
    let published = source_compiler::compile_for_target(stage.source(), target.target_name(), None)
        .unwrap()
        .into_parts()
        .0;
    let artifact = CanonicalTerminalArtifact::from_bytes(&published.to_bytes()).unwrap();
    let module = artifact.semantic_module();
    let [external] = module.interfaces.external_providers.as_slice() else {
        panic!("expected the selected fixed-image supplier")
    };
    let boundary = module
        .interfaces
        .boundary_machines
        .iter()
        .find(|boundary| boundary.identity == external.requirement_identity)
        .unwrap();
    let select = |module: &terminal_psi::TerminalModule, target: &str| {
        ClosedLogicalProviders::select(module, artifact.view().identity(), target, &[boundary.id])
    };
    let logical = select(module, target.target_name()).unwrap();
    let inputs = TargetSupplyExecutionInputs {
        components: None,
        calling_applications: vec![],
        external_bindings: vec![],
        evaluated_native_bindings: vec![],
        evaluated_binding_identities: Default::default(),
        terminal_authority_permissions: vec![],
        terminal_authority_permission_identities: Default::default(),
    };
    let realized = realize_target_supply(TargetSupplyRealizationRequest::from_execution_inputs(
        artifact.view(),
        &logical,
        inputs,
    ))
    .unwrap();
    let [plan] = realized.provider_declarations() else {
        panic!("expected one native provider plan")
    };
    assert_eq!(plan.target, target.target_name());
    assert_eq!(
        realized.row_executions().next().unwrap(),
        [Some(
            CompilerIntrinsicExecutionIdentity::ProgramImageRetainedSuffixStart
        )]
    );
    assert_eq!(
        plan.row_numeric_realizations[0].as_deref(),
        module
            .identities
            .text(external.realization.machine.identity())
    );

    let mut wrong_target = module.clone();
    wrong_target.interfaces.provider_selections[0].target =
        TargetProfile::LinuxX64.target_name().to_owned();
    assert!(
        select(&wrong_target, TargetProfile::LinuxX64.target_name())
            .unwrap_err()
            .diagnostic(&wrong_target)
            .contains("requires uefi_x86_64")
    );

    let mut invented_ensures = module.clone();
    invented_ensures
        .interfaces
        .boundary_machines
        .iter_mut()
        .find(|declaration| declaration.id == boundary.id)
        .unwrap()
        .scalar_ensures
        .push(semantic_vocabulary::Proposition::Falsehood);
    assert!(
        select(&invented_ensures, target.target_name())
            .unwrap_err()
            .diagnostic(&invented_ensures)
            .contains("differs from its checked Core member")
    );

    let mut impostor = module.clone();
    impostor.interfaces.external_providers[0]
        .realization
        .machine = external.requirement.clone();
    assert!(
        select(&impostor, target.target_name())
            .unwrap_err()
            .diagnostic(&impostor)
            .contains("differs from its checked Core member")
    );
}

fn published_firmware_application() -> CanonicalTerminalArtifact {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "firmware-entry-grants",
        false,
    )
    .unwrap();
    let path = temporary.path().join("build.omg");
    std::fs::write(
        &path,
        r#"use main.omg as app;
use {omega}/language/core/program_image.omg;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "firmware" };
    Build::select_provider::<|program_image|::ProgramImage, |program_image|::ProgramImageNativeProvider>(builder, uefi_x86_64);
    Build::bind(builder, uefi_x86_64::ProgramEntry, |app|::entry);
}"#,
    )
    .unwrap();
    std::fs::write(temporary.path().join("main.omg"), r"use {omega}/language/core/extent.omg;
use {omega}/language/core/program_image.omg;
machine entry(image: |extent|::Extent in |extent|::Extent::Granted, storage: |extent|::Extent in |extent|::Extent::Granted) reaches |program_image|::ProgramImage crashes none {
    let suffix: address = |program_image|::ProgramImage::retained_suffix_start();
    transition { _ -> retain(image, storage) }
    machine retain(image: |extent|::Extent in |extent|::Extent::Granted, storage: |extent|::Extent in |extent|::Extent::Granted) {
        transition { _ -> retain(image, storage) }
    }
}").unwrap();
    let target = TargetProfile::UefiX64;
    let project = super::open_project(&path, &[target]);
    let stage = project.product_stage(&[target]).unwrap();
    let published = source_compiler::compile_for_target(stage.source(), target.target_name(), None)
        .unwrap()
        .into_parts()
        .0;
    CanonicalTerminalArtifact::from_bytes(&published.to_bytes()).unwrap()
}

#[test]
fn scalar_firmware_observation_preserves_the_entrys_existing_storage_grants_on_replay() {
    let reopened = published_firmware_application();
    let target = TargetProfile::UefiX64;
    let module = reopened.semantic_module();
    let declared = module.program_entry_contract(target.target_name()).unwrap();
    let entry = module.machine(declared.entry).unwrap();
    let [image, storage] = entry.structural_parameters.as_slice() else {
        panic!("expected two installed parameters")
    };
    assert_eq!(image.qualifications.len(), 1);
    assert_eq!(image.qualifications, storage.qualifications);
    assert_ne!(image.place, storage.place);
}

#[test]
fn uefi_nominates_only_the_two_qualified_entry_occurrences() {
    use provider_realization::program_entry::SelectedCompilerProgramEntry;
    let reopened = published_firmware_application();
    let module = reopened.semantic_module();
    let profile = TargetProfile::UefiX64;
    let entry = module.program_entry_contract(profile.target_name()).unwrap().entry;
    let nominate = |module: &terminal_psi::TerminalModule| {
        SelectedCompilerProgramEntry::from_terminal(module, entry, profile.program_entry_slot())
    };
    let selected = nominate(module).unwrap();
    let parameters = selected.source_signature().visible_parameters();
    assert_eq!(parameters.len(), 2);
    for (index, parameter) in parameters.iter().enumerate() {
        assert_eq!(parameter.visible_parameter_index(), index);
        assert_eq!(parameter.value_shape().byte_size, 16);
        let [base, length] = parameter.extent_value_layout().fields();
        assert_eq!((base.byte_offset(), length.byte_offset()), (0, 8));
    }

    let mut missing = module.clone();
    missing.machines.iter_mut().find(|row| row.id == entry).unwrap()
        .structural_parameters[1].qualifications.clear();
    assert!(nominate(&missing).unwrap_err()[0].message.contains("whole Extent grant"));

    let mut shared = module.clone();
    let entry_body = shared.machines.iter_mut().find(|row| row.id == entry).unwrap();
    entry_body.entry_claims[1].claim = entry_body.entry_claims[0].claim;
    let errors = nominate(&shared).expect_err("each entry argument needs its own claim");
    assert!(
        errors[0].message.contains("a distinct whole claim for each argument"),
        "{errors:?}"
    );

    let mut changed_type = module.clone();
    let checked = changed_type.machines.iter_mut().find(|row| row.id == entry)
        .unwrap().contract.declaration.as_mut().unwrap();
    checked.parameter_types[0] = checked.result_type;
    assert!(nominate(&changed_type).unwrap_err()[0].message.contains("declared types differ"));
}

#[test]
fn saved_psi_retains_private_grants_only_across_a_closed_observation() {
    for (footprint, should_preserve) in [("none", true), ("ExternalState invokes ExternalState", false)] {
        let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
            "firmware-observation-footprint", false,
        ).unwrap();
        let path = temporary.path().join("build.omg");
        std::fs::write(&path, r#"use main.omg as app;
machine build(b: @mutable BuildPlan) {
            b.product = BuildProduct::Application { name: "firmware" };
            Build::bind(b, uefi_x86_64::ProgramEntry, |app|::entry);
        }"#).unwrap();
        let source = r"use {omega}/language/core/extent.omg;
public boundary trait ExternalState { machine step() reaches none crashes none; }
public boundary trait Probe { machine sense() -> address reaches FOOTPRINT crashes none; }
machine entry(image: |extent|::Extent in |extent|::Extent::Granted, storage: |extent|::Extent in |extent|::Extent::Granted)
reaches Probe + ExternalState invokes Probe invokes ExternalState crashes none {
    let observation: address = Probe::sense();
    transition { _ -> retain(image, storage) }
    machine retain(image: |extent|::Extent in |extent|::Extent::Granted, storage: |extent|::Extent in |extent|::Extent::Granted) {
        transition { _ -> retain(image, storage) }
    }
}";
        std::fs::write(temporary.path().join("main.omg"), source.replace("FOOTPRINT", footprint)).unwrap();
        let target = TargetProfile::UefiX64;
        let project = super::open_project(&path, &[target]);
        let stage = project.product_stage(&[target]).unwrap();
        let issued = source_compiler::compile_for_target(stage.source(), target.target_name(), None).unwrap().into_parts().0;
        let reopened = CanonicalTerminalArtifact::from_bytes(&issued.to_bytes());
        if should_preserve {
            assert!(reopened.is_ok(), "{reopened:?}");
        } else {
            let error = reopened.unwrap_err();
            assert!(error.to_string().contains("unsupplied routed prerequisites"), "{error}");
        }
    }
}

#[test]
fn storage_transport_requires_a_grant_on_each_owned_argument() {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "firmware-storage-transport",
        false,
    )
    .unwrap();
    let path = temporary.path().join("build.omg");
    std::fs::write(&path, r#"use {omega}/language/core/extent.omg;
machine build(b: @mutable BuildPlan) {
    b.product = BuildProduct::Package { name: "transport" };
    Build::select_provider::<|extent|::ProgramStorageEntry, |extent|::ProgramStorageEntryNativeProvider>(b, uefi_x86_64);
}"#).unwrap();
    std::fs::write(
        temporary.path().join("main.omg"),
        r"use {omega}/language/core/extent.omg;
public machine forward(first: |extent|::Extent in |extent|::Extent::Granted, second: |extent|::Extent in |extent|::Extent::Granted)
reaches |extent|::ProgramStorageEntry invokes |extent|::ProgramStorageEntry crashes Trap crashes Abort {
    |extent|::ProgramStorageEntry::enter(first, second);
}",
    )
    .unwrap();
    let target = TargetProfile::UefiX64;
    let (returned, inputs) = super::run_project_build(&path, &[target]);
    let product = returned
        .settle_dependency(inputs.clone())
        .unwrap()
        .open_product(inputs)
        .unwrap();
    let stage = product.product_stage(&[target]).unwrap();
    let published = source_compiler::compile_for_target(stage.source(), target.target_name(), None)
        .unwrap()
        .into_parts()
        .0;
    let artifact = CanonicalTerminalArtifact::from_bytes(&published.to_bytes()).unwrap();
    let module = artifact.semantic_module();
    let [supply] = module.interfaces.external_providers.as_slice() else {
        panic!("selected external transport")
    };
    let boundary = module
        .interfaces
        .boundary_machines
        .iter()
        .find(|row| row.identity == supply.requirement_identity)
        .unwrap();
    let [image, storage] = boundary.requires.as_slice() else {
        panic!("two grant obligations")
    };
    assert_eq!((image.argument_index, storage.argument_index), (0, 1));
    assert!(image.path.is_empty() && storage.path.is_empty());
    assert_eq!(image.domain, storage.domain);
    assert!(boundary.program_local_root_introductions.is_empty());
    let select = |module: &terminal_psi::TerminalModule| {
        ClosedLogicalProviders::select(
            module,
            artifact.view().identity(),
            target.target_name(),
            &[boundary.id],
        )
    };
    let logical = select(module).unwrap();
    let realized = realize_target_supply(TargetSupplyRealizationRequest::from_execution_inputs(
        artifact.view(),
        &logical,
        TargetSupplyExecutionInputs {
            components: None,
            calling_applications: vec![],
            external_bindings: vec![],
            evaluated_native_bindings: vec![],
            evaluated_binding_identities: Default::default(),
            terminal_authority_permissions: vec![],
            terminal_authority_permission_identities: Default::default(),
        },
    ))
    .unwrap();
    assert_eq!(
        realized.row_executions().next().unwrap(),
        [Some(
            CompilerIntrinsicExecutionIdentity::ProgramStorageEntry
        )]
    );
    for substitute in 0..3 {
        let mut changed = module.clone();
        let required = changed
            .interfaces
            .boundary_machines
            .iter_mut()
            .find(|row| row.id == boundary.id)
            .unwrap();
        match substitute {
            0 => { required.requires.pop(); }
            1 => required.requires[1].argument_index = 0,
            _ => required.scalar_requires[1] = required.scalar_requires[0].clone(),
        }
        assert!(
            select(&changed)
                .unwrap_err()
                .diagnostic(&changed)
                .contains("differs from its checked Core member")
        );
    }
}
