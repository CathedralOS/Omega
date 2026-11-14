// Structural type, parameter, place, path, and field-initializer identity encoders.
//
// `encode_structural_type`, `encode_structural_place`, and `encode_authored_field_storage`
// serialize structural type declarations, place declarations, and authored field storage recipes
// so the plan identity and recovery codec share a single canonical wire format. Equal element-view
// geometry does not make stored backing permissions equal, so `ElementView` shapes encode their
// optional access ceiling alongside the element type.

use super::calling::{encode_placement, encode_shape};
use super::{
    encode_ids, encode_integer, encode_integer_type, encode_len, encode_option, encode_scalar_type,
};
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
            super::structural::encode_operation_result(bytes, result);
        }
        abstract_operations::StructuralWindowRoot::OwnedBlockParameter { block, parameter } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
            encode_structural_parameter(bytes, parameter);
        }
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
    encode_projected_qualifications(bytes, &parameter.additional_input_qualifications);
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
    origin: &terminal_psi::StructuralReferenceOrigin,
) {
    match origin {
        terminal_psi::StructuralReferenceOrigin::Parameter(source) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, source.place.get().to_le_bytes());
        }
        terminal_psi::StructuralReferenceOrigin::Invocation { carrier_type, .. } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, carrier_type.get().to_le_bytes());
        }
    }
    bytes.push(match origin.extent() {
        semantic_vocabulary::ReferenceOriginExtent::Projected => 1,
        semantic_vocabulary::ReferenceOriginExtent::WithinRegion => 2,
    });
    encode_len(bytes, origin.path().len());
    for projection in origin.path() {
        match projection {
            terminal_psi::ReferenceOriginProjection::Exact(segment) => {
                encode_structural_projection(bytes, segment)
            }
            terminal_psi::ReferenceOriginProjection::Element => bytes.push(9),
        }
    }
    encode_access(bytes, origin.access());
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
    encode_option(bytes, argument.element_stride, |bytes, stride| {
        crate::codec_bytes::push_encoded(bytes, stride.to_le_bytes())
    });
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
    terminal_psi::structural_path_encoding::encode_structural_path_segment(
        bytes,
        segment,
        Vec::extend_from_slice,
        encode_scalar_type,
    );
}

