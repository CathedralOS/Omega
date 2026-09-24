use crate::lowerer::Lowerer;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

mod domain_membership;
mod name_paths;
mod operators;
mod table;
#[cfg(test)]
mod tests;

pub(crate) fn lower_expression_handle(
    lowerer: &mut Lowerer,
    expression: resolved::expression::ExpressionHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    let Lowerer {
        typed_trees,
        source_trees,
        equality_scope,
        ..
    } = lowerer;
    let source = &source_trees.tables.bodies.expressions;
    table::lower_expression_handle_from_table_in_scope(
        source_trees,
        source,
        typed_trees,
        expression,
        equality_scope.as_ref(),
    )
}

pub(crate) fn lower_expression_handle_from_table(
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::TypedTrees,
    expression: resolved::expression::ExpressionHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    lower_expression_handle_from_table_with_self_substitution(
        None, source, target, expression, None,
    )
}

pub(crate) fn lower_expression_handle_from_table_with_self_substitution(
    program: Option<&resolved::SymbolResolvedTrees>,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::TypedTrees,
    expression: resolved::expression::ExpressionHandle,
    self_substitution: Option<typed::expression::ExpressionHandle>,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    table::lower_expression_handle_from_table_with_self_substitution(
        program,
        source,
        target,
        expression,
        self_substitution,
    )
}

/// Lower an expression in proof-fact position: equality over recursive
/// (proof-only) data stays a raw Binary for the structural entailment judge
/// instead of demanding runtime synthesis.
pub(crate) fn lower_expression_handle_from_table_in_fact_position(
    program: &resolved::SymbolResolvedTrees,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::TypedTrees,
    expression: resolved::expression::ExpressionHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    table::lower_expression_handle_from_table_in_fact_position(program, source, target, expression)
}

pub(crate) fn lower_static_machine_argument(
    program: Option<&resolved::SymbolResolvedTrees>,
    target: &mut typed::TypedTrees,
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
    argument: &resolved::expression::StaticMachineArgument,
) -> Result<typed::expression::StaticMachineArgument, Diagnostic> {
    let type_reference = if argument.type_reference.is_valid() {
        let program = program.ok_or_else(|| {
            Diagnostic::error("structural static type argument requires its resolved program")
        })?;
        crate::type_reference::lower_type_reference_into_trees_with_exposure(
            program,
            target,
            program.child_type_reference(argument.type_reference),
            exposure,
        )?
    } else {
        typed::types::TypeReferenceHandle::invalid()
    };
    Ok(typed::expression::StaticMachineArgument {
        type_reference,
        path: argument
            .path
            .iter()
            .map(crate::lowerer::name::lower_name)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        application: argument
            .application
            .as_ref()
            .map(|application| {
                Ok::<_, Diagnostic>(Box::new(typed::expression::StaticSymbolApplication {
                    lifetime_arguments: application
                        .lifetime_arguments
                        .iter()
                        .map(crate::lowerer::name::lower_name)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    arguments: application
                        .arguments
                        .iter()
                        .map(|argument| {
                            lower_static_machine_argument(program, target, exposure, argument)
                        })
                        .collect::<Result<Vec<_>, Diagnostic>>()?
                        .into_boxed_slice(),
                }))
            })
            .transpose()?,
        const_literal: argument.const_literal.clone(),
        evidence_projection: argument.evidence_projection.as_ref().map(|projection| {
            typed::expression::EvidenceProjection {
                term: crate::lowerer::name::lower_name(&projection.term),
                member: crate::lowerer::name::lower_name(&projection.member),
            }
        }),
        symbol: argument.symbol,
    })
}
