/*
This test checks that Omega's default calling plans put each argument and
result byte where C code built by clang expects it, so foreign imports and
callbacks exchange the right values. It generates a fixed set of C signatures,
asks clang where every byte goes on each hosted target (`clang_mir.rs`), asks
`evaluate_call_plan` the same, and compares the two byte by byte.

The platform ABI documents are the answer key; clang is only a cross-check.
Every admitted signature must agree. A mismatch or unreadable clang output
fails the test; refused signatures are reported separately.

It needs clang with the AArch64 and X86 backends, so it is `#[ignore]`d and
the post-merge workflow runs it:
`mbx test -p calling-conventions --lib clang_crosscheck -- --ignored --nocapture`.
Set `OMEGA_CLANG` to choose the clang binary.
*/

mod clang_mir;
mod signatures;

use crate::structural_shapes::classify_scalar_leaves;
use crate::{
    CallPlan, CallSignature, CallingPolicy, IndirectPointerLocation, MachineRegister, ValueClass,
    ValueLocation, ValueShape, evaluate_call_plan,
};
use clang_mir::{Placements, caller_bodies, clang_placements};
use signatures::{GeneratedSignature, GeneratedValue, c_source, generated_signatures};
use std::collections::BTreeMap;
use std::process::Command;

/// Fixed so every run compares the same signatures.
const SEED: u64 = 0x4f6d_6567_6141_4249;
const SIGNATURE_COUNT: usize = 1440;

struct HostedTarget {
    name: &'static str,
    clang_triple: &'static str,
    native: fn() -> target::NativeTarget,
}

const TARGETS: [HostedTarget; 4] = [
    HostedTarget {
        name: "linux_arm64",
        clang_triple: "aarch64-linux-gnu",
        native: target::NativeTarget::linux_arm64,
    },
    HostedTarget {
        name: "macos_arm64",
        clang_triple: "arm64-apple-macos",
        native: target::NativeTarget::macos_arm64,
    },
    HostedTarget {
        name: "linux_x86_64",
        clang_triple: "x86_64-linux-gnu",
        native: target::NativeTarget::linux_x64,
    },
    HostedTarget {
        name: "windows_x86_64",
        clang_triple: "x86_64-pc-windows-msvc",
        native: target::NativeTarget::windows_x64,
    },
];

#[test]
#[ignore = "needs clang; the post-merge workflow runs it with --ignored"]
fn clang_crosscheck() {
    let started = std::time::Instant::now();
    let clang = std::env::var("OMEGA_CLANG").unwrap_or_else(|_| "clang".to_string());
    if !Command::new(&clang)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        println!(
            "clang cross-check skipped: `{clang}` did not run; install clang or set OMEGA_CLANG"
        );
        return;
    }
    let signatures = generated_signatures(SEED, SIGNATURE_COUNT);
    let mir = clang_mir_by_target(&clang, &signatures);

    let mut failures = Vec::new();
    for target in &TARGETS {
        let report = compare_target(target, &signatures, &mir[target.name]);
        println!(
            "{}: {} signatures, {} agree, {} unexplained, {} refused by Omega, {} unreadable clang output",
            target.name,
            signatures.len(),
            report.agree,
            report.unexplained.len(),
            report.refused,
            report.harness.len(),
        );
        for text in report.unexplained.iter().chain(&report.harness) {
            failures.push(format!("{}: {text}", target.name));
        }
    }
    println!("clang cross-check took {:.1?}", started.elapsed());
    if !failures.is_empty() {
        for failure in &failures {
            println!("WOULD BLOCK: {failure}");
        }
        panic!(
            "{} clang cross-check failures (listed above)",
            failures.len()
        );
    }
}

