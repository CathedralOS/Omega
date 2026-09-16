//! A caller keeping a parameter and earlier call results live across later
//! calls commits a frame whose call sites must execute on the ABI's declared
//! stack boundary. The validated layout records the target-owned call-site
//! contract — the selected preservation convention's declared stack
//! alignment, carried as both `pre_call_stack_alignment` and
//! `abi_stack_alignment_bytes` — and the committed extent keeps exactly the
//! architecture's call-entry residue modulo that alignment, so the
//! post-prologue stack pointer meets the boundary again before any call in
//! the body runs. The emitted prologue commits exactly that extent and the
//! epilogue releases it; replay resolves the same declared contract rather
//! than trusting the producer's record, so a borrowed alignment or a
//! residue-breaking extent fails closed while the artifact still reaches
//! ordinary callable publication on every admitted target.

use crate::tests::{
    AdmissionProfile, MachineId, NativeTarget, OptimizationSelections, canonical_artifact,
    compiler_baseline_request_v1, optimize_artifact_sections, scalar_call_preserving_artifact,
    selected_abi_preservation, stage_function_fragment_frame_application,
    stage_optimized_fixed_frame_text_section, stage_optimized_function_fragment_emission,
    stage_optimized_relocation_free_object_container,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
    stage_validated_optimized_object_artifact, stage_validated_optimized_ordinary_callable_entry,
    validate_optimized_ordinary_callable_entry, validate_target_frame_layout,
};

/// Decode the stack-pointer commit an x86-64 prologue performs before its
/// save slots: the `sub rsp` opening the prologue. This frame never needs
/// probing, so the commit is one immediate.
fn x86_64_prologue_commit(prologue: &[u8]) -> u64 {
    match prologue {
        [0x48, 0x83, 0xec, immediate, ..] => u64::from(*immediate),
        [0x48, 0x81, 0xec, rest @ ..] => {
            u64::from(u32::from_le_bytes(rest[..4].try_into().unwrap()))
        }
        _ => panic!("prologue must open with the stack commit: {prologue:02x?}"),
    }
}

/// Decode the stack-pointer release an x86-64 epilogue performs after its
/// restores: the trailing `add rsp`.
fn x86_64_epilogue_release(epilogue: &[u8]) -> u64 {
    match epilogue {
        [.., 0x48, 0x83, 0xc4, immediate] => u64::from(*immediate),
        rest @ [.., 0x48, 0x81, 0xc4] => u64::from(u32::from_le_bytes(
            rest[rest.len() - 4..].try_into().unwrap(),
        )),
        _ => panic!("epilogue must end with the stack release: {epilogue:02x?}"),
    }
}

/// Decode the stack-pointer commit an AAPCS64 prologue performs before its
/// save slots: the leading `sub sp` words, each immediate applied at its
/// encoded shift.
fn aarch64_prologue_commit(prologue: &[u8]) -> u64 {
    prologue
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| u32::from_le_bytes(*chunk))
        .take_while(|word| word & 0xff00_03ff == 0xd100_03ff)
        .map(|word| u64::from((word >> 10) & 0xfff) << (((word >> 22) & 1) * 12))
        .sum()
}

/// Decode the stack-pointer release an AAPCS64 epilogue performs after its
/// restores: the trailing `add sp` words, each immediate applied at its
/// encoded shift.
fn aarch64_epilogue_release(epilogue: &[u8]) -> u64 {
    epilogue
        .as_chunks::<4>()
        .0
        .iter()
        .rev()
        .map(|chunk| u32::from_le_bytes(*chunk))
        .take_while(|word| word & 0xff00_03ff == 0x9100_03ff)
        .map(|word| u64::from((word >> 10) & 0xfff) << (((word >> 22) & 1) * 12))
        .sum()
}

