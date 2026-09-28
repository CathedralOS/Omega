//! The hosted receiver bridge end-to-end: `bind_hosted_receiver` joins the
//! admitted source signature, exact physical contract, Fused service roster,
//! and derived stack demand onto a fragment-custody object, and every emitted
//! image independently replays the bridge bytes, partitions, relocations, and
//! entry selection through `validate_image`.
//!
//! This route needs `fragment_replay` custody (`receiver_layout` reads the
//! current entry's abstract and target graphs through `entry_source`), which
//! only the `build_function_fragment_object_artifact` lane retains; until the
//! shared container fixture existed here it was exercised only by
//! native-realization's own tests.

use super::fragment_container::compile_attached_entry;
use super::hosted_exit_runtime::assert_exit;
use image_emission::{
    bind_hosted_receiver, derive_stack_demand, emit_direct_executable_image,
    validate_direct_executable_image,
};
use target::TargetProfile;

/// A provisioned `&mut self` receiver with one zero-valid scalar field. The
/// stored write keeps the entry nonempty; `Main` carries no erased fields, so
/// the Fused establishment roster is empty by construction.
const RECEIVER_STORE: &str = r#"
    data Main { value: i32; }
    machine Main::launch(&mut self) {
        self.value = 17;
    }
"#;

/// The exact admitted physical contract for each bridge target, mirroring the
/// sealed package plans the production settlement carries.
fn physical_contract(
    profile: TargetProfile,
) -> program_entry_plan::ProgramEntryPhysicalContractPlan {
    let (slot, package, digest, requirement, plan, parameters, result) = match profile {
        TargetProfile::MacosArm64 => (
            TargetProfile::MacosArm64.program_entry_slot(),
            target::ProgramEntryPhysicalContractPackage::MacosArm64,
            program_entry_plan::exact_macos_arm64_physical_contract_package_source_digest(),
            program_entry_plan::MACOS_ARM64_PHYSICAL_REQUIREMENT_IDENTITY,
            program_entry_plan::exact_macos_arm64_physical_boundary_entry_plan(),
            vec![
                program_entry_plan::MACOS_ARM64_I32_TYPE_IDENTITY,
                program_entry_plan::MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
                program_entry_plan::MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
                program_entry_plan::MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
            ],
            program_entry_plan::MACOS_ARM64_I32_TYPE_IDENTITY,
        ),
        TargetProfile::LinuxX64 => (
            TargetProfile::LinuxX64.program_entry_slot(),
            target::ProgramEntryPhysicalContractPackage::LinuxX86_64,
            program_entry_plan::exact_linux_x86_64_physical_contract_package_source_digest(),
            program_entry_plan::LINUX_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
            program_entry_plan::exact_linux_x86_64_physical_boundary_entry_plan(),
            vec![program_entry_plan::LINUX_X86_64_ADDRESS_TYPE_IDENTITY],
            program_entry_plan::LINUX_X86_64_I32_TYPE_IDENTITY,
        ),
        TargetProfile::LinuxArm64 => (
            TargetProfile::LinuxArm64.program_entry_slot(),
            target::ProgramEntryPhysicalContractPackage::LinuxArm64,
            program_entry_plan::exact_linux_arm64_physical_contract_package_source_digest(),
            program_entry_plan::LINUX_ARM64_PHYSICAL_REQUIREMENT_IDENTITY,
            program_entry_plan::exact_linux_arm64_physical_boundary_entry_plan(),
            vec![program_entry_plan::LINUX_ARM64_U64_TYPE_IDENTITY],
            program_entry_plan::LINUX_ARM64_I32_TYPE_IDENTITY,
        ),
        _ => panic!("no hosted receiver bridge admits {profile:?}"),
    };
    program_entry_plan::ProgramEntryPhysicalContractPlan::new(
        slot,
        requirement.into(),
        package,
        digest,
        // The package-source report fingerprint is retained for diagnostics
        // only; contract custody replays the strong source digest instead.
        0,
        parameters.into_iter().map(str::to_owned).collect(),
        result.into(),
        plan.contract_report_fingerprint(),
        plan.plan().clone(),
    )
    .expect("exact admitted physical contract")
}

