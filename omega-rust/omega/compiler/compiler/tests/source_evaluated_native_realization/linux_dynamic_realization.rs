use super::{
    Fixture, admit_import, realize_linux_dynamic, realize_linux_dynamic_outcome,
    terminal_authority_permission_policy, terminal_authority_policy,
};
use compiler::{
    RetainedNativeRealizationRequest, SourceEvaluatedImportSettlement,
    realize_retained_native_artifact,
};
use native_realization as native;
use task_plans::SameStackContributionAdmissionReceiptId;

#[test]
fn import_bearing_linux_compiler_route_retains_non_installable_dynamic_candidate() {
    let fixture = Fixture::new_linux_named("linux-dynamic", false);
    let candidate = realize_linux_dynamic(fixture.compile_terminal(), 0x454c_4600_0001);
    let native::RequestedNativeArtifact::DynamicElf(candidate) = candidate else {
        panic!("normalized ELF imports must select dynamic native custody")
    };
    candidate
        .validate()
        .expect("dynamic native candidate independently replays");
    assert_eq!(candidate.target(), target::NativeTarget::linux_x64());
    assert_eq!(
        candidate.object().object().layout.normalized_imports.len(),
        1
    );
    assert_eq!(candidate.image().output().final_image_imports, 1);
    assert!(candidate.image().output().bytes.starts_with(b"\x7fELF"));

    let replay = native::DynamicElfNativeArtifact::from_replayed_parts(candidate.into_parts())
        .expect("exact dynamic native parts replay");
    let mut psi_substitution = replay.into_parts();

    let donor_fixture = Fixture::new_linux_named("linux-dynamic-marker", true);
    let donor = realize_linux_dynamic(donor_fixture.compile_terminal(), 0x454c_4600_0002);
    let native::RequestedNativeArtifact::DynamicElf(donor) = donor else {
        unreachable!("donor has one normalized ELF import")
    };
    let donor_parts = donor.into_parts();
    assert_ne!(
        psi_substitution.psi_artifact.manifest().identity(),
        donor_parts.psi_artifact.manifest().identity(),
    );
    psi_substitution.psi_artifact = donor_parts.psi_artifact;
    assert!(
        native::DynamicElfNativeArtifact::from_replayed_parts(psi_substitution).is_err(),
        "substituting only canonical Terminal PSI must fail closed",
    );

    let object_candidate = realize_linux_dynamic(fixture.compile_terminal(), 0x454c_4600_0003);
    let native::RequestedNativeArtifact::DynamicElf(object_candidate) = object_candidate else {
        unreachable!("fixture has one normalized ELF import")
    };
    let mut object_substitution = object_candidate.into_parts();
    object_substitution.object = donor_parts.object;
    assert!(
        native::DynamicElfNativeArtifact::from_replayed_parts(object_substitution).is_err(),
        "substituting the outer object while retaining the requested image must fail closed",
    );

    let rejected = fixture.compile_terminal();
    let rejected_subsystem = rejected
        .native_realization_proposal()
        .expect("native proposal")
        .subsystem();
    let admission = admit_import(
        &rejected,
        SameStackContributionAdmissionReceiptId::from_normalized_identity(0x454c_4600_0004)
            .unwrap(),
    );
    let policy = terminal_authority_policy(&rejected);
    let permission_policy = terminal_authority_permission_policy(&rejected);
    let (request, diagnostics) = realize_retained_native_artifact(
        rejected,
        RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: policy,
            accepted_package_terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy: Some(permission_policy),
            image_request: native::ExecutableImageEmissionRequest::direct(rejected_subsystem),
            imports: &[SourceEvaluatedImportSettlement::new(
                &admission.execution,
                &admission.same_stack,
            )],
        },
    )
    .expect_err("import-bearing ELF cannot enter direct image custody");
    assert!(matches!(
        request,
        native::ExecutableImageEmissionRequest::Direct { subsystem, .. }
            if subsystem == rejected_subsystem
    ));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires an exact normalized interpreter")
    }));
}

#[test]
fn external_boundary_requirement_via_leaf_reaches_selected_instruction_stage() {
    // A top-level `boundary requirement` satisfied by an external `via` leaf:
    // the direct call is not redirected — it lowers through the requirement's
    // own retained boundary seam, and the import-settlement join keys on the
    // requirement machine's normalized overload identity. The legalized
    // normalized foreign call is reached, then the selection stage stops the
    // artifact: `NormalizedForeignCall` has no construction emission yet, so
    // the demanded foreign call fails closed as a custody mismatch instead of
    // silently dropping the boundary.
    let fixture = Fixture::new_linux_boundary_requirement_named("external-requirement-via-dynamic");
    let diagnostics = realize_linux_dynamic_outcome(fixture.compile_terminal(), 0x454c_4600_0100)
        .expect_err("selection emits no normalized foreign call instruction yet")
        .1;
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("SourceCustodyMismatch")),
        "selection stage fails closed on the requirement seam foreign call: {diagnostics:#?}",
    );
}

#[test]
fn rejected_native_reentry_returns_the_exact_dynamic_interpreter() {
    let fixture = Fixture::new_linux_named("linux-reentry-recovery", false);
    let retained = fixture.compile_terminal();
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let interpreter = target::normalize_elf_interpreter_plan(
        b"/lib64/ld-linux-x86-64.so.2".to_vec(),
        target::TargetProfile::LinuxX64,
    )
    .expect("canonical Linux x86-64 interpreter");
    let expected_interpreter = interpreter.clone();
    let (image_request, diagnostics) = realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: policy,
            accepted_package_terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy: Some(permission_policy),
            image_request: native::ExecutableImageEmissionRequest::dynamic_elf(interpreter),
            imports: &[],
        },
    )
    .expect_err("a demanded import cannot omit its admitted custody");
    let native::ExecutableImageEmissionRequest::DynamicElf { interpreter } = image_request else {
        panic!("native rejection must retain dynamic interpreter custody");
    };
    assert_eq!(interpreter, expected_interpreter);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("has no supplied execution"));
}