#[test]
fn call_site_stack_contract_replays_through_ordinary_callable_entry() {
    let caller = MachineId::new(crate::tests::SCALAR_CALL_PRESERVING_CALLER).unwrap();
    let (semantic, proof) = scalar_call_preserving_artifact();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&OptimizationSelections::new([]).unwrap()),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap_or_else(|error| panic!("{target:?} physical: {error:?}"));
        let realization = physical.fixed_frame_for_test();
        let layout = realization.frame().plan();
        let current = realization.allocation().current();
        let row = layout
            .functions
            .iter()
            .find(|function| function.machine == caller)
            .unwrap_or_else(|| panic!("{target:?}: missing caller frame row"));
        assert!(row.contains_call, "{target:?}");
        // The recorded call-site contract is the ABI's own declaration,
        // recovered here from the validated register environment rather than
        // the row's fields: the declared convention stack alignment must
        // appear as both the pre-call boundary the body keeps and the ABI
        // alignment restatement.
        let declared = selected_abi_preservation(current.register_environment())
            .unwrap_or_else(|_| panic!("{target:?}: no declared ABI convention"))
            .convention
            .stack_alignment;
        assert_eq!(row.pre_call_stack_alignment, declared, "{target:?}");
        assert_eq!(row.abi_stack_alignment_bytes, declared, "{target:?}");
        // The committed extent carries exactly the call-entry residue the
        // architecture's call sequence leaves pushed: x86-64 `call` pushes
        // the eight-byte return address, so the frame size keeps residue 8
        // modulo the boundary; AArch64 `bl` writes the link register, so the
        // frame stays on the boundary itself.
        let entry_residue = match target.architecture {
            target::Architecture::X86_64 => 8,
            target::Architecture::Aarch64 => 0,
        };
        assert!(row.frame_size_bytes > 0, "{target:?}");
        assert_eq!(
            row.frame_size_bytes % u64::from(declared),
            entry_residue,
            "{target:?}: the committed extent must restore the call-site boundary: {:?}",
            row.frame_size_bytes
        );
        // The emitted frame performs the recorded contract: the prologue
        // commits exactly the non-resident extent and the epilogue releases
        // it, so every call in the body executes with the stack pointer on
        // the declared boundary.
        let protocol = realization.protocol().plan();
        let encoding = protocol
            .functions
            .iter()
            .find(|function| function.machine == caller)
            .unwrap();
        let prologue = encoding.prologue.bytes(&protocol.bytes).unwrap();
        let epilogue = encoding.epilogue.bytes(&protocol.bytes).unwrap();
        let committed = row.frame_size_bytes - row.red_zone_resident_bytes;
        let (committed_bytes, released_bytes) = match target.architecture {
            target::Architecture::X86_64 => (
                x86_64_prologue_commit(prologue),
                x86_64_epilogue_release(epilogue),
            ),
            target::Architecture::Aarch64 => (
                aarch64_prologue_commit(prologue),
                aarch64_epilogue_release(epilogue),
            ),
        };
        assert_eq!(
            committed_bytes, committed,
            "{target:?}: prologue commits a different extent: {prologue:02x?}"
        );
        assert_eq!(
            released_bytes, committed,
            "{target:?}: epilogue releases a different extent: {epilogue:02x?}"
        );
        // The produced layout replays exactly; a row recording a borrowed
        // alignment or an extent that breaks the call-entry residue fails
        // closed on every target.
        let plan = layout.clone();
        let validate = |candidate: machine_code::TargetFrameLayoutPlan| {
            validate_target_frame_layout(
                realization.machine(),
                realization.requirements(),
                realization.storage(),
                current.register_environment(),
                candidate,
            )
        };
        assert!(validate(plan.clone()).is_ok(), "{target:?}");
        let index = layout
            .functions
            .iter()
            .position(|function| function.machine == caller)
            .unwrap();
        let mut widened = plan.clone();
        widened.functions[index].pre_call_stack_alignment = declared * 2;
        assert!(
            validate(widened).is_err(),
            "{target:?}: a call-site boundary other than the declared alignment is not canonical"
        );
        let mut restated = plan.clone();
        restated.functions[index].abi_stack_alignment_bytes = declared / 2;
        assert!(
            validate(restated).is_err(),
            "{target:?}: the ABI alignment restatement cannot borrow a different value"
        );
        let mut shifted = plan.clone();
        shifted.functions[index].frame_size_bytes += 8;
        assert!(
            validate(shifted).is_err(),
            "{target:?}: an extent that loses the call-entry residue is not canonical"
        );
        // The same program still reaches ordinary callable publication on
        // every admitted target, with its three calls resolved inside the
        // committed frame.
        let emitted = stage_optimized_function_fragment_emission(
            physical.into_function_fragment_emission_source(),
        )
        .unwrap_or_else(|error| panic!("{target:?} emission: {error:?}"));
        let framed = stage_function_fragment_frame_application(emitted)
            .unwrap_or_else(|error| panic!("{target:?} frame: {error:?}"));
        let text = stage_optimized_fixed_frame_text_section(framed)
            .unwrap_or_else(|error| panic!("{target:?} text: {error:?}"));
        let calls = &text.text_section().resolved_internal_machine_calls;
        assert_eq!(calls.len(), 3, "{target:?}");
        assert!(
            calls.iter().all(|call| call.caller == caller),
            "{target:?}: {calls:?}"
        );
        let object = stage_optimized_relocation_free_object_container(text)
            .unwrap_or_else(|error| panic!("{target:?} object: {error:?}"));
        let artifact = stage_validated_optimized_object_artifact(
            canonical_artifact(&semantic, &proof),
            object,
        )
        .unwrap_or_else(|error| panic!("{target:?} artifact: {error:?}"));
        let callable = stage_validated_optimized_ordinary_callable_entry(artifact)
            .unwrap_or_else(|error| panic!("{target:?} callable: {error:?}"));
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&callable).unwrap(),
            callable.custody(),
            "{target:?}"
        );
    }
}