/// Bind the exact bridge the admitted settlement selects: same source
/// signature, same target-slot physical contract, empty Fused roster, and the
/// derived entry stack demand.
fn bind_receiver(
    artifact: &mut image_emission::ObjectArtifact,
    signature: &program_entry_plan::SelectedProgramEntrySourceSignature,
    profile: TargetProfile,
    cleanup_occupancy: bool,
) {
    let demand = derive_stack_demand(artifact, artifact.entry()).expect("entry stack demand");
    bind_hosted_receiver(
        artifact,
        signature,
        &physical_contract(profile),
        &[],
        &demand,
        cleanup_occupancy,
    )
    .expect("the exact admitted bridge binds the emitted object");
}

#[test]
fn hosted_receiver_binds_and_replays_through_emission_on_all_bridge_targets() {
    for profile in [
        TargetProfile::LinuxX64,
        TargetProfile::LinuxArm64,
        TargetProfile::MacosArm64,
    ] {
        let compiled = compile_attached_entry(RECEIVER_STORE, "Main::launch", profile);
        let mut artifact = compiled.artifact;
        assert!(
            std::ptr::eq(
                artifact
                    .fragment_source_for_test()
                    .expect("fragment custody is retained"),
                compiled.container.as_ref(),
            ),
            "{profile:?} the object retains the exact staged container custody",
        );
        let unbound = emit_direct_executable_image(&artifact, 3).expect("unbound image");
        bind_receiver(&mut artifact, &compiled.signature, profile, false);
        let binding = artifact
            .hosted_receiver_binding()
            .expect("the binding is retained on the sealed object");
        assert_eq!(binding.source(), &compiled.signature);
        assert_eq!(
            binding.physical_contract().target_slot(),
            profile.program_entry_slot(),
        );

        let image = emit_direct_executable_image(&artifact, 3).expect("receiver image emits");
        assert_ne!(
            image.output().final_text_bytes,
            unbound.output().final_text_bytes,
            "{profile:?} appends its exact bridge text",
        );
        validate_direct_executable_image(&artifact, &image)
            .expect("the emitted bridge replays its exact custody");

        // The image-side join is re-derived, not trusted: a stale image
        // emitted before binding cannot stand in for the bound object.
        assert!(
            validate_direct_executable_image(&artifact, &unbound).is_err(),
            "{profile:?} an unbound-custody image cannot replay a bound object",
        );

        if profile == TargetProfile::LinuxX64
            && cfg!(all(target_os = "linux", target_arch = "x86_64"))
        {
            // The Linux x86-64 bridge is a real process entry: kernel arrival,
            // private stack switch, receiver in rdi, exit_group status zero.
            assert_exit(&image.output().bytes, 0);
        }
    }
}

#[test]
fn hosted_receiver_binding_fails_closed_before_bytes_exist() {
    let profile = TargetProfile::LinuxX64;
    let compiled = compile_attached_entry(RECEIVER_STORE, "Main::launch", profile);
    let signature = compiled.signature;
    let physical = physical_contract(profile);

    // A binding is unique: a second bind on an already-bound object rejects.
    let mut artifact = compiled.artifact;
    bind_receiver(&mut artifact, &signature, profile, false);
    let demand = derive_stack_demand(&artifact, artifact.entry()).expect("demand");
    assert!(
        bind_hosted_receiver(&mut artifact, &signature, &physical, &[], &demand, false).is_err(),
        "a second hosted receiver binding must reject",
    );

    // The physical contract is exact per target: a Darwin contract cannot
    // satisfy the Linux x86-64 bridge.
    let mut unbound = compile_attached_entry(RECEIVER_STORE, "Main::launch", profile).artifact;
    assert!(
        bind_hosted_receiver(
            &mut unbound,
            &signature,
            &physical_contract(TargetProfile::MacosArm64),
            &[],
            &demand,
            false,
        )
        .is_err(),
        "a substituted physical contract rejects before binding",
    );

    // The source signature is bound to the same target slot: a signature
    // minted for Darwin cannot join the Linux bridge even with the Linux
    // contract.
    let darwin_signature = {
        let darwin =
            compile_attached_entry(RECEIVER_STORE, "Main::launch", TargetProfile::MacosArm64);
        darwin.signature
    };
    let mut unbound = compile_attached_entry(RECEIVER_STORE, "Main::launch", profile).artifact;
    assert!(
        bind_hosted_receiver(
            &mut unbound,
            &darwin_signature,
            &physical,
            &[],
            &demand,
            false
        )
        .is_err(),
        "a target-slot-substituted source signature rejects",
    );

    // A receiver-free declaration cannot stand in for the provisioned
    // receiver the emitted object was compiled for.
    let free_signature =
        program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            signature.target_slot(),
            signature.machine_symbol(),
            signature.state_symbol(),
            signature.machine_name().into(),
            signature.state_name().into(),
            signature.normalized_callable_identity().into(),
            program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
            signature.visible_parameters().to_vec(),
        )
        .expect("free declaration is valid alone");
    let mut unbound = compile_attached_entry(RECEIVER_STORE, "Main::launch", profile).artifact;
    assert!(
        bind_hosted_receiver(
            &mut unbound,
            &free_signature,
            &physical,
            &[],
            &demand,
            false
        )
        .is_err(),
        "a receiver source substitution rejects",
    );

    // Without retained fragment custody there is no current entry graph to
    // join: the binder fails closed before any bridge byte exists.
    let mut unbound = compile_attached_entry(RECEIVER_STORE, "Main::launch", profile).artifact;
    unbound.clear_fragment_replay_for_test();
    assert!(
        bind_hosted_receiver(&mut unbound, &signature, &physical, &[], &demand, false).is_err(),
        "an object without fragment replay custody cannot bind a receiver",
    );
}

