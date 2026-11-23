use crate::expressions::postfix::{SelectionPosition, parse_name_selection};
use crate::token_input::cursor::Input;
use crate::types::references::parse_type_reference_handle;
use source_text::SourceSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::types::TypeReferenceHandle;
use tokens::{PunctuationKind, TokenKind};

pub(crate) fn try_uninvoked_application<'tokens, 'source>(
    syntax: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(ExpressionNode, Input<'tokens, 'source>)> {
    let (_, after_name) = input.take_path_head().ok()?;
    if !after_name.at_path_member() && !after_name.at_expression_arguments() {
        return None;
    }
    let checkpoint = syntax.append_checkpoint();
    let accepted = (|| {
        let (selection, rest) =
            parse_name_selection(syntax, input, SelectionPosition::Expression).ok()??;
        let application = selection.application?;
        if rest.at_punctuation(PunctuationKind::LeftParen) {
            return None;
        }
        let mut members = arena::HandleSpan::empty();
        for member in selection.path {
            syntax
                .expressions
                .append_identifier_path_member_to_span(&mut members, member);
        }
        Some((
            ExpressionNode::Name {
                members,
                application: Some(application),
            },
            rest,
        ))
    })();
    if accepted.is_none() {
        syntax.rewind_appended(checkpoint);
    }
    accepted
}

pub(crate) fn spanned(
    syntax_trees: &mut SyntaxTrees,
    node: ExpressionNode,
    source_span: SourceSpan,
) -> ExpressionHandle {
    let expression = syntax_trees.expressions.insert(node);
    syntax_trees
        .expressions
        .set_source_span(expression, source_span);
    expression
}

pub(crate) fn try_applied_requirement_head<'tokens, 'source>(
    syntax: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(TypeReferenceHandle, Input<'tokens, 'source>)> {
    let (_, after_name) = input.take_path_head().ok()?;
    if !after_name.at_path_member() && !after_name.at_expression_arguments() {
        return None;
    }
    probe_applied_requirement_head(syntax, input)
}

#[inline(never)]
fn probe_applied_requirement_head<'tokens, 'source>(
    syntax: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(TypeReferenceHandle, Input<'tokens, 'source>)> {
    let checkpoint = syntax.append_checkpoint();
    let accepted = (|| {
        // This expression owns the applied receiver head; the call parser
        // consumes its member. A full type can itself include that member.
        let receiver_checkpoint = syntax.append_checkpoint();
        let (name, rest) = crate::types::references::parse_static_name(input).ok()?;
        if !rest.at_expression_arguments() {
            return None;
        }
        let arguments_start = rest
            .take_punctuation(PunctuationKind::ColonColon, "::")
            .ok()?;
        // An applied head is followed by `::member`. Find the list's end
        // before parsing it, so a list that is not a head is not read twice.
        let after_arguments = arguments_start.after_static_argument_list()?;
        if !after_arguments.at_punctuation(PunctuationKind::ColonColon)
            || after_arguments.at_expression_arguments()
        {
            return None;
        }
        let (reference, rest) = crate::types::references::parse_named_type_reference(
            syntax,
            name,
            arguments_start,
            false,
        )
        .ok()?;
        if !matches!(
            syntax.type_references.type_reference(reference),
            syntax_trees::types::TypeReferenceNode::Generic { .. }
        ) {
            return None;
        }
        let after_head = rest;
        let mut separator = rest;
        loop {
            let next = separator
                .take_punctuation(PunctuationKind::ColonColon, "::")
                .ok()?;
            let (_, after_member) = next.take_identifier().ok()?;
            let is_call = if after_member.at_expression_arguments() {
                let argument_checkpoint = syntax.append_checkpoint();
                let parsed = crate::expressions::postfix::try_parse_static_machine_arguments(
                    syntax,
                    after_member,
                );
                syntax.rewind_appended(argument_checkpoint);
                parsed.ok()?.is_some()
            } else {
                after_member.at_punctuation(PunctuationKind::LeftParen)
            };
            if is_call {
                if separator.tokens.len() == after_head.tokens.len() {
                    return Some((reference, separator));
                }
                let mut receiver = input;
                receiver.tokens = &input.tokens[..input.tokens.len() - separator.tokens.len()];
                syntax.rewind_appended(receiver_checkpoint);
                let (selection, end) =
                    parse_name_selection(syntax, receiver, SelectionPosition::Expression)
                        .ok()??;
                if !end.tokens.is_empty() {
                    return None;
                }
                let reference = syntax.type_references.insert(
                    syntax_trees::types::TypeReferenceNode::StaticSelection(selection),
                );
                return Some((reference, separator));
            }
            separator = if after_member.at_expression_arguments() {
                let args = after_member
                    .take_punctuation(PunctuationKind::ColonColon, "::")
                    .ok()?;
                crate::expressions::postfix::try_parse_static_symbol_application(syntax, args)
                    .ok()??
                    .1
            } else {
                after_member
            };
        }
    })();
    if accepted.is_none() {
        syntax.rewind_appended(checkpoint);
    }
    accepted
}

pub(crate) fn try_generic_type_operand<'tokens, 'source>(
    syntax: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    allow_body_delimiter: bool,
    is_equality_operand: impl FnOnce(Input<'tokens, 'source>) -> bool,
) -> Option<(TypeReferenceHandle, Input<'tokens, 'source>)> {
    let checkpoint = syntax.append_checkpoint();
    let accepted = (|| {
        let (reference, rest) = parse_type_reference_handle(syntax, input).ok()?;
        if !matches!(
            syntax.type_references.type_reference(reference),
            syntax_trees::types::TypeReferenceNode::Generic { .. }
        ) {
            return None;
        }
        let accepted = rest.tokens.is_empty()
            || [
                PunctuationKind::Arrow,
                PunctuationKind::EqualEqual,
                PunctuationKind::ExclamationEqual,
                PunctuationKind::PipePipe,
                PunctuationKind::AndAnd,
                PunctuationKind::Comma,
                PunctuationKind::Semicolon,
                PunctuationKind::LeftBrace,
                PunctuationKind::RightBrace,
                PunctuationKind::RightParen,
            ]
            .into_iter()
            .any(|punctuation| rest.at_punctuation(punctuation))
            || [
                "requires",
                "ensures",
                "terminates",
                "decreases",
                "reaches",
                "effects",
                "invokes",
                "suspends",
                "blocks",
                "crashes",
                "boundary",
                "satisfies",
            ]
            .into_iter()
            .any(|keyword| rest.at_contextual(keyword));
        (accepted
            && is_equality_operand(rest)
            && (allow_body_delimiter || !rest.at_punctuation(PunctuationKind::LeftBrace)))
        .then_some((reference, rest))
    })();
    if accepted.is_none() {
        syntax.rewind_appended(checkpoint);
    }
    accepted
}

pub(crate) fn starts_fixed_array_type(input: Input<'_, '_>) -> bool {
    let mut bracket_depth = 0usize;
    let mut parenthesis_depth = 0usize;
    let mut brace_depth = 0usize;
    for token in input.tokens {
        match token.kind {
            TokenKind::Punctuation(PunctuationKind::LeftBracket) => bracket_depth += 1,
            TokenKind::Punctuation(PunctuationKind::LeftParen) => parenthesis_depth += 1,
            TokenKind::Punctuation(PunctuationKind::RightParen) => {
                let Some(depth) = parenthesis_depth.checked_sub(1) else {
                    return false;
                };
                parenthesis_depth = depth;
            }
            TokenKind::Punctuation(PunctuationKind::LeftBrace) => brace_depth += 1,
            TokenKind::Punctuation(PunctuationKind::RightBrace) => {
                let Some(depth) = brace_depth.checked_sub(1) else {
                    return false;
                };
                brace_depth = depth;
            }
            TokenKind::Punctuation(PunctuationKind::RightBracket) => {
                if bracket_depth <= 1 {
                    return false;
                }
                bracket_depth -= 1;
            }
            TokenKind::Punctuation(PunctuationKind::Semicolon)
                if bracket_depth == 1 && parenthesis_depth == 0 && brace_depth == 0 =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

pub(crate) fn starts_lifetime_reference_type(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::At)
        && input
            .take_punctuation(PunctuationKind::At, "@")
            .is_ok_and(|rest| rest.at_punctuation(PunctuationKind::Apostrophe))
}
