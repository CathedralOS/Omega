use super::assert_native_exit_code;
use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, compile, compile_rooted_canary_for_native_host,
    compile_with_auxiliary_artifacts, fs, hosted_main_program_entry_build_for, pass_canary,
};

#[test]
fn entry_run_args_bytes_canary_runs() {
    // The canonical entry `Main::run(&self, args: &[u8])`: the prologue binds
    // `args` as a 32-byte view over the spilled argument registers, so
    // `args.len == 32` holds deterministically (exit 5) regardless of what the
    // OS passed in the registers. NATIVE-ONLY (the interpreter has no entry-
    // argument notion yet, so this is not a differential canary). The
    // efi_application twin of this program was boot-verified under QEMU/OVMF
    // ("Warning Stale Data" = the same 5).
    let canary = pass_canary(fixture_roster::TARGETS_ENTRY_RUN_ARGS_BYTES);
    let build_dir = std::env::temp_dir().join(format!("omega-run-args-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_with_auxiliary_artifacts(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("entry run-args canary should compile");
    let footprint_artifact = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
        .expect("entry run-args footprint evidence should be written");
    assert!(
        footprint_artifact.contains("\"origin\": \"entry_storage\"")
            && footprint_artifact.contains("\"origin\": \"entry_slice_descriptor\"")
            && footprint_artifact.contains("\"origin\": \"exit_result_registers\"")
            && footprint_artifact.contains("\"enumeration_complete\": false"),
        "bytes handoff must retain entry-storage, descriptor, and exit-register evidence without claiming final completeness"
    );
    assert_native_exit_code(
        &compilation,
        5,
        "entry run-args canary",
        "the canonical byte-view argument should retain its 32-byte handoff bound",
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_utf16_literal_exit_canary_runs() {
    // `utf16"Hello from Omega"` (CR LF NUL escaped) desugars at parse to the integer array
    // literal of its UTF-16 code units: 'H'=72 at [0], newline=10 at [17], NUL at
    // [18] (exit 70). Native must match the interpreter (both see plain
    // integers -- the sugar is gone before resolution).
    let canary = pass_canary(fixture_roster::TEXT_RUNTIME_UTF16_LITERAL_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-utf16-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("utf16 literal canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "Utf16 literal canary",
        "the greeting's exact Utf16 code units should verify",
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_case_array_element_write_exit_canary_runs() {
    // Array-of-CASE element writes (const + runtime index) with payload
    // read-back -- the case-vocabulary Plan's foundation shape. At{8,4}+At{16,8}
    // matched back = 12 + 24 = exit 36; native must match the interpreter (the
    // interpreter is the L0 build-time engine).
    let canary = pass_canary(fixture_roster::COLLECTIONS_RUNTIME_CASE_ARRAY_ELEMENT_WRITE_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-case-array-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("case-array element write canary should compile");
    assert_native_exit_code(
        &compilation,
        36,
        "case-array element-write canary",
        "both constant- and runtime-indexed case payloads should read back",
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_policy_authored_plan_exit_canary_runs() {
    // RUNG 2b: an inline `CompactBinary::plan` grammar policy AUTHORS the wire
    // plan (L0-evaluated against materialized schema facts incl. FieldKind);
    // the codec's tag bytes come from it, and the hand-computed roundtrip
    // bytes still hold exactly (exit 70). The fail twin proves divergence is
    // a compile error.
    let canary = pass_canary(fixture_roster::WIRE_RUNTIME_WIRE_POLICY_AUTHORED_PLAN_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-wire-policy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("policy-authored wire plan canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "policy-authored wire-plan canary",
        "the authored plan should roundtrip its exact bytes",
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_policy_authored_nested_exit_canary_runs() {
    // RUNG 2c: nested CHILD tags come from the child schema's own authored
    // plan -- the byte-pinned nested roundtrip holds exactly with the inline
    // `CompactBinary::plan` policy evaluated for both parent and child.
    let canary = pass_canary(fixture_roster::WIRE_RUNTIME_WIRE_POLICY_AUTHORED_NESTED_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-wire-policy-nested-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("policy-authored nested wire plan canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "policy-authored nested wire-plan canary",
        "the parent and child authored plans should roundtrip their exact nested bytes",
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["windows_x86_64", "linux_arm64"] {
        let cross_dir = std::env::temp_dir().join(format!(
            "omega-wire-policy-nested-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&cross_dir);
        let source_dir = cross_dir.join("src");
        let cross_build_dir = cross_dir.join("build");
        fs::create_dir_all(&source_dir).expect("create wire-policy cross-target source");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy nested wire-policy canary");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write wire-policy cross-build source");
        compile(CanaryCompileSpec {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(cross_build_dir),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("nested wire policy should cross-compile for {target}: {diagnostics:?}")
        });
        let _ = fs::remove_dir_all(&cross_dir);
    }
}

#[cfg(windows)]
#[test]
fn efi_struct_handoff_prologue_spreads_registers() {
    // Ladder step 3: the boundary entry's sole struct parameter receives the
    // argument registers spread across its 8-byte chunks. Pins the prologue:
    // store #0 = mov r15,imm64 + mov [r15+0],rcx (49 89 8F disp 0); store #1 =
    // mov r15,imm64 + mov [r15+8],rdx (49 89 97 disp 8).
    let canary = pass_canary(fixture_roster::TARGETS_EFI_STRUCT_HANDOFF);
    let build_dir =
        std::env::temp_dir().join(format!("omega-struct-handoff-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("struct-handoff canary should compile");
    let bytes = fs::read(build_dir.join("omega-program.exe")).expect("read emitted PE");
    let lfanew = u32::from_le_bytes([bytes[0x3c], bytes[0x3d], bytes[0x3e], bytes[0x3f]]) as usize;
    let opt = lfanew + 4 + 20;
    let opt_size = u16::from_le_bytes([bytes[lfanew + 4 + 16], bytes[lfanew + 4 + 17]]) as usize;
    let section_count = u16::from_le_bytes([bytes[lfanew + 6], bytes[lfanew + 7]]) as usize;
    let mut text_raw = None;
    for section in 0..section_count {
        let header = opt + opt_size + section * 40;
        if &bytes[header..header + 5] == b".text" {
            text_raw = Some(u32::from_le_bytes([
                bytes[header + 20],
                bytes[header + 21],
                bytes[header + 22],
                bytes[header + 23],
            ]) as usize);
        }
    }
    let text = text_raw.expect(".text section");
    // store #0: [10-byte mov r15,imm64] 49 89 8F <disp32 0>
    assert_eq!(&bytes[text..text + 2], &[0x49, 0xbf], "frame-base mov #0");
    assert_eq!(
        &bytes[text + 10..text + 17],
        &[0x49, 0x89, 0x8f, 0, 0, 0, 0],
        "rcx -> handoff.handle @ +0"
    );
    // store #1 immediately follows: 49 BF ... 49 89 97 08 00 00 00
    let second = text + 17;
    assert_eq!(
        &bytes[second..second + 2],
        &[0x49, 0xbf],
        "frame-base mov #1"
    );
    assert_eq!(
        &bytes[second + 10..second + 17],
        &[0x49, 0x89, 0x97, 8, 0, 0, 0],
        "rdx -> handoff.table @ +8"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[cfg(windows)]
#[test]
fn efi_vtable_call_emits_indirect_dispatch() {
    // The external-leaf VtableField(output_string) call lowers to `mov rax, [rcx+8];
    // call rax` -- read OutputString from the con_out protocol struct and
    // dispatch. Pins those bytes in .text (the whole selection->encode chain;
    // the live boot awaits the reference-param projection routing fix).
    let canary = pass_canary(fixture_roster::TARGETS_EFI_VTABLE_CALL);
    let build_dir = std::env::temp_dir().join(format!("omega-vtable-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_with_auxiliary_artifacts(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("vtable-call canary should compile");
    let bytes = fs::read(build_dir.join("omega-program.exe")).expect("read emitted PE");
    let needle = [0x48u8, 0x8b, 0x81, 0x08, 0x00, 0x00, 0x00, 0xff, 0xd0];
    assert!(
        bytes.windows(needle.len()).any(|window| window == needle),
        "expected `mov rax, [rcx+8]; call rax` (named vtable-field dispatch) in .text"
    );
    let footprints = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
        .expect("vtable-call boundary footprints should be emitted");
    assert!(
        footprints.contains("\"origin\": \"compiler_body_outbound_indirect_call\""),
        "vtable dispatch must retain its independently derived call footprint"
    );
    let regions = fs::read_to_string(build_dir.join("13_executable_regions.json"))
        .expect("vtable-call executable-region evidence should be emitted");
    assert!(
        regions.contains("\"certificate_marker\": \"omega.final-footprint-certificate.current\"")
            && regions.contains("\"compiler_function_body_specification\""),
        "vtable dispatch must reach final-byte replay"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_ref_param_direct_faces_deref_not_flat() {
    // Task #37: the DIRECT guard-subject and machine-target reads through an
    // entry ref-param must DEREFERENCE the pointer slot (pointee copies in the
    // report), never fold flat (`frame_storage@72` = slot 8 + con_out 64).
    let canary = pass_canary(fixture_roster::TARGETS_EFI_REF_PARAM_DIRECT_FACES);
    let build_dir = std::env::temp_dir().join(format!("omega-refparam-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_with_auxiliary_artifacts(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("ref-param direct-faces canary should compile");
    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert!(
        report.contains("omega_runtime_frame_storage[ConstOffset(8), Deref, ConstOffset(64)]"),
        "expected the con_out DEREF (place frame[8].deref+64) in the report"
    );
    assert!(
        report.contains("omega_runtime_frame_storage[ConstOffset(8), Deref, ConstOffset(32)]"),
        "expected the firmware_revision DEREF (place frame[8].deref+32) in the report"
    );
    assert!(
        report.contains("omega_runtime_frame_storage[ConstOffset(8), Deref, ConstOffset(48)]"),
        "expected the con_in DEREF (place frame[8].deref+48) feeding the transition arg"
    );
    assert!(
        !report.contains("omega_runtime_frame_storage[ConstOffset(72)]"),
        "flat slot+field read (frame place ConstOffset(72) = con_out) regressed -- an entry-ref-param member folded flat instead of dereferencing"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_ref_param_call_arg_derefs_and_dispatches() {
    // The direct host-call arg `output_string(table.con_out, ..)` must deref
    // (pointee frame@8 +64), never fold flat (frame_storage@72 fed firmware
    // poison into the vtable dispatch), and the `mov rax,[rcx+8]; call rax`
    // dispatch bytes must survive the hoist.
    let canary = pass_canary(fixture_roster::TARGETS_EFI_REF_PARAM_CALL_ARG);
    let build_dir = std::env::temp_dir().join(format!("omega-refarg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_with_auxiliary_artifacts(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("ref-param call-arg canary should compile");
    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert!(
        report.contains("omega_runtime_frame_storage[ConstOffset(8), Deref, ConstOffset(64)]"),
        "expected the con_out DEREF (place frame[8].deref+64) feeding the call arg"
    );
    assert!(
        !report.contains("omega_runtime_frame_storage[ConstOffset(72)]"),
        "flat slot+field read (frame place ConstOffset(72)) regressed for the call-arg face"
    );
    let bytes = fs::read(build_dir.join("omega-program.exe")).expect("read emitted PE");
    let needle = [0x48u8, 0x8b, 0x81, 0x08, 0x00, 0x00, 0x00, 0xff, 0xd0];
    assert!(
        bytes.windows(needle.len()).any(|window| window == needle),
        "expected `mov rax, [rcx+8]; call rax` (named vtable-field dispatch) in .text"
    );
    let _ = fs::remove_dir_all(&build_dir);
}
