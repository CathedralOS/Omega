use super::{Reader, Writer, decode_operation, encode_operation};
use semantic_vocabulary::{ClaimId, OperationId, PlaceId, StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralOperationResult, StructuralPathQualification,
    StructuralPathSegment, StructuralResultClaimBinding, TerminalIdentityIndex,
};

#[test]
fn placed_reference_wire_preserves_owner_roles_and_backing_kind() {
    let source = StructuralArgument {
        place: PlaceId::new(2).unwrap(),
        access: StructuralAccess::MutableBorrow,
        path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(4))],
    };
    let resident = terminal_psi::ResidentReferenceSource {
        base: semantic_vocabulary::StructuralFieldId::new(7).unwrap(),
        length: semantic_vocabulary::StructuralFieldId::new(8).unwrap(),
        granted: StructuralDomainId::new(9).unwrap(),
        resident: StructuralDomainId::new(10).unwrap(),
    };
    let mut encodings = Vec::new();
    for roles in [
        None,
        Some(resident),
        Some(terminal_psi::ResidentReferenceSource {
            base: resident.length,
            length: resident.base,
            ..resident
        }),
    ] {
        let mut writer = Writer::default();
        super::storage_operations::encode_establish_placed_reference(
            &mut writer,
            source.clone(),
            roles,
        )
        .unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            reader.u8().unwrap(),
            super::operation_tags::ESTABLISH_PLACED_REFERENCE
        );
        assert_eq!(
            super::storage_operations::decode_establish_placed_reference(&mut reader).unwrap(),
            OperationKind::EstablishPlacedReference {
                source: source.clone(),
                resident: roles
            }
        );
        assert_eq!(reader.remaining(), 0);
        encodings.push(bytes);
    }
    assert_ne!(encodings[0], encodings[1]);
    assert_ne!(encodings[1], encodings[2]);
}

#[test]
fn owned_value_wire_preserves_projected_source_and_claim_identity() {
    let path = vec![StructuralPathSegment::Field(TerminalIdentityIndex(4))];
    let operation = Operation {
        id: OperationId::new(9).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        kind: OperationKind::MoveStructuralValue {
            source: StructuralArgument {
                place: PlaceId::new(2).unwrap(),
                path,
                access: StructuralAccess::Owned,
            },
        },
        result: OperationResult::Structural(StructuralOperationResult {
            place: PlaceId::new(3).unwrap(),
            structural_type: StructuralTypeId::new(1).unwrap(),
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: vec![StructuralDomainId::new(5).unwrap()],
            projected_qualifications: vec![StructuralPathQualification {
                path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(6))],
                domain: StructuralDomainId::new(7).unwrap(),
            }],
            qualification_establishments: Vec::new(),
            claims: vec![StructuralResultClaimBinding {
                claim: ClaimId::new(8).unwrap(),
                path: Vec::new(),
            }],
        }),
    };
    let mut writer = Writer::default();
    encode_operation(&mut writer, &operation).unwrap();
    let bytes = writer.finish();
    let mut reader = Reader::new(&bytes);
    assert_eq!(decode_operation(&mut reader).unwrap(), operation);
    assert_eq!(reader.remaining(), 0);

    let mut copy = operation.clone();
    copy.kind = OperationKind::StructuralLeafCopy {
        source: PlaceId::new(2).unwrap(),
        path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(4))],
    };
    let mut writer = Writer::default();
    encode_operation(&mut writer, &copy).unwrap();
    assert_ne!(
        bytes,
        writer.finish(),
        "copy and move must have distinct wire meanings"
    );
}
