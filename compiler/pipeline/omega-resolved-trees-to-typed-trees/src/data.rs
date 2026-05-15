use crate::program::Lowerer;
use crate::type_reference::lower_type_reference;
use omega_core::diagnostics::Diagnostic;
use omega_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_data_definition(
    lowerer: &mut Lowerer,
    data_definition: &resolved::data::DataDefinition,
) -> Result<typed::data::DataDefinition, Diagnostic> {
    let members = lowerer
        .source_program
        .data_members(data_definition.members)
        .iter()
        .map(|member| lower_data_member(lowerer, member))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(typed::data::DataDefinition {
        symbol: data_definition.symbol,
        name: crate::name::lower_name(&data_definition.name),
        type_parameters: lowerer
            .source_program
            .data_type_parameters(data_definition.type_parameters)
            .iter()
            .map(|parameter| typed::data::TypeParameter {
                symbol: parameter.symbol,
                name: crate::name::lower_name(&parameter.name),
            })
            .collect(),
        members,
    })
}

fn lower_data_member(
    lowerer: &mut Lowerer,
    member: &resolved::data::DataMember,
) -> Result<typed::data::DataMember, Diagnostic> {
    match member {
        resolved::data::DataMember::Field(field) => {
            Ok(typed::data::DataMember::Field(typed::data::DataField {
                symbol: field.symbol,
                name: crate::name::lower_name(&field.name),
                type_reference: lower_type_reference(lowerer, &field.type_reference)?,
            }))
        }
        resolved::data::DataMember::Variant(variant) => {
            Ok(typed::data::DataMember::Variant(typed::data::DataVariant {
                symbol: variant.symbol,
                name: crate::name::lower_name(&variant.name),
            }))
        }
    }
}
