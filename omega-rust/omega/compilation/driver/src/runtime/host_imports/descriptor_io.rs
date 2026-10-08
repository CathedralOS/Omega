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

use super::TerminalRuntimeError;
use super::buffered_output::BufferedStreams;
use calling_conventions::{CallSignature, ValueShape, validate_boundary_entry_plan};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use std::io::Read;
use std::sync::{Arc, Mutex};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryScalarBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalProgram, TerminalScalarValue,
};
use terminal_psi::{
    BoundaryCompletion, BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralTypeShape,
};

mod captured_storage;
use captured_storage::CapturedStorage;

#[derive(Clone, Copy)]
enum DescriptorOperation {
    Read,
    Write,
}
#[derive(Clone, Copy)]
enum DescriptorImplementation {
    WindowsRead,
    WindowsWrite,
    DarwinRead,
    DarwinWrite,
}
impl DescriptorImplementation {
    fn operation(self) -> DescriptorOperation {
        match self {
            Self::WindowsRead | Self::DarwinRead => DescriptorOperation::Read,
            _ => DescriptorOperation::Write,
        }
    }
    fn locator(self) -> Result<target::NormalizedForeignLocator, TerminalRuntimeError> {
        let (candidate, target) = match self {
            Self::DarwinRead | Self::DarwinWrite => (
                target::ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                    symbol: match self {
                        Self::DarwinRead => b"_read".to_vec(),
                        _ => b"_write".to_vec(),
                    },
                },
                target::TargetProfile::MacosArm64,
            ),
            Self::WindowsRead | Self::WindowsWrite => (
                target::ForeignLocatorCandidate::PeByName {
                    library: b"msvcrt.dll".to_vec(),
                    export: match self {
                        Self::WindowsRead => b"_read".to_vec(),
                        _ => b"_write".to_vec(),
                    },
                },
                target::TargetProfile::WindowsX64,
            ),
        };
        target::normalize_foreign_locator(candidate, target).map_err(|_| {
            TerminalRuntimeError::InvalidProvision(
                "host descriptor implementation has an invalid locator",
            )
        })
    }
}
enum DescriptorBuffer {
    Scalar(ScalarType),
    Aggregate(NativeBorrowedStorageLayout),
    Bytes,
}

