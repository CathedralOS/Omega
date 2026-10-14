//! Complete scalar field restrictions retained independently of the raw carrier.
//! Intersect exact proof-integer endpoints before converting to the field's
//! signed or unsigned carrier; an authored endpoint can exceed that carrier.
//! General domain predicates use the shared declaration-owned projection and
//! remain conjoined with every retained interval shell.
use super::{
    CheckedUnitStructuralFieldType, DataMember, PrimitiveType, SymbolHandle, TypeReferenceHandle,
    TypeReferenceNode,
};
use numerics::bignum::BigInt;
use semantic_vocabulary::{BoundedIntegerType, IntegerValue};

/// Whether lending a scalar field must remain an owning-record observation.
/// Exclusive access retains its complete receiving restriction, and atomic
/// storage never becomes an ordinary scalar reference. A shared loan may expose
/// a bounded or refined scalar's carrier: it retains the exact backing and
/// lifetime but supplies no store or invariant-establishment authority.
pub(crate) fn field_requires_place_observation(
    program: &dyn ::validation::CheckingInput,
    field: SymbolHandle,
    leaf_type: TypeReferenceHandle,
    access: checked_trees::CheckedStructuralAccess,
) -> bool {
    let Some(primitive) = program.primitive_type_reference(leaf_type) else {
        return false;
    };
    match retain_scalar_field(
        program,
        leaf_type,
        &[],
        primitive,
        stated_field_interval(program, field),
    ) {
        Some(CheckedUnitStructuralFieldType::Scalar(_)) => false,
        Some(CheckedUnitStructuralFieldType::BoundedInteger(_))
        | Some(CheckedUnitStructuralFieldType::RefinedScalar { .. })
            if access == checked_trees::CheckedStructuralAccess::SharedBorrow =>
        {
            false
        }
        _ => true,
    }
}

/// Whether primitive storage can retain the declaration's complete receiving
/// restriction. Parameters and local declarations establish the same storage;
/// replacements use the exact assignment judgment against its retained range.
/// A domain kept only in custody (such as Finite) is not an unrestricted store,
/// and arbitrary predicates still require their declaration-owned operation.
pub(in crate::execution::terminal_unit) fn supports_primitive_storage(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    let Some(primitive) = program.primitive_type_reference(type_reference) else {
        return false;
    };
    match retain_scalar_field(program, type_reference, &[], primitive, None) {
        Some(CheckedUnitStructuralFieldType::Scalar(_)) => {
            validation::is_plain_scalar_storage(program, type_reference)
        }
        Some(CheckedUnitStructuralFieldType::BoundedInteger(_)) => true,
        _ => false,
    }
}

/// The owner's interval-only `where` bound on `field`: the record's facts for
/// a direct field, the selected case's facts for a payload field.
fn stated_field_interval(
    program: &dyn ::validation::CheckingInput,
    field: SymbolHandle,
) -> Option<(Option<i64>, Option<i64>)> {
    let (definition, _) = program.any_field_owner(field)?;
    let intervals = if program.data_field(definition, field).is_some() {
        validation::data_where_field_intervals(program, definition)
    } else {
        let variant = program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant)
                    if program
                        .data_payload_fields(variant)
                        .iter()
                        .any(|payload| payload.symbol == field) =>
                {
                    Some(variant)
                }
                _ => None,
            })?;
        validation::case_where_field_intervals(program, variant)
    }?;
    intervals
        .into_iter()
        .find(|(symbol, _, _)| *symbol == field)
        .map(|(_, low, high)| (low, high))
}

pub(in crate::execution::terminal_unit) fn retain_scalar_field(
    program: &dyn ::validation::CheckingInput,
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
                    // A compiler-known value domain is not a storage
                    // restriction: `in Finite` names value-domain membership
                    // the custody channel establishes at construction and
                    // demands at consumption — the same invariant-window
                    // discipline `where` facts already ride. Per-store
                    // refinement would misread transient poison as a
                    // violation, so the field keeps its scalar carrier here.
                    if let typed_trees::types::TypeConstraintNode::Domain(domain) = constraint
                        && domain.subject
                            == typed_trees::types::DomainConstraintSubject::Value(
                                language_semantics::value_domain::ValueDomain::Finite,
                            )
                    {
                        continue;
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
                        && matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
                    {
                        // IEEE endpoints are values of the field's own format,
                        // not integer bounds: the restriction is the ordered
                        // comparison pair a parameter's range publishes.
                        let next = float_range_predicate(
                            program,
                            primitive,
                            *minimum,
                            *maximum,
                            *end_inclusive,
                        )?;
                        predicate = Some(match predicate {
                            None => next,
                            Some(previous) => checked_trees::CheckedBooleanExpression::And {
                                left: Box::new(previous),
                                right: Box::new(next),
                            },
                        });
                    } else if let typed_trees::types::TypeConstraintNode::Range {
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
            // A selector is a plain word with no restriction of its own.
            TypeReferenceNode::DynamicTrait { .. } => break,
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
    let integer_type = crate::values::integer_type(primitive)?;
    let carrier_integer = |value| match value {
        IntegerValue::Signed(value) => BigInt::from_i128(value),
        IntegerValue::Unsigned(value) => BigInt::from_u128(value),
    };
    let minimum = minimum.max(carrier_integer(integer_type.minimum_value()));
    let maximum = maximum.min(carrier_integer(integer_type.maximum_value()));
    if minimum > maximum {
        // An empty predicate makes construction impossible, not storage
        // undefined. Inactive cases still need their ordinary carrier shape;
        // retain false through the general predicate representation rather
        // than dropping the restriction or refusing the entire containing type.
        return validation::scalar_field_restriction(
            primitive,
            None,
            Some(checked_trees::CheckedBooleanExpression::Constant(false)),
        );
    }
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

/// `minimum <= value` and `value <= maximum` (`<` when exclusive) over the
/// field's IEEE format. Ordered comparisons reject NaN, as the range does.
fn float_range_predicate(
    program: &dyn ::validation::CheckingInput,
    primitive: PrimitiveType,
    minimum: typed_trees::expression::ExpressionHandle,
    maximum: typed_trees::expression::ExpressionHandle,
    end_inclusive: bool,
) -> Option<checked_trees::CheckedBooleanExpression> {
    use checked_trees::CheckedIeeeFloatComparisonKind as Kind;
    use checked_trees::CheckedScalarExpression as Scalar;
    let endpoint = |expression| {
        Some(Box::new(Scalar::IeeeFloatLiteral {
            value: validation::closed_float_range_endpoint(program, expression, primitive)?,
        }))
    };
    let subject = || {
        Box::new(Scalar::Parameter {
            position: 0,
            primitive_type: primitive,
        })
    };
    Some(checked_trees::CheckedBooleanExpression::And {
        left: Box::new(
            checked_trees::CheckedBooleanExpression::ScalarIeeeFloatComparison {
                kind: Kind::LessOrEqual,
                left: endpoint(minimum)?,
                right: subject(),
            },
        ),
        right: Box::new(
            checked_trees::CheckedBooleanExpression::ScalarIeeeFloatComparison {
                kind: if end_inclusive {
                    Kind::LessOrEqual
                } else {
                    Kind::LessThan
                },
                left: subject(),
                right: endpoint(maximum)?,
            },
        ),
    })
}
