//! Closed numeric field restrictions retained independently of the raw carrier.

use super::*;
use semantic_vocabulary::{BoundedIntegerType, IntegerSign, IntegerType, IntegerValue};

pub(super) fn retain_scalar_field(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    primitive: PrimitiveType,
) -> Option<CheckedUnitStructuralFieldType> {
    let mut declared_bounds: Option<(i128, i128)> = None;
    let mut has_non_exact_domain = false;
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
                    if let typed_trees::types::TypeConstraintNode::Range {
                        minimum,
                        maximum,
                        end_inclusive,
                    } = constraint
                    {
                        // Use the same closed-expression evaluation as source range
                        // validation. Failure is unsupported, never an unbounded field.
                        let lower = i128::from(
                            validation::closed_integer_range_bound(program, *minimum)?.to_i64()?,
                        );
                        let upper = i128::from(
                            validation::closed_integer_range_maximum(
                                program,
                                *maximum,
                                *end_inclusive,
                            )?
                            .to_i64()?,
                        );
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
                type_reference = *replacement;
            }
            _ => return None,
        }
    }
    let Some((minimum, maximum)) = declared_bounds else {
        return Some(CheckedUnitStructuralFieldType::Scalar(primitive));
    };
    if has_non_exact_domain {
        return None;
    }
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
            IntegerValue::Signed(minimum.max(carrier_minimum)),
            IntegerValue::Signed(maximum.min(carrier_maximum)),
        ),
        (IntegerValue::Unsigned(carrier_minimum), IntegerValue::Unsigned(carrier_maximum)) => (
            IntegerValue::Unsigned(
                u128::try_from(minimum.max(i128::try_from(carrier_minimum).ok()?)).ok()?,
            ),
            IntegerValue::Unsigned(
                u128::try_from(maximum.min(i128::try_from(carrier_maximum).ok()?)).ok()?,
            ),
        ),
        _ => return None,
    };
    Some(CheckedUnitStructuralFieldType::BoundedInteger(
        BoundedIntegerType::new(integer_type, minimum, maximum).ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field_type(source: &str) -> (TypedTrees, TypeReferenceHandle) {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
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
    fn bounded_integer_fields_retain_signed_unsigned_and_closed_expression_ranges() {
        for (spelling, primitive, minimum, maximum) in [
            (
                "i8 [-128..=-1]",
                PrimitiveType::I8,
                IntegerValue::Signed(-128),
                IntegerValue::Signed(-1),
            ),
            (
                "i32 [0..=255]",
                PrimitiveType::I32,
                IntegerValue::Signed(0),
                IntegerValue::Signed(255),
            ),
            (
                "u64 [0..=500]",
                PrimitiveType::U64,
                IntegerValue::Unsigned(0),
                IntegerValue::Unsigned(500),
            ),
            (
                "i16 [0 - 3..=10 * 2]",
                PrimitiveType::I16,
                IntegerValue::Signed(-3),
                IntegerValue::Signed(20),
            ),
            (
                "u8 [12..=12]",
                PrimitiveType::U8,
                IntegerValue::Unsigned(12),
                IntegerValue::Unsigned(12),
            ),
            (
                "u8 [0..256]",
                PrimitiveType::U8,
                IntegerValue::Unsigned(0),
                IntegerValue::Unsigned(255),
            ),
            (
                "i8 [-128..-1]",
                PrimitiveType::I8,
                IntegerValue::Signed(-128),
                IntegerValue::Signed(-2),
            ),
        ] {
            let (program, reference) =
                field_type(&format!("data Carrier {{ value: {spelling}; }}"));
            let Some(CheckedUnitStructuralFieldType::BoundedInteger(integer)) =
                retain_scalar_field(&program, reference, &[], primitive)
            else {
                panic!("range {spelling} was erased");
            };
            assert_eq!(
                (integer.minimum(), integer.maximum()),
                (minimum, maximum),
                "{spelling}"
            );
        }
    }

    #[test]
    fn bounded_integer_field_type_substitution_preserves_the_replacement_restriction() {
        let (program, reference) =
            field_type("data Carrier<T> { value: T; } data Replacement { value: i32 [0..=255]; }");
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            panic!("formal type");
        };
        let replacement = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Replacement")
            .unwrap();
        let [DataMember::Field(field)] = program.data_members(replacement) else {
            panic!("replacement field");
        };
        let Some(CheckedUnitStructuralFieldType::BoundedInteger(integer)) = retain_scalar_field(
            &program,
            reference,
            &[(*symbol, field.type_reference)],
            PrimitiveType::I32,
        ) else {
            panic!("substituted restriction");
        };
        assert_eq!(integer.minimum(), IntegerValue::Signed(0));
        assert_eq!(integer.maximum(), IntegerValue::Signed(255));
    }

    #[test]
    fn bounded_integer_fields_do_not_turn_unsupported_ranges_into_raw_scalars() {
        for (spelling, primitive) in [
            ("f32 [0..=1]", PrimitiveType::F32),
            ("addr [0..=1]", PrimitiveType::Addr),
            ("u64 [0..=18446744073709551615]", PrimitiveType::U64),
            ("u8 [256..=300]", PrimitiveType::U8),
            ("u8 [0..0]", PrimitiveType::U8),
            ("i8 [-128..-128]", PrimitiveType::I8),
            ("i32 [0..=255] in Wrapping", PrimitiveType::I32),
        ] {
            let (program, reference) =
                field_type(&format!("data Carrier {{ value: {spelling}; }}"));
            assert!(
                retain_scalar_field(&program, reference, &[], primitive).is_none(),
                "{spelling}"
            );
        }
        let (program, reference) = field_type("data Carrier { value: u64; }");
        assert_eq!(
            retain_scalar_field(&program, reference, &[], PrimitiveType::U64),
            Some(CheckedUnitStructuralFieldType::Scalar(PrimitiveType::U64))
        );
    }

    #[test]
    fn bounded_integer_fields_reject_unevaluated_bounds_and_intersect_shells() {
        let (program, reference) =
            field_type("data Carrier<const Maximum: i32> { value: i32 [0..=Maximum]; }");
        assert!(retain_scalar_field(&program, reference, &[], PrimitiveType::I32).is_none());
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
            retain_scalar_field(&program, reference, &[], PrimitiveType::I32)
        else {
            panic!("nested range shells");
        };
        assert_eq!(integer.minimum(), IntegerValue::Signed(12));
        assert_eq!(integer.maximum(), IntegerValue::Signed(100));
    }
}
