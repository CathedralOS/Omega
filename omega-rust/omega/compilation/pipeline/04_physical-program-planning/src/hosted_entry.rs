//! Select hosted helper operations, storage and register effects.
use calling_conventions::{
    MachineRegister as R, MachineState as S, MachineStateSet, RegisterSet, StateEffectsEvidence,
};
use target::{Architecture, NativeTarget};

use machine_code::hosted_entry::{EntrySymbol, HostedEntryMode};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiverStorage {
    pub contents: Vec<u8>,
    pub observation_header: Vec<u8>,
    pub runtime_position: u32,
}

use physical_recipes::hosted_entry as recipes;
use recipes::HostedEntryInstructions;
pub use recipes::{
    A64Template, FailureCondition, HostedEntryContract, Operation, X86Reference, X86Template,
};
pub type HostedEntryProgram = recipes::HostedEntryProgram<crate::PhysicalRecipeOrigin>;

pub fn plan_hosted_entry(
    target: NativeTarget,
    mode: HostedEntryMode,
    storage: &[ReceiverStorage],
) -> Result<HostedEntryProgram, &'static str> {
    if ![
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_x64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ]
    .contains(&target)
        || mode != HostedEntryMode::Receiver && !storage.is_empty()
        || mode == HostedEntryMode::Receiver && storage.is_empty()
    {
        return Err("hosted entry has an unsupported target or storage mode");
    }
    let mut operations = Vec::new();
    let mut imports = Vec::new();
    let call_overhead = match mode {
        HostedEntryMode::LoaderStack => 0,
        HostedEntryMode::Receiver if target == NativeTarget::windows_x64() => 72,
        HostedEntryMode::OwnedStack if target == NativeTarget::windows_x64() => 40,
        _ if target.architecture == Architecture::X86_64 => 8,
        _ => 0,
    };
    match mode {
        HostedEntryMode::LoaderStack => plan_loader(target, &mut operations)?,
        HostedEntryMode::OwnedStack if target.architecture == Architecture::X86_64 => {
            reference(
                &mut operations,
                X86Reference::StoreRsp,
                EntrySymbol::Scratch,
                0,
            );
            reference(
                &mut operations,
                X86Reference::Address(R::X86Rsp),
                EntrySymbol::StackTop,
                0,
            );
            if target == NativeTarget::windows_x64() {
                operations.push(Operation::X86Stack {
                    bytes: 32,
                    release: false,
                });
            }
            operations.push(Operation::Call(EntrySymbol::Body));
            reference(
                &mut operations,
                X86Reference::LoadRsp,
                EntrySymbol::Scratch,
                0,
            );
            operations.push(Operation::X86Fixed(
                if target == NativeTarget::linux_x64() {
                    X86Template::LinuxUnitExitZero
                } else {
                    X86Template::ReturnZero
                },
            ));
        }
        _ if target.architecture == Architecture::Aarch64 => {
            plan_aarch64(target, storage, &mut operations)?
        }
        HostedEntryMode::Receiver if target == NativeTarget::windows_x64() => {
            plan_windows(storage, &mut operations, &mut imports)?
        }
        HostedEntryMode::Receiver if target == NativeTarget::linux_x64() => {
            plan_linux(storage, &mut operations)?
        }
        _ => return Err("hosted entry has no physical program for its selected target"),
    }
    let effects = operation_effects(target.architecture, &operations);
    Ok(HostedEntryProgram::from_planner(
        crate::PhysicalRecipeOrigin(()),
        HostedEntryInstructions {
            target,
            mode,
            operations,
            imports,
            effects,
            call_overhead,
        },
    ))
}

