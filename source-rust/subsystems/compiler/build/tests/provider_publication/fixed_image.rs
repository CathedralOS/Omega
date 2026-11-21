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
