use crate::assembly::parse::parse_asm_block_statement_handles;
use crate::statements::bindings::try_desugar_atomic_let;
use crate::statements::destructuring::{
    reject_retired_proof_output_binding, try_parse_destructure_let, try_parse_proof_output_binding,
};
use crate::syntax_storage::tables::{exprs, exprs_of, items, stmts};

use crate::declarations::items::starts_machine_declaration;
use crate::expressions::parse::parse_expression_handle;
use crate::statements::bindings::try_desugar_atomic_store;
use crate::statements::expressions::{
    expression_handle_to_statement_call, root_binding_declaration,
};
use crate::statements::local_data::{
    copy_compound_assignment_target, parse_discard_statement_handle,
    parse_if_transition_statement_handle, parse_local_data,
};
use crate::token_input::cursor::{Input, ParseResult};
use crate::transitions::blocks::parse_transition_block_handles;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};
use syntax_trees::statement::{
    StatementHandle, StatementNode, TableAssignment, TableTransition, TransitionExit,
    TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode,
};
use tokens::{KeywordKind, PunctuationKind};

pub(crate) fn parse_statement_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    reject_retired_proof_output_binding(input)?;
    if input.at_keyword(KeywordKind::Transition) {
        let next = input.take_keyword(KeywordKind::Transition, "transition")?;
        parse_transition_block_handles(syntax_trees, next)
    } else if input.at_contextual("asm") {
        parse_asm_block_statement_handles(syntax_trees, input)
    } else if let Some(parsed) = try_parse_proof_output_binding(syntax_trees, input) {
        Ok(parsed)
    } else if let Some(parsed) = try_parse_destructure_let(syntax_trees, input) {
        Ok(parsed)
    } else if input.at_keyword(KeywordKind::Let) {
        let input = input.take_keyword(KeywordKind::Let, "let")?;
        let (binding, rest) = parse_local_data(syntax_trees, input)?;
        let statements = if let Some(statements) = try_desugar_atomic_let(syntax_trees, &binding) {
            statements
        } else {
            let statement = syntax_trees
                .statements
                .insert(StatementNode::LocalData(binding));
            let handle = items(syntax_trees).append_statement_handle(statement);
            HandleSpan::from_parts(handle, 1)
        };
        Ok((statements, rest))
    } else {
        let (statement, rest) = parse_statement_handle(syntax_trees, input)?;
        let handle = items(syntax_trees).append_statement_handle(statement);
        Ok((HandleSpan::from_parts(handle, 1), rest))
    }
}

fn finish_semicolon_statement<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    node: StatementNode,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((stmts(syntax_trees).insert(node), input))
}

