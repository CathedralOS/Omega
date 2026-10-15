use super::calling::{encode_placement, encode_shape};
use super::encoding::{encode_ids, encode_len};
use super::scalar::encode_scalar_type;
use semantic_vocabulary::{IeeeFloatFormat, StructuralPlaceKind};
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, EntryClaim, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape,
};

pub(super) fn encode_window_root(
    bytes: &mut Vec<u8>,
    root: &abstract_operations::StructuralWindowRoot,
) {
    match root {
        abstract_operations::StructuralWindowRoot::Parameter(parameter) => {
            bytes.push(0);
            encode_structural_parameter(bytes, parameter);
        }
        abstract_operations::StructuralWindowRoot::OwnedResult { producer, result } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, producer.get().to_le_bytes());
            super::structural_result::encode_operation_result(bytes, result);
        }
        abstract_operations::StructuralWindowRoot::OwnedBlockParameter { block, parameter } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
            encode_structural_parameter(bytes, parameter);
        }
    }
}

#[cfg(test)]
mod block_parameter_tests;
#[cfg(test)]
mod parameter_tests;
#[cfg(test)]
mod scalar_refinement_tests;

#[test]
fn element_view_encoding_distinguishes_access_ceilings() {
    let mut encodings = Vec::new();
    for access in [
        None,
        Some(StructuralAccess::SharedBorrow),
        Some(StructuralAccess::MutableBorrow),
        Some(StructuralAccess::WriteOnlyBorrow),
    ] {
        let declaration = StructuralTypeDeclaration {
            quotient_carrier: None,
            no_code_affine_cleanup: false,
            descriptor_cleanup: true,
            scalar_restriction: None,
            id: semantic_vocabulary::StructuralTypeId::new(2).unwrap(),
            identity: terminal_psi::TerminalIdentityIndex(1),
            shape: StructuralTypeShape::ElementView {
                element: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                access,
            },
        };
        let mut bytes = Vec::new();
        encode_structural_type(&mut bytes, &declaration);
        assert!(!encodings.contains(&bytes));
        encodings.push(bytes);
    }
}

pub(super) fn encode_structural_parameter(
    bytes: &mut Vec<u8>,
    parameter: &StructuralParameterDeclaration,
) {
    crate::codec_bytes::push_encoded(bytes, parameter.place.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, parameter.position.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, parameter.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, parameter.multiplicity);
    encode_access(bytes, parameter.access);
    encode_ids(
        bytes,
        parameter
            .qualifications
            .iter()
            .map(|qualification| qualification.get()),
    );
    encode_projected_qualifications(bytes, &parameter.projected_qualifications);
}

pub(super) fn encode_target_structural_parameter(
    bytes: &mut Vec<u8>,
    parameter: &target_operations::TargetStructuralParameter,
) {
    crate::codec_bytes::push_encoded(bytes, parameter.place.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, parameter.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, parameter.multiplicity);
    encode_access(bytes, parameter.access);
    encode_shape(bytes, parameter.shape);
    encode_placement(bytes, &parameter.placement);
    encode_projected_qualifications(bytes, &parameter.projected_qualifications);
}

pub(super) fn encode_projected_qualifications(
    bytes: &mut Vec<u8>,
    rows: &[terminal_psi::StructuralPathQualification],
) {
    encode_len(bytes, rows.len());
    for row in rows {
        encode_structural_path(bytes, &row.path);
        crate::codec_bytes::push_encoded(bytes, row.domain.get().to_le_bytes());
    }
}

pub(super) fn encode_structural_argument(bytes: &mut Vec<u8>, argument: &StructuralArgument) {
    crate::codec_bytes::push_encoded(bytes, argument.place.get().to_le_bytes());
    encode_structural_path(bytes, &argument.path);
    encode_access(bytes, argument.access);
}

pub(super) fn encode_reference_origin(
    bytes: &mut Vec<u8>,
    origin: &terminal_psi::ReferenceOriginSource,
) {
    crate::codec_bytes::push_encoded(bytes, origin.place.get().to_le_bytes());
    bytes.push(match origin.extent {
        semantic_vocabulary::ReferenceOriginExtent::Projected => 1,
        semantic_vocabulary::ReferenceOriginExtent::WithinRegion => 2,
    });
    encode_len(bytes, origin.path.len());
    for projection in &origin.path {
        match projection {
            terminal_psi::ReferenceOriginProjection::Exact(segment) => {
                encode_structural_projection(bytes, segment)
            }
            terminal_psi::ReferenceOriginProjection::Element => bytes.push(9),
        }
    }
    encode_access(bytes, origin.access);
}

