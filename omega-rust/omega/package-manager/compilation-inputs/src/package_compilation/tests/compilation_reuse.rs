use super::{
    TempTree, accepted_console_binding, generated_bundle, generated_source, identity,
    three_package_generated_inputs,
};
use crate::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, BuildExecutionInputs,
    BuildSnapshotRequest, BuildSourceCaptureObligation, BuildSourceCaptureRequest,
    PackageCompilationInputs, SourceAssemblyInputs,
};
use std::path::Path;
use target::TargetProfile;

fn build_inputs(inputs: &PackageCompilationInputs) -> BuildExecutionInputs {
    BuildExecutionInputs::for_unsponsored(
        Some(inputs),
        Some(TargetProfile::MacosArm64),
        Some(&BuildSnapshotRequest::new(Vec::new())),
        Some(Path::new("publication")),
    )
}

#[test]
fn compilation_reuse_separates_entry_source_from_later_binding_authority() {
    let tree = TempTree::new();
    let inputs = three_package_generated_inputs(&tree);
    let source = |inputs: &PackageCompilationInputs| {
        SourceAssemblyInputs::new(Some(inputs), Some(TargetProfile::MacosArm64))
    };
    let accepted = inputs
        .clone()
        .with_accepted_semantic_bindings(vec![accepted_console_binding(identity(1), 7)])
        .unwrap();
    assert_eq!(source(&inputs), source(&accepted));
    assert_ne!(build_inputs(&inputs), build_inputs(&accepted));

    // Entry bindings seed ordinary source imports, unlike a later service
    // nomination. Every catalogued physical contract must partition assembly.
    for profile in TargetProfile::ALL {
        let Some(contract) = profile.program_entry_slot().physical_contract_package else {
            continue;
        };
        let role = crate::program_entry_semantic_binding_role(contract);
        let entry = AcceptedSemanticBinding::new_service(
            role,
            identity(1),
            "ProgramEntry",
            effects::provider_plan::ServiceSchemaDigest::from_digest([9; 32]),
        )
        .unwrap();
        let selected = inputs
            .clone()
            .with_accepted_semantic_bindings(vec![entry])
            .unwrap();
        assert_ne!(source(&inputs), source(&selected), "{role:?}");
    }
    assert_ne!(
        source(&inputs),
        SourceAssemblyInputs::new(Some(&inputs), Some(TargetProfile::WindowsX64)),
    );
    assert_ne!(
        source(&inputs),
        SourceAssemblyInputs::new(
            Some(
                &inputs
                    .clone()
                    .with_compilation_purpose(build_declarations::DependencyPurpose::Build)
                    .unwrap()
            ),
            Some(TargetProfile::MacosArm64),
        ),
    );
    // Keep the non-entry control explicit when the role catalog expands.
    assert!(
        accepted
            .accepted_semantic_binding(AcceptedSemanticBindingRole::ConsoleExitProcessI32)
            .is_some()
    );
}

#[test]
fn compilation_reuse_compares_generated_content_without_erasing_child_custody() {
    let tree = TempTree::new();
    let inputs = three_package_generated_inputs(&tree);
    let generated = |target, marker, bytes: &[u8]| {
        inputs
            .clone()
            .with_complete_dependency_generated_sources(vec![
                generated_bundle(&inputs, identity(2), target, marker, Vec::new()),
                generated_bundle(
                    &inputs,
                    identity(3),
                    target,
                    marker,
                    vec![generated_source(b"answer.omg", bytes)],
                ),
            ])
            .unwrap()
    };
    let linux = generated(TargetProfile::LinuxX64, 3, b"pub const ANSWER: u32 = 7;");
    let windows = generated(TargetProfile::WindowsX64, 4, b"pub const ANSWER: u32 = 7;");
    let changed = generated(TargetProfile::WindowsX64, 4, b"pub const ANSWER: u32 = 8;");
    let source = |inputs: &PackageCompilationInputs| SourceAssemblyInputs::new(Some(inputs), None);
    assert_eq!(source(&linux), source(&windows));
    assert_eq!(build_inputs(&linux), build_inputs(&windows));
    assert_ne!(source(&windows), source(&changed));
    assert_ne!(build_inputs(&windows), build_inputs(&changed));
    assert!(
        linux
            .validate_dependency_generated_source_target(Some(TargetProfile::LinuxX64))
            .is_ok()
    );
    assert!(
        linux
            .validate_dependency_generated_source_target(Some(TargetProfile::WindowsX64))
            .is_err()
    );
}

#[test]
fn compilation_reuse_keeps_capture_output_obligations_and_live_directories_exact() {
    let empty = BuildSnapshotRequest::new(Vec::new());
    let required = BuildSnapshotRequest::new([b"artifact".to_vec()]);
    let scoped = BuildSnapshotRequest::scoped(
        Vec::new(),
        BuildSourceCaptureRequest::new([(
            b"main.omg".to_vec(),
            BuildSourceCaptureObligation::Required,
        )])
        .unwrap(),
    );
    let key = |snapshot: Option<&BuildSnapshotRequest>, directory: &str| {
        BuildExecutionInputs::for_unsponsored(
            None,
            Some(TargetProfile::MacosArm64),
            snapshot,
            Some(Path::new(directory)),
        )
    };
    assert_eq!(key(Some(&empty), "first"), key(Some(&empty), "second"));
    assert_ne!(key(Some(&empty), "first"), key(Some(&required), "second"));
    assert_ne!(key(Some(&empty), "first"), key(Some(&scoped), "second"));
    assert_ne!(key(None, "first"), key(None, "second"));
    assert_ne!(key(None, "first"), key(Some(&empty), "first"));
}
