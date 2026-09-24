//! Range constraint intervals, dependent maxima and containment.

use crate::proof_contracts::arithmetic_domains::interval::Interval;
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// S4: the value range a type declares via a `Range` constraint (`x: i32 [0..N]`),
/// so a bounded value's exact arithmetic can be proven in-range instead of using
/// the full type width. Inclusive bounds (a sound over-approximation either way).
/// `None` when the type has no literal range constraint. Looks through reference
/// shells.
/// `range-constraints-require-exact-domain` (Zach, 2026-07-13: "this is just a
/// compile error"): a declared
/// RANGE constraint combined with a non-Exact arithmetic domain is
/// ill-formed. The range is only enforced under Exact stores, so
/// `u8 [0..=4] in Wrapping` accepted `self.i = 100` silently -- the
/// declaration lied to every reader. Rejecting at the declaration keeps the
/// two features composable-by-omission: keep the range (Exact proof
/// obligations enforce it) or keep the domain (defined overflow, full type
/// range), never both.
pub(crate) fn check_range_under_non_exact_domain(
    program: &TypedTrees,
    handle: typed_trees::types::TypeReferenceHandle,
    owner: crate::value_custody::type_references::TypeReferenceOwner<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use numerics::arithmetic::ArithmeticDomain;
    // Walk Reference wrappers AND nested Constrained spellings: `u8 [0..=4]
    // in Wrapping` may parse as Constrained(Constrained(u8, [Range]),
    // [Domain]), where the shallow per-node accessors see only one
    // constraint set each.
    fn has_range_constraint(
        program: &TypedTrees,
        handle: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Reference { referee, .. } => has_range_constraint(program, *referee),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| matches!(constraint, TypeConstraintNode::Range { .. }))
                    || has_range_constraint(program, *base_type)
            }
            _ => false,
        }
    }
    if !has_range_constraint(program, handle) {
        return;
    }
    let domain = program.type_reference_table.arithmetic_domain(handle);
    if domain == ArithmeticDomain::Exact {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "{owner} declares a range constraint together with the `{domain:?}` domain: ranges are only enforced under Exact arithmetic, so the combination is ill-formed (a store outside the range would be silently accepted). Keep the range and drop the domain, or keep the domain and drop the range"
    )));
}

pub(crate) fn range_constraint_interval(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<Interval> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            range_constraint_interval(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .filter_map(|constraint| match constraint {
                TypeConstraintNode::Range {
                    minimum,
                    maximum,
                    end_inclusive,
                } => closed_range_interval(program, *minimum, *maximum, *end_inclusive).or_else(
                    || {
                        Some(Interval {
                            low: Some(
                                crate::closed_integer_range_bound(program, *minimum)?.to_i64()?,
                            ),
                            high: Some(
                                crate::closed_integer_range_maximum(
                                    program,
                                    *maximum,
                                    *end_inclusive,
                                )
                                .and_then(|value| value.to_i64())
                                .or_else(|| {
                                    dependent_maximum_substituted(program, *maximum, *end_inclusive)
                                })?,
                            ),
                        })
                    },
                ),
                // A DECLARED domain's predicates bound the place exactly as a
                // bracketed range does: membership requires every predicate,
                // and a place declared `in D` establishes D at every write. The
                // derivation is shared with the index prover so a bound moved
                // off the revoked suffix reaches both readers.
                TypeConstraintNode::Domain(domain) => {
                    super::domain_intervals::declared_domain_predicate_bounds(program, domain).map(
                        |(minimum, maximum)| Interval {
                            low: minimum.to_i64(),
                            high: maximum.to_i64(),
                        },
                    )
                }
                _ => None,
            })
            .chain(range_constraint_interval(program, *base_type))
            .reduce(Interval::intersect),
        _ => None,
    }
}

