//! Replay byte-domain meaning and its concrete carrier from typed selections.
//! A family spans capacities, but neither its first declaration nor a checked
//! plan chooses the meaning of another carrier. Nominal domain custody remains
//! with the existing qualification and establishment-route owners.

use checked_trees::{CheckedTrees, CheckedUnitStructuralDomainPlan};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

use crate::lowering_error::{LoweringError, unsupported};

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedUnitStructuralDomainPlan,
) -> Result<(), LoweringError> {
    let mut meaning = None;
    for definition in checked
        .domain_definitions()
        .iter()
        .filter(|definition| definition.semantic_id == plan.domain)
    {
        let predicate = checked_trees::byte_predicates::domain_byte_predicate(
            &checked.typed,
            definition.symbol,
        );
        if meaning.is_some_and(|previous| previous != predicate) {
            return unsupported("byte domain family has conflicting source predicates");
        }
        meaning = Some(predicate);
    }
    let expected_predicate = meaning.flatten();
    if plan.byte_predicate != expected_predicate {
        return unsupported("byte domain definition differs from checked source");
    }
    let Some(predicate) = expected_predicate else {
        // Bodyless, nominal, indexed and routed domains retain their existing
        // independent evidence paths; this query grants them no byte predicate.
        return Ok(());
    };
    if checked.semantic_domains.name(plan.domain) != Some(plan.identity.as_str())
        || !plan.establishment_routes.is_empty()
    {
        return unsupported("byte domain plan changed its authored family or routes");
    }

    let mut found_carrier = false;
    for (reference, _, constraints) in checked
        .type_reference_table
        .constrained_type_reference_sites()
    {
        let mut selections = checked
            .type_reference_table
            .constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint {
                TypeConstraintNode::Domain(domain) if domain.semantic_id == plan.domain => {
                    Some(domain)
                }
                _ => None,
            });
        let Some(selection) = selections.next() else {
            continue;
        };
        let carrier = unqualified_carrier(checked, reference)?;
        if checked.normalized_type_identity(carrier).as_str() != plan.carrier_type_identity {
            continue;
        }
        if selections.next().is_some() {
            return unsupported("byte domain carrier has ambiguous authored selections");
        }
        let definition = typed_trees::domain::domain_by_symbol(&checked.typed, selection.symbol)
            .ok_or(LoweringError::Unsupported(
                "byte domain carrier lost its selected declaration",
            ))?;
        if definition.semantic_id != plan.domain
            || checked_trees::byte_predicates::domain_byte_predicate(
                &checked.typed,
                definition.symbol,
            ) != Some(predicate)
            || definition.establishment_routes != plan.establishment_routes
        {
            return unsupported("byte domain carrier selected a different source theory");
        }
        // The concrete qualification site is the application evidence for a
        // generic carrier declaration. Its template target need not equal this
        // normalized concrete carrier, and no substitution is guessed here.
        found_carrier = true;
    }
    if !found_carrier {
        // A contract such as `requires bytes in Utf8` can name an otherwise
        // unqualified carrier. Its closed declaration supplies that exact
        // carrier without requiring an unrelated `in Utf8` type site. Generic
        // declarations still need the concrete application evidence above.
        for definition in checked.domain_definitions().iter().filter(|definition| {
            definition.semantic_id == plan.domain
                && checked.domain_type_parameters(definition).is_empty()
        }) {
            let carrier = unqualified_carrier(checked, definition.target_type)?;
            if checked.normalized_type_identity(carrier).as_str() == plan.carrier_type_identity
                && checked_trees::byte_predicates::domain_byte_predicate(
                    &checked.typed,
                    definition.symbol,
                ) == Some(predicate)
                && definition.establishment_routes == plan.establishment_routes
            {
                found_carrier = true;
            }
        }
    }
    if !found_carrier {
        return unsupported("byte domain plan has no exact authored carrier selection");
    }
    Ok(())
}

fn unqualified_carrier(
    checked: &CheckedTrees,
    mut reference: TypeReferenceHandle,
) -> Result<TypeReferenceHandle, LoweringError> {
    let mut visited = Vec::new();
    loop {
        if !checked
            .type_reference_table
            .contains_type_reference(reference)
            || visited.contains(&reference)
        {
            return unsupported("byte domain carrier has an invalid type chain");
        }
        visited.push(reference);
        match checked.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => reference = *referee,
            _ => return Ok(reference),
        }
    }
}
