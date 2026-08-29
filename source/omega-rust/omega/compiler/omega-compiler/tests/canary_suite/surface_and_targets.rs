use super::*;

#[test]
fn free_machine_named_transition_is_rejected_as_a_nonlocal_jump() {
    let canary = fail_canary("calls/free_machine_named_transition_rejected");
    let diagnostics = compile_canary_without_output_for_target(&canary, "macos_arm64")
        .expect_err("a named transition must not enter another free machine");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let expected = fs::read_to_string(canary.join("expected.txt"))
        .expect("foreign named-transition canary should pin its diagnostic");
    assert!(
        combined.contains(expected.trim()),
        "expected the unsupported-transition diagnostic, got:\n{combined}"
    );
    assert!(
        !combined.contains("exact local target state is missing"),
        "a foreign free-machine target must not enter local-state ownership lookup:\n{combined}"
    );
}

#[test]
fn retired_domain_when_surface_is_absent_from_authored_corpus() {
    let root = repo_root();
    let tracked = Command::new("git")
        .args([
            "-C",
            root.to_str().expect("UTF-8 repository path"),
            "ls-files",
            "-z",
        ])
        .output()
        .expect("list tracked corpus files");
    assert!(tracked.status.success(), "git ls-files should succeed");

    let retired_keyword = "when";
    let retired_role = "classifier";
    let retired_name_fragments = [
        ["domain", retired_role].join("_"),
        ["machine", retired_role].join("_"),
        [retired_keyword, retired_role].join("_"),
        ["const", "domain", retired_role].join("_"),
    ];
    let syntax_exceptions = [
        "tests/omega/fail/domains/domain_when_clause_retired/main.omg",
        "wiki/language_guide/chapter_8_domains.md",
    ];
    let mut violations = Vec::new();

    for relative in String::from_utf8_lossy(&tracked.stdout).split('\0') {
        if relative.is_empty() || !root.join(relative).is_file() {
            continue;
        }
        if retired_name_fragments
            .iter()
            .any(|fragment| relative.contains(fragment))
        {
            violations.push(format!("retired vocabulary in path `{relative}`"));
        }

        let extension = Path::new(relative)
            .extension()
            .and_then(|extension| extension.to_str());
        if !matches!(extension, Some("omg" | "rs" | "md" | "txt")) {
            continue;
        }
        let Ok(source) = fs::read_to_string(root.join(relative)) else {
            continue;
        };
        for (index, line) in source.lines().enumerate() {
            let lower = line.to_ascii_lowercase();
            let old_declaration = lower.trim_start().starts_with("domain ")
                && lower.contains(&format!(" {retired_keyword} "));
            let old_multiline_clause = extension == Some("omg")
                && lower
                    .trim_start()
                    .starts_with(&format!("{retired_keyword} "));
            let old_prose = lower.contains(retired_keyword) && lower.contains(retired_role);
            let old_identifier = retired_name_fragments
                .iter()
                .any(|fragment| lower.contains(fragment));
            if (old_declaration || old_multiline_clause || old_prose || old_identifier)
                && !syntax_exceptions.contains(&relative)
            {
                violations.push(format!("{}:{}: {}", relative, index + 1, line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "retired domain `when` surface remains:\n{}",
        violations.join("\n")
    );
}

#[test]
fn filesystem_open_flags_are_not_provider_values() {
    let root = repo_root();
    let portable = fs::read_to_string(root.join("source/library/std/filesystem.omg"))
        .expect("read portable filesystem module");
    assert!(
        !portable.contains("FilesystemHost::O_"),
        "portable filesystem code must use semantic OpenOptions, not foreign provider constants"
    );

    for target in ["windows_x64", "macos_arm64", "linux_x64", "linux_arm64"] {
        let relative = format!("source/library/std/targets/{target}/filesystem_impl.omg");
        let source = fs::read_to_string(root.join(&relative))
            .unwrap_or_else(|error| panic!("read {relative}: {error}"));
        assert!(
            source.contains("Filesystem::encode_open_options"),
            "{relative} must keep foreign open flags in checked target-format code"
        );
    }
}

#[test]
fn linux_direct_syscall_wrappers_do_not_read_ambient_errno() {
    let root = repo_root();
    for target in ["linux_x64", "linux_arm64"] {
        let relative = format!("source/library/std/targets/{target}/filesystem_impl.omg");
        let source = fs::read_to_string(root.join(&relative))
            .unwrap_or_else(|error| panic!("read {relative}: {error}"));
        assert!(
            !source.contains("self.host.errno()"),
            "{relative} must decode explicit -errno syscall results, not ambient libc state"
        );
        assert!(
            source.contains("Filesystem::native_error_code_i32")
                && source.contains("Filesystem::native_error_code_i64")
                && source.contains("self.last_error_i32(")
                && source.contains("self.last_error_i64("),
            "{relative} must retain both direct-syscall result widths through target-owned classification"
        );
    }
}

#[test]
fn provides_syntax_is_retired_from_omega_sources() {
    let root = repo_root();
    let tracked = Command::new("git")
        .args([
            "-C",
            root.to_str().expect("UTF-8 repository path"),
            "ls-files",
            "-z",
            "*.omg",
        ])
        .output()
        .expect("list tracked Omega source files");
    assert!(tracked.status.success(), "git ls-files should succeed");

    let mut declarations = Vec::new();

    for relative in String::from_utf8_lossy(&tracked.stdout).split('\0') {
        if relative.is_empty() {
            continue;
        }
        let normalized = relative.replace('\\', "/");
        let path = root.join(relative);
        if !path.is_file() {
            // Permit verification before a staged deletion has been committed;
            // committed CI sees only paths that still exist.
            continue;
        }
        let source =
            fs::read_to_string(path).unwrap_or_else(|error| panic!("read {relative}: {error}"));
        for line in source.lines() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("//")
                && trimmed.contains(" provides ")
                && trimmed.ends_with('{')
            {
                declarations.push(normalized.clone());
            }
        }
    }

    assert!(
        declarations.is_empty(),
        "authored `provides` syntax is retired; declarations remain:\n{}",
        declarations.join("\n")
    );
}

#[test]
fn effects_reach_syntax_is_retired_from_omega_sources() {
    let root = repo_root();
    let tracked = Command::new("git")
        .args([
            "-C",
            root.to_str().expect("UTF-8 repository path"),
            "ls-files",
            "-z",
            "*.omg",
        ])
        .output()
        .expect("list tracked Omega source files");
    assert!(tracked.status.success(), "git ls-files should succeed");

    let mut clauses = Vec::new();
    for relative in String::from_utf8_lossy(&tracked.stdout).split('\0') {
        if relative.is_empty() {
            continue;
        }
        let path = root.join(relative);
        if !path.is_file() {
            continue;
        }
        let source =
            fs::read_to_string(path).unwrap_or_else(|error| panic!("read {relative}: {error}"));
        for (index, line) in source.lines().enumerate() {
            let authored = line.split_once("//").map_or(line, |(authored, _)| authored);
            let has_legacy_token = authored
                .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                .any(|token| token == "effects");
            if has_legacy_token {
                clauses.push(format!("{relative}:{}: {}", index + 1, line.trim()));
            }
        }
    }

    assert!(
        clauses.is_empty(),
        "authored `effects` reach syntax is retired; clauses remain:\n{}",
        clauses.join("\n")
    );
}

#[cfg(windows)]
#[test]
fn windows_x64_cli_mvp_emits_runnable_pe() {
    let sample = sample_project("cli/basics/cli_mvp");
    let main_path = sample.join("main.omg");
    // Build into the in-repo `build/` so the committed/runnable artifact always
    // matches HEAD: a passing suite leaves a fresh exe at samples/cli/basics/cli_mvp/build/.
    // (Regenerated clean each run; NOT deleted afterward, unlike the temp-dir
    // canaries.) Prevents the "run the exe in the folder and see stale garbage"
    // trap.
    let build_dir = sample.join("build");
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("Windows x64 CLI MVP should compile to a PE executable");

    let output = Command::new(build_dir.join("omega-program.exe"))
        .output()
        .expect("Windows x64 PE executable should run");

    assert!(
        output.status.success(),
        "generated PE exited with {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    // The greeting plus the hold-the-window closer (every sample ends with a
    // read_line pause so a double-clicked exe doesn't vanish; a piped stdin
    // hits EOF and returns immediately, which is why this test still runs).
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Hello, Omega.\n[press Enter to close]\n"
    );
}

// A Windows PE that bakes absolute VAs must ship a `.reloc` section and set
// DYNAMICBASE, so the loader can place it at any base (Windows ASLR, UEFI's
// arbitrary base). Asserted on the emitted header: the base-relocation
// directory (index 5) is non-empty and DllCharacteristics carries 0x0040.
// (The RUN tests already prove the .reloc DATA correct -- they execute under
// ASLR, so a wrong entry would crash them.)
#[cfg(windows)]
#[test]
fn windows_pe_ships_base_relocations_and_dynamicbase() {
    let sample = sample_project("cli/basics/cli_mvp");
    let build_dir = std::env::temp_dir().join(format!("omega-reloc-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: sample.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("cli_mvp should compile to a PE");

    let bytes = fs::read(build_dir.join("omega-program.exe")).expect("read emitted PE");
    let lfanew = u32::from_le_bytes([bytes[0x3c], bytes[0x3d], bytes[0x3e], bytes[0x3f]]) as usize;
    let optional = lfanew + 4 + 20;
    let dll_characteristics = u16::from_le_bytes([bytes[optional + 70], bytes[optional + 71]]);
    assert_ne!(
        dll_characteristics & 0x0040,
        0,
        "DYNAMICBASE must be set for a relocatable PE"
    );
    let dir5 = optional + 112 + 5 * 8;
    let reloc_rva = u32::from_le_bytes([
        bytes[dir5],
        bytes[dir5 + 1],
        bytes[dir5 + 2],
        bytes[dir5 + 3],
    ]);
    let reloc_size = u32::from_le_bytes([
        bytes[dir5 + 4],
        bytes[dir5 + 5],
        bytes[dir5 + 6],
        bytes[dir5 + 7],
    ]);
    assert!(
        reloc_rva != 0 && reloc_size != 0,
        "the base-relocation directory must be populated (rva {reloc_rva:#x}, size {reloc_size})"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The `subsystem` word in a target block reaches the PE optional header:
// `subsystem gui` -> 2, no declaration -> console 3. Asserted on the emitted
// bytes (the Subsystem u16 sits at e_lfanew + 4 (sig) + 20 (COFF) + 68).
#[cfg(windows)]
#[test]
fn build_omg_subsystem_reaches_the_pe_header() {
    // The settled build model: image facts come from build.omg's augmenting
    // `build(b: &mut Build)` machine (interpreted at build time), never an
    // in-source config word. Pins: Gui -> PE Subsystem 2; NO build.omg -> the
    // ZII default (Console, 3).
    let read_subsystem = |bytes: &[u8]| -> u16 {
        let lfanew =
            u32::from_le_bytes([bytes[0x3c], bytes[0x3d], bytes[0x3e], bytes[0x3f]]) as usize;
        let at = lfanew + 4 + 20 + 68;
        u16::from_le_bytes([bytes[at], bytes[at + 1]])
    };
    const MAIN: &str = r#"
boundary trait Console {
    machine exit_process(return_code: i32);
}
data Main {
    console: Console;
}
machine Main::main(&mut self) {
    self.console.exit_process(0);
}
"#;
    // The Build/Subsystem vocabulary is TOOLCHAIN-INJECTED (a virtual
    // prelude source) -- build.omg authors only the machine.
    const BUILD_GUI: &str = r#"
machine build(builder: &mut Build) {
    builder.application("build-subsystem-gui");
    builder.subsystem = Subsystem::Gui;
}
"#;

    for (build_omg, expected) in [(Some(BUILD_GUI), 2u16), (None, 3u16)] {
        let dir = std::env::temp_dir().join(format!(
            "omega-buildomg-{}-{}",
            expected,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create build.omg canary dir");
        fs::write(dir.join("main.omg"), MAIN).expect("write main.omg");
        if let Some(build_source) = build_omg {
            fs::write(dir.join("build.omg"), build_source).expect("write build.omg");
        }

        compile(CanaryCompileSpec {
            root_path: dir.join("main.omg"),
            build_dir: Some(dir.join("build")),
            target_name: None,
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .expect("build.omg canary should compile");

        let bytes = fs::read(dir.join("build").join("omega-program.exe")).expect("read emitted PE");
        assert_eq!(
            read_subsystem(&bytes),
            expected,
            "PE Subsystem with build.omg present={}",
            build_omg.is_some()
        );
        let _ = fs::remove_dir_all(&dir);
    }
}

#[test]
fn build_static_machine_selection_reaches_pe_subsystem() {
    let canary = pass_canary("build/static_machine_parameter_config_compile");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-build-static-machine-selection-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("static machine selection in build.omg should compile to PE");

    let bytes = fs::read(build_dir.join("omega-program.exe")).expect("read emitted PE");
    let lfanew = u32::from_le_bytes([bytes[0x3c], bytes[0x3d], bytes[0x3e], bytes[0x3f]]) as usize;
    let subsystem_at = lfanew + 4 + 20 + 68;
    assert_eq!(
        u16::from_le_bytes([bytes[subsystem_at], bytes[subsystem_at + 1]]),
        70,
        "the selected helper's build-time result must reach PE metadata"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// Cross-compile cli_mvp to a linux_x64 ELF and verify its structure + syscall
// sequences. No execution (the suite host is Windows): the x86_64 Linux System V
// syscall host-call path + ELF emission are validated by the emitted bytes. Guards
// the genericized host-call pipeline (x86_64 now has both win32-import and
// linux-syscall host calls).
/// The float-comparison and text-`!=` x86_64 encoder arms were implemented
/// and byte-reviewed from an arm64 host, where the suite cannot RUN x86
/// output. Pin the compile level: both canaries must keep cross-compiling to
/// a linux_x64 ELF, so an x86-side selection/emission refusal cannot hide
/// behind this host's aarch64-only runtime. (Their runtime behavior is
/// pinned natively on aarch64 + the interpreter by their own suite tests.)
#[test]
fn linux_x64_recent_encoder_canaries_compile() {
    for canary_name in [
        "arithmetic/runtime_float_compare_bool_exit",
        "text/runtime_text_not_equals_exit",
    ] {
        let canary = pass_canary(canary_name);
        let scratch = std::env::temp_dir().join(format!(
            "omega-x64-enc-{}-{}",
            canary_name.replace('/', "-"),
            std::process::id()
        ));
        compile_single_file_hosted_main(&canary, &scratch, "linux_x64").unwrap_or_else(|error| {
            panic!("{canary_name} should cross-compile for linux_x64: {error:?}")
        });
        let elf =
            fs::read(scratch.join("out").join("omega-program")).expect("linux_x64 ELF emitted");
        assert_eq!(&elf[0..4], b"\x7fELF", "ELF magic for {canary_name}");
        let _ = fs::remove_dir_all(&scratch);
    }
}

/// Pin x86_64's explicit F8 count policies from a non-x86 host: Wrapping masks
/// the count before the shift, and Trapping compares the full count and aborts.
#[test]
fn linux_x64_wrapping_shift_masked_count_bytes() {
    // F8b (ch5 shift-count ruling): a Wrapping shift MASKS its count to the
    // operand width. On x86 the hardware `shl` masks mod 32/64 already, so
    // the retired modular-value zero CLAMP must be GONE and the plain
    // width-correct shl present; sub-word operands carry the explicit
    // `and r11d, 7/15`.
    let canary = pass_canary("arithmetic/runtime_shift_count_domain_exit");
    let scratch = std::env::temp_dir().join(format!("omega-x64-shlclamp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let count_domain = scratch.join("count-domain");
    compile_single_file_hosted_main(&canary, &count_domain, "linux_x64")
        .expect("at-width shift canary should cross-compile for linux_x64");
    let elf = fs::read(count_domain.join("out/omega-program")).expect("linux_x64 ELF emitted");
    // The plain width-correct shl (mov ecx, r11d + shl r10d, cl) runs the
    // hardware mask; the retired zero clamp must not follow anywhere.
    let shl32 = [0x44, 0x89, 0xd9, 0x41, 0xd3, 0xe2];
    assert!(
        elf.windows(shl32.len()).any(|window| window == shl32),
        "the width-correct 32-bit shl must be emitted"
    );
    for width_bits in [32u8, 64] {
        let clamp = [
            0x31, 0xc0, 0x49, 0x83, 0xfb, width_bits, 0x4c, 0x0f, 0x43, 0xd0,
        ];
        assert!(
            !elf.windows(clamp.len()).any(|window| window == clamp),
            "the RETIRED Wrapping shl zero clamp at width {width_bits} must be gone (F8b)"
        );
    }

    // The OPERAND-POSITION arm (the at-width canary nests `b << c` under an
    // add): the plain node-width shl followed DIRECTLY by the node-width
    // extension (mov r10d, r10d for unsigned width 4) -- no clamp between.
    let atwidth = pass_canary("arithmetic/runtime_shift_atwidth_signed_modular_exit");
    let atwidth_case = scratch.join("atwidth");
    compile_single_file_hosted_main(&atwidth, &atwidth_case, "linux_x64")
        .expect("at-width modular canary should cross-compile for linux_x64");
    let elf2 = fs::read(atwidth_case.join("out/omega-program")).expect("linux_x64 ELF emitted");
    let operand_shl_then_extend = [0x44, 0x89, 0xd9, 0x41, 0xd3, 0xe2, 0x45, 0x89, 0xd2];
    assert!(
        elf2.windows(operand_shl_then_extend.len())
            .any(|window| window == operand_shl_then_extend),
        "the operand-position masked shl + node-width extension must be emitted"
    );

    // SUB-WORD masked counts: the explicit AND (and r11d, 7 / 15) before the
    // width-correct shift (the new subword canary exercises u8 + i16).
    let subword = pass_canary("arithmetic/runtime_shift_subword_masked_count_exit");
    let subword_case = scratch.join("subword");
    compile_single_file_hosted_main(&subword, &subword_case, "linux_x64")
        .expect("sub-word masked-count canary should cross-compile for linux_x64");
    let elf3 = fs::read(subword_case.join("out/omega-program")).expect("linux_x64 ELF emitted");
    for mask in [7u8, 15] {
        let and_mask = [0x41, 0x83, 0xe3, mask];
        assert!(
            elf3.windows(and_mask.len())
                .any(|window| window == and_mask),
            "the sub-word count mask (and r11d, {mask}) must be emitted"
        );
    }

    // Saturating `<<` (the promoted slice-C canary): the count cap
    // (mov eax,#w + cmp r11,#w + cmovae r11,rax -- the COUNT register)
    // followed by the 64-bit shl and the u8 cmova clamp tail.
    let shl_sat = pass_canary("arithmetic/runtime_shl_saturating_exit");
    let shl_sat_case = scratch.join("shl-saturating");
    compile_single_file_hosted_main(&shl_sat, &shl_sat_case, "linux_x64")
        .expect("saturating shl canary should cross-compile for linux_x64");
    let elf_sat = fs::read(shl_sat_case.join("out/omega-program")).expect("linux_x64 ELF emitted");
    let sat_cap_shl_clamp = [
        0xb8, 8, 0, 0, 0, // mov eax, 8 (u8 width)
        0x49, 0x83, 0xfb, 8, // cmp r11, 8
        0x4c, 0x0f, 0x43, 0xd8, // cmovae r11, rax (cap the count)
        0x4c, 0x89, 0xd9, // mov rcx, r11
        0x49, 0xd3, 0xe2, // shl r10, cl (64-bit exact)
        0x49, 0xbb, 255, 0, 0, 0, 0, 0, 0, 0, // mov r11, 255
        0x4d, 0x39, 0xda, // cmp r10, r11
        0x4d, 0x0f, 0x47, 0xd3, // cmova r10, r11
    ];
    assert!(
        elf_sat
            .windows(sat_cap_shl_clamp.len())
            .any(|window| window == sat_cap_shl_clamp),
        "the saturating shl cap + shift + clamp sequence must be emitted"
    );

    // Arithmetic `>>` (the shr at-width canary, WRAPPING): F8b masks the
    // count -- the plain mov-ecx + sar runs the hardware mask at both
    // widths, and the RETIRED count SATURATION (mov eax,width-1 + cmp +
    // cmovae into the count register) must be gone.
    let shr = pass_canary("arithmetic/runtime_shift_right_atwidth_exit");
    let shr_case = scratch.join("shift-right-atwidth");
    compile_single_file_hosted_main(&shr, &shr_case, "linux_x64")
        .expect("at-width shr canary should cross-compile for linux_x64");
    let elf_shr = fs::read(shr_case.join("out/omega-program")).expect("linux_x64 ELF emitted");
    let sar_32 = [0x44, 0x89, 0xd9, 0x41, 0xd3, 0xfa]; // mov ecx, r11d + sar r10d, cl
    let sar_64 = [0x4c, 0x89, 0xd9, 0x49, 0xd3, 0xfa]; // mov rcx, r11 + sar r10, cl
    for (name, sequence) in [("32-bit", &sar_32[..]), ("64-bit", &sar_64[..])] {
        assert!(
            elf_shr
                .windows(sequence.len())
                .any(|window| window == sequence),
            "the {name} width-correct sar must be emitted"
        );
    }
    for width_bits in [32u8, 64] {
        let saturate = [
            0xb8,
            width_bits - 1,
            0,
            0,
            0,
            0x49,
            0x83,
            0xfb,
            width_bits,
            0x4c,
            0x0f,
            0x43,
            0xd8,
        ];
        assert!(
            !elf_shr
                .windows(saturate.len())
                .any(|window| window == saturate),
            "the RETIRED Wrapping >> count saturation at width {width_bits} must be gone (F8b)"
        );
    }

    let trapping = pass_canary("arithmetic/runtime_trapping_shift_count_exit");
    let trapping_case = scratch.join("trapping");
    compile_single_file_hosted_main(&trapping, &trapping_case, "linux_x64")
        .expect("trapping shift canary should cross-compile for linux_x64");
    let elf4 = fs::read(trapping_case.join("out/omega-program")).expect("linux_x64 ELF emitted");
    let guard = [0x49, 0x83, 0xfb, 32, 0x72, 0x02, 0x0f, 0x0b];
    assert!(
        elf4.windows(guard.len()).any(|window| window == guard),
        "the Trapping full-count comparison and ud2 path must be emitted"
    );

    // The MIN-idiom sat-subtract (the promoted canary): left (a convert of
    // 0) extends, the wide immediate right does NOT, one exact 64-bit sub,
    // then the signed upper bound of the shared tail.
    let min_idiom = pass_canary("arithmetic/runtime_sat_min_idiom_exit");
    let min_idiom_case = scratch.join("sat-min-idiom");
    compile_single_file_hosted_main(&min_idiom, &min_idiom_case, "linux_x64")
        .expect("MIN idiom canary should cross-compile for linux_x64");
    let elf_min =
        fs::read(min_idiom_case.join("out/omega-program")).expect("linux_x64 ELF emitted");
    let min_idiom_sub = [
        0x4d, 0x63, 0xd2, // movsxd r10, r10d (left extends; right immediate skipped)
        0x4d, 0x29, 0xda, // sub r10, r11 (exact 64-bit)
        0x49, 0xbb, 0xff, 0xff, 0xff, 0x7f, 0, 0, 0, 0, // mov r11, i32::MAX
        0x4d, 0x39, 0xda, // cmp r10, r11
        0x4d, 0x0f, 0x4f, 0xd3, // cmovg r10, r11
    ];
    assert!(
        elf_min
            .windows(min_idiom_sub.len())
            .any(|window| window == min_idiom_sub),
        "the MIN-idiom wide-sub + immediate-skip sequence must be emitted"
    );

    // The wire decode-boundary utf8 validator (the promoted refusal canary):
    // the walk's pointer/end setup (mov rcx,r15 / mov r11,r15 / add r11,rax)
    // followed by the loop-head compare and the lead load.
    let utf8_canary = pass_canary("wire/runtime_wire_utf8_invalid_refused_exit");
    let utf8_case = scratch.join("wire-utf8-refusal");
    compile_single_file_hosted_main(&utf8_canary, &utf8_case, "linux_x64")
        .expect("utf8 refusal canary should cross-compile for linux_x64");
    let elf_utf8 = fs::read(utf8_case.join("out/omega-program")).expect("linux_x64 ELF emitted");
    let validator_head = [
        0x4c, 0x89, 0xf9, // mov rcx, r15
        0x4d, 0x89, 0xfb, // mov r11, r15
        0x49, 0x01, 0xc3, // add r11, rax
        0x4c, 0x39, 0xd9, // cmp rcx, r11 (loop head)
        0x0f, 0x83, // jae rel32 (to Done)
    ];
    assert!(
        elf_utf8
            .windows(validator_head.len())
            .any(|window| window == validator_head),
        "the utf8 validation walk must be emitted"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn linux_x64_cli_mvp_emits_elf_with_syscalls() {
    let sample = sample_project("cli/basics/cli_mvp");
    let main_path = sample.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-linux-x64-cli-mvp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("linux_x64 cli_mvp should compile to an ELF executable");

    let elf = fs::read(build_dir.join("omega-program")).expect("linux_x64 ELF should be emitted");

    // ELF64 magic + e_machine == EM_X86_64 (62) at offset 18.
    assert_eq!(&elf[0..4], b"\x7fELF", "ELF magic");
    assert_eq!(
        u16::from_le_bytes([elf[18], elf[19]]),
        62,
        "e_machine should be EM_X86_64"
    );
    // The `syscall` instruction (0F 05) and the exit_group syscall number setup
    // (`mov rax, 231`) must be present -- the System V syscall sequence.
    assert!(
        elf.windows(2).any(|w| w == [0x0f, 0x05]),
        "ELF should contain a `syscall` (0F 05) instruction"
    );
    assert!(
        elf.windows(10)
            .any(|w| w == [0x48, 0xb8, 0xe7, 0, 0, 0, 0, 0, 0, 0]),
        "ELF should set rax = 231 (exit_group) for the exit syscall"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn external_leaf_syscall_reaches_linux_x64_backend() {
    let canary = pass_canary("providers/external_leaf_syscall_compile");
    let scratch = std::env::temp_dir().join(format!("omega-via-syscall-{}", std::process::id()));
    let build_dir = scratch.join("out");
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_target_with_auxiliary_artifacts(
        &canary,
        build_dir.clone(),
        "linux_x64",
    )
    .expect("qualified Binding::Syscall leaf should cross-compile for linux_x64");

    let trust = fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("external-leaf syscall trust report should be emitted");
    assert!(
        trust.contains("provider plan: linux_x64::satisfies::RawProcess ["),
        "syscall leaf must travel through target-scoped provider admission reporting:\n{trust}"
    );
    let footprints = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
        .expect("external-leaf syscall footprints should be emitted");
    assert!(
        footprints.contains("\"origin\": \"compiler_body_outbound_syscall_result\""),
        "value-returning syscall leaf must retain its result-store footprint"
    );
    assert!(
        footprints.contains("\"origin\": \"compiler_body_outbound_syscall_storage_arguments\""),
        "runtime-scalar syscall arguments must retain their storage-relocation footprint"
    );
    assert!(
        footprints
            .contains("\"origin\": \"compiler_body_outbound_syscall_result_storage_arguments\""),
        "result-bearing runtime-scalar syscalls must retain argument/result relocations"
    );
    let elf = fs::read(build_dir.join("omega-program"))
        .expect("external-leaf syscall ELF should be emitted");
    let exit_sequence = [
        0x48, 0xb8, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0f, 0x05,
    ];
    assert!(
        elf.windows(exit_sequence.len())
            .any(|window| window == exit_sequence),
        "external leaf must emit `mov rax, 60; syscall`, not a compatibility import"
    );
    let _ = fs::remove_dir_all(&scratch);

    let arm_scratch =
        std::env::temp_dir().join(format!("omega-via-syscall-arm-{}", std::process::id()));
    let arm_out = arm_scratch.join("out");
    let _ = fs::remove_dir_all(&arm_scratch);
    compile_rooted_canary_for_target_with_auxiliary_artifacts(
        &canary,
        arm_out.clone(),
        "linux_arm64",
    )
    .expect("qualified Binding::Syscall leaf should cross-compile for linux_arm64");
    let arm_elf =
        fs::read(arm_out.join("omega-program")).expect("external-leaf arm syscall ELF emitted");
    let arm_footprints = fs::read_to_string(arm_out.join("08_boundary_footprints.json"))
        .expect("external-leaf arm syscall footprints should be emitted");
    assert!(
        arm_footprints.contains("\"origin\": \"compiler_body_outbound_syscall_result\""),
        "AArch64 value-returning syscall leaf must retain its result-store footprint"
    );
    assert!(
        arm_footprints.contains("\"origin\": \"compiler_body_outbound_syscall_storage_arguments\""),
        "AArch64 runtime-scalar syscall arguments must retain their storage-relocation footprint"
    );
    assert!(
        arm_footprints
            .contains("\"origin\": \"compiler_body_outbound_syscall_result_storage_arguments\""),
        "AArch64 result-bearing runtime-scalar syscalls must retain argument/result relocations"
    );
    let arm_exit_sequence = [0xa8, 0x0b, 0x80, 0xd2, 0x01, 0x00, 0x00, 0xd4];
    assert!(
        arm_elf
            .windows(arm_exit_sequence.len())
            .any(|window| window == arm_exit_sequence),
        "external leaf must emit `mov x8, 93; svc #0` on AArch64"
    );
    let _ = fs::remove_dir_all(&arm_scratch);
}

// Atomics end-to-end across architectures. The host (windows_x64) RUNS the
// program (fetch_add + compare_exchange, exit 70). aarch64 cannot execute on
// this box, so the linux_arm64 build is verified by the emitted ELF carrying the
// real, ordering-selected LSE atomic instructions: LDADD (NoOrdering fetch_add)
// and CASAL (ReceivePublish compare_exchange), both returning the instruction-observed
// prior value in the result register.
#[test]
fn atomics_cross_platform_emits_real_atomics() {
    let sample = sample_project("cli/systems/atomics_cross");
    let main_path = sample.join("main.omg");

    // --- windows_x64: compile + run ---
    let win_dir = std::env::temp_dir().join(format!("omega-atomics-win-{}", std::process::id()));
    let _ = fs::remove_dir_all(&win_dir);
    compile_with_auxiliary_artifacts(CanaryCompileSpec {
        root_path: main_path.clone(),
        build_dir: Some(win_dir.clone()),
        target_name: Some("windows_x64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("atomics_cross should compile for windows_x64");
    let win_footprints = fs::read_to_string(win_dir.join("08_boundary_footprints.json"))
        .expect("windows atomics boundary footprints should be emitted");
    assert!(
        win_footprints.contains("\"origin\": \"compiler_body_outbound_immediate_import\""),
        "Windows literal ExitProcess calls must retain their exact direct-import footprint"
    );
    assert!(
        win_footprints.contains("\"origin\": \"compiler_body_atomic_operation\""),
        "Windows atomics must retain their compiler-body atomic footprint"
    );
    let win_regions = fs::read_to_string(win_dir.join("13_executable_regions.json"))
        .expect("windows atomic final executable-region evidence should be emitted");
    assert!(
        win_regions
            .contains("\"certificate_marker\": \"omega.final-footprint-certificate.current\"")
            && win_regions.contains("\"compiler_function_body_specification\""),
        "Windows atomics must reach final-byte validation"
    );
    // Only a windows host can execute the PE; elsewhere the windows_x64 build
    // is compile-verified and the aarch64 ELF instruction checks below carry
    // the semantic weight.
    #[cfg(windows)]
    {
        let output = Command::new(win_dir.join("omega-program.exe"))
            .output()
            .expect("windows_x64 atomics_cross should run");
        assert_eq!(
            output.status.code(),
            Some(70),
            "expected RMW and every legal load/store ordering family to round-trip \
             (exit 70); got {:?}\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let _ = fs::remove_dir_all(&win_dir);

    // --- linux_arm64: cross-emit + disassemble-by-bytes ---
    let arm_dir = std::env::temp_dir().join(format!("omega-atomics-arm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&arm_dir);
    compile_with_auxiliary_artifacts(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(arm_dir.clone()),
        target_name: Some("linux_arm64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("atomics_cross should compile for linux_arm64");
    let arm_footprints = fs::read_to_string(arm_dir.join("08_boundary_footprints.json"))
        .expect("AArch64 atomics boundary footprints should be emitted");
    assert!(
        arm_footprints.contains("\"origin\": \"compiler_body_atomic_operation\""),
        "AArch64 atomics must retain their compiler-body atomic footprint"
    );
    let arm_regions = fs::read_to_string(arm_dir.join("13_executable_regions.json"))
        .expect("AArch64 atomic final executable-region evidence should be emitted");
    assert!(
        arm_regions
            .contains("\"certificate_marker\": \"omega.final-footprint-certificate.current\"")
            && arm_regions.contains("\"compiler_function_body_specification\""),
        "AArch64 atomics must reach final-byte validation"
    );
    let elf = fs::read(arm_dir.join("omega-program")).expect("linux_arm64 ELF should be emitted");

    assert_eq!(
        u16::from_le_bytes([elf[18], elf[19]]),
        183,
        "e_machine should be EM_AARCH64"
    );
    // LDADD w17, w26, [x16] = 0xB831021A (NoOrdering fetch_add; prior in W26).
    assert!(
        elf.windows(4).any(|w| w == [0x1a, 0x02, 0x31, 0xb8]),
        "linux_arm64 ELF should contain LDADD w17,w26,[x16] for NoOrdering fetch_add"
    );
    // CASAL w26, w17, [x16] = 0x88FAFE11 (compare_exchange).
    assert!(
        elf.windows(4).any(|w| w == [0x11, 0xfe, 0xfa, 0x88]),
        "linux_arm64 ELF should contain CASAL w26,w17,[x16] for compare_exchange"
    );
    let _ = fs::remove_dir_all(&arm_dir);
}

// Cross-compile the dungeon crawler (the richest non-visual sample) to a linux_x64
// ELF. Unlike cli_mvp this drives runtime-storage syscall arguments: the room
// descriptions are bounded UTF-8 carriers in statically-allocated state regions, so the
// write(2) syscall marshals their runtime pointer/length into rsi/rdx via the r11/rax
// staging path. No execution (Windows host); validated by the emitted ELF + the
// presence of the runtime-storage load sequence (`mov r11, imm64` then a load).
#[test]
fn linux_x64_dungeon_crawler_emits_elf_with_runtime_storage_syscalls() {
    let sample = sample_project("cli/games/dungeon_crawler_cli");
    let main_path = sample.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-linux-x64-dungeon-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("linux_x64 dungeon crawler should compile to an ELF executable");

    let elf = fs::read(build_dir.join("omega-program")).expect("linux_x64 ELF should be emitted");

    assert_eq!(&elf[0..4], b"\x7fELF", "ELF magic");
    assert_eq!(
        u16::from_le_bytes([elf[18], elf[19]]),
        62,
        "e_machine should be EM_X86_64"
    );
    assert!(
        elf.windows(2).any(|w| w == [0x0f, 0x05]),
        "ELF should contain a `syscall` (0F 05) instruction"
    );
    // Runtime-storage syscall marshalling: `mov r11, imm64` (49 BB, the relocated
    // region base) followed somewhere by `mov rax, [r11 + disp32]` (49 8B 83) and the
    // staging `mov rsi, rax` (48 89 C6). Their presence proves a runtime bounded carrier
    // was marshalled into a syscall argument rather than rejected by the encoder.
    assert!(
        elf.windows(2).any(|w| w == [0x49, 0xbb]),
        "ELF should load a relocated region base into volatile r11 for a runtime-storage syscall arg"
    );
    assert!(
        elf.windows(3).any(|w| w == [0x49, 0x8b, 0x83]),
        "ELF should read a bounded-carrier field into rax (mov rax, [r11+disp32])"
    );
    // The line read (read_line) lowers to a byte-at-a-time read(2) loop, NOT a win32
    // ReadFile import: each iteration reads one byte -- `mov edx,1` (count) + `mov eax,0`
    // (read syscall number) + syscall. Its presence proves stdin input works via the
    // syscall path (a win32-import read would emit a `call rel32`, no read syscall).
    assert!(
        elf.windows(12).any(|w| w
            == [
                0xba, 0x01, 0x00, 0x00, 0x00, 0xb8, 0x00, 0x00, 0x00, 0x00, 0x0f, 0x05
            ]),
        "ELF should set up a read(2) line-read loop (mov edx,1; mov eax,0; syscall)"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[cfg(windows)]
#[test]
fn windows_x64_dungeon_crawler_emits_runnable_pe() {
    let sample = sample_project("cli/games/dungeon_crawler_cli");
    let main_path = sample.join("main.omg");
    // Build into the in-repo `build/` so the runnable artifact always matches HEAD
    // (regenerated clean each run, NOT deleted afterward). This is the durable fix
    // for the stale-artifact trap: `samples/cli/games/dungeon_crawler_cli/build/omega-program.exe`
    // is rewritten by every green suite run.
    let build_dir = sample.join("build");
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("Windows x64 dungeon crawler should compile to a PE executable");

    let mut child = Command::new(build_dir.join("omega-program.exe"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Windows x64 dungeon PE executable should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        // Drive the full non-visual game loop: move into deeper rooms, claim the
        // treasure (inventory mutation), and resolve combat. This exercises
        // movement-at-depth, room events, the item/use path, and the enemy/fight
        // path -- the systems that make the dungeon a real integration marker.
        .write_all(b"north\r\nnorth\r\nuse\r\nnorth\r\nfight\r\nquit\r\n")
        .expect("scripted dungeon input should be written");
    let output = child
        .wait_with_output()
        .expect("Windows x64 dungeon PE executable should finish");

    assert!(
        output.status.success(),
        "generated dungeon PE exited with {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Dungeon Crawler"));
    assert!(stdout.contains("A bottomless dark room near the dungeon heart."));
    // The room event and paths lines are produced by inline-branching calls that
    // run in render's CONTINUATION after the dispatched (looping) find_room call.
    // They must render their own text, not echo the description (regression guard
    // for the leaf-binding + cross-segment frame-slot resolution fix).
    assert!(
        stdout.contains("The room is quiet."),
        "room event line should render, not echo the description\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("[Paths] north"),
        "room paths line should render, not echo the description\nstdout:\n{stdout}"
    );
    // Look-AT-DEPTH: after moving north (R00 -> R01) the deeper room must render its
    // OWN generated data, not blank/stale. find_room is a VALUE-position call to a
    // looping machine; before the value-return-in-dispatch keystone it returned a
    // too-small room_count at the real program's scale so deeper rooms rendered
    // empty. Guards that the keystone fixed the dungeon's "trash on move" bug.
    assert!(
        stdout.contains("== Branch Room =="),
        "deeper room (R01) name should render after moving north\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("A shallow limestone room with fresh claw marks."),
        "deeper room (R01) description should render after moving north\nstdout:\n{stdout}"
    );
    // The item/use path: claiming the treasure cache (deeper room) mutates state
    // and reports it -- a cross-machine inventory write driven from a command.
    assert!(
        stdout.contains("You take the treasure."),
        "the `use` command should claim the treasure in the treasure room\nstdout:\n{stdout}"
    );
    // The combat path: an enemy room resolves a fight to a win + reward.
    assert!(
        stdout.contains("The enemy collapses."),
        "the `fight` command should resolve combat in an enemy room\nstdout:\n{stdout}"
    );
    // Intentionally NOT removing build_dir: leave the fresh, verified artifact in
    // samples/cli/games/dungeon_crawler_cli/build/ so running the in-repo exe matches HEAD.
}

#[test]
fn contract_canary_visualizes_flow_contract_summaries() {
    let canary = pass_canary("domains/contracts_domain_membership_surface");
    let scratch =
        std::env::temp_dir().join(format!("omega-contract-canary-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let source = scratch.join("source");
    let build_dir = scratch.join("out");
    fs::create_dir_all(&source).expect("create exact-entry contract canary source directory");
    fs::copy(canary.join("main.omg"), source.join("main.omg"))
        .expect("copy contract visualization canary");
    fs::write(
        source.join("build.omg"),
        hosted_main_program_entry_build("macos_arm64"),
    )
    .expect("write exact macOS AArch64 ProgramEntry binding");

    let compilation = production_compile(CanaryCompileSpec {
        root_path: source.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("macos_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("exact-root contract canary should compile with visual artifacts");
    assert!(compilation.wrote_output());

    let state_graph = fs::read_to_string(build_dir.join("06_state_graph.html"))
        .expect("state graph visualization should be written");
    let control_flow = fs::read_to_string(build_dir.join("07_control_flow.html"))
        .expect("control flow visualization should be written");
    let checked_trees = fs::read_to_string(build_dir.join("05_checked_trees.html"))
        .expect("checked tree visualization should be written");
    let abstract_operations = fs::read_to_string(build_dir.join("08_abstract_operations.html"))
        .expect("abstract operations visualization should be written");
    let boundary_footprints = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
        .expect("boundary footprint evidence should be written");
    let executable_regions = fs::read_to_string(build_dir.join("13_executable_regions.json"))
        .expect("final executable-region inventory should be written");
    let machine_instructions = fs::read_to_string(build_dir.join("11_machine_instructions.html"))
        .expect("machine instructions visualization should be written");

    assert!(
        state_graph.contains("contract call #1.0 requires 1 ensures 1"),
        "state graph should show propagated contract call summaries"
    );
    assert!(
        control_flow.contains("contract call #1.0 requires 1 ensures 1"),
        "control flow should show propagated contract call summaries"
    );
    assert!(
        checked_trees.contains("call Main::heal::heal"),
        "checked tree visualization should expose checked call nodes"
    );
    assert!(
        checked_trees.contains("contracts: requires 1 ensures 1"),
        "checked call nodes should summarize propagated contract counts"
    );
    assert!(
        checked_trees.contains("borrow: access player: mutable invalidations 1"),
        "checked call nodes should surface borrow access and invalidation detail"
    );
    assert!(
        checked_trees.contains("Main::main::main [checked]"),
        "checked tree visualization should now be a scoped graph view instead of a text report"
    );
    assert!(
        abstract_operations.contains("Main::main::main [0]")
            && abstract_operations.contains("00 EnterFunction @ statement 0"),
        "abstract operations should render backend state blocks with ordered instruction lines"
    );
    assert!(
        boundary_footprints.contains("\"evidence_stage\": \"encoded_machine\"")
            && boundary_footprints.contains("\"boundary_contract_fingerprint\": \"0x")
            && !boundary_footprints.contains("\"boundary_contract_fingerprint\": null")
            && boundary_footprints.contains("\"origin\": \"call_return_mechanics\"")
            && boundary_footprints.contains("\"origin\": \"dispatch_scaffold\"")
            && boundary_footprints.contains("\"enumeration_complete\": false")
            && boundary_footprints.contains("\"composed\"")
            && boundary_footprints.contains("\"fragments\""),
        "boundary footprint artifact should publish post-emission retained evidence bound to its validated contract without claiming final completeness"
    );
    let compiler_entry_region = executable_regions
        .lines()
        .find(|line| line.contains("\"origin\": \"compiler_function\""))
        .expect("final inventory should contain the compiler entry function");
    assert!(
        compiler_entry_region.contains("\"footprint\": {\"fingerprint\": \"0x"),
        "the exact placed compiler entry region must retain its composed encoded-machine footprint:\n{compiler_entry_region}"
    );
    assert!(
        executable_regions.contains(
            "\"certificate_marker\": \"omega.final-footprint-certificate.current\""
        )
            && executable_regions.contains("\"certificate_fingerprint\": \"0x")
            && executable_regions.contains("\"coverage_fingerprint\": \"0x")
            && executable_regions.contains("\"placement_stage\": \"final_image\"")
            && executable_regions.contains("\"origin\": \"compiler_function\"")
            && executable_regions.contains(
                "\"final_byte_validated_classes\": [\"compiler_function_relocation_envelope\", \"compiler_function_call_return_mechanics\""
            )
            && executable_regions.contains("\"enumeration_complete\": true")
            && executable_regions.contains("\"region_enumeration_complete\": true")
            && executable_regions.contains("\"footprint_enumeration_complete\": true")
            && executable_regions.contains(
                "\"absent_by_construction_classes\": [\"relaxation_products\", \"veneers\", \"generated_stubs\", \"admitted_leaves\"]"
            )
            && executable_regions.contains("\"boundary_contract_fingerprint\": \"0x")
            && !executable_regions.contains("\"boundary_contract_fingerprint\": null")
            && executable_regions.contains("\"implementation_evidence_fingerprint\": \"0x")
            && executable_regions.contains("\"compiler_text_validation\": {")
            && executable_regions.contains("\"compiler_function_validation\": {")
            && executable_regions.contains("\"function_count\": 1")
            && executable_regions.contains("\"instruction_count\": ")
            && executable_regions.contains("\"checked_assembly_instruction_count\": ")
            && executable_regions.contains("\"fixed_mechanics_instruction_count\": 2")
            && executable_regions.contains("\"fixed_mechanics_validation_fingerprint\": \"0x")
            && executable_regions.contains("\"fixed_mechanics_boundary_contract_fingerprint\": \"0x")
            && executable_regions.contains("\"fixed_mechanics_footprint_fingerprint\": \"0x")
            && executable_regions.contains("\"body_specification_instruction_count\": ")
            && executable_regions.contains("\"body_specification_validation_fingerprint\": \"0x")
            && executable_regions.contains("\"body_specification_boundary_contract_fingerprint\": \"0x")
            && executable_regions.contains("\"body_specification_footprint_fingerprint\": \"0x")
            && executable_regions.contains("\"composed_footprint_fingerprint\": \"0x")
            && executable_regions.contains("\"compiler_function_body_specification\"")
            && executable_regions.contains("\"compiler_function_instruction_enumeration\"")
            && executable_regions.contains("\"relocation_envelope_fingerprint\": \"0x")
            && executable_regions.contains("\"derivation_fingerprint\": \"0x")
            && executable_regions.contains("\"boundary_placement_binding_fingerprint\": \"0x")
            && executable_regions.contains("\"text_fingerprint\": \"0x")
            && executable_regions.contains("\"byte_fingerprint\": \"0x")
            && executable_regions.contains("\"unclassified_gaps\"")
            && executable_regions.contains("\"missing_classes\": []"),
        "final executable inventory should classify placed compiler code with complete footprint enumeration"
    );
    assert!(
        machine_instructions.contains("Machine Instructions")
            && machine_instructions.contains("DispatchLoopEnter")
            && machine_instructions.contains("Main::main::main [0]")
            && machine_instructions.contains("control:")
            && machine_instructions.contains("terminator:"),
        "machine instruction stage should render block-local instruction listings"
    );

    let _ = fs::remove_dir_all(&scratch);
}
