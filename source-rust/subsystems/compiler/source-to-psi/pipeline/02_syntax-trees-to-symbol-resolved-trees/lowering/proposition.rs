/*
    Proposition definitions: binders, parameters, and primitive, witness, or
    transparent bodies.
*/

use crate::resolution::lowering::expression::lower_expression_into_table;
use crate::resolution::lowering::lowerer::Lowerer;
use crate::resolution::lowering::state::lower_state_parameters;
use crate::resolution::lowering::type_reference::lower_type_reference_handle;
use crate::source_resolved::data::DataProperties;
use crate::source_resolved::proposition::{
    PropositionBinder, PropositionBinderKind, PropositionBody, PropositionDefinition,
};
use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use syntax::item::TypeParameter;
use syntax::item::TypeParameterKind;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_proposition_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    proposition: &syntax::item::PropositionDefinition,
) -> Result<PropositionDefinition, Diagnostic> {
    let binders = lower_proposition_binders(lowerer, syntax_trees, proposition.type_parameters)?;
    let parameters = lower_state_parameters(lowerer, syntax_trees, proposition.parameters)?;
    let body = match proposition.body {
        syntax::item::PropositionBody::Primitive => PropositionBody::Primitive,
        syntax::item::PropositionBody::Witness { evidence } => PropositionBody::Witness {
            evidence: lower_type_reference_handle(lowerer, syntax_trees, evidence)?,
        },
        syntax::item::PropositionBody::Transparent { proposition } => {
            PropositionBody::Transparent {
                proposition: lower_expression_into_table(lowerer, syntax_trees, proposition)?,
            }
        }
    };

    Ok(PropositionDefinition {
        symbol: SymbolHandle::invalid(),
        name: crate::resolution::lowering::name::lower_name(syntax_trees, &proposition.name),
        is_public: proposition.is_public,
        binders,
        parameters,
        transparent_formula_source_span: proposition.transparent_formula_source_span,
        body,
    })
}

fn lower_proposition_binders(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    binders: HandleSpan<TypeParameter>,
) -> Result<HandleSpan<PropositionBinder>, Diagnostic> {
    let mut lowered = Vec::new();
    for binder in syntax_trees.items.type_parameters(binders) {
        let kind = match binder.kind {
            TypeParameterKind::Type => PropositionBinderKind::Type,
            TypeParameterKind::Const { type_reference } => PropositionBinderKind::Const {
                type_reference: lower_type_reference_handle(lowerer, syntax_trees, type_reference)?,
            },
            TypeParameterKind::Value { .. } => {
                return Err(Diagnostic::error(format!(
                    "proposition declaration binder `{}` cannot be a runtime-capable value binder; value binders are legal only on machine signature generics",
                    binder.name.as_str(syntax_trees)
                )));
            }
            TypeParameterKind::Machine { .. } => PropositionBinderKind::Machine,
            TypeParameterKind::Implementation => {
                return Err(Diagnostic::error(format!(
                    "proposition declaration binder `{}` cannot select an implementation; implementation slots belong to trait telescopes",
                    binder.name.as_str(syntax_trees)
                )));
            }
            TypeParameterKind::Proposition { .. } => {
                return Err(Diagnostic::error(format!(
                    "proposition declaration binder `{}` cannot itself be a generic proposition parameter; generic proposition signatures belong to trait abstraction surfaces",
                    binder.name.as_str(syntax_trees)
                )));
            }
        };
        lowered.push(PropositionBinder {
            symbol: SymbolHandle::invalid(),
            name: crate::resolution::lowering::name::lower_name(syntax_trees, &binder.name),
            kind,
            bounds: DataProperties {
                carry: binder.bounds.carry,
                multiplicity: binder.bounds.multiplicity,
            },
        });
    }

    Ok(lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .proposition_binders
        .insert_many(lowered))
}
