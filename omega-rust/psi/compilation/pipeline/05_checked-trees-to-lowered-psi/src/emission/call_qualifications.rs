//! Normal-call memberships have one publication inventory. Complete numeric
//! field invariants belong to their exact receiving declaration; additional
//! nominal memberships require a catalog entry and an occurrence publication.

use checked_trees::CheckedTrees;
use symbols::SymbolHandle;

use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::lowering_error::LoweringError;

pub(crate) fn for_call<'a>(
    checked: &'a CheckedTrees,
    machine: SymbolHandle,
    coordinate: SourceCallCoordinate,
    mut has_home: impl FnMut(facts::PlaceRoot) -> bool + 'a,
) -> impl Iterator<Item = &'a facts::Fact> {
    let point = facts::ProgramPoint::CallEnsures {
        machine_symbol: machine,
        state_symbol: coordinate.state,
        statement_index: coordinate.statement_index,
        call_ordinal: coordinate.call_ordinal,
    };
    checked
        .facts
        .semantic
        .facts
        .iter()
        .filter_map(move |(_, fact)| {
            (fact.origin == facts::FactOrigin::CallEnsures
                && fact.point == point
                && fact
                    .place
                    .as_place()
                    .is_some_and(|place| has_home(checked.facts.semantic.places.get(place).root))
                && fact.payload.domain_membership().is_some())
            .then_some(fact)
        })
}

/// Independently reconstruct the complete selected domain, including every
/// predicate conjunct, before discharging its publication in declared storage.
/// Equal scalar widths or an unrelated restriction never establish this test.
pub(crate) fn declaration_represents(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    fact: &facts::Fact,
) -> Result<bool, LoweringError> {
    let facts::ProgramPoint::CallEnsures {
        state_symbol,
        statement_index,
        ..
    } = fact.point
    else {
        return Ok(false);
    };
    let Some((_, _, domain_symbol, semantic_domain)) = fact.payload.domain_membership() else {
        return Ok(false);
    };
    let Some(place) = fact.place.as_place() else {
        return Ok(false);
    };
    let place = checked.facts.semantic.places.get(place);
    let segments = checked
        .facts
        .semantic
        .place_segments
        .span_or_empty(place.segments);
    let Some(reference) = validation::canonical_place_type_reference(
        &checked.typed,
        state_symbol,
        statement_index,
        &validation::CanonicalPlace {
            root: place.root,
            segments: segments.to_vec(),
        },
    ) else {
        return Ok(false);
    };
    let Some(primitive) = checked.primitive_type_reference(reference) else {
        return Ok(false);
    };
    let Some(restriction) =
        crate::expression_preparation::qualifications::retained_scalar_restriction(
            checked, reference,
        )
    else {
        return Ok(false);
    };
    let constraints =
        validation::field_domains::declared_domain_constraint_records(&checked.typed, reference);
    let mut selected = constraints.into_iter().filter(|constraint| {
        constraint.symbol == domain_symbol && constraint.semantic_id == semantic_domain
    });
    let Some(constraint) = selected.next() else {
        return Ok(false);
    };
    if selected.next().is_some() {
        return Ok(false);
    }
    let Some(definition) = checked
        .typed
        .domain_definition_for_instance(domain_symbol, semantic_domain)
    else {
        return Ok(false);
    };
    if !validation::field_domains::domain_predicate_theory_is_complete(&checked.typed, definition)
        || !definition.establishment_routes.is_empty()
        || !typed_trees::domain::required_subject_memberships(&checked.typed, definition)
            .is_some_and(|required| required.is_empty())
        || validation::field_domains::domain_byte_predicate(&checked.typed, domain_symbol).is_some()
        || checked
            .facts
            .qualifications
            .content
            .for_semantic_domain(semantic_domain)
            .is_some()
    {
        return Ok(false);
    }
    let Some(predicate) = validation::scalar_domain_value_predicate(
        &checked.typed,
        constraint,
        checked_trees::CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: primitive,
        },
    ) else {
        return Ok(false);
    };
    let required = crate::structural_types::lower_field_type(
        identities,
        checked,
        &checked_trees::CheckedUnitStructuralFieldType::RefinedScalar {
            primitive,
            predicate,
        },
        &[],
    )?;
    let declared =
        crate::structural_types::lower_field_type(identities, checked, restriction, &[])?;
    Ok(terminal_semantics::scalar_field_entails(
        &declared, &required,
    ))
}
