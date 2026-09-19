//! Rejoin declaration-owned folds before any use copies their materialized value.
//!
//! The semantic evaluator establishes the result and builtin meanings. Resolution
//! independently checks the retained scalar payload, source roots, and complete
//! selected dependency/operator roster. Original roots still undergo ordinary
//! declaration-source resolution; initializer implementation exposure never
//! becomes public index exposure just because its constant is public.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure as Exposure,
    AuthoredDeclarationSelectionIntrinsic as Intrinsic, AuthoredDeclarationSelectionKind as Kind,
    AuthoredDeclarationSelectionLateBinding as LateBinding,
    AuthoredDeclarationSelectionTarget as Target,
};
use source::SourceSpan;
use symbol_resolved_trees::{SymbolResolvedTrees, expression::ExpressionHandle};
use symbols::SymbolKind;
use syntax_trees::{SyntaxTrees, item::ConstDefinition, types::ConstArgumentOrigin};

pub(crate) struct PendingInitializer {
    declaration: SourceSpan,
    materialized: ExpressionHandle,
    pub(crate) authored: ExpressionHandle,
    references: Vec<(SourceSpan, String)>,
    operators: Vec<SourceSpan>,
    receipt: syntax_trees::item::ConstInitializerNormalization,
}

fn error(reference: SourceSpan, reason: &str) -> Diagnostic {
    Diagnostic::error(format!("constant initializer normalization: {reason}"))
        .with_source_span(reference)
}

pub(crate) fn retain(
    lowerer: &mut crate::resolution::lowerer::Lowerer,
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    materialized: ExpressionHandle,
) -> Result<(), Diagnostic> {
    let Some(receipt) = &definition.normalization else {
        return Ok(());
    };
    let reference = syntax.expressions.source_span(definition.value);
    if !syntax
        .expressions
        .contains_expression(receipt.authored_expression)
        || receipt.authored_expression == definition.value
        || syntax.expressions.source_span(receipt.authored_expression) != reference
    {
        return Err(error(reference, "lost its exact authored expression"));
    }
    let value = super::declaration_values::public_declaration_value_encoding(
        syntax,
        definition,
        lowerer.constant_selection.as_ref(),
    )
    .map_err(|_| error(reference, "materialized value is not canonical"))?;
    if value != receipt.canonical_result_encoding {
        return Err(error(
            reference,
            "materialized value drifted from its canonical result",
        ));
    }
    let mut references = Vec::new();
    let mut operators = Vec::new();
    let mut pending = vec![receipt.authored_expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !syntax.expressions.contains_expression(expression) {
            return Err(error(reference, "authored expression has an invalid child"));
        }
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        use syntax_trees::expression::{ExpressionNode, MatchPattern};
        match syntax.expressions.expression(expression) {
            ExpressionNode::Integer(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_) => {}
            ExpressionNode::Name(path) => {
                let members = syntax.expressions.identifier_path_members(*path);
                let (Some(first), Some(last)) = (members.first(), members.last()) else {
                    return Err(error(reference, "authored dependency lost its name"));
                };
                let reference = SourceSpan::new(
                    first.source_span().source_id,
                    source::Span::new(first.source_span().span.start, last.source_span().span.end),
                );
                let name = members
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::");
                let identifier = syntax_trees::identifier::Identifier::new(name.clone(), reference);
                if lowerer
                    .constant_selection
                    .as_ref()
                    .is_some_and(|selection| {
                        selection
                            .bare_case(syntax, &identifier)
                            .is_ok_and(|selected| selected.is_some())
                    })
                {
                    // A literal case retains ordinary constructor selection;
                    // it is not an evaluated named-constant dependency.
                    continue;
                }
                references.push((reference, name));
            }
            ExpressionNode::Binary(binary) => {
                operators.push(syntax.expressions.source_span(expression));
                pending.push(binary.right);
                pending.push(binary.left);
            }
            ExpressionNode::Call(call) => {
                let arguments = syntax.expressions.expression_handles(call.arguments);
                if arguments.len() != call.arguments.len() {
                    return Err(error(reference, "authored call has a stale argument span"));
                }
                pending.extend(arguments.iter().rev().copied());
            }
            ExpressionNode::ArrayLiteral(elements) => {
                pending.extend(
                    syntax
                        .expressions
                        .expression_handles(*elements)
                        .iter()
                        .rev()
                        .copied(),
                );
            }
            ExpressionNode::StructLiteral(literal) => {
                let fields = syntax.expressions.struct_fields(literal.fields);
                if fields.len() != literal.fields.len() {
                    return Err(error(
                        reference,
                        "authored constructor has a stale field span",
                    ));
                }
                pending.extend(fields.iter().rev().map(|field| field.value));
            }
            ExpressionNode::Match(dispatch) => {
                for arm in syntax.expressions.match_arms(dispatch.arms).iter().rev() {
                    pending.push(arm.value);
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                }
                pending.push(dispatch.subject);
            }
            _ => {
                return Err(error(
                    reference,
                    "authored expression is outside constant leaf evaluation",
                ));
            }
        }
    }
    let original =
        lowerer.with_authored_expression_exposure(Exposure::PrivateImplementation, |lowerer| {
            crate::lowering::expression::lower_expression_into_table(
                lowerer,
                syntax,
                receipt.authored_expression,
            )
        })?;
    lowerer.pending_const_values.push(original);
    lowerer.pending_const_initializers.push(PendingInitializer {
        declaration: definition.name.source_span(),
        materialized,
        authored: original,
        references,
        operators,
        receipt: receipt.clone(),
    });
    Ok(())
}

