//! Retain current predicates from independently live facts, without repeating
//! domain lowering or growing all-pairs structural equality scans.
//!
//! Templates live for this invocation only, under one immutable program and
//! operator catalog. Membership identity is checked before lookup. Grouping
//! uses necessary observation identities only; complete predicate equality
//! still decides duplicates, and original first-appearance order is restored.

use super::{BoundObservation, BoundPredicate};
use facts::{FactContextHandle, FactPayload, FactPlan, ProgramPoint};
use language_semantics::SemanticDomainId;
use std::cmp::Ordering;
use symbols::SymbolHandle;
use validation::CanonicalPlace;

pub(crate) fn retained(
    program: &dyn validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    point: ProgramPoint,
) -> Vec<BoundPredicate> {
    let mut rows = Vec::new();
    let mut domains = Vec::new();
    let availability =
        super::availability::CasePremiseAvailability::new(program, semantic, contexts, point);
    for handle in semantic.unique_context_facts(contexts.iter().copied()) {
        let fact = semantic.facts.get(handle);
        if let Some(predicates) = domain_predicates(
            program,
            operators,
            semantic,
            &availability,
            fact,
            &mut domains,
        ) {
            rows.extend(predicates);
            continue;
        }
        let FactPayload::BooleanPredicate { predicate } = fact.payload else {
            continue;
        };
        if !semantic.boolean_predicates.is_valid(predicate) {
            continue;
        }
        let predicate = semantic.boolean_predicates.get(predicate);
        if predicate.role != facts::BooleanPredicateRole::CurrentObservation {
            continue;
        }
        let Some(bindings) = semantic.boolean_predicate_bindings.span(predicate.bindings) else {
            continue;
        };
        let Some(bindings) = bindings
            .iter()
            .map(|binding| {
                if !semantic.places.is_valid(binding.place) {
                    return None;
                }
                Some(BoundObservation {
                    operand: binding.operand.clone(),
                    type_reference: binding.type_reference,
                    place: crate::flow::canonical_place_from_semantic_place(
                        program,
                        semantic,
                        semantic.places.get(binding.place),
                    )?,
                })
            })
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        rows.push(BoundPredicate {
            machine: predicate.machine,
            entry_state: predicate.entry_state,
            expression: predicate.expression.clone(),
            bindings,
        });
    }
    unique_predicates(rows)
}

struct DomainTemplate {
    symbol: SymbolHandle,
    identity: SemanticDomainId,
    predicates: Option<Vec<BoundPredicate>>,
}

/// Live membership supplies predicates only on its own exact subject. The
/// public goal/veto binder remains independent of this membership consumer.
fn domain_predicates(
    program: &dyn validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    semantic: &FactPlan,
    availability: &super::availability::CasePremiseAvailability<'_>,
    fact: &facts::Fact,
    domains: &mut Vec<DomainTemplate>,
) -> Option<Vec<BoundPredicate>> {
    let (_, _, symbol, identity) = fact.payload.domain_membership()?;
    let domain = program.domain_definition(symbol)?;
    if domain.semantic_id != identity {
        return None;
    }
    let subject = crate::flow::canonical_place_from_semantic_place(
        program,
        semantic,
        semantic.places.get(fact.place.as_place()?),
    )?;
    // Only declaration lowering is reusable. Availability belongs to this
    // exact subject and consumer scope, never to the shared domain template.
    if !availability.allows(&subject) {
        return None;
    }
    let position = if let Some(position) = domains
        .iter()
        .position(|row| row.symbol == symbol && row.identity == identity)
    {
        position
    } else {
        let position = domains.len();
        domains.push(DomainTemplate {
            symbol,
            identity,
            // Existing lowering retains its telescope, selected operators,
            // per-proof budget and generic/index/alias refusal behavior.
            predicates: super::domain_predicates_for_subject(
                program,
                operators,
                domain,
                &CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(domain.subject_symbol),
                    segments: Vec::new(),
                },
            ),
        });
        position
    };
    Some(
        domains[position]
            .predicates
            .as_ref()?
            .iter()
            .map(|template| {
                let mut predicate = template.clone();
                for binding in &mut predicate.bindings {
                    // The binder above already checked that every template
                    // root is the domain's formal. Change only its place prefix;
                    // operand, type, owner coordinates and formula stay exact.
                    let mut place = subject.clone();
                    place.extend_segments(&binding.place.segments);
                    binding.place = place;
                }
                predicate
            })
            .collect(),
    )
}

