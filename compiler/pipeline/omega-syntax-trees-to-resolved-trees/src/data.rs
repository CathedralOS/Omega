use crate::program::Lowerer;
use crate::type_reference::lower_type_reference_handle;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_syntax_trees::{self as syntax, SyntaxTrees};
use omega_resolved_trees::data::{DataDefinition, DataField, DataMember, DataVariant, TypeParameter};

pub(crate) fn lower_data_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    data_definition: &syntax::item::DataDefinition,
) -> Result<DataDefinition, Diagnostic> {
    let members = data_definition
        .members
        .iter()
        .map(|member| lower_data_member(lowerer, syntax_trees, member))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DataDefinition {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&data_definition.name),
        type_parameters: data_definition
            .type_parameters
            .iter()
            .map(|parameter| TypeParameter {
                symbol: SymbolHandle::invalid(),
                name: crate::name::lower_name(&parameter.name),
            })
            .collect(),
        members,
    })
}

fn lower_data_member(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    member: &syntax::item::DataMember,
) -> Result<DataMember, Diagnostic> {
    match member {
        syntax::item::DataMember::Field(field) => Ok(DataMember::Field(DataField {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(&field.name),
            type_reference: lower_type_reference_handle(lowerer, syntax_trees, field.type_reference)?,
        })),
        syntax::item::DataMember::Variant(variant) => Ok(DataMember::Variant(DataVariant {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(&variant.name),
        })),
    }
}