/// Reconstruct the transitive roster from original references, not receipt
/// membership. Even omitted or unused dependencies must rejoin current values.
fn reconstruct(
    program: &SymbolResolvedTrees,
    records: &[PendingInitializer],
    root: usize,
) -> Result<(Vec<ConstArgumentOrigin>, Vec<SourceSpan>), Diagnostic> {
    let mut origins = Vec::new();
    let mut operators = Vec::new();
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(ordinal) = pending.pop() {
        if visited.contains(&ordinal) {
            continue;
        }
        visited.push(ordinal);
        let record = &records[ordinal];
        for operator in &record.operators {
            if !operators.contains(operator) {
                operators.push(*operator);
            }
        }
        let mut dependencies = Vec::new();
        for (reference, name) in &record.references {
            let selected = program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    name,
                    &[SymbolKind::Const],
                    *reference,
                )
                .ok_or_else(|| {
                    error(*reference, "dependency no longer selects an exact constant")
                })?;
            dependencies.push((*reference, selected));
        }
        let closure = super::initializer_dependencies::collect(program, record.authored)
            .map_err(|reason| error(record.declaration, &reason))?;
        for (reference, declaration) in closure.constants {
            let selected = program
                .const_declarations
                .iter()
                .find(|candidate| {
                    program.symbols.symbol_source_span(candidate.symbol) == Some(declaration)
                })
                .ok_or_else(|| error(reference, "closure dependency lost its exact declaration"))?
                .symbol;
            if !dependencies.contains(&(reference, selected)) {
                dependencies.push((reference, selected));
            }
        }
        for (reference, selected) in &dependencies {
            let declaration = program
                .const_declarations
                .iter()
                .find(|declaration| declaration.symbol == *selected)
                .ok_or_else(|| error(*reference, "selected dependency lost its declaration"))?;
            let source = program
                .symbols
                .symbol_source_span(*selected)
                .ok_or_else(|| error(*reference, "selected dependency lost its source"))?;
            if !declaration.is_public && !program.symbols.same_source_package(*reference, source) {
                return Err(error(
                    *reference,
                    "dependency selects a private foreign declaration",
                ));
            }
            let origin = ConstArgumentOrigin {
                reference: *reference,
                declaration: source,
                initializer: declaration.initializer_source_span,
                canonical_value_encoding: declaration.canonical_value_encoding.clone().ok_or_else(
                    || error(*reference, "dependency has no completed canonical value"),
                )?,
            };
            if !origins.contains(&origin) {
                origins.push(origin);
            }
            if let Some(dependency) = records
                .iter()
                .position(|record| record.declaration == source)
            {
                if dependency == root {
                    return Err(error(*reference, "normalized initializer dependency cycle"));
                }
                pending.push(dependency);
            } else {
                append_retained_custody(
                    program,
                    declaration,
                    records[root].declaration,
                    &mut origins,
                    &mut operators,
                )?;
            }
        }
    }
    Ok((origins, operators))
}

