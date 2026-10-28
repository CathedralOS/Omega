//! Structural contract of a published numeric meaning. This does not select
//! or authorize a provider, and target geometry remains an Omega obligation.

pub fn validate_numeric_intrinsic_signature(
    boundary: &crate::BoundaryMachineDeclaration,
    structural_types: &[crate::StructuralTypeDeclaration],
) -> Result<(), &'static str> {
    use language_semantics::numeric_intrinsics::NumericResult;
    let Some(meaning) = boundary.numeric_intrinsic else {
        return Ok(());
    };
    let signature = meaning
        .signature()
        .ok_or("unsupported numeric intrinsic meaning")?;
    if boundary.role != crate::BoundaryDeclarationRole::Callable
        || boundary.completion != crate::BoundaryCompletion::NormalReturn
        || boundary.attachment.is_some()
        || !boundary.structural_parameters.is_empty()
        || boundary.scalar_parameters.len() != signature.operand_count
        || boundary
            .scalar_parameters
            .iter()
            .any(|parameter| Some(*parameter) != signature.operand.scalar_type())
    {
        return Err("numeric intrinsic has a mismatched boundary parameter signature");
    }
    let result_matches = match (&boundary.result, signature.result) {
        (crate::BoundaryMachineResult::Scalar(result), NumericResult::Primitive(primitive)) => {
            Some(result.scalar_type) == primitive.scalar_type()
        }
        (crate::BoundaryMachineResult::Scalar(result), NumericResult::Boolean) => {
            result.scalar_type == semantic_vocabulary::ScalarType::Boolean
        }
        (crate::BoundaryMachineResult::Structural(result), NumericResult::FloatClass) => {
            structural_types
                .iter()
                .find(|declaration| declaration.id == result.structural_type)
                .is_some_and(crate::StructuralTypeDeclaration::is_float_class_shape)
        }
        _ => false,
    };
    if !result_matches {
        return Err("numeric intrinsic has a mismatched boundary result signature");
    }
    Ok(())
}
