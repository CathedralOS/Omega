//! Declaration dependencies of inline scalar and byte fields.
//!
//! A carrier names the original declared type, not another owned payload.
//! Field-only predicates remain on the field. Ingress checks implication in
//! that direction so a carrier cannot strengthen the field's promised values.

use semantic_vocabulary::StructuralFieldId;
use terminal_psi::{
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralTypeDeclaration, StructuralTypeShape,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineCarrierError {
    pub field: StructuralFieldId,
    pub reason: &'static str,
}

/// Original storage identity, never authority to construct, copy, or borrow
/// that value. Ingress validates predicates; operations retain field duties.
pub fn inline_carrier_declaration<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration>,
    field: &StructuralFieldDeclaration,
) -> Option<&'a StructuralTypeDeclaration> {
    if field.relevance.is_erased() {
        return None;
    }
    let identity = field.inline_carrier?;
    let carrier = crate::exactly_one::exactly_one(types, |candidate| candidate.id == identity)?;
    match (&field.field_type, &carrier.shape) {
        (
            StructuralFieldType::Scalar(_)
            | StructuralFieldType::IeeeFloat(_)
            | StructuralFieldType::BoundedInteger(_)
            | StructuralFieldType::RefinedScalar(_),
            StructuralTypeShape::PrimitiveScalar(scalar),
        ) if field.field_type.scalar_type() == Some(*scalar) => Some(carrier),
        (_, StructuralTypeShape::ByteSequence(declared))
            if field
                .field_type
                .byte_sequence_carrier()
                .is_some_and(|field| crate::byte_carrier_geometry_matches(field, *declared)) =>
        {
            Some(carrier)
        }
        _ => None,
    }
}

pub fn validate_inline_carriers(
    types: &[StructuralTypeDeclaration],
    declaration: &StructuralTypeDeclaration,
) -> Result<(), InlineCarrierError> {
    let (fields, cases): (&[StructuralFieldDeclaration], &[StructuralCaseDeclaration]) =
        match &declaration.shape {
            StructuralTypeShape::Record { fields, .. } => (fields, &[]),
            StructuralTypeShape::Sum { cases } => (&[], cases),
            StructuralTypeShape::Mixed { fields, cases, .. } => (fields, cases),
            _ => (&[], &[]),
        };
    for field in fields
        .iter()
        .chain(cases.iter().flat_map(|case| &case.fields))
    {
        let Some(identity) = field.inline_carrier else {
            continue;
        };
        let fail = |reason| InlineCarrierError {
            field: field.id,
            reason,
        };
        let carrier =
            crate::exactly_one::exactly_one(types.iter(), |candidate| candidate.id == identity)
                .ok_or_else(|| fail("inline field carrier has no exact declaration"))?;
        if field.relevance.is_erased() || field.field_type.as_structural().is_some() {
            return Err(fail(
                "inline carrier cannot belong to an erased or structural field",
            ));
        }
        let compatible = match (&field.field_type, &carrier.shape) {
            (
                StructuralFieldType::Scalar(_)
                | StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::BoundedInteger(_)
                | StructuralFieldType::RefinedScalar(_),
                StructuralTypeShape::PrimitiveScalar(scalar),
            ) => {
                let plain = StructuralFieldType::Scalar(*scalar);
                crate::scalar_field_entails(
                    &field.field_type,
                    carrier.scalar_restriction.as_ref().unwrap_or(&plain),
                )
            }
            (_, StructuralTypeShape::ByteSequence(declared)) => {
                // Stored access belongs to the field. Its declared byte type
                // names the descriptor without that loan's access shell.
                field
                    .field_type
                    .byte_sequence_carrier()
                    .is_some_and(|field| crate::byte_carrier_geometry_matches(field, *declared))
            }
            _ => false,
        };
        if !compatible {
            return Err(fail(
                "inline field does not establish its declared carrier contract",
            ));
        }
    }
    Ok(())
}