#[test]
fn hosted_receiver_emission_replays_binding_custody_exactly() {
    for (profile, mutate) in [
        (TargetProfile::LinuxX64, "source"),
        (TargetProfile::LinuxArm64, "demand"),
        (TargetProfile::MacosArm64, "byte_count"),
    ] {
        let compiled = compile_attached_entry(RECEIVER_STORE, "Main::launch", profile);
        let mut artifact = compiled.artifact;
        bind_receiver(&mut artifact, &compiled.signature, profile, false);
        match mutate {
            // A substituted receiver source after binding is a custody
            // replacement, not an alias.
            "source" => {
                *artifact
                    .hosted_receiver_source_mut_for_test()
                    .expect("bound source") =
                    program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
                        compiled.signature.target_slot(),
                        compiled.signature.machine_symbol(),
                        compiled.signature.state_symbol(),
                        compiled.signature.machine_name().into(),
                        compiled.signature.state_name().into(),
                        compiled.signature.normalized_callable_identity().into(),
                        program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
                        compiled.signature.visible_parameters().to_vec(),
                    )
                    .expect("free declaration is valid alone");
            }
            // A stack-ceiling substitution disagrees with the demand the
            // object itself derives.
            "demand" => {
                *artifact
                    .hosted_receiver_stack_ceiling_mut_for_test()
                    .expect("bound demand") += 16;
            }
            // A receiver-extent substitution disagrees with the layout the
            // current graph derives.
            "byte_count" => {
                *artifact
                    .hosted_receiver_byte_count_mut_for_test()
                    .expect("bound receiver extent") += 8;
            }
            _ => unreachable!(),
        }
        assert!(
            emit_direct_executable_image(&artifact, 3).is_err(),
            "{profile:?} a {mutate} substitution rejects before bridge bytes exist",
        );
    }
}

/// Emission replays the retained fragment custody itself: an object whose
/// current bytes or custody no longer match what the container staged rejects
/// before the bridge is even considered.
#[test]
fn hosted_receiver_emission_rejects_mutated_fragment_custody() {
    let profile = TargetProfile::LinuxX64;
    let compiled = compile_attached_entry(RECEIVER_STORE, "Main::launch", profile);

    // A byte substitution in the object's current text breaks the exact
    // staged-container replay the emission gate performs first.
    let mut artifact = compiled.artifact.clone();
    bind_receiver(&mut artifact, &compiled.signature, profile, false);
    artifact.text_bytes_mut_for_test()[0] ^= 0xff;
    assert!(
        emit_direct_executable_image(&artifact, 3).is_err(),
        "object text divergence from retained custody rejects emission",
    );

    // Dropping custody after binding removes the current entry graph the
    // receiver layout is derived from.
    let mut artifact = compiled.artifact;
    bind_receiver(&mut artifact, &compiled.signature, profile, false);
    artifact.clear_fragment_replay_for_test();
    assert!(
        emit_direct_executable_image(&artifact, 3).is_err(),
        "a bound object without retained custody rejects emission",
    );
}