pub(super) struct DescriptorIoImport<'input> {
    signature: TerminalImportSignature,
    operation: DescriptorOperation,
    implementation: DescriptorImplementation,
    buffer: DescriptorBuffer,
    input: Option<Arc<Mutex<Box<dyn Read + Send + 'input>>>>,
    output: BufferedStreams,
    descriptor_type: IntegerType,
    count_type: IntegerType,
    result_type: IntegerType,
}
impl<'input> DescriptorIoImport<'input> {
    pub(super) fn admit(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        storage_layouts: &[NativeBorrowedStorageLayout],
        input: Option<Arc<Mutex<Box<dyn Read + Send + 'input>>>>,
        output: BufferedStreams,
    ) -> Result<Self, TerminalRuntimeError> {
        let invalid = || {
            TerminalRuntimeError::InvalidProvision(
                "foreign descriptor adapter differs from its exact admitted declaration/ABI",
            )
        };
        let (locator, _) = binding.import().ok_or_else(invalid)?;
        // Explicit host offers, never arbitrary export-name/source recognition.
        let mut implementation = None;
        for offered in [
            DescriptorImplementation::WindowsRead,
            DescriptorImplementation::WindowsWrite,
            DescriptorImplementation::DarwinRead,
            DescriptorImplementation::DarwinWrite,
        ] {
            if offered.locator()? == *locator {
                implementation = Some(offered);
                break;
            }
        }
        let implementation = implementation.ok_or_else(|| {
            TerminalRuntimeError::missing(
                "runtime-host-import-bindings",
                "no trusted host implementation matches this complete foreign locator",
            )
        })?;
        let operation = implementation.operation();
        let descriptor_type = IntegerType::new(IntegerSign::Signed, 32).map_err(|_| invalid())?;
        let width: u16 = match implementation {
            DescriptorImplementation::DarwinRead | DescriptorImplementation::DarwinWrite => 64,
            _ => 32,
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
                if captured_storage::scalar_extent(*scalar).is_ok() =>
            {
                DescriptorBuffer::Scalar(*scalar)
            }
            Some(StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access,
            })) if matches!(
                implementation,
                DescriptorImplementation::DarwinRead | DescriptorImplementation::DarwinWrite
            ) && access.is_none_or(|access| access == formal.access) =>
            {
                DescriptorBuffer::Bytes
            }
            Some(StructuralTypeShape::Record { .. } | StructuralTypeShape::FixedArray { .. }) => {
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
            DescriptorOperation::Write => matches!(
                formal.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            ),
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
            result: Some(ValueShape::integer(u32::from(width / 8), width / 8)),
        };
        if !binding.has_valid_entry_mechanism()
            || binding.requirement_identity() != declaration.identity
            || binding.provider_plan_report_identity() == 0
            || !binding
                .boundary_entry_plan
                .call
                .callback_materializations
                .is_empty()
            || validate_boundary_entry_plan(binding.boundary_entry_plan.clone(), &native_signature)
                .is_err()
            || signature.completion != BoundaryCompletion::NormalReturn
            || signature.scalar_parameters != scalar_parameters
            || signature.result != BoundaryMachineResult::Scalar(ScalarType::Integer(result_type))
            || declaration.parameter_order != parameter_order
            || !access_supported
            || formal.multiplicity != StructuralMultiplicity::Unrestricted
            || !formal.qualifications.is_empty()
            || !formal.projected_qualifications.is_empty()
            || !declaration.requires.is_empty()
            || !declaration.scalar_requires.is_empty()
            || !declaration.scalar_ensures.is_empty()
            || !declaration.content_guarantees.is_empty()
            || !declaration.program_local_root_introductions.is_empty()
            || !declaration.crash_routes.is_empty()
        {
            return Err(invalid());
        }
        if matches!(operation, DescriptorOperation::Read) && input.is_none() {
            return Err(TerminalRuntimeError::missing(
                "runtime-standard-input",
                "no finite replay stream was supplied",
            ));
        }
        Ok(Self {
            signature,
            operation,
            implementation,
            buffer,
            input,
            output,
            descriptor_type,
            count_type,
            result_type,
        })
    }
    fn result(&self, count: i128) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let value = IntegerValue::Signed(count);
        if !self.result_type.admits(value) {
            return Err(invalid_operands());
        }
        Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: self.result_type,
            value,
        }))
    }
}
fn invalid_operands() -> TerminalEffectRejection {
    TerminalEffectRejection::new("descriptor I/O operands differ from the installed contract")
}
impl TerminalImportAdapter for DescriptorIoImport<'_> {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}
impl TerminalEffectHandler for DescriptorIoImport<'_> {
    fn scalar_storage_arguments(
        &self,
        _boundary: semantic_vocabulary::BoundaryMachineId,
    ) -> &[usize] {
        match self.buffer {
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
        scalar_buffers: &mut [TerminalBoundaryScalarBuffer],
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
        let [formal] = self.signature.structural_parameters.as_slice() else {
            return Err(invalid_operands());
        };
        if *scalar_type != self.descriptor_type
            || !self
                .descriptor_type
                .admits(IntegerValue::Signed(*descriptor))
            || structural_arguments.len() != 1
            || structural_arguments[0].structural_type != formal.structural_type
        {
            return Err(invalid_operands());
        }
        let mut captured = None;
        let (bytes, count) = match &self.buffer {
            DescriptorBuffer::Bytes => {
                let [Some(bytes)] = byte_sequence_arguments.as_slice() else {
                    return Err(invalid_operands());
                };
                if arguments.len() != 1 || !scalar_buffers.is_empty() {
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
                    || *scalar_type != self.count_type
                    || !self.count_type.admits(IntegerValue::Unsigned(*count))
                    || !matches!(byte_sequence_arguments.as_slice(), [None])
                {
                    return Err(invalid_operands());
                }
                let layout = match &self.buffer {
                    DescriptorBuffer::Aggregate(layout) => Some(layout),
                    DescriptorBuffer::Scalar(scalar) => {
                        let [buffer] = scalar_buffers else {
                            return Err(invalid_operands());
                        };
                        if buffer.value().scalar_type() != *scalar {
                            return Err(invalid_operands());
                        }
                        None
                    }
                    _ => return Err(invalid_operands()),
                };
                captured = Some(CapturedStorage::capture(
                    scalar_buffers,
                    formal.access,
                    layout,
                )?);
                let storage = captured.as_ref().ok_or_else(invalid_operands)?;
                let count = usize::try_from(*count).map_err(|_| invalid_operands())?;
                // A logical integer formal can observe a narrower captured
                // codec. Only its authenticated physical bytes are offered;
                // the formal's width grants neither padding nor adjacent data.
                let extent = storage.bytes.len();
                if count > extent {
                    return Err(TerminalEffectRejection::new(
                        "descriptor byte count exceeds the exact borrowed storage extent",
                    ));
                }
                (storage.bytes.as_slice(), count)
            }
        };
        let result = self.result(i128::try_from(count).map_err(|_| invalid_operands())?)?;
        match self.operation {
            DescriptorOperation::Write => {
                // Count/result validation precedes append; even mutable scalar
                // inputs are read without replacing their captured payload.
                if !matches!(*descriptor, 1 | 2)
                    && matches!(self.implementation, DescriptorImplementation::WindowsWrite)
                {
                    return self.result(-1);
                }
                self.output.append(*descriptor, &bytes[..count])?;
                Ok(result)
            }
            DescriptorOperation::Read => {
                if *descriptor != 0 {
                    if matches!(self.implementation, DescriptorImplementation::DarwinRead) {
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
                let destination = match &self.buffer {
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
                    Err(_) => return self.result(-1),
                };
                drop(input);
                let result = self.result(i128::try_from(moved).map_err(|_| invalid_operands())?)?;
                if moved != 0
                    && let Some(storage) = captured
                {
                    storage.stage(scalar_buffers)?;
                }
                Ok(result)
            }
        }
    }
}