fn closed_range_interval(
    program: &TypedTrees,
    minimum: ExpressionHandle,
    maximum: ExpressionHandle,
    end_inclusive: bool,
) -> Option<Interval> {
    let minimum = crate::closed_integer_range_bound(program, minimum)?;
    let maximum = crate::closed_integer_range_maximum(program, maximum, end_inclusive)?;
    // A proof-integer predecessor can fall below i64::MIN. Preserve bottom
    // before projecting into bounded storage: None also means no constraint
    // to several store-checking callers and must not erase an empty range.
    if minimum > maximum {
        return Some(Interval {
            low: Some(1),
            high: Some(0),
        });
    }
    Some(Interval {
        low: Some(minimum.to_i64()?),
        high: Some(maximum.to_i64()?),
    })
}

/// R1a dependent maximum in interval position: `[0..=self.count]` reads as
/// the named field's own enforced literal HIGH plus the offset -- sound
/// because the field's range is store-enforced at every write, so the
/// dependent bound can never exceed it. The field is resolved by NAME
/// across all data definitions and must be UNIQUE (or agree everywhere);
/// an ambiguous name with disagreeing ranges bails rather than guesses
/// (this helper has no machine context; the declaration gate has already
/// verified the binding machine's own field is ranged).
fn dependent_maximum_substituted(
    program: &TypedTrees,
    maximum: typed_trees::expression::ExpressionHandle,
    end_inclusive: bool,
) -> Option<i64> {
    let symbolic = typed_trees::dependent_ranges::symbolic_range_maximum(
        &program.expression_table,
        maximum,
        end_inclusive,
    )?;
    let mut resolved: Option<i64> = None;
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                continue;
            };
            if field.name.as_str() != symbolic.field.as_str() || !field.type_reference.is_valid() {
                continue;
            }
            let high = range_constraint_interval(program, field.type_reference)
                .and_then(|interval| interval.high)?;
            match resolved {
                None => resolved = Some(high),
                Some(existing) if existing == high => {}
                Some(_) => return None,
            }
        }
    }
    resolved?.checked_add(symbolic.offset)
}

/// Containment of a cleanly-analyzed store interval in the target's declared
/// Exact `[a..=b]` range. Both ends must be proven: later reads trust this
/// range as an invariant, so an unknown store cannot establish it. Non-Exact
/// shells and unranged targets remain deliberately permissive.
pub(crate) fn check_range_containment(
    program: &TypedTrees,
    target_type: TypeReferenceHandle,
    interval: Interval,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(declared) = enforced_declared_range_interval(program, target_type) else {
        return;
    };
    let contained_low = match (interval.low, declared.low) {
        (Some(low), Some(declared_low)) => low >= declared_low,
        _ => false,
    };
    let contained_high = match (interval.high, declared.high) {
        (Some(high), Some(declared_high)) => high <= declared_high,
        _ => false,
    };
    let empty = declared
        .low
        .zip(declared.high)
        .is_some_and(|(low, high)| low > high);
    if empty || !contained_low || !contained_high {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} stores a value not provably within its declared range: the range is a \
             store-enforced invariant every read trusts (indexes, exact arithmetic), so the \
             stored value must be proven to honor it. Narrow the value with a dominating \
             guard, a dependent bound, or a requires coupling -- or widen/remove the range",
        )));
    }
}

/// The declared literal `[a..=b]` of a type reference, ONLY under all-Exact
/// Constrained shells (non-Exact ranges are deliberately permissive).
pub(crate) fn enforced_declared_range_interval(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<Interval> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            enforced_declared_range_interval(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = program.type_reference_table.constraints(*constraints);
            if constraints.iter().any(|constraint| {
                matches!(
                    constraint,
                    TypeConstraintNode::ArithmeticDomain(domain)
                        if *domain != ArithmeticDomain::Exact
                )
            }) {
                return None;
            }
            // Every shell contributes conjunctively. Taking only the first
            // range could hide a later empty interval and authorize a store.
            constraints
                .iter()
                .filter_map(|constraint| match constraint {
                    TypeConstraintNode::Range {
                        minimum,
                        maximum,
                        end_inclusive,
                    } => closed_range_interval(program, *minimum, *maximum, *end_inclusive),
                    _ => None,
                })
                .chain(enforced_declared_range_interval(program, *base_type))
                .reduce(Interval::intersect)
        }
        _ => None,
    }
}
