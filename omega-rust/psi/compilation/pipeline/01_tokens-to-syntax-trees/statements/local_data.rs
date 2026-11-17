use crate::expressions::parse::parse_expression_handle;
use crate::statements::expressions::expression_handle_to_statement_call;
use crate::syntax_storage::tables::{exprs, exprs_of, stmts};
use crate::token_input::cursor::{Input, ParseResult};
use crate::types::references::parse_type_reference_handle_allowing_borrow;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, TableIndexedExpression, TableMemberExpression,
};
use syntax_trees::statement::{StatementHandle, StatementNode, TableLocalData};
use tokens::PunctuationKind;

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

    Ok((stmts(syntax_trees).insert(StatementNode::Call(call)), rest))
}

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
    let copy = match exprs_of(syntax_trees).expression(expression).clone() {
        node @ (ExpressionNode::Name { .. } | ExpressionNode::Member(_)) => {
            copied_name_or_member(syntax_trees, node)?
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = copy_compound_assignment_target(syntax_trees, indexed.collection)?;
            let index = copy_stable_compound_assignment_index(syntax_trees, indexed.index)?;
            ExpressionNode::Indexed(TableIndexedExpression { collection, index })
        }
        _ => return None,
    };

    Some(exprs(syntax_trees).insert(copy))
}

fn copied_name_or_member(
    syntax_trees: &mut SyntaxTrees,
    node: ExpressionNode,
) -> Option<ExpressionNode> {
    Some(match node {
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } if path_application.is_none() => ExpressionNode::Name {
            members: exprs(syntax_trees)
                .copy_identifier_path_prefix(path.to_owned(), path.to_owned().len()),
            application: None,
        },
        ExpressionNode::Member(member) => ExpressionNode::Member(TableMemberExpression {
            receiver: copy_compound_assignment_target(syntax_trees, member.receiver)?,
            access_source_span: member.access_source_span,
            member: member.member,
            case_variant: member.case_variant.clone(),
        }),
        _ => return None,
    })
}

fn copy_stable_compound_assignment_index(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let copy = match exprs_of(syntax_trees).expression(expression).clone() {
        ExpressionNode::Boolean(value) => ExpressionNode::Boolean(value),
        ExpressionNode::Integer(value) => ExpressionNode::Integer(value),
        node @ (ExpressionNode::Name { .. } | ExpressionNode::Member(_)) => {
            copied_name_or_member(syntax_trees, node)?
        }
        _ => return None,
    };

    Some(exprs(syntax_trees).insert(copy))
}

pub(crate) fn parse_local_data<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TableLocalData> {
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
    let (relevance, input) =
        crate::parameters::relevance::parse_binding_relevance_brackets(input, "local")?;
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
            role: language_core::LocalBindingRole::Value,
            name,
            type_reference,
            initial_value,
            is_mutable,
            relevance,
            pattern_fields: Default::default(),
        },
        input,
    ))
}
