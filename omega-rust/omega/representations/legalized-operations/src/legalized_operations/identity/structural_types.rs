use super::calling::{encode_placement, encode_shape};
use super::scalar::encode_scalar_type;
use super::shared::*;

pub(super) fn encode_structural_parameter(
    bytes: &mut Vec<u8>,
    parameter: &StructuralParameterDeclaration,
) {
    bytes.extend_from_slice(&parameter.place.get().to_le_bytes());
    bytes.extend_from_slice(&parameter.position.to_le_bytes());
    bytes.push(u8::from(parameter.is_self));
    bytes.extend_from_slice(&parameter.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, parameter.multiplicity);
    encode_access(bytes, parameter.access);
    encode_ids(
        bytes,
        parameter
            .qualifications
            .iter()
            .map(|qualification| qualification.get()),
    );
}

pub(super) fn encode_target_structural_parameter(
    bytes: &mut Vec<u8>,
    parameter: &target_operations::TargetStructuralParameter,
) {
    bytes.extend_from_slice(&parameter.place.get().to_le_bytes());
    bytes.extend_from_slice(&parameter.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, parameter.multiplicity);
    encode_access(bytes, parameter.access);
    encode_shape(bytes, parameter.shape);
    encode_placement(bytes, &parameter.placement);
}

pub(super) fn encode_structural_argument(bytes: &mut Vec<u8>, argument: &StructuralArgument) {
    bytes.extend_from_slice(&argument.place.get().to_le_bytes());
    encode_structural_path(bytes, &argument.path);
    encode_access(bytes, argument.access);
}

pub(super) fn encode_target_structural_argument(
    bytes: &mut Vec<u8>,
    argument: &target_operations::TargetStructuralArgument,
) {
    bytes.extend_from_slice(&argument.place.get().to_le_bytes());
    encode_access(bytes, argument.access);
    encode_structural_path(bytes, &argument.path);
    bytes.extend_from_slice(&argument.root_structural_type.get().to_le_bytes());
    bytes.extend_from_slice(&argument.structural_type.get().to_le_bytes());
    encode_shape(bytes, argument.shape);
    bytes.extend_from_slice(&argument.source_byte_offset.to_le_bytes());
    encode_option_u64(bytes, argument.fixed_array_length);
    encode_option_u32(bytes, argument.element_stride);
    encode_placement(bytes, &argument.source);
    encode_placement(bytes, &argument.destination);
}

pub(super) fn encode_entry_claim(bytes: &mut Vec<u8>, claim: &EntryClaim) {
    bytes.extend_from_slice(&claim.claim.get().to_le_bytes());
    bytes.extend_from_slice(&claim.input.get().to_le_bytes());
    encode_structural_path(bytes, &claim.path);
}

pub(super) fn encode_structural_path(bytes: &mut Vec<u8>, path: &[StructuralPathSegment]) {
    encode_len(bytes, path.len());
    for segment in path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                bytes.push(1);
                encode_string(bytes, identity);
            }
            StructuralPathSegment::FixedIndex(index) => {
                bytes.push(2);
                bytes.extend_from_slice(&index.to_le_bytes());
            }
        }
    }
}

pub(super) fn encode_structural_place(bytes: &mut Vec<u8>, place: StructuralPlaceDeclaration) {
    bytes.extend_from_slice(&place.id.get().to_le_bytes());
    match place.kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            bytes.push(1);
            bytes.extend_from_slice(&position.to_le_bytes());
            bytes.push(u8::from(is_self));
        }
        StructuralPlaceKind::Result => bytes.push(2),
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&producer.get().to_le_bytes());
            bytes.extend_from_slice(&structural_type.get().to_le_bytes());
        }
        StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            structural_type,
        } => {
            bytes.push(4);
            bytes.extend_from_slice(&declaration_ordinal.to_le_bytes());
            bytes.extend_from_slice(&structural_type.get().to_le_bytes());
        }
        StructuralPlaceKind::ProviderAttachment {
            attachment,
            field,
            boundary,
        } => {
            bytes.push(5);
            bytes.extend_from_slice(&attachment.get().to_le_bytes());
            bytes.extend_from_slice(&field.get().to_le_bytes());
            bytes.extend_from_slice(&boundary.get().to_le_bytes());
        }
        StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            structural_type,
            construction,
        } => {
            bytes.push(if construction.is_some() { 7 } else { 6 });
            bytes.extend_from_slice(&declaration_ordinal.to_le_bytes());
            bytes.extend_from_slice(&structural_type.get().to_le_bytes());
            if let Some(construction) = construction {
                bytes.extend_from_slice(&construction.root_structural_type.get().to_le_bytes());
                bytes.extend_from_slice(&construction.index.to_le_bytes());
            }
        }
    }
}

