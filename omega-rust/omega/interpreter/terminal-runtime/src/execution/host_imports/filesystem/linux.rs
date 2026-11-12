// We admit and execute exact Linux kernel filesystem system calls under explicit
// host delegation. One table (`KERNEL_CALLS`) names every call std's POSIX
// providers bind, by number on each Linux target, with the declared parameters
// in kernel order; admission matches the declaration and ABI against that row
// and execution presents each operand through the same classification. Raw OS
// statuses and descriptor ownership belong to the matching Linux host, while
// authenticated captures provide path, input and stat bytes.

use calling_conventions::{CallSignature, ValueShape};
use native_provider_runtime::{KernelOperand, KernelResult, LinuxFilesystemOperation};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{IntegerValue, ScalarType};
use target::TargetProfile;
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryStorageBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalProgram, TerminalScalarValue,
};
use terminal_psi::{BoundaryMachineDeclaration, BoundaryMachineResult, StructuralAccess};

use super::super::borrowed_storage::{CapturedStorage, StorageTransfer};
use super::{FilesystemContext, Formal, TerminalRuntimeError, matches_parameters, rejection};

fn terminated_path(
    buffers: &[TerminalBoundaryStorageBuffer],
    argument: usize,
    layout: &NativeBorrowedStorageLayout,
    invalid: impl Fn() -> TerminalEffectRejection,
) -> Result<std::ffi::CString, TerminalEffectRejection> {
    let captured = CapturedStorage::capture(
        argument,
        buffers,
        StructuralAccess::SharedBorrow,
        Some(layout),
        usize::try_from(layout.byte_size).map_err(|_| invalid())?,
        StorageTransfer::Observe,
    )?;
    Ok(
        std::ffi::CStr::from_bytes_with_nul(captured.terminated_prefix()?)
            .map_err(|_| invalid())?
            .to_owned(),
    )
}

// A host namespace executes kernel calls only on the matching Linux host; the
// runner's private namespace answers them on any host.
fn matching_host(context: &FilesystemContext, binding: &NormalizedForeignCallBinding) -> bool {
    binding.system_call().is_some_and(|call| {
        (context.private().is_some() || Some(call.target) == TargetProfile::host_if_supported())
            && matches!(
                call.target,
                TargetProfile::LinuxX64 | TargetProfile::LinuxArm64
            )
    })
}

fn authority(
    context: &FilesystemContext,
    program: &TerminalProgram,
    declaration: &BoundaryMachineDeclaration,
) -> Result<(), &'static str> {
    if !context.supplied {
        return Err(if context.rooted {
            "the filesystem namespace root could not be opened"
        } else {
            "no explicit host filesystem namespace was supplied"
        });
    }
    context.admit_services(program, declaration)
}

pub(in super::super) struct LinuxFilesystemWrite {
    context: FilesystemContext,
    authority: Result<(), &'static str>,
}

impl LinuxFilesystemWrite {
    pub(in super::super) fn admit(
        context: &FilesystemContext,
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
    ) -> Option<Self> {
        matching_host(context, binding).then(|| Self {
            context: context.clone(),
            authority: authority(context, program, declaration),
        })
    }

    pub(in super::super) fn write(
        &self,
        descriptor: i32,
        bytes: &[u8],
    ) -> Result<i128, TerminalEffectRejection> {
        self.authority.map_err(rejection)?;
        if let Some(private) = self.context.private() {
            return Ok(i128::from(private.borrow_mut().write(descriptor, bytes)));
        }
        self.context
            .linux
            .borrow()
            .write(descriptor, bytes)
            .map(i128::from)
            .map_err(|error| rejection(format!("host Linux write: {error}")))
    }

    /// Whether `descriptor` still names a modeled standard stream; once the
    /// program closes one, its number belongs to the host descriptor table.
    pub(in super::super) fn models_standard(&self, descriptor: i128) -> bool {
        i32::try_from(descriptor).is_ok_and(|descriptor| match self.context.private() {
            Some(private) => private.borrow().models_standard(descriptor),
            None => self.context.linux.borrow().models_standard(descriptor),
        })
    }

    pub(in super::super) fn read(
        &self,
        descriptor: i32,
        bytes: &mut [u8],
    ) -> Result<i128, TerminalEffectRejection> {
        self.authority.map_err(rejection)?;
        if let Some(private) = self.context.private() {
            return Ok(i128::from(private.borrow_mut().read(descriptor, bytes)));
        }
        self.context
            .linux
            .borrow()
            .read(descriptor, bytes)
            .map(i128::from)
            .map_err(|error| rejection(format!("host Linux read: {error}")))
    }
}

