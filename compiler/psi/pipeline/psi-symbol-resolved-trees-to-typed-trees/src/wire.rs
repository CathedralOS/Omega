use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_into_table;
use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

pub(crate) fn lower_wire_schema(
    lowerer: &mut Lowerer,
    wire_schema: &resolved::wire::WireSchema,
) -> Result<typed::wire::WireSchema, Diagnostic> {
    let members = lower_wire_members(lowerer, wire_schema.members)?;

    Ok(typed::wire::WireSchema {
        symbol: wire_schema.symbol,
        name: crate::name::lower_name(&wire_schema.name),
        encoding: wire_schema.encoding.as_ref().map(crate::name::lower_name),
        members,
    })
}

fn lower_wire_members(
    lowerer: &mut Lowerer,
    members: HandleSpan<resolved::wire::WireMember>,
) -> Result<HandleSpan<typed::wire::WireMember>, Diagnostic> {
    let mut lowered = Vec::new();

    for member in lowerer.source_trees.wire_members(members) {
        lowered.push(match member {
            resolved::wire::WireMember::Field(field) => {
                typed::wire::WireMember::Field(typed::wire::WireField {
                    number: field.number,
                    name: crate::name::lower_name(&field.name),
                    relevance: field.relevance,
                    type_reference: lower_type_reference_into_table(
                        lowerer,
                        &field.type_reference,
                    )?,
                })
            }
            resolved::wire::WireMember::Reserved(reserved) => {
                typed::wire::WireMember::Reserved(typed::wire::WireReserved {
                    number: reserved.number,
                })
            }
            resolved::wire::WireMember::Version(version) => {
                let members = lower_wire_members(lowerer, version.members)?;
                typed::wire::WireMember::Version(typed::wire::WireVersion {
                    name: crate::name::lower_name(&version.name),
                    members,
                })
            }
        });
    }

    Ok(lowerer.typed_trees.append_wire_members(lowered))
}
