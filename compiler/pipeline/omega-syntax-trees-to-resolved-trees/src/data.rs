use crate::program::Lowerer;
use crate::type_reference::lower_type_reference_handle;
use omega_core::arena::{Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_resolved_trees::data::{
    DataDefinition, DataDefinitionStorage, DataField, DataMember, DataVariant, TypeParameter,
};
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_data_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    data_definition: &syntax::item::DataDefinition,
) -> Result<DataDefinition, Diagnostic> {
    let type_parameters =
        lower_type_parameters(lowerer, syntax_trees, data_definition.type_parameters);
    let members = lower_data_members(lowerer, syntax_trees, data_definition.members)?;

    Ok(DataDefinition {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&data_definition.name),
        storage: DataDefinitionStorage {
            type_parameters,
            members,
        },
    })
}

fn lower_type_parameters(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    type_parameters: HandleSpan<syntax::item::TypeParameter>,
) -> HandleSpan<TypeParameter> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for parameter in syntax_trees.items.type_parameters(type_parameters) {
        let parameter = lowerer
            .program
            .tables
            .declarations
            .data_type_parameters
            .append(TypeParameter {
                symbol: SymbolHandle::invalid(),
                name: crate::name::lower_name(&parameter.name),
            });
        if count == 0 {
            start = parameter;
        }
        count = count
            .checked_add(1)
            .expect("data type parameter span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}

fn lower_data_members(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    members: HandleSpan<syntax::item::DataMember>,
) -> Result<HandleSpan<DataMember>, Diagnostic> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for member in syntax_trees.items.data_members(members) {
        let member = lower_data_member(lowerer, syntax_trees, member)?;
        let member = lowerer
            .program
            .tables
            .declarations
            .data_members
            .append(member);
        if count == 0 {
            start = member;
        }
        count = count
            .checked_add(1)
            .expect("data member span count overflow");
    }

    if count == 0 {
        Ok(HandleSpan::empty())
    } else {
        Ok(HandleSpan::from_parts(start, count))
    }
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
            type_reference: lower_type_reference_handle(
                lowerer,
                syntax_trees,
                field.type_reference,
            )?,
        })),
        syntax::item::DataMember::Variant(variant) => Ok(DataMember::Variant(DataVariant {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(&variant.name),
        })),
    }
}
