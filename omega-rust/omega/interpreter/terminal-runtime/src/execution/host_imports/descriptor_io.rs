/*
We provide trusted descriptor reads and writes over finite input and private
output streams, with optional Linux filesystem descriptor delegation.

Exact normalized locators, declarations and target ABI establish compatibility.
Scalar leaves and byte views expose their original backing through one capture
and writeback protocol. Native lowering retains record and array placement.
Views retain their exact live range and pointer/length ABI; mutable views allow
fixed-extent writes without granting allocation tails or resizing.

Descriptor 0 reads the finite replay stream. Descriptors 1 and 2 append to
private output buffers. Each keeps that meaning until the program closes it;
the number then belongs to the delegated descriptor table, which reuses it for
the next file exactly as the kernel would. Matching-host filesystem delegation uses an owned guest
descriptor namespace and preserves kernel errors and partial transfers, with
Linux VFS transfers capped at 0x7fff0000 bytes. Versioned Linux libc uses the
same finite input, private output and captured backing with its size_t/ssize_t
ABI. Unsupported descriptors refuse before I/O. Input errors refuse without a
fabricated libc result or TLS errno update; successful and EOF reads leave that
state alone. The host explicitly assumes these implementations are equivalent;
checked library wrappers remain guest code.
*/

use std::io::Read;
use std::sync::{Arc, Mutex};

use calling_conventions::{CallSignature, ValueShape};
use runtime_abi::{DescriptorAbi, DescriptorOperation, NativeBorrowedStorageLayout};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryStorageBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalProgram, TerminalScalarValue,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, ByteSequenceCarrier,
    StructuralAccess, StructuralMultiplicity, StructuralTypeShape,
};

use super::TerminalRuntimeError;
use super::borrowed_storage::{self, CapturedStorage, StorageTransfer};
use super::buffered_output::BufferedStreams;

enum DescriptorBuffer {
    Scalar(ScalarType),
    Aggregate(NativeBorrowedStorageLayout),
    Bytes,
}

struct DescriptorContract {
    signature: TerminalImportSignature,
    operation: DescriptorOperation,
    abi: DescriptorAbi,
    buffer: DescriptorBuffer,
    descriptor_type: IntegerType,
    count_type: IntegerType,
    result_type: IntegerType,
}

pub(super) struct DescriptorIoImport<'input> {
    contract: DescriptorContract,
    input: Option<Arc<Mutex<Box<dyn Read + Send + 'input>>>>,
    output: BufferedStreams,
    linux_filesystem: Option<super::filesystem::LinuxFilesystemWrite>,
}

impl<'input> DescriptorIoImport<'input> {
    pub(super) fn admit(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        storage_layouts: &[NativeBorrowedStorageLayout],
        input: Option<Arc<Mutex<Box<dyn Read + Send + 'input>>>>,
        output: BufferedStreams,
        filesystem: &super::filesystem::FilesystemContext,
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        let mut adapter = Self::for_filesystem(
            program,
            declaration,
            binding,
            storage_layouts,
            input,
            output,
        )?;
        if let Some(adapter) = &mut adapter
            && matches!(adapter.contract.abi, DescriptorAbi::LinuxKernel)
            && matches!(
                adapter.contract.operation,
                DescriptorOperation::Write | DescriptorOperation::Read
            )
        {
            adapter.linux_filesystem = super::filesystem::LinuxFilesystemWrite::admit(
                filesystem,
                program,
                declaration,
                binding,
            );
        }
        if adapter.as_ref().is_some_and(|adapter| {
            matches!(adapter.contract.operation, DescriptorOperation::Read)
                && adapter.input.is_none()
        }) {
            return Err(TerminalRuntimeError::missing(
                "runtime-standard-input",
                "no finite replay stream was supplied",
            ));
        }
        Ok(adapter)
    }

