//! The proof-free i32 scalar-call reference route:
//! `emit_scalar_call_reference_linux_x86_64_image` appends the product-owned
//! Linux x86-64 exit shim to an exact published semantic identity, emits the
//! ELF image, and runs independent final-image replay before returning. The
//! fingerprint gate exists because `ObjectFunction` does not retain scalar
//! arity: an unused entry parameter can produce byte-identical machine code,
//! so only the published identity may acquire zero-argument process-entry
//! semantics. This route has no other in-crate or downstream caller; these
//! tests pin its custody gates, the relocated shim bytes, and the physically
//! executed exit status inside the owning crate.

use super::{integer_return, machine_id, scalar_call_plan, two_function_plan};
use image::FinalExecutableRegionOrigin;
use image_emission::{
    ObjectArtifact, build_object_artifact, emit_scalar_call_reference_linux_x86_64_image,
};
use machine_code::{MachineCodePlan, ScalarControlFlowEvidence, ScalarStackEvidence};
use target::NativeTarget;
use terminal_psi::SemanticFingerprint;

/// The exact published proof-free i32 scalar-call reference identity, repinned
/// here as literal bytes: the emitting crate keeps its constant private, and
/// an independent copy keeps the identity gate honest against drift in either
/// direction.
const SCALAR_CALL_REFERENCE_FINGERPRINT: [u8; 32] = [
    0x02, 0x5f, 0x4b, 0x5a, 0xa3, 0xdf, 0xd7, 0x0c, 0xdc, 0x92, 0x84, 0x3c, 0x41, 0x4c, 0x0b, 0x86,
    0x91, 0x08, 0x9e, 0x09, 0x19, 0x27, 0xb5, 0x49, 0x61, 0xff, 0x45, 0x34, 0xd9, 0x47, 0x64, 0x3d,
];