/// A checked copy/loan names its exact destination. Untyped storage queries
/// retain the field's own carrier. This grants no construction or loan rights.
pub fn inline_field_presentation<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    field: &StructuralFieldDeclaration,
    expected: Option<semantic_vocabulary::StructuralTypeId>,
    contract: Option<StructuralFieldType>,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    let declared = inline_carrier_declaration(types.clone(), field)?;
    let Some(expected) = expected else {
        return Some(declared.id);
    };
    let expected = crate::exactly_one::exactly_one(types.clone(), |row| row.id == expected)?;
    let compatible = match contract {
        Some(contract) => contract.declares_leaf(expected),
        None if field.field_type.byte_sequence_carrier().is_some() => {
            crate::borrowed_referent_presentation(types, declared.id, expected.id).is_some()
        }
        None => false,
    };
    compatible.then_some(expected.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        Proposition, ScalarRefinement, ScalarTerm, ScalarType, StructuralTypeId,
    };
    use terminal_psi::{
        BindingRelevance, StructuralPathSegment, StructuralRecordStorage, TerminalIdentityIndex,
    };

    fn scalar(raw: u64) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(raw).unwrap(),
            identity: TerminalIdentityIndex(raw as u32),
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: true,
            descriptor_cleanup: true,
            quotient_carrier: None,
            scalar_restriction: None,
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        }
    }

    fn fixture() -> (Vec<StructuralTypeDeclaration>, Vec<StructuralPathSegment>) {
        let mut root = scalar(1);
        root.shape = StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Packed,
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(1).unwrap(),
                identity: TerminalIdentityIndex(4),
                relevance: BindingRelevance::Relevant,
                storage: None,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                inline_carrier: Some(StructuralTypeId::new(2).unwrap()),
            }],
        };
        (
            vec![root, scalar(2), scalar(3)],
            vec![StructuralPathSegment::Field(TerminalIdentityIndex(4))],
        )
    }

    #[test]
    fn original_field_carrier_is_exact_among_equal_contracts() {
        let (mut declarations, path) = fixture();
        for _ in 0..2 {
            let root = declarations.iter().find(|row| row.id.get() == 1).unwrap();
            validate_inline_carriers(&declarations, root).unwrap();
            assert_eq!(
                crate::runtime_structural_path_tip(declarations.iter(), root.id, &path)
                    .unwrap()
                    .id
                    .get(),
                2
            );
            declarations.swap(1, 2);
        }
        let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
            panic!("record")
        };
        fields[0].inline_carrier = Some(StructuralTypeId::new(99).unwrap());
        assert!(validate_inline_carriers(&declarations, &declarations[0]).is_err());
        assert!(
            crate::runtime_structural_path_tip(declarations.iter(), declarations[0].id, &path)
                .is_none()
        );
    }

    #[test]
    fn stronger_field_predicate_survives_a_weaker_declared_carrier() {
        let (mut declarations, path) = fixture();
        let restriction = StructuralFieldType::RefinedScalar(
            ScalarRefinement::new(
                ScalarType::Boolean,
                Proposition::Equal(
                    ScalarTerm::value(ScalarRefinement::SUBJECT, ScalarType::Boolean),
                    ScalarTerm::boolean(true),
                ),
            )
            .unwrap(),
        );
        let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
            panic!("record")
        };
        fields[0].field_type = restriction.clone();
        validate_inline_carriers(&declarations, &declarations[0]).unwrap();
        assert_eq!(
            crate::primitive_projection_restriction(declarations.iter(), declarations[0].id, &path),
            Some(&restriction)
        );
        // Reversing implication would mint an unproved predicate on the leaf.
        declarations[1].scalar_restriction = Some(restriction);
        let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
            panic!("record")
        };
        fields[0].field_type = StructuralFieldType::Scalar(ScalarType::Boolean);
        assert!(validate_inline_carriers(&declarations, &declarations[0]).is_err());
    }

    #[test]
    fn typed_field_presentation_keeps_the_requested_identity_or_refuses() {
        let (declarations, path) = fixture();
        let root = &declarations[0];
        let field = crate::runtime_structural_field(declarations.iter(), root.id, &path).unwrap();
        let expected = declarations[2].id;
        assert_eq!(
            inline_field_presentation(declarations.iter(), field, None, None),
            field.inline_carrier
        );
        assert_eq!(
            inline_field_presentation(
                declarations.iter(),
                field,
                Some(expected),
                field.field_type.canonical_leaf_type()
            ),
            Some(expected)
        );
        let missing = StructuralTypeId::new(99).unwrap();
        assert_eq!(
            inline_field_presentation(
                declarations.iter(),
                field,
                Some(missing),
                field.field_type.canonical_leaf_type()
            ),
            None
        );
        assert_eq!(
            crate::borrowed_primitive_projection_declaration(
                declarations.iter(),
                root.id,
                &path,
                Some(missing)
            ),
            None
        );
    }

    #[test]
    fn inline_carrier_rejects_wrong_scalar_atomic_and_erased_fields() {
        let (mut declarations, _) = fixture();
        for field_type in [
            StructuralFieldType::Scalar(ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    8,
                )
                .unwrap(),
            )),
            StructuralFieldType::Atomic(ScalarType::Boolean),
            StructuralFieldType::Structural(declarations[1].id),
            StructuralFieldType::Erased {
                type_identity: TerminalIdentityIndex(4),
            },
        ] {
            let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
                panic!("record")
            };
            fields[0].field_type = field_type;
            assert!(validate_inline_carriers(&declarations, &declarations[0]).is_err());
        }
    }

    #[test]
    fn byte_field_keeps_exact_carrier_access_and_extent() {
        use terminal_psi::{ByteSequenceCarrier, StructuralAccess};
        let (mut declarations, path) = fixture();
        let carrier = ByteSequenceCarrier::BoundedView {
            access: Some(StructuralAccess::MutableBorrow),
            capacity: 6,
        };
        for declaration in &mut declarations[1..] {
            declaration.shape =
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                    access: None,
                    capacity: 6,
                });
        }
        let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
            panic!("record")
        };
        fields[0].field_type = StructuralFieldType::ByteSequence(carrier);
        validate_inline_carriers(&declarations, &declarations[0]).unwrap();
        assert_eq!(
            crate::runtime_structural_path_tip(declarations.iter(), declarations[0].id, &path)
                .unwrap()
                .id,
            declarations[1].id
        );
        for wrong in [
            ByteSequenceCarrier::BoundedView {
                access: Some(StructuralAccess::SharedBorrow),
                capacity: 6,
            },
            ByteSequenceCarrier::BoundedView {
                access: None,
                capacity: 7,
            },
        ] {
            declarations[1].shape = StructuralTypeShape::ByteSequence(wrong);
            assert!(validate_inline_carriers(&declarations, &declarations[0]).is_err());
            assert!(
                crate::runtime_structural_path_tip(declarations.iter(), declarations[0].id, &path)
                    .is_none()
            );
        }
    }
}
