//! Encode hosted entry operations and emit symbolic placement fields.
use calling_conventions::MachineRegister as R;
use machine_code::hosted_entry::{EncodedHostedEntry, EntryFixup, EntryFixupKind, EntrySymbol};
use physical_recipes::hosted_entry::{
    A64Template, FailureCondition, HostedEntryProgram, Operation, X86Reference, X86Template,
};
use target::Architecture;
pub fn encode_hosted_entry<Origin>(
    plan: HostedEntryProgram<Origin>,
) -> Result<EncodedHostedEntry, &'static str> {
    let (contract, operations) = plan.into_parts();
    let mut bytes = Vec::new();
    let mut fixups = Vec::new();
    let mut failures = Vec::new();
    let mut failed = None;
    for operation in operations {
        match operation {
            Operation::X86Fixed(template) => encode_x86_template(template, &mut bytes),
            Operation::X86Reference {
                operation,
                destination,
                addend,
            } => {
                let opcode = match operation {
                    X86Reference::StoreRsp => [0x48, 0x89, 0x25],
                    X86Reference::StoreRax => [0x48, 0x89, 0x05],
                    X86Reference::LoadRsp => [0x48, 0x8b, 0x25],
                    X86Reference::LoadRcx => [0x48, 0x8b, 0x0d],
                    X86Reference::Address(register) => {
                        let code = match register {
                            R::X86Rax => 0,
                            R::X86Rcx => 1,
                            R::X86Rdx => 2,
                            R::X86Rsp => 4,
                            R::X86Rsi => 6,
                            R::X86Rdi => 7,
                            R::X86R8 => 8,
                            R::X86R9 => 9,
                            _ => return Err("hosted pointer register is not encodable"),
                        };
                        [0x48 | ((code >> 3) << 2), 0x8d, 0x05 | ((code & 7) << 3)]
                    }
                };
                bytes.extend(opcode);
                x86_reference(&mut bytes, &mut fixups, destination, addend);
            }
            Operation::X86Initialize {
                destination,
                contents,
            } => {
                for (offset, byte) in contents.into_iter().enumerate() {
                    bytes.extend([0xc6, 0x05]);
                    x86_reference(&mut bytes, &mut fixups, destination, offset as i64 - 1);
                    bytes.push(byte);
                }
            }
            Operation::X86Stack {
                bytes: count,
                release,
            } => bytes.extend([0x48, 0x83, if release { 0xc4 } else { 0xec }, count]),
            Operation::X86StackConstant { offset, value } => {
                bytes.extend([0x48, 0xc7, 0x44, 0x24, offset]);
                bytes.extend(value.to_le_bytes());
            }
            Operation::X86WriteLength {
                bytes: count,
                windows,
            } => {
                if windows {
                    bytes.extend([0x41, 0xb8]);
                } else {
                    bytes.push(0xba);
                }
                bytes.extend(count.to_le_bytes());
            }
            Operation::X86CompareLength {
                bytes: count,
                windows,
            } => {
                if windows {
                    bytes.extend([0x81, 0x7c, 0x24, 48]);
                } else {
                    bytes.extend([0x0f, 0x05, 0x48, 0x3d]);
                }
                bytes.extend(count.to_le_bytes());
            }
            Operation::X86Failure(condition) => {
                bytes.extend([
                    0x0f,
                    match condition {
                        FailureCondition::Zero => 0x84,
                        FailureCondition::NotEqual => 0x85,
                        FailureCondition::AboveOrEqual => 0x83,
                    },
                ]);
                failures.push(bytes.len());
                bytes.extend([0; 4]);
            }
            Operation::FailureLabel => {
                if failed.replace(bytes.len()).is_some() {
                    return Err("hosted failure label is duplicated");
                }
            }
            Operation::A64Fixed(template) => encode_a64_template(template, &mut bytes),
            Operation::A64Address {
                register,
                destination,
            } => a64_address(&mut bytes, &mut fixups, register, destination),
            Operation::A64Initialize {
                destination,
                contents,
            } => {
                a64_address(&mut bytes, &mut fixups, 9, destination);
                for byte in contents {
                    words(
                        &mut bytes,
                        &[
                            0x5280_000a | (u32::from(byte) << 5),
                            0x3900_012a,
                            0x9100_0529,
                        ],
                    );
                }
            }
            Operation::A64Count { register, value } => words(
                &mut bytes,
                &[
                    0xd280_0000 | ((value & 0xffff) << 5) | u32::from(register),
                    0xf2a0_0000 | ((value >> 16) << 5) | u32::from(register),
                ],
            ),
            Operation::Call(destination) => {
                if contract.target.architecture == Architecture::X86_64 {
                    bytes.push(0xe8);
                    x86_reference(&mut bytes, &mut fixups, destination, 0);
                } else {
                    fixups.push(EntryFixup {
                        offset: bytes.len(),
                        destination,
                        addend: 0,
                        kind: EntryFixupKind::Aarch64Branch26,
                    });
                    words(&mut bytes, &[0x9400_0000]);
                }
            }
        }
    }
    for offset in failures {
        let target = failed.ok_or("hosted failure branch lost its target")?;
        let delta = i32::try_from(target as i128 - offset as i128 - 4)
            .map_err(|_| "hosted failure branch exceeds rel32")?;
        bytes[offset..offset + 4].copy_from_slice(&delta.to_le_bytes());
    }
    Ok(EncodedHostedEntry {
        target: contract.target,
        mode: contract.mode,
        bytes,
        fixups,
        imports: contract.imports,
        effects: contract.effects,
    })
}

