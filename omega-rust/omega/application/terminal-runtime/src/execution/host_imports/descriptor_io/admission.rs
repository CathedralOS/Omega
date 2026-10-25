//! Exact descriptor offers and their complete declaration/ABI admission.
//! Native placement and semantic buffer permission are separate checks. A
//! pointer-sized carrier never authorizes guessed backing, padding or tails.
//! Linux syscall numbers are target data (syscall_64.tbl and asm-generic/unistd.h),
//! not names of source requirements. The normalized syscall ABI currently admits
//! only one-word parameters, so borrowed pointer/length aggregates stay unavailable
//! there rather than receiving a made-up register split.

use calling_conventions::{CallSignature, ValueShape};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{TerminalImportSignature, TerminalProgram};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, ByteSequenceCarrier,
    StructuralAccess, StructuralMultiplicity, StructuralTypeShape,
};

use super::super::borrowed_storage;
use super::{DescriptorAbi, DescriptorBuffer, DescriptorOperation, TerminalRuntimeError};

pub(super) struct DescriptorContract {
    pub signature: TerminalImportSignature,
    pub operation: DescriptorOperation,
    pub abi: DescriptorAbi,
    pub buffer: DescriptorBuffer,
    pub descriptor_type: IntegerType,
    pub count_type: IntegerType,
    pub result_type: IntegerType,
}

fn select(
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

pub(super) fn admit(
    program: &TerminalProgram,
    declaration: &BoundaryMachineDeclaration,
    binding: &NormalizedForeignCallBinding,
    storage_layouts: &[NativeBorrowedStorageLayout],
) -> Result<Option<DescriptorContract>, TerminalRuntimeError> {
    let Some((abi, operation)) = select(binding)? else {
        return Ok(None);
    };
    let invalid = || {
        TerminalRuntimeError::InvalidProvision(
            "foreign descriptor adapter differs from its exact admitted declaration/ABI",
        )
    };
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
            // The selected 64-bit C plan carries this exact pair as buffer
            // pointer and size_t. Windows CRT has a 32-bit count; a syscall
            // admits one-word authored operands instead of this C pair.
            if matches!(abi, DescriptorAbi::DarwinLibSystem | DescriptorAbi::LinuxLibc)
                && access.is_none_or(|access| access == formal.access) =>
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
    super::super::admit_exact_foreign_call(&signature, declaration, binding, &native_signature)?;

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
