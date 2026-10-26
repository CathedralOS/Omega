//! Execute emitted adapters against the host C compiler's actual ABI.
//! The retained-pointer provider is the same independently authored C fixture
//! used by the language fact; this test establishes only physical transport.

use std::{fmt::Write, path::PathBuf, process::Command};

use calling_conventions::{
    CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use machine_emission::native_foreign_bridge::{
    NativeForeignBridgeRequest, emit_native_foreign_bridge,
};
use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, IntegerType, ScalarType};
use target::{Architecture, NativeTarget, ObjectFormat};

fn append_bridge(
    assembly: &mut String,
    target: NativeTarget,
    symbol: &str,
    parameters: Vec<ValueShape>,
    result: Option<ValueShape>,
    scalar_parameters: Vec<Option<ScalarType>>,
) {
    let bridge = emit_native_foreign_bridge(NativeForeignBridgeRequest {
        target,
        scalar_parameters,
        foreign: evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature { parameters, result },
        )
        .expect("host C calling plan"),
    })
    .expect("physical C adapter");
    let prefix = if target.object_format == ObjectFormat::MachO {
        "_"
    } else {
        ""
    };
    writeln!(
        assembly,
        ".p2align 4\n.globl {prefix}bridge_{symbol}\n{prefix}bridge_{symbol}:"
    )
    .unwrap();
    // Keep the unresolved ordinary call with the assembler's real relocation.
    // All other instructions are the bytes emitted by the production encoder.
    let (call_start, call_size, mnemonic) = match target.architecture {
        Architecture::X86_64 => (bridge.call_field_offset() - 1, 5, "call"),
        Architecture::Aarch64 => (bridge.call_field_offset(), 4, "bl"),
    };
    for part in [
        &bridge.bytes()[..call_start],
        &bridge.bytes()[call_start + call_size..],
    ]
    .iter()
    .enumerate()
    {
        if part.0 == 1 {
            writeln!(assembly, "{mnemonic} {prefix}{symbol}").unwrap();
        }
        for chunk in part.1.chunks(16) {
            let bytes = chunk
                .iter()
                .map(|byte| format!("0x{byte:02x}"))
                .collect::<Vec<_>>()
                .join(",");
            writeln!(assembly, ".byte {bytes}").unwrap();
        }
    }
}

#[test]
fn actual_c_retention_stack_float_and_record_calls() {
    let target = NativeTarget::host();
    let word = ValueShape::integer(8, 8);
    let pointer = ValueShape::borrowed_reference(8, 8);
    let mut assembly = String::from(".text\n");
    let unsigned = |bits| {
        Some(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
        ))
    };
    let signed = |bits| {
        Some(ScalarType::Integer(
            IntegerType::new(IntegerSign::Signed, bits).unwrap(),
        ))
    };
    for (symbol, parameters, result, scalars) in [
        ("retention_acquire", vec![], Some(word), vec![]),
        (
            "retention_register",
            vec![word, pointer],
            Some(word),
            vec![None, None],
        ),
        (
            "retention_complete",
            vec![word, pointer],
            Some(word),
            vec![None, None],
        ),
        ("retention_release", vec![word], None, vec![None]),
        (
            "stack_words",
            vec![word; 12],
            Some(word),
            vec![unsigned(64); 12],
        ),
        (
            "stack_floats",
            vec![ValueShape::float(8); 10],
            Some(ValueShape::float(8)),
            vec![Some(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)); 10],
        ),
        (
            "small_record",
            vec![ValueShape::integer(16, 8), word],
            Some(ValueShape::integer(16, 8)),
            vec![None, unsigned(64)],
        ),
        (
            "large_record",
            vec![ValueShape::integer(24, 8), word],
            Some(ValueShape::integer(24, 8)),
            vec![None, unsigned(64)],
        ),
        (
            "narrow_scalars_and_records",
            vec![
                ValueShape::integer(1, 1),
                ValueShape::integer(2, 2),
                ValueShape::integer(1, 1),
                ValueShape::integer(2, 2),
                ValueShape::integer(1, 1),
                ValueShape::integer(2, 2),
            ],
            Some(word),
            vec![signed(8), signed(16), None, None, unsigned(8), unsigned(16)],
        ),
    ] {
        append_bridge(&mut assembly, target, symbol, parameters, result, scalars);
    }
    if target.object_format == ObjectFormat::Elf {
        assembly.push_str(".section .note.GNU-stack,\"\",@progbits\n");
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("omega-foreign-abi-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("bridge.S"), assembly).unwrap();
    std::fs::write(
        directory.join("driver.c"),
        include_str!("native_foreign_bridge.c"),
    )
    .unwrap();
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let provider = repository
        .ancestors()
        .nth(5)
        .unwrap()
        .join("samples/cli/facts/foreign_retained_shared_borrow/fixture/provider.c");
    let executable = directory.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    let output = Command::new(std::env::var_os("CC").unwrap_or_else(|| {
        if cfg!(windows) {
            "clang".into()
        } else {
            "cc".into()
        }
    }))
    .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"])
    .arg(directory.join("driver.c"))
    .arg(directory.join("bridge.S"))
    .arg(provider)
    .arg("-o")
    .arg(&executable)
    .output()
    .expect("host C compiler");
    assert!(
        output.status.success(),
        "C build failed; retained at {}: {}",
        directory.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(executable)
        .output()
        .expect("actual C ABI execution");
    assert!(
        output.status.success(),
        "C execution failed; retained at {}: {:?}\n{}\n{}",
        directory.display(),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn another_host_calling_policy_is_rejected() {
    for target in [
        NativeTarget::macos_arm64(),
        NativeTarget::linux_arm64(),
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
    ] {
        let plan = evaluate_ordinary_boundary_entry_plan(
            if CallingPolicy::native_for_target(target) == CallingPolicy::MicrosoftX64 {
                CallingPolicy::SystemVAMD64
            } else {
                CallingPolicy::MicrosoftX64
            },
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8)],
                result: None,
            },
        )
        .unwrap();
        assert!(
            emit_native_foreign_bridge(NativeForeignBridgeRequest {
                target,
                foreign: plan,
                scalar_parameters: vec![None],
            })
            .is_err()
        );
    }
}