#[test]
fn hosted_receiver_image_replay_rejects_mutated_bridge_bytes() {
    let profile = TargetProfile::LinuxX64;
    let compiled = compile_attached_entry(RECEIVER_STORE, "Main::launch", profile);
    let mut artifact = compiled.artifact;
    bind_receiver(&mut artifact, &compiled.signature, profile, false);
    let image = emit_direct_executable_image(&artifact, 3).expect("receiver image emits");
    validate_direct_executable_image(&artifact, &image).expect("exact image replays");

    // The bridge occupies the fixed 37-byte suffix of the emitted text:
    // `...; mov eax, 231; syscall; ud2`. Corrupting the syscall opcode is a
    // byte substitution the independent reader decodes back out.
    let mut corrupted_syscall = image.clone();
    let len = corrupted_syscall.output().final_text_bytes.len();
    corrupted_syscall.output_mut_for_test().final_text_bytes[len - 2] = 0x06;
    assert!(
        validate_direct_executable_image(&artifact, &corrupted_syscall).is_err(),
        "a substituted bridge opcode rejects on replay",
    );

    // Corrupting the call rel32 displacement retargets the semantic
    // continuation the bridge invokes.
    let mut retargeted = image.clone();
    retargeted.output_mut_for_test().final_text_bytes[len - 15] ^= 0x01;
    assert!(
        validate_direct_executable_image(&artifact, &retargeted).is_err(),
        "a retargeted bridge call rejects on replay",
    );

    // The sealed image does not follow a different object's custody either.
    let other = compile_attached_entry(RECEIVER_STORE, "Main::launch", TargetProfile::LinuxArm64);
    let mut other_artifact = other.artifact;
    bind_receiver(
        &mut other_artifact,
        &other.signature,
        TargetProfile::LinuxArm64,
        false,
    );
    assert!(
        validate_direct_executable_image(&other_artifact, &image).is_err(),
        "an image cannot be rejoined to a substituted object",
    );
}

/// A receiver whose cleanup must occupy its hosted extent binds with the
/// occupancy tracked on the binding: the emitted partitions and bridge bytes
/// are unchanged, and the occupancy follows the sealed binding exactly.
#[test]
fn hosted_receiver_cleanup_occupancy_is_tracked_on_the_binding() {
    for profile in [
        TargetProfile::LinuxX64,
        TargetProfile::LinuxArm64,
        TargetProfile::MacosArm64,
    ] {
        let compiled = compile_attached_entry(RECEIVER_STORE, "Main::launch", profile);
        let mut artifact = compiled.artifact;
        bind_receiver(&mut artifact, &compiled.signature, profile, true);
        let binding = artifact
            .hosted_receiver_binding()
            .expect("the binding is retained on the sealed object");
        assert!(
            binding.cleanup_occupancy(),
            "{profile:?} the binding tracks the admitted cleanup occupancy",
        );
        assert_eq!(binding.source(), &compiled.signature);

        let image = emit_direct_executable_image(&artifact, 3).expect("receiver image emits");
        validate_direct_executable_image(&artifact, &image).unwrap_or_else(|error| {
            panic!("{profile:?} occupancy-tracked binding must replay: {error:?}")
        });
    }
}

/// The same admitted bridge on the Darwin target is a returning dyld entry:
/// emission replays it, and any image mutation still fails closed.
#[test]
fn hosted_receiver_darwin_image_replay_rejects_mutated_bridge_bytes() {
    let profile = TargetProfile::MacosArm64;
    let compiled = compile_attached_entry(RECEIVER_STORE, "Main::launch", profile);
    let mut artifact = compiled.artifact;
    bind_receiver(&mut artifact, &compiled.signature, profile, false);
    let image = emit_direct_executable_image(&artifact, 0).expect("Darwin receiver image emits");
    validate_direct_executable_image(&artifact, &image).expect("exact image replays");

    // The Darwin bridge is sixteen fixed A64 words appended to text; the
    // second-to-last word is the `mov w0, #0` that publishes status zero.
    let mut corrupted = image.clone();
    let len = corrupted.output().final_text_bytes.len();
    corrupted.output_mut_for_test().final_text_bytes[len - 8] ^= 0x01;
    assert!(
        validate_direct_executable_image(&artifact, &corrupted).is_err(),
        "a substituted Darwin bridge word rejects on replay",
    );
}
