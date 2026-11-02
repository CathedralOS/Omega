//! Final native result-placement replay for admitted boundary realizations.

#[cfg(test)]
use crate::object_artifact::test_support::structural_declaration;
use semantic_vocabulary::{IntegerSign, ScalarType};

pub(crate) fn hosted_read_byte_declaration_is_valid(
    declaration: &terminal_psi::StructuralTypeDeclaration,
) -> bool {
    let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return false;
    };
    let [empty, byte] = cases.as_slice() else {
        return false;
    };
    let [field] = byte.fields.as_slice() else {
        return false;
    };
    empty.fields.is_empty()
        && declaration.identity != terminal_psi::TerminalIdentityIndex::ABSENT
        && empty.identity != terminal_psi::TerminalIdentityIndex::ABSENT
        && byte.identity != terminal_psi::TerminalIdentityIndex::ABSENT
        && field.identity != terminal_psi::TerminalIdentityIndex::ABSENT
        && empty.id < byte.id
        && empty.identity != byte.identity
        && !field.relevance.is_erased()
        && match field.field_type {
            terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                !integer.is_address()
                    && integer.sign() == IntegerSign::Signed
                    && integer.bits() == 32
            }
            terminal_psi::StructuralFieldType::BoundedInteger(bounds) => {
                let integer = bounds.integer_type();
                !integer.is_address()
                    && integer.sign() == IntegerSign::Signed
                    && integer.bits() == 32
                    && bounds.contains(semantic_vocabulary::IntegerValue::Signed(0))
                    && bounds.contains(semantic_vocabulary::IntegerValue::Signed(255))
            }
            _ => false,
        }
}

#[cfg(test)]
pub(crate) fn test_byte_read_declaration(
    id: semantic_vocabulary::StructuralTypeId,
) -> terminal_psi::StructuralTypeDeclaration {
    structural_declaration(
        id,
        terminal_psi::TerminalIdentityIndex(1),
        terminal_psi::StructuralTypeShape::Sum {
            cases: vec![
                terminal_psi::StructuralCaseDeclaration {
                    id: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(2),
                    fields: vec![],
                },
                terminal_psi::StructuralCaseDeclaration {
                    id: semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(3),
                    fields: vec![terminal_psi::StructuralFieldDeclaration {
                        inline_carrier: None,
                        storage: None,
                        id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                        identity: terminal_psi::TerminalIdentityIndex(4),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: terminal_psi::StructuralFieldType::BoundedInteger(
                            semantic_vocabulary::BoundedIntegerType::new(
                                semantic_vocabulary::IntegerType::new(IntegerSign::Signed, 32)
                                    .unwrap(),
                                semantic_vocabulary::IntegerValue::Signed(0),
                                semantic_vocabulary::IntegerValue::Signed(255),
                            )
                            .unwrap(),
                        ),
                    }],
                },
            ],
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{hosted_read_byte_declaration_is_valid, test_byte_read_declaration};
    use semantic_vocabulary::StructuralTypeId;

    #[test]
    fn read_byte_declaration_replay_rejects_noncanonical_case_and_field_custody() {
        let original = test_byte_read_declaration(StructuralTypeId::new(3).unwrap());
        for mutation in 0..8 {
            let mut changed = original.clone();
            let terminal_psi::StructuralTypeShape::Sum { cases } = &mut changed.shape else {
                unreachable!()
            };
            match mutation {
                0 => changed.identity = terminal_psi::TerminalIdentityIndex(0),
                1 => cases[0].identity = terminal_psi::TerminalIdentityIndex(0),
                2 => cases[1].identity = terminal_psi::TerminalIdentityIndex(0),
                3 => cases[1].fields[0].identity = terminal_psi::TerminalIdentityIndex(0),
                4 => cases[1].id = cases[0].id,
                5 => cases[0].id = semantic_vocabulary::StructuralCaseId::new(3).unwrap(),
                6 => cases[1].identity = cases[0].identity.clone(),
                7 => cases[1].fields[0].relevance = terminal_psi::BindingRelevance::Erased,
                _ => unreachable!(),
            }
            assert!(
                !hosted_read_byte_declaration_is_valid(&changed),
                "mutation {mutation}"
            );
        }
    }
}