    pub(super) fn for_filesystem(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        storage_layouts: &[NativeBorrowedStorageLayout],
        input: Option<Arc<Mutex<Box<dyn Read + Send + 'input>>>>,
        output: BufferedStreams,
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        let Some(contract) = admit_contract(program, declaration, binding, storage_layouts)? else {
            return Ok(None);
        };
        Ok(Some(Self {
            contract,
            input,
            output,
            linux_filesystem: None,
        }))
    }

    fn result(&self, count: i128) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let value = IntegerValue::Signed(count);
        if !self.contract.result_type.admits(value) {
            return Err(invalid_operands());
        }
        if self.contract.signature.result.is_unit() {
            return Ok(TerminalEffectResult::Unit);
        }
        Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: self.contract.result_type,
            value,
        }))
    }

    pub(super) fn validate_native_transfer(
        &self,
        arguments: &[TerminalScalarValue],
        buffers: &[TerminalBoundaryStorageBuffer],
    ) -> Result<(), TerminalEffectRejection> {
        let layout = match &self.contract.buffer {
            DescriptorBuffer::Scalar(_) => None,
            DescriptorBuffer::Aggregate(layout) => Some(layout),
            DescriptorBuffer::Bytes => return Ok(()),
        };
        let [
            _,
            TerminalScalarValue::Integer {
                scalar_type,
                value: IntegerValue::Unsigned(count),
            },
        ] = arguments
        else {
            return Err(invalid_operands());
        };
        if *scalar_type != self.contract.count_type {
            return Err(invalid_operands());
        }
        let count = usize::try_from(*count).map_err(|_| invalid_operands())?;
        validate_transfer_count(self.contract.abi, count)?;
        let [formal] = self.contract.signature.structural_parameters.as_slice() else {
            return Err(invalid_operands());
        };
        CapturedStorage::capture(
            0,
            buffers,
            formal.access,
            layout,
            count,
            match self.contract.operation {
                DescriptorOperation::Read => StorageTransfer::ReplacePrefix,
                DescriptorOperation::Write => StorageTransfer::Observe,
            },
        )?;
        Ok(())
    }
}

fn select_binding(
    binding: &NormalizedForeignCallBinding,
) -> Result<Option<(DescriptorAbi, DescriptorOperation)>, TerminalRuntimeError> {
    Ok(if let Some(system_call) = binding.system_call() {
        runtime_abi::descriptor_system_call(system_call.target, system_call.number)
    } else {
        binding
            .import()
            .and_then(|(locator, _)| runtime_abi::descriptor_import(locator))
    })
}

