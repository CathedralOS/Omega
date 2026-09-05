use super::*;

#[path = "../fixture_rosters/inline_asm.rs"]
pub(super) mod fixture_roster;

fn compile_x86_image(canary_name: &str, build_name: &str, success: &str) -> (PathBuf, Vec<u8>) {
    let canary = pass_canary(canary_name);
    let build_dir = std::env::temp_dir().join(format!("{build_name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .unwrap_or_else(|diagnostics| panic!("{success}:\n{diagnostics:#?}"));

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86 ELF");
    (build_dir, image)
}

fn assert_aarch64_rejects(canary_name: &str, expected: &str) {
    let canary = pass_canary(canary_name);
    let diagnostics = compile_canary_without_output_for_target(&canary, "linux_arm64")
        .expect_err("x86-only assembly must refuse an AArch64 target");
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains(expected),
        "expected `{expected}`, got:\n{rendered}"
    );
}

fn assert_contract_rejects(canary_name: &str, expected: &str) {
    let diagnostics = compile_canary_without_output(&fail_canary(canary_name))
        .expect_err("invalid assembly contract should reject");
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains(expected),
        "expected `{expected}` for {canary_name}, got:\n{rendered}"
    );
}

#[test]
fn x86_asm_fences_emit_exact_bytes_and_refuse_aarch64() {
    let (build_dir, image) = compile_x86_image(
        fixture_roster::ASM_FENCES_COMPILE,
        "omega-asm-fences",
        "x86 memory fences should cross-compile",
    );
    let sequence = [
        0x0f, 0xae, 0xe8, // lfence
        0x0f, 0xae, 0xf8, // sfence
        0x0f, 0xae, 0xf0, // mfence
    ];
    assert!(
        image
            .windows(sequence.len())
            .any(|window| window == sequence),
        "expected consecutive LFENCE/SFENCE/MFENCE bytes in the emitted image"
    );
    let _ = fs::remove_dir_all(build_dir);

    assert_aarch64_rejects(
        fixture_roster::ASM_FENCES_COMPILE,
        "asm instruction `lfence` is x86_64-only",
    );
}

#[test]
fn x86_asm_interrupt_control_emits_exact_bytes_and_refuses_aarch64() {
    let (build_dir, image) = compile_x86_image(
        fixture_roster::ASM_INTERRUPT_CONTROL_COMPILE,
        "omega-asm-interrupt-control",
        "x86 interrupt control should cross-compile under the boot-root authority",
    );
    assert!(
        image.windows(2).any(|window| window == [0xfa, 0xfb]),
        "expected consecutive CLI/STI bytes in the emitted image"
    );
    let _ = fs::remove_dir_all(build_dir);

    assert_aarch64_rejects(
        fixture_roster::ASM_INTERRUPT_CONTROL_COMPILE,
        "asm instruction `cli` is x86_64-only",
    );
}

#[test]
fn hosted_cli_cannot_claim_machine_owner_authority_with_an_effect_row() {
    assert_contract_rejects(
        fixture_roster::ASM_CLI_REQUIRES_MACHINE_AUTHORITY,
        "asm instruction `cli`, which requires a FREESTANDING boundary root",
    );
}

#[test]
fn x86_asm_flags_emit_balanced_sequences_and_refuse_aarch64() {
    let (build_dir, image) = compile_x86_image(
        fixture_roster::ASM_FLAGS_COMPILE,
        "omega-asm-flags",
        "x86 RFLAGS operations should compile under boot-root authority",
    );
    assert!(
        image
            .windows(5)
            .any(|window| window == [0x9c, 0x41, 0x5a, 0x49, 0xbf]),
        "expected pushfq; pop r10; destination-base load"
    );
    assert!(
        image.windows(3).any(|window| window == [0x41, 0x52, 0x9d]),
        "expected push r10; popfq balanced restore tail"
    );
    let _ = fs::remove_dir_all(build_dir);

    assert_aarch64_rejects(
        fixture_roster::ASM_FLAGS_COMPILE,
        "asm instruction `pushfq` is x86_64-only",
    );
}

#[test]
fn asm_flags_enforce_authority_and_saved_place_contracts() {
    for &(name, expected) in fixture_roster::FLAGS_FAIL_CANARIES {
        assert_contract_rejects(name, expected);
    }
}

#[test]
fn x86_asm_msr_emits_structured_sequences_and_refuse_aarch64() {
    let (build_dir, image) = compile_x86_image(
        fixture_roster::ASM_MSR_COMPILE,
        "omega-asm-msr",
        "x86 MSR operations should compile under boot-root authority",
    );
    assert!(
        image
            .windows(5)
            .any(|window| window == [0x0f, 0x32, 0x41, 0x89, 0xc2]),
        "expected RDMSR followed by the EDX:EAX combine"
    );
    assert!(
        image
            .windows(6)
            .any(|window| window == [0x48, 0xc1, 0xea, 0x20, 0x0f, 0x30]),
        "expected the high-half split followed by WRMSR"
    );
    let _ = fs::remove_dir_all(build_dir);

    assert_aarch64_rejects(
        fixture_roster::ASM_MSR_COMPILE,
        "asm instruction `rdmsr` is x86_64-only",
    );
}

#[test]
fn asm_msr_enforces_authority_and_value_contracts() {
    for &(name, expected) in fixture_roster::MSR_FAIL_CANARIES {
        assert_contract_rejects(name, expected);
    }
}

#[test]
fn x86_asm_control_registers_emit_exact_sequences_and_refuse_aarch64() {
    let (build_dir, image) = compile_x86_image(
        fixture_roster::ASM_CONTROL_REGISTERS_COMPILE,
        "omega-asm-control-registers",
        "x86 control-register operations should compile under boot-root authority",
    );
    for (register, read, write) in [
        (
            "cr0",
            [0x41, 0x0f, 0x20, 0xc2],
            Some([0x41, 0x0f, 0x22, 0xc2]),
        ),
        ("cr2", [0x41, 0x0f, 0x20, 0xd2], None),
        (
            "cr3",
            [0x41, 0x0f, 0x20, 0xda],
            Some([0x41, 0x0f, 0x22, 0xda]),
        ),
        (
            "cr4",
            [0x41, 0x0f, 0x20, 0xe2],
            Some([0x41, 0x0f, 0x22, 0xe2]),
        ),
    ] {
        assert!(
            image.windows(4).any(|window| window == read),
            "expected exact read sequence for {register}"
        );
        if let Some(write) = write {
            assert!(
                image.windows(4).any(|window| window == write),
                "expected exact write sequence for {register}"
            );
        }
    }
    let _ = fs::remove_dir_all(build_dir);

    assert_aarch64_rejects(
        fixture_roster::ASM_CONTROL_REGISTERS_COMPILE,
        "asm instruction `read_cr0` is x86_64-only",
    );
}

#[test]
fn asm_control_registers_enforce_authority_and_value_contracts() {
    for &(name, expected) in fixture_roster::CONTROL_REGISTER_FAIL_CANARIES {
        assert_contract_rejects(name, expected);
    }
}
