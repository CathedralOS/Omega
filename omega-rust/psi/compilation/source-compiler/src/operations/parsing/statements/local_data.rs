use crate::parsing::expressions::parse::parse_expression_handle;
use crate::parsing::statements::expressions::expression_handle_to_statement_call;
use crate::parsing::token_input::cursor::{Input, ParseResult};
use crate::parsing::types::references::parse_type_reference_handle_allowing_borrow;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, TableIndexedExpression, TableMemberExpression,
};
use syntax_trees::statement::{StatementHandle, StatementNode, TableLocalData};
use syntax_trees::types::TypeReferenceHandle;
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

    Ok((
        syntax_trees.statements.insert(StatementNode::Call(call)),
        rest,
    ))
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
    let copy = match syntax_trees.expressions.expression(expression).clone() {
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

    Some(syntax_trees.expressions.insert(copy))
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
            members: syntax_trees
                .expressions
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
    let copy = match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Boolean(value) => ExpressionNode::Boolean(value),
        ExpressionNode::Integer(value) => ExpressionNode::Integer(value),
        node @ (ExpressionNode::Name { .. } | ExpressionNode::Member(_)) => {
            copied_name_or_member(syntax_trees, node)?
        }
        _ => return None,
    };

    Some(syntax_trees.expressions.insert(copy))
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
        crate::parsing::parameters::relevance::parse_binding_relevance_brackets(input, "local")?;
    let (type_reference, input) = if input.at_punctuation(PunctuationKind::Colon) {
        let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
        parse_type_reference_handle_allowing_borrow(syntax_trees, input)?
    } else {
        (TypeReferenceHandle::invalid(), input)
    };
    let (initial_value, input) = if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (expression, input) = parse_expression_handle(syntax_trees, input)?;
        (expression, input)
    } else {
        (ExpressionHandle::invalid(), input)
    };
    if !type_reference.is_valid() && !initial_value.is_valid() {
        return Err(input.error_here("a local without a type annotation requires an initializer"));
    }
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
