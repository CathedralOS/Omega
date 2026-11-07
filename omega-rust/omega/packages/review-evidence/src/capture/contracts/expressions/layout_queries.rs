// Checked layout-query (`size` / `alignment`) projection in inert review expressions.
// Normalized type identity owns query denotation, while the remaining static payload
// verifies resolved source correspondence against immutable compiler custody.

use crate::capture::SourceDeclarationInput;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::types::signature_type_identity;
use crate::record::{
    PackageReviewContractExpression, PackageReviewLayoutQueryKind, PackageReviewTypeIdentity,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, StaticMachineArgument, TableCallExpression};

pub(super) fn project_layout_query(
    compilation: &SourceDeclarationInput<'_>,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    expression: ExpressionHandle,
    call: &TableCallExpression,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    let query = call
        .layout_query
        .as_ref()
        .ok_or_else(|| context.rejected("has no layout query"))?;
    let original_call = compilation
        .original_program
        .typed
        .expression_table
        .call_node(expression)
        .ok_or_else(|| context.rejected("layout query has no immutable authored occurrence"))?;
    let original_query = original_call
        .layout_query
        .as_ref()
        .ok_or_else(|| context.rejected("layout query has no immutable authored query"))?;
    if original_query.kind != query.kind {
        return Err(context.rejected("layout-query kind differs from immutable source custody"));
    }
    for call in [call, original_call] {
        if call.receiver.is_valid()
            || call.target_symbol.is_valid()
            || !call.arguments.is_empty()
            || !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.native_code_layout.is_some()
            || call.build_selection.is_valid()
            || call.authored_machine_application.is_valid()
            || call.exact_coercion_target.is_valid()
            || call.requirement_application.is_valid()
            || call.static_machine_parameter.is_valid()
            || call.static_requirement_dispatch.is_some()
            || call.authored_call_selection.is_some()
            || call.authored_call_binding.is_some()
        {
            return Err(
                context.rejected("has a non-query semantic payload on a layout observation")
            );
        }
    }
    let original = compilation.original();
    let operand = operand_type(compilation, context, binders, &query.operand)?;
    let expected = operand_type(&original, context, binders, &original_query.operand)?;
    if operand != expected {
        return Err(context.rejected("layout-query operand differs from immutable source custody"));
    }
    if query.operand.path != original_query.operand.path
        || source_operand(compilation, context, &query.operand)?
            != source_operand(&original, context, &original_query.operand)?
    {
        return Err(context.rejected("layout-query source operand differs from immutable custody"));
    }
    let selections = compilation
        .expression_table
        .authored_selection_occurrences(expression)
        .collect::<Vec<_>>();
    let originals = original
        .expression_table
        .authored_selection_occurrences(expression)
        .collect::<Vec<_>>();
    if selections != originals
        || selections.iter().any(|selection| {
            compilation
                .authored_declaration_selections()
                .get(*selection)
                != original.authored_declaration_selections().get(*selection)
        })
    {
        return Err(context
            .rejected("layout-query authored operand selections differ from immutable custody"));
    }
    let kind = match query.kind {
        language_core::data_layout_queries::DataLayoutQueryKind::Size => {
            PackageReviewLayoutQueryKind::Size
        }
        language_core::data_layout_queries::DataLayoutQueryKind::Alignment => {
            PackageReviewLayoutQueryKind::Alignment
        }
    };
    Ok(PackageReviewContractExpression::LayoutQuery { kind, operand })
}

fn operand_type(
    compilation: &SourceDeclarationInput<'_>,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    operand: &StaticMachineArgument,
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    if !operand.type_reference.is_valid()
        || operand.const_literal.is_some()
        || operand.evidence_projection.is_some()
    {
        return Err(context.rejected("layout query does not retain one normalized type operand"));
    }
    signature_type_identity(
        compilation,
        compilation.checking_input,
        compilation.exact_toolchain_sources,
        operand.type_reference,
        binders,
        context.lifetime_binders,
        &[],
        context.lifetime_substitutions,
        false,
        context.selection_exposure,
    )
}

fn source_operand(
    compilation: &SourceDeclarationInput<'_>,
    context: &ContractProjectionContext<'_>,
    operand: &StaticMachineArgument,
) -> Result<Option<String>, Vec<Diagnostic>> {
    if !operand.symbol.is_valid() && operand.application.is_none() {
        return Ok(None);
    }
    let owner = match context.point {
        facts::ProgramPoint::Definition { symbol } => symbol,
        facts::ProgramPoint::Machine { machine_symbol } => machine_symbol,
        _ => SymbolHandle::invalid(),
    };
    let mut source = operand.clone();
    source.type_reference = Default::default();
    let program = compilation.checking_input.typed_program();
    let identity = typed_trees::type_identity::portable_applied_index_identity(
        program,
        typed_trees::type_identity::IndexIdentityArgument::StaticArgument(&source),
        owner,
        &[],
    )
    .ok_or_else(|| {
        context.rejected("layout-query source application has no complete portable identity")
    })?;
    Ok(Some(program.type_identity_text(identity).ok_or_else(
        || context.rejected("layout-query source application identity is unavailable"),
    )?))
}