fn runtime_order(storage: &[ReceiverStorage], maximum: usize) -> Result<Vec<usize>, &'static str> {
    if storage.len() > maximum {
        return Err("hosted receiver exceeds its admitted parameter registers");
    }
    let mut order = vec![usize::MAX; storage.len()];
    for (index, row) in storage.iter().enumerate() {
        let slot = order
            .get_mut(row.runtime_position as usize)
            .ok_or("hosted receiver parameter is outside its telescope")?;
        if *slot != usize::MAX {
            return Err("hosted receiver repeats a parameter");
        }
        *slot = index;
    }
    if order.contains(&usize::MAX) {
        return Err("hosted receiver has an incomplete parameter telescope");
    }
    Ok(order)
}

fn reference(
    ops: &mut Vec<Operation>,
    operation: X86Reference,
    destination: EntrySymbol,
    addend: i64,
) {
    ops.push(Operation::X86Reference {
        operation,
        destination,
        addend,
    });
}

fn plan_loader(target: NativeTarget, ops: &mut Vec<Operation>) -> Result<(), &'static str> {
    if target.architecture == Architecture::Aarch64 {
        let returning = target == NativeTarget::macos_arm64();
        if returning {
            ops.push(Operation::A64Fixed(A64Template::SaveLoaderFrame));
        }
        ops.push(Operation::Call(EntrySymbol::Body));
        if returning {
            ops.push(Operation::A64Fixed(A64Template::RestoreLoaderFrame));
            ops.push(Operation::A64Fixed(A64Template::ReturnZero));
        } else {
            ops.push(Operation::A64Fixed(A64Template::LinuxExitZero));
        }
    } else {
        let bytes = if target == NativeTarget::windows_x64() {
            40
        } else if target == NativeTarget::macos_x64() {
            8
        } else {
            0
        };
        if bytes != 0 {
            ops.push(Operation::X86Stack {
                bytes,
                release: false,
            });
        }
        ops.push(Operation::Call(EntrySymbol::Body));
        if bytes != 0 {
            ops.push(Operation::X86Stack {
                bytes,
                release: true,
            });
        }
        ops.push(Operation::X86Fixed(
            if target == NativeTarget::linux_x64() {
                X86Template::LinuxUnitExitZero
            } else {
                X86Template::ReturnZero
            },
        ));
    }
    Ok(())
}

fn plan_aarch64(
    target: NativeTarget,
    storage: &[ReceiverStorage],
    ops: &mut Vec<Operation>,
) -> Result<(), &'static str> {
    let darwin = target == NativeTarget::macos_arm64();
    ops.push(Operation::A64Address {
        register: 9,
        destination: EntrySymbol::Scratch,
    });
    ops.push(Operation::A64Fixed(A64Template::SaveStackLink));
    ops.push(Operation::A64Address {
        register: 10,
        destination: EntrySymbol::StackTop,
    });
    ops.push(Operation::A64Fixed(A64Template::SwitchStack));
    for (index, row) in storage.iter().enumerate() {
        ops.push(Operation::A64Initialize {
            destination: EntrySymbol::PrivateRam(index),
            contents: row.contents.clone(),
        });
    }
    for (register, index) in runtime_order(storage, 8)?.into_iter().enumerate() {
        ops.push(Operation::A64Address {
            register: register as u8,
            destination: EntrySymbol::PrivateRam(index),
        });
    }
    ops.push(Operation::Call(EntrySymbol::Body));
    for (index, row) in storage.iter().enumerate() {
        ops.push(Operation::A64Initialize {
            destination: EntrySymbol::ObservationHeader(index),
            contents: row.observation_header.clone(),
        });
        for (destination, length) in [
            (
                EntrySymbol::ObservationHeader(index),
                row.observation_header.len(),
            ),
            (EntrySymbol::PrivateRam(index), row.contents.len()),
        ] {
            let value = u32::try_from(length).map_err(|_| "hosted observation exceeds u32")?;
            ops.push(Operation::A64Fixed(A64Template::LoadFd));
            ops.push(Operation::A64Address {
                register: 1,
                destination,
            });
            ops.push(Operation::A64Count { register: 2, value });
            ops.push(Operation::A64Fixed(if darwin {
                A64Template::DarwinWrite
            } else {
                A64Template::LinuxWrite
            }));
            ops.push(Operation::A64Count { register: 9, value });
            ops.push(Operation::A64Fixed(A64Template::CheckWrite));
            ops.push(Operation::A64Fixed(if darwin {
                A64Template::DarwinFailure
            } else {
                A64Template::LinuxFailure
            }));
        }
    }
    ops.push(Operation::A64Address {
        register: 9,
        destination: EntrySymbol::Scratch,
    });
    ops.push(Operation::A64Fixed(if darwin {
        A64Template::DarwinComplete
    } else {
        A64Template::LinuxComplete
    }));
    Ok(())
}

