//! Explicit trusted implementations of the MSVCRT descriptor calls over this
//! runtime's finite input and private output, not a compiler semantic rewrite.
//!
//! The host offers two concrete implementations below. Their complete normalized
//! locator, target, evaluated ABI and demanded declaration must match an already
//! admitted native settlement before either is installed in that exact slot.
//! This join checks compatibility; it does not prove foreign behavior or grant
//! authority from an export spelling. The receiving runtime explicitly assumes
//! semantic equivalence under the embedding contract. Unsupported contracts
//! decline before effects. Std's checked byte/line wrappers remain guest code.

use std::io::Read;
use std::sync::{Arc, Mutex};

use calling_conventions::{CallSignature, ValueShape, validate_boundary_entry_plan};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryScalarBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalProgram, TerminalScalarValue,
};
use terminal_psi::{
    BoundaryCompletion, BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
    StructuralAccess, StructuralMultiplicity, StructuralTypeShape,
};

use super::TerminalRuntimeError;
use super::buffered_output::BufferedBytes;

#[derive(Clone, Copy)]
enum DescriptorOperation {
    Read,
    Write,
}

impl DescriptorOperation {
    fn locator(self) -> Result<target::NormalizedForeignLocator, TerminalRuntimeError> {
        target::normalize_foreign_locator(
            target::ForeignLocatorCandidate::PeByName {
                library: b"msvcrt.dll".to_vec(),
                export: match self {
                    Self::Read => b"_read".to_vec(),
                    Self::Write => b"_write".to_vec(),
                },
            },
            target::TargetProfile::WindowsX64,
        )
        .map_err(|_| {
            TerminalRuntimeError::InvalidProvision(
                "host descriptor implementation has an invalid locator",
            )
        })
    }
}

pub(super) struct DescriptorIoImport<'input> {
    signature: TerminalImportSignature,
    operation: DescriptorOperation,
    input: Option<Arc<Mutex<Box<dyn Read + Send + 'input>>>>,
    output: BufferedBytes,
    signed: IntegerType,
    unsigned: IntegerType,
}

impl<'input> DescriptorIoImport<'input> {
    pub(super) fn admit(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        input: Option<Arc<Mutex<Box<dyn Read + Send + 'input>>>>,
        output: BufferedBytes,
    ) -> Result<Self, TerminalRuntimeError> {
        let invalid = || {
            TerminalRuntimeError::InvalidProvision(
                "foreign descriptor adapter differs from its exact admitted declaration/ABI",
            )
        };
        let (locator, _) = binding.import().ok_or_else(invalid)?;
        // These are host policy choices, not behavior inferred from an arbitrary
        // import's text. A lookalike locator, ordinal, syscall or ABI gets none.
        let mut operation = None;
        for offered in [DescriptorOperation::Read, DescriptorOperation::Write] {
            if offered.locator()? == *locator {
                operation = Some(offered);
                break;
            }
        }
        let operation = operation.ok_or_else(|| {
            TerminalRuntimeError::missing(
                "runtime-host-import-bindings",
                "no explicitly trusted host implementation matches this complete foreign locator",
            )
        })?;
        let signed = IntegerType::new(IntegerSign::Signed, 32).map_err(|_| invalid())?;
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 32).map_err(|_| invalid())?;
        let signature = TerminalImportSignature::from(declaration);
        let [buffer] = signature.structural_parameters.as_slice() else {
            return Err(invalid());
        };
        let access_supported = match operation {
            DescriptorOperation::Read => buffer.access == StructuralAccess::MutableBorrow,
            DescriptorOperation::Write => matches!(
                buffer.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            ),
        };
        let native_signature = CallSignature {
            parameters: vec![
                ValueShape::integer(4, 4),
                // Foreign policy classifies the materialized reference as its
                // target pointer carrier. The portable declaration separately
                // fixes the borrowed referent to one i32 cell below.
                ValueShape::integer(8, 8),
                ValueShape::integer(4, 4),
            ],
            result: Some(ValueShape::integer(4, 4)),
        };
        if locator.target() != target::TargetProfile::WindowsX64
            || !binding.has_valid_entry_mechanism()
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
            || signature.scalar_parameters
                != [ScalarType::Integer(signed), ScalarType::Integer(unsigned)]
            || signature.result != BoundaryMachineResult::Scalar(ScalarType::Integer(signed))
            || declaration.parameter_order
                != [
                    BoundaryParameterKind::Scalar,
                    BoundaryParameterKind::Structural,
                    BoundaryParameterKind::Scalar,
                ]
            || !access_supported
            || buffer.multiplicity != StructuralMultiplicity::Unrestricted
            || !buffer.qualifications.is_empty()
            || !buffer.projected_qualifications.is_empty()
            || !matches!(program.structural_type(buffer.structural_type).map(|row| &row.shape),
                Some(StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))) if *integer == signed)
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
            input,
            output,
            signed,
            unsigned,
        })
    }

    fn result(&self, count: i32) -> TerminalEffectResult {
        TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: self.signed,
            value: IntegerValue::Signed(i128::from(count)),
        })
    }
}

