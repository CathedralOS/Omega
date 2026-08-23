use crate::parse_error::ParseError;
use crate::parser::input::Input;
use psi_tokens::{CommentKind, KeywordKind, PunctuationKind, Token, TokenKind};

pub(super) fn unexpected_eof<'tokens, 'source>(
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

pub(super) fn expected<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    token: &Token<'source>,
    expected: impl Into<String>,
) -> ParseError {
    ParseError::at_source_span(
        format!(
            "expected {}, found {}",
            expected.into(),
            describe_token(token)
        ),
        input.source_span(token),
    )
}

pub(super) fn expected_one_of_here<'tokens, 'source>(
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
            format!("{rendered}, found {}", describe_token(token)),
            input.source_span(token),
        ),
        None => ParseError::at_source_span(format!("{rendered}, found EOF"), Default::default()),
    }
}

fn describe_token(token: &Token<'_>) -> String {
    match token.kind {
        TokenKind::Identifier => format!("identifier `{}`", token.lexeme.as_str()),
        TokenKind::NumericLiteral(_) => format!("numeric literal `{}`", token.lexeme.as_str()),
        TokenKind::StringLiteral => "string literal".to_owned(),
        TokenKind::Keyword(keyword) => format!("keyword `{}`", render_keyword(keyword)),
        TokenKind::Punctuation(punctuation) => {
            format!("punctuation `{}`", render_punctuation(punctuation))
        }
        TokenKind::Whitespace => "whitespace".to_owned(),
        TokenKind::Comment(CommentKind::Line) => "line comment".to_owned(),
        TokenKind::Comment(CommentKind::Block) => "block comment".to_owned(),
    }
}

fn render_keyword(keyword: KeywordKind) -> &'static str {
    match keyword {
        KeywordKind::As => "as",
        KeywordKind::Capability => "capability",
        KeywordKind::Data => "data",
        KeywordKind::CallingConvention => "calling_convention",
        KeywordKind::Contains => "contains",
        KeywordKind::Else => "else",
        KeywordKind::Enum => "enum",
        KeywordKind::Entry => "entry",
        KeywordKind::False => "false",
        KeywordKind::Foreign => "foreign",
        KeywordKind::Host => "host",
        KeywordKind::If => "if",
        KeywordKind::Invariant => "invariant",
        KeywordKind::Let => "let",
        KeywordKind::Library => "library",
        KeywordKind::Loop => "loop",
        KeywordKind::Machine => "machine",
        KeywordKind::Match => "match",
        KeywordKind::Owns => "owns",
        KeywordKind::Platform => "platform",
        KeywordKind::Pub => "pub",
        KeywordKind::Return => "return",
        KeywordKind::SelfType => "Self",
        KeywordKind::SelfValue => "self",
        KeywordKind::State => "state",
        KeywordKind::Struct => "struct",
        KeywordKind::Target => "target",
        KeywordKind::Transition => "transition",
        KeywordKind::True => "true",
        KeywordKind::Use => "use",
        KeywordKind::When => "when",
        KeywordKind::While => "while",
        KeywordKind::Unknown => "<unknown>",
    }
}

fn render_punctuation(punctuation: PunctuationKind) -> &'static str {
    match punctuation {
        PunctuationKind::Ampersand => "&",
        PunctuationKind::AndAnd => "&&",
        PunctuationKind::Apostrophe => "'",
        PunctuationKind::Arrow => "->",
        PunctuationKind::Asterisk => "*",
        PunctuationKind::Caret => "^",
        PunctuationKind::Colon => ":",
        PunctuationKind::ColonColon => "::",
        PunctuationKind::Comma => ",",
        PunctuationKind::Dot => ".",
        PunctuationKind::DotDot => "..",
        PunctuationKind::DotDotEqual => "..=",
        PunctuationKind::Equal => "=",
        PunctuationKind::EqualEqual => "==",
        PunctuationKind::Exclamation => "!",
        PunctuationKind::ExclamationEqual => "!=",
        PunctuationKind::Greater => ">",
        PunctuationKind::GreaterEqual => ">=",
        PunctuationKind::GreaterGreater => ">>",
        PunctuationKind::Hash => "#",
        PunctuationKind::LeftBrace => "{",
        PunctuationKind::LeftBracket => "[",
        PunctuationKind::LeftParen => "(",
        PunctuationKind::Less => "<",
        PunctuationKind::LessEqual => "<=",
        PunctuationKind::LessLess => "<<",
        PunctuationKind::Minus => "-",
        PunctuationKind::Percent => "%",
        PunctuationKind::Pipe => "|",
        PunctuationKind::PipePipe => "||",
        PunctuationKind::Plus => "+",
        PunctuationKind::PlusEqual => "+=",
        PunctuationKind::MinusEqual => "-=",
        PunctuationKind::AsteriskEqual => "*=",
        PunctuationKind::SlashEqual => "/=",
        PunctuationKind::PercentEqual => "%=",
        PunctuationKind::RightBrace => "}",
        PunctuationKind::RightBracket => "]",
        PunctuationKind::RightParen => ")",
        PunctuationKind::Semicolon => ";",
        PunctuationKind::Slash => "/",
        PunctuationKind::Tilde => "~",
        PunctuationKind::Unknown => "<unknown>",
    }
}