pub(super) fn encode_structural_type(bytes: &mut Vec<u8>, declaration: &StructuralTypeDeclaration) {
    bytes.extend_from_slice(&declaration.id.get().to_le_bytes());
    encode_string(bytes, &declaration.identity);
    match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(scalar_type) => {
            bytes.push(6);
            encode_scalar_type(bytes, *scalar_type);
        }
        StructuralTypeShape::ByteSequence(carrier) => {
            bytes.push(1);
            encode_byte_sequence_carrier(bytes, *carrier);
        }
        StructuralTypeShape::Record { fields } => {
            bytes.push(2);
            encode_structural_fields(bytes, fields);
        }
        StructuralTypeShape::FixedArray { element, length } => {
            bytes.push(3);
            bytes.extend_from_slice(&element.get().to_le_bytes());
            bytes.extend_from_slice(&length.to_le_bytes());
        }
        StructuralTypeShape::Sum { cases } => {
            bytes.push(4);
            encode_len(bytes, cases.len());
            for case in cases {
                bytes.extend_from_slice(&case.id.get().to_le_bytes());
                encode_string(bytes, &case.identity);
                encode_structural_fields(bytes, &case.fields);
            }
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            bytes.push(5);
            encode_structural_fields(bytes, fields);
            encode_len(bytes, cases.len());
            for case in cases {
                bytes.extend_from_slice(&case.id.get().to_le_bytes());
                encode_string(bytes, &case.identity);
                encode_structural_fields(bytes, &case.fields);
            }
        }
    }
}

pub(super) fn encode_structural_fields(bytes: &mut Vec<u8>, fields: &[StructuralFieldDeclaration]) {
    encode_len(bytes, fields.len());
    for field in fields {
        bytes.extend_from_slice(&field.id.get().to_le_bytes());
        encode_string(bytes, &field.identity);
        bytes.push(match field.relevance {
            BindingRelevance::Relevant => 1,
            BindingRelevance::Erased => 2,
        });
        match &field.field_type {
            StructuralFieldType::Scalar(scalar) => {
                bytes.push(1);
                encode_scalar_type(bytes, *scalar);
            }
            StructuralFieldType::IeeeFloat(format) => {
                bytes.push(2);
                bytes.push(match format {
                    IeeeFloatFormat::Binary32 => 1,
                    IeeeFloatFormat::Binary64 => 2,
                });
            }
            StructuralFieldType::ByteSequence(carrier) => {
                bytes.push(3);
                encode_byte_sequence_carrier(bytes, *carrier);
            }
            StructuralFieldType::Structural(structural_type) => {
                bytes.push(4);
                bytes.extend_from_slice(&structural_type.get().to_le_bytes());
            }
            StructuralFieldType::Erased { type_identity } => {
                bytes.push(5);
                encode_string(bytes, type_identity);
            }
        }
    }
}

pub(super) fn encode_byte_sequence_carrier(bytes: &mut Vec<u8>, carrier: ByteSequenceCarrier) {
    match carrier {
        ByteSequenceCarrier::BorrowedView => bytes.push(1),
        ByteSequenceCarrier::BoundedOwned { capacity } => {
            bytes.push(2);
            bytes.extend_from_slice(&capacity.to_le_bytes());
        }
    }
}

pub(super) fn encode_multiplicity(bytes: &mut Vec<u8>, multiplicity: StructuralMultiplicity) {
    bytes.push(match multiplicity {
        StructuralMultiplicity::Unrestricted => 1,
        StructuralMultiplicity::Affine => 2,
        StructuralMultiplicity::Linear => 3,
    });
}

pub(super) fn encode_access(bytes: &mut Vec<u8>, access: StructuralAccess) {
    bytes.push(match access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
}

pub(super) fn encode_option_u64(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

pub(super) fn encode_option_u32(bytes: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

pub(super) fn encode_string(bytes: &mut Vec<u8>, value: &str) {
    encode_len(bytes, value.len());
    bytes.extend_from_slice(value.as_bytes());
}
