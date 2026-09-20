//! Hosted receiver coverage in the differential lane: a provisioned
//! `&mut self` entry binds the exact per-target bridge, the sealed image
//! replays its custody, and — where the host executes the format — the emitted
//! file runs as a real process with no test-supplied self.

#[path = "common/hosted_receiver.rs"]
mod hosted_receiver;

use target::TargetProfile;

/// A receiver-bearing entry whose stores only land if the emitted bridge
/// provisions the receiver's storage: a mis-provisioned `self` faults under
/// physical execution instead of silently completing.
const RECEIVER_STORE: &str = r#"
    data Main { value: i32; bytes: [u8; 8]; }
    machine Main::main(&mut self) {
        self.value = 17;
        self.bytes[3] = 9;
        transition self.value == 17 && self.bytes[3] == 9 && self.bytes[0] == 0 {
            true -> finish()
            false -> finish()
        }
        state finish(&mut self) { self.value = 0; }
    }
"#;

#[test]
fn hosted_receiver_bridge_binds_emits_and_replays_on_all_hosted_targets() {
    for profile in [
        TargetProfile::LinuxX64,
        TargetProfile::LinuxArm64,
        TargetProfile::MacosArm64,
        TargetProfile::WindowsX64,
    ] {
        let compiled =
            hosted_receiver::compile_attached_entry(RECEIVER_STORE, "Main::main", profile);
        assert!(
            std::ptr::eq(
                compiled
                    .artifact
                    .fragment_source_for_test()
                    .expect("fragment custody is retained"),
                compiled.container.as_ref(),
            ),
            "{profile:?} the object retains the exact staged container custody",
        );
        let mut artifact = compiled.artifact;
        assert!(
            artifact.hosted_receiver_binding().is_none(),
            "{profile:?} emission alone never provisions the receiver",
        );
        hosted_receiver::bind_hosted_receiver(&mut artifact, &compiled.signature, profile);
        assert_eq!(
            artifact
                .hosted_receiver_binding()
                .expect("bound receiver")
                .source(),
            &compiled.signature,
        );

        let image = hosted_receiver::emit_receiver_image(&artifact, profile);
        let magic: &[u8] = match profile {
            TargetProfile::LinuxX64 | TargetProfile::LinuxArm64 => b"\x7fELF",
            TargetProfile::MacosArm64 => &[0xcf, 0xfa, 0xed, 0xfe],
            TargetProfile::WindowsX64 => b"MZ",
            _ => unreachable!(),
        };
        assert!(
            image.output().bytes.starts_with(magic),
            "{profile:?} emits its hosted container",
        );

        if profile == TargetProfile::LinuxX64
            && cfg!(all(target_os = "linux", target_arch = "x86_64"))
        {
            // The Linux x86-64 bridge is a real process entry: kernel arrival,
            // private stack switch, provisioned receiver in rdi, exit_group
            // status zero on normal completion.
            let (code, _stdout, stderr) = hosted_receiver::run_emitted_image(&image.output().bytes);
            assert_eq!(
                code,
                Some(0),
                "hosted receiver process must complete normally: {stderr:?}",
            );
        }
    }
}

/// The bridge's custody gates still hold from the shared lane: a substituted
/// physical contract, a receiver-free source spelling, or dropped fragment
/// custody each reject before any bridge byte exists.
#[test]
fn hosted_receiver_binding_fails_closed_on_substituted_custody() {
    let profile = TargetProfile::LinuxX64;
    let compiled = hosted_receiver::compile_attached_entry(RECEIVER_STORE, "Main::main", profile);
    let signature = compiled.signature.clone();
    let linux = hosted_receiver::physical_contract(profile);
    let demand = image_emission::derive_stack_demand(&compiled.artifact, compiled.artifact.entry())
        .expect("entry stack demand");

    // A Darwin contract cannot satisfy the Linux x86-64 bridge.
    let mut substituted = compiled.artifact.clone();
    assert!(
        image_emission::bind_hosted_receiver(
            &mut substituted,
            &signature,
            &hosted_receiver::physical_contract(TargetProfile::MacosArm64),
            &[],
            &demand,
        )
        .is_err(),
        "a substituted physical contract rejects before binding",
    );

    // A receiver-free spelling cannot stand in for the provisioned receiver
    // the emitted object was compiled for.
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
    let mut substituted = compiled.artifact.clone();
    assert!(
        image_emission::bind_hosted_receiver(
            &mut substituted,
            &free_signature,
            &linux,
            &[],
            &demand
        )
        .is_err(),
        "a receiver source substitution rejects",
    );

    // Without retained fragment custody there is no current entry graph to
    // join: the binder fails closed before any bridge byte exists.
    let mut uncustodied = compiled.artifact;
    uncustodied.clear_fragment_replay_for_test();
    assert!(
        image_emission::bind_hosted_receiver(&mut uncustodied, &signature, &linux, &[], &demand,)
            .is_err(),
        "an object without fragment replay custody cannot bind a receiver",
    );
}

/// An image emitted before binding cannot replay as the bound object's
/// custody: the sealed object carries bridge bytes the pre-binding image
/// never had.
#[test]
fn hosted_receiver_unbound_image_cannot_replay_as_bound() {
    for profile in [
        TargetProfile::LinuxX64,
        TargetProfile::LinuxArm64,
        TargetProfile::MacosArm64,
        TargetProfile::WindowsX64,
    ] {
        let compiled =
            hosted_receiver::compile_attached_entry(RECEIVER_STORE, "Main::main", profile);
        let mut artifact = compiled.artifact;
        let unbound = hosted_receiver::emit_receiver_image_unchecked(&artifact, profile)
            .expect("an unbound object still emits a plain image");
        hosted_receiver::bind_hosted_receiver(&mut artifact, &compiled.signature, profile);
        assert!(
            image_emission::validate_executable_image(&artifact, &unbound).is_err(),
            "{profile:?} a pre-binding image cannot rejoin the bound object",
        );
    }
}