fn plan_linux(storage: &[ReceiverStorage], ops: &mut Vec<Operation>) -> Result<(), &'static str> {
    reference(ops, X86Reference::StoreRsp, EntrySymbol::Scratch, 0);
    reference(
        ops,
        X86Reference::Address(R::X86Rsp),
        EntrySymbol::StackTop,
        0,
    );
    for (index, row) in storage.iter().enumerate() {
        ops.push(Operation::X86Initialize {
            destination: EntrySymbol::PrivateRam(index),
            contents: row.contents.clone(),
        });
    }
    for (ordinal, index) in runtime_order(storage, 6)?.into_iter().enumerate() {
        reference(
            ops,
            X86Reference::Address(
                [
                    R::X86Rdi,
                    R::X86Rsi,
                    R::X86Rdx,
                    R::X86Rcx,
                    R::X86R8,
                    R::X86R9,
                ][ordinal],
            ),
            EntrySymbol::PrivateRam(index),
            0,
        );
    }
    ops.push(Operation::Call(EntrySymbol::Body));
    for (index, row) in storage.iter().enumerate() {
        ops.push(Operation::X86Initialize {
            destination: EntrySymbol::ObservationHeader(index),
            contents: row.observation_header.clone(),
        });
        for (destination, length) in [
            (
                EntrySymbol::ObservationHeader(index),
                row.observation_header.len(),
            ),
            (EntrySymbol::PrivateRam(index), row.contents.len()),
        ] {
            let bytes = u32::try_from(length).map_err(|_| "hosted observation exceeds u32")?;
            ops.push(Operation::X86Fixed(X86Template::LinuxWriteStart));
            reference(ops, X86Reference::Address(R::X86Rsi), destination, 0);
            ops.push(Operation::X86WriteLength {
                bytes,
                windows: false,
            });
            ops.push(Operation::X86CompareLength {
                bytes,
                windows: false,
            });
            ops.push(Operation::X86Fixed(X86Template::LinuxWriteCheck));
        }
    }
    ops.push(Operation::X86Fixed(X86Template::LinuxExitZero));
    Ok(())
}