pub(super) fn encode_target_structural_argument(
    bytes: &mut Vec<u8>,
    argument: &target_operations::TargetStructuralArgument,
) {
    crate::codec_bytes::push_encoded(bytes, argument.place.get().to_le_bytes());
    encode_access(bytes, argument.access);
    encode_structural_path(bytes, &argument.path);
    crate::codec_bytes::push_encoded(bytes, argument.root_structural_type.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, argument.structural_type.get().to_le_bytes());
    encode_shape(bytes, argument.shape);
    crate::codec_bytes::push_encoded(bytes, argument.source_byte_offset.to_le_bytes());
    optimization_core::encode_option_u64(bytes, argument.fixed_array_length);
    encode_option_u32(bytes, argument.element_stride);
    match &argument.source {
        target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
            bytes.push(5);
            crate::codec_bytes::push_encoded(bytes, psi_operation.get().to_le_bytes());
        }
        target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
            psi_operation,
        } => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, psi_operation.get().to_le_bytes());
        }
        target_operations::TargetStructuralArgumentSource::BlockParameter { block, place } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
        }
        target_operations::TargetStructuralArgumentSource::Placement(placement) => {
            bytes.push(0);
            encode_placement(bytes, placement);
        }
        target_operations::TargetStructuralArgumentSource::EstablishedByteView {
            psi_operation,
        } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, psi_operation.get().to_le_bytes());
        }
        target_operations::TargetStructuralArgumentSource::EstablishedElementView {
            psi_operation,
        } => {
            bytes.push(6);
            crate::codec_bytes::push_encoded(bytes, psi_operation.get().to_le_bytes());
        }
    }
    encode_placement(bytes, &argument.destination);
}

pub(super) fn encode_entry_claim(bytes: &mut Vec<u8>, claim: &EntryClaim) {
    crate::codec_bytes::push_encoded(bytes, claim.claim.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, claim.input.get().to_le_bytes());
    encode_structural_path(bytes, &claim.path);
}

pub(super) fn encode_structural_path(bytes: &mut Vec<u8>, path: &[StructuralPathSegment]) {
    encode_len(bytes, path.len());
    for segment in path {
        encode_structural_projection(bytes, segment);
    }
}

fn encode_structural_projection(bytes: &mut Vec<u8>, segment: &StructuralPathSegment) {
    match segment {
        StructuralPathSegment::AllElements => bytes.push(10),
        StructuralPathSegment::Case(case) => {
            bytes.push(7);
            crate::codec_bytes::push_encoded(bytes, case.get().to_le_bytes());
        }
        StructuralPathSegment::Referent => bytes.push(3),
        StructuralPathSegment::FixedByteRange { start, end } => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, start.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, end.to_le_bytes());
        }
        StructuralPathSegment::Field(identity) => {
            bytes.push(1);
            bytes.extend_from_slice(&identity.0.to_le_bytes());
        }
        StructuralPathSegment::FixedIndex(index) => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, index.to_le_bytes());
        }
        StructuralPathSegment::RuntimeIndex { index, obligation } => {
            bytes.push(5);
            crate::codec_bytes::push_encoded(bytes, index.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
        }
        StructuralPathSegment::RepresentationView {
            structural_type,
            stored_scalar,
        } => {
            bytes.push(8);
            crate::codec_bytes::push_encoded(bytes, structural_type.get().to_le_bytes());
            bytes.push(u8::from(stored_scalar.is_some()));
            if let Some(scalar) = stored_scalar {
                encode_scalar_type(bytes, *scalar);
            }
        }
    }
}

pub(super) fn encode_structural_place(bytes: &mut Vec<u8>, place: StructuralPlaceDeclaration) {
    crate::codec_bytes::push_encoded(bytes, place.id.get().to_le_bytes());
    match place.kind {
        StructuralPlaceKind::BlockParameter { block, position } => {
            bytes.push(8);
            crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, position.to_le_bytes());
        }
        StructuralPlaceKind::Parameter { position } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, position.to_le_bytes());
        }
        StructuralPlaceKind::Result => bytes.push(2),
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, producer.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, structural_type.get().to_le_bytes());
        }
        StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            structural_type,
        } => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, declaration_ordinal.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, structural_type.get().to_le_bytes());
        }
        StructuralPlaceKind::ProviderAttachment {
            attachment,
            field,
            boundary,
        } => {
            bytes.push(5);
            crate::codec_bytes::push_encoded(bytes, attachment.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, field.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, boundary.get().to_le_bytes());
        }
        StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            structural_type,
            construction,
        } => {
            bytes.push(if construction.is_some() { 7 } else { 6 });
            crate::codec_bytes::push_encoded(bytes, declaration_ordinal.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, structural_type.get().to_le_bytes());
            if let Some(construction) = construction {
                crate::codec_bytes::push_encoded(
                    bytes,
                    construction.root_structural_type.get().to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(bytes, construction.index.to_le_bytes());
            }
        }
    }
}

