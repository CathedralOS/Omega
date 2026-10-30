//! Locators select known C operations; exact declarations and complete ABI
//! applications determine whether their values can cross this particular slot.

use super::{AllocationOperation, TerminalRuntimeError};
use calling_conventions::{BoundaryCallingApplication, CallSignature, ValueShape};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, IntegerSign, IntegerType, ScalarType,
};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{TerminalImportSignature, TerminalProgram};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralAccess,
};

pub(super) fn select(
    binding: &NormalizedForeignCallBinding,
) -> Result<Option<AllocationOperation>, TerminalRuntimeError> {
    for (symbol, operation) in [
        (b"calloc".as_slice(), AllocationOperation::Allocate),
        (b"free".as_slice(), AllocationOperation::Release),
    ] {
        if super::super::matches_platform_c_symbol(
            binding,
            symbol,
            &[
                b"/usr/lib/libSystem.B.dylib".as_slice(),
                b"libSystem.B.dylib".as_slice(),
            ],
            Some(b"msvcrt.dll"),
        )? {
            return Ok(Some(operation));
        }
    }
    Ok(None)
}

pub(super) fn validate(
    program: &TerminalProgram,
    declaration: &BoundaryMachineDeclaration,
    binding: &NormalizedForeignCallBinding,
    application: Option<&BoundaryCallingApplication>,
    signature: &TerminalImportSignature,
    operation: AllocationOperation,
) -> Result<Option<Vec<Path>>, TerminalRuntimeError> {
    let invalid = TerminalRuntimeError::InvalidProvision;
    let address = ScalarType::Integer(
        IntegerType::address(64)
            .map_err(|_| invalid("allocation requires a supported address carrier"))?,
    );
    let unsigned = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64)
            .map_err(|_| invalid("allocation requires a supported size carrier"))?,
    );
    let mut payload = None;
    let native = match operation {
        AllocationOperation::Allocate => {
            if signature.scalar_parameters != [unsigned, unsigned]
                || !signature.structural_parameters.is_empty()
                || signature.result != BoundaryMachineResult::Scalar(address.into())
                || declaration.parameter_order != [BoundaryParameterKind::Scalar; 2]
            {
                return Err(invalid(
                    "calloc requires two u64 sizes and a bare address result",
                ));
            }
            CallSignature {
                parameters: vec![ValueShape::integer(8, 8); 2],
                result: Some(ValueShape::integer(8, 8)),
            }
        }
        AllocationOperation::Release => {
            if !signature.result.is_unit() {
                return Err(invalid("free must return Unit"));
            }
            if signature.scalar_parameters == [address]
                && signature.structural_parameters.is_empty()
                && declaration.parameter_order == [BoundaryParameterKind::Scalar]
            {
                // An ordinary raw address has no structural payload lane.
            } else {
                let [parameter] = signature.structural_parameters.as_slice() else {
                    return Err(invalid(
                        "free requires exactly one address-bearing owned value",
                    ));
                };
                if !signature.scalar_parameters.is_empty()
                    || declaration.parameter_order != [BoundaryParameterKind::Structural]
                    || parameter.access != StructuralAccess::Owned
                {
                    return Err(invalid(
                        "free cannot consume a borrowed or multi-argument value",
                    ));
                }
                let path = super::super::native_word::scalar_payload_path(
                    program,
                    parameter.structural_type,
                    address,
                )?;
                let application = application.ok_or(invalid(
                    "owned free requires the complete admitted calling application",
                ))?;
                if application.signature.parameters.len() != 1
                    || application.signature.result.is_some()
                {
                    return Err(invalid(
                        "owned free calling application has a different argument/result roster",
                    ));
                }
                let envelope = super::super::native_word::native_word_envelope(
                    &application.signature,
                    application.signature.parameters[0],
                )
                .map_err(invalid)?;
                if envelope.byte_size != 8 || envelope.alignment != 8 {
                    return Err(invalid(
                        "free native argument must occupy one aligned pointer word",
                    ));
                }
                payload = Some(path);
            }
            CallSignature {
                parameters: vec![ValueShape::integer(8, 8)],
                result: None,
            }
        }
    };
    super::super::admit_exact_foreign_call(signature, declaration, binding, &native)?;
    if let Some(application) = application {
        if application.plan.plan() != &binding.boundary_entry_plan
            || application.commitment == [0; 32]
            || !application.opaque_applications.is_empty()
        {
            return Err(invalid(
                "allocation calling application is unbound or contains opaque/callback transport",
            ));
        }
    }
    Ok(payload)
}