/// How the adapter presents one declared parameter to the kernel.
#[derive(Clone, Copy)]
enum Operand {
    /// `i32` directory operand; `AT_FDCWD` names the namespace root.
    Directory,
    /// `i32` guest descriptor.
    Descriptor,
    /// Integer passed through unchanged.
    Value(Formal),
    /// `u64` address the provider always passes as null (`utimensat`'s path).
    NullAddress,
    /// Shared fixed byte array holding a NUL-terminated path.
    Path,
    /// Shared fixed byte array of at least this many bytes, read by the kernel.
    Input(u64),
    /// Mutable fixed byte array the kernel fills with one `stat` record.
    Metadata,
    /// Shared byte view: address and length.
    InputView,
    /// Mutable byte view: address and length.
    OutputView,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Effect {
    Status,
    Acquire,
    Close,
}

/// One admitted kernel operation: its numbers on both Linux targets, its
/// declared parameters in kernel order, its result width and descriptor effect.
struct KernelCall {
    operation: LinuxFilesystemOperation,
    numbers: (i64, i64),
    operands: &'static [Operand],
    result: Formal,
    effect: Effect,
}

const fn call(
    operation: LinuxFilesystemOperation,
    x86_64: i64,
    arm64: i64,
    operands: &'static [Operand],
    result: Formal,
    effect: Effect,
) -> KernelCall {
    KernelCall {
        operation,
        numbers: (x86_64, arm64),
        operands,
        result,
        effect,
    }
}

const I32: Operand = Operand::Value(Formal::Signed(32));
const U32: Operand = Operand::Value(Formal::Unsigned(32));
const I64: Operand = Operand::Value(Formal::Signed(64));
const STATUS: Formal = Formal::Signed(32);
const COUNT: Formal = Formal::Signed(64);

// Every filesystem system call std's POSIX providers bind on Linux. A binding
// whose number is absent here is not a filesystem operation this host supplies.
const KERNEL_CALLS: &[KernelCall] = {
    use Effect::{Acquire, Close, Status};
    use LinuxFilesystemOperation as Op;
    use Operand::{
        Descriptor as Fd, Directory as Dir, InputView, Metadata, NullAddress, OutputView, Path,
    };
    &[
        call(Op::Openat, 257, 56, &[Dir, Path, I32, I32], STATUS, Acquire),
        call(Op::Close, 3, 57, &[Fd], STATUS, Close),
        call(Op::Dup, 32, 23, &[Fd], STATUS, Acquire),
        call(Op::Lseek, 8, 62, &[Fd, I64, I32], COUNT, Status),
        call(Op::Mkdirat, 258, 34, &[Dir, Path, I32], STATUS, Status),
        call(Op::Unlinkat, 263, 35, &[Dir, Path, I32], STATUS, Status),
        call(
            Op::Newfstatat,
            262,
            79,
            &[Dir, Path, Metadata, I32],
            STATUS,
            Status,
        ),
        call(Op::Fstat, 5, 80, &[Fd, Metadata], STATUS, Status),
        call(Op::Getdents64, 217, 61, &[Fd, OutputView], COUNT, Status),
        call(Op::Pread64, 17, 67, &[Fd, OutputView, I64], COUNT, Status),
        call(Op::Pwrite64, 18, 68, &[Fd, InputView, I64], COUNT, Status),
        call(
            Op::Readlinkat,
            267,
            78,
            &[Dir, Path, OutputView],
            COUNT,
            Status,
        ),
        call(Op::Fchmodat, 268, 53, &[Dir, Path, U32], STATUS, Status),
        call(Op::Fchmod, 91, 52, &[Fd, U32], STATUS, Status),
        call(
            Op::Renameat,
            264,
            38,
            &[Dir, Path, Dir, Path],
            STATUS,
            Status,
        ),
        call(
            Op::Linkat,
            265,
            37,
            &[Dir, Path, Dir, Path, I32],
            STATUS,
            Status,
        ),
        call(Op::Symlinkat, 266, 36, &[Path, Dir, Path], STATUS, Status),
        call(Op::Ftruncate, 77, 46, &[Fd, I64], STATUS, Status),
        // `utimensat(fd, NULL, times, flags)` sets the descriptor's own times.
        call(
            Op::Utimensat,
            280,
            88,
            &[Fd, NullAddress, Operand::Input(32), I32],
            STATUS,
            Status,
        ), // utimensat
        call(Op::Fsync, 74, 82, &[Fd], STATUS, Status),
        call(Op::Fdatasync, 75, 83, &[Fd], STATUS, Status),
        call(Op::Flock, 73, 32, &[Fd, I32], STATUS, Status),
        call(
            Op::Fchownat,
            260,
            54,
            &[Dir, Path, I32, I32, I32],
            STATUS,
            Status,
        ),
        call(Op::Fchown, 93, 55, &[Fd, I32, I32], STATUS, Status),
    ]
};

impl Operand {
    fn formal(self, arm64: bool) -> Formal {
        let window = |minimum, access| Formal::UnsignedWindow {
            bits: 8,
            minimum,
            access,
        };
        match self {
            Self::Directory | Self::Descriptor => Formal::Signed(32),
            Self::Value(formal) => formal,
            Self::NullAddress => Formal::Unsigned(64),
            Self::Path => window(1, StructuralAccess::SharedBorrow),
            Self::Input(minimum) => window(minimum, StructuralAccess::SharedBorrow),
            Self::Metadata => window(
                metadata_extent(arm64) as u64,
                StructuralAccess::MutableBorrow,
            ),
            Self::InputView => Formal::ByteView(StructuralAccess::SharedBorrow),
            Self::OutputView => Formal::ByteView(StructuralAccess::MutableBorrow),
        }
    }

