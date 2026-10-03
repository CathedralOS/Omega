//! Complete scalar field restrictions retained independently of the raw carrier.
//! Intersect exact proof-integer endpoints before converting to the field's
//! signed or unsigned carrier; an authored endpoint can exceed that carrier.
//! General domain predicates use the shared declaration-owned projection and
//! remain conjoined with every retained interval shell.
use super::{
    CheckedUnitStructuralFieldType, PrimitiveType, SymbolHandle, TypeReferenceHandle,
    TypeReferenceNode, TypedTrees,
};
use numerics::bignum::BigInt;
use semantic_vocabulary::{BoundedIntegerType, IntegerSign, IntegerType, IntegerValue};

pub(in crate::execution::terminal_unit) fn retain_scalar_field(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    primitive: PrimitiveType,
    stated: Option<(Option<i64>, Option<i64>)>,
) -> Option<CheckedUnitStructuralFieldType> {
    // The owning data's interval-only `where` facts restrict the field as a
    // bracketed range did: every write establishes them and every read may
    // assume them. An unstated side is the carrier's own extreme.
    let mut declared_bounds: Option<(BigInt, BigInt)> = stated.map(|(lower, upper)| {
        (
            lower.map_or_else(|| BigInt::from_i128(i128::MIN), BigInt::from_i64),
            upper.map_or_else(|| BigInt::from_u128(u128::MAX), BigInt::from_i64),
        )
    });
    let mut has_non_exact_domain = false;
    let mut predicate = None;
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in program.type_reference_table.constraints(*constraints) {
                    if let typed_trees::types::TypeConstraintNode::ArithmeticDomain(domain) =
                        constraint
                    {
                        has_non_exact_domain |=
                            *domain != numerics::arithmetic::ArithmeticDomain::Exact;
                    }
                    // A declared domain whose membership is exactly an interval
                    // restricts the field as a bracketed range did: the place
                    // establishes it at every write and every read may assume
                    // it. An unstated side is the carrier's own extreme.
                    if let typed_trees::types::TypeConstraintNode::Domain(domain) = constraint {
                        let Some((lower, upper)) =
                            validation::exact_declared_domain_carrier_interval(
                                program, primitive, domain,
                            )
                        else {
                            let next = validation::scalar_field_domain_predicate(
                                program, domain, primitive,
                            )?;
                            predicate = Some(match predicate {
                                None => next,
                                Some(previous) => checked_trees::CheckedBooleanExpression::And {
                                    left: Box::new(previous),
                                    right: Box::new(next),
                                },
                            });
                            continue;
                        };
                        declared_bounds = Some(match declared_bounds {
                            Some((previous_lower, previous_upper)) => {
                                (previous_lower.max(lower), previous_upper.min(upper))
                            }
                            None => (lower, upper),
                        });
                    }
                    if matches!(constraint, typed_trees::types::TypeConstraintNode::Named(_)) {
                        return None;
                    }
                    if let typed_trees::types::TypeConstraintNode::Range {
                        minimum,
                        maximum,
                        end_inclusive,
                    } = constraint
                    {
                        // Use the same closed-expression evaluation as source range
                        // validation. Failure is unsupported, never an unbounded field.
                        let lower = validation::closed_integer_range_bound(program, *minimum)?;
                        let upper = validation::closed_integer_range_maximum(
                            program,
                            *maximum,
                            *end_inclusive,
                        )?;
                        declared_bounds = Some(match declared_bounds {
                            Some((previous_lower, previous_upper)) => {
                                (previous_lower.max(lower), previous_upper.min(upper))
                            }
                            None => (lower, upper),
                        });
                    }
                }
                type_reference = *base_type;
            }
            TypeReferenceNode::Named { symbol, .. } => {
                let Some((_, replacement)) = substitutions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == symbol)
                else {
                    break;
                };
                // A substitution can bind a parameter to its own reference;
                // rewriting it again only loops.
                if *replacement == type_reference {
                    break;
                }
                type_reference = *replacement;
            }
            _ => return None,
        }
    }
    let atomic = matches!(program.type_reference_table.type_reference(type_reference),
        TypeReferenceNode::Named { symbol, .. }
            if program.symbols.builtin_type_atom(*symbol).is_some_and(|atom| atom.is_atomic()));
    if atomic {
        // Atomic events have no restricted-resident establishment proof. Do
        // not silently erase a declared interval while retaining the cell.
        return (declared_bounds.is_none() && predicate.is_none())
            .then_some(CheckedUnitStructuralFieldType::Atomic(primitive));
    }
    if has_non_exact_domain && (declared_bounds.is_some() || predicate.is_some()) {
        return None;
    }
    let Some((minimum, maximum)) = declared_bounds else {
        return validation::scalar_field_restriction(primitive, None, predicate);
    };
    let (sign, bits) = match primitive {
        PrimitiveType::I8 => (IntegerSign::Signed, 8),
        PrimitiveType::I16 => (IntegerSign::Signed, 16),
        PrimitiveType::I32 => (IntegerSign::Signed, 32),
        PrimitiveType::I64 => (IntegerSign::Signed, 64),
        PrimitiveType::U8 => (IntegerSign::Unsigned, 8),
        PrimitiveType::U16 => (IntegerSign::Unsigned, 16),
        PrimitiveType::U32 => (IntegerSign::Unsigned, 32),
        PrimitiveType::U64 => (IntegerSign::Unsigned, 64),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 | PrimitiveType::Addr => {
            return None;
        }
    };
    let integer_type = IntegerType::new(sign, bits).ok()?;
    let (minimum, maximum) = match (integer_type.minimum_value(), integer_type.maximum_value()) {
        (IntegerValue::Signed(carrier_minimum), IntegerValue::Signed(carrier_maximum)) => (
            IntegerValue::Signed(i128::from(
                minimum.max(BigInt::from_i128(carrier_minimum)).to_i64()?,
            )),
            IntegerValue::Signed(i128::from(
                maximum.min(BigInt::from_i128(carrier_maximum)).to_i64()?,
            )),
        ),
        (IntegerValue::Unsigned(carrier_minimum), IntegerValue::Unsigned(carrier_maximum)) => (
            IntegerValue::Unsigned(u128::from(
                minimum.max(BigInt::from_u128(carrier_minimum)).to_u64()?,
            )),
            IntegerValue::Unsigned(u128::from(
                maximum.min(BigInt::from_u128(carrier_maximum)).to_u64()?,
            )),
        ),
        _ => return None,
    };
    validation::scalar_field_restriction(
        primitive,
        Some(BoundedIntegerType::new(integer_type, minimum, maximum).ok()?),
        predicate,
    )
}

