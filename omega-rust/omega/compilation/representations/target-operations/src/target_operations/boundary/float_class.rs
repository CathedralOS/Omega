//! The structural output contract of an already selected classification builtin.
//! Selection supplies the execution identity. This shape check never selects
//! an implementation from a source name or from a similarly shaped sum.
use calling_conventions::{ConventionalSumLayout, ValueShape};
use semantic_vocabulary::ScalarType;
use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape};

pub fn conventional_float_class_layout(
    declaration: &StructuralTypeDeclaration,
) -> Option<ConventionalSumLayout> {
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return None;
    };
    let [nan, infinity, normal, subnormal, zero] = cases.as_slice() else {
        return None;
    };
    if !nan.fields.is_empty()
        || [infinity, normal, subnormal, zero].iter().any(|case| {
            !matches!(case.fields.as_slice(), [field] if !field.relevance.is_erased()
            && field.field_type == StructuralFieldType::Scalar(ScalarType::Boolean))
        })
    {
        return None;
    }
    let layout = calling_conventions::evaluate_conventional_sum_layout(
        &[],
        &[
            vec![],
            vec![ValueShape::integer(1, 1)],
            vec![ValueShape::integer(1, 1)],
            vec![ValueShape::integer(1, 1)],
            vec![ValueShape::integer(1, 1)],
        ],
    )
    .ok()?;
    // The selected scalar form computes these complete bytes. Admission derives
    // the layout first; a future different sum layout must change the form.
    if layout.shape != ValueShape::integer(8, 4)
        || layout.tag_byte_offset != 0 || layout.tag_shape != ValueShape::integer(4, 4)
        || layout.cases[1..].iter().any(|case| !matches!(case.fields.as_slice(), [field] if field.byte_offset == 4 && field.shape == ValueShape::integer(1, 1))) {
        return None;
    }
    Some(layout)
}

#[cfg(test)]
mod tests {
    use super::conventional_float_class_layout;
    use calling_conventions::ValueShape;
    use semantic_vocabulary::{ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
        StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
    };

    fn declaration() -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            quotient_carrier: None,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: StructuralTypeId::new(1).unwrap(),
            identity: terminal_psi::TerminalIdentityIndex(1),
            shape: StructuralTypeShape::Sum {
                cases: (0..5)
                    .map(|index| StructuralCaseDeclaration {
                        id: StructuralCaseId::new(index + 1).unwrap(),
                        identity: terminal_psi::TerminalIdentityIndex(10 + index as u32),
                        fields: if index == 0 {
                            vec![]
                        } else {
                            vec![StructuralFieldDeclaration {
                                id: StructuralFieldId::new(index).unwrap(),
                                identity: terminal_psi::TerminalIdentityIndex(2),
                                relevance: BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                                storage: None,
                            }]
                        },
                    })
                    .collect(),
            },
        }
    }

    #[test]
    fn conventional_float_class_geometry_is_derived_without_identity_selection() {
        let layout = conventional_float_class_layout(&declaration()).unwrap();
        assert_eq!(layout.shape, ValueShape::integer(8, 4));
        assert_eq!(layout.tag_byte_offset, 0);
        assert_eq!(layout.tag_shape, ValueShape::integer(4, 4));
        assert!(layout.cases[0].fields.is_empty());
        for case in &layout.cases[1..] {
            assert_eq!(case.fields[0].byte_offset, 4);
            assert_eq!(case.fields[0].shape, ValueShape::integer(1, 1));
        }
    }

    #[test]
    fn conventional_float_class_geometry_refuses_malformed_payloads() {
        // These are independent input-shape refusals, not forged compiler rows.
        for mutation in 0..7 {
            let mut declaration = declaration();
            let StructuralTypeShape::Sum { cases } = &mut declaration.shape else {
                unreachable!();
            };
            match mutation {
                0 => {
                    cases.pop();
                }
                1 => {
                    cases.push(cases[4].clone());
                }
                2 => {
                    cases[0].fields = cases[1].fields.clone();
                }
                3 => {
                    cases[1].fields.clear();
                }
                4 => {
                    cases[1].fields[0].field_type = StructuralFieldType::Scalar(
                        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32),
                    );
                }
                5 => {
                    cases[1].fields[0].relevance = BindingRelevance::Erased;
                }
                6 => {
                    let second = cases[2].fields[0].clone();
                    cases[1].fields.push(second);
                }
                _ => unreachable!(),
            }
            assert!(
                conventional_float_class_layout(&declaration).is_none(),
                "mutation {mutation}"
            );
        }
        let mut declaration = declaration();
        declaration.shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
        assert!(conventional_float_class_layout(&declaration).is_none());
    }
}
