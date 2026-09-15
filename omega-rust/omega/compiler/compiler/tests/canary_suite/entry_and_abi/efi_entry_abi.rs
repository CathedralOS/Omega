use super::fixture_roster;
use super::write_cross_target_application_build;
use crate::{CanaryCompileProduct, CanaryCompileSpec, compile, fs, pass_canary};

#[test]
fn efi_freestanding_skeleton_emits_importless_subsystem_10_pe() {
    // The first-boot milestone-1 skeleton (BOOTED under QEMU/OVMF 2026-07-03:
    // "Image Return Status = Success"; returning 5 printed "Warning Stale
    // Data"). This build independently selects subsystem 10 and a freestanding
    // empty host ABI baseline, so the emitted PE32+ must have no import directory/IAT
    // (services arrive via the entry's parameters, never imports). This pins
    // the emitted HEADER FACTS so a regression that re-populates host bindings
    // for the EFI target (or loses the empty-import path) fails here, without
    // needing QEMU in CI.
    let canary = pass_canary(fixture_roster::TARGETS_EFI_FREESTANDING_SKELETON);
    let build_dir = std::env::temp_dir().join(format!("omega-efi-skeleton-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("EFI freestanding skeleton should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    let e_lfanew = u32::from_le_bytes(image[0x3c..0x40].try_into().unwrap()) as usize;
    let optional_header = e_lfanew + 4 + 20;
    let magic = u16::from_le_bytes(
        image[optional_header..optional_header + 2]
            .try_into()
            .unwrap(),
    );
    assert_eq!(magic, 0x20b, "expected a PE32+ optional header");
    let subsystem = u16::from_le_bytes(
        image[optional_header + 68..optional_header + 70]
            .try_into()
            .unwrap(),
    );
    assert_eq!(subsystem, 10, "expected subsystem EFI_APPLICATION (10)");
    // Data directories: import = index 1, IAT = index 12 (each 8 bytes at
    // optional_header + 112 + index*8 for PE32+).
    let directory = |index: usize| -> (u32, u32) {
        let offset = optional_header + 112 + index * 8;
        (
            u32::from_le_bytes(image[offset..offset + 4].try_into().unwrap()),
            u32::from_le_bytes(image[offset + 4..offset + 8].try_into().unwrap()),
        )
    };
    assert_eq!(directory(1), (0, 0), "expected NO import directory");
    assert_eq!(directory(12), (0, 0), "expected NO import address table");

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_entry_arguments_prologue_unmarshals_rcx_rdx() {
    // The entry prologue's argument unmarshal (BOOT-VERIFIED under QEMU/OVMF:
    // the same program returns 4 without the stub and 5 with it -- firmware's
    // ImageHandle arrives through RCX). Pin the emitted OPCODE SKELETON at the
    // very start of .text so CI needs no QEMU: for each of the two declared
    // parameters, `mov r15, imm64` (49 BF <8-byte frame base, relocated>) then
    // `mov [r15+disp32], rcx/rdx` (49 89 8F / 49 89 97 + disp32 0 / 8).
    let canary = pass_canary(fixture_roster::TARGETS_EFI_ENTRY_ARGUMENTS);
    let build_dir = std::env::temp_dir().join(format!("omega-efi-args-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("EFI entry-arguments canary should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    // Locate .text's raw offset from the section table.
    let e_lfanew = u32::from_le_bytes(image[0x3c..0x40].try_into().unwrap()) as usize;
    let optional_size =
        u16::from_le_bytes(image[e_lfanew + 20..e_lfanew + 22].try_into().unwrap()) as usize;
    let section_count = u16::from_le_bytes(image[e_lfanew + 6..e_lfanew + 8].try_into().unwrap());
    let sections = e_lfanew + 4 + 20 + optional_size;
    let text_raw = (0..section_count as usize)
        .map(|index| sections + index * 40)
        .find(|offset| &image[*offset..*offset + 6] == b".text\0")
        .map(|offset| {
            u32::from_le_bytes(image[offset + 20..offset + 24].try_into().unwrap()) as usize
        })
        .expect(".text section should exist");

    let entry = image[text_raw..]
        .windows(34)
        .find(|window| {
            window[0..2] == [0x49, 0xbf]
                && window[10..13] == [0x49, 0x89, 0x8f]
                && window[17..19] == [0x49, 0xbf]
                && window[27..30] == [0x49, 0x89, 0x97]
        })
        .expect("entry argument unmarshal sequence should follow the target prologue");
    // Store 0: mov r15, imm64 ; mov [r15+0], rcx
    assert_eq!(&entry[0..2], &[0x49, 0xbf], "store 0: mov r15, imm64");
    assert_eq!(
        &entry[10..13],
        &[0x49, 0x89, 0x8f],
        "store 0: mov [r15+disp32], rcx"
    );
    assert_eq!(
        &entry[13..17],
        &0u32.to_le_bytes(),
        "param 0 frame offset 0"
    );
    // Store 1: mov r15, imm64 ; mov [r15+8], rdx
    assert_eq!(&entry[17..19], &[0x49, 0xbf], "store 1: mov r15, imm64");
    assert_eq!(
        &entry[27..30],
        &[0x49, 0x89, 0x97],
        "store 1: mov [r15+disp32], rdx"
    );
    assert_eq!(
        &entry[30..34],
        &8u32.to_le_bytes(),
        "param 1 frame offset 8"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_float_entry_argument_unmarshals_xmm0() {
    let canary = pass_canary(fixture_roster::TARGETS_EFI_FLOAT_ENTRY_ARGUMENT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-float-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("EFI float entry-argument canary should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    let e_lfanew = u32::from_le_bytes(image[0x3c..0x40].try_into().unwrap()) as usize;
    let optional_size =
        u16::from_le_bytes(image[e_lfanew + 20..e_lfanew + 22].try_into().unwrap()) as usize;
    let section_count = u16::from_le_bytes(image[e_lfanew + 6..e_lfanew + 8].try_into().unwrap());
    let sections = e_lfanew + 4 + 20 + optional_size;
    let text_raw = (0..section_count as usize)
        .map(|index| sections + index * 40)
        .find(|offset| &image[*offset..*offset + 6] == b".text\0")
        .map(|offset| {
            u32::from_le_bytes(image[offset + 20..offset + 24].try_into().unwrap()) as usize
        })
        .expect(".text section should exist");

    let entry = image[text_raw..]
        .windows(19)
        .find(|window| {
            window[0..2] == [0x49, 0xbf] && window[10..15] == [0xf2, 0x41, 0x0f, 0x11, 0x87]
        })
        .expect("floating entry argument unmarshal should follow the target prologue");
    assert_eq!(&entry[0..2], &[0x49, 0xbf], "mov r15, frame base");
    assert_eq!(
        &entry[10..15],
        &[0xf2, 0x41, 0x0f, 0x11, 0x87],
        "movsd [r15+disp32], xmm0"
    );
    assert_eq!(&entry[15..19], &0u32.to_le_bytes(), "frame offset 0");

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_float_entry_result_round_trips_through_xmm0() {
    let canary = pass_canary(fixture_roster::TARGETS_EFI_FLOAT_RESULT_ENTRY);
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-float-result-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("EFI float arithmetic entry-result canary should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    assert!(
        image
            .windows(5)
            .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x87]),
        "expected incoming f64 stored from XMM0"
    );
    assert!(
        image
            .windows(5)
            .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x87]),
        "expected computed terminal f64 loaded into XMM0"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_literal_entry_result_uses_native_vector_registers() {
    let canary = pass_canary(fixture_roster::TARGETS_EFI_FLOAT_LITERAL_RESULT_ENTRY);
    for target in ["uefi_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-{target}-float-literal-result-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let src_dir = scratch.join("src");
        let out_dir = scratch.join("out");
        fs::create_dir_all(&src_dir).expect("scratch source directory");
        fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
        write_cross_target_application_build(&src_dir);
        compile(CanaryCompileSpec {
            root_path: src_dir.join("main.omg"),
            build_dir: Some(out_dir.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .expect("float-literal entry result should cross-compile");

        let output_name = if target == "uefi_x86_64" {
            "omega-program.exe"
        } else {
            "omega-program"
        };
        let image = fs::read(out_dir.join(output_name)).expect("read emitted image");
        if target == "uefi_x86_64" {
            assert!(
                image
                    .windows(5)
                    .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x87]),
                "UEFI x64 float-literal entry missing terminal XMM0 scratch load"
            );
            assert!(
                image
                    .windows(8)
                    .any(|window| { window == 1.5f64.to_bits().to_le_bytes() }),
                "UEFI x64 float-literal entry missing the f64 bit pattern"
            );
        } else {
            assert!(
                image.windows(4).any(|window| {
                    let instruction = u32::from_le_bytes(window.try_into().expect("word"));
                    instruction & !0x003f_fc00 == 0xfd40_0200
                }),
                "Linux ARM64 float-literal entry missing terminal d0 scratch load"
            );
        }
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn scalar_float_entry_result_uses_native_vector_registers_on_linux() {
    let canary = pass_canary(fixture_roster::TARGETS_EFI_FLOAT_RESULT_ENTRY);
    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-{target}-float-entry-result-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let src_dir = scratch.join("src");
        let out_dir = scratch.join("out");
        fs::create_dir_all(&src_dir).expect("scratch source directory");
        fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
        write_cross_target_application_build(&src_dir);
        compile(CanaryCompileSpec {
            root_path: src_dir.join("main.omg"),
            build_dir: Some(out_dir.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .expect("scalar-float arithmetic entry result should cross-compile");

        let image = fs::read(out_dir.join("omega-program")).expect("read emitted ELF");
        if target == "linux_x86_64" {
            for (name, opcode) in [
                ("incoming XMM0 store", [0xf2, 0x41, 0x0f, 0x11, 0x87]),
                ("terminal XMM0 load", [0xf2, 0x41, 0x0f, 0x10, 0x87]),
            ] {
                assert!(
                    image.windows(opcode.len()).any(|window| window == opcode),
                    "Linux x64 scalar-float entry missing {name}"
                );
            }
        } else {
            assert!(
                image
                    .windows(4)
                    .any(|window| window == 0xfd00_0200u32.to_le_bytes()),
                "Linux ARM64 scalar-float entry missing incoming d0 store"
            );
            assert!(
                image.windows(4).any(|window| {
                    let instruction = u32::from_le_bytes(window.try_into().expect("word"));
                    instruction & !0x003f_fc00 == 0xfd40_0200
                }),
                "Linux ARM64 scalar-float entry missing terminal d0 scratch load"
            );
        }
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn constant_u64_entry_result_uses_the_declared_native_width() {
    let canary = pass_canary(fixture_roster::TARGETS_EFI_U64_CONSTANT_RESULT_ENTRY);
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-u64-result-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("EFI u64 constant result should compile");
    let image = fs::read(build_dir.join("omega-program.exe")).expect("read emitted PE");
    assert!(
        image
            .windows(10)
            .any(|window| window == [0x48, 0xb8, 7, 0, 0, 0, 0, 0, 0, 0]),
        "u64 constant terminal must write the full RAX register"
    );
    let _ = fs::remove_dir_all(&build_dir);

    let scratch = std::env::temp_dir().join(format!(
        "omega-linux-arm64-u64-result-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    write_cross_target_application_build(&src_dir);
    compile(CanaryCompileSpec {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("Linux ARM64 u64 constant result should compile");
    let image = fs::read(out_dir.join("omega-program")).expect("read emitted ELF");
    assert!(
        image
            .windows(4)
            .any(|window| window == 0xd280_00e0u32.to_le_bytes()),
        "u64 constant terminal must write the full X0 register"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn efi_small_aggregate_entry_uses_rcx_and_rax() {
    let canary = pass_canary(fixture_roster::TARGETS_EFI_SMALL_AGGREGATE_ENTRY);
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-small-record-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("Microsoft x64 direct record entry should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    assert!(
        image
            .windows(17)
            .any(|window| { window[0..2] == [0x49, 0xbf] && window[10..13] == [0x49, 0x89, 0x8f] }),
        "expected the incoming eight-byte record stored from rcx"
    );
    assert!(
        image
            .windows(17)
            .any(|window| { window[0..2] == [0x49, 0xbf] && window[10..13] == [0x49, 0x8b, 0x87] }),
        "expected the terminal eight-byte record loaded into rax"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_large_result_entry_saves_rcx_shifts_argument_and_returns_pointer() {
    let canary = pass_canary(fixture_roster::TARGETS_EFI_LARGE_RESULT_ENTRY);
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-large-result-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("Microsoft x64 indirect-result entry should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    assert!(
        image.windows(34).any(|window| {
            window[10..13] == [0x49, 0x89, 0x8f] && window[27..30] == [0x49, 0x89, 0x97]
        }),
        "expected hidden rcx capture before the shifted declared rdx parameter"
    );
    assert!(
        image.windows(3).any(|window| window == [0x4d, 0x8b, 0xbf]),
        "expected terminal record copy through the saved result pointer"
    );
    assert!(
        image.windows(3).any(|window| window == [0x49, 0x8b, 0x87]),
        "expected the saved result pointer returned in rax"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_large_aggregate_entry_copies_from_rdx_pointer() {
    let canary = pass_canary(fixture_roster::TARGETS_EFI_LARGE_AGGREGATE_ENTRY);
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-large-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("Microsoft x64 register-indirect entry record should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    assert!(
        image
            .windows(5)
            .any(|window| window == [0x4c, 0x8b, 0xda, 0x49, 0xbf]),
        "expected the RDX record pointer preserved in r11 before frame materialization"
    );
    for source_offset in [0u32, 8, 16] {
        let mut load = vec![0x4d, 0x8b, 0x93];
        load.extend(source_offset.to_le_bytes());
        assert!(
            image.windows(load.len()).any(|window| window == load),
            "expected record fragment load through r11+{source_offset}"
        );
    }
    assert!(
        image.windows(3).any(|window| window == [0x4d, 0x89, 0x87]),
        "expected the following scalar stored from positional R8"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_large_aggregate_stack_entry_loads_pointer_after_shadow_space() {
    let canary = pass_canary(fixture_roster::TARGETS_EFI_LARGE_AGGREGATE_STACK_ENTRY);
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-large-stack-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("Microsoft x64 stack-indirect entry record should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    assert!(
        image
            .windows(10)
            .any(|window| { window == [0x4c, 0x8b, 0x9c, 0x24, 120, 0, 0, 0, 0x49, 0xbf] }),
        "expected fifth-slot pointer loaded after saved frame, return address, and shadow space"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_fifth_entry_argument_unmarshals_from_the_ms_x64_stack_area() {
    let canary = pass_canary(fixture_roster::TARGETS_EFI_STACK_ENTRY_ARGUMENT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-stack-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("EFI stack entry-argument canary should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    let e_lfanew = u32::from_le_bytes(image[0x3c..0x40].try_into().unwrap()) as usize;
    let optional_size =
        u16::from_le_bytes(image[e_lfanew + 20..e_lfanew + 22].try_into().unwrap()) as usize;
    let section_count = u16::from_le_bytes(image[e_lfanew + 6..e_lfanew + 8].try_into().unwrap());
    let sections = e_lfanew + 4 + 20 + optional_size;
    let text_raw = (0..section_count as usize)
        .map(|index| sections + index * 40)
        .find(|offset| &image[*offset..*offset + 6] == b".text\0")
        .map(|offset| {
            u32::from_le_bytes(image[offset + 20..offset + 24].try_into().unwrap()) as usize
        })
        .expect(".text section should exist");

    let prologue = [
        0x53, 0x55, 0x56, 0x57, 0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57,
    ];
    assert_eq!(&image[text_raw..text_raw + prologue.len()], &prologue);

    // Four 17-byte register stores precede the fifth parameter's 25-byte stack
    // copy. Locate the exact copy shape rather than baking in the independently
    // evolving fixed-prologue width. Its source displacement is saved frame
    // (80, including MXCSR control-state preservation) + return address (8) +
    // shadow space (32).
    let stack_copy = image[text_raw..]
        .windows(25)
        .find(|window| {
            window[0..2] == [0x49, 0xbf]
                && window[10..18] == [0x4c, 0x8b, 0x94, 0x24, 120, 0, 0, 0]
                && window[18..21] == [0x4d, 0x89, 0x97]
        })
        .expect("fifth stack-argument copy should be present");
    assert_eq!(&stack_copy[0..2], &[0x49, 0xbf], "mov r15, frame base");
    assert_eq!(
        &stack_copy[10..18],
        &[0x4c, 0x8b, 0x94, 0x24, 120, 0, 0, 0],
        "mov r10, [rsp + saved-frame + return-address + shadow-space]"
    );
    assert_eq!(
        &stack_copy[18..25],
        &[0x4d, 0x89, 0x97, 32, 0, 0, 0],
        "mov [r15 + fifth-slot], r10"
    );

    let _ = fs::remove_dir_all(&build_dir);
}
