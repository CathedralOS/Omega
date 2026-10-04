//! Arithmetic normalization after the selected callee has an identity.
//!
//! `abs` and `clamp` reuse subtraction and min/max. Expanding by spelling
//! before resolution used to erase same-named callbacks and could bind the
//! generated min/max to an unrelated callback. Every generated call here names
//! its exact builtin; authored visibility, selection partition and operation
//! coordinates follow the original occurrence through the rewrite.
//!
//! Absolute value retains the existing shared-operand expansion and its
//! call-operand restriction. This pass does not introduce another evaluator or
//! change the source operation's overflow policy.

use crate::resolution::lowerer::{Lowerer, PendingAuthoredExpression};
use diagnostics::Diagnostic;
use source::{SourceSpan, Span};
use symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable, TableBinaryExpression,
    TableCallExpression,
};
use symbol_resolved_trees::name::DiagnosticName;
use symbols::BuiltinFunction;

pub(crate) fn normalize(lowerer: &mut Lowerer) -> Result<(), Vec<Diagnostic>> {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let selected: Vec<_> = expressions
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Call(call) = expression else {
                return None;
            };
            let function = lowerer
                .symbol_resolved_trees
                .symbols
                .builtin_function_for_symbol(call.target_symbol)?;
            function
                .is_source_normalization()
                .then_some((handle, function))
        })
        .collect();
    for (expression, function) in selected {
        normalize_call(lowerer, expression, function).map_err(|diagnostic| vec![diagnostic])?;
    }
    Ok(())
}

fn expressions(lowerer: &mut Lowerer) -> &mut ExpressionTable {
    &mut lowerer.symbol_resolved_trees.tables.bodies.expressions
}

fn normalize_call(
    lowerer: &mut Lowerer,
    expression: ExpressionHandle,
    function: BuiltinFunction,
) -> Result<(), Diagnostic> {
    let ExpressionNode::Call(call) = expressions(lowerer).expression(expression).clone() else {
        return Ok(());
    };
    let arguments = expressions(lowerer)
        .expression_handles(call.arguments)
        .to_vec();
    let expected = if function == BuiltinFunction::Abs {
        1
    } else {
        3
    };
    if arguments.len() != expected
        || call.receiver.is_valid()
        || !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
    {
        return Err(Diagnostic::error(format!(
            "{} requires {expected} ordinary value arguments and no receiver, static or evidence arguments",
            function.name(),
        )));
    }
    let token = call.target.source_span();
    let normalized = match function {
        BuiltinFunction::Abs => {
            let operand = arguments[0];
            if matches!(
                expressions(lowerer).expression(operand),
                ExpressionNode::Call(_)
            ) {
                return Err(Diagnostic::error(
                    "abs(...) duplicates its argument, so a call argument would run twice; \
                     bind it to a local first (`let v = f(); abs(v)`)",
                ));
            }
            let zero = expressions(lowerer).insert(ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::zero(),
            ));
            let negated =
                expressions(lowerer).insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: zero,
                    operator: BinaryOperator::Subtract,
                    right: operand,
                }));
            retain_occurrence(lowerer, expression, negated, token);
            builtin_call(
                lowerer,
                &call,
                BuiltinFunction::Max,
                token,
                &[operand, negated],
            )?
        }
        BuiltinFunction::Clamp => {
            let maximum =
                builtin_call(lowerer, &call, BuiltinFunction::Max, token, &arguments[..2])?;
            let maximum = expressions(lowerer).insert(ExpressionNode::Call(maximum));
            retain_occurrence(lowerer, expression, maximum, token);
            // The two operations have distinct authored coordinates, so one
            // occurrence never selects both minimum and maximum declarations.
            let last = expressions(lowerer).source_span(arguments[2]);
            let application =
                if last.source_id == token.source_id && last.span.end > token.span.start {
                    SourceSpan::new(token.source_id, Span::new(token.span.start, last.span.end))
                } else {
                    token
                };
            builtin_call(
                lowerer,
                &call,
                BuiltinFunction::Min,
                application,
                &[maximum, arguments[2]],
            )?
        }
        _ => return Ok(()),
    };
    *expressions(lowerer).expression_mut(expression) = ExpressionNode::Call(normalized);
    Ok(())
}

