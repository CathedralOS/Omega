//! Statement parsing.
//!
//! This file selects statement grammar and publishes each form's statement span.
//! `discard_and_local_data.rs` parses discards, `if` transitions and local
//! data, `inline_assembly.rs` parses `asm` blocks and their contracts,
//! `atomic_lets.rs` classifies parsed initializers and expands atomic bindings
//! and stores, `destructure_and_proof_output.rs` parses destructuring and
//! proof-output bindings and `statement_tables.rs` copies expression
//! handles and identifier paths into the statement table.

use super::atomic_lets::try_desugar_atomic_let;
use super::destructure_and_proof_output::{
    reject_retired_proof_output_binding, try_parse_destructure_let, try_parse_proof_output_binding,
};
use super::inline_assembly::parse_asm_block_statement_handles;

use crate::bodies::statements::atomic_lets::try_desugar_atomic_store;
use crate::bodies::statements::discard_and_local_data::{
    copy_compound_assignment_target, parse_discard_statement_handle,
    parse_if_transition_statement_handle, parse_local_data,
};
use crate::bodies::statements::statement_tables::{
    expression_handle_to_statement_call, root_binding_declaration,
};
use crate::bodies::transitions::parse_transition::parse_transition_block_handles;
use crate::expressions::parse_expression::parse_expression_handle;
use crate::input::token_cursor::{Input, ParseResult};
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
            let handle = syntax_trees.items.append_statement_handle(statement);
            HandleSpan::from_parts(handle, 1)
        };
        Ok((statements, rest))
    } else {
        let (statement, rest) = parse_statement_handle(syntax_trees, input)?;
        let handle = syntax_trees.items.append_statement_handle(statement);
        Ok((HandleSpan::from_parts(handle, 1), rest))
    }
}

fn parse_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    if input.at_keyword(KeywordKind::If) {
        return parse_if_transition_statement_handle(syntax_trees, input);
    }

    // `relax` RETIRED (owner, 2026-07-17): superseded by invariant windows
    // (ch11) -- a write that momentarily violates a `where` fact OPENS a
    // window the consumption points police; no scope spelling needed.
    if input.at_contextual("relax") {
        return Err(input.error_here(
            "`relax` is retired: invariant windows (ch11) supersede it -- write \
             plainly; a momentary violation opens a window that must close before \
             any read, call, or terminal exit",
        ));
    }

    // TASK RUNTIME TR1: implicit fire-and-forget and the synchronous spawn
    // desugar are retired. `spawn` remains a legal ordinary identifier when
    // it is not followed by the former block syntax.
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
        let cause = match cause.as_str() {
            "Trap" => syntax_trees::item::CrashCause::Trap,
            "Abort" => syntax_trees::item::CrashCause::Abort,
            _ => {
                return Err(input.error_here(format!(
                    "unknown crash cause `{}`; expected `Trap` or `Abort`",
                    cause.as_str()
                )));
            }
        };
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        let target = syntax_trees
            .statements
            .insert_transition_target(TransitionTargetNode::Terminal);
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Transition(TableTransition {
                    target,
                    continuation: TransitionTargetHandle::invalid(),
                    guard: TransitionGuardNode::Always,
                    proof_selectors: HandleSpan::empty(),
                    exit: TransitionExit::Crash(cause),
                    source_span,
                })),
            input,
        ));
    }

    let (expression, input) = parse_expression_handle(syntax_trees, input)?;

    if let Some(binding) = root_binding_declaration(syntax_trees, expression)? {
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::RootBinding(binding)),
            input,
        ));
    }

    // ATOMICS STAGE 1 (ch17, M2): `atomic_place.store(value, ordering);` is
    // desugared here into `atomic_place = value;`. The postfix parser keeps
    // the Call node intact (target="store", 2 arguments) so we can detect it.
    // The postfix parser has already rejected orderings that stores cannot
    // express. Exact target ordering strength remains a lowering obligation.
    if let Some(assignment) = try_desugar_atomic_store(syntax_trees, expression) {
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Assignment(assignment)),
            input,
        ));
    }

    if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (value, input) = parse_expression_handle(syntax_trees, input)?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Assignment(TableAssignment {
                    target: expression,
                    value,
                })),
            input,
        ));
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
        let value =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: read_target,
                    operator,
                    right,
                }));
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Assignment(TableAssignment {
                    target: expression,
                    value,
                })),
            input,
        ));
    }

    if input.at_punctuation(PunctuationKind::RightBrace) {
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Expression(expression)),
            input,
        ));
    }

    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    if let Some(call) = expression_handle_to_statement_call(syntax_trees, expression) {
        Ok((
            syntax_trees.statements.insert(StatementNode::Call(call)),
            input,
        ))
    } else {
        Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Expression(expression)),
            input,
        ))
    }
}