fn parse_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    if input.at_keyword(KeywordKind::If) {
        return parse_if_transition_statement_handle(syntax_trees, input);
    }

    if input.at_contextual("relax") {
        return Err(input.error_here(
            "`relax` is retired: invariant windows (ch11) supersede it -- write \
             plainly; a momentary violation opens a window that must close before \
             any read, call, or terminal exit",
        ));
    }

    if input.at_contextual("spawn") {
        let after_spawn = input.take_contextual("spawn")?;
        if after_spawn.at_punctuation(PunctuationKind::LeftBrace) {
            return Err(input.error_here(
                "statement `spawn { ... }` is retired: task activation requires an \
                 explicit runtime capability and returns a linear `Task<T>` that must \
                 be settled or transferred; implicit detach is not supported",
            ));
        }
    }

    if input.at_contextual("_") {
        let after_underscore = input.take_contextual("_")?;
        if after_underscore.at_punctuation(PunctuationKind::Equal) {
            return parse_discard_statement_handle(syntax_trees, after_underscore);
        }
    }

    if input.at_contextual("trap") {
        return Err(input.error_here(
            "statement `trap` is retired; write `crash Trap;` and publish a covering `crashes Trap` route",
        ));
    }

    if input.at_contextual("crash") {
        let source_span = input
            .tokens
            .first()
            .map(|token| input.source_span(token))
            .unwrap_or_default();
        let input = input.take_contextual("crash")?;
        let (cause, input) = input.take_identifier()?;
        let cause = match cause.as_str(syntax_trees) {
            "Trap" => syntax_trees::item::CrashCause::Trap,
            "Abort" => syntax_trees::item::CrashCause::Abort,
            _ => {
                return Err(input.error_here(format!(
                    "unknown crash cause `{}`; expected `Trap` or `Abort`",
                    cause.as_str(syntax_trees)
                )));
            }
        };
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        let target = syntax_trees
            .statements
            .insert_transition_target(TransitionTargetNode::Terminal);
        return Ok((
            stmts(syntax_trees).insert(StatementNode::Transition(TableTransition {
                patterns: HandleSpan::empty(),
                target,
                continuation: TransitionTargetHandle::invalid(),
                guard: TransitionGuardNode::Always,
                proof_selectors: HandleSpan::empty(),
                exit: TransitionExit::Crash(cause),
                ends_dispatch: true,
                source_span,
            })),
            input,
        ));
    }

    let (expression, input) = parse_expression_handle(syntax_trees, input)?;

    if let Some(binding) = root_binding_declaration(syntax_trees, expression)? {
        return finish_semicolon_statement(
            syntax_trees,
            StatementNode::RootBinding(binding),
            input,
        );
    }

    if let Some(assignment) = try_desugar_atomic_store(syntax_trees, expression) {
        return finish_semicolon_statement(
            syntax_trees,
            StatementNode::Assignment(assignment),
            input,
        );
    }

    if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (value, input) = parse_expression_handle(syntax_trees, input)?;
        return finish_semicolon_statement(
            syntax_trees,
            StatementNode::Assignment(TableAssignment {
                target: expression,
                value,
            }),
            input,
        );
    }

    for (punctuation, label, operator) in [
        (PunctuationKind::PlusEqual, "+=", BinaryOperator::Add),
        (PunctuationKind::MinusEqual, "-=", BinaryOperator::Subtract),
        (
            PunctuationKind::AsteriskEqual,
            "*=",
            BinaryOperator::Multiply,
        ),
        (PunctuationKind::SlashEqual, "/=", BinaryOperator::Divide),
        (PunctuationKind::PercentEqual, "%=", BinaryOperator::Modulo),
    ] {
        if !input.at_punctuation(punctuation) {
            continue;
        }
        let read_target = copy_compound_assignment_target(syntax_trees, expression)
            .ok_or_else(|| input.error_here("compound assignment target must be a place"))?;
        let input = input.take_punctuation(punctuation, label)?;
        let (right, input) = parse_expression_handle(syntax_trees, input)?;
        let value = exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
            policy: numerics::arithmetic::ArithmeticDomain::Exact,
            left: read_target,
            operator,
            right,
        }));
        return finish_semicolon_statement(
            syntax_trees,
            StatementNode::Assignment(TableAssignment {
                target: expression,
                value,
            }),
            input,
        );
    }

    if input.at_punctuation(PunctuationKind::RightBrace) || starts_machine_declaration(input) {
        let target = syntax_trees
            .statements
            .insert_transition_target(TransitionTargetNode::Value(expression));
        let source_span = exprs_of(syntax_trees).source_span(expression);
        return Ok((
            stmts(syntax_trees).insert(StatementNode::Transition(TableTransition {
                patterns: HandleSpan::empty(),
                target,
                continuation: TransitionTargetHandle::invalid(),
                guard: TransitionGuardNode::Always,
                proof_selectors: HandleSpan::empty(),
                exit: TransitionExit::Ordinary,
                ends_dispatch: true,
                source_span,
            })),
            input,
        ));
    }

    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    if let Some(call) = expression_handle_to_statement_call(syntax_trees, expression) {
        Ok((stmts(syntax_trees).insert(StatementNode::Call(call)), input))
    } else {
        Ok((
            stmts(syntax_trees).insert(StatementNode::Expression(expression)),
            input,
        ))
    }
}