/// A seeded base already owns the complete transitive roster on its resolved
/// initializer. Rejoin those exact selections rather than resolving its source
/// again under an extension's namespace.
fn append_retained_custody(
    program: &SymbolResolvedTrees,
    owner: &symbol_resolved_trees::constant::ConstDeclaration,
    root: SourceSpan,
    origins: &mut Vec<ConstArgumentOrigin>,
    operators: &mut Vec<SourceSpan>,
) -> Result<(), Diagnostic> {
    let owner_source = program.symbols.symbol_source_span(owner.symbol);
    for occurrence in program
        .tables
        .bodies
        .expressions
        .authored_selection_occurrences(owner.initializer)
    {
        let selection = program
            .authored_declaration_selections()
            .get(occurrence)
            .ok_or_else(|| error(root, "retained initializer lost its exact selection"))?;
        let reference = selection.source_span();
        match (selection.kind(), selection.target()) {
            (Kind::Call, Target::Resolved(selected))
                if matches!(
                    program.symbols.get(selected.selected_symbol()).kind,
                    SymbolKind::Machine | SymbolKind::State
                ) =>
            {
                // The detached original rederives call custody. A copied call
                // selection is neither a constant origin nor a builtin premise.
            }
            (Kind::Operator, Target::Intrinsic(Intrinsic::BuiltinOperator)) => {
                if !operators.contains(&reference) {
                    operators.push(reference);
                }
            }
            (Kind::StaticPathSegment, Target::Resolved(selected))
                if matches!(
                    program.symbols.get(selected.selected_symbol()).kind,
                    SymbolKind::Module | SymbolKind::Data | SymbolKind::Variant
                ) =>
            {
                // Bare literal cases retain their ordinary resolved namespace,
                // carrier and case path rows. These are constructor custody,
                // not evaluated constant dependencies. Other static selections
                // must still select an exact retained constant below.
            }
            (Kind::StaticPathSegment, Target::Resolved(selected)) => {
                let declaration = program
                    .const_declarations
                    .iter()
                    .find(|declaration| declaration.symbol == selected.selected_symbol())
                    .ok_or_else(|| {
                        error(
                            reference,
                            "retained initializer dependency is not a constant",
                        )
                    })?;
                let source = program
                    .symbols
                    .symbol_source_span(declaration.symbol)
                    .ok_or_else(|| {
                        error(reference, "retained initializer dependency lost its source")
                    })?;
                if source == root || Some(source) == owner_source {
                    return Err(error(reference, "normalized initializer dependency cycle"));
                }
                if !declaration.is_public && !program.symbols.same_source_package(reference, source)
                {
                    return Err(error(
                        reference,
                        "retained initializer selects a private foreign declaration",
                    ));
                }
                let origin = ConstArgumentOrigin {
                    reference,
                    declaration: source,
                    initializer: declaration.initializer_source_span,
                    canonical_value_encoding: declaration
                        .canonical_value_encoding
                        .clone()
                        .ok_or_else(|| {
                            error(
                                reference,
                                "retained dependency has no completed canonical value",
                            )
                        })?,
                };
                if !origins.contains(&origin) {
                    origins.push(origin);
                }
            }
            (
                Kind::StructLiteralType
                | Kind::StructLiteralCase
                | Kind::StructLiteralField
                | Kind::CaseReference,
                Target::Resolved(_),
            ) => {
                // The resolved initializer retains these exact declaration
                // selections through every copy. They are not scalar-evaluation
                // premises and therefore do not occupy normalization receipt slots.
            }
            _ => {
                return Err(error(
                    reference,
                    "retained initializer selection has no checked scalar meaning",
                ));
            }
        }
    }
    Ok(())
}

