//! Discard statements, `if` transition statements and local data bindings.

use crate::bodies::statements::statement_tables::expression_handle_to_statement_call;
use crate::expressions::parse_expression::parse_expression_handle;
use crate::input::token_cursor::{Input, ParseResult};
use crate::type_syntax::parse_type::parse_type_reference_handle_allowing_borrow;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, TableIndexedExpression, TableMemberExpression,
};
use syntax_trees::statement::{StatementHandle, StatementNode, TableLocalData};
use tokens::PunctuationKind;

/// `_ = call();` -- an explicit-discard statement. The call executes and its
/// non-unit result is intentionally dropped (frozen decision 9: discarding a
/// non-unit result silently is a compile error; `_ =` is the spelling for an
/// intentional discard).
pub(crate) fn parse_discard_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
    let (expression, rest) = parse_expression_handle(syntax_trees, input)?;
    let rest = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;

    let Some(mut call) = expression_handle_to_statement_call(syntax_trees, expression) else {
        return Err(input.error_here("`_ =` discards a call result; only a call can follow `_ =`"));
    };
    call.discards_result = true;

    Ok((
        syntax_trees.statements.insert(StatementNode::Call(call)),
        rest,
    ))
}

/// RETIRED (settled 2026-07-02: "if isn't a thing"). The `if` STATEMENT had
/// no `else` and never set a continuation, so its dispatch could always fall
/// through -- unwritable since the no-silent-fall-through rule, and used
/// exactly once in the whole corpus. Dispatch is `transition`. (The pattern
/// guard `Type::Case { x } if x > 3 ->` inside a transition arm is a
/// DIFFERENT surface and stays.)
pub(crate) fn parse_if_transition_statement_handle<'tokens, 'source>(
    _syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    Err(input.error_here(
        "the `if` statement is retired; dispatch is `transition <guard> { true -> ... _ -> ... }` \
         (every arm set must provably cover all cases)",
    ))
}

pub(crate) fn copy_compound_assignment_target(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let copy = match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Name(path) => {
            let path = syntax_trees
                .expressions
                .copy_identifier_path_prefix(path, path.len());
            ExpressionNode::Name(path)
        }
        ExpressionNode::SelfValue => ExpressionNode::SelfValue,
        ExpressionNode::Member(member) => {
            let receiver = copy_compound_assignment_target(syntax_trees, member.receiver)?;
            ExpressionNode::Member(TableMemberExpression {
                receiver,
                member: member.member,
                case_variant: member.case_variant.clone(),
            })
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = copy_compound_assignment_target(syntax_trees, indexed.collection)?;
            let index = copy_stable_compound_assignment_index(syntax_trees, indexed.index)?;
            ExpressionNode::Indexed(TableIndexedExpression { collection, index })
        }
        _ => return None,
    };

    Some(syntax_trees.expressions.insert(copy))
}

fn copy_stable_compound_assignment_index(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let copy = match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Boolean(value) => ExpressionNode::Boolean(value),
        ExpressionNode::Integer(value) => ExpressionNode::Integer(value),
        ExpressionNode::Name(path) => {
            let path = syntax_trees
                .expressions
                .copy_identifier_path_prefix(path, path.len());
            ExpressionNode::Name(path)
        }
        ExpressionNode::SelfValue => ExpressionNode::SelfValue,
        ExpressionNode::Member(member) => {
            let receiver = copy_compound_assignment_target(syntax_trees, member.receiver)?;
            ExpressionNode::Member(TableMemberExpression {
                receiver,
                member: member.member,
                case_variant: member.case_variant.clone(),
            })
        }
        _ => return None,
    };

    Some(syntax_trees.expressions.insert(copy))
}

pub(crate) fn parse_local_data<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TableLocalData> {
    // `let mut x: T` -- the mutable-local spelling (ch3/ch14). `mut` stays
    // contextual: `let mut: T` (a local literally named mut) keeps parsing
    // because the identifier arm only fires when ANOTHER identifier follows.
    let (is_mutable, input) = if input.at_contextual("mut")
        && input
            .take_contextual("mut")
            .is_ok_and(|rest| rest.at_name_like())
    {
        (true, input.take_contextual("mut")?)
    } else {
        (false, input)
    };
    let (name, input) = input.take_identifier()?;
    // `[erased]` marks the binding occurrence: `let proof [erased]: T` is a
    // proof-side local with the same relevance marker as an erased field.
    let (relevance, input) =
        crate::parameters::binding_properties::parse_binding_relevance_brackets(input, "local")?;
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (type_reference, input) = parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
    let (initial_value, input) = if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (expression, input) = parse_expression_handle(syntax_trees, input)?;
        (expression, input)
    } else {
        (ExpressionHandle::invalid(), input)
    };
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;

    Ok((
        TableLocalData {
            name,
            type_reference,
            initial_value,
            is_mutable,
            relevance,
        },
        input,
    ))
}
