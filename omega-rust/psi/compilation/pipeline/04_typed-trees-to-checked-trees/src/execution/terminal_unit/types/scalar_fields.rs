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
    let (sign, bits) = match primitive {
        PrimitiveType::I8 => (IntegerSign::Signed, 8),
        PrimitiveType::I16 => (IntegerSign::Signed, 16),
        PrimitiveType::I32 => (IntegerSign::Signed, 32),
        PrimitiveType::I64 => (IntegerSign::Signed, 64),
        PrimitiveType::U8 => (IntegerSign::Unsigned, 8),
        PrimitiveType::U16 => (IntegerSign::Unsigned, 16),
        PrimitiveType::U32 => (IntegerSign::Unsigned, 32),
        PrimitiveType::U64 => (IntegerSign::Unsigned, 64),
        PrimitiveType::Selector
        | PrimitiveType::Bool
        | PrimitiveType::F32
        | PrimitiveType::F64
        | PrimitiveType::Addr => {
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
