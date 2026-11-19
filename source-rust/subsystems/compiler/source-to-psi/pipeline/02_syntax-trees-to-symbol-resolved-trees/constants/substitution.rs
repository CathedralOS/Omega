/* Substituting resolved constants into expressions. */

use crate::resolution::constants::declaration_values::selected_expression_constant;
use crate::resolution::tables::body_expressions;
use crate::source_resolved::SymbolResolvedTrees;
use diagnostics::Diagnostic;
use symbols::SymbolKind;
use syntax_trees::item::ConstDefinition;

pub(crate) fn substitute_resolved_constants(
    program: &mut SymbolResolvedTrees,
    authored: &[crate::resolution::lowering::lowerer::PendingAuthoredExpression],
    selections: &mut Vec<crate::resolution::lowering::lowerer::PendingConstSelection>,
    mode: crate::resolution::lowering::lowerer::ConstResolutionMode,
) -> Result<(), Diagnostic> {
    use crate::source_resolved::expression::ExpressionNode;
    let declarations = program
        .roots
        .const_declarations
        .iter()
        .map(|declaration| declaration.symbol)
        .collect::<Vec<_>>();
    for (ordinal, symbol) in declarations.iter().enumerate() {
        if declarations[..ordinal].iter().any(|other| {
            program.symbols.name(*other) == program.symbols.name(*symbol)
                && program.symbols.same_symbol_source_package(*other, *symbol)
                && !program.symbols.source_scopes_separate(*other, *symbol)
        }) {
            return Err(Diagnostic::error(format!(
                "duplicate const `{}`",
                program.symbols.display_path(*symbol, "::")
            ))
            .with_source_span(
                program
                    .symbols
                    .symbol_source_span(*symbol)
                    .unwrap_or_default(),
            ));
        }
        if let Some((carrier, case)) = program.symbols.name(*symbol).rsplit_once("::") {
            let reference = program
                .symbols
                .symbol_source_span(*symbol)
                .unwrap_or_default();
            if let Some(data) = program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    carrier,
                    &[SymbolKind::Data],
                    reference,
                )
                && program
                    .symbols
                    .find_child_by_name_and_kind(data, case, SymbolKind::Variant)
                    .is_some()
            {
                return Err(Diagnostic::error(format!(
                    "const `{}` collides with the case `{case}`",
                    program.symbols.display_path(*symbol, "::")
                ))
                .with_source_span(reference));
            }
        } else if program
            .data_definitions
            .iter()
            .map(|declaration| declaration.symbol)
            .chain(program.machines.iter().map(|machine| machine.symbol))
            .any(|other| {
                program.symbols.name(other) == program.symbols.name(*symbol)
                    && program.symbols.same_symbol_source_package(other, *symbol)
                    && !program.symbols.source_scopes_separate(other, *symbol)
            })
        {
            return Err(Diagnostic::error(format!(
                "free-floating const `{}` collides with a declaration in the same namespace",
                program.symbols.display_path(*symbol, "::")
            ))
            .with_source_span(
                program
                    .symbols
                    .symbol_source_span(*symbol)
                    .unwrap_or_default(),
            ));
        }
    }
    let unsupported_array_projection_sources = body_expressions(program)
        .iter_expressions()
        .filter_map(|(_, node)| {
            let mut collection = match node {
                ExpressionNode::Indexed(indexed)
                    if !matches!(
                        program.tables.bodies.expressions.expression(indexed.index),
                        ExpressionNode::Integer(_)
                    ) || matches!(
                        body_expressions(program).expression(indexed.collection),
                        ExpressionNode::Borrow(_)
                    ) =>
                {
                    indexed.collection
                }
                ExpressionNode::Borrow(borrow)
                    if borrow.access != language_core::ReferenceAccess::Shared
                        || matches!(
                            body_expressions(program).expression(borrow.target),
                            ExpressionNode::Indexed(_) | ExpressionNode::Member(_)
                        ) =>
                {
                    borrow.target
                }
                _ => return None,
            };
            loop {
                match program.tables.bodies.expressions.expression(collection) {
                    ExpressionNode::Borrow(borrow) => collection = borrow.target,
                    ExpressionNode::Indexed(inner) => collection = inner.collection,
                    ExpressionNode::Member(member) => collection = member.receiver,
                    _ => break,
                }
            }
            Some(collection)
        })
        .collect::<Vec<_>>();
    for occurrence in authored {
        let Some((reference, selected)) =
            selected_expression_constant(program, occurrence.expression)
        else {
            continue;
        };
        let Some(declaration) = program.roots.const_declaration(selected) else {
            return Err(Diagnostic::error(
                "resolved constant selection has no retained declaration",
            ));
        };
        if !declaration.is_public
            && !program
                .symbols
                .symbol_source_span(selected)
                .is_some_and(|source| program.symbols.same_source_package(reference, source))
        {
            return Err(Diagnostic::error(format!(
                "constant `{}` is private to its declaring package",
                program.symbols.display_path(selected, "::"),
            ))
            .with_source_span(reference));
        }
        if mode == crate::resolution::lowering::lowerer::ConstResolutionMode::ArgumentSelection
            || declaration.evaluated_value.is_none()
        {
            // Evaluation replaces this retained name by its resolved leaf.
            // Carry the constant already selected for this use, including
            // when earlier name binding stopped at its qualifying carrier.
            let expressions = &mut program.tables.bodies.expressions;
            let ExpressionNode::Name(path) = expressions.expression_mut(occurrence.expression)
            else {
                unreachable!("selected_expression_constant only selects name paths");
            };
            path.symbol = selected;
            let member_symbols = path.member_symbols;
            expressions.set_name_path_member_symbol_at_offset(
                member_symbols,
                member_symbols.count() - 1,
                selected,
            );
            expressions.set_unresolved_name(occurrence.expression, Default::default());
            selections.push(
                crate::resolution::lowering::lowerer::PendingConstSelection {
                    expression: occurrence.expression,
                    source_span: reference,
                    declaration: selected,
                    exposure: occurrence.exposure,
                },
            );
            continue;
        }
        let initializer = declaration.initializer;
        if !body_expressions(program).expression_is_valid(initializer)
            || program.tables.bodies.expressions.source_span(initializer)
                != declaration.initializer_source_span
        {
            return Err(Diagnostic::error(
                "constant substitution lost its exact declaration initializer",
            )
            .with_source_span(reference));
        }
        if matches!(
            program.tables.bodies.expressions.expression(initializer),
            ExpressionNode::ArrayLiteral(_)
        ) && unsupported_array_projection_sources.contains(&occurrence.expression)
        {
            return Err(Diagnostic::error(
            "array constant projection currently requires unborrowed literal integer selectors; dynamic indexing, borrowing and slicing require value-based array projection"
        ).with_source_span(reference));
        }
        let initializer = if matches!(
            program.tables.bodies.expressions.expression(initializer),
            ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::StructLiteral(_)
                | ExpressionNode::Name(_)
        ) {
            program
                .tables
                .bodies
                .expressions
                .copy_from_self(initializer)
        } else {
            initializer
        };
        let initializer_selections = body_expressions(program)
            .authored_selection_occurrences(initializer)
            .collect::<Vec<_>>();
        program
            .tables
            .bodies
            .expressions
            .attach_authored_selection_occurrences(occurrence.expression, initializer_selections);
        let value = body_expressions(program).expression(initializer).clone();
        *program
            .tables
            .bodies
            .expressions
            .expression_mut(occurrence.expression) = value;
        program
            .tables
            .bodies
            .expressions
            .set_source_span(occurrence.expression, reference);
        selections.push(
            crate::resolution::lowering::lowerer::PendingConstSelection {
                expression: occurrence.expression,
                source_span: reference,
                declaration: selected,
                exposure: occurrence.exposure,
            },
        );
    }
    Ok(())
}

pub(crate) fn semantic_const_name(
    syntax_trees: &syntax_trees::SyntaxTrees,
    definition: &ConstDefinition,
) -> String {
    if definition.scope.as_str(syntax_trees).is_empty() {
        definition.name.as_str(syntax_trees).to_owned()
    } else {
        format!(
            "{}::{}",
            definition.scope.as_str(syntax_trees),
            definition.name.as_str(syntax_trees)
        )
    }
}