fn admit_contract(
    program: &TerminalProgram,
    declaration: &BoundaryMachineDeclaration,
    binding: &NormalizedForeignCallBinding,
    storage_layouts: &[NativeBorrowedStorageLayout],
) -> Result<Option<DescriptorContract>, TerminalRuntimeError> {
    let Some((abi, operation)) = select_binding(binding)? else {
        return Ok(None);
    };
    let invalid = super::unprovisioned(
        "foreign descriptor adapter differs from its exact admitted declaration/ABI",
    );
    let descriptor_type = IntegerType::new(IntegerSign::Signed, 32).map_err(|_| invalid())?;
    let width: u16 = match abi {
        DescriptorAbi::WindowsCrt => 32,
        DescriptorAbi::DarwinLibSystem | DescriptorAbi::LinuxLibc | DescriptorAbi::LinuxKernel => {
            64
        }
    };
    let count_type = IntegerType::new(IntegerSign::Unsigned, width).map_err(|_| invalid())?;
    let result_type = IntegerType::new(IntegerSign::Signed, width).map_err(|_| invalid())?;
    let signature = TerminalImportSignature::from(declaration);
    let [formal] = signature.structural_parameters.as_slice() else {
        return Err(invalid());
    };
    let buffer = match program
        .structural_type(formal.structural_type)
        .map(|row| &row.shape)
    {
        Some(StructuralTypeShape::PrimitiveScalar(scalar))
            if borrowed_storage::scalar_extent(*scalar).is_ok() =>
        {
            DescriptorBuffer::Scalar(*scalar)
        }
        Some(StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { access }))
            if matches!(
                abi,
                DescriptorAbi::DarwinLibSystem
                    | DescriptorAbi::LinuxLibc
                    | DescriptorAbi::LinuxKernel
            ) && access.is_none_or(|access| access == formal.access) =>
        {
            DescriptorBuffer::Bytes
        }
        Some(
            terminal_psi::record_or_fixed_array_shape_pattern!()
            | terminal_psi::sum_or_mixed_shape_pattern!(),
        ) => {
            let layout = storage_layouts
                .iter()
                .find(|layout| layout.structural_type == formal.structural_type)
                .ok_or_else(invalid)?;
            DescriptorBuffer::Aggregate(layout.clone())
        }
        _ => return Err(invalid()),
    };
    let access_supported = match operation {
        DescriptorOperation::Read => formal.access == StructuralAccess::MutableBorrow,
        DescriptorOperation::Write => {
            matches!(formal.access, terminal_psi::reading_borrow_pair_pattern!())
        }
    };
    let (scalar_parameters, parameter_order, native_parameters) = match &buffer {
        DescriptorBuffer::Scalar(_) | DescriptorBuffer::Aggregate(_) => (
            vec![
                ScalarType::Integer(descriptor_type),
                ScalarType::Integer(count_type),
            ],
            vec![
                BoundaryParameterKind::Scalar,
                BoundaryParameterKind::Structural,
                BoundaryParameterKind::Scalar,
            ],
            vec![
                ValueShape::integer(4, 4),
                ValueShape::integer(8, 8),
                ValueShape::integer(u32::from(width / 8), width / 8),
            ],
        ),
        DescriptorBuffer::Bytes => (
            vec![ScalarType::Integer(descriptor_type)],
            vec![
                BoundaryParameterKind::Scalar,
                BoundaryParameterKind::Structural,
            ],
            vec![ValueShape::integer(4, 4), ValueShape::integer(16, 8)],
        ),
    };
    let native_signature = CallSignature {
        parameters: native_parameters,
        result: if signature.result.is_unit() {
            None
        } else {
            Some(ValueShape::integer(u32::from(width / 8), width / 8))
        },
    };
    if signature.scalar_parameters != scalar_parameters
        || (!signature.result.is_unit()
            && signature.result
                != BoundaryMachineResult::Scalar(ScalarType::Integer(result_type).into()))
        || declaration.parameter_order != parameter_order
        || !access_supported
        || formal.multiplicity != StructuralMultiplicity::Unrestricted
        || !formal.qualifications.is_empty()
        || !formal.projected_qualifications.is_empty()
    {
        return Err(invalid());
    }
    super::admit_exact_foreign_call(&signature, declaration, binding, &native_signature)?;

    Ok(Some(DescriptorContract {
        signature,
        operation,
        abi,
        buffer,
        descriptor_type,
        count_type,
        result_type,
    }))
}

fn validate_transfer_count(
    abi: DescriptorAbi,
    count: usize,
) -> Result<(), TerminalEffectRejection> {
    if matches!(abi, DescriptorAbi::LinuxKernel | DescriptorAbi::LinuxLibc) && count > 0x7fff0000 {
        return Err(TerminalEffectRejection::new(
            "transfer count requires the selected kernel page-size limit",
        ));
    }
    Ok(())
}

fn invalid_operands() -> TerminalEffectRejection {
    TerminalEffectRejection::new("descriptor I/O operands differ from the installed contract")
}

impl TerminalImportAdapter for DescriptorIoImport<'_> {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.contract.signature.clone()
    }
}

