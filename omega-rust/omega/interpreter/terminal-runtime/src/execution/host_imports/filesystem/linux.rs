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
}

enum Operation {
    OpenAt(NativeBorrowedStorageLayout),
    Close,
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
        let open = match (call.target, call.number) {
            (TargetProfile::LinuxX64, 257) | (TargetProfile::LinuxArm64, 56) => true,
            (TargetProfile::LinuxX64, 3) | (TargetProfile::LinuxArm64, 57) => false,
            _ => return Ok(None),
        };
        let invalid = super::super::unprovisioned(
            "Linux filesystem entry differs from its exact declaration/ABI",
        );
        let expected: &[Formal] = if open {
            &[
                Formal::Signed(32),
                Formal::UnsignedArray(4096, 8, StructuralAccess::SharedBorrow),
                Formal::Signed(32),
                Formal::Signed(32),
            ]
        } else {
            &[Formal::Signed(32)]
        };
        if !matches_parameters(program, declaration, expected, layouts)
            || !(declaration.result.is_unit()
                || matches!(declaration.result, BoundaryMachineResult::Scalar(result)
                    if super::scalar_matches(result.scalar_type, Formal::Signed(32))))
        {
            return Err(invalid());
        }
        let signature = TerminalImportSignature::from(declaration);
        let native = CallSignature {
            parameters: if open {
                vec![
                    ValueShape::integer(4, 4),
                    ValueShape::integer(8, 8),
                    ValueShape::integer(4, 4),
                    ValueShape::integer(4, 4),
                ]
            } else {
                vec![ValueShape::integer(4, 4)]
            },
            result: (!signature.result.is_unit()).then(|| ValueShape::integer(4, 4)),
        };
        super::super::admit_exact_foreign_call(&signature, declaration, binding, &native)?;
        let operation = if open {
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
            Operation::OpenAt(layout.clone())
        } else {
            Operation::Close
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
                Operation::OpenAt(_) => &[0],
                Operation::Close => &[],
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
                } if ScalarType::Integer(*scalar_type) == *expected => {
                    i32::try_from(*value).map_err(|_| invalid())
                }
                _ => Err(invalid()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let status = match (&self.operation, scalars.as_slice()) {
            (Operation::OpenAt(layout), [directory, flags, mode]) => {
                if call.structural_arguments[0].structural_type != layout.structural_type {
                    return Err(invalid());
                }
                let captured = CapturedStorage::capture(
                    buffers,
                    StructuralAccess::SharedBorrow,
                    Some(layout),
                    4096,
                    StorageTransfer::Observe,
                )?;
                let path = std::ffi::CStr::from_bytes_with_nul(captured.terminated_prefix()?)
                    .map_err(|_| invalid())?;
                self.context
                    .linux
                    .borrow_mut()
                    .open_at(*directory, path, *flags, *mode)
            }
            (Operation::Close, [descriptor]) if buffers.is_empty() => {
                self.context.linux.borrow_mut().close(*descriptor)
            }
            _ => return Err(invalid()),
        }
        .map_err(|error| rejection(format!("host Linux filesystem: {error}")))?;
        self.result(status)
    }
}
