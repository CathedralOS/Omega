//! Trusted descriptor implementations over finite input and private streams.
//! Exact normalized locators, declarations and target ABI authenticate
//! compatibility, not semantic authority inferred from export names. Scalar
//! leaves expose original backing through one capture/writeback protocol. Native
//! lowering retains record/array placement; the adapter never guesses offsets
//! or manufactures padding. Views supply their exact live range
//! and retained pointer/length ABI. Mutable views stage fixed-extent writes into
//! the original backing. Neither protocol grants allocation tails or resizing.
//! The host explicitly assumes equivalence of these offered implementations;
//! checked library wrappers remain guest code, and no real host I/O occurs here.
//! The same transfer serves retained or discarded counts. Linux kernel errors
//! are not libc errno: unknown descriptors and host replay failures reject,
//! rather than manufacturing a raw kernel status or touching OS descriptors.

use std::io::Read;
use std::sync::{Arc, Mutex};

use runtime_abi::{DescriptorAbi, DescriptorOperation, NativeBorrowedStorageLayout};
use semantic_vocabulary::{IntegerValue, ScalarType};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryStorageBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalProgram, TerminalScalarValue,
};
use terminal_psi::{BoundaryMachineDeclaration, StructuralAccess};

use super::TerminalRuntimeError;
use super::borrowed_storage::{CapturedStorage, StorageTransfer};
use super::buffered_output::BufferedStreams;

mod admission;

enum DescriptorBuffer {
    Scalar(ScalarType),
    Aggregate(NativeBorrowedStorageLayout),
    Bytes,
}

pub(super) struct DescriptorIoImport<'input> {
    contract: admission::DescriptorContract,
    input: Option<Arc<Mutex<Box<dyn Read + Send + 'input>>>>,
    output: BufferedStreams,
}

impl<'input> DescriptorIoImport<'input> {
    /// Absence is an unoffered binding, not an incompatible offered contract.
    /// Missing finite input rejects during preflight, before any guest effects.
    pub(super) fn admit(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        storage_layouts: &[NativeBorrowedStorageLayout],
        input: Option<Arc<Mutex<Box<dyn Read + Send + 'input>>>>,
        output: BufferedStreams,
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        let Some(contract) = admission::admit(program, declaration, binding, storage_layouts)?
        else {
            return Ok(None);
        };
        if matches!(contract.operation, DescriptorOperation::Read) && input.is_none() {
            return Err(TerminalRuntimeError::missing(
                "runtime-standard-input",
                "no finite replay stream was supplied",
            ));
        }
        Ok(Some(Self {
            contract,
            input,
            output,
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
    fn storage_arguments(&self, _boundary: semantic_vocabulary::BoundaryMachineId) -> &[usize] {
        match self.contract.buffer {
            DescriptorBuffer::Aggregate(_) => &[0],
            _ => &[],
        }
    }
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "descriptor I/O requires authenticated buffer dispatch",
        ))
    }
    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
        storage_buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            byte_sequence_arguments,
            ..
        } = effect
        else {
            return Err(invalid_operands());
        };
        let Some(TerminalScalarValue::Integer {
            scalar_type,
            value: IntegerValue::Signed(descriptor),
        }) = arguments.first()
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
            || structural_arguments.len() != 1
            || structural_arguments[0].structural_type != formal.structural_type
        {
            return Err(invalid_operands());
        }
        let mut captured = None;
        let (bytes, count) = match &self.contract.buffer {
            DescriptorBuffer::Bytes => {
                let [Some(bytes)] = byte_sequence_arguments.as_slice() else {
                    return Err(invalid_operands());
                };
                if arguments.len() != 1 || !storage_buffers.is_empty() {
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
                (bytes.as_slice(), bytes.len())
            }
            DescriptorBuffer::Scalar(_) | DescriptorBuffer::Aggregate(_) => {
                let [
                    _,
                    TerminalScalarValue::Integer {
                        scalar_type,
                        value: IntegerValue::Unsigned(count),
                    },
                ] = arguments.as_slice()
                else {
                    return Err(invalid_operands());
                };
                if !byte_buffers.is_empty()
                    || *scalar_type != self.contract.count_type
                    || !self
                        .contract
                        .count_type
                        .admits(IntegerValue::Unsigned(*count))
                    || !matches!(byte_sequence_arguments.as_slice(), [None])
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
                // Linux VFS caps transfers at INT_MAX & PAGE_MASK. Without a
                // selected kernel page-size model, stay below the common limit
                // for supported 4K/16K/64K pages. Larger requests need that model;
                // never silently report an impossible full-count write. This is
                // interpreter availability, not a compiler count restriction.
                if matches!(self.contract.abi, DescriptorAbi::LinuxKernel) && count > 0x7fff0000 {
                    return Err(TerminalEffectRejection::new(
                        "transfer count requires the selected kernel page-size limit",
                    ));
                }
                captured = Some(CapturedStorage::capture(
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
                // A logical integer formal can observe a narrower captured
                // codec. Only its authenticated physical bytes are offered;
                // the formal's width grants neither padding nor adjacent data.
                (storage.bytes.as_slice(), count)
            }
        };
        let result = self.result(i128::try_from(count).map_err(|_| invalid_operands())?)?;
        match self.contract.operation {
            DescriptorOperation::Write => {
                // Count/result validation precedes append; even mutable scalar
                // inputs are read without replacing their captured payload.
                if !matches!(*descriptor, 1 | 2)
                    && matches!(self.contract.abi, DescriptorAbi::WindowsCrt)
                {
                    return self.result(-1);
                }
                self.output.append(*descriptor, &bytes[..count])?;
                Ok(result)
            }
            DescriptorOperation::Read => {
                if *descriptor != 0 {
                    if !matches!(self.contract.abi, DescriptorAbi::WindowsCrt) {
                        return Err(TerminalEffectRejection::new(
                            "input descriptor is not the admitted finite stream",
                        ));
                    }
                    return self.result(-1);
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
                            "finite input failed without an admitted raw kernel error status",
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
