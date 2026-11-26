use crate::statements::parse::parse_statement_handles;
use crate::token_input::cursor::{Input, ParseResult};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode, TableBlockExpression};
use syntax_trees::statement::{
    StatementHandle, StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};
use tokens::PunctuationKind;

pub(crate) fn parse_statements<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    requires_result: bool,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    let mut statements = arena::HandleSpan::empty();
    while (requires_result && input.at_identifier_then_contextual("machine"))
        || !crate::declarations::machine_bodies::starts_machine_member(input)
    {
        if input.at_punctuation(PunctuationKind::Arrow) {
            return Err(input.error_here(
                "machine entry bodies must use the `transition` keyword; bare `->` transitions are not supported",
            ));
        }
        let (parsed, rest) = parse_statement_handles(syntax_trees, input)?;
        let completes_body =
            syntax_trees
                .items
                .statements(parsed)
                .last()
                .is_some_and(|statement| {
                    matches!(
                        syntax_trees.statements.statement(*statement),
                        syntax_trees::statement::StatementNode::Dispatch(_)
                            | syntax_trees::statement::StatementNode::Transition(_)
                            | syntax_trees::statement::StatementNode::Expression(_)
                    )
                })
                && crate::declarations::items::starts_machine_declaration(rest);
        statements.extend_contiguous(parsed);
        input = rest;
        if completes_body {
            break;
        }
    }
    Ok((statements, input))
}

/// The block value of a dispatch arm, `{ let port: u32 = 1; port }`, from
/// its `{` through its `}`. Each statement uses the ordinary body grammar,
/// and an expression the `}` closes is the block's value, as it is a
/// machine body's. A block with no final expression has the Unit value.
///
/// A block holds bindings, assignments and calls. A `transition`, `crash`
/// or `asm` statement inside one would complete or leave the enclosing
/// machine from the middle of an arm, which no later stage lowers, so it is
/// refused here at the statement.
pub(crate) fn parse_block_value<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let start = input;
    let depth = input.depth();
    let mut input = input
        .take_punctuation(PunctuationKind::LeftBrace, "{")?
        .deepen()?;
    let frontier = syntax_trees.items.statement_handle_frontier();
    let mut tail = ExpressionHandle::invalid();
    let mut tail_statement = None;
    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let statement_start = input;
        let (parsed, rest) = parse_statement_handles(syntax_trees, input)?;
        for &statement in syntax_trees.items.statements(parsed) {
            match syntax_trees.statements.statement(statement) {
                // The ordinary grammar reads an expression the `}` closes as
                // a bare-value transition; inside a block it is the value.
                StatementNode::Transition(transition)
                    if rest.at_punctuation(PunctuationKind::RightBrace)
                        && transition.patterns.is_empty()
                        && transition.guard == TransitionGuardNode::Always
                        && transition.exit == TransitionExit::Ordinary
                        && matches!(
                            syntax_trees.statements.transition_target(transition.target),
                            TransitionTargetNode::Value(_)
                        ) =>
                {
                    let TransitionTargetNode::Value(value) =
                        syntax_trees.statements.transition_target(transition.target)
                    else {
                        unreachable!("the guard matched a value target");
                    };
                    tail = *value;
                    tail_statement = Some(statement);
                }
                StatementNode::Transition(_)
                | StatementNode::Dispatch(_)
                | StatementNode::Assembly(_)
                | StatementNode::AuthoredAssembly(_)
                | StatementNode::AssemblyFact(_)
                | StatementNode::RootBinding(_)
                | StatementNode::ProofOutputBindingStatement(_) => {
                    return Err(statement_start.error_here(
                        syntax_trees::root_bindings::BLOCK_VALUE_STATEMENT_ERROR,
                    ));
                }
                StatementNode::Assignment(_)
                | StatementNode::Call(_)
                | StatementNode::Expression(_)
                | StatementNode::LocalData(_) => {}
            }
        }
        input = rest;
    }
    let input = input
        .take_punctuation(PunctuationKind::RightBrace, "}")?
        .with_depth(depth);
    let statements: Vec<StatementHandle> = syntax_trees
        .items
        .detach_statement_handles_since(frontier)
        .into_iter()
        .filter(|statement| Some(*statement) != tail_statement)
        .collect();
    let statements = syntax_trees.statements.insert_block_statements(statements);
    let block = syntax_trees
        .expressions
        .insert(ExpressionNode::Block(TableBlockExpression {
            statements,
            tail,
        }));
    syntax_trees
        .expressions
        .set_source_span(block, start.source_span_until(input));
    Ok((block, input))
}
