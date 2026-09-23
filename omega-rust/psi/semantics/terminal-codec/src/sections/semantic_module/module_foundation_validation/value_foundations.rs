//! Operations that establish a value: scalar arrays and cases, byte-sequence
//! literals, trivial affine locals, records and primitive locals.

use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralFieldType, StructuralMultiplicity,
    StructuralPlaceDeclaration, StructuralTypeShape, TerminalMachine, TerminalModule,
};

pub(super) fn validate_establish_scalar_array(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishScalarArray { elements } = &operation.kind else {
        unreachable!("dispatched validate_establish_scalar_array")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("scalar array establishment has no structural result");
    };
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !matches!(
            machine.structural_places.iter().find(|place| place.id == result.place),
            Some(StructuralPlaceDeclaration {
                kind: StructuralPlaceKind::OperationResult { producer, structural_type }, ..
            }) if *producer == operation.id && *structural_type == result.structural_type
        )
        || terminal_semantics::scalar_array_leaf_shape(
            module.structural_types.iter(),
            result.structural_type,
        )
        .is_none_or(|(_, count)| u64::try_from(elements.len()).ok() != Some(count))
    {
        return malformed("scalar array establishment has an invalid result shape");
    }
    Ok(())
}

pub(super) fn validate_establish_scalar_case(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishScalarCase {
        result_case,
        fields,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_establish_scalar_case")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("scalar case establishment has no structural result");
    };
    if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return malformed("scalar case establishment has an invalid result surface");
    }
    if !matches!(
        machine.structural_places.iter().find(|place| place.id == result.place),
        Some(StructuralPlaceDeclaration {
            kind: StructuralPlaceKind::OperationResult { producer, structural_type },
            ..
        }) if *producer == operation.id && *structural_type == result.structural_type
    ) {
        return malformed("scalar case establishment has no matching result place");
    }
    let Some(declaration) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
    else {
        return malformed("scalar case establishment has an unknown structural type");
    };
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return malformed("scalar case establishment requires a sum type");
    };
    let Some(selected) = cases.iter().find(|case| case.id == *result_case) else {
        return malformed("scalar case establishment requires an exact member");
    };
    if selected.fields.len() != fields.len() {
        return malformed("scalar case establishment requires every selected field");
    }
    for (declaration, field) in selected.fields.iter().zip(fields) {
        if declaration.id != field.field
            || declaration.relevance.is_erased()
            || declaration.field_type.scalar_type().is_none()
            || matches!(
                declaration.field_type,
                StructuralFieldType::BoundedInteger(_)
            ) != field.range_obligation.is_some()
        {
            return malformed("scalar case establishment has an invalid field binding");
        }
    }
    // Full module validation independently checks exact operand types,
    // dominance and declaration-derived obligations before acceptance.
    Ok(())
}

pub(super) fn validate_establish_structural_case(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishStructuralCase {
        result_case,
        fields,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_establish_structural_case")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("structural case establishment has no structural result");
    };
    if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return malformed("structural case establishment has an invalid result surface");
    }
    if !matches!(
        machine.structural_places.iter().find(|place| place.id == result.place),
        Some(StructuralPlaceDeclaration {
            kind: StructuralPlaceKind::OperationResult { producer, structural_type },
            ..
        }) if *producer == operation.id && *structural_type == result.structural_type
    ) {
        return malformed("structural case establishment has no matching result place");
    }
    let Some(declaration) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
    else {
        return malformed("structural case establishment has an unknown structural type");
    };
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return malformed("structural case establishment requires a sum type");
    };
    let Some(selected) = cases.iter().find(|case| case.id == *result_case) else {
        return malformed("structural case establishment requires an exact member");
    };
    if selected.fields.len() != fields.len() {
        return malformed("structural case establishment requires every selected field");
    }
    for (declaration, field) in selected.fields.iter().zip(fields) {
        if declaration.id != field.field || declaration.relevance.is_erased() {
            return malformed("structural case establishment has an invalid field binding");
        }
        match (&field.value, &declaration.field_type) {
            (
                terminal_psi::RecordFieldValue::Scalar {
                    range_obligation, ..
                },
                field_type,
            ) if field_type.scalar_type().is_some() => {
                if range_obligation.is_some()
                    != matches!(field_type, StructuralFieldType::BoundedInteger(_))
                {
                    return malformed("structural case establishment has an invalid field binding");
                }
            }
            (
                terminal_psi::RecordFieldValue::Structural(argument),
                StructuralFieldType::Structural(_),
            ) if argument.access == terminal_psi::StructuralAccess::Owned
                && argument.path.is_empty() => {}
            _ => return malformed("structural case establishment has an invalid field binding"),
        }
    }
    // Full module validation independently checks exact operand types,
    // dominance and declaration-derived obligations before acceptance.
    Ok(())
}