pub(super) fn encode_structural_type(bytes: &mut Vec<u8>, declaration: &StructuralTypeDeclaration) {
    crate::codec_bytes::push_encoded(bytes, declaration.id.get().to_le_bytes());
    bytes.extend_from_slice(&declaration.identity.0.to_le_bytes());
    bytes.push(u8::from(declaration.no_code_affine_cleanup));
    bytes.push(u8::from(declaration.descriptor_cleanup));
    match &declaration.shape {
        StructuralTypeShape::BindingCarrier => bytes.push(11),
        StructuralTypeShape::Reference { referent, access } => {
            bytes.push(7);
            crate::codec_bytes::push_encoded(bytes, referent.get().to_le_bytes());
            encode_access(bytes, *access);
        }
        StructuralTypeShape::PrimitiveScalar(scalar_type) => {
            bytes.push(if declaration.scalar_restriction.is_some() {
                12
            } else {
                6
            });
            encode_scalar_type(bytes, *scalar_type);
            if let Some(restriction) = &declaration.scalar_restriction {
                encode_scalar_restriction(bytes, restriction);
            }
        }
        StructuralTypeShape::ByteSequence(carrier) => {
            bytes.push(1);
            encode_byte_sequence_carrier(bytes, *carrier);
        }
        StructuralTypeShape::ElementView { element, access } => {
            bytes.push(8);
            crate::codec_bytes::push_encoded(bytes, element.get().to_le_bytes());
            // Equal geometry does not make stored backing permissions equal.
            bytes.push(u8::from(access.is_some()));
            if let Some(access) = access {
                encode_access(bytes, *access);
            }
        }
        StructuralTypeShape::Record { fields, storage } => {
            match storage {
                terminal_psi::StructuralRecordStorage::PlacedView(storage) => {
                    bytes.push(14);
                    crate::codec_bytes::push_encoded(bytes, storage.byte_size.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, storage.byte_alignment.to_le_bytes());
                }
                terminal_psi::StructuralRecordStorage::PlacedAccessor(storage) => {
                    bytes.push(15);
                    crate::codec_bytes::push_encoded(bytes, storage.byte_size.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, storage.byte_alignment.to_le_bytes());
                }
                terminal_psi::StructuralRecordStorage::Packed => bytes.push(2),
                terminal_psi::StructuralRecordStorage::Authored(storage) => {
                    bytes.push(10);
                    crate::codec_bytes::push_encoded(bytes, storage.byte_size.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, storage.byte_alignment.to_le_bytes());
                }
                terminal_psi::StructuralRecordStorage::TargetLayout(application) => {
                    bytes.push(13);
                    crate::codec_bytes::push_encoded(bytes, application.0.to_le_bytes());
                }
            }
            encode_structural_fields(bytes, fields);
            if matches!(
                storage,
                terminal_psi::StructuralRecordStorage::Authored(_)
                    | terminal_psi::StructuralRecordStorage::PlacedView(_)
                    | terminal_psi::StructuralRecordStorage::PlacedAccessor(_)
            ) {
                for field in fields {
                    encode_authored_field_storage(bytes, field.storage.as_ref());
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, element.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, length.to_le_bytes());
        }
        StructuralTypeShape::Sum { cases } => {
            bytes.push(4);
            encode_len(bytes, cases.len());
            for case in cases {
                crate::codec_bytes::push_encoded(bytes, case.id.get().to_le_bytes());
                bytes.extend_from_slice(&case.identity.0.to_le_bytes());
                encode_structural_fields(bytes, &case.fields);
            }
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            bytes.push(5);
            encode_structural_fields(bytes, fields);
            encode_len(bytes, cases.len());
            for case in cases {
                crate::codec_bytes::push_encoded(bytes, case.id.get().to_le_bytes());
                bytes.extend_from_slice(&case.identity.0.to_le_bytes());
                encode_structural_fields(bytes, &case.fields);
            }
        }
    }
}

fn encode_authored_field_storage(
    bytes: &mut Vec<u8>,
    storage: Option<&semantic_vocabulary::AuthoredFieldStorage>,
) {
    use semantic_vocabulary::AuthoredFieldStorage;
    match storage {
        None => bytes.push(0),
        Some(AuthoredFieldStorage::Scalar {
            byte_offset,
            stored_scalar,
        }) => {
            bytes.push(if stored_scalar.is_some() { 2 } else { 1 });
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
            if let Some(scalar) = stored_scalar {
                encode_scalar_type(bytes, *scalar);
            }
        }
        Some(AuthoredFieldStorage::Repeated {
            byte_offset,
            element_stride,
        }) => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, element_stride.to_le_bytes());
        }
        Some(AuthoredFieldStorage::Direct { byte_offset }) => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }
        Some(AuthoredFieldStorage::Fragmented(recipe)) => {
            bytes.push(6);
            encode_len(bytes, recipe.fragments().len());
            for fragment in recipe.fragments() {
                crate::codec_bytes::push_encoded(
                    bytes,
                    fragment.container_byte_offset.to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(
                    bytes,
                    fragment.container_width_bits.to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(bytes, fragment.destination_lsb.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, fragment.source_lsb.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, fragment.width.to_le_bytes());
            }
        }
    }
}

