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

/// Whether the record or case-payload field `field` keeps a scalar
/// restriction in its Terminal declaration: a bound or predicate from its type
/// or its owner's interval-only `where` facts, or an atomic cell. `leaf_type`
/// is the field's type as the projecting place sees it.
///
/// Terminal keeps such restrictions out of borrowed shapes, so a restricted
/// field never lends its storage to a reference carrier. Its observations and
/// stores reach it through the owning record, where reads transport the
/// declared invariant and writes re-prove it. This applies the same retention
/// as shape collection; it is a predicate, not a second field catalog.
pub(crate) fn field_retains_scalar_restriction(
    program: &dyn ::validation::CheckingInput,
    field: SymbolHandle,
    leaf_type: TypeReferenceHandle,
) -> bool {
    let Some(primitive) = program.primitive_type_reference(leaf_type) else {
        return false;
    };
    !matches!(
        retain_scalar_field(
            program,
            leaf_type,
            &[],
            primitive,
            stated_field_interval(program, field),
        ),
        Some(CheckedUnitStructuralFieldType::Scalar(_))
    )
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
