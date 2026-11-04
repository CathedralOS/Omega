//! Declaration queries over typed-owned indexes and invocation-owned field rows.
use typed_trees::data::DataMember;

/// The declaration-table answer for a symbol: absent, exactly one
/// definition at a position, or a duplicate.
#[derive(Clone, Copy)]
pub(crate) enum DataDefinitionLookup {
    Missing,
    Unique(u32),
    Duplicate,
}

/// The data-definition table's answer for `symbol`. The program's owned
/// declaration index keeps every position per symbol across mutations, so
/// the answer — including the negative and duplicate cases — is a single
/// index query rather than a re-scan of the whole declaration slice.
pub(crate) fn memoized_data_definition_lookup(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
) -> DataDefinitionLookup {
    program.with_data_definition_index(|index| match index.positions_for_symbol(symbol) {
        [] => DataDefinitionLookup::Missing,
        [first] => DataDefinitionLookup::Unique(*first),
        [_, ..] => DataDefinitionLookup::Duplicate,
    })
}

/// A data definition by symbol, resolved through the program's owned
/// declaration index — `find`-style callers read the first match; callers
/// that require uniqueness should use [`memoized_data_definition_lookup`]
/// directly.
pub(crate) fn data_definition_by_symbol(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    program.data_definition_by_symbol(symbol)
}

/// The declared type of the one field `symbol` names across every data
/// definition.
pub(crate) fn data_field_type_by_symbol(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_members(data))
        .find_map(|member| match member {
            DataMember::Field(field) if symbol.is_valid() && field.symbol == symbol => {
                Some(field.type_reference)
            }
            _ => None,
        })
}

/// A field may be lent without the parent's standing facts only when none
/// depends on that field. This is declaration identity, not equal spelling:
/// a nested selection still visits its owning field, and predicate calls
/// still visit every argument. Unknown name resolution and whole-owner uses
/// cannot establish independence. The field's own type remains unchanged.
pub fn data_field_is_where_independent(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
    field: &typed_trees::data::DataField,
) -> bool {
    let case_facts = program
        .data_members(definition)
        .iter()
        .find_map(|member| {
            let variant = member.as_variant()?;
            program
                .data_payload_fields(variant)
                .iter()
                .any(|payload| payload.symbol == field.symbol)
                .then(|| program.proof_facts.span_or_empty(variant.where_facts))
        })
        .unwrap_or(&[]);
    program
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .chain(case_facts)
        .all(|fact| {
            !crate::proof_contracts::default_domains::data_predicate_fields(
                program, definition, fact,
            )
            .contains(&field.symbol)
        })
}
