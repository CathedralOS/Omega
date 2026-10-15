use super::{
    BlockId, Cursor, PlaceId, StructuralAccess, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
};
use crate::FixedViewCopyDecodeError;
use crate::register_homes::recovery::fixed_view_copy::codec::selected::structural::declarations::decode_path;
use crate::register_homes::recovery::fixed_view_copy::codec::selected::structural::declarations::decode_place;
use crate::register_homes::recovery::fixed_view_copy::codec::selected::structural::declarations::decode_type;
use crate::register_homes::recovery::fixed_view_copy::codec::selected::structural::declarations::encode_place;
use crate::register_homes::recovery::fixed_view_copy::codec::selected::structural::declarations::encode_type;
use selected_instructions::effects::program::identity::encode_path;

#[test]
fn element_view_encoding_round_trips_each_access_ceiling() {
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
            id: StructuralTypeId::new(2).unwrap(),
            identity: terminal_psi::TerminalIdentityIndex(1),
            shape: StructuralTypeShape::ElementView {
                element: StructuralTypeId::new(1).unwrap(),
                access,
            },
        };
        let mut bytes = Vec::new();
        encode_type(&mut bytes, &declaration).unwrap();
        assert!(!encodings.contains(&bytes));
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_type(&mut cursor).unwrap(), declaration);
        assert_eq!(cursor.remaining(), 0);
        encodings.push(bytes);
    }
}