fn plan_windows(
    storage: &[ReceiverStorage],
    ops: &mut Vec<Operation>,
    imports: &mut Vec<target::NormalizedForeignLocator>,
) -> Result<(), &'static str> {
    for name in [
        "GetEnvironmentVariableA",
        "CreateFileA",
        "WriteFile",
        "FlushFileBuffers",
        "CloseHandle",
    ] {
        imports.push(
            target::normalize_foreign_locator(
                target::ForeignLocatorCandidate::PeByName {
                    library: b"kernel32.dll".to_vec(),
                    export: name.as_bytes().to_vec(),
                },
                target::TargetProfile::WindowsX64,
            )
            .map_err(|_| "private entry observer import is invalid")?,
        );
    }
    reference(ops, X86Reference::StoreRsp, EntrySymbol::Scratch, 0);
    ops.push(Operation::X86Fixed(X86Template::AlignStack));
    ops.push(Operation::X86Fixed(X86Template::ObserverFrame));
    ops.push(Operation::X86Fixed(X86Template::EnvironmentArgument));
    ops.push(Operation::Call(EntrySymbol::Import(0)));
    ops.push(Operation::X86Fixed(X86Template::TestEax));
    ops.push(Operation::X86Failure(FailureCondition::Zero));
    ops.push(Operation::X86Fixed(X86Template::CheckEnvironmentExtent));
    ops.push(Operation::X86Failure(FailureCondition::AboveOrEqual));
    ops.push(Operation::X86Fixed(X86Template::OpenFileArguments));
    for (offset, value) in [(32, 3), (40, 0), (48, 0)] {
        ops.push(Operation::X86StackConstant { offset, value });
    }
    ops.push(Operation::Call(EntrySymbol::Import(1)));
    ops.push(Operation::X86Fixed(X86Template::CheckFileHandle));
    ops.push(Operation::X86Failure(FailureCondition::Zero));
    reference(ops, X86Reference::StoreRax, EntrySymbol::Scratch, 8);
    reference(
        ops,
        X86Reference::Address(R::X86Rsp),
        EntrySymbol::StackTop,
        0,
    );
    ops.push(Operation::X86Stack {
        bytes: 64,
        release: false,
    });
    for (index, row) in storage.iter().enumerate() {
        ops.push(Operation::X86Initialize {
            destination: EntrySymbol::PrivateRam(index),
            contents: row.contents.clone(),
        });
    }
    for (ordinal, index) in runtime_order(storage, 5)?.into_iter().enumerate() {
        reference(
            ops,
            X86Reference::Address([R::X86Rcx, R::X86Rdx, R::X86R8, R::X86R9, R::X86Rax][ordinal]),
            EntrySymbol::PrivateRam(index),
            0,
        );
        if ordinal == 4 {
            ops.push(Operation::X86Fixed(X86Template::FifthPointer));
        }
    }
    ops.push(Operation::Call(EntrySymbol::Body));
    reference(ops, X86Reference::LoadRsp, EntrySymbol::Scratch, 0);
    ops.push(Operation::X86Fixed(X86Template::ObserverStack));
    for (index, row) in storage.iter().enumerate() {
        ops.push(Operation::X86Initialize {
            destination: EntrySymbol::ObservationHeader(index),
            contents: row.observation_header.clone(),
        });
        for (destination, length) in [
            (
                EntrySymbol::ObservationHeader(index),
                row.observation_header.len(),
            ),
            (EntrySymbol::PrivateRam(index), row.contents.len()),
        ] {
            reference(ops, X86Reference::LoadRcx, EntrySymbol::Scratch, 8);
            reference(ops, X86Reference::Address(R::X86Rdx), destination, 0);
            let bytes = u32::try_from(length).map_err(|_| "hosted observation exceeds u32")?;
            ops.push(Operation::X86WriteLength {
                bytes,
                windows: true,
            });
            ops.push(Operation::X86Fixed(X86Template::WriteArguments));
            ops.push(Operation::Call(EntrySymbol::Import(2)));
            ops.push(Operation::X86Fixed(X86Template::TestEax));
            ops.push(Operation::X86Failure(FailureCondition::Zero));
            ops.push(Operation::X86CompareLength {
                bytes,
                windows: true,
            });
            ops.push(Operation::X86Failure(FailureCondition::NotEqual));
        }
    }
    for index in [3, 4] {
        reference(ops, X86Reference::LoadRcx, EntrySymbol::Scratch, 8);
        ops.push(Operation::Call(EntrySymbol::Import(index)));
        ops.push(Operation::X86Fixed(X86Template::TestEax));
        ops.push(Operation::X86Failure(FailureCondition::Zero));
    }
    ops.push(Operation::X86Fixed(X86Template::ObserverSuccess));
    ops.push(Operation::FailureLabel);
    ops.push(Operation::X86Fixed(X86Template::ObserverFailure));
    reference(ops, X86Reference::LoadRsp, EntrySymbol::Scratch, 0);
    ops.push(Operation::X86Fixed(X86Template::Return));
    Ok(())
}

