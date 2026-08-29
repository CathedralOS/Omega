use super::*;

fn assert_native_exit_code(
    report: &CompileReport,
    expected: i32,
    fixture: &str,
    expectation: &str,
) {
    let executable = report
        .checked_native_executable_path()
        .unwrap_or_else(|| panic!("{fixture} lost its exact executable publication receipt"));
    let output = Command::new(executable)
        .output()
        .unwrap_or_else(|error| panic!("{fixture} should run: {error}"));
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{expectation}; expected exit {expected}, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// =============================================================================
// ch17 Atomics (concurrency stage 1) RUN canaries
// =============================================================================

/// M2 -- AtomicU32 load/store round-trip for NoOrdering,
/// Publish-to-Receive, and GlobalOrder. The last pair pins the exact
/// target-specific global-order store realization rather than only parser legality.
#[test]
fn runtime_atomic_load_store_exit_canary_runs() {
    let canary = pass_canary("atomics/runtime_atomic_load_store_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-atomic-load-store-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("atomic load/store canary should compile from its authored root");
    assert_native_exit_code(
        &compilation,
        70,
        "atomic load/store canary",
        "every legal atomic load/store ordering pair should roundtrip",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

/// M3 -- AtomicU32 fetch_add: returns PRIOR value, increments cell.
/// Native lowering uses one RMW instruction and returns that instruction's
/// observed prior. Two successive calls check both returned and stored values.
#[test]
fn runtime_atomic_fetch_add_exit_canary_runs() {
    let canary = pass_canary("atomics/runtime_atomic_fetch_add_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-atomic-fetch-add-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("atomic fetch_add canary should compile from its authored root");
    assert_native_exit_code(
        &compilation,
        70,
        "atomic fetch-add canary",
        "each fetch-add should return the prior value and retain the incremented cell",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_atomic_fetch_sub_exit_canary_runs() {
    let canary = pass_canary("atomics/runtime_atomic_fetch_sub_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-atomic-fetch-sub-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("atomic fetch_sub canary should compile from its authored root");
    assert_native_exit_code(
        &compilation,
        70,
        "atomic fetch-sub canary",
        "fetch-sub should return each prior value and retain wrapping subtraction",
    );
    let _ = fs::remove_dir_all(&build_dir);

    let arm_dir =
        std::env::temp_dir().join(format!("omega-atomic-fetch-sub-arm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&arm_dir);
    compile_rooted_canary_for_target(&canary, arm_dir.clone(), "linux_arm64")
        .expect("atomic fetch_sub canary should cross-compile from its authored root");
    let elf = fs::read(arm_dir.join("omega-program")).expect("arm64 fetch_sub ELF should exist");
    assert!(
        elf.windows(4).any(|word| word == [0xf1, 0x03, 0x11, 0x4b]),
        "arm64 fetch_sub must contain SUB w17,wzr,w17"
    );
    assert!(
        elf.windows(4).any(|word| word == [0x1a, 0x02, 0xf1, 0xb8]),
        "arm64 fetch_sub must contain LDADDAL w17,w26,[x16]"
    );
    let _ = fs::remove_dir_all(&arm_dir);
}

#[test]
fn runtime_atomic_fetch_xor_exit_canary_runs() {
    let canary = pass_canary("atomics/runtime_atomic_fetch_xor_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-atomic-fetch-xor-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("atomic fetch_xor canary should compile from its authored root");
    assert_native_exit_code(
        &compilation,
        70,
        "atomic fetch-xor canary",
        "successive fetch-xor operations should retain their exact prior and stored values",
    );
    let _ = fs::remove_dir_all(&build_dir);

    let arm_dir =
        std::env::temp_dir().join(format!("omega-atomic-fetch-xor-arm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&arm_dir);
    compile_rooted_canary_for_target(&canary, arm_dir.clone(), "linux_arm64")
        .expect("atomic fetch_xor canary should cross-compile from its authored root");
    let elf = fs::read(arm_dir.join("omega-program")).expect("arm64 fetch_xor ELF should exist");
    assert!(
        elf.windows(4).any(|word| word == [0x1a, 0x22, 0xf1, 0xb8]),
        "arm64 fetch_xor must contain LDEORAL w17,w26,[x16]"
    );
    let _ = fs::remove_dir_all(&arm_dir);
}

#[test]
fn runtime_atomic_fetch_or_exit_canary_runs() {
    let canary = pass_canary("atomics/runtime_atomic_fetch_or_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-atomic-fetch-or-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("atomic fetch_or canary should compile from its authored root");
    assert_native_exit_code(
        &compilation,
        75,
        "atomic fetch-or canary",
        "successive fetch-or operations should retain their exact prior and stored values",
    );
    let _ = fs::remove_dir_all(&build_dir);

    let arm_dir =
        std::env::temp_dir().join(format!("omega-atomic-fetch-or-arm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&arm_dir);
    compile_rooted_canary_for_target(&canary, arm_dir.clone(), "linux_arm64")
        .expect("atomic fetch_or canary should cross-compile from its authored root");
    let elf = fs::read(arm_dir.join("omega-program")).expect("arm64 fetch_or ELF should exist");
    assert!(
        elf.windows(4).any(|word| word == [0x1a, 0x32, 0xf1, 0xb8]),
        "arm64 fetch_or must contain LDSETAL w17,w26,[x16]"
    );
    let _ = fs::remove_dir_all(&arm_dir);
}

#[test]
fn runtime_atomic_fetch_and_exit_canary_runs() {
    let canary = pass_canary("atomics/runtime_atomic_fetch_and_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-atomic-fetch-and-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("atomic fetch_and canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("atomic fetch_and canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("atomic fetch_and canary should run");
    assert_eq!(
        output.status.code(),
        Some(80),
        "expected fetch_and 14&11=10 then 10&7=2; got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    let arm_dir =
        std::env::temp_dir().join(format!("omega-atomic-fetch-and-arm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&arm_dir);
    compile_rooted_canary_for_target(&canary, arm_dir.clone(), "linux_arm64")
        .expect("atomic fetch_and canary should cross-compile from its authored root");
    let elf = fs::read(arm_dir.join("omega-program")).expect("arm64 fetch_and ELF should exist");
    assert!(
        elf.windows(8)
            .any(|window| window == [0xf1, 0x03, 0x31, 0x2a, 0x1a, 0x12, 0xf1, 0xb8]),
        "arm64 fetch_and must contain MVN w17,w17 then LDCLRAL w17,w26,[x16]"
    );
    let _ = fs::remove_dir_all(&arm_dir);
}

#[test]
fn runtime_atomic_swap_exit_canary_runs() {
    let canary = pass_canary("atomics/runtime_atomic_swap_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-atomic-swap-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("atomic swap canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("atomic swap canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("atomic swap canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected swap to return 10 and store 42; got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    let arm_dir =
        std::env::temp_dir().join(format!("omega-atomic-swap-arm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&arm_dir);
    compile_rooted_canary_for_target(&canary, arm_dir.clone(), "linux_arm64")
        .expect("atomic swap canary should cross-compile from its authored root");
    let elf = fs::read(arm_dir.join("omega-program")).expect("arm64 swap ELF should exist");
    assert!(
        elf.windows(4).any(|word| word == [0x1a, 0x82, 0xf1, 0xb8]),
        "arm64 swap must contain SWPAL w17,w26,[x16]"
    );
    let _ = fs::remove_dir_all(&arm_dir);
}

/// M4 -- AtomicU32 compare_exchange: returns PRIOR value; swaps only when
/// *place == expected.
/// Success path: CAS(10, 99) when counter==10 → prior==10, counter becomes 99.
/// Failure path: CAS(10, 42) when counter==99 → prior==99, counter stays 99.
#[test]
fn runtime_atomic_compare_exchange_exit_canary_runs() {
    let canary = pass_canary("atomics/runtime_atomic_compare_exchange_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-atomic-cmpxchg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("atomic compare_exchange canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("atomic compare_exchange canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("atomic compare_exchange canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected compare_exchange success(10→99, prior=10) + failure(10 vs 99, prior=99, cell=99) \
         (exit 70); exit 71=bad prior_s, 72=bad after swap, 73=bad prior_f, 74=bad after fail; \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// std Console BYTE OPS natively (the ByteRead ruling: Eof = ordinal 0 = the
// composite's pre-zeroed slot; no sentinel). One compile, two runs: empty
// stdin takes the Eof-first arm (exit 70 -- the differential's leg), piped
// "AB" echoes both bytes and exits 70 + 65 + 66 = 201 (the byte-arrival arm:
// payload delivery, tag-1 write, write_byte pass-through).
#[test]
fn runtime_console_byte_echo_exit_canary_runs() {
    let canary = pass_canary("host/runtime_console_byte_echo_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-console-byte-echo-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("console byte echo canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .stdin(std::process::Stdio::null())
        .output()
        .expect("console byte echo canary should run on empty stdin");
    assert_eq!(
        output.status.code(),
        Some(70),
        "empty stdin must take the Eof arm (the ZII zero slot), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "empty stdin must echo nothing, got {:?}",
        String::from_utf8_lossy(&output.stdout)
    );

    let mut piped = Command::new(build_dir.join(executable_name()))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("console byte echo canary should spawn");
    use std::io::Write;
    piped
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(b"AB")
        .expect("write test bytes");
    let piped = piped.wait_with_output().expect("piped run should finish");
    assert_eq!(
        piped.status.code(),
        Some(201),
        "piped AB must sum 70+65+66, got {:?}\nstderr:\n{}",
        piped.status.code(),
        String::from_utf8_lossy(&piped.stderr)
    );
    assert_eq!(
        piped.stdout,
        b"AB",
        "write_byte must echo the raw bytes, got {:?}",
        String::from_utf8_lossy(&piped.stdout)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// THE FIELD MODEL: an attached `Binding::VtableField(output_string)` leaf
// names the fn-ptr field and the provider type's layout computes its offset (+8 behind the
// leading `reset` field; headers fall out free, no magic slot count). The
// emitted dispatch must be byte-identical to the VtableSlot(1) original:
// `mov rax, [rcx+8]; call rax`. Cross-compiled for uefi_x64, so this pins
// the whole chain (parse -> external binding row -> layout-resolved mechanism ->
// relocation placement -> PE bytes) on EVERY host -- unlike the
// cfg(windows) slot twin.
#[test]
fn efi_vtable_field_call_emits_indirect_dispatch() {
    let canary = pass_canary("targets/efi_vtable_field_call");
    let build_dir = std::env::temp_dir().join(format!("omega-vtable-field-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("vtable field-model canary should cross-compile for uefi_x64");
    let bytes = fs::read(build_dir.join("omega-program.exe")).expect("read emitted PE");
    let needle = [0x48u8, 0x8b, 0x81, 0x08, 0x00, 0x00, 0x00, 0xff, 0xd0];
    assert!(
        bytes.windows(needle.len()).any(|window| window == needle),
        "expected `mov rax, [rcx+8]; call rax` (field-model dispatch at the \
         layout-computed +8) in .text"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// The same authored field model on ELF/x86-64 must use the normalized SysV
// plan: the receiver occupies rdi, and the indirect dispatch reads +8 from it.
// This stays freestanding, so it exercises the full source-to-ELF path without
// depending on the still-missing ELF dynamic-import image support.
#[test]
fn sysv_vtable_field_call_emits_indirect_dispatch() {
    let canary = pass_canary("targets/sysv_vtable_field_call");
    let build_dir =
        std::env::temp_dir().join(format!("omega-sysv-vtable-field-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("vtable field-model canary should cross-compile for linux_x64");
    let bytes = fs::read(build_dir.join("omega-program")).expect("read emitted ELF image");
    let needle = [0x48u8, 0x8b, 0x87, 0x08, 0x00, 0x00, 0x00, 0xff, 0xd0];
    assert!(
        bytes.windows(needle.len()).any(|window| window == needle),
        "expected `mov rax, [rdi+8]; call rax` (SysV field-model dispatch at the \
         layout-computed +8) in .text"
    );
    let sret_needle = [0x48u8, 0x8b, 0x86, 0x10, 0x00, 0x00, 0x00, 0xff, 0xd0];
    assert!(
        bytes
            .windows(sret_needle.len())
            .any(|window| window == sret_needle),
        "expected `mov rax, [rsi+16]; call rax`: the MEMORY-class result's hidden \
         rdi pointer must shift the receiver to rsi"
    );
    let sse_needle = [0x48u8, 0x8b, 0x87, 0x18, 0x00, 0x00, 0x00, 0xff, 0xd0];
    assert!(
        bytes
            .windows(sse_needle.len())
            .any(|window| window == sse_needle),
        "expected layout-resolved +24 dispatch for the two-f64 SSE/SSE record call"
    );
    let packed_sse_needle = [0x48u8, 0x8b, 0x87, 0x20, 0x00, 0x00, 0x00, 0xff, 0xd0];
    assert!(
        bytes
            .windows(packed_sse_needle.len())
            .any(|window| window == packed_sse_needle),
        "expected layout-resolved +32 dispatch for the packed three-f32 record call"
    );
    let mixed_needle = [0x48u8, 0x8b, 0x87, 0x28, 0x00, 0x00, 0x00, 0xff, 0xd0];
    assert!(
        bytes
            .windows(mixed_needle.len())
            .any(|window| window == mixed_needle),
        "expected layout-resolved +40 dispatch for the mixed INTEGER/SSE record call"
    );
    let split_sse_needle = [0x48u8, 0x8b, 0x87, 0x30, 0x00, 0x00, 0x00, 0xff, 0xd0];
    assert!(
        bytes
            .windows(split_sse_needle.len())
            .any(|window| window == split_sse_needle),
        "expected layout-resolved +48 dispatch for the non-homogeneous SSE/SSE record call"
    );
    let nested_needle = [0x48u8, 0x8b, 0x87, 0x38, 0x00, 0x00, 0x00, 0xff, 0xd0];
    assert!(
        bytes
            .windows(nested_needle.len())
            .any(|window| window == nested_needle),
        "expected layout-resolved +56 dispatch for the nested INTEGER/SSE record call"
    );
    let array_needle = [0x48u8, 0x8b, 0x87, 0x40, 0x00, 0x00, 0x00, 0xff, 0xd0];
    assert!(
        bytes
            .windows(array_needle.len())
            .any(|window| window == array_needle),
        "expected layout-resolved +64 dispatch for the array-member INTEGER/SSE record call"
    );
    let wrapped_needle = [0x48u8, 0x8b, 0x87, 0x48, 0x00, 0x00, 0x00, 0xff, 0xd0];
    assert!(
        bytes
            .windows(wrapped_needle.len())
            .any(|window| window == wrapped_needle),
        "expected layout-resolved +72 dispatch for the one-eightbyte nested SSE record call"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// M2-LADDER #1: `&mut` OUT-PARAMS through a field-model vtable call, MS-x64
// (GetMemoryMap's six-argument shape). Pins the WHOLE marshaling: borrow
// arguments lower as ADDRESSES (before the 2026-07-17 fix the scalar-first
// order read the POINTEE values and handed firmware garbage write targets),
// args 5-6 spill to the stack at [rsp+0x20]/[rsp+0x28], and the dispatch
// reads the layout-computed +40 field. Cross-compiled for uefi_x64 on every
// host.
#[test]
fn efi_two_table_function_leaves_cross_compile() {
    let canary = pass_canary("targets/efi_two_table_function_leaves");
    let build_dir =
        std::env::temp_dir().join(format!("omega-two-table-leaves-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_with_auxiliary_artifacts(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("two attached table-function leaves should cross-compile for uefi_x64");
    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert!(
        report.contains(
            " table function EfiBootServicesTable.get_memory_map (+40) arity 6 (table not passed)"
        ),
        "get_memory_map must select the attached TableFunction leaf with one dispatch-only table operand"
    );
    assert!(
        report.contains(
            " table function EfiBootServicesTable.exit_boot_services (+48) arity 3 (table not passed)"
        ),
        "exit_boot_services must select the attached TableFunction leaf with one dispatch-only table operand"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_out_param_call_marshals_addresses_and_stack_args() {
    let canary = pass_canary("targets/efi_out_param_call");
    let build_dir = std::env::temp_dir().join(format!("omega-out-param-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_with_auxiliary_artifacts(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("out-param canary should cross-compile for uefi_x64");
    let bytes = fs::read(build_dir.join("omega-program.exe")).expect("read emitted PE");
    // The dispatch: mov rax, [rcx+40]; call rax (get_memory_map behind
    // hdr + four leading fn-ptr fields).
    let dispatch = [0x48u8, 0x8b, 0x81, 0x28, 0x00, 0x00, 0x00, 0xff, 0xd0];
    assert!(
        bytes
            .windows(dispatch.len())
            .any(|window| window == dispatch),
        "expected `mov rax, [rcx+40]; call rax` (field-model dispatch) in .text"
    );
    // The stack spills: mov [rsp+0x20], rax and mov [rsp+0x28], rax (args 5-6).
    let spill_5 = [0x48u8, 0x89, 0x44, 0x24, 0x20];
    let spill_6 = [0x48u8, 0x89, 0x44, 0x24, 0x28];
    assert!(
        bytes.windows(spill_5.len()).any(|window| window == spill_5),
        "expected the fifth argument's stack spill (mov [rsp+0x20], rax)"
    );
    assert!(
        bytes.windows(spill_6.len()).any(|window| window == spill_6),
        "expected the sixth argument's stack spill (mov [rsp+0x28], rax)"
    );
    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert!(
        report.contains("address &omega_machine_Main::main_storage@0"),
        "the `&mut self.map_size` out-param must marshal as an ADDRESS operand"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// The x86_64 console byte-op encoders, pinned CROSS-TARGET from any host:
// windows_x64 gets the GetStdHandle(+/-10/-11) + ReadFile/WriteFile import
// flavor; linux_x64 the read/write syscall flavor. Both keep the ZII shape
// (pre-zeroed ByteRead slot; the conditional tag-1 store is the only
// count>0 write).
#[test]
fn cross_console_byte_targets_emit_x86_64_flavors() {
    let canary = pass_canary("host/cross_console_byte_targets");
    let main_path = canary.join("main.omg");

    // This canary declares a nonempty ProgramEntry matrix for all four targets.
    // Each successful compile therefore proves target-scoped selection found
    // that target's exact row; a missing selected-target row rejects before the
    // remaining corpus's temporary Main::main naming fallback can run.

    let build_dir = std::env::temp_dir().join(format!("omega-bytes-win-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: main_path.clone(),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("byte-op canary should cross-compile for windows_x64");
    let pe = fs::read(build_dir.join("omega-program.exe")).expect("read emitted PE");
    for (name, needle) in [
        ("STD_INPUT_HANDLE", &[0xb9u8, 0xf6, 0xff, 0xff, 0xff][..]),
        ("STD_OUTPUT_HANDLE", &[0xb9u8, 0xf5, 0xff, 0xff, 0xff][..]),
        (
            "count-1 + bytes-read out-param",
            &[
                0x41u8, 0xb8, 0x01, 0x00, 0x00, 0x00, 0x4c, 0x8d, 0x4c, 0x24, 0x28,
            ][..],
        ),
        (
            "conditional tag-1 store",
            &[0x74u8, 0x0b, 0x41, 0xc7, 0x86][..],
        ),
    ] {
        assert!(
            pe.windows(needle.len()).any(|window| window == needle),
            "windows_x64 byte-op image missing the {name} fragment"
        );
    }
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["macos_arm64", "linux_arm64"] {
        let build_dir =
            std::env::temp_dir().join(format!("omega-bytes-{target}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&build_dir);
        compile(CanaryCompileSpec {
            root_path: main_path.clone(),
            build_dir: Some(build_dir.clone()),
            target_name: Some(target.to_owned()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("byte-op canary should cross-compile for {target}: {diagnostics:#?}")
        });
        let _ = fs::remove_dir_all(&build_dir);
    }

    let build_dir = std::env::temp_dir().join(format!("omega-bytes-linux-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("byte-op canary should cross-compile for linux_x64");
    let elf = fs::read(build_dir.join("omega-program")).expect("read emitted ELF");
    for (name, needle) in [
        (
            "fd-0 + payload lea",
            &[0x48u8, 0x31, 0xff, 0x49, 0x8d, 0xb6][..],
        ),
        (
            "read syscall",
            &[0xb8u8, 0x00, 0x00, 0x00, 0x00, 0x0f, 0x05][..],
        ),
        (
            "write syscall",
            &[0xb8u8, 0x01, 0x00, 0x00, 0x00, 0x0f, 0x05][..],
        ),
        (
            "conditional tag-1 store",
            &[0x7eu8, 0x0b, 0x41, 0xc7, 0x86][..],
        ),
    ] {
        assert!(
            elf.windows(needle.len()).any(|window| window == needle),
            "linux_x64 byte-op image missing the {name} fragment"
        );
    }
    let _ = fs::remove_dir_all(&build_dir);
}

// The console byte-op fence: a FIELD-target read_byte is outside the served
// shape and must refuse LOUDLY with the actionable message (the composite
// owns the whole ByteRead result; nothing generic exists to fall back to,
// so a silent miss would be a ZII field). Probe-swept 2026-07-17: the
// indexed-place write arg SERVES, statement/pure discards refuse at the
// frontend, an unused `let` refuses here too (no slot to serve).
#[test]
fn console_byte_field_target_rejected_canary_is_rejected() {
    let canary = fail_canary("host/console_byte_field_target_rejected");
    let scratch = unique_no_output_build_dir();
    let diagnostics =
        match compile_single_file_hosted_main(&canary, &scratch, native_hosted_target()) {
            Ok(report) => panic!(
                "expected the field-target read_byte canary to reject, but it compiled: {}",
                report.summary()
            ),
            Err(diagnostics) => diagnostics,
        };
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("did not lower to its byte-op instruction")
            && combined.contains("let r: ByteRead"),
        "expected the actionable byte-op blocker (serving-shape hint), got:\n{combined}"
    );
    let _ = fs::remove_dir_all(scratch);
}

// Dijkstra's Dutch flag partition: an enum-array three-pointer in-place
// partition as a field-counter state machine -- indexed ENUM guard subject,
// runtime-indexed enum-element swaps, and literal re-guard states dominating
// every indexed access (the bubble_sort idiom over a sum element type).
#[test]
fn runtime_dutch_flag_partition_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_dutch_flag_partition_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-dutch-flag-partition-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dutch flag partition canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dutch flag partition canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dutch flag partition canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected [Red, White, Blue] after the partition (exit 70; 71-73 = \
         wrong element at index 0/1/2, 96-98 = invariant re-guard tripped), \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Float semantics F1: a `Wrapping` policy domain on a float primitive is a hard
// compile error -- there is no modular reading of a float (ch5 Float Facts).
#[test]
fn immutable_arg_for_mut_param_rejected_canary_is_rejected() {
    // Borrow-mutability enforcement (2026-07-18): an immutable lend
    // (`&self.c` -- the shared `&` vanishes at parse time) for a `&mut T`
    // parameter is a compile error; the legitimate bare-name FORWARD of a
    // `&mut` binding stays legal (checked semantically, not syntactically).
    let canary = fail_canary("calls/immutable_arg_for_mut_param_rejected");
    let diagnostics = match compile_canary_without_output(&canary) {
        Ok(report) => panic!(
            "expected the immutable lend for a `&mut` parameter to reject, but it compiled: {}",
            report.summary()
        ),
        Err(diagnostics) => diagnostics,
    };
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("declared `&mut`") && combined.contains("lends only immutable access"),
        "expected the immutable-lend diagnostic to name the mismatch, got:\n{combined}"
    );
}

#[test]
fn float_wrapping_domain_rejected_canary_is_rejected() {
    let canary = fail_canary("arithmetic/float_wrapping_domain_rejected");
    let diagnostics = match compile_canary_without_output(&canary) {
        Ok(report) => panic!(
            "expected `f32 in Wrapping` to reject, but it compiled: {}",
            report.summary()
        ),
        Err(diagnostics) => diagnostics,
    };
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("Wrapping") && combined.contains("no modular reading of a float"),
        "expected the wrapping-on-float diagnostic to name the policy and the reason, \
         got:\n{combined}"
    );
}

// float_saturating_domain_rejected RETIRED 2026-07-16: the F5 fence lifted
// -- Saturating/Trapping float policies now LOWER (interp + both native backends; the
// pinned semantics live in pass/arithmetic/float_saturating_overflow_exit
// + the float_trapping_{overflow,divzero,invalid}_traps canaries).