#[test]
fn scalar_refinement_round_trip_retains_full_predicate_and_rejects_foreign_subject() {
    use semantic_vocabulary::{Proposition, ScalarRefinement, ScalarTerm, ScalarType, ValueId};
    use terminal_psi::{BindingRelevance, StructuralFieldDeclaration, StructuralFieldType};
    let predicate = Proposition::Equal(
        ScalarTerm::value(ScalarRefinement::SUBJECT, ScalarType::Boolean),
        ScalarTerm::boolean(true),
    );
    let declaration = StructuralTypeDeclaration {
        quotient_carrier: None,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        scalar_restriction: None,
        id: StructuralTypeId::new(7).unwrap(),
        identity: terminal_psi::TerminalIdentityIndex(1),
        shape: StructuralTypeShape::Record {
            storage: terminal_psi::StructuralRecordStorage::Packed,
            fields: vec![StructuralFieldDeclaration {
                storage: None,
                id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                identity: terminal_psi::TerminalIdentityIndex(2),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::RefinedScalar(
                    ScalarRefinement::new(ScalarType::Boolean, predicate.clone()).unwrap(),
                ),
            }],
        },
    };
    let mut bytes = Vec::new();
    encode_type(&mut bytes, &declaration).unwrap();
    let mut cursor = Cursor::new(&bytes);
    assert_eq!(decode_type(&mut cursor).unwrap(), declaration);
    assert_eq!(cursor.remaining(), 0);
    let predicate_length = terminal_codec::canonical_proposition_order_key(&predicate)
        .unwrap()
        .len();
    let mut foreign = predicate;
    foreign.remap_value_ids(|_| ValueId::new(2).unwrap());
    let foreign = terminal_codec::canonical_proposition_order_key(&foreign).unwrap();
    bytes.truncate(bytes.len() - predicate_length - 8);
    bytes.extend_from_slice(&u64::try_from(foreign.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(&foreign);
    assert_eq!(
        decode_type(&mut Cursor::new(&bytes)),
        Err(FixedViewCopyDecodeError::InvalidScalarRefinement)
    );
}

#[test]
fn fused_binding_type_round_trip_distinguishes_empty_record_and_unknown_tag() {
    let declaration = StructuralTypeDeclaration {
        quotient_carrier: None,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
        id: StructuralTypeId::new(7).unwrap(),
        identity: terminal_psi::TerminalIdentityIndex(3),
        shape: StructuralTypeShape::BindingCarrier,
    };
    let mut encoded = Vec::new();
    encode_type(&mut encoded, &declaration).unwrap();
    assert_eq!(encoded.last(), Some(&9));
    let mut cursor = Cursor::new(&encoded);
    assert_eq!(decode_type(&mut cursor).unwrap(), declaration);
    assert_eq!(cursor.remaining(), 0);

    let mut record = declaration.clone();
    record.shape = StructuralTypeShape::Record {
        storage: terminal_psi::StructuralRecordStorage::Packed,
        fields: Vec::new(),
    };
    let mut record_bytes = Vec::new();
    encode_type(&mut record_bytes, &record).unwrap();
    assert_ne!(encoded, record_bytes);
    *encoded.last_mut().unwrap() = 255;
    assert_eq!(
        decode_type(&mut Cursor::new(&encoded)),
        Err(FixedViewCopyDecodeError::UnknownStructuralTypeShape(255)),
    );
}

#[test]
fn fixed_byte_range_path_round_trip_retains_exact_endpoints_and_rejects_truncation() {
    for (start, end) in [(0, 0), (1, 3), (2, 4), (u64::MAX - 1, u64::MAX)] {
        let path = vec![StructuralPathSegment::FixedByteRange { start, end }];
        let mut bytes = Vec::new();
        encode_path(&mut bytes, &path);
        let mut expected = 1_u64.to_le_bytes().to_vec();
        expected.push(4);
        crate::codec_bytes::push_encoded(&mut expected, start.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut expected, end.to_le_bytes());
        assert_eq!(bytes, expected);
        let mut cursor = Cursor::new(&bytes);
        let decoded = decode_path(&mut cursor).unwrap();
        assert_eq!(decoded, path);
        assert_eq!(cursor.remaining(), 0);
        let mut reencoded = Vec::new();
        encode_path(&mut reencoded, &decoded);
        assert_eq!(reencoded, bytes);
        for truncated_length in 0..bytes.len() {
            assert!(decode_path(&mut Cursor::new(&bytes[..truncated_length])).is_err());
        }
    }
}

#[test]
fn reference_type_and_referent_path_round_trip_without_erasing_custody() {
    for (access, descriptor_cleanup) in [
        (StructuralAccess::MutableBorrow, false),
        (StructuralAccess::SharedBorrow, true),
        (StructuralAccess::MutableBorrow, true),
        (StructuralAccess::WriteOnlyBorrow, true),
    ] {
        let declaration = StructuralTypeDeclaration {
            quotient_carrier: None,
            no_code_affine_cleanup: false,
            descriptor_cleanup,
            scalar_restriction: None,
            id: StructuralTypeId::new(7).unwrap(),
            identity: terminal_psi::TerminalIdentityIndex(4),
            shape: StructuralTypeShape::Reference {
                referent: StructuralTypeId::new(3).unwrap(),
                access,
            },
        };
        let mut bytes = Vec::new();
        encode_type(&mut bytes, &declaration).unwrap();
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_type(&mut cursor).unwrap(), declaration);
        assert_eq!(cursor.remaining(), 0);
    }
    let path = vec![StructuralPathSegment::Referent];
    let mut bytes = Vec::new();
    encode_path(&mut bytes, &path);
    let mut cursor = Cursor::new(&bytes);
    assert_eq!(decode_path(&mut cursor).unwrap(), path);
    assert_eq!(cursor.remaining(), 0);
}

#[test]
fn block_parameter_place_round_trip_retains_exact_tag_block_and_position() {
    for (block, position) in [(1, 0), (2, 0), (1, 1), (u64::MAX, u32::MAX)] {
        let place = StructuralPlaceDeclaration {
            id: PlaceId::new(7).unwrap(),
            kind: StructuralPlaceKind::BlockParameter {
                block: BlockId::new(block).unwrap(),
                position,
            },
        };
        let mut encoded = Vec::new();
        encode_place(&mut encoded, place);
        let mut expected = 7_u64.to_le_bytes().to_vec();
        expected.push(8);
        crate::codec_bytes::push_encoded(&mut expected, block.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut expected, position.to_le_bytes());
        assert_eq!(encoded, expected);
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(decode_place(&mut cursor).unwrap(), place);
        assert_eq!(cursor.remaining(), 0);
    }
}

#[test]
fn block_parameter_place_rejects_zero_block_and_truncated_coordinates() {
    let mut encoded = Vec::new();
    encode_place(
        &mut encoded,
        StructuralPlaceDeclaration {
            id: PlaceId::new(7).unwrap(),
            kind: StructuralPlaceKind::BlockParameter {
                block: BlockId::new(3).unwrap(),
                position: 0,
            },
        },
    );
    for length in 0..encoded.len() {
        assert!(matches!(
            decode_place(&mut Cursor::new(&encoded[..length])),
            Err(FixedViewCopyDecodeError::Truncated)
        ));
    }
    crate::codec_bytes::patch_encoded(&mut encoded, 9, 0_u64.to_le_bytes());
    assert!(matches!(
        decode_place(&mut Cursor::new(&encoded)),
        Err(FixedViewCopyDecodeError::InvalidSemanticId(0))
    ));
}

/// A borrowed view's access survives the round trip: a standalone view type
/// carries none, a record field's view carries its own. Decoding every view as
/// shared turned an access-free type into a different declaration.
#[test]
fn byte_view_type_round_trip_retains_its_access() {
    for access in [
        None,
        Some(StructuralAccess::SharedBorrow),
        Some(StructuralAccess::MutableBorrow),
        Some(StructuralAccess::WriteOnlyBorrow),
    ] {
        let declaration = StructuralTypeDeclaration {
            quotient_carrier: None,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: StructuralTypeId::new(7).unwrap(),
            identity: terminal_psi::TerminalIdentityIndex(5),
            shape: StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { access },
            ),
        };
        let mut bytes = Vec::new();
        encode_type(&mut bytes, &declaration).unwrap();
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_type(&mut cursor).unwrap(), declaration);
        assert_eq!(cursor.remaining(), 0);
    }
}

#[test]
fn case_path_round_trip_retains_canonical_identity_and_rejects_zero() {
    let case = semantic_vocabulary::StructuralCaseId::new(41).unwrap();
    let path = vec![
        StructuralPathSegment::Case(case),
        StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(6)),
    ];
    let mut bytes = Vec::new();
    encode_path(&mut bytes, &path);
    let mut cursor = Cursor::new(&bytes);
    assert_eq!(decode_path(&mut cursor).unwrap(), path);
    assert_eq!(cursor.remaining(), 0);
    let mut other = Vec::new();
    encode_path(
        &mut other,
        &[
            StructuralPathSegment::FixedIndex(case.get()),
            StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(6)),
        ],
    );
    assert_ne!(bytes, other);
    let mut zero = Vec::new();
    encode_path(&mut zero, &[StructuralPathSegment::Case(case)]);
    let start = zero.len() - 8;
    zero[start..].fill(0);
    assert!(matches!(
        decode_path(&mut Cursor::new(&zero)),
        Err(FixedViewCopyDecodeError::InvalidSemanticId(0))
    ));
    bytes.pop();
    assert!(decode_path(&mut Cursor::new(&bytes)).is_err());
}