fn operation_effects(architecture: Architecture, operations: &[Operation]) -> StateEffectsEvidence {
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    let mut used = Vec::new();
    let mut clobbered = Vec::new();
    for operation in operations {
        match operation {
            Operation::X86Reference { operation, .. } => match operation {
                X86Reference::StoreRsp => {
                    reads.push(R::X86Rsp);
                    used.push(S::StackPointer);
                }
                X86Reference::StoreRax => reads.push(R::X86Rax),
                X86Reference::LoadRsp | X86Reference::Address(R::X86Rsp) => {
                    writes.push(R::X86Rsp);
                    clobbered.push(S::StackPointer);
                }
                X86Reference::LoadRcx => writes.push(R::X86Rcx),
                X86Reference::Address(register) => writes.push(*register),
            },
            Operation::X86Stack { .. } => {
                reads.push(R::X86Rsp);
                writes.push(R::X86Rsp);
                used.push(S::StackPointer);
                clobbered.extend([S::StackPointer, S::Flags]);
            }
            Operation::X86StackConstant { .. } => {
                reads.push(R::X86Rsp);
                used.push(S::StackPointer);
            }
            Operation::X86WriteLength { windows, .. } => {
                writes.push(if *windows { R::X86R8 } else { R::X86Rdx })
            }
            Operation::X86CompareLength { windows, .. } => {
                clobbered.push(S::Flags);
                if *windows {
                    reads.push(R::X86Rsp);
                    used.push(S::StackPointer);
                } else {
                    reads.extend([R::X86Rax, R::X86Rdi, R::X86Rsi, R::X86Rdx]);
                    writes.extend([R::X86Rax, R::X86Rcx, R::X86R11]);
                }
            }
            Operation::X86Failure(_) => used.push(S::Flags),
            Operation::X86Initialize { .. } | Operation::FailureLabel => {}
            Operation::A64Address { register, .. } => {
                reads.push(R::Aarch64X(*register));
                writes.push(R::Aarch64X(*register));
            }
            Operation::A64Initialize { .. } => {
                reads.extend([R::Aarch64X(9), R::Aarch64X(10)]);
                writes.extend([R::Aarch64X(9), R::Aarch64X(10)]);
            }
            Operation::A64Count { register, .. } => {
                reads.push(R::Aarch64X(*register));
                writes.push(R::Aarch64X(*register));
            }
            Operation::Call(_) => match architecture {
                Architecture::X86_64 => {
                    reads.push(R::X86Rsp);
                    writes.push(R::X86Rsp);
                    used.push(S::StackPointer);
                    clobbered.push(S::StackPointer);
                }
                Architecture::Aarch64 => writes.push(R::Aarch64X(30)),
            },
            Operation::X86Fixed(template) => x86_template_effects(
                *template,
                &mut reads,
                &mut writes,
                &mut used,
                &mut clobbered,
            ),
            Operation::A64Fixed(template) => a64_template_effects(
                *template,
                &mut reads,
                &mut writes,
                &mut used,
                &mut clobbered,
            ),
        }
    }
    StateEffectsEvidence::new(
        RegisterSet::new(reads),
        RegisterSet::new(writes),
        MachineStateSet::new(used),
        MachineStateSet::new(clobbered),
    )
}