fn x86_reference(
    bytes: &mut Vec<u8>,
    fixups: &mut Vec<EntryFixup>,
    destination: EntrySymbol,
    addend: i64,
) {
    fixups.push(EntryFixup {
        offset: bytes.len(),
        destination,
        addend,
        kind: EntryFixupKind::X86Relative32,
    });
    bytes.extend([0; 4]);
}
fn words(bytes: &mut Vec<u8>, values: &[u32]) {
    bytes.extend(values.iter().flat_map(|word| word.to_le_bytes()));
}
fn a64_address(
    bytes: &mut Vec<u8>,
    fixups: &mut Vec<EntryFixup>,
    register: u8,
    destination: EntrySymbol,
) {
    let register = u32::from(register);
    fixups.push(EntryFixup {
        offset: bytes.len(),
        destination,
        addend: 0,
        kind: EntryFixupKind::Aarch64Page21,
    });
    fixups.push(EntryFixup {
        offset: bytes.len() + 4,
        destination,
        addend: 0,
        kind: EntryFixupKind::Aarch64PageOffset12,
    });
    words(
        bytes,
        &[
            0x9000_0000 | register,
            0x9100_0000 | (register << 5) | register,
        ],
    );
}
fn encode_x86_template(template: X86Template, bytes: &mut Vec<u8>) {
    use X86Template::*;
    let encoded: &[u8] = match template {
        AlignStack => &[0x48, 0x83, 0xe4, 0xf0],
        ObserverFrame => &[0x48, 0x81, 0xec, 0, 1, 0, 0],
        EnvironmentArgument => {
            for (index, byte) in b"OMEGA_PRIVATE_OBSERVATION_PIPE\0".iter().enumerate() {
                bytes.extend([0xc6, 0x44, 0x24, 64 + index as u8, *byte]);
            }
            &[
                0x48, 0x8d, 0x4c, 0x24, 64, 0x48, 0x8d, 0x54, 0x24, 96, 0x41, 0xb8, 128, 0, 0, 0,
            ]
        }
        OpenFileArguments => &[
            0x48, 0x8d, 0x4c, 0x24, 96, 0xba, 0, 0, 0, 0x40, 0x45, 0x31, 0xc0, 0x45, 0x31, 0xc9,
        ],
        FifthPointer => &[0x48, 0x89, 0x44, 0x24, 32],
        ObserverStack => &[0x48, 0x83, 0xe4, 0xf0, 0x48, 0x83, 0xec, 64],
        TestEax => &[0x85, 0xc0],
        CheckEnvironmentExtent => &[0x3d, 128, 0, 0, 0],
        CheckFileHandle => &[0x48, 0x83, 0xf8, 0xff],
        WriteArguments => &[
            0x4c, 0x8d, 0x4c, 0x24, 48, 0x48, 0xc7, 0x44, 0x24, 32, 0, 0, 0, 0,
        ],
        ReturnZero => &[0x31, 0xc0, 0xc3],
        Return => &[0xc3],
        LinuxExitZero => &[0x31, 0xff, 0xb8, 0xe7, 0, 0, 0, 0x0f, 0x05, 0x0f, 0x0b],
        LinuxUnitExitZero => &[
            0xbf, 0, 0, 0, 0, 0xb8, 0xe7, 0, 0, 0, 0x0f, 0x05, 0x0f, 0x0b,
        ],
        LinuxWriteStart => &[0xb8, 1, 0, 0, 0, 0xbf, 3, 0, 0, 0],
        LinuxWriteCheck => &[
            0x74, 14, 0xbf, 200, 0, 0, 0, 0xb8, 0xe7, 0, 0, 0, 0x0f, 0x05, 0x0f, 0x0b,
        ],
        ObserverSuccess => &[0x31, 0xc0, 0xeb, 5],
        ObserverFailure => &[0xb8, 200, 0, 0, 0],
    };
    bytes.extend_from_slice(encoded);
}
fn encode_a64_template(template: A64Template, bytes: &mut Vec<u8>) {
    use A64Template::*;
    words(
        bytes,
        match template {
            SaveStackLink => &[0x9100_03ea, 0xa900_792a],
            SwitchStack => &[0x9100_015f],
            LoadFd => &[0xd280_0060],
            DarwinWrite => &[0xd280_0090, 0xd400_1001, 0x5400_00a2],
            LinuxWrite => &[0xd280_0808, 0xd400_0001],
            CheckWrite => &[0xeb09_001f, 0x5400_00a0],
            DarwinFailure => &[0xd280_1900, 0xd280_0030, 0xd400_1001, 0xd420_0000],
            LinuxFailure => &[0xd280_1900, 0xd280_0bc8, 0xd400_0001, 0xd420_0000],
            DarwinComplete => &[0xa940_792a, 0x9100_015f, 0x5280_0000, 0xd65f_03c0],
            LinuxComplete => &[
                0xa940_792a,
                0x9100_015f,
                0x5280_0000,
                0xd280_0bc8,
                0xd400_0001,
                0xd420_0000,
            ],
            SaveLoaderFrame => &[0xa9bf_7bfd, 0x9100_03fd],
            RestoreLoaderFrame => &[0xa8c1_7bfd],
            ReturnZero => &[0x5280_0000, 0xd65f_03c0],
            LinuxExitZero => &[0xd280_0000, 0xd280_0bc8, 0xd400_0001, 0xd420_0000],
        },
    );
}