pub(super) fn encode_structural_fields(bytes: &mut Vec<u8>, fields: &[StructuralFieldDeclaration]) {
    encode_len(bytes, fields.len());
    for field in fields {
        crate::codec_bytes::push_encoded(bytes, field.id.get().to_le_bytes());
        bytes.extend_from_slice(&field.identity.0.to_le_bytes());
        bytes.push(match field.relevance {
            BindingRelevance::Relevant => 1,
            BindingRelevance::Erased => 2,
        });
        encode_scalar_restriction(bytes, &field.field_type);
    }
}

pub(super) fn encode_byte_sequence_carrier(bytes: &mut Vec<u8>, carrier: ByteSequenceCarrier) {
    match carrier {
        ByteSequenceCarrier::BorrowedView { .. } => bytes.push(1),
        ByteSequenceCarrier::BoundedView { access, capacity } => {
            bytes.push(3);
            bytes.push(match access {
                Some(access) => match access {
                    StructuralAccess::Owned => 1,
                    StructuralAccess::SharedBorrow => 2,
                    StructuralAccess::MutableBorrow => 3,
                    StructuralAccess::WriteOnlyBorrow => 4,
                },
                None => 0,
            });
            crate::codec_bytes::push_encoded(bytes, capacity.to_le_bytes());
        }
        ByteSequenceCarrier::BoundedOwned { capacity } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, capacity.to_le_bytes());
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

pub(super) fn encode_option_u32(bytes: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(value) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

pub(super) fn encode_string(bytes: &mut Vec<u8>, value: &str) {
    encode_len(bytes, value.len());
    bytes.extend_from_slice(value.as_bytes());
}

/// Encode a record/case field-initializer roster: each entry carries the
/// field identity and its value — a scalar (tag 0, value + optional range
/// obligation) or a structural argument (tag 1).
pub(super) fn encode_record_initializers(
    bytes: &mut Vec<u8>,
    fields: &[terminal_psi::RecordFieldInitializer],
) {
    encode_len(bytes, fields.len());
    for field in fields {
        crate::codec_bytes::push_encoded(bytes, field.field.get().to_le_bytes());
        match &field.value {
            terminal_psi::RecordFieldValue::Scalar {
                value,
                range_obligation,
            } => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, value.get().to_le_bytes());
                optimization_core::encode_option_u64(bytes, range_obligation.map(|id| id.get()));
            }
            terminal_psi::RecordFieldValue::Structural(argument) => {
                bytes.push(1);
                encode_structural_argument(bytes, argument);
            }
        }
    }
}

fn encode_scalar_restriction(bytes: &mut Vec<u8>, field_type: &StructuralFieldType) {
    match field_type {
        StructuralFieldType::RefinedScalar(refinement) => {
            bytes.push(9);
            let identity = optimization_unit::scalar_refinement_identity_bytes(refinement);
            encode_len(bytes, identity.len());
            bytes.extend_from_slice(&identity);
        }
        StructuralFieldType::Atomic(scalar) => {
            bytes.push(8);
            encode_scalar_type(bytes, *scalar);
        }
        StructuralFieldType::BoundedInteger(bounds) => {
            bytes.push(6);
            super::scalar::encode_integer_type(bytes, bounds.integer_type());
            super::scalar::encode_integer(bytes, bounds.minimum());
            super::scalar::encode_integer(bytes, bounds.maximum());
        }
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
        StructuralFieldType::PredicateByteSequence {
            carrier,
            predicates,
        } => {
            bytes.push(7);
            encode_byte_sequence_carrier(bytes, *carrier);
            bytes.push(predicates.bits());
        }
        StructuralFieldType::ByteSequence(carrier) => {
            bytes.push(3);
            encode_byte_sequence_carrier(bytes, *carrier);
        }
        StructuralFieldType::Structural(structural_type) => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, structural_type.get().to_le_bytes());
        }
        StructuralFieldType::Erased { type_identity } => {
            bytes.push(5);
            bytes.extend_from_slice(&type_identity.0.to_le_bytes());
        }
    }
}