    fn native(self) -> ValueShape {
        match self {
            Self::Directory | Self::Descriptor => ValueShape::integer(4, 4),
            Self::Value(Formal::Signed(bits) | Formal::Unsigned(bits)) if bits <= 32 => {
                ValueShape::integer(4, 4)
            }
            Self::InputView | Self::OutputView => ValueShape::integer(16, 8),
            _ => ValueShape::integer(8, 8),
        }
    }

    const fn structural(self) -> bool {
        matches!(
            self,
            Self::Path | Self::Input(_) | Self::Metadata | Self::InputView | Self::OutputView
        )
    }

    /// Fixed arrays reach the adapter as captured storage; views as bytes.
    const fn captured(self) -> bool {
        matches!(self, Self::Path | Self::Input(_) | Self::Metadata)
    }
}

pub(in super::super) struct LinuxFilesystemImport {
    signature: TerminalImportSignature,
    boundary: semantic_vocabulary::BoundaryMachineId,
    number: i64,
    arm64: bool,
    call: &'static KernelCall,
    /// Each captured structural argument's index and exact produced layout.
    captured: Vec<(usize, NativeBorrowedStorageLayout)>,
    captured_arguments: Vec<usize>,
    context: FilesystemContext,
    authority: Result<(), &'static str>,
}

impl LinuxFilesystemImport {
    pub(in super::super) fn admit(
        context: &FilesystemContext,
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        layouts: &[NativeBorrowedStorageLayout],
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        if !matching_host(context, binding) {
            return Ok(None);
        }
        let system_call = binding.system_call().expect("matching kernel host");
        let arm64 = matches!(system_call.target, TargetProfile::LinuxArm64);
        if let Some(private) = context.private() {
            private.borrow_mut().select_target(arm64);
        }
        let Some(call) = KERNEL_CALLS.iter().find(|call| {
            system_call.number
                == if arm64 {
                    call.numbers.1
                } else {
                    call.numbers.0
                }
        }) else {
            return Ok(None);
        };
        let invalid = super::super::unprovisioned(
            "Linux filesystem entry differs from its exact declaration/ABI",
        );
        let expected: Vec<Formal> = call
            .operands
            .iter()
            .map(|operand| operand.formal(arm64))
            .collect();
        // A status-only call may discard its result; a count or offset may not.
        let shape_matches = matches_parameters(program, declaration, &expected, layouts)
            && match declaration.result {
                BoundaryMachineResult::Scalar(result) => {
                    super::scalar_matches(result.scalar_type, call.result)
                }
                _ => declaration.result.is_unit() && matches!(call.result, Formal::Signed(32)),
            };
        if !shape_matches {
            return Err(invalid());
        }
        let signature = TerminalImportSignature::from(declaration);
        let native = CallSignature {
            parameters: call
                .operands
                .iter()
                .map(|operand| operand.native())
                .collect(),
            result: (!signature.result.is_unit()).then(|| match call.result {
                Formal::Signed(64) => ValueShape::integer(8, 8),
                _ => ValueShape::integer(4, 4),
            }),
        };
        super::super::admit_exact_foreign_call(&signature, declaration, binding, &native)?;
        let mut captured = Vec::new();
        for (index, operand) in call
            .operands
            .iter()
            .filter(|operand| operand.structural())
            .enumerate()
        {
            if !operand.captured() {
                continue;
            }
            let Formal::UnsignedWindow { minimum, .. } = operand.formal(arm64) else {
                unreachable!("captured operands are fixed byte windows");
            };
            let formal = &signature.structural_parameters[index];
            let layout = layouts
                .iter()
                .find(|layout| layout.structural_type == formal.structural_type)
                .filter(|layout| u64::from(layout.byte_size) >= minimum)
                .ok_or_else(|| {
                    TerminalRuntimeError::missing(
                        "runtime-filesystem",
                        "Linux filesystem buffer lost its exact produced byte-array layout",
                    )
                })?
                .clone();
            captured.push((index, layout));
        }
        Ok(Some(Self {
            signature,
            boundary: declaration.id,
            number: system_call.number,
            arm64,
            call,
            captured_arguments: captured.iter().map(|(index, _)| *index).collect(),
            captured,
            context: context.clone(),
            authority: authority(context, program, declaration),
        }))
    }