#[cfg(test)]
mod tests {
    use super::super::DataMember;
    use super::{
        CheckedUnitStructuralFieldType, IntegerValue, PrimitiveType, TypeReferenceHandle,
        TypeReferenceNode, TypedTrees,
    };
    use crate::execution::terminal_unit::types::scalar_fields::retain_scalar_field;
    use crate::tests::front_end::typed_program;

    fn field_type(source: &str) -> (TypedTrees, TypeReferenceHandle) {
        let program = typed_program(source);
        let carrier = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Carrier")
            .unwrap();
        let [DataMember::Field(field)] = program.data_members(carrier) else {
            panic!("one field");
        };
        let reference = field.type_reference;
        (program, reference)
    }

    #[test]
    fn scalar_field_predicates_keep_named_subjects_and_complete_boolean_restrictions() {
        for predicate in ["value != 0", "value < 0 || 10 <= value", "!(value == 0)"] {
            let (program, reference) = field_type(&format!(
                "domain i32::Restricted(value) requires {predicate};
                 data Carrier {{ value: i32 in Restricted; }}"
            ));
            assert!(
                matches!(
                    retain_scalar_field(&program, reference, &[], PrimitiveType::I32, None),
                    Some(CheckedUnitStructuralFieldType::RefinedScalar {
                        primitive: PrimitiveType::I32,
                        ..
                    })
                ),
                "{predicate}"
            );
            assert!(matches!(
                retain_scalar_field(
                    &program,
                    reference,
                    &[],
                    PrimitiveType::I32,
                    Some((Some(-10), Some(20)))
                ),
                Some(CheckedUnitStructuralFieldType::RefinedScalar {
                    predicate: checked_trees::CheckedBooleanExpression::And { .. },
                    ..
                })
            ));
        }
        let (program, reference) = field_type(
            "domain bool::Enabled(flag) requires flag;
             data Carrier { value: bool in Enabled; }",
        );
        assert!(matches!(
            retain_scalar_field(&program, reference, &[], PrimitiveType::Bool, None),
            Some(CheckedUnitStructuralFieldType::RefinedScalar {
                primitive: PrimitiveType::Bool,
                predicate: checked_trees::CheckedBooleanExpression::Parameter { position: 0 },
            })
        ));
    }

