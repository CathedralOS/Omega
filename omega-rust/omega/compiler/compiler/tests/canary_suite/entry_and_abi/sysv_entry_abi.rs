use super::fixture_roster;
use crate::{CanaryCompileProduct, CanaryCompileSpec, compile, fs, pass_canary};

#[test]
fn sysv_small_aggregate_entry_spreads_consecutive_gprs() {
    let canary = pass_canary(fixture_roster::TARGETS_SYSV_SMALL_AGGREGATE_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-small-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("SysV two-eightbyte record should cross-compile through rsi/rdx");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(30).any(|window| {
            window[10..13] == [0x49, 0x89, 0xb7] && window[27..30] == [0x49, 0x89, 0x97]
        }),
        "expected consecutive runtime-frame stores from rsi and rdx"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_erased_small_aggregate_entry_spreads_only_relevant_fields() {
    let canary = pass_canary(fixture_roster::TARGETS_SYSV_ERASED_SMALL_AGGREGATE_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-erased-small-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("erased-stripped SysV record should cross-compile through rsi/rdx");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(30).any(|window| {
            window[10..13] == [0x49, 0x89, 0xb7] && window[27..30] == [0x49, 0x89, 0x97]
        }),
        "erased evidence must not interrupt consecutive rsi/rdx frame stores"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_hfa_entry_argument_packs_eightbytes_into_xmm_registers() {
    let canary = pass_canary(fixture_roster::TARGETS_SYSV_HFA_ENTRY_ARGUMENT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-sysv-hfa-entry-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("SysV f64 pair entry should cross-compile through xmm0/xmm1");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(38).any(|window| {
            window[10..15] == [0xf2, 0x41, 0x0f, 0x11, 0x87]
                && window[29..34] == [0xf2, 0x41, 0x0f, 0x11, 0x8f]
        }),
        "expected packed entry stores from xmm0 and xmm1"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_mixed_aggregate_entry_uses_independent_register_banks() {
    let canary = pass_canary(fixture_roster::TARGETS_SYSV_MIXED_AGGREGATE_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-mixed-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("SysV INTEGER/SSE entry record should cross-compile through rsi/xmm0");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(55).any(|window| {
            window[10..13] == [0x49, 0x89, 0xb7]
                && window[27..32] == [0xf2, 0x41, 0x0f, 0x11, 0x87]
                && window[46..51] == [0xf2, 0x41, 0x0f, 0x11, 0x8f]
        }),
        "expected mixed entry stores from rsi/xmm0 followed by scalar xmm1"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_mixed_aggregate_entry_rolls_wholly_to_stack() {
    let canary = pass_canary(fixture_roster::TARGETS_SYSV_MIXED_AGGREGATE_STACK_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-mixed-aggregate-stack-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("register-exhausted SysV mixed record should cross-compile wholly from the stack");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    let first_load = [0x4c, 0x8b, 0x94, 0x24, 72, 0, 0, 0];
    let second_load = [0x4c, 0x8b, 0x94, 0x24, 80, 0, 0, 0];
    assert!(
        image
            .windows(43)
            .any(|window| { window[10..18] == first_load && window[35..43] == second_load }),
        "expected both mixed-record fragments loaded from the incoming stack"
    );
    assert!(
        image
            .windows(5)
            .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x87]),
        "expected the trailing float to retain xmm0 after aggregate rollback"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_small_aggregate_entry_rolls_wholly_to_stack() {
    let canary = pass_canary(fixture_roster::TARGETS_SYSV_SMALL_AGGREGATE_STACK_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-small-aggregate-stack-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("register-exhausted SysV record should cross-compile wholly from the stack");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    let first_load = [0x4c, 0x8b, 0x94, 0x24, 72, 0, 0, 0];
    let second_load = [0x4c, 0x8b, 0x94, 0x24, 80, 0, 0, 0];
    assert!(
        image
            .windows(43)
            .any(|window| { window[10..18] == first_load && window[35..43] == second_load }),
        "expected both aggregate fragments loaded from the incoming stack"
    );
    assert!(
        image.windows(3).any(|window| window == [0x4d, 0x89, 0x8f]),
        "expected the trailing scalar to retain the rolled-back r9 slot"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_large_aggregate_entry_copies_the_memory_class_stack_value() {
    let canary = pass_canary(fixture_roster::TARGETS_SYSV_LARGE_AGGREGATE_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-large-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("SysV MEMORY-class entry record should copy from the incoming stack");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    for source_offset in [72u8, 80, 88] {
        let load = [0x4c, 0x8b, 0x94, 0x24, source_offset, 0, 0, 0];
        assert!(
            image.windows(load.len()).any(|window| window == load),
            "expected MEMORY-class fragment load from incoming rsp+{source_offset}"
        );
    }
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_wide_aggregate_entry_uses_general_memory_classification() {
    let canary = pass_canary(fixture_roster::TARGETS_SYSV_WIDE_AGGREGATE_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-wide-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("SysV record beyond 32 bytes should use general MEMORY stack passing");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    for source_offset in [72u8, 80, 88, 96, 104] {
        let load = [0x4c, 0x8b, 0x94, 0x24, source_offset, 0, 0, 0];
        assert!(
            image.windows(load.len()).any(|window| window == load),
            "expected wide MEMORY-class fragment load from incoming rsp+{source_offset}"
        );
    }
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_large_result_entry_saves_and_uses_the_hidden_pointer() {
    let canary = pass_canary(fixture_roster::TARGETS_SYSV_LARGE_RESULT_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-large-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("SysV MEMORY-result entry should preserve and populate its hidden pointer");

    let footprint_artifact = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
        .expect("SysV indirect-result footprint evidence should be written");
    assert!(
        footprint_artifact.contains("\"boundary_contract_fingerprint\": \"0x")
            && footprint_artifact.contains("\"origin\": \"call_return_mechanics\"")
            && footprint_artifact.contains("\"origin\": \"exit_indirect_result_copy\"")
            && footprint_artifact.contains("\"enumeration_complete\": false"),
        "SysV hidden-result copy must bind evidence to its contract without claiming final completeness"
    );

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(34).any(|window| {
            window[10..13] == [0x49, 0x89, 0xbf] && window[27..30] == [0x49, 0x89, 0xb7]
        }),
        "expected hidden rdi capture before the declared rsi parameter store"
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
fn sysv_large_hfa_result_entry_remains_memory_class() {
    let canary = pass_canary(fixture_roster::TARGETS_SYSV_LARGE_HFA_RESULT_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-large-hfa-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("SysV HFA above 16 bytes should remain a MEMORY-class entry result");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(3).any(|window| window == [0x49, 0x89, 0xbf]),
        "expected incoming rdi hidden-result pointer capture"
    );
    assert!(
        image.windows(3).any(|window| window == [0x4d, 0x8b, 0xbf]),
        "expected the 24-byte terminal copied through the saved pointer"
    );
    assert!(
        image.windows(3).any(|window| window == [0x49, 0x8b, 0x87]),
        "expected the saved result pointer returned in rax"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_small_result_entry_loads_rax_and_rdx() {
    let canary = pass_canary(fixture_roster::TARGETS_SYSV_SMALL_RESULT_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-small-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("SysV INTEGER/INTEGER entry result should load rax/rdx");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(34).any(|window| {
            window[10..13] == [0x49, 0x8b, 0x87] && window[27..30] == [0x49, 0x8b, 0x97]
        }),
        "expected terminal record fragments loaded into rax and rdx"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_hfa_result_entry_loads_xmm0_and_xmm1() {
    let canary = pass_canary(fixture_roster::TARGETS_SYSV_HFA_RESULT_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-hfa-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("SysV SSE/SSE entry result should load xmm0/xmm1");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(38).any(|window| {
            window[10..15] == [0xf2, 0x41, 0x0f, 0x10, 0x87]
                && window[29..34] == [0xf2, 0x41, 0x0f, 0x10, 0x8f]
        }),
        "expected terminal record fragments loaded into xmm0 and xmm1"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_mixed_result_entry_loads_rax_and_xmm0() {
    let canary = pass_canary(fixture_roster::TARGETS_SYSV_MIXED_RESULT_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-mixed-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("SysV INTEGER/SSE entry result should load rax/xmm0");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(36).any(|window| {
            window[10..13] == [0x49, 0x8b, 0x87] && window[27..32] == [0xf2, 0x41, 0x0f, 0x10, 0x87]
        }),
        "expected terminal record fragments loaded into rax and xmm0"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_wrapped_float_entry_uses_xmm0_in_both_directions() {
    let canary = pass_canary(fixture_roster::TARGETS_SYSV_WRAPPED_FLOAT_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-wrapped-float-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("one-eightbyte nested SSE record should use xmm0 for entry and result");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image
            .windows(5)
            .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x87]),
        "expected incoming wrapped f64 stored from xmm0"
    );
    assert!(
        image
            .windows(5)
            .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x87]),
        "expected terminal wrapped f64 loaded into xmm0"
    );
    let _ = fs::remove_dir_all(&build_dir);
}
