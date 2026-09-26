//! Substituting resolved constants into expressions.

use crate::constant::declaration_values::selected_expression_constant;
use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::SymbolKind;
use syntax_trees::item::ConstDefinition;

pub(crate) fn substitute_resolved_constants(
    program: &mut SymbolResolvedTrees,
    authored: &[crate::resolution::lowerer::PendingAuthoredExpression],
    selections: &mut Vec<crate::resolution::lowerer::PendingConstSelection>,
    retain_selection_only: bool,
) -> Result<(), Diagnostic> {
    use symbol_resolved_trees::expression::ExpressionNode;
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
            .symbols
            .child_handles(program.symbols.root())
            .into_iter()
            .flatten()
            .any(|other| {
                matches!(
                    program.symbols.get(other).kind,
                    SymbolKind::Data | SymbolKind::Machine
                ) && program.symbols.name(other) == program.symbols.name(*symbol)
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
    // Fixed scalar projections preserve their full typed indexing expression.
    // Dynamic selectors, slicing and borrowed projections still need value/view
    // lowering. Fence their original root before substituting names, including
    // nonliteral outer selectors in a nested projection. Field selection does
    // not create storage: borrowing TABLE[0].field has the same constant root
    // as borrowing TABLE[0], even when fields and indexes alternate.
    let unsupported_array_projection_sources = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, node)| {
            let mut collection = match node {
                ExpressionNode::Indexed(indexed)
                    if !matches!(
                        program.tables.bodies.expressions.expression(indexed.index),
                        ExpressionNode::Integer(_)
                    ) || matches!(
                        program
                            .tables
                            .bodies
                            .expressions
                            .expression(indexed.collection),
                        ExpressionNode::Borrow(_)
                    ) =>
                {
                    indexed.collection
                }
                ExpressionNode::Borrow(borrow) => borrow.target,
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
        let Some(declaration_ordinal) = declarations.iter().position(|symbol| *symbol == selected)
        else {
            return Err(Diagnostic::error(
                "resolved constant selection has no retained declaration",
            ));
        };
        if retain_selection_only {
            // This private prepass publishes declaration custody, not a value
            // for typing or execution. Leave the resolved expression intact.
            selections.push(crate::resolution::lowerer::PendingConstSelection {
                expression: occurrence.expression,
                source_span: reference,
                declaration_ordinal,
                exposure: occurrence.exposure,
            });
            continue;
        }
        let declaration = &program.roots.const_declarations[declaration_ordinal];
        let initializer = declaration.initializer;
        if !program
            .tables
            .bodies
            .expressions
            .expression_is_valid(initializer)
            || program.tables.bodies.expressions.source_span(initializer)
                != declaration.initializer_source_span
        {
            return Err(Diagnostic::error(
                "constant substitution lost its exact declaration initializer",
            )
            .with_source_span(reference));
        }
        // Every aggregate child belongs to this use. Constructor and field
        // symbols were selected in the declaring source before this deep copy;
        // consumer spelling and later numeric landing cannot reinterpret them.
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
        let initializer_selections = program
            .tables
            .bodies
            .expressions
            .authored_selection_occurrences(initializer)
            .collect::<Vec<_>>();
        program
            .tables
            .bodies
            .expressions
            .attach_authored_selection_occurrences(occurrence.expression, initializer_selections);
        let value = program
            .tables
            .bodies
            .expressions
            .expression(initializer)
            .clone();
        *program
            .tables
            .bodies
            .expressions
            .expression_mut(occurrence.expression) = value;
        // The replacement remains this exact authored use, not the dependency's
        // declaration root. Replay distinguishes direct substituted values from
        // inherited transitive selection rows by this resolved occurrence.
        program
            .tables
            .bodies
            .expressions
            .set_source_span(occurrence.expression, reference);
        selections.push(crate::resolution::lowerer::PendingConstSelection {
            expression: occurrence.expression,
            source_span: reference,
            declaration_ordinal,
            exposure: occurrence.exposure,
        });
    }
    Ok(())
}

pub(crate) fn semantic_const_name(definition: &ConstDefinition) -> String {
    if definition.scope.as_str().is_empty() {
        definition.name.as_str().to_owned()
    } else {
        format!(
            "{}::{}",
            definition.scope.as_str(),
            definition.name.as_str()
        )
    }
}