impl TerminalEffectHandler for DescriptorIoImport<'_> {
    fn storage_requirements(
        &self,
        _boundary: semantic_vocabulary::BoundaryMachineId,
        _arguments: &[terminal_interpreter::TerminalScalarValue],
    ) -> terminal_interpreter::TerminalCallStorageRequirements<'_> {
        terminal_interpreter::TerminalCallStorageRequirements {
            original: false,
            arguments: match self.contract.buffer {
                DescriptorBuffer::Aggregate(_) => &[0],
                _ => &[],
            },
        }
    }

    handle_effect_requires_dispatch!("descriptor I/O requires authenticated buffer dispatch");

    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
        storage_buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let call = super::call_operands(effect, invalid_operands)?;
        let Some(TerminalScalarValue::Integer {
            scalar_type,
            value: IntegerValue::Signed(descriptor),
        }) = call.arguments.first()
        else {
            return Err(invalid_operands());
        };
        let [formal] = self.contract.signature.structural_parameters.as_slice() else {
            return Err(invalid_operands());
        };
        if *scalar_type != self.contract.descriptor_type
            || !self
                .contract
                .descriptor_type
                .admits(IntegerValue::Signed(*descriptor))
            || call.structural_arguments.len() != 1
            || call.structural_arguments[0].structural_type != formal.structural_type
        {
            return Err(invalid_operands());
        }
        let mut captured = None;
        let (bytes, count) = match &self.contract.buffer {
            DescriptorBuffer::Bytes => {
                let [Some(bytes)] = call.byte_sequence_arguments else {
                    return Err(invalid_operands());
                };
                if call.arguments.len() != 1 || !storage_buffers.is_empty() {
                    return Err(invalid_operands());
                }
                match formal.access {
                    StructuralAccess::SharedBorrow if byte_buffers.is_empty() => {}
                    StructuralAccess::MutableBorrow => {
                        let [buffer] = byte_buffers else {
                            return Err(invalid_operands());
                        };
                        if buffer.argument_index() != 0
                            || buffer.bytes() != bytes.as_slice()
                            || u128::from(buffer.capacity()) != bytes.len() as u128
                        {
                            return Err(invalid_operands());
                        }
                    }
                    _ => return Err(invalid_operands()),
                }
                validate_transfer_count(self.contract.abi, bytes.len())?;
                (bytes.as_slice(), bytes.len())
            }
            DescriptorBuffer::Scalar(_) | DescriptorBuffer::Aggregate(_) => {
                let [
                    _,
                    TerminalScalarValue::Integer {
                        scalar_type,
                        value: IntegerValue::Unsigned(count),
                    },
                ] = call.arguments
                else {
                    return Err(invalid_operands());
                };
                if !byte_buffers.is_empty()
                    || *scalar_type != self.contract.count_type
                    || !self
                        .contract
                        .count_type
                        .admits(IntegerValue::Unsigned(*count))
                    || !matches!(call.byte_sequence_arguments, [None])
                {
                    return Err(invalid_operands());
                }
                let layout = match &self.contract.buffer {
                    DescriptorBuffer::Aggregate(layout) => Some(layout),
                    DescriptorBuffer::Scalar(scalar) => {
                        let [TerminalBoundaryStorageBuffer::Scalar(buffer)] = storage_buffers
                        else {
                            return Err(invalid_operands());
                        };
                        if buffer.value().scalar_type() != *scalar {
                            return Err(invalid_operands());
                        }
                        None
                    }
                    _ => return Err(invalid_operands()),
                };
                let count = usize::try_from(*count).map_err(|_| invalid_operands())?;
                validate_transfer_count(self.contract.abi, count)?;
                captured = Some(CapturedStorage::capture(
                    0,
                    storage_buffers,
                    formal.access,
                    layout,
                    count,
                    match self.contract.operation {
                        DescriptorOperation::Read => StorageTransfer::ReplacePrefix,
                        DescriptorOperation::Write => StorageTransfer::Observe,
                    },
                )?);
                let storage = captured.as_ref().ok_or_else(invalid_operands)?;
                (storage.bytes.as_slice(), count)
            }
        };
        let result = self.result(i128::try_from(count).map_err(|_| invalid_operands())?)?;
        match self.contract.operation {
            DescriptorOperation::Write => {
                if !matches!(*descriptor, 1 | 2)
                    && matches!(self.contract.abi, DescriptorAbi::WindowsCrt)
                {
                    return self.result(-1);
                }
                if let Some(filesystem) = &self.linux_filesystem
                    && !(matches!(*descriptor, 1 | 2) && filesystem.models_standard(*descriptor))
                {
                    let descriptor = i32::try_from(*descriptor).map_err(|_| invalid_operands())?;
                    return self.result(filesystem.write(descriptor, &bytes[..count])?);
                }
                self.output.append(*descriptor, &bytes[..count])?;
                Ok(result)
            }
            DescriptorOperation::Read => {
                let linux_file = matches!(self.contract.abi, DescriptorAbi::LinuxKernel)
                    && self.linux_filesystem.as_ref().is_some_and(|filesystem| {
                        *descriptor != 0 || !filesystem.models_standard(*descriptor)
                    });
                if *descriptor != 0 && !linux_file {
                    if !matches!(self.contract.abi, DescriptorAbi::WindowsCrt) {
                        return Err(TerminalEffectRejection::new(
                            "input descriptor is not the admitted finite stream",
                        ));
                    }
                    return self.result(-1);
                }
                if linux_file {
                    let descriptor = i32::try_from(*descriptor).map_err(|_| invalid_operands())?;
                    let kind = match &self.contract.buffer {
                        DescriptorBuffer::Bytes => true,
                        DescriptorBuffer::Scalar(_) | DescriptorBuffer::Aggregate(_) => false,
                    };
                    let status = {
                        let destination = if kind {
                            let [buffer] = byte_buffers else {
                                return Err(invalid_operands());
                            };
                            let bytes = buffer.bytes_mut();
                            if bytes.len() != count {
                                return Err(invalid_operands());
                            }
                            bytes
                        } else {
                            let storage = captured.as_mut().ok_or_else(invalid_operands)?;
                            if storage.bytes.len() < count {
                                return Err(invalid_operands());
                            }
                            &mut storage.bytes[..count]
                        };
                        self.linux_filesystem
                            .as_ref()
                            .ok_or_else(invalid_operands)?
                            .read(descriptor, destination)?
                    };
                    if status > 0 {
                        let moved = usize::try_from(status).map_err(|_| invalid_operands())?;
                        if moved > count {
                            return Err(TerminalEffectRejection::new(
                                "kernel read exceeded the requested byte extent",
                            ));
                        }
                        if let Some(storage) = captured {
                            storage.stage(storage_buffers, moved)?;
                        }
                    }
                    return self.result(status);
                }
                if count == 0 {
                    return Ok(result);
                }
                let input = self.input.as_ref().ok_or_else(invalid_operands)?;
                let mut input = input
                    .lock()
                    .map_err(|_| TerminalEffectRejection::new("stdin snapshot lock is poisoned"))?;
                let destination = match &self.contract.buffer {
                    DescriptorBuffer::Scalar(_) | DescriptorBuffer::Aggregate(_) => {
                        &mut captured.as_mut().ok_or_else(invalid_operands)?.bytes[..count]
                    }
                    DescriptorBuffer::Bytes => {
                        let [buffer] = byte_buffers else {
                            return Err(invalid_operands());
                        };
                        buffer.bytes_mut()
                    }
                };
                let moved = match input.read(destination) {
                    Ok(moved) if moved <= count => moved,
                    Ok(_) => {
                        return Err(TerminalEffectRejection::new(
                            "input reader exceeded the requested byte extent",
                        ));
                    }
                    Err(_)
                        if matches!(
                            self.contract.abi,
                            DescriptorAbi::WindowsCrt | DescriptorAbi::DarwinLibSystem
                        ) =>
                    {
                        return self.result(-1);
                    }
                    Err(_) => {
                        return Err(TerminalEffectRejection::new(
                            if matches!(self.contract.abi, DescriptorAbi::LinuxLibc) {
                                "finite input failed without an admitted libc errno result"
                            } else {
                                "finite input failed without an admitted raw kernel error status"
                            },
                        ));
                    }
                };
                drop(input);
                let result = self.result(i128::try_from(moved).map_err(|_| invalid_operands())?)?;
                if moved != 0
                    && let Some(storage) = captured
                {
                    storage.stage(storage_buffers, moved)?;
                }
                Ok(result)
            }
        }
    }
}