/// One branch-free scalar entry returning a determinate i32: the emitted shim
/// calls it and exits with its low byte, so byte-level custody and the
/// physical run agree on exit status 37.
fn scalar_reference_plan() -> MachineCodePlan {
    let mut plan = two_function_plan();
    plan.psi.program_fingerprint =
        SemanticFingerprint::from_bytes(SCALAR_CALL_REFERENCE_FINGERPRINT);
    plan.entry = machine_id(1);
    plan.functions.truncate(1);
    let function = &mut plan.functions[0];
    function.bytes = integer_return(37);
    function.scalar_stack = Some(ScalarStackEvidence {
        mutations: Vec::new(),
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    plan
}

fn scalar_reference_artifact() -> ObjectArtifact {
    build_object_artifact(&scalar_reference_plan()).expect("scalar-reference object")
}

#[test]
fn published_scalar_reference_binds_one_exit_shim_and_replays_to_its_result() {
    let artifact = scalar_reference_artifact();
    assert_eq!(
        artifact.relocations().record_count(),
        0,
        "the fixture carries no relocations of its own",
    );
    let entry_text_len = artifact.text_bytes().len();

    let image = emit_scalar_call_reference_linux_x86_64_image(&artifact)
        .expect("the exact scalar-call reference emits its Linux x86-64 image");
    assert_eq!(image.psi(), artifact.psi());
    assert_eq!(image.target(), NativeTarget::linux_x64());

    let shim = image.linux_x86_scalar_exit_shim();
    let entry = artifact.entry_function();
    assert_eq!(shim.text_offset, entry_text_len);
    assert_eq!(shim.byte_count, 16);
    assert_eq!(
        shim.relocation_offset,
        shim.text_offset + 1,
        "the owned relocation covers the call rel32 immediate",
    );
    assert_eq!(
        shim.target_symbol, entry.symbol,
        "the shim calls the exact semantic entry symbol",
    );

    let output = image.output();
    assert!(output.bytes.starts_with(b"\x7fELF"));
    assert_eq!(output.final_image_imports, 0);
    assert_eq!(
        output.final_image_relocations, 1,
        "the shim call relocation is the image's only relocation",
    );
    assert_eq!(
        output.final_text_bytes.len(),
        entry_text_len + shim.byte_count,
    );
    assert!(output.compiler_text_validation.is_some());

    // Independently parse the ELF64 header: e_entry selects the appended shim
    // inside .text, not the semantic entry.
    let encoded_entry = u64::from_le_bytes(
        output.bytes[24..32]
            .try_into()
            .expect("ELF64 e_entry field"),
    );
    assert_eq!(
        encoded_entry,
        output.final_image_layout.text_address + shim.text_offset as u64,
    );

    let shim_regions = output
        .executable_regions
        .regions
        .iter()
        .filter(|region| region.symbol == "omega_terminal_linux_x86_64_scalar_exit_entry")
        .collect::<Vec<_>>();
    assert_eq!(
        shim_regions.len(),
        1,
        "the exit shim binds exactly one executable region",
    );
    assert_eq!(
        shim_regions[0].origin,
        FinalExecutableRegionOrigin::CompilerFunction,
    );
    assert_eq!(shim_regions[0].section_offset, shim.text_offset);
    assert_eq!(shim_regions[0].byte_count, shim.byte_count);

    // The writer relocates only the call immediate: it resolves to the
    // semantic entry and every other shim byte is verbatim.
    let final_shim = &output.final_text_bytes[shim.text_offset..shim.text_offset + shim.byte_count];
    assert_eq!(final_shim[0], 0xe8, "call rel32 opcode");
    assert_eq!(
        &final_shim[5..],
        &[0x89, 0xc7, 0xb8, 0xe7, 0, 0, 0, 0x0f, 0x05, 0x0f, 0x0b],
        "exit_group(edi = result) body stays byte-exact",
    );
    let displacement = i32::from_le_bytes(final_shim[1..5].try_into().expect("rel32 immediate"));
    let call_end = output.final_image_layout.text_address + shim.text_offset as u64 + 5;
    assert_eq!(
        call_end.wrapping_add_signed(i64::from(displacement)),
        output.final_image_layout.text_address + entry.text_offset as u64,
        "the relocated call reaches the semantic entry function",
    );

    // The emitted image is a real static executable: the entry's i32 result
    // becomes the low byte of the exit_group status. The image is Linux
    // x86-64 — the shim bytes asserted above are x86-64 opcodes ending in
    // `exit_group` — so only that host can run it; `assert_exit` admits
    // Linux AArch64 and macOS AArch64 too, where this ELF is not executable.
    if NativeTarget::host() == NativeTarget::linux_x64() {
        crate::hosted_exit_runtime::assert_exit(&output.bytes, 37);
    }
}

#[test]
fn scalar_call_reference_rejects_foreign_target_identity_or_unaccounted_entry() {
    // Another target cannot host the Linux x86-64 shim even under the exact
    // published identity.
    let mut aarch64_plan = scalar_call_plan(NativeTarget::macos_arm64());
    aarch64_plan.psi.program_fingerprint =
        SemanticFingerprint::from_bytes(SCALAR_CALL_REFERENCE_FINGERPRINT);
    let aarch64 = build_object_artifact(&aarch64_plan).expect("AArch64 scalar object");
    let error = emit_scalar_call_reference_linux_x86_64_image(&aarch64)
        .expect_err("the shim cannot target another architecture");
    assert!(
        error.message.contains("scalar entry shim cannot target"),
        "unexpected diagnostic: {}",
        error.message,
    );

    // The same scalar body under a different semantic identity is not the
    // published reference: scalar arity is erased from `ObjectFunction`, so
    // the fingerprint is the only binding against a byte-identical impostor.
    let mut renamed_plan = scalar_reference_plan();
    renamed_plan.psi.program_fingerprint = SemanticFingerprint::from_bytes([9; 32]);
    let renamed = build_object_artifact(&renamed_plan).expect("foreign-identity object");
    let error = emit_scalar_call_reference_linux_x86_64_image(&renamed)
        .expect_err("a foreign semantic identity must not acquire the shim");
    assert!(
        error
            .message
            .contains("requires the exact published semantic identity"),
        "unexpected diagnostic: {}",
        error.message,
    );

    // An entry without retained scalar stack custody cannot take the
    // returning-scalar shim even under the exact identity.
    let mut unaccounted_plan = scalar_reference_plan();
    unaccounted_plan.functions[0].scalar_stack = None;
    let unaccounted = build_object_artifact(&unaccounted_plan).expect("unaccounted-scalar object");
    let error = emit_scalar_call_reference_linux_x86_64_image(&unaccounted)
        .expect_err("an entry without scalar custody must reject");
    assert!(
        error
            .message
            .contains("not a completely accounted returning scalar function"),
        "unexpected diagnostic: {}",
        error.message,
    );

    // Drifted entry bytes lose the returning boundary: the last text byte is
    // the entry's `ret`, and mutating it severs the scalar-return contract.
    let mut drifted = scalar_reference_artifact();
    let last = drifted.text_bytes().len() - 1;
    drifted.text_bytes_mut_for_test()[last] = 0x90;
    let error = emit_scalar_call_reference_linux_x86_64_image(&drifted)
        .expect_err("a drifted entry terminator must reject");
    assert!(
        error
            .message
            .contains("not a completely accounted returning scalar function"),
        "unexpected diagnostic: {}",
        error.message,
    );
}
