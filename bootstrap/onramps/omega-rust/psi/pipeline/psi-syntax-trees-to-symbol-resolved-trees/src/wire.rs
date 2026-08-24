use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_handle;
use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::wire::{
    WireField, WireMember, WireReserved, WireSchema, WireVersion,
};
use psi_symbols::SymbolHandle;
use psi_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_wire_schema(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    wire_data: &syntax::item::WireDataDefinition,
) -> Result<WireSchema, Diagnostic> {
    let members = lower_wire_members(lowerer, syntax_trees, wire_data.members)?;

    Ok(WireSchema {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&wire_data.name),
        is_public: wire_data.is_public,
        encoding: wire_data
            .encoding
            .as_ref()
            .map(|encoding| crate::name::lower_name(encoding)),
        members,
    })
}

/// Chapter 20: field numbers are INERT schema facts, so a numbered `data`
/// is ALSO a plain program type -- instantiable, member-addressable, ZII
/// like any data. Build its regular DataDefinition from the schema's
/// CURRENT-era fields (Reserved entries and Version blocks are wire
/// HISTORY, not fields), sharing the already-lowered type references. The
/// corpus's Message/Sample twin pattern was forced by this registration's
/// absence, not chosen.
pub(crate) fn data_definition_from_wire_schema(
    lowerer: &mut Lowerer,
    schema: &WireSchema,
) -> psi_symbol_resolved_trees::data::DataDefinition {
    use psi_symbol_resolved_trees::data::{
        DataDefinition, DataDefinitionStorage, DataField, DataMember, DataProperties,
    };
    let fields: Vec<DataField> = lowerer
        .symbol_resolved_trees
        .wire_members(schema.members)
        .iter()
        .filter_map(|member| match member {
            WireMember::Field(field) => Some(DataField {
                identity: Some(field.number),
                symbol: SymbolHandle::invalid(),
                name: field.name.clone(),
                relevance: field.relevance,
                type_reference: field.type_reference.clone(),
            }),
            _ => None,
        })
        .collect();
    let mut members = psi_arena::HandleSpan::empty();
    for field in fields {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .data_members
            .append_to_span(&mut members, DataMember::Field(field));
    }
    DataDefinition {
        symbol: SymbolHandle::invalid(),
        name: schema.name.clone(),
        is_public: schema.is_public,
        storage: DataDefinitionStorage {
            supply_mode: psi_language_semantics::DataSupplyMode::CheckedShape,
            lifetime_parameters: Vec::new(),
            type_parameters: psi_arena::HandleSpan::empty(),
            quotient: None,
            where_facts: psi_arena::HandleSpan::empty(),
            zero_gated: false,
            retired_identities: lowerer
                .symbol_resolved_trees
                .wire_members(schema.members)
                .iter()
                .filter_map(|member| match member {
                    WireMember::Reserved(retired) => Some(retired.number),
                    _ => None,
                })
                .collect(),
            properties: DataProperties::default(),
            members,
        },
    }
}

fn lower_wire_members(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    members: HandleSpan<syntax::item::WireDataMember>,
) -> Result<HandleSpan<WireMember>, Diagnostic> {
    // Version member lists are lowered depth-first so each list lands as a
    // contiguous span in the shared wire member arena.
    let mut lowered = Vec::new();

    for member in syntax_trees.items.wire_data_members(members) {
        lowered.push(match member {
            syntax::item::WireDataMember::Field(field) => WireMember::Field(WireField {
                number: field.number,
                name: crate::name::lower_name(&field.name),
                relevance: field.relevance,
                type_reference: lower_type_reference_handle(
                    lowerer,
                    syntax_trees,
                    field.type_reference,
                )?,
            }),
            syntax::item::WireDataMember::Reserved(reserved) => {
                WireMember::Reserved(WireReserved {
                    number: reserved.number,
                })
            }
            syntax::item::WireDataMember::Version(version) => {
                let members = lower_wire_members(lowerer, syntax_trees, version.members)?;
                WireMember::Version(WireVersion {
                    name: crate::name::lower_name(&version.name),
                    members,
                })
            }
        });
    }

    Ok(lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .wire_members
        .insert_many(lowered))
}