/// Lowers the generated callers with clang for every target at once.
fn clang_mir_by_target(
    clang: &str,
    signatures: &[GeneratedSignature],
) -> BTreeMap<&'static str, String> {
    let dir = std::env::temp_dir().join(format!("omega-clang-crosscheck-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("signatures.c");
    std::fs::write(&source, c_source(signatures)).unwrap();
    let children: Vec<_> = TARGETS
        .iter()
        .map(|target| {
            let output = dir.join(format!("{}.mir", target.name));
            let child = Command::new(clang)
                .arg(format!("--target={}", target.clang_triple))
                .args(["-O0", "-ffreestanding", "-fno-stack-protector", "-S"])
                .args(["-mllvm", "-stop-after=finalize-isel"])
                .arg(&source)
                .arg("-o")
                .arg(&output)
                .spawn()
                .unwrap_or_else(|error| panic!("start clang: {error}"));
            (target.name, output, child)
        })
        .collect();
    let mir = children
        .into_iter()
        .map(|(name, output, mut child)| {
            let status = child.wait().unwrap();
            assert!(status.success(), "clang failed for {name}: {status}");
            (name, std::fs::read_to_string(output).unwrap())
        })
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    mir
}

#[derive(Default)]
struct TargetReport {
    agree: usize,
    refused: usize,
    unexplained: Vec<String>,
    harness: Vec<String>,
}

fn compare_target(
    target: &HostedTarget,
    signatures: &[GeneratedSignature],
    mir: &str,
) -> TargetReport {
    let policy = CallingPolicy::native_for_target((target.native)());
    let bodies = caller_bodies(mir);
    let mut report = TargetReport::default();
    for (k, signature) in signatures.iter().enumerate() {
        let Some(plan) = omega_plan(policy, signature) else {
            // A refused signature is safe: Omega rejects the binding.
            report.refused += 1;
            continue;
        };
        let Some(body) = bodies.get(&k) else {
            report
                .harness
                .push(format!("signature {k}: clang emitted no caller c{k}"));
            continue;
        };
        let clang = match clang_placements(k, body, signature.parameters.len()) {
            Ok(placements) => placements,
            Err(failure) => {
                report.harness.push(format!(
                    "signature {k}: unreadable clang output: {}",
                    failure.0
                ));
                continue;
            }
        };
        let omega = omega_placements(&plan);
        let rows = compared_rows(signature, &plan, &clang, &omega);
        if rows.iter().all(|row| !row.differs) {
            report.agree += 1;
            continue;
        }
        report.unexplained.push(mismatch_text(k, signature, &rows));
    }
    report
}

fn omega_plan(policy: CallingPolicy, signature: &GeneratedSignature) -> Option<CallPlan> {
    let shape = |value: &GeneratedValue| -> Option<ValueShape> {
        let alignment = u16::try_from(value.alignment).ok()?;
        if !value.record {
            let (_, size, is_float) = value.leaves[0];
            return Some(if is_float {
                ValueShape::float(u16::try_from(size).ok()?)
            } else {
                ValueShape::integer(size, alignment)
            });
        }
        let referent = ValueShape::integer(value.byte_size, alignment);
        let class = classify_scalar_leaves(&value.leaves, referent, policy).ok()?;
        Some(ValueShape { class, ..referent })
    };
    let parameters = signature
        .parameters
        .iter()
        .map(shape)
        .collect::<Option<_>>()?;
    let result = match &signature.result {
        Some(result) => Some(shape(result)?),
        None => None,
    };
    evaluate_call_plan(policy, &CallSignature { parameters, result }).ok()
}

fn register_name(register: MachineRegister) -> String {
    match register {
        MachineRegister::X86Xmm(index) => format!("xmm{index}"),
        MachineRegister::Aarch64X(index) => format!("x{index}"),
        MachineRegister::Aarch64V(index) => format!("v{index}"),
        other => format!("{other:?}")
            .trim_start_matches("X86")
            .to_lowercase(),
    }
}

/// Byte placements in the same spelling `clang_mir` uses.
fn omega_placements(plan: &CallPlan) -> Placements {
    let bytes = |locations: &[ValueLocation]| {
        let mut map = BTreeMap::new();
        for location in locations {
            match *location {
                ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => {
                    for b in value_byte_offset..value_byte_offset + byte_size {
                        let at =
                            format!("reg:{}@{}", register_name(register), b - value_byte_offset);
                        map.insert(b, at);
                    }
                }
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    ..
                } => {
                    for b in value_byte_offset..value_byte_offset + byte_size {
                        map.insert(
                            b,
                            format!("sp+{}", stack_byte_offset + b - value_byte_offset),
                        );
                    }
                }
                ValueLocation::Indirect {
                    pointer, byte_size, ..
                } => {
                    let pointer = match pointer {
                        IndirectPointerLocation::Register(register) => {
                            format!("reg:{}", register_name(register))
                        }
                        IndirectPointerLocation::Stack {
                            stack_byte_offset, ..
                        } => {
                            format!("sp+{stack_byte_offset}")
                        }
                    };
                    for b in 0..byte_size {
                        map.insert(b, format!("ind({pointer})@{b}"));
                    }
                }
            }
        }
        map
    };
    Placements {
        parameters: plan
            .parameters
            .iter()
            .map(|p| bytes(&p.locations))
            .collect(),
        result: plan
            .result
            .as_ref()
            .map(|r| bytes(&r.locations))
            .unwrap_or_default(),
    }
}

/// One parameter or the result, with where its first byte goes on each side.
struct Row<'a> {
    name: String,
    value: &'a GeneratedValue,
    class: Option<ValueClass>,
    first_difference: Option<(u32, String, String)>,
    differs: bool,
}

/// An indirect copy's temporary offset is the caller's choice, so compare
/// only where the pointer travels.
fn comparable(location: &str) -> &str {
    if location.starts_with("ind(") {
        location.split(")@").next().unwrap_or(location)
    } else {
        location
    }
}

fn compared_rows<'a>(
    signature: &'a GeneratedSignature,
    plan: &CallPlan,
    clang: &Placements,
    omega: &Placements,
) -> Vec<Row<'a>> {
    let none = "<none>".to_string();
    let row = |name: String,
               value: &'a GeneratedValue,
               class,
               clang: &BTreeMap<u32, String>,
               omega: &BTreeMap<u32, String>| {
        let first_difference = value.value_bytes().find_map(|b| {
            let (c, w) = (
                clang.get(&b).unwrap_or(&none),
                omega.get(&b).unwrap_or(&none),
            );
            (comparable(c) != comparable(w)).then(|| (b, c.clone(), w.clone()))
        });
        Row {
            name,
            value,
            class,
            differs: first_difference.is_some(),
            first_difference,
        }
    };
    let mut rows: Vec<_> = signature
        .parameters
        .iter()
        .enumerate()
        .map(|(i, value)| {
            row(
                format!("p{i}"),
                value,
                Some(plan.parameters[i].shape.class),
                &clang.parameters[i],
                &omega.parameters[i],
            )
        })
        .collect();
    if let Some(result) = &signature.result {
        rows.push(row(
            "result".to_string(),
            result,
            None,
            &clang.result,
            &omega.result,
        ));
    }
    rows
}

fn mismatch_text(k: usize, signature: &GeneratedSignature, rows: &[Row<'_>]) -> String {
    let mut text = format!(
        "signature {k} {} disagrees with clang",
        signature.describe()
    );
    for row in rows {
        if let Some((byte, clang, omega)) = &row.first_difference {
            text.push_str(&format!(
                "\n    {} {} {:?}: byte {byte} clang={clang} omega={omega}",
                row.name,
                row.value.describe(),
                row.class
            ));
        }
    }
    text
}