pub fn encode_structural_place(bytes: &mut Vec<u8>, place: StructuralPlaceDeclaration) {
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
    bytes.push(u8::from(declaration.owns_linear_claim));
    bytes.push(u8::from(declaration.no_code_affine_cleanup));
    bytes.push(u8::from(declaration.descriptor_cleanup));
    bytes.push(u8::from(declaration.copyable));
    match &declaration.shape {
        StructuralTypeShape::Opaque => bytes.push(19),
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
        StructuralTypeShape::AtomicCell(cell) => {
            bytes.push(16);
            encode_scalar_restriction(bytes, &(*cell).into());
        }
        StructuralTypeShape::ByteSequence(carrier) => {
            bytes.push(1);
            encode_byte_sequence_carrier(bytes, *carrier);
        }
        StructuralTypeShape::ElementView { element, access } => {
            bytes.push(8);
            crate::codec_bytes::push_encoded(bytes, element.get().to_le_bytes());
            bytes.push(u8::from(access.is_some()));
            if let Some(access) = access {
                encode_access(bytes, *access);
            }
        }
        StructuralTypeShape::Record { fields, storage } => {
            match storage {
                terminal_psi::StructuralRecordStorage::AccessView(storage) => {
                    bytes.push(14);
                    crate::codec_bytes::push_encoded(bytes, storage.byte_size.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, storage.byte_alignment.to_le_bytes());
                }
                terminal_psi::StructuralRecordStorage::Accessor(storage) => {
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
                    | terminal_psi::StructuralRecordStorage::AccessView(_)
                    | terminal_psi::StructuralRecordStorage::Accessor(_)
            ) {
                for field in fields {
                    encode_authored_field_storage(bytes, field.storage.as_ref());
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, element.get().to_le_bytes());
            match length {
                terminal_psi::ArrayExtent::Literal(length) => {
                    bytes.push(0);
                    crate::codec_bytes::push_encoded(bytes, length.to_le_bytes());
                }
                terminal_psi::ArrayExtent::Application(application) => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, application.get().to_le_bytes());
                }
            }
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

pub fn encode_authored_field_storage(
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
        bytes.push(u8::from(field.inline_carrier.is_some()));
        if let Some(carrier) = field.inline_carrier {
            bytes.extend_from_slice(&carrier.get().to_le_bytes());
        }
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

pub(super) fn encode_string(bytes: &mut Vec<u8>, value: &str) {
    encode_len(bytes, value.len());
    bytes.extend_from_slice(value.as_bytes());
}

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
        StructuralFieldType::AtomicBoundedInteger(bounds) => {
            bytes.push(10);
            encode_integer_type(bytes, bounds.integer_type());
            encode_integer(bytes, bounds.minimum());
            encode_integer(bytes, bounds.maximum());
        }
        StructuralFieldType::Atomic(scalar) => {
            bytes.push(8);
            encode_scalar_type(bytes, *scalar);
        }
        StructuralFieldType::BoundedInteger(bounds) => {
            bytes.push(6);
            encode_integer_type(bytes, bounds.integer_type());
            encode_integer(bytes, bounds.minimum());
            encode_integer(bytes, bounds.maximum());
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

#[cfg(test)]
mod tests {
    use super::{
        BindingRelevance, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
        StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
        StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration,
        StructuralTypeShape, encode_structural_fields, encode_structural_parameter,
        encode_structural_place, encode_structural_type, encode_target_structural_parameter,
    };
    use calling_conventions::{ValuePlacement, ValueShape};
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, Proposition, ScalarRefinement, ScalarTerm,
        ScalarType, StructuralFieldId,
    };

    #[test]
    fn runtime_path_uses_the_canonical_segment_tag() {
        let mut bytes = Vec::new();
        super::encode_structural_path(
            &mut bytes,
            &[terminal_psi::StructuralPathSegment::RuntimeIndex {
                index: semantic_vocabulary::ValueId::new(2).unwrap(),
                obligation: semantic_vocabulary::ObligationId::new(7).unwrap(),
            }],
        );
        let mut expected = 1_u64.to_le_bytes().to_vec();
        expected.push(6);
        expected.extend_from_slice(&2_u64.to_le_bytes());
        expected.extend_from_slice(&7_u64.to_le_bytes());
        assert_eq!(bytes, expected);
    }

    fn assert_distinct_encodings(
        declarations: impl IntoIterator<Item = StructuralTypeDeclaration>,
    ) {
        let mut encodings = Vec::new();
        for declaration in declarations {
            let mut bytes = Vec::new();
            encode_structural_type(&mut bytes, &declaration);
            assert!(!encodings.contains(&bytes));
            encodings.push(bytes);
        }
    }

    #[test]
    fn element_view_encoding_distinguishes_access_ceilings() {
        assert_distinct_encodings(
            [
                None,
                Some(StructuralAccess::SharedBorrow),
                Some(StructuralAccess::MutableBorrow),
                Some(StructuralAccess::WriteOnlyBorrow),
            ]
            .map(|access| StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: true,
                scalar_restriction: None,
                id: semantic_vocabulary::StructuralTypeId::new(2).unwrap(),
                identity: terminal_psi::TerminalIdentityIndex(1),
                shape: StructuralTypeShape::ElementView {
                    element: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                    access,
                },
            }),
        );
    }

    #[test]
    fn logical_copy_permission_changes_encoded_identity() {
        assert_distinct_encodings([false, true].map(|flag| StructuralTypeDeclaration {
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: flag,
            descriptor_cleanup: true,
            scalar_restriction: None,
            id: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
            identity: terminal_psi::TerminalIdentityIndex(1),
            shape: StructuralTypeShape::Record {
                fields: Vec::new(),
                storage: terminal_psi::StructuralRecordStorage::Packed,
            },
        }));
    }

    #[test]
    fn declaration_owned_linearity_changes_encoded_identity() {
        assert_distinct_encodings([false, true].map(|flag| StructuralTypeDeclaration {
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: flag,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
            identity: terminal_psi::TerminalIdentityIndex(1),
            shape: StructuralTypeShape::Record {
                fields: Vec::new(),
                storage: terminal_psi::StructuralRecordStorage::Packed,
            },
        }));
    }

    #[test]
    fn retained_block_place_identity_binds_block_and_position() {
        let encode = |block, position| {
            let mut bytes = Vec::new();
            encode_structural_place(
                &mut bytes,
                StructuralPlaceDeclaration {
                    id: semantic_vocabulary::PlaceId::new(1).unwrap(),
                    kind: StructuralPlaceKind::BlockParameter {
                        block: semantic_vocabulary::BlockId::new(block).unwrap(),
                        position,
                    },
                },
            );
            bytes
        };
        assert_ne!(encode(2, 0), encode(3, 0));
        assert_ne!(encode(2, 0), encode(2, 1));
        assert_eq!(
            encode(2, 0),
            [
                &1_u64.to_le_bytes()[..],
                &[8],
                &2_u64.to_le_bytes(),
                &0_u32.to_le_bytes()
            ]
            .concat()
        );
    }

    #[test]
    fn semantic_and_target_parameters_bind_ordered_projected_qualifications() {
        let rows = [1, 2]
            .map(|index| terminal_psi::StructuralPathQualification {
                path: vec![StructuralPathSegment::FixedIndex(index)],
                domain: semantic_vocabulary::StructuralDomainId::new(index + 10).unwrap(),
            })
            .to_vec();
        let semantic = StructuralParameterDeclaration {
            place: semantic_vocabulary::PlaceId::new(1).unwrap(),
            position: 0,
            structural_type: semantic_vocabulary::StructuralTypeId::new(2).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::WriteOnlyBorrow,
            qualifications: Vec::new(),
            projected_qualifications: rows.clone(),
            additional_input_qualifications: Vec::new(),
        };
        let shape = ValueShape::integer(8, 8);
        let target = target_operations::TargetStructuralParameter {
            place: semantic.place,
            structural_type: semantic.structural_type,
            multiplicity: semantic.multiplicity,
            access: semantic.access,
            projected_qualifications: rows.clone(),
            shape,
            placement: ValuePlacement {
                shape,
                locations: Vec::new(),
            },
        };
        let encode_semantic = |parameter: &StructuralParameterDeclaration| {
            let mut bytes = Vec::new();
            encode_structural_parameter(&mut bytes, parameter);
            bytes
        };
        let encode_target = |parameter: &target_operations::TargetStructuralParameter| {
            let mut bytes = Vec::new();
            encode_target_structural_parameter(&mut bytes, parameter);
            bytes
        };
        for mutation in 0..5 {
            let mut changed = rows.clone();
            match mutation {
                0 => changed[0].domain = semantic_vocabulary::StructuralDomainId::new(99).unwrap(),
                1 => changed[0].path[0] = StructuralPathSegment::FixedIndex(99),
                2 => changed.swap(0, 1),
                3 => {
                    changed.pop();
                }
                _ => changed.clear(),
            }
            let mut changed_semantic = semantic.clone();
            changed_semantic.projected_qualifications = changed.clone();
            let mut changed_target = target.clone();
            changed_target.projected_qualifications = changed;
            assert_ne!(
                encode_semantic(&semantic),
                encode_semantic(&changed_semantic)
            );
            assert_ne!(encode_target(&target), encode_target(&changed_target));
        }
    }

    #[test]
    fn exact_inline_carrier_changes_field_identity() {
        let encodings = [None, Some(2), Some(3)].map(|identity| {
            let field = StructuralFieldDeclaration {
                id: StructuralFieldId::new(1).unwrap(),
                identity: terminal_psi::TerminalIdentityIndex(1),
                relevance: BindingRelevance::Relevant,
                storage: None,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                inline_carrier: identity
                    .map(|identity| semantic_vocabulary::StructuralTypeId::new(identity).unwrap()),
            };
            let mut bytes = Vec::new();
            encode_structural_fields(&mut bytes, &[field]);
            bytes
        });
        for (position, encoding) in encodings.iter().enumerate() {
            assert!(!encodings[..position].contains(encoding));
        }
    }

    #[test]
    fn refined_field_identity_retains_carrier_predicate_operator_and_literal() {
        let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let carrier = ScalarType::Integer(integer);
        let subject = ScalarTerm::value(ScalarRefinement::SUBJECT, carrier);
        let zero = ScalarTerm::integer(integer, IntegerValue::Signed(0)).unwrap();
        let one = ScalarTerm::integer(integer, IntegerValue::Signed(1)).unwrap();
        let fields = [
            (
                carrier,
                Proposition::LessThan(zero.clone(), subject.clone()),
            ),
            (carrier, Proposition::LessOrEqual(zero, subject.clone())),
            (carrier, Proposition::LessThan(one, subject)),
            (carrier, Proposition::Truth),
            (ScalarType::Boolean, Proposition::Truth),
        ];
        let identities = fields.map(|(carrier, predicate)| {
            let field = StructuralFieldDeclaration {
                inline_carrier: None,
                storage: None,
                id: StructuralFieldId::new(1).unwrap(),
                identity: terminal_psi::TerminalIdentityIndex(1),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::RefinedScalar(
                    ScalarRefinement::new(carrier, predicate).unwrap(),
                ),
            };
            let mut bytes = Vec::new();
            encode_structural_fields(&mut bytes, &[field]);
            bytes
        });
        for (position, identity) in identities.iter().enumerate() {
            assert!(!identities[..position].contains(identity));
        }
    }
}
