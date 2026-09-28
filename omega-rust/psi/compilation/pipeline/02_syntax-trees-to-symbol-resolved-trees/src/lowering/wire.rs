//! Wire schema derivation for lowered data definitions.

use crate::resolution::lowerer::Lowerer;
use symbol_resolved_trees::data::{DataDefinition, DataMember, DataProperties};
use symbol_resolved_trees::wire::{WireField, WireMember, WireReserved, WireSchema};

/// Derive the current record codec's view from the ordinary declaration.
///
/// A codec view exists only for records the generated codec can check against
/// the public wire requirement end to end. The view is withheld for any
/// declaration carrying an obligation the codec cannot establish:
/// whole-record domain (`where` facts, which cover every `zero_gated` and
/// membership-gated carrier), declared properties, lifetime or type
/// parameters, variant members, fields without `#n` identities, bodyless
/// forms (quotient and opaque `boundary data` both lower with no members),
/// and records whose supply mode is not `CheckedShape`. Those declarations
/// still lower normally; only this consumer-specific view is withheld.
/// Generic codec selection remains a separate consumer requirement.
pub(crate) fn derive_wire_schema(lowerer: &mut Lowerer, definition: &DataDefinition) {
    if !definition.type_parameters.is_empty()
        || !definition.lifetime_parameters.is_empty()
        || definition.properties != DataProperties::default()
        || !definition.where_facts.is_empty()
        || definition.supply_mode != language_core::DataSupplyMode::CheckedShape
    {
        return;
    }
    let fields = lowerer
        .symbol_resolved_trees
        .data_members(definition.members);
    if fields.iter().any(|member| match member {
        DataMember::Field(field) => field.identity.is_none(),
        DataMember::Variant(_) => true,
    }) || (fields.is_empty() && definition.retired_identities.is_empty())
    {
        return;
    }
    let mut members = Vec::with_capacity(fields.len() + definition.retired_identities.len());
    for member in fields {
        if let DataMember::Field(field) = member {
            let Some(number) = field.identity else {
                return;
            };
            members.push(WireMember::Field(WireField {
                number,
                name: field.name.clone(),
                relevance: field.relevance,
                type_reference: field.type_reference.clone(),
            }));
        }
    }
    members.extend(
        definition
            .retired_identities
            .iter()
            .map(|number| WireMember::Reserved(WireReserved { number: *number })),
    );
    let members = lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .wire_members
        .insert_many(members);
    lowerer.symbol_resolved_trees.wire_schemas.push(WireSchema {
        symbol: Default::default(),
        name: definition.name.clone(),
        is_public: definition.is_public,
        encoding: None,
        members,
    });
}

#[cfg(test)]
mod tests {}
