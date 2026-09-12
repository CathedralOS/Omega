use crate::lowerer::Lowerer;
use symbol_resolved_trees::data::{DataDefinition, DataMember, DataProperties};
use symbol_resolved_trees::wire::{WireField, WireMember, WireReserved, WireSchema};

/// Derive the current record codec's view from the ordinary declaration.
/// The current generated record codec cannot establish whole-record domain,
/// declared-property, or lifetime obligations. Those declarations still lower
/// normally; only this consumer-specific view is withheld. Generic codec
/// selection remains a separate consumer requirement.
pub(crate) fn derive_wire_schema(lowerer: &mut Lowerer, definition: &DataDefinition) {
    if !definition.type_parameters.is_empty()
        || !definition.lifetime_parameters.is_empty()
        || definition.properties != DataProperties::default()
        || !definition.where_facts.is_empty()
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
mod tests {
    use crate::lower_syntax_trees;
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees::data::DataMember;
    use symbol_resolved_trees::wire::WireMember;
    use tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn codec_view_shares_ordinary_fields_and_retirements() {
        let tokens = Lexer::new("pub data Message { #7 value: u32; retired #8; }")
            .tokenize()
            .expect("tokens");
        let syntax = parse_syntax_trees(&tokens).expect("syntax");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let definition = &resolved.data_definitions[0];
        let schema = &resolved.wire_schemas[0];
        assert_eq!(definition.name, schema.name);
        assert_eq!(definition.is_public, schema.is_public);
        let [DataMember::Field(field)] = resolved.data_members(definition.members) else {
            panic!("ordinary field");
        };
        let [WireMember::Field(wire), WireMember::Reserved(retired)] =
            resolved.wire_members(schema.members)
        else {
            panic!("derived view");
        };
        assert_eq!(field.identity, Some(wire.number));
        assert_eq!(field.name, wire.name);
        assert_eq!(field.relevance, wire.relevance);
        assert_eq!(field.type_reference, wire.type_reference);
        assert_eq!(definition.retired_identities, [retired.number]);
    }

    #[test]
    fn codec_view_does_not_drop_whole_record_or_ownership_obligations() {
        for declaration in [
            "data Message where value <= 10 { #7 value: u32; }",
            "data Message [copy] { #7 value: u32; }",
            "data Message<'scope> { #7 value: &'scope u32; }",
            "data Message { #7 value: u32; case #9 Ready; }",
        ] {
            let tokens = Lexer::new(declaration).tokenize().expect("tokens");
            let syntax = parse_syntax_trees(&tokens).expect("syntax");
            let resolved = lower_syntax_trees(&syntax).expect("ordinary data resolution");
            assert_eq!(resolved.data_definitions.len(), 1);
            assert!(
                resolved.wire_schemas.is_empty(),
                "unsupported implicit codec: {declaration}"
            );
        }
    }
}
