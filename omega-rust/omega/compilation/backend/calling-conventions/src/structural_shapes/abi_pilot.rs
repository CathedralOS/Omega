// Throwaway ABI pilot harness: reads generated signatures, classifies records
// with the real `classify_scalar_leaves`, evaluates the real call plan, and
// prints placements. Not for landing.
use super::classify_scalar_leaves;
use crate::{
    CallSignature, CallingPolicy, IndirectPointerLocation, MachineRegister, ValueClass,
    ValueLocation, ValueShape, evaluate_call_plan,
};
use std::fmt::Write as _;

fn shape(tokens: &[&str], policy: CallingPolicy) -> Result<ValueShape, String> {
    let size: u32 = tokens[1].parse().unwrap();
    let align: u16 = tokens[2].parse().unwrap();
    match tokens[0] {
        "int" => Ok(ValueShape::integer(size, align)),
        "float" => Ok(ValueShape::float(size as u16)),
        "rec" => {
            let leaves: Vec<(u32, u32, bool)> = tokens[3]
                .split(',')
                .map(|leaf| {
                    let mut parts = leaf.split(':').map(|p| p.parse::<u32>().unwrap());
                    (parts.next().unwrap(), parts.next().unwrap(), parts.next().unwrap() == 1)
                })
                .collect();
            let referent = ValueShape::integer(size, align);
            let class = classify_scalar_leaves(&leaves, referent, policy)
                .map_err(|_| "InvalidStructuralShape".to_string())?;
            Ok(ValueShape { class, ..referent })
        }
        other => panic!("unknown kind {other}"),
    }
}

fn reg(register: MachineRegister) -> String {
    match register {
        MachineRegister::X86Xmm(i) => format!("xmm{i}"),
        MachineRegister::Aarch64X(i) => format!("x{i}"),
        MachineRegister::Aarch64V(i) => format!("v{i}"),
        other => format!("{other:?}").trim_start_matches("X86").to_lowercase(),
    }
}

fn locations(out: &mut String, locations: &[ValueLocation]) {
    for location in locations {
        match *location {
            ValueLocation::Register { register, value_byte_offset, byte_size } => {
                write!(out, " ; REG {} {value_byte_offset} {byte_size}", reg(register)).unwrap()
            }
            ValueLocation::Stack { stack_byte_offset, value_byte_offset, byte_size, .. } => write!(
                out,
                " ; STACK {stack_byte_offset} {value_byte_offset} {byte_size}"
            )
            .unwrap(),
            ValueLocation::Indirect { pointer, byte_size, .. } => match pointer {
                IndirectPointerLocation::Register(register) => {
                    write!(out, " ; IND reg:{} {byte_size}", reg(register)).unwrap()
                }
                IndirectPointerLocation::Stack { stack_byte_offset, .. } => {
                    write!(out, " ; IND sp+{stack_byte_offset} {byte_size}").unwrap()
                }
            },
        }
    }
}

#[test]
fn abi_pilot_dump() {
    let Ok(input) = std::env::var("ABI_PILOT_IN") else { return };
    let output = std::env::var("ABI_PILOT_OUT").expect("ABI_PILOT_OUT");
    let text = std::fs::read_to_string(input).unwrap();
    let mut out = String::new();
    let mut policy = CallingPolicy::SystemVAMD64;
    let mut id = String::new();
    let mut parameters = Vec::new();
    let mut result = None;
    let mut refusal: Option<String> = None;
    for line in text.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        match tokens.first().copied() {
            Some("TARGET") => {
                let target = match tokens[1] {
                    "linux_x86_64" => target::NativeTarget::linux_x64(),
                    "linux_arm64" => target::NativeTarget::linux_arm64(),
                    "macos_arm64" => target::NativeTarget::macos_arm64(),
                    "windows_x86_64" => target::NativeTarget::windows_x64(),
                    other => panic!("{other}"),
                };
                policy = CallingPolicy::native_for_target(target);
            }
            Some("SIG") => {
                id = tokens[1].to_string();
                parameters.clear();
                result = None;
                refusal = None;
            }
            Some("P") | Some("R") if tokens.get(1) != Some(&"none") => {
                match shape(&tokens[1..], policy) {
                    Ok(s) if tokens[0] == "P" => parameters.push(s),
                    Ok(s) => result = Some(s),
                    Err(e) => refusal = Some(format!("classify: {e}")),
                }
            }
            Some("END") => {
                if let Some(r) = refusal.take() {
                    writeln!(out, "SIG {id} REFUSED {r}").unwrap();
                    continue;
                }
                let signature = CallSignature { parameters: parameters.clone(), result };
                match evaluate_call_plan(policy, &signature) {
                    Err(e) => writeln!(out, "SIG {id} REFUSED {}", e.0).unwrap(),
                    Ok(plan) => {
                        writeln!(out, "SIG {id} OK").unwrap();
                        for (i, p) in plan.parameters.iter().enumerate() {
                            let class = match p.shape.class {
                                ValueClass::HomogeneousFloatAggregate { members } => format!("hfa{members}"),
                                ValueClass::SystemVAggregate { first, second } => format!("sysv{first:?}{second:?}"),
                                c => format!("{c:?}"),
                            };
                            write!(out, "P {i} {class}").unwrap();
                            locations(&mut out, &p.locations);
                            out.push('\n');
                        }
                        if let Some(r) = &plan.result {
                            write!(out, "R").unwrap();
                            locations(&mut out, &r.locations);
                            out.push('\n');
                        }
                    }
                }
            }
            _ => {}
        }
    }
    std::fs::write(output, out).unwrap();
}
