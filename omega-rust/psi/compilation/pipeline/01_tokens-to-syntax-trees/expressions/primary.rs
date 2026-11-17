use crate::syntax_storage::tables::exprs;
use crate::token_input::cursor::Input;
use crate::types::references::parse_type_reference_handle;
use source::SourceSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::types::TypeReferenceHandle;
use tokens::{PunctuationKind, TokenKind};

pub(crate) fn try_uninvoked_application<'tokens, 'source>(
    syntax: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    allows_type_expression: bool,
) -> Option<(ExpressionNode, Input<'tokens, 'source>)> {
    let (_, after_name) = input.expect_token().ok()?;
    if !after_name.at_punctuation(PunctuationKind::Less)
        && !after_name.at_punctuation(PunctuationKind::ColonColon)
    {
        return None;
    }
    let checkpoint = syntax.append_checkpoint();
    let accepted = (|| {
        let (selection, rest) =
            crate::expressions::postfix::parse_static_name_selection(syntax, input).ok()??;
        let application = selection.application?;
        if allows_type_expression && !application.member.is_valid() {
            return None;
        }
        if ![
            PunctuationKind::Comma,
            PunctuationKind::RightParen,
            PunctuationKind::RightBracket,
            PunctuationKind::Semicolon,
            PunctuationKind::Greater,
        ]
        .into_iter()
        .any(|delimiter| rest.at_punctuation(delimiter))
        {
            return None;
        }
        let mut members = arena::HandleSpan::empty();
        for member in selection.path {
            exprs(syntax).append_identifier_path_member_to_span(&mut members, member);
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
    let expression = exprs(syntax_trees).insert(node);
    exprs(syntax_trees).set_source_span(expression, source_span);
    expression
}

pub(crate) fn try_applied_requirement_head<'tokens, 'source>(
    syntax: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(TypeReferenceHandle, Input<'tokens, 'source>)> {
    let (_, after_name) = input.expect_token().ok()?;
    if !after_name.at_punctuation(PunctuationKind::Less)
        && !after_name.at_punctuation(PunctuationKind::ColonColon)
    {
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
        let (name, rest) = crate::types::references::parse_static_name(input).ok()?;
        let (reference, rest) =
            crate::types::references::parse_named_type_reference(syntax, name, rest, false).ok()?;
        if !matches!(
            syntax.type_references.type_reference(reference),
            syntax_trees::types::TypeReferenceNode::Generic { .. }
        ) {
            return None;
        }
        let after_separator = rest
            .take_punctuation(PunctuationKind::ColonColon, "::")
            .ok()?;
        let (_, after_member) = after_separator.take_identifier().ok()?;
        (after_member.at_punctuation(PunctuationKind::LeftParen)
            || after_member.at_punctuation(PunctuationKind::Less))
        .then_some((reference, rest))
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
        (accepted && (allow_body_delimiter || !rest.at_punctuation(PunctuationKind::LeftBrace)))
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
    input.at_punctuation(PunctuationKind::Ampersand)
        && input
            .take_punctuation(PunctuationKind::Ampersand, "&")
            .is_ok_and(|rest| rest.at_punctuation(PunctuationKind::Apostrophe))
}