/// After ordinary authored selections have been recorded, close duplicate
/// declaration-side operator obligations using the receipt's evaluated meaning.
/// Instantiated partitions and unrelated source occurrences retain their own
/// selection obligations.
pub(crate) fn finalize_operator_obligations(
    program: &mut SymbolResolvedTrees,
    records: &[PendingInitializer],
) -> Result<(), Diagnostic> {
    let occurrences = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind() == Kind::Operator
                && selection.exposure() == Exposure::PrivateImplementation
                && selection.compiler_partition().is_none()
                && matches!(
                    selection.target(),
                    Target::LateBound(LateBinding::CheckedOperator)
                )
                && records.iter().any(|record| {
                    record
                        .receipt
                        .builtin_operators
                        .contains(&selection.source_span())
                })
        })
        .map(|selection| (selection.occurrence_id(), selection.source_span()))
        .collect::<Vec<_>>();
    for (occurrence, reference) in occurrences {
        program
            .finalize_intrinsic_authored_declaration_selection(
                occurrence,
                LateBinding::CheckedOperator,
                Intrinsic::BuiltinOperator,
            )
            .map_err(|reason| {
                error(
                    reference,
                    &format!("retained builtin selection finalization failed: {reason:?}"),
                )
            })?;
    }
    Ok(())
}

pub(crate) fn finalize(
    program: &mut SymbolResolvedTrees,
    records: &[PendingInitializer],
) -> Result<(), Diagnostic> {
    for (ordinal, record) in records.iter().enumerate() {
        let (origins, operators) = reconstruct(program, records, ordinal)?;
        let call_custody = super::initializer_dependencies::collect(program, record.authored)
            .map_err(|reason| error(record.declaration, &reason))?;
        let calls = &call_custody.calls;
        if origins.len() != record.receipt.selections.len()
            || origins
                .iter()
                .any(|origin| !record.receipt.selections.contains(origin))
            || operators.len() != record.receipt.builtin_operators.len()
            || operators
                .iter()
                .any(|operator| !record.receipt.builtin_operators.contains(operator))
            || calls.len() != record.receipt.call_selections.len()
            || calls
                .iter()
                .any(|call| !record.receipt.call_selections.contains(call))
        {
            return Err(error(
                record.declaration,
                "authored declaration, value, or operator custody drifted",
            ));
        }
        let mut occurrences = Vec::new();
        for (reference, selected) in call_custody.call_targets {
            occurrences.push(
                program
                    .record_resolved_authored_declaration_selection(
                        reference,
                        Exposure::PrivateImplementation,
                        Kind::Call,
                        selected,
                    )
                    .map_err(crate::constant::selections::const_selection_record_diagnostic)?,
            );
        }
        for origin in origins {
            let selected = program
                .const_declarations
                .iter()
                .find(|declaration| {
                    program.symbols.symbol_source_span(declaration.symbol)
                        == Some(origin.declaration)
                })
                .ok_or_else(|| error(origin.reference, "selected declaration disappeared"))?
                .symbol;
            occurrences.push(
                program
                    .record_resolved_authored_declaration_selection(
                        origin.reference,
                        Exposure::PrivateImplementation,
                        Kind::StaticPathSegment,
                        selected,
                    )
                    .map_err(crate::constant::selections::const_selection_record_diagnostic)?,
            );
        }
        for operator in operators {
            let occurrence = program
                .record_late_bound_authored_declaration_selection(
                    operator,
                    Exposure::PrivateImplementation,
                    Kind::Operator,
                    LateBinding::CheckedOperator,
                )
                .map_err(crate::constant::selections::const_selection_record_diagnostic)?;
            program
                .finalize_intrinsic_authored_declaration_selection(
                    occurrence,
                    LateBinding::CheckedOperator,
                    Intrinsic::BuiltinOperator,
                )
                .map_err(|reason| {
                    error(
                        operator,
                        &format!("builtin selection finalization failed: {reason:?}"),
                    )
                })?;
            occurrences.push(occurrence);
        }
        program
            .tables
            .bodies
            .expressions
            .attach_authored_selection_occurrences(record.materialized, occurrences);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
