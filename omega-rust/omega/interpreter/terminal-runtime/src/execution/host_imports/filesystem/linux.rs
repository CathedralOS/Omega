//! Exact Linux kernel filesystem entries under explicit host delegation.
//! Raw OS statuses and descriptor ownership belong to the matching host;
//! authenticated captures provide bytes, never guest numeric addresses.

use calling_conventions::{CallSignature, ValueShape};
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
    layout: &NativeBorrowedStorageLayout,
    invalid: impl Fn() -> TerminalEffectRejection,
) -> Result<std::ffi::CString, TerminalEffectRejection> {
    let captured = CapturedStorage::capture(
        buffers,
        StructuralAccess::SharedBorrow,
        Some(layout),
        4096,
        StorageTransfer::Observe,
    )?;
    Ok(
        std::ffi::CStr::from_bytes_with_nul(captured.terminated_prefix()?)
            .map_err(|_| invalid())?
            .to_owned(),
    )
}

fn matching_host(binding: &NormalizedForeignCallBinding) -> bool {
    binding.system_call().is_some_and(|call| {
        Some(call.target) == TargetProfile::host_if_supported()
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
        return Err("no explicit host filesystem namespace was supplied");
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
        matching_host(binding).then(|| Self {
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
        self.context
            .linux
            .borrow()
            .write(descriptor, bytes)
            .map(i128::from)
            .map_err(|error| rejection(format!("host Linux write: {error}")))
    }

    pub(in super::super) fn read(
        &self,
        descriptor: i32,
        bytes: &mut [u8],
    ) -> Result<i128, TerminalEffectRejection> {
        self.authority.map_err(rejection)?;
        self.context
            .linux
            .borrow()
            .read(descriptor, bytes)
            .map(i128::from)
            .map_err(|error| rejection(format!("host Linux read: {error}")))
    }
}

enum OperationKind {
    Open,
    Close,
    Lseek,
    UnlinkAt,
}

enum Operation {
    OpenAt(NativeBorrowedStorageLayout),
    UnlinkAt(NativeBorrowedStorageLayout),
    Close,
    /// `lseek(fd, offset, whence) -> off_t`. The offset is a full signed
    /// 64-bit kernel cursor, so it cannot share the `i32` scalar narrowing
    /// used by descriptor-only entries.
    Lseek,
}

pub(in super::super) struct LinuxFilesystemImport {
    signature: TerminalImportSignature,
    boundary: semantic_vocabulary::BoundaryMachineId,
    operation: Operation,
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
        if !matching_host(binding) {
            return Ok(None);
        }
        let call = binding.system_call().expect("matching kernel host");
        // Numbers are the authored Build syscall rows for `PosixFilesystemLinux`,
        // not names of the source requirement.
        let operation = match (call.target, call.number) {
            (TargetProfile::LinuxX64, 257) | (TargetProfile::LinuxArm64, 56) => OperationKind::Open,
            (TargetProfile::LinuxX64, 3) | (TargetProfile::LinuxArm64, 57) => OperationKind::Close,
            (TargetProfile::LinuxX64, 8) | (TargetProfile::LinuxArm64, 62) => OperationKind::Lseek,
            (TargetProfile::LinuxX64, 263) | (TargetProfile::LinuxArm64, 35) => {
                OperationKind::UnlinkAt
            }
            _ => return Ok(None),
        };
        let invalid = super::super::unprovisioned(
            "Linux filesystem entry differs from its exact declaration/ABI",
        );
        let (expected, result_formal): (&[Formal], Formal) = match operation {
            OperationKind::Open => (
                &[
                    Formal::Signed(32),
                    Formal::UnsignedArray(4096, 8, StructuralAccess::SharedBorrow),
                    Formal::Signed(32),
                    Formal::Signed(32),
                ],
                Formal::Signed(32),
            ),
            OperationKind::Close => (&[Formal::Signed(32)], Formal::Signed(32)),
            OperationKind::Lseek => (
                &[Formal::Signed(32), Formal::Signed(64), Formal::Signed(32)],
                Formal::Signed(64),
            ),
            OperationKind::UnlinkAt => (
                &[
                    Formal::Signed(32),
                    Formal::UnsignedArray(4096, 8, StructuralAccess::SharedBorrow),
                    Formal::Signed(32),
                ],
                Formal::Signed(32),
            ),
        };
        let shape_matches = match operation {
            OperationKind::Lseek => {
                super::matches_signature(program, declaration, expected, result_formal, layouts)
            }
            OperationKind::Open | OperationKind::Close | OperationKind::UnlinkAt => {
                matches_parameters(program, declaration, expected, layouts)
                    && (declaration.result.is_unit()
                        || matches!(declaration.result, BoundaryMachineResult::Scalar(result)
                            if super::scalar_matches(result.scalar_type, result_formal)))
            }
        };
        if !shape_matches {
            return Err(invalid());
        }
        let signature = TerminalImportSignature::from(declaration);
        let native = CallSignature {
            parameters: match operation {
                OperationKind::Open => vec![
                    ValueShape::integer(4, 4),
                    ValueShape::integer(8, 8),
                    ValueShape::integer(4, 4),
                    ValueShape::integer(4, 4),
                ],
                OperationKind::Close => vec![ValueShape::integer(4, 4)],
                OperationKind::Lseek => vec![
                    ValueShape::integer(4, 4),
                    ValueShape::integer(8, 8),
                    ValueShape::integer(4, 4),
                ],
                OperationKind::UnlinkAt => vec![
                    ValueShape::integer(4, 4),
                    ValueShape::integer(8, 8),
                    ValueShape::integer(4, 4),
                ],
            },
            result: (!signature.result.is_unit()).then(|| match operation {
                OperationKind::Lseek => ValueShape::integer(8, 8),
                OperationKind::Open | OperationKind::Close | OperationKind::UnlinkAt => {
                    ValueShape::integer(4, 4)
                }
            }),
        };
        super::super::admit_exact_foreign_call(&signature, declaration, binding, &native)?;
        let operation = match operation {
            OperationKind::Open | OperationKind::UnlinkAt => {
                let formal = &signature.structural_parameters[0];
                let layout = layouts
                    .iter()
                    .find(|layout| layout.structural_type == formal.structural_type)
                    .filter(|layout| layout.byte_size == 4096)
                    .ok_or_else(|| {
                        TerminalRuntimeError::missing(
                            "runtime-filesystem",
                            "Linux path lost its exact produced byte-array layout",
                        )
                    })?;
                if matches!(operation, OperationKind::Open) {
                    Operation::OpenAt(layout.clone())
                } else {
                    Operation::UnlinkAt(layout.clone())
                }
            }
            OperationKind::Close => Operation::Close,
            OperationKind::Lseek => Operation::Lseek,
        };
        Ok(Some(Self {
            signature,
            boundary: declaration.id,
            operation,
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
            arguments: match self.operation {
                Operation::OpenAt(_) | Operation::UnlinkAt(_) => &[0],
                Operation::Close | Operation::Lseek => &[],
            },
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
            || !bytes.is_empty()
        {
            return Err(invalid());
        }
        let scalars = call
            .arguments
            .iter()
            .zip(&self.signature.scalar_parameters)
            .map(|(value, expected)| match value {
                TerminalScalarValue::Integer {
                    scalar_type,
                    value: IntegerValue::Signed(value),
                } if ScalarType::Integer(*scalar_type) == *expected => Ok(*value),
                _ => Err(invalid()),
            })
            .collect::<Result<Vec<i128>, _>>()?;
        let narrow = |value: i128| i32::try_from(value).map_err(|_| invalid());
        let status = match (&self.operation, scalars.as_slice()) {
            (Operation::OpenAt(layout), [directory, flags, mode]) => {
                if call.structural_arguments[0].structural_type != layout.structural_type {
                    return Err(invalid());
                }
                let path = terminated_path(buffers, layout, invalid)?;
                self.context.linux.borrow_mut().open_at(
                    narrow(*directory)?,
                    &path,
                    narrow(*flags)?,
                    narrow(*mode)?,
                )
            }
            (Operation::UnlinkAt(layout), [directory, flags]) => {
                if call.structural_arguments[0].structural_type != layout.structural_type {
                    return Err(invalid());
                }
                let path = terminated_path(buffers, layout, invalid)?;
                self.context.linux.borrow_mut().unlinkat(
                    narrow(*directory)?,
                    &path,
                    narrow(*flags)?,
                )
            }
            (Operation::Close, [descriptor]) if buffers.is_empty() => {
                self.context.linux.borrow_mut().close(narrow(*descriptor)?)
            }
            (Operation::Lseek, [descriptor, offset, whence]) if buffers.is_empty() => {
                let offset = i64::try_from(*offset).map_err(|_| invalid())?;
                self.context
                    .linux
                    .borrow()
                    .lseek(narrow(*descriptor)?, offset, narrow(*whence)?)
            }
            _ => return Err(invalid()),
        }
        .map_err(|error| rejection(format!("host Linux filesystem: {error}")))?;
        self.result(status)
    }
}
