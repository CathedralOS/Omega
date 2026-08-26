use crate::lowerer::Lowerer;
use crate::state::lower_state_parameters;
use crate::type_reference::lower_type_reference_handle;
use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::data::DataProperties;
use psi_symbol_resolved_trees::proposition::{
    PropositionBinder, PropositionBinderKind, PropositionBody, PropositionDefinition,
};
use psi_symbols::SymbolHandle;
use psi_syntax_trees::{self as syntax, SyntaxTrees};

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
                proposition: crate::expression::lower_expression_into_table(
                    lowerer,
                    syntax_trees,
                    proposition,
                )?,
            }
        }
    };

    Ok(PropositionDefinition {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&proposition.name),
        is_public: proposition.is_public,
        binders,
        parameters,
        body,
    })
}

fn lower_proposition_binders(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    binders: HandleSpan<syntax::item::TypeParameter>,
) -> Result<HandleSpan<PropositionBinder>, Diagnostic> {
    let mut lowered = Vec::new();
    for binder in syntax_trees.items.type_parameters(binders) {
        let kind = match binder.kind {
            syntax::item::TypeParameterKind::Type => PropositionBinderKind::Type,
            syntax::item::TypeParameterKind::Const { type_reference } => {
                PropositionBinderKind::Const {
                    type_reference: lower_type_reference_handle(
                        lowerer,
                        syntax_trees,
                        type_reference,
                    )?,
                }
            }
            syntax::item::TypeParameterKind::Machine { .. } => PropositionBinderKind::Machine,
            syntax::item::TypeParameterKind::Proposition { .. } => {
                return Err(Diagnostic::error(format!(
                    "proposition declaration binder `{}` cannot itself be a generic proposition parameter; generic proposition signatures belong to trait abstraction surfaces",
                    binder.name.as_str()
                )));
            }
        };
        lowered.push(PropositionBinder {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(&binder.name),
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
