/*
This test checks that Omega's default calling plans put each argument and
result byte where C code built by clang expects it, so foreign imports and
callbacks exchange the right values. It generates a fixed set of C signatures,
asks clang where every byte goes on each hosted target (`clang_mir.rs`), asks
`evaluate_call_plan` the same, and compares the two byte by byte.

The platform ABI documents are the answer key; clang is only a cross-check.
A mismatch therefore counts only when a quoted ABI rule explains it, and each
cause still open on `main` is one `KNOWN_MISMATCHES` entry naming its heading
in OWNER_CODE_CONCERNS.md together with the rule. The test fails on a mismatch
no entry explains, on an entry whose cause no longer reproduces (delete it when
its fix lands), on an entry whose concern heading is gone, and on clang output
the tracer cannot read.

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
use std::path::{Path, PathBuf};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Cause {
    /// A 16-byte-aligned value starts one integer register later than clang.
    AppleOddRegister,
    /// A stack argument lands too early because a non-HFA record on the
    /// stack (itself or an earlier one) was packed instead of taking an
    /// 8-byte-aligned slot of whole 8-byte units.
    DarwinRecordStackSlot,
}

struct KnownMismatch {
    target: &'static str,
    cause: Cause,
    /// The `### ` heading in OWNER_CODE_CONCERNS.md that tracks the fix.
    concern: &'static str,
    /// The quoted ABI rule that makes clang right and Omega wrong.
    rule: &'static str,
}

const KNOWN_MISMATCHES: &[KnownMismatch] = &[
    KnownMismatch {
        target: "macos_arm64",
        cause: Cause::AppleOddRegister,
        concern: "macOS arm64 starts 16-byte-aligned register arguments on an even register, where Apple allows an odd one",
        rule: "Apple, Writing ARM64 code for Apple platforms: \"Apple platforms allow the \
               argument to start in an odd-numbered xN register\" for a 16-byte-aligned \
               argument passed in general-purpose registers (AAPCS64 C.10 rounds the NGRN \
               up to even).",
    },
    KnownMismatch {
        target: "macos_arm64",
        cause: Cause::DarwinRecordStackSlot,
        concern: "macOS arm64 packs a record stack argument at its natural alignment instead of a whole 8-byte slot",
        rule: "AAPCS64 B.5: \"If the argument type is a Composite Type then the size of the \
               argument is rounded up to the nearest multiple of 8 bytes.\" C.14: \"The NSAA \
               is rounded up to the nearest multiple of the larger of 8 or the Natural \
               Alignment of the argument's type.\" Apple's natural-alignment stack packing \
               covers the scalars its document shows; clang keeps B.5 and C.14 for non-HFA \
               records on Apple targets.",
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

    let mut failures = known_list_failures();
    let mut seen_causes = BTreeMap::new();
    for target in &TARGETS {
        let report = compare_target(target, &signatures, &mir[target.name]);
        println!(
            "{}: {} signatures, {} agree, {} known mismatches, {} unexplained, {} refused by Omega, {} unreadable clang output",
            target.name,
            signatures.len(),
            report.agree,
            report.known.values().map(Vec::len).sum::<usize>(),
            report.unexplained.len(),
            report.refused,
            report.harness.len(),
        );
        for (cause, ids) in &report.known {
            let entry = KNOWN_MISMATCHES
                .iter()
                .find(|entry| entry.target == target.name && entry.cause == *cause)
                .expect("only listed causes are counted as known");
            println!(
                "  known {cause:?}: {} signatures, e.g. {:?}\n    concern: {}\n    rule: {}",
                ids.len(),
                &ids[..ids.len().min(5)],
                entry.concern,
                entry.rule
            );
            seen_causes.insert((target.name, *cause), ids.len());
        }
        for text in report.unexplained.iter().chain(&report.harness) {
            failures.push(format!("{}: {text}", target.name));
        }
    }
    for entry in KNOWN_MISMATCHES {
        if !seen_causes.contains_key(&(entry.target, entry.cause)) {
            failures.push(format!(
                "known mismatch {:?} on {} no longer reproduces; delete its KNOWN_MISMATCHES entry and the concern \"{}\"",
                entry.cause, entry.target, entry.concern
            ));
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

/// Each `KNOWN_MISMATCHES` entry must name a live OWNER_CODE_CONCERNS.md
/// heading, so the list cannot outlive the concern that tracks the fix.
fn known_list_failures() -> Vec<String> {
    let concerns = repository_root().join("OWNER_CODE_CONCERNS.md");
    let text = std::fs::read_to_string(&concerns)
        .unwrap_or_else(|error| panic!("read {}: {error}", concerns.display()));
    KNOWN_MISMATCHES
        .iter()
        .filter(|entry| !text.lines().any(|line| line.strip_prefix("### ") == Some(entry.concern)))
        .map(|entry| {
            format!(
                "known mismatch {:?} names the concern \"{}\", which OWNER_CODE_CONCERNS.md no longer has",
                entry.cause, entry.concern
            )
        })
        .collect()
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|dir| dir.join("OWNER_CODE_CONCERNS.md").is_file())
        .expect("the crate lives inside the repository")
        .to_path_buf()
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
    known: BTreeMap<Cause, Vec<usize>>,
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
        match cause_of(target.name, &rows) {
            Some(cause)
                if KNOWN_MISMATCHES
                    .iter()
                    .any(|entry| entry.target == target.name && entry.cause == cause) =>
            {
                report.known.entry(cause).or_default().push(k);
            }
            cause => report
                .unexplained
                .push(mismatch_text(k, signature, &rows, cause)),
        }
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
    clang_first: String,
    omega_first: String,
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
        let first = value.leaves[0].0;
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
            clang_first: comparable(clang.get(&first).unwrap_or(&none)).to_string(),
            omega_first: comparable(omega.get(&first).unwrap_or(&none)).to_string(),
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

fn stack_offset(location: &str) -> Option<u32> {
    location.strip_prefix("sp+")?.parse().ok()
}

fn x_register(location: &str) -> Option<u32> {
    location
        .strip_prefix("reg:x")?
        .split('@')
        .next()?
        .parse()
        .ok()
}

/// Names the ABI rule behind a mismatching signature from its first wrong
/// parameter; later differences usually follow from that one.
fn cause_of(target: &str, rows: &[Row<'_>]) -> Option<Cause> {
    let index = rows
        .iter()
        .position(|row| row.differs && row.class.is_some())?;
    let (earlier, row) = (&rows[..index], &rows[index]);
    if target != "macos_arm64" {
        return None;
    }
    if row.value.alignment == 16
        && row.class == Some(ValueClass::Integer)
        && let (Some(clang), Some(omega)) =
            (x_register(&row.clang_first), x_register(&row.omega_first))
        && omega == clang + 1
    {
        return Some(Cause::AppleOddRegister);
    }
    // A non-HFA record narrower than a whole 8-byte-aligned slot, which C
    // gives one anyway (AAPCS64 B.5 and C.14).
    let packed_record = |row: &Row<'_>| {
        row.value.record
            && !matches!(
                row.class,
                Some(ValueClass::HomogeneousFloatAggregate { .. })
            )
            && (row.value.alignment < 8 || !row.value.byte_size.is_multiple_of(8))
            && row.clang_first.starts_with("sp+")
    };
    if (packed_record(row) || earlier.iter().any(packed_record))
        && let (Some(clang), Some(omega)) = (
            stack_offset(&row.clang_first),
            stack_offset(&row.omega_first),
        )
        && clang > omega
    {
        return Some(Cause::DarwinRecordStackSlot);
    }
    None
}

fn mismatch_text(
    k: usize,
    signature: &GeneratedSignature,
    rows: &[Row<'_>],
    cause: Option<Cause>,
) -> String {
    let mut text = format!(
        "signature {k} {} disagrees with clang",
        signature.describe()
    );
    if let Some(cause) = cause {
        text.push_str(&format!(
            " (looks like {cause:?}, which has no entry for this target)"
        ));
    }
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