    fn result(&self, status: i64) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if self.signature.result.is_unit() {
            return Ok(TerminalEffectResult::Unit);
        }
        let Some(ScalarType::Integer(carrier)) = self.signature.result.scalar() else {
            return Err(rejection("Linux filesystem result lost its signed carrier"));
        };
        let value = IntegerValue::Signed(i128::from(status));
        if !carrier.admits(value) {
            return Err(rejection(
                "Linux filesystem status exceeds its result carrier",
            ));
        }
        Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: carrier,
            value,
        }))
    }
}

import_signature_via_clone!(LinuxFilesystemImport);

/// One operand's interpreter-side storage while the kernel call runs.
enum Staged<'layout> {
    Scalar(i64),
    Path(std::ffi::CString),
    Input(CapturedStorage<'layout>),
    Metadata(usize, CapturedStorage<'layout>, Vec<u8>),
    InputView(Vec<u8>),
    OutputView(usize),
}

impl TerminalEffectHandler for LinuxFilesystemImport {
    handle_effect_requires_dispatch!(
        "Linux filesystem entry requires authenticated buffer dispatch"
    );

    fn storage_requirements(
        &self,
        _: semantic_vocabulary::BoundaryMachineId,
        _: &[TerminalScalarValue],
    ) -> terminal_interpreter::TerminalCallStorageRequirements<'_> {
        terminal_interpreter::TerminalCallStorageRequirements {
            original: false,
            arguments: &self.captured_arguments,
        }
    }

    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        bytes: &mut [TerminalBoundaryByteBuffer],
        buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let invalid =
            || rejection("Linux filesystem invocation differs from its admitted signature");
        self.authority.map_err(rejection)?;
        let call = super::super::call_operands(effect, invalid)?;
        if *call.boundary != self.boundary
            || *call.result != self.signature.result
            || call.arguments.len() != self.signature.scalar_parameters.len()
            || call.structural_arguments.len() != self.signature.structural_parameters.len()
            || call.byte_sequence_arguments.len() != call.structural_arguments.len()
            || buffers.len() != self.captured.len()
            || call
                .structural_arguments
                .iter()
                .zip(&self.signature.structural_parameters)
                .any(|(actual, formal)| actual.structural_type != formal.structural_type)
        {
            return Err(invalid());
        }
        let mut scalars = call.arguments.iter().zip(&self.signature.scalar_parameters);
        let mut structural = 0usize;
        let mut captured = self.captured.iter().enumerate();
        let mut outputs = 0usize;
        let mut staged = Vec::with_capacity(self.call.operands.len());
        for operand in self.call.operands {
            let piece = if operand.structural() {
                let index = structural;
                structural += 1;
                let presented = &call.byte_sequence_arguments[index];
                match operand {
                    Operand::InputView => Staged::InputView(presented.clone().ok_or_else(invalid)?),
                    Operand::OutputView => {
                        let Some(presented) = presented else {
                            return Err(invalid());
                        };
                        let output = bytes
                            .iter()
                            .position(|buffer| buffer.argument_index() == index)
                            .ok_or_else(invalid)?;
                        let buffer = &bytes[output];
                        if buffer.bytes() != presented.as_slice()
                            || u128::from(buffer.capacity()) != presented.len() as u128
                        {
                            return Err(invalid());
                        }
                        outputs += 1;
                        Staged::OutputView(output)
                    }
                    _ => {
                        let (slot, (argument, layout)) = captured.next().ok_or_else(invalid)?;
                        if *argument != index || presented.is_some() {
                            return Err(invalid());
                        }
                        let buffer = &buffers[slot..=slot];
                        match operand {
                            Operand::Path => {
                                Staged::Path(terminated_path(buffer, index, layout, invalid)?)
                            }
                            Operand::Input(extent) => {
                                let extent = usize::try_from(*extent).map_err(|_| invalid())?;
                                Staged::Input(CapturedStorage::capture(
                                    index,
                                    buffer,
                                    StructuralAccess::SharedBorrow,
                                    Some(layout),
                                    extent,
                                    StorageTransfer::Observe,
                                )?)
                            }
                            _ => Staged::Metadata(
                                slot,
                                CapturedStorage::capture(
                                    index,
                                    buffer,
                                    StructuralAccess::MutableBorrow,
                                    Some(layout),
                                    metadata_extent(self.arm64),
                                    StorageTransfer::ReplacePrefix,
                                )?,
                                vec![0; 144],
                            ),
                        }
                    }
                }
            } else {
                let (value, expected) = scalars.next().ok_or_else(invalid)?;
                let value = match value {
                    TerminalScalarValue::Integer {
                        scalar_type,
                        value: IntegerValue::Signed(value),
                    } if ScalarType::Integer(*scalar_type) == *expected => {
                        i64::try_from(*value).map_err(|_| invalid())?
                    }
                    TerminalScalarValue::Integer {
                        scalar_type,
                        value: IntegerValue::Unsigned(value),
                    } if ScalarType::Integer(*scalar_type) == *expected => {
                        i64::try_from(*value).map_err(|_| invalid())?
                    }
                    _ => return Err(invalid()),
                };
                match operand {
                    Operand::Directory | Operand::Descriptor => {
                        i32::try_from(value).map_err(|_| invalid())?;
                    }
                    // The provider passes no path; a nonzero address would
                    // name interpreter memory the guest never lent.
                    Operand::NullAddress if value != 0 => return Err(invalid()),
                    _ => {}
                }
                Staged::Scalar(value)
            };
            staged.push(piece);
        }
        if outputs != bytes.len() {
            return Err(invalid());
        }
        let mut views: Vec<Option<&mut TerminalBoundaryByteBuffer>> =
            bytes.iter_mut().map(Some).collect();
        let mut kernel = Vec::with_capacity(staged.len());
        for (operand, piece) in self.call.operands.iter().zip(&mut staged) {
            kernel.push(match (operand, piece) {
                (Operand::Directory, Staged::Scalar(value)) => {
                    KernelOperand::Directory(*value as i32)
                }
                (Operand::Descriptor, Staged::Scalar(value)) => {
                    KernelOperand::Descriptor(*value as i32)
                }
                (_, Staged::Scalar(value)) => KernelOperand::Value(*value),
                (_, Staged::Path(path)) => KernelOperand::Path(path),
                (Operand::Input(extent), Staged::Input(storage)) => {
                    KernelOperand::Input(&storage.bytes[..*extent as usize])
                }
                (_, Staged::Metadata(_, _, window)) => KernelOperand::Output(window),
                (_, Staged::InputView(view)) => KernelOperand::InputView(view),
                (_, Staged::OutputView(output)) => KernelOperand::OutputView(
                    views[*output].take().ok_or_else(invalid)?.bytes_mut(),
                ),
                _ => return Err(invalid()),
            });
        }
        let result = match self.call.effect {
            Effect::Acquire => KernelResult::Descriptor,
            Effect::Status | Effect::Close => KernelResult::Status,
        };
        let status = if let Some(private) = self.context.private() {
            // The runner's namespace answers in the kernel's own convention.
            let mut private = private.borrow_mut();
            Ok(if self.call.effect == Effect::Close {
                let [KernelOperand::Descriptor(descriptor)] = kernel.as_slice() else {
                    return Err(invalid());
                };
                private.close(*descriptor)
            } else {
                private.call(self.call.operation, &mut kernel)
            })
        } else if self.call.effect == Effect::Close {
            let [KernelOperand::Descriptor(descriptor)] = kernel.as_slice() else {
                return Err(invalid());
            };
            self.context.linux.borrow_mut().close(*descriptor)
        } else {
            self.context
                .linux
                .borrow_mut()
                .call(self.number, &mut kernel, result)
        }
        .map_err(|error| rejection(format!("host Linux filesystem: {error}")))?;
        drop(kernel);
        if status == 0 {
            for piece in staged {
                if let Staged::Metadata(slot, mut output, window) = piece {
                    let extent = metadata_extent(self.arm64);
                    output.bytes[..extent].copy_from_slice(&window[..extent]);
                    output.stage(&mut buffers[slot..=slot], extent)?;
                }
            }
        }
        self.result(status)
    }
}

// One `struct stat` on the program's Linux target. A host namespace runs only
// on the matching host, so its target is the host's.
fn metadata_extent(arm64: bool) -> usize {
    if arm64 { 128 } else { 144 }
}
