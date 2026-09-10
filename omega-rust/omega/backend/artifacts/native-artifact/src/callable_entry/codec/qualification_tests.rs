use super::*;
use semantic_vocabulary::ScalarQualificationSetId;

fn declaration(qualification: u64) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(7).expect("value identity"),
        scalar_type: ScalarType::Boolean,
        qualifications: ScalarQualificationSetId::new(qualification),
    }
}

#[test]
fn qualified_result_cannot_silently_encode_as_bare() {
    let mut bytes = vec![19];
    assert!(matches!(
        encode_result_declaration(&mut bytes, &declaration(1)),
        Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature)
    ));
    assert_eq!(bytes, [19]);
}

#[test]
fn bare_result_wire_preserves_the_complete_supported_declaration() {
    let original = declaration(0);
    let mut bytes = Vec::new();
    encode_result_declaration(&mut bytes, &original).expect("bare result");
    let mut cursor = Cursor::new(&bytes);
    let decoded = ValueDeclaration {
        id: decode_id(&mut cursor, ValueId::new).expect("value identity"),
        scalar_type: decode_scalar(&mut cursor).expect("scalar carrier"),
        qualifications: ScalarQualificationSetId::ZERO,
    };
    assert_eq!(decoded, original);
}

#[test]
fn installed_signature_rejects_parameter_and_result_qualification() {
    let bare = declaration(0);
    let qualified = declaration(1);
    assert!(super::super::reconstruction::validate_entry_qualifications(&[bare], &bare).is_ok());
    for (parameter, result) in [(qualified, bare), (bare, qualified)] {
        assert!(matches!(
            super::super::reconstruction::validate_entry_qualifications(&[parameter], &result),
            Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature)
        ));
    }
}
