use super::assert_native_exit_code;
use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, compile, compile_rooted_canary_for_native_host, fs,
    hosted_main_program_entry_build_for, hosted_program_entry_owner, native_hosted_target,
    pass_canary,
};

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
fn efi_vtable_call_emits_indirect_dispatch() {
    // The external-leaf VtableField(output_string) call lowers to `mov rax, [rcx+8];
    // call rax` -- read OutputString from the con_out protocol struct and
    // dispatch. Pins those bytes in .text (the whole selection->encode chain;
    // the live boot awaits the reference-param projection routing fix).
    let canary = pass_canary(fixture_roster::TARGETS_EFI_VTABLE_CALL);
    let build_dir = std::env::temp_dir().join(format!("omega-vtable-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
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
fn efi_ref_param_call_arg_derefs_and_dispatches() {
    // The direct host-call arg `output_string(table.con_out, ..)` must deref
    // (pointee frame@8 +64), never fold flat (frame_storage@72 fed firmware
    // poison into the vtable dispatch), and the `mov rax,[rcx+8]; call rax`
    // dispatch bytes must survive the hoist.
    let canary = pass_canary(fixture_roster::TARGETS_EFI_REF_PARAM_CALL_ARG);
    let build_dir = std::env::temp_dir().join(format!("omega-refarg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
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

#[test]
fn acquires_through_helper_return_original_main_entry_runs() {
    // ENTRY-CONTENT-ROOTS: the entry receiver's routed-Service custody sits
    // three records deep -- `Main{ backup: Backup{ vault: Vault{ desktop:
    // Binding<Desktop> }}}` erases to a zero-extent receiver whose only
    // remaining obligation is the transitive Fused field's provisioned
    // occurrence. Binding `Main::main` (not the probe shim the fixture's
    // authored build selects) must replay that erased field against its
    // selected provider plan and run the original nested binding/
    // helper-return chain natively: `main -> Backup::stage -> Vault::pick
    // -> Desktop::choose_folder` through `DesktopProvider`, exiting clean.
    let canary = pass_canary(fixture_roster::ACQUIRES_THROUGH_HELPER_RETURN);
    let target = native_hosted_target();
    let scratch = std::env::temp_dir().join(format!("omega-acquires-entry-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let source = scratch.join("source");
    fs::create_dir_all(&source).expect("create acquires scratch source directory");
    fs::copy(canary.join("main.omg"), source.join("main.omg"))
        .expect("copy acquires_through_helper_return source");
    let root_owner = hosted_program_entry_owner(target);
    fs::write(
        source.join("build.omg"),
        format!(
            "machine build(builder: &mut Build) {{\n    builder.application(\"acquires_helper_return_main\");\n    builder.select_provider<Desktop, DesktopProvider>();\n    builder.roots.bind({root_owner}::ProgramEntry, Main::main);\n}}\n"
        ),
    )
    .expect("write Main::main entry binding with the Fused Desktop selection");
    let build_dir = scratch.join("out");
    let compilation = compile(CanaryCompileSpec {
        root_path: source.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some(target.into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .unwrap_or_else(|diagnostics| {
        panic!("nested-Service program entry must produce its executable: {diagnostics:#?}")
    });
    assert_native_exit_code(
        &compilation,
        0,
        "acquires_through_helper_return Main::main",
        "the erased transitive Fused field must provision its occurrence and run the helper-return chain",
    );
    let _ = fs::remove_dir_all(&scratch);
}