impl TerminalImportAdapter for DescriptorIoImport<'_> {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalEffectHandler for DescriptorIoImport<'_> {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "descriptor I/O requires authenticated scalar-loan dispatch",
        ))
    }

    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
        scalar_buffers: &mut [TerminalBoundaryScalarBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let invalid = || {
            TerminalEffectRejection::new(
                "descriptor I/O operands differ from the installed contract",
            )
        };
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            return Err(invalid());
        };
        let [
            TerminalScalarValue::Integer {
                scalar_type: descriptor_type,
                value: IntegerValue::Signed(descriptor),
            },
            TerminalScalarValue::Integer {
                scalar_type: count_type,
                value: IntegerValue::Unsigned(count),
            },
        ] = arguments.as_slice()
        else {
            return Err(invalid());
        };
        let [buffer] = scalar_buffers else {
            return Err(invalid());
        };
        let [formal] = self.signature.structural_parameters.as_slice() else {
            return Err(invalid());
        };
        let TerminalScalarValue::Integer {
            scalar_type: cell_type,
            value: IntegerValue::Signed(cell),
        } = buffer.value()
        else {
            return Err(invalid());
        };
        if !byte_buffers.is_empty()
            || structural_arguments.len() != 1
            || *descriptor_type != self.signed
            || *count_type != self.unsigned
            || cell_type != self.signed
            || buffer.argument_index() != 0
            || buffer.access() != formal.access
        {
            return Err(invalid());
        }
        let count = usize::try_from(*count).map_err(|_| invalid())?;
        let mut bytes = i32::try_from(cell).map_err(|_| invalid())?.to_le_bytes();
        if count > bytes.len() {
            return Err(TerminalEffectRejection::new(
                "descriptor byte count exceeds the exact borrowed scalar extent",
            ));
        }
        let expected_descriptor = match self.operation {
            DescriptorOperation::Read => 0,
            DescriptorOperation::Write => 1,
        };
        if *descriptor != expected_descriptor {
            return Ok(self.result(-1));
        }
        if count == 0 {
            return Ok(self.result(0));
        }
        match self.operation {
            DescriptorOperation::Write => {
                self.output.append(&bytes[..count])?;
                Ok(self.result(i32::try_from(count).map_err(|_| invalid())?))
            }
            DescriptorOperation::Read => {
                let input = self.input.as_ref().ok_or_else(invalid)?;
                let mut input = input
                    .lock()
                    .map_err(|_| TerminalEffectRejection::new("stdin snapshot lock is poisoned"))?;
                // One foreign read attempt. Failures return -1, not a fabricated
                // Crash: the checked read_byte body's conversion owns its Trap.
                let mut transferred = [0u8; 4];
                let moved = match input.read(&mut transferred[..count]) {
                    Ok(moved) if moved <= count => moved,
                    Ok(_) => {
                        return Err(TerminalEffectRejection::new(
                            "input reader exceeded the requested byte extent",
                        ));
                    }
                    Err(_) => return Ok(self.result(-1)),
                };
                drop(input);
                bytes[..moved].copy_from_slice(&transferred[..moved]);
                // MSVCRT writes only the transferred low bytes. Preserve every
                // untouched byte of the actual little-endian i32 cell.
                buffer.replace(TerminalScalarValue::Integer {
                    scalar_type: self.signed,
                    value: IntegerValue::Signed(i128::from(i32::from_le_bytes(bytes))),
                })?;
                Ok(self.result(i32::try_from(moved).map_err(|_| invalid())?))
            }
        }
    }
}
