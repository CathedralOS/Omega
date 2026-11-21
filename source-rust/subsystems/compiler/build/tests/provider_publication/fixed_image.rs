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
        r#"use omega\language\core\ProgramImage;
use omega\language\core\ProgramImageNativeProvider;
machine build(builder: @mut Build) {
    Build::package(builder, "firmware_provider");
    Build::select_provider::<ProgramImage, ProgramImageNativeProvider>(builder, uefi_x86_64);
}"#,
    )
    .unwrap();
    std::fs::write(
        temporary.path().join("main.omg"),
        r"use omega\language\core\ProgramImage;
public machine image_tail() -> addr reaches ProgramImage crashes none {
    ProgramImage::retained_suffix_start()
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
        module, &logical, inputs,
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

#[test]
fn scalar_firmware_observation_preserves_the_entrys_existing_storage_grants_on_replay() {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "firmware-entry-grants",
        false,
    )
    .unwrap();
    let path = temporary.path().join("build.omg");
    std::fs::write(
        &path,
        r#"use omega\language\core\ProgramImage;
use omega\language\core\ProgramImageNativeProvider;
machine build(builder: @mut Build) {
    Build::application(builder, "firmware");
    Build::select_provider::<ProgramImage, ProgramImageNativeProvider>(builder, uefi_x86_64);
    Build::bind(builder, uefi_x86_64\ProgramEntry, entry);
}"#,
    )
    .unwrap();
    std::fs::write(temporary.path().join("main.omg"), r"use omega\language\core\Extent;
use omega\language\core\Extent\Granted;
use omega\language\core\ProgramImage;
machine entry(image: Extent in Granted, storage: Extent in Granted) reaches ProgramImage crashes none {
    let suffix: addr = ProgramImage::retained_suffix_start();
    transition { _ -> retain(image, storage) }
    machine retain(image: Extent in Granted, storage: Extent in Granted) {
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
    let reopened = CanonicalTerminalArtifact::from_bytes(&published.to_bytes()).unwrap();
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
fn saved_psi_retains_private_grants_only_across_a_closed_observation() {
    for (footprint, should_preserve) in [("none", true), ("ExternalState invokes ExternalState", false)] {
        let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
            "firmware-observation-footprint", false,
        ).unwrap();
        let path = temporary.path().join("build.omg");
        std::fs::write(&path, r#"machine build(b: @mut Build) {
            Build::application(b, "firmware");
            Build::bind(b, uefi_x86_64\ProgramEntry, entry);
        }"#).unwrap();
        let source = r"use omega\language\core\Extent;
use omega\language\core\Extent\Granted;
public boundary trait ExternalState { machine step() reaches none crashes none; }
public boundary trait Probe { machine sense() -> addr reaches FOOTPRINT crashes none; }
machine entry(image: Extent in Granted, storage: Extent in Granted)
reaches Probe + ExternalState invokes Probe invokes ExternalState crashes none {
    let observation: addr = Probe::sense();
    transition { _ -> retain(image, storage) }
    machine retain(image: Extent in Granted, storage: Extent in Granted) {
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
            reopened.unwrap();
        } else {
            let error = reopened.unwrap_err();
            assert!(error.to_string().contains("unsupplied routed prerequisites"), "{error}");
        }
    }
}
