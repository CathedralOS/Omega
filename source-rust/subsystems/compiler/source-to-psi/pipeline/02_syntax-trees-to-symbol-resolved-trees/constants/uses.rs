//! Bind each constant use to the declaration visible at that source location.

use crate::resolution::constants::declaration_values::selected_expression_constant;
use crate::resolution::tables::body_expressions;
use crate::source_resolved::SymbolResolvedTrees;
use diagnostics::Diagnostic;
use symbols::SymbolKind;

pub(crate) fn bind_constant_uses(
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
        let initializer = declaration.initializer;
        if mode != crate::resolution::lowering::lowerer::ConstResolutionMode::ArgumentSelection
            && declaration.evaluated_value.is_some()
            && (!body_expressions(program).expression_is_valid(initializer)
                || program.tables.bodies.expressions.source_span(initializer)
                    != declaration.initializer_source_span)
        {
            return Err(Diagnostic::error(
                "constant use selected a declaration with a missing or mismatched initializer",
            )
            .with_source_span(reference));
        }
        // A use keeps the declaration selected in its own scope. Evaluation
        // stores the value on that declaration, including for literal initializers.
        let expressions = &mut program.tables.bodies.expressions;
        let ExpressionNode::Name(path) = expressions.expression_mut(occurrence.expression) else {
            unreachable!("selected_expression_constant only selects name paths");
        };
        path.symbol = selected;
        let member_symbols = path.member_symbols;
        if member_symbols.count() == 1 {
            path.head_symbol = selected;
        }
        let head = path.head_symbol;
        let members = path.members;
        let owner_path = crate::resolution::names::constant_owner_path(
            &program.symbols,
            expressions.name_path_members(members),
            head,
        );
        for (index, member) in owner_path.into_iter().enumerate() {
            expressions.set_name_path_member_symbol_at_offset(
                member_symbols,
                index.try_into().expect("name path member count overflow"),
                member,
            );
        }
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
    }
    Ok(())
}