    #[test]
    fn scalar_fields_do_not_drop_unsupported_predicates_or_domain_identity() {
        let (program, reference) = field_type(
            "domain i32::Even(value) requires value % 2 == 0;
             data Carrier { value: i32 in Even; }",
        );
        assert!(retain_scalar_field(&program, reference, &[], PrimitiveType::I32, None).is_none());
        let (mut program, reference) = field_type(
            "domain i32::Restricted(value) requires value != 0;
             data Carrier { value: i32 in Restricted; }",
        );
        let TypeReferenceNode::Constrained { constraints, .. } =
            *program.type_reference_table.type_reference(reference)
        else {
            panic!("constrained field");
        };
        let mut rows = program
            .type_reference_table
            .constraints(constraints)
            .to_vec();
        let typed_trees::types::TypeConstraintNode::Domain(domain) = &mut rows[0] else {
            panic!("domain constraint");
        };
        domain.semantic_id = language_semantics::SemanticDomainId::NULL;
        let constraints = program.type_reference_table.insert_constraints(rows);
        let TypeReferenceNode::Constrained { base_type, .. } =
            *program.type_reference_table.type_reference(reference)
        else {
            panic!("constrained field");
        };
        program.type_reference_table.substitute_node(
            reference,
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            },
        );
        assert!(retain_scalar_field(&program, reference, &[], PrimitiveType::I32, None).is_none());
    }

    #[test]
    fn scalar_field_predicates_reject_routed_domains_and_authored_closed_operators() {
        for source in [
            "domain i32::Restricted(value) requires value != 0 established by Issuer::issue;
             trait Issuer { machine issue() -> i32 ensures result in i32::Restricted; }
             data Carrier { value: i32 in Restricted; }",
            "operator + i32::custom(left: i32, right: i32) -> i32;
             domain i32::Restricted(value) requires value != 1 + 1;
             data Carrier { value: i32 in Restricted; }",
        ] {
            let (program, reference) = field_type(source);
            assert!(
                retain_scalar_field(&program, reference, &[], PrimitiveType::I32, None).is_none()
            );
        }
    }

    #[test]
    fn bounded_integer_fields_do_not_turn_unsupported_ranges_into_raw_scalars() {
        for (spelling, primitive) in [
            ("f32 [0..=1]", PrimitiveType::F32),
            ("addr [0..=1]", PrimitiveType::Addr),
            (
                "u64 [18446744073709551616..=18446744073709551617]",
                PrimitiveType::U64,
            ),
            (
                "u64 [18446744073709551615..18446744073709551615]",
                PrimitiveType::U64,
            ),
            ("u8 [256..=300]", PrimitiveType::U8),
            ("u8 [0..0]", PrimitiveType::U8),
            ("i8 [-128..-128]", PrimitiveType::I8),
            ("i32 [0..=255] in Wrapping", PrimitiveType::I32),
        ] {
            let (program, reference) =
                field_type(&format!("data Carrier {{ value: {spelling}; }}"));
            assert!(
                retain_scalar_field(&program, reference, &[], primitive, None).is_none(),
                "{spelling}"
            );
        }
        let (program, reference) = field_type("data Carrier { value: u64; }");
        assert_eq!(
            retain_scalar_field(&program, reference, &[], PrimitiveType::U64, None),
            Some(CheckedUnitStructuralFieldType::Scalar(PrimitiveType::U64))
        );
    }

    #[test]
    fn bounded_integer_fields_reject_unevaluated_bounds_and_intersect_shells() {
        let (program, reference) =
            field_type("data Carrier<const Maximum: i32> { value: i32 [0..=Maximum]; }");
        assert!(retain_scalar_field(&program, reference, &[], PrimitiveType::I32, None).is_none());
        let (mut program, reference) = field_type(
            "data Carrier { value: i32 [0..=255]; } data Restriction { value: i32 [12..=100]; }",
        );
        let restriction = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Restriction")
            .unwrap();
        let [DataMember::Field(field)] = program.data_members(restriction) else {
            panic!("restriction field");
        };
        let TypeReferenceNode::Constrained { constraints, .. } = *program
            .type_reference_table
            .type_reference(field.type_reference)
        else {
            panic!("restriction shell");
        };
        // Exercise nested typed shells directly; source syntax has one range slot.
        let reference = program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: reference,
                constraints,
            });
        let Some(CheckedUnitStructuralFieldType::BoundedInteger(integer)) =
            retain_scalar_field(&program, reference, &[], PrimitiveType::I32, None)
        else {
            panic!("nested range shells");
        };
        assert_eq!(integer.minimum(), IntegerValue::Signed(12));
        assert_eq!(integer.maximum(), IntegerValue::Signed(100));
    }
}
