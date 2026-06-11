use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_handle;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::wire::{
    WireField, WireMember, WireReserved, WireSchema, WireVersion,
};
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_wire_schema(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    wire_data: &syntax::item::WireDataDefinition,
) -> Result<WireSchema, Diagnostic> {
    let members = lower_wire_members(lowerer, syntax_trees, wire_data.members)?;

    Ok(WireSchema {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&wire_data.name),
        encoding: wire_data
            .encoding
            .as_ref()
            .map(|encoding| crate::name::lower_name(encoding)),
        members,
    })
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
