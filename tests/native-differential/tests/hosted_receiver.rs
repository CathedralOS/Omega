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
