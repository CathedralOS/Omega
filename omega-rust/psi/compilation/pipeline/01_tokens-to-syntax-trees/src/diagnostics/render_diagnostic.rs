use crate::diagnostics::parse_error::ParseError;
use crate::input::token_cursor::Input;
use tokens::{CommentKind, Token, TokenKind};

pub(crate) fn unexpected_eof<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    expected: impl Into<String>,
) -> ParseError {
    ParseError::at_source_span(
        format!("expected {}, found EOF", expected.into()),
        input
            .tokens
            .first()
            .map(|token| input.source_span(token))
            .unwrap_or_default(),
    )
}

pub(crate) fn expected<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    token: &Token,
    expected: impl Into<String>,
) -> ParseError {
    ParseError::at_source_span(
        format!(
            "expected {}, found {}",
            expected.into(),
            describe_token(input, token)
        ),
        input.source_span(token),
    )
}

pub(crate) fn expected_one_of_here<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    expected: &[&str],
) -> ParseError {
    let rendered = match expected {
        [] => "unexpected token".to_owned(),
        [only] => format!("expected {only}"),
        _ => format!("expected one of {}", expected.join(", ")),
    };

    match input.tokens.first() {
        Some(token) => ParseError::at_source_span(
            format!("{rendered}, found {}", describe_token(input, token)),
            input.source_span(token),
        ),
        None => ParseError::at_source_span(format!("{rendered}, found EOF"), Default::default()),
    }
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