fn x86_template_effects(
    template: X86Template,
    reads: &mut Vec<R>,
    writes: &mut Vec<R>,
    used: &mut Vec<S>,
    clobbered: &mut Vec<S>,
) {
    use X86Template::*;
    match template {
        AlignStack | ObserverFrame | ObserverStack => {
            reads.push(R::X86Rsp);
            writes.push(R::X86Rsp);
            used.push(S::StackPointer);
            clobbered.extend([S::StackPointer, S::Flags]);
        }
        EnvironmentArgument => {
            reads.push(R::X86Rsp);
            writes.extend([R::X86Rcx, R::X86Rdx, R::X86R8]);
            used.push(S::StackPointer);
        }
        OpenFileArguments => {
            reads.extend([R::X86Rsp, R::X86R8, R::X86R9]);
            writes.extend([R::X86Rcx, R::X86Rdx, R::X86R8, R::X86R9]);
            used.push(S::StackPointer);
            clobbered.push(S::Flags);
        }
        FifthPointer => {
            reads.extend([R::X86Rsp, R::X86Rax]);
            used.push(S::StackPointer);
        }
        TestEax | CheckEnvironmentExtent | CheckFileHandle => {
            reads.push(R::X86Rax);
            clobbered.push(S::Flags);
        }
        WriteArguments => {
            reads.push(R::X86Rsp);
            writes.push(R::X86R9);
            used.push(S::StackPointer);
        }
        ReturnZero => {
            reads.extend([R::X86Rax, R::X86Rsp]);
            writes.extend([R::X86Rax, R::X86Rsp]);
            used.push(S::StackPointer);
            clobbered.extend([S::StackPointer, S::Flags]);
        }
        Return => {
            reads.push(R::X86Rsp);
            writes.push(R::X86Rsp);
            used.push(S::StackPointer);
            clobbered.push(S::StackPointer);
        }
        LinuxExitZero | LinuxUnitExitZero | LinuxWriteCheck => {
            reads.extend([R::X86Rdi, R::X86Rax]);
            writes.extend([R::X86Rdi, R::X86Rax, R::X86Rcx, R::X86R11]);
            clobbered.push(S::Flags);
            if matches!(template, LinuxWriteCheck) {
                used.push(S::Flags);
            }
        }
        LinuxWriteStart => writes.extend([R::X86Rax, R::X86Rdi]),
        ObserverSuccess => {
            reads.push(R::X86Rax);
            writes.push(R::X86Rax);
            clobbered.push(S::Flags);
        }
        ObserverFailure => writes.push(R::X86Rax),
    }
}
fn a64_template_effects(
    template: A64Template,
    reads: &mut Vec<R>,
    writes: &mut Vec<R>,
    used: &mut Vec<S>,
    clobbered: &mut Vec<S>,
) {
    use A64Template::*;
    let x = R::Aarch64X;
    match template {
        SaveStackLink => {
            reads.extend([x(9), x(10), x(30)]);
            writes.push(x(10));
            used.push(S::StackPointer);
        }
        SwitchStack => {
            reads.push(x(10));
            clobbered.push(S::StackPointer);
        }
        LoadFd => writes.push(x(0)),
        DarwinWrite | LinuxWrite => {
            let selector = if matches!(template, DarwinWrite) {
                16
            } else {
                8
            };
            reads.extend([x(0), x(1), x(2), x(selector)]);
            writes.extend([x(0), x(selector)]);
            if selector == 16 {
                clobbered.push(S::Flags);
                used.push(S::Flags);
            }
        }
        CheckWrite => {
            reads.extend([x(0), x(9)]);
            used.push(S::Flags);
            clobbered.push(S::Flags);
        }
        DarwinFailure | LinuxFailure | LinuxExitZero => {
            let selector = if matches!(template, DarwinFailure) {
                16
            } else {
                8
            };
            reads.extend([x(0), x(selector)]);
            writes.extend([x(0), x(selector)]);
            if selector == 16 {
                clobbered.push(S::Flags);
            }
        }
        DarwinComplete | LinuxComplete => {
            reads.extend([x(9), x(10), x(30)]);
            writes.extend([x(0), x(10), x(30)]);
            clobbered.push(S::StackPointer);
            if matches!(template, LinuxComplete) {
                reads.extend([x(0), x(8)]);
                writes.push(x(8));
            }
        }
        SaveLoaderFrame => {
            reads.extend([x(29), x(30)]);
            writes.push(x(29));
            used.push(S::StackPointer);
            clobbered.push(S::StackPointer);
        }
        RestoreLoaderFrame => {
            writes.extend([x(29), x(30)]);
            used.push(S::StackPointer);
            clobbered.push(S::StackPointer);
        }
        ReturnZero => {
            writes.push(x(0));
            reads.push(x(30));
        }
    }
}
