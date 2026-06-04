use crate::expression::lower_expression_into_table;
use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_handle;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::data::{
    DataDefinition, DataDefinitionStorage, DataField, DataMember, DataVariant, TypeParameter,
    TypeParameterKind,
};
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_data_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    data_definition: &syntax::item::DataDefinition,
) -> Result<DataDefinition, Diagnostic> {
    let type_parameters =
        lower_type_parameters(lowerer, syntax_trees, data_definition.type_parameters)?;
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

pub(crate) fn lower_type_parameters(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    type_parameters: HandleSpan<syntax::item::TypeParameter>,
) -> Result<HandleSpan<TypeParameter>, Diagnostic> {
    let mut lowered = Vec::new();
    for parameter in syntax_trees.items.type_parameters(type_parameters) {
        let kind = match &parameter.kind {
            syntax::item::TypeParameterKind::Type => TypeParameterKind::Type,
            syntax::item::TypeParameterKind::Const { type_reference } => TypeParameterKind::Const {
                type_reference: lower_type_reference_handle(
                    lowerer,
                    syntax_trees,
                    *type_reference,
                )?,
            },
        };
        lowered.push(TypeParameter {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(&parameter.name),
            kind,
        });
    }

    Ok(lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .data_type_parameters
        .insert_many(lowered))
}

fn lower_data_members(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    members: HandleSpan<syntax::item::DataMember>,
) -> Result<HandleSpan<DataMember>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for member in syntax_trees.items.data_members(members) {
        if matches!(member, syntax::item::DataMember::Version(_)) {
            continue;
        }
        let member = lower_data_member(lowerer, syntax_trees, member)?;
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .data_members
            .append_to_span(&mut span, member);
    }

    Ok(span)
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
            initial_value: field
                .initial_value
                .is_valid()
                .then(|| {
                    lower_expression_into_table(
                        syntax_trees,
                        &mut lowerer.symbol_resolved_trees.tables.bodies.expressions,
                        field.initial_value,
                    )
                })
                .transpose()?
                .unwrap_or_else(omega_symbol_resolved_trees::expression::ExpressionHandle::invalid),
        })),
        syntax::item::DataMember::Variant(variant) => Ok(DataMember::Variant(DataVariant {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(&variant.name),
        })),
        syntax::item::DataMember::Version(_) => unreachable!("data versions are metadata"),
    }
}