/// Emission re-derives the bound receiver's custody: a substituted source
/// signature, stack ceiling, or receiver extent each rejects before any
/// bridge byte is emitted.
#[test]
fn hosted_receiver_emission_rejects_mutated_binding_custody() {
    for (profile, mutate) in [
        (TargetProfile::LinuxX64, "source"),
        (TargetProfile::LinuxArm64, "demand"),
        (TargetProfile::MacosArm64, "byte_count"),
    ] {
        let compiled =
            hosted_receiver::compile_attached_entry(RECEIVER_STORE, "Main::main", profile);
        let mut artifact = compiled.artifact;
        hosted_receiver::bind_hosted_receiver(&mut artifact, &compiled.signature, profile);
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
            hosted_receiver::emit_receiver_image_unchecked(&artifact, profile).is_err(),
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
    let compiled = hosted_receiver::compile_attached_entry(RECEIVER_STORE, "Main::main", profile);

    // A byte substitution in the object's current text breaks the exact
    // staged-container replay the emission gate performs first.
    let mut artifact = compiled.artifact.clone();
    hosted_receiver::bind_hosted_receiver(&mut artifact, &compiled.signature, profile);
    artifact.text_bytes_mut_for_test()[0] ^= 0xff;
    assert!(
        hosted_receiver::emit_receiver_image_unchecked(&artifact, profile).is_err(),
        "object text divergence from retained custody rejects emission",
    );

    // Dropping custody after binding removes the current entry graph the
    // receiver layout is derived from.
    let mut artifact = compiled.artifact;
    hosted_receiver::bind_hosted_receiver(&mut artifact, &compiled.signature, profile);
    artifact.clear_fragment_replay_for_test();
    assert!(
        hosted_receiver::emit_receiver_image_unchecked(&artifact, profile).is_err(),
        "a bound object without retained custody rejects emission",
    );
}

/// The sealed image replays only against its exact bound object: a corrupted
/// bridge opcode, a retargeted call displacement, or a substituted object all
/// fail the independent replay.
#[test]
fn hosted_receiver_image_replay_rejects_mutated_bridge_bytes() {
    let profile = TargetProfile::LinuxX64;
    let compiled = hosted_receiver::compile_attached_entry(RECEIVER_STORE, "Main::main", profile);
    let mut artifact = compiled.artifact;
    hosted_receiver::bind_hosted_receiver(&mut artifact, &compiled.signature, profile);
    let image = hosted_receiver::emit_receiver_image(&artifact, profile);

    // The bridge occupies the fixed 37-byte suffix of the emitted text:
    // `...; mov eax, 231; syscall; ud2`. Corrupting the syscall opcode is a
    // byte substitution the independent reader decodes back out.
    let mut corrupted_syscall = image.clone();
    let len = corrupted_syscall.output().final_text_bytes.len();
    corrupted_syscall.output_mut_for_test().final_text_bytes[len - 2] = 0x06;
    assert!(
        image_emission::validate_executable_image(&artifact, &corrupted_syscall).is_err(),
        "a substituted bridge opcode rejects on replay",
    );

    // Corrupting the call rel32 displacement retargets the semantic
    // continuation the bridge invokes.
    let mut retargeted = image.clone();
    retargeted.output_mut_for_test().final_text_bytes[len - 15] ^= 0x01;
    assert!(
        image_emission::validate_executable_image(&artifact, &retargeted).is_err(),
        "a retargeted bridge call rejects on replay",
    );

    // The sealed image does not follow a different object's custody either.
    let other = hosted_receiver::compile_attached_entry(
        RECEIVER_STORE,
        "Main::main",
        TargetProfile::LinuxArm64,
    );
    let mut other_artifact = other.artifact;
    hosted_receiver::bind_hosted_receiver(
        &mut other_artifact,
        &other.signature,
        TargetProfile::LinuxArm64,
    );
    assert!(
        image_emission::validate_executable_image(&other_artifact, &image).is_err(),
        "an image cannot be rejoined to a substituted object",
    );
}

/// The Darwin bridge is a returning dyld entry of sixteen fixed A64 words;
/// a substituted word still fails the independent replay.
#[test]
fn hosted_receiver_darwin_image_replay_rejects_mutated_bridge_bytes() {
    let profile = TargetProfile::MacosArm64;
    let compiled = hosted_receiver::compile_attached_entry(RECEIVER_STORE, "Main::main", profile);
    let mut artifact = compiled.artifact;
    hosted_receiver::bind_hosted_receiver(&mut artifact, &compiled.signature, profile);
    let image = hosted_receiver::emit_receiver_image(&artifact, profile);

    // The second-to-last fixed bridge word is the `mov w0, #0` that publishes
    // status zero.
    let mut corrupted = image.clone();
    let len = corrupted.output().final_text_bytes.len();
    corrupted.output_mut_for_test().final_text_bytes[len - 8] ^= 0x01;
    assert!(
        image_emission::validate_executable_image(&artifact, &corrupted).is_err(),
        "a substituted Darwin bridge word rejects on replay",
    );
}
