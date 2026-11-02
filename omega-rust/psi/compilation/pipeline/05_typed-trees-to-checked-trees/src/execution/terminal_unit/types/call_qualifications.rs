//! A completed call can establish a domain independently of its storage's
//! declared qualifications. Retain that selected theory on the exact field or
//! referent carrier before catalog pruning. This describes checked evidence;
//! it introduces no membership, entry assumption, or new storage authority.
use super::*;
use crate::execution::terminal_unit::symbol_key;

pub(in crate::execution::terminal_unit) fn retain_call_qualification_domains<'operation>(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    operations: impl Iterator<
        Item = (
            SymbolHandle,
            SymbolHandle,
            &'operation CheckedUnitEffectOperationPlan,
        ),
    >,
    retained: &mut BTreeSet<typed_trees::type_identity::TypeIdentityId>,
) {
    let coordinates = operations
        .filter_map(|(_, state, operation)| {
            let call = operation.call()?;
            Some((
                symbol_key(state),
                call.coordinate.statement_index as usize,
                call.coordinate.call_ordinal as usize,
            ))
        })
        .collect::<BTreeSet<_>>();
    let mut binders = BTreeMap::new();
    for (_, fact) in facts.semantic.facts.iter() {
        let facts::ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        } = fact.point
        else {
            continue;
        };
        if fact.origin != facts::FactOrigin::CallEnsures
            || !coordinates.contains(&(symbol_key(state_symbol), statement_index, call_ordinal))
        {
            continue;
        }
        let Some((_, _, domain_symbol, domain)) = fact.payload.domain_membership() else {
            continue;
        };
        let Some(place) = fact.place.as_place() else {
            continue;
        };
        let place = facts.semantic.places.get(place);
        let selected = validation::canonical_place_type_reference(
            program,
            state_symbol,
            statement_index,
            &validation::CanonicalPlace {
                root: place.root,
                segments: facts
                    .semantic
                    .place_segments
                    .span_or_empty(place.segments)
                    .to_vec(),
            },
        );
        let Some((_, carrier)) = selected.and_then(|selected| {
            validation::result_membership_carrier(program, selected, domain_symbol)
        }) else {
            continue;
        };
        // Scalar declaration restrictions are published by the scalar owner.
        if program.primitive_type_reference(carrier).is_some() {
            continue;
        }
        let parameters = binders
            .entry(symbol_key(machine_symbol))
            .or_insert_with(|| {
                program
                    .machine_by_symbol(machine_symbol)
                    .map(|machine| machine_binders(program, machine))
                    .unwrap_or_default()
            });
        if let Some(identity) =
            shapes.add_selected_domain(domain, domain_symbol, carrier, parameters, true)
        {
            retained.insert(identity);
        }
    }
}
