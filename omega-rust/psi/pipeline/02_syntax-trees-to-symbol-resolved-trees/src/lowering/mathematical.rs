//! Mathematical `let`/`boundary let` declarations: binders, the ordinary
//! telescope, the dependent result type, and the transparent or
//! named-assumption body.
//!
//! These declarations live on `SyntaxTreeRoots::mathematical_definitions`,
//! outside the `Item` grammar, so they lower through their own entry point
//! after `lower_items`. The binder kinds stay the authored `Type`/`Const`/
//! `Value` carriers — a `u: core::Level` binder reads as a universe level only
//! once its carrier resolves, which is the typed-tree elaboration leg's
//! classification to make, not this stage's.

use crate::lowering::data::lower_type_parameters;
use crate::lowering::expression::lower_expression_into_table;
use crate::lowering::name::lower_name;
use crate::lowering::type_reference::lower_type_reference_handle;
use crate::resolution::lowerer::Lowerer;
use diagnostics::Diagnostic;
use symbol_resolved_trees::mathematical::{
    MathematicalBody, MathematicalDefinition, MathematicalParameter, MathematicalType,
    MathematicalTypeHandle,
};
use symbols::SymbolHandle;
use syntax_trees::{self as syntax, SyntaxTrees};

/// Translate every parsed mathematical declaration into the resolved carrier,
/// in authored order, after the ordinary root items. Symbols stay invalid
/// here; `symbols::assign` allocates them like every other declaration kind.
pub(crate) fn lower_mathematical_definitions(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
) -> Result<(), Vec<Diagnostic>> {
    for handle in syntax.root_mathematical_definition_handles() {
        let definition = lower_mathematical_definition(
            lowerer,
            syntax,
            syntax.root_mathematical_definition(*handle),
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        lowerer
            .symbol_resolved_trees
            .roots
            .mathematical_definitions
            .push(definition);
    }
    Ok(())
}

fn lower_mathematical_definition(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    definition: &syntax::item::MathematicalDefinition,
) -> Result<MathematicalDefinition, Diagnostic> {
    let binders = lower_type_parameters(lowerer, syntax, definition.type_parameters)?;

    let mut parameters = Vec::new();
    for handle in syntax.items.mathematical_parameters(definition.parameters) {
        let parameter = syntax.items.mathematical_parameter(*handle);
        parameters.push(MathematicalParameter {
            symbol: SymbolHandle::invalid(),
            name: lower_name(&parameter.name),
            relevance: parameter.relevance,
            ty: lower_mathematical_type(lowerer, syntax, parameter.ty)?,
        });
    }
    let parameters = lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .mathematical_parameters
        .insert_many(parameters);

    let result = lower_mathematical_type(lowerer, syntax, definition.result)?;

    let body = match definition.body {
        syntax::item::MathematicalDefinitionBody::Definition(term) => {
            MathematicalBody::Definition(lower_expression_into_table(lowerer, syntax, term)?)
        }
        syntax::item::MathematicalDefinitionBody::Assumption => MathematicalBody::Assumption,
    };

    Ok(MathematicalDefinition {
        symbol: SymbolHandle::invalid(),
        name: lower_name(&definition.name),
        is_public: definition.is_public,
        binders,
        parameters,
        result,
        body,
    })
}

/// Lower the declaration-local mathematical type grammar. Ordinary references
/// reuse the shared type lowering; arrows and value-argument applications are
/// the proof-surface-only forms.
fn lower_mathematical_type(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    handle: syntax::item::MathematicalTypeHandle,
) -> Result<MathematicalTypeHandle, Diagnostic> {
    let node = match syntax.items.mathematical_type(handle) {
        syntax::item::MathematicalTypeNode::Ordinary(type_reference) => MathematicalType::Ordinary(
            lower_type_reference_handle(lowerer, syntax, *type_reference)?,
        ),
        syntax::item::MathematicalTypeNode::Arrow {
            binder,
            domain,
            codomain,
        } => MathematicalType::Arrow {
            binder: binder.as_ref().map(lower_name),
            domain: lower_mathematical_type(lowerer, syntax, *domain)?,
            codomain: lower_mathematical_type(lowerer, syntax, *codomain)?,
        },
        syntax::item::MathematicalTypeNode::Application { callee, arguments } => {
            let arguments = syntax
                .expressions
                .expression_handles(*arguments)
                .iter()
                .map(|argument| lower_expression_into_table(lowerer, syntax, *argument))
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            MathematicalType::Application {
                callee: lower_mathematical_type(lowerer, syntax, *callee)?,
                arguments: lowerer
                    .symbol_resolved_trees
                    .tables
                    .bodies
                    .expressions
                    .insert_expression_handles(arguments),
            }
        }
    };
    Ok(lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .mathematical_types
        .insert(node))
}
