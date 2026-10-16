//! Entry points for typing one expression from a resolved expression table.
//!
//! Each entry builds an `ExpressionTableLowerer` (in `table`) and lowers one
//! expression handle and its operands into the typed trees. They differ in
//! their inputs:
//!
//! - `lower_expression_handle` reads the package's body expression table
//!   through the `Lowerer` and carries its current equality scope, so `==`
//!   can find an operand's data type. Declaration and statement lowering use
//!   it.
//! - `lower_expression_handle_from_table` and
//!   `lower_expression_handle_from_table_with_symbol_substitution` take any
//!   expression table, with or without the resolved program; the second can
//!   substitute one resolved binder with a typed expression. Type references and
//!   constraints use them.
//! - `lower_expression_handle_from_table_in_fact_position` lowers a proof
//!   fact: equality over recursive, proof-only data stays a raw binary
//!   comparison instead of requiring runtime equality.
//!
//! `lower_static_machine_argument` lowers a static machine argument with its
//! type reference and nested application arguments; machine, trait and call
//! lowering use it, and so does `table`.
//!
//! Children: `table` holds the expression-table lowerer; `domain_membership`,
//! `name_paths` and `operators` lower membership tests, name paths and binary
//! operators for it.

use crate::lowerer::Lowerer;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

mod domain_membership;
mod name_paths;
mod operators;
mod table;
pub(crate) use table::declared_receiver_method_target;

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
    lower_expression_handle_from_table_with_symbol_substitution(
        None, source, target, expression, None,
    )
}

pub(crate) fn lower_expression_handle_from_table_with_symbol_substitution(
    program: Option<&resolved::SymbolResolvedTrees>,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::TypedTrees,
    expression: resolved::expression::ExpressionHandle,
    symbol_substitution: Option<(symbols::SymbolHandle, typed::expression::ExpressionHandle)>,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    table::lower_expression_handle_from_table_with_symbol_substitution(
        program,
        source,
        target,
        expression,
        symbol_substitution,
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
    let mut selected = typed::expression::StaticMachineArgument {
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
    };
    // Nominal type selections retain the whole operand. Looking up a bare
    // named carrier later would erase its lifetime/type application before
    // both ordinary call admission and contextual provider formation.
    if !selected.type_reference.is_valid()
        && selected.symbol.is_valid()
        && target.symbols.get(selected.symbol).kind == symbols::SymbolKind::Data
        && selected.application.is_some()
    {
        let type_reference = target
            .static_operand_type_reference(&selected)
            .map_err(Diagnostic::error)?;
        // Static operands have one custody form, not parallel named and
        // structural authorities for the same selection.
        selected = typed::expression::StaticMachineArgument {
            type_reference,
            ..typed::expression::StaticMachineArgument::default()
        };
    }
    if selected.type_reference.is_valid()
        && (target
            .static_const_literal_from_type_reference(selected.type_reference)
            .is_some()
            || matches!(target.type_reference_table.type_reference(selected.type_reference), typed::types::TypeReferenceNode::Named { symbol, .. } if !symbol.is_valid()))
    {
        selected = target
            .static_operand_from_type_reference(selected.type_reference)
            .map_err(Diagnostic::error)?;
    }
    Ok(selected)
}

pub(crate) fn lower_fact_expression(
    lowerer: &mut Lowerer,
    expression: resolved::expression::ExpressionHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    table::lower_scoped_fact_expression(
        lowerer.source_trees,
        &lowerer.source_trees.tables.bodies.expressions,
        &mut lowerer.typed_trees,
        expression,
        lowerer.equality_scope.as_ref(),
    )
}