fn builtin_call(
    lowerer: &mut Lowerer,
    original: &TableCallExpression,
    function: BuiltinFunction,
    span: SourceSpan,
    arguments: &[ExpressionHandle],
) -> Result<TableCallExpression, Diagnostic> {
    let target_symbol = lowerer
        .symbol_resolved_trees
        .symbols
        .builtin_function_symbol(function)
        .ok_or_else(|| {
            Diagnostic::error("arithmetic normalization requires an installed builtin identity")
        })?;
    let arguments = expressions(lowerer).insert_expression_handles(arguments.iter().copied());
    let mut call = original.clone();
    call.target = DiagnosticName::from_str(function.name(), span);
    call.target_symbol = target_symbol;
    call.arguments = arguments;
    Ok(call)
}

fn retain_occurrence(
    lowerer: &mut Lowerer,
    original: ExpressionHandle,
    generated: ExpressionHandle,
    span: SourceSpan,
) {
    let exposure = expressions(lowerer).authored_expression_exposure(original);
    let partition = expressions(lowerer).compiler_selection_partition(original);
    expressions(lowerer).set_source_span(generated, span);
    if let Some(exposure) = exposure {
        expressions(lowerer).set_authored_expression_exposure(generated, exposure);
        lowerer
            .pending_authored_expressions
            .push(PendingAuthoredExpression {
                expression: generated,
                exposure,
            });
    }
    if let Some(partition) = partition {
        expressions(lowerer).set_compiler_selection_partition(generated, partition);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ResolutionRequest, resolve};
    use source_files_to_tokens::Lexer;
    use tokens_to_syntax_trees::parse_syntax_trees;

    fn resolved(source: &str) -> symbol_resolved_trees::SymbolResolvedTrees {
        let syntax =
            parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
        resolve(ResolutionRequest::new(syntax)).expect("resolve")
    }

    #[test]
    fn arithmetic_named_callbacks_are_not_expanded() {
        for (name, parameters, arguments) in [
            ("abs", "input: i32", "value"),
            (
                "clamp",
                "input: i32, lower: i32, upper: i32",
                "value, 0, 10",
            ),
        ] {
            let program = resolved(&format!(
                "machine evaluate<machine {name}>(value: i32) -> bool
                 where machine {name}({parameters}) -> bool; {{ ({name}({arguments})) }}"
            ));
            let calls: Vec<_> = program
                .tables
                .bodies
                .expressions
                .iter_expressions()
                .filter_map(|(_, expression)| {
                    if let ExpressionNode::Call(call) = expression {
                        Some(call)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(calls.len(), 1);
            assert_eq!(
                program.symbols.get(calls[0].target_symbol).kind,
                symbols::SymbolKind::MachineParameter
            );
        }
    }

    #[test]
    fn generated_minimum_and_maximum_keep_exact_builtin_identities() {
        let program = resolved(
            "machine evaluate<machine min, machine max>(value: i32) -> i32
             where machine min(left: i32, right: i32) -> bool;
             where machine max(left: i32, right: i32) -> bool;
             { (clamp(value, 0, 10)) }",
        );
        let functions: Vec<_> = program
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .filter_map(|(_, expression)| {
                if let ExpressionNode::Call(call) = expression {
                    program
                        .symbols
                        .builtin_function_for_symbol(call.target_symbol)
                } else {
                    None
                }
            })
            .collect();
        assert!(functions.contains(&BuiltinFunction::Min));
        assert!(functions.contains(&BuiltinFunction::Max));
        assert!(!functions.contains(&BuiltinFunction::Clamp));
    }

    #[test]
    fn genuine_absolute_value_expands_after_resolution() {
        let program = resolved("machine evaluate(value: i32) -> i32 { (abs(value)) }");
        assert!(program.tables.bodies.expressions.iter_expressions().any(|(_, expression)| {
            matches!(expression, ExpressionNode::Call(call) if program.symbols.builtin_function_for_symbol(call.target_symbol) == Some(BuiltinFunction::Max))
        }));
        assert!(!program.tables.bodies.expressions.iter_expressions().any(|(_, expression)| {
            matches!(expression, ExpressionNode::Call(call) if program.symbols.builtin_function_for_symbol(call.target_symbol) == Some(BuiltinFunction::Abs))
        }));
    }
}