pub(super) fn validate_establish_byte_sequence_literal(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishByteSequenceLiteral { destination, .. } = &operation.kind else {
        unreachable!("dispatched validate_establish_byte_sequence_literal")
    };
    if operation.result != OperationResult::Unit {
        return malformed("byte-sequence literal establishment declares a scalar result");
    }
    let Some(StructuralPlaceDeclaration {
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        },
        ..
    }) = machine
        .structural_places
        .iter()
        .find(|place| place.id == *destination)
    else {
        return malformed("byte-sequence literal establishment has no literal declaration");
    };
    let Some(declaration) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == *structural_type)
    else {
        return malformed("byte-sequence literal has an unknown structural type");
    };
    if !matches!(
        declaration.shape,
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView)
    ) {
        return malformed("byte-sequence literal must use a borrowed byte-sequence type");
    }
    Ok(())
}

pub(super) fn validate_establish_trivial_affine_local(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishTrivialAffineLocal { destination } = &operation.kind else {
        unreachable!("dispatched validate_establish_trivial_affine_local")
    };
    if operation.result != OperationResult::Unit {
        return malformed("trivial affine local establishment declares a scalar result");
    }
    let Some(StructuralPlaceDeclaration {
        kind: StructuralPlaceKind::TrivialAffineLocal {
            structural_type, ..
        },
        ..
    }) = machine
        .structural_places
        .iter()
        .find(|place| place.id == *destination)
    else {
        return malformed("trivial affine local establishment has no local declaration");
    };
    let Some(declaration) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == *structural_type)
    else {
        return malformed("trivial affine local has an unknown structural type");
    };
    if !matches!(
        &declaration.shape,
        StructuralTypeShape::Record { fields } if fields.is_empty()
    ) {
        return malformed("trivial affine local must have an empty record type");
    }
    Ok(())
}

pub(super) fn validate_establish_record(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishRecord { fields } = &operation.kind else {
        unreachable!("dispatched validate_establish_record")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("record has no structural result");
    };
    if !matches!(result.multiplicity, StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted)
        || !result.qualifications.is_empty() || !result.projected_qualifications.is_empty() || !result.claims.is_empty()
        || !machine.structural_places.iter().any(|place| place.id == result.place && matches!(place.kind, StructuralPlaceKind::OperationResult { producer, structural_type } if producer == operation.id && structural_type == result.structural_type)) {
        return malformed("record result custody is noncanonical");
    }
    let Some(declaration) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
    else {
        return malformed("record type is absent");
    };
    let StructuralTypeShape::Record {
        fields: declarations,
    } = &declaration.shape
    else {
        return malformed("record result is not a record");
    };
    if fields.len() != declarations.len() {
        return malformed("record field roster differs");
    }
    for (field, declaration) in fields.iter().zip(declarations) {
        if field.field != declaration.id
            || declaration.relevance != terminal_psi::BindingRelevance::Relevant
        {
            return malformed("record field identity differs");
        }
        match (&field.value, &declaration.field_type) {
            (
                terminal_psi::RecordFieldValue::Scalar {
                    range_obligation, ..
                },
                field_type,
            ) if field_type.scalar_type().is_some() => {
                if range_obligation.is_some()
                    != matches!(field_type, StructuralFieldType::BoundedInteger(_))
                {
                    return malformed("record range obligation differs");
                }
            }
            (
                terminal_psi::RecordFieldValue::Structural(argument),
                StructuralFieldType::Structural(_),
            ) if argument.access == terminal_psi::StructuralAccess::Owned
                && argument.path.is_empty() => {}
            _ => return malformed("record field operand differs"),
        }
    }
    Ok(())
}

pub(super) fn validate_establish_primitive_local(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishPrimitiveLocal { .. } = &operation.kind else {
        unreachable!("dispatched validate_establish_primitive_local")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("primitive local has no structural result");
    };
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !machine.structural_places.iter().any(|place| {
            place.id == result.place
                && matches!(place.kind,
                StructuralPlaceKind::OperationResult { producer, structural_type }
                    if producer == operation.id && structural_type == result.structural_type)
        })
        || !module.structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
        })
    {
        return malformed("primitive local result custody is noncanonical");
    }
    Ok(())
}