fn binding_order(left: &BoundPredicate, right: &BoundPredicate) -> Ordering {
    let handle_key = |handle: SymbolHandle| (handle.arena_index(), handle.generation());
    handle_key(left.machine)
        .cmp(&handle_key(right.machine))
        .then_with(|| handle_key(left.entry_state).cmp(&handle_key(right.entry_state)))
        .then_with(|| left.bindings.len().cmp(&right.bindings.len()))
        .then_with(|| {
            left.bindings
                .iter()
                .zip(&right.bindings)
                .map(|(left, right)| left.place.compare_identity(&right.place))
                .find(|order| !order.is_eq())
                .unwrap_or(Ordering::Equal)
        })
}

fn unique_predicates(rows: Vec<BoundPredicate>) -> Vec<BoundPredicate> {
    let mut rows: Vec<_> = rows.into_iter().enumerate().collect();
    rows.sort_by(|(_, left), (_, right)| binding_order(left, right));
    let mut retained: Vec<(usize, BoundPredicate)> = Vec::new();
    let mut group_start = 0;
    for (order, row) in rows {
        if retained
            .last()
            .is_none_or(|(_, previous)| !binding_order(previous, &row).is_eq())
        {
            group_start = retained.len();
        }
        // A bucket is not equality. Keep distinct formulas, operand/type
        // bindings and nonadjacent A,B,A cases with the original Eq verdict.
        if !retained[group_start..]
            .iter()
            .any(|(_, previous)| *previous == row)
        {
            retained.push((order, row));
        }
    }
    retained.sort_by_key(|(order, _)| *order);
    retained.into_iter().map(|(_, row)| row).collect()
}

#[cfg(test)]
mod tests {
    use super::{BoundObservation, BoundPredicate, CanonicalPlace, unique_predicates};
    use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};
    use facts::PlaceRoot;
    use symbols::SymbolHandle;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

    #[test]
    fn predicate_buckets_preserve_complete_equality_and_first_appearance() {
        let first = BoundPredicate {
            machine: SymbolHandle::from_parts(10, 1),
            entry_state: SymbolHandle::from_parts(11, 1),
            expression: CheckedBooleanExpression::Constant(true),
            bindings: vec![BoundObservation {
                operand: CheckedScalarExpression::Parameter {
                    position: 0,
                    primitive_type: PrimitiveType::U64,
                },
                place: CanonicalPlace {
                    root: PlaceRoot::Symbol(SymbolHandle::from_parts(2, 1)),
                    segments: Vec::new(),
                },
                type_reference: TypeReferenceHandle::from_parts(3, 1),
            }],
        };
        let mut other_formula = first.clone();
        other_formula.expression = CheckedBooleanExpression::Constant(false);
        let mut earlier_key = first.clone();
        earlier_key.bindings[0].place.root = PlaceRoot::Symbol(SymbolHandle::from_parts(1, 1));
        let mut other_operand = first.clone();
        other_operand.bindings[0].operand = CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::U64,
        };
        let mut other_type = first.clone();
        other_type.bindings[0].type_reference = TypeReferenceHandle::from_parts(3, 2);
        let mut other_generation = first.clone();
        other_generation.bindings[0].place.root = PlaceRoot::Symbol(SymbolHandle::from_parts(2, 2));
        let mut no_bindings = first.clone();
        no_bindings.bindings.clear();
        let rows = vec![
            first.clone(),
            other_formula.clone(),
            earlier_key,
            first,
            other_operand,
            other_type,
            other_generation,
            no_bindings,
            other_formula,
        ];
        let mut expected = Vec::new();
        for row in &rows {
            if !expected.contains(row) {
                expected.push(row.clone());
            }
        }
        assert_eq!(unique_predicates(rows), expected);
    }
}
