//! Admit a complete C record of ordinary 64-bit scalar words.
//! The host operation supplies its exact scalar contract; this join retains
//! semantic codecs, ordered native formals, field geometry and the evaluated
//! calling application before either register or indirect results are offered.

use super::super::TerminalRuntimeError;
use calling_conventions::{
    BoundaryCallingApplication, BoundaryValueClass, BoundaryValueGraph, CallSignature,
};
use semantic_vocabulary::{IeeeFloatFormat, ScalarType, StructuralFieldId, StructuralTypeId};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{TerminalImportSignature, TerminalProgram};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralFieldType,
    StructuralMultiplicity, StructuralRecordStorage, StructuralTypeShape,
};

pub(super) struct ScalarRecordAdmission<const FIELD_COUNT: usize> {
    pub signature: TerminalImportSignature,
    pub structural_type: StructuralTypeId,
    pub fields: [StructuralFieldId; FIELD_COUNT],
}

pub(super) fn admit_c_word_record<const FIELD_COUNT: usize>(
    program: &TerminalProgram,
    declaration: &BoundaryMachineDeclaration,
    binding: &NormalizedForeignCallBinding,
    application: Option<&BoundaryCallingApplication>,
    scalar_parameters: &[ScalarType],
    field_types: [ScalarType; FIELD_COUNT],
) -> Result<ScalarRecordAdmission<FIELD_COUNT>, TerminalRuntimeError> {
    let invalid = TerminalRuntimeError::InvalidProvision;
    let signature = TerminalImportSignature::from(declaration);
    let BoundaryMachineResult::Structural(result) = &signature.result else {
        return Err(invalid("C word-record import requires a structural result"));
    };
    if signature.scalar_parameters.as_slice() != scalar_parameters
        || !signature.structural_parameters.is_empty()
        || declaration.parameter_order.len() != scalar_parameters.len()
        || declaration
            .parameter_order
            .iter()
            .any(|kind| *kind != BoundaryParameterKind::Scalar)
        || !matches!(
            result.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
    {
        return Err(invalid(
            "C word-record import differs from its exact scalar/nonlinear contract",
        ));
    }
    // Pending target layout has no effective field codecs. Treating absent
    // authored storage as C packing would admit equal-size reinterpretations.
    // Only a target-closed declaration can supply the C field contract here.
    let shape = program
        .structural_type(result.structural_type)
        .ok_or_else(|| invalid("C word-record result declaration is absent"))?;
    let StructuralTypeShape::Record { fields, storage } = &shape.shape else {
        return Err(invalid("C word-record result is not a record"));
    };
    if matches!(storage, StructuralRecordStorage::TargetLayout(_)) {
        return Err(invalid(
            "C word-record import requires its resolved target layout codecs",
        ));
    }
    if shape.quotient_carrier.is_some()
        || matches!(
            storage,
            StructuralRecordStorage::PlacedView(_) | StructuralRecordStorage::PlacedAccessor(_)
        )
        || fields.len() != FIELD_COUNT
        || fields
            .iter()
            .zip(field_types)
            .enumerate()
            .any(|(ordinal, (field, scalar))| {
                let unrestricted = match field.field_type {
                    StructuralFieldType::Scalar(actual) => actual == scalar,
                    StructuralFieldType::IeeeFloat(format) => {
                        scalar == ScalarType::IeeeFloat(format)
                    }
                    _ => false,
                };
                field.relevance.is_erased()
                    || !unrestricted
                    || !has_c_scalar_storage(field.storage.as_ref(), (ordinal * 8) as u64, scalar)
            })
    {
        return Err(invalid(
            "C word-record fields must retain exact ordered scalar codecs and ordinary storage",
        ));
    }
    let application = application.ok_or_else(|| {
        invalid("C word-record import requires its complete native calling application")
    })?;
    if application.signature.parameters.len() != scalar_parameters.len()
        || application.plan.plan() != &binding.boundary_entry_plan
        || application.commitment == [0; 32]
        || binding
            .import()
            .is_none_or(|(locator, _)| application.target != locator.target().native_target())
        || !application.opaque_applications.is_empty()
    {
        return Err(invalid(
            "C word-record native application differs from its public signature",
        ));
    }
    let native_parameters = application
        .signature
        .native_parameters
        .span(application.signature.native_parameter_rows)
        .ok_or_else(|| invalid("C word-record native parameter rows are absent"))?;
    if native_parameters.len() != scalar_parameters.len()
        || native_parameters
            .iter()
            .enumerate()
            .any(|(ordinal, parameter)| {
                parameter.native_ordinal != ordinal as u32
                    || parameter.origin
                        != (calling_conventions::BoundaryNativeParameterOrigin::SemanticFormal {
                            formal_ordinal: ordinal as u32,
                        })
                    || parameter.shape
                        != calling_conventions::BoundaryNativeParameterShape::Semantic(
                            application.signature.parameters[ordinal],
                        )
            })
    {
        return Err(invalid(
            "C word-record native parameter order differs from its semantic formals",
        ));
    }
    let graph = BoundaryValueGraph {
        shapes: &application.signature.shapes,
        fields: &application.signature.fields,
    };
    for (root, scalar) in application
        .signature
        .parameters
        .iter()
        .zip(scalar_parameters)
    {
        let shape = graph
            .shapes
            .get(usize::from(*root))
            .ok_or_else(|| invalid("C word-record input shape is absent"))?;
        if shape.class != word_class(*scalar)? || shape.byte_size != 8 || shape.alignment != 8 {
            return Err(invalid(
                "C word-record input class or eight-byte geometry differs",
            ));
        }
    }
    let result_root = application
        .signature
        .result
        .ok_or_else(|| invalid("C word-record native result is absent"))?;
    let native_result = graph
        .shapes
        .get(usize::from(result_root))
        .ok_or_else(|| invalid("C word-record native result shape is absent"))?;
    let BoundaryValueClass::Record {
        first_field,
        field_count,
    } = native_result.class
    else {
        return Err(invalid(
            "C word-record native result lost its complete record shape",
        ));
    };
    let byte_size = FIELD_COUNT
        .checked_mul(8)
        .and_then(|size| u16::try_from(size).ok())
        .ok_or_else(|| invalid("C word-record result extent exceeds native geometry"))?;
    if usize::from(field_count) != FIELD_COUNT
        || native_result.byte_size != byte_size
        || native_result.alignment != 8
    {
        return Err(invalid(
            "C word-record native result extent or alignment differs",
        ));
    }
    let start = usize::from(first_field);
    let members = graph
        .fields
        .get(start..start + FIELD_COUNT)
        .ok_or_else(|| invalid("C word-record native fields are absent"))?;
    for (ordinal, (field, scalar)) in members.iter().zip(field_types).enumerate() {
        let leaf = graph
            .shapes
            .get(usize::from(field.shape))
            .ok_or_else(|| invalid("C word-record native leaf is absent"))?;
        if usize::from(field.byte_offset) != ordinal * 8
            || leaf.class != word_class(scalar)?
            || leaf.byte_size != 8
            || leaf.alignment != 8
        {
            return Err(invalid(
                "C word-record native field codec class or geometry differs",
            ));
        }
    }
    let policy = application.plan.plan().call.policy;
    let native = CallSignature {
        parameters: application
            .signature
            .parameters
            .iter()
            .map(|root| graph.classify(*root, policy))
            .collect::<Result<_, _>>()
            .map_err(|error| {
                TerminalRuntimeError::missing(
                    "runtime-host-scalar-record-abi",
                    format!("C word-record argument ABI classification failed: {error}"),
                )
            })?,
        result: Some(graph.classify(result_root, policy).map_err(|error| {
            TerminalRuntimeError::missing(
                "runtime-host-scalar-record-abi",
                format!("C word-record result ABI classification failed: {error}"),
            )
        })?),
    };
    super::admit_exact_foreign_call(&signature, declaration, binding, &native)?;
    Ok(ScalarRecordAdmission {
        structural_type: result.structural_type,
        fields: std::array::from_fn(|field_index| fields[field_index].id),
        signature,
    })
}

fn word_class(scalar: ScalarType) -> Result<BoundaryValueClass, TerminalRuntimeError> {
    match scalar {
        ScalarType::Integer(integer) if integer.bits() == 64 && !integer.is_address() => {
            Ok(BoundaryValueClass::Integer)
        }
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => Ok(BoundaryValueClass::Float),
        _ => Err(TerminalRuntimeError::InvalidProvision(
            "C word-record host contract requires a 64-bit integer or IEEE scalar",
        )),
    }
}

/// Native classification retains width/class, not the effective signed/float
/// codec. Equal-size reinterpretations cannot stand in for the C field type.
fn has_c_scalar_storage(
    storage: Option<&semantic_vocabulary::AuthoredFieldStorage>,
    offset: u64,
    scalar: ScalarType,
) -> bool {
    use semantic_vocabulary::AuthoredFieldStorage;
    match storage {
        None => true,
        Some(AuthoredFieldStorage::Scalar {
            byte_offset,
            stored_scalar,
        }) => *byte_offset == offset && stored_scalar.is_none_or(|stored| stored == scalar),
        Some(AuthoredFieldStorage::Direct { byte_offset }) => *byte_offset == offset,
        Some(AuthoredFieldStorage::Repeated { .. } | AuthoredFieldStorage::Fragmented(_)) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{AuthoredFieldStorage, IntegerSign, IntegerType};

    #[test]
    fn c_record_admission_retains_signed_codec_not_only_equal_width() {
        let signed = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
        let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        assert!(has_c_scalar_storage(None, 0, signed));
        for stored_scalar in [None, Some(signed)] {
            assert!(has_c_scalar_storage(
                Some(&AuthoredFieldStorage::Scalar {
                    byte_offset: 8,
                    stored_scalar,
                }),
                8,
                signed
            ));
        }
        assert!(has_c_scalar_storage(
            Some(&AuthoredFieldStorage::Direct { byte_offset: 8 }),
            8,
            signed
        ));
        assert!(!has_c_scalar_storage(
            Some(&AuthoredFieldStorage::Scalar {
                byte_offset: 8,
                stored_scalar: Some(unsigned),
            }),
            8,
            signed
        ));
        assert!(!has_c_scalar_storage(
            Some(&AuthoredFieldStorage::Scalar {
                byte_offset: 0,
                stored_scalar: Some(signed),
            }),
            8,
            signed
        ));
        assert!(!has_c_scalar_storage(
            Some(&AuthoredFieldStorage::Repeated {
                byte_offset: 8,
                element_stride: 8,
            }),
            8,
            signed
        ));
    }
    #[test]
    fn float_words_reject_equal_sized_integer_codecs() {
        let float = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
        let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        assert!(has_c_scalar_storage(None, 40, float));
        assert!(has_c_scalar_storage(
            Some(&AuthoredFieldStorage::Scalar {
                byte_offset: 40,
                stored_scalar: Some(float),
            }),
            40,
            float
        ));
        assert!(!has_c_scalar_storage(
            Some(&AuthoredFieldStorage::Scalar {
                byte_offset: 40,
                stored_scalar: Some(integer),
            }),
            40,
            float
        ));
        assert!(!has_c_scalar_storage(
            Some(&AuthoredFieldStorage::Scalar {
                byte_offset: 32,
                stored_scalar: Some(float),
            }),
            40,
            float
        ));
        assert_eq!(word_class(float).unwrap(), BoundaryValueClass::Float);
        assert_eq!(word_class(integer).unwrap(), BoundaryValueClass::Integer);
    }
}
