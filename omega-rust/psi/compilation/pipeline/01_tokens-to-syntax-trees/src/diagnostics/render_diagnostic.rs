use crate::diagnostics::parse_error::ParseError;
use crate::input::token_cursor::Input;
use tokens::{CommentKind, Token, TokenKind};

/// One `expected …, found …` parse diagnostic: the candidate set renders once,
/// then names the token the cursor holds (or EOF). Every reject path in the
/// parser builds its message here, so the shape and the span it points at
/// cannot drift between the keyword, punctuation and literal expectations.
fn expected_at(input: Input<'_, '_>, token: Option<&Token>, expected: &[&str]) -> ParseError {
    let expected = match expected {
        [] => "unexpected token".to_owned(),
        [only] => format!("expected {only}"),
        _ => format!("expected one of {}", expected.join(", ")),
    };

    match token {
        Some(token) => ParseError::at_source_span(
            format!("{expected}, found {}", describe_token(input, token)),
            input.source_span(token),
        ),
        None => ParseError::at_source_span(format!("{expected}, found EOF"), Default::default()),
    }
}

pub(crate) fn unexpected_eof(input: Input<'_, '_>, expected: impl Into<String>) -> ParseError {
    let expected = expected.into();
    expected_at(input, None, &[expected.as_str()])
}

pub(crate) fn expected(
    input: Input<'_, '_>,
    token: &Token,
    expected: impl Into<String>,
) -> ParseError {
    let expected = expected.into();
    expected_at(input, Some(token), &[expected.as_str()])
}

pub(crate) fn expected_one_of_here(input: Input<'_, '_>, expected: &[&str]) -> ParseError {
    expected_at(input, input.tokens.first(), expected)
}

fn describe_token(input: Input<'_, '_>, token: &Token) -> String {
    match token.kind {
        TokenKind::Identifier => format!("identifier `{}`", input.text(token).as_str()),
        TokenKind::NumericLiteral(_) => format!("numeric literal `{}`", input.text(token).as_str()),
        TokenKind::StringLiteral => "string literal".to_owned(),
        TokenKind::Keyword(keyword) => format!("keyword `{}`", keyword.lexeme()),
        TokenKind::Punctuation(punctuation) => {
            format!("punctuation `{}`", punctuation.lexeme())
        }
        TokenKind::Whitespace => "whitespace".to_owned(),
        TokenKind::Comment(CommentKind::Line) => "line comment".to_owned(),
        TokenKind::Comment(CommentKind::Block) => "block comment".to_owned(),
    }
}
