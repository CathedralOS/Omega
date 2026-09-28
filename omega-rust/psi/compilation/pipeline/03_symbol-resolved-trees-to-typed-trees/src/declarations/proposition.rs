use crate::expressions::expression::lower_expression_handle_from_table_in_fact_position;
use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_into_table;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

use crate::expressions::proposition::lower_proposition_application;

pub(crate) fn lower_proposition_definition(
    lowerer: &mut Lowerer,
    proposition: &resolved::proposition::PropositionDefinition,
) -> Result<typed::proposition::PropositionDefinition, Diagnostic> {
    let mut typed_proposition = typed::proposition::PropositionDefinition {
        symbol: proposition.symbol,
        name: crate::lowerer::name::lower_name(&proposition.name),
        is_public: proposition.is_public,
        binders: Default::default(),
        parameters: Default::default(),
        transparent_formula_source_span: proposition.transparent_formula_source_span,
        body: typed::proposition::PropositionBody::Primitive,
    };

    for binder in lowerer
        .source_trees
        .tables
        .declarations
        .proposition_binders
        .span_or_empty(proposition.binders)
    {
        let kind = match &binder.kind {
            resolved::proposition::PropositionBinderKind::Type => {
                typed::proposition::PropositionBinderKind::Type
            }
            resolved::proposition::PropositionBinderKind::Const { type_reference } => {
                typed::proposition::PropositionBinderKind::Const {
                    type_reference: lower_type_reference_into_table(lowerer, type_reference)?,
                }
            }
            resolved::proposition::PropositionBinderKind::Machine => {
                typed::proposition::PropositionBinderKind::Machine
            }
        };
        lowerer.typed_trees.push_proposition_binder(
            &mut typed_proposition,
            typed::proposition::PropositionBinder {
                symbol: binder.symbol,
                name: crate::lowerer::name::lower_name(&binder.name),
                kind,
                bounds: typed::data::DataProperties {
                    carry: binder.bounds.carry,
                    multiplicity: binder.bounds.multiplicity,
                },
            },
        );
    }

    for parameter in lowerer
        .source_trees
        .state_parameters(proposition.parameters)
    {
        let parameter = crate::signatures::parameters::lower_state_parameter(lowerer, parameter)?;
        lowerer
            .typed_trees
            .push_proposition_parameter(&mut typed_proposition, parameter);
    }

    typed_proposition.body = match &proposition.body {
        resolved::proposition::PropositionBody::Primitive => {
            typed::proposition::PropositionBody::Primitive
        }
        resolved::proposition::PropositionBody::Witness { evidence } => {
            typed::proposition::PropositionBody::Witness {
                evidence: lower_type_reference_into_table(lowerer, evidence)?,
            }
        }
        resolved::proposition::PropositionBody::Transparent { proposition } => {
            let formula = if let resolved::expression::ExpressionNode::Call(call) = lowerer
                .source_trees
                .tables
                .bodies
                .expressions
                .expression(*proposition)
                && call.target_symbol.is_valid()
                && matches!(
                    lowerer.source_trees.symbols.get(call.target_symbol).kind,
                    symbols::SymbolKind::Proposition | symbols::SymbolKind::PropositionParameter
                ) {
                typed::proposition::PropositionFormula::Application(lower_proposition_application(
                    lowerer, call,
                )?)
            } else {
                typed::proposition::PropositionFormula::BooleanExpression(
                    lower_expression_handle_from_table_in_fact_position(
                        lowerer.source_trees,
                        &lowerer.source_trees.tables.bodies.expressions,
                        &mut lowerer.typed_trees,
                        *proposition,
                    )?,
                )
            };
            typed::proposition::PropositionBody::Transparent {
                proposition: formula,
            }
        }
    };

    Ok(typed_proposition)
}
