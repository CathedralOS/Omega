// Select and validate `calloc`, `free`, and `memset` foreign call bindings
// (`select`, `validate`, `AddressLocation`), checking both bare scalar pointers
// and single-scalar owned record envelopes (`Allocation { pointer }`).

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
        (b"memset".as_slice(), AllocationOperation::Initialize),
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

pub(super) enum AddressLocation {
    BareScalar,
    OwnedArgument(Vec<Path>),
    RecordResult(semantic_vocabulary::StructuralTypeId, Vec<Path>),
}

pub(super) fn validate(
    program: &TerminalProgram,
    declaration: &BoundaryMachineDeclaration,
    binding: &NormalizedForeignCallBinding,
    application: Option<&BoundaryCallingApplication>,
    signature: &TerminalImportSignature,
    operation: AllocationOperation,
) -> Result<AddressLocation, TerminalRuntimeError> {
    let invalid = TerminalRuntimeError::InvalidProvision;
    let address = ScalarType::Integer(
        IntegerType::address(64)
            .map_err(|_| invalid("allocation requires a supported address carrier"))?,
    );
    let unsigned = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64)
            .map_err(|_| invalid("allocation requires a supported size carrier"))?,
    );
    let mut location = AddressLocation::BareScalar;
    let native = match operation {
        AllocationOperation::Allocate => {
            if signature.scalar_parameters != [unsigned, unsigned]
                || !signature.structural_parameters.is_empty()
                || declaration.parameter_order != [BoundaryParameterKind::Scalar; 2]
            {
                return Err(invalid(
                    "calloc requires two u64 sizes and an address result",
                ));
            }
            match &signature.result {
                BoundaryMachineResult::Scalar(actual) if *actual == address.into() => {}
                BoundaryMachineResult::Structural(result) => {
                    let path = super::super::native_word::scalar_payload_path(
                        program,
                        result.structural_type,
                        address,
                    )?;
                    let application = application.ok_or(invalid(
                        "calloc record result requires the complete admitted calling application",
                    ))?;
                    let result_root = application.signature.result.ok_or(invalid(
                        "calloc record result calling application carries no result shape",
                    ))?;
                    let envelope = super::super::native_word::native_word_envelope(
                        &application.signature,
                        result_root,
                    )
                    .map_err(invalid)?;
                    if envelope.byte_size != 8 || envelope.alignment != 8 {
                        return Err(invalid(
                            "calloc record result must occupy one aligned pointer word",
                        ));
                    }
                    location = AddressLocation::RecordResult(result.structural_type, path);
                }
                _ => {
                    return Err(invalid(
                        "calloc requires two u64 sizes and an address result",
                    ));
                }
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
                location = AddressLocation::OwnedArgument(path);
            }
            CallSignature {
                parameters: vec![ValueShape::integer(8, 8)],
                result: None,
            }
        }
        AllocationOperation::Initialize => {
            let signed32 = ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, 32)
                    .map_err(|_| invalid("memset requires a supported i32 value carrier"))?,
            );
            if signature.scalar_parameters != [signed32, unsigned]
                || declaration.parameter_order
                    != [
                        BoundaryParameterKind::Structural,
                        BoundaryParameterKind::Scalar,
                        BoundaryParameterKind::Scalar,
                    ]
            {
                return Err(invalid(
                    "memset requires an owned allocation, an i32 fill and a u64 size",
                ));
            }
            let [parameter] = signature.structural_parameters.as_slice() else {
                return Err(invalid(
                    "memset requires exactly one address-bearing owned value",
                ));
            };
            if parameter.access != StructuralAccess::Owned {
                return Err(invalid("memset cannot write through a borrowed value"));
            }
            let path = super::super::native_word::scalar_payload_path(
                program,
                parameter.structural_type,
                address,
            )?;
            let BoundaryMachineResult::Structural(result) = &signature.result else {
                return Err(invalid(
                    "memset result must be a structural reference to the filled storage",
                ));
            };
            let Some(terminal_psi::StructuralTypeShape::Reference {
                access: StructuralAccess::MutableBorrow,
                ..
            }) = program
                .structural_type(result.structural_type)
                .map(|declaration| &declaration.shape)
            else {
                return Err(invalid("memset result must be an exclusive reference"));
            };
            let application = application.ok_or(invalid(
                "memset requires the complete admitted calling application",
            ))?;
            if application.signature.parameters.len() != 3 {
                return Err(invalid(
                    "memset calling application has a different argument roster",
                ));
            }
            for (index, (byte_size, alignment)) in [(8, 8), (4, 4), (8, 8)].iter().enumerate() {
                let envelope = super::super::native_word::native_word_envelope(
                    &application.signature,
                    application.signature.parameters[index],
                )
                .map_err(invalid)?;
                if envelope.byte_size != *byte_size || envelope.alignment != *alignment {
                    return Err(invalid(
                        "memset native argument must occupy its declared word",
                    ));
                }
            }
            let result_root = application.signature.result.ok_or(invalid(
                "memset calling application carries no result shape",
            ))?;
            let shape = application
                .signature
                .shapes
                .get(usize::from(result_root))
                .ok_or(invalid("memset result references an absent shape"))?;
            if shape.class != calling_conventions::BoundaryValueClass::Reference
                || shape.byte_size != 8
                || shape.alignment != 8
            {
                return Err(invalid(
                    "memset result must occupy one aligned pointer word",
                ));
            }
            location = AddressLocation::OwnedArgument(path);
            CallSignature {
                parameters: vec![
                    ValueShape::integer(8, 8),
                    ValueShape::integer(4, 4),
                    ValueShape::integer(8, 8),
                ],
                result: Some(ValueShape::integer(8, 8)),
            }
        }
    };
    super::super::admit_exact_foreign_call(signature, declaration, binding, &native)?;
    if let Some(application) = application
        && (application.plan.plan() != &binding.boundary_entry_plan
            || application.commitment == [0; 32]
            || !application.opaque_applications.is_empty())
    {
        return Err(invalid(
            "allocation calling application is unbound or contains opaque/callback transport",
        ));
    }
    Ok(location)
}
