use std::iter::Peekable;
use std::str::CharIndices;

use crate::LexError;
use omega_core::Span;
use omega_tokens::{
    CommentKind, KeywordKind, NumericLiteralKind, PunctuationKind, Token, TokenKind,
    TokenStream, TokenText, WhitespaceKind,
};

pub struct Lexer<'source> {
    source: &'source str,
    chars: Peekable<CharIndices<'source>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LexedToken {
    kind: TokenKind,
    span: Span,
}

impl<'source> Lexer<'source> {
    pub fn new(source: &'source str) -> Self {
        Self {
            source,
            chars: source.char_indices().peekable(),
        }
    }

    pub fn tokenize(mut self) -> Result<TokenStream<'source>, LexError> {
        let mut tokens = Vec::new();

        while let Some(token) = self.lex_next_token()? {
            tokens.push(self.build_token(token));
        }

        Ok(TokenStream::new(tokens))
    }

    fn lex_next_token(&mut self) -> Result<Option<LexedToken>, LexError> {
        let Some((start, character)) = self.chars.next() else {
            return Ok(None);
        };

        let token = if character.is_whitespace() {
            self.lex_whitespace(start, character)
        } else if character == '/' && self.peek_character() == Some('/') {
            self.lex_line_comment(start)
        } else if character == '/' && self.peek_character() == Some('*') {
            self.lex_block_comment(start)?
        } else if character.is_ascii_alphabetic() || character == '_' {
            self.lex_identifier_or_keyword(start, character)
        } else if character.is_ascii_digit() {
            self.lex_number(start, character)
        } else if character == '"' {
            self.lex_string_token(start)?
        } else {
            self.lex_punctuation(start, character)?
        };

        Ok(Some(token))
    }

    fn build_token(&self, token: LexedToken) -> Token<'source> {
        let lexeme = match token.kind {
            TokenKind::StringLiteral => {
                let raw = &self.source[token.span.start..token.span.end];
                let value = self
                    .decode_string_literal(raw)
                    .expect("string literal token should already be validated");
                TokenText::owned(value)
            }
            _ => TokenText::source(&self.source[token.span.start..token.span.end]),
        };

        Token {
            kind: token.kind,
            lexeme,
            span: token.span,
        }
    }

    fn lex_whitespace(&mut self, start: usize, first: char) -> LexedToken {
        let mut end = start + first.len_utf8();
        let mut saw_newline = first == '\n' || first == '\r';
        let mut saw_other = !saw_newline;

        while let Some((next_index, next)) = self.chars.peek().copied() {
            if !next.is_whitespace() {
                break;
            }

            saw_newline |= next == '\n' || next == '\r';
            saw_other |= next != '\n' && next != '\r';
            end = next_index + next.len_utf8();
            self.chars.next();
        }

        let kind = match (saw_newline, saw_other) {
            (true, false) => WhitespaceKind::Newline,
            (false, true) => WhitespaceKind::Space,
            _ => WhitespaceKind::Mixed,
        };

        LexedToken {
            kind: TokenKind::Whitespace(kind),
            span: Span::new(start, end),
        }
    }

    fn lex_line_comment(&mut self, start: usize) -> LexedToken {
        let mut end = start + '/'.len_utf8();
        self.chars.next();
        end += '/'.len_utf8();

        for (_, character) in self.chars.by_ref() {
            if character == '\n' {
                break;
            }
            end += character.len_utf8();
        }

        LexedToken {
            kind: TokenKind::Comment(CommentKind::Line),
            span: Span::new(start, end),
        }
    }

    fn lex_block_comment(&mut self, start: usize) -> Result<LexedToken, LexError> {
        self.chars.next();
        let mut depth = 1usize;

        while let Some((index, character)) = self.chars.next() {
            match (character, self.peek_character()) {
                ('/', Some('*')) => {
                    self.chars.next();
                    depth += 1;
                }
                ('*', Some('/')) => {
                    self.chars.next();
                    depth -= 1;
                    if depth == 0 {
                        return Ok(LexedToken {
                            kind: TokenKind::Comment(CommentKind::Block),
                            span: Span::new(start, index + 2),
                        });
                    }
                }
                _ => {}
            }
        }

        Err(LexError::new(
            "unterminated block comment",
            Span::new(start, self.source.len()),
        ))
    }

    fn lex_identifier_or_keyword(&mut self, start: usize, first: char) -> LexedToken {
        let mut end = start + first.len_utf8();

        while let Some((next_index, next)) = self.chars.peek().copied() {
            if next.is_ascii_alphanumeric() || next == '_' {
                end = next_index + next.len_utf8();
                self.chars.next();
            } else {
                break;
            }
        }

        let lexeme = &self.source[start..end];
        let kind = KeywordKind::from_lexeme(lexeme)
            .map(TokenKind::Keyword)
            .unwrap_or(TokenKind::Identifier);

        LexedToken {
            kind,
            span: Span::new(start, end),
        }
    }

    fn lex_number(&mut self, start: usize, first: char) -> LexedToken {
        let mut end = start + first.len_utf8();
        let mut is_float = false;

        while let Some((next_index, next)) = self.chars.peek().copied() {
            if next.is_ascii_digit() {
                end = next_index + next.len_utf8();
                self.chars.next();
            } else {
                break;
            }
        }

        if self.peek_character() == Some('.') {
            let mut clone = self.chars.clone();
            clone.next();

            if matches!(clone.peek(), Some((_, character)) if character.is_ascii_digit()) {
                is_float = true;

                if let Some((dot_index, dot)) = self.chars.next() {
                    end = dot_index + dot.len_utf8();
                }

                while let Some((next_index, next)) = self.chars.peek().copied() {
                    if next.is_ascii_digit() {
                        end = next_index + next.len_utf8();
                        self.chars.next();
                    } else {
                        break;
                    }
                }

                if self.peek_character() == Some('f') {
                    if let Some((suffix_index, suffix)) = self.chars.next() {
                        end = suffix_index + suffix.len_utf8();
                    }
                }
            }
        }

        LexedToken {
            kind: if is_float {
                TokenKind::NumericLiteral(NumericLiteralKind::Float)
            } else {
                TokenKind::NumericLiteral(NumericLiteralKind::Integer)
            },
            span: Span::new(start, end),
        }
    }

    fn lex_string_token(&mut self, start: usize) -> Result<LexedToken, LexError> {
        let end = self.lex_string_end(start)?;
        self.decode_string_literal(&self.source[start..end])?;
        Ok(LexedToken {
            kind: TokenKind::StringLiteral,
            span: Span::new(start, end),
        })
    }

    fn lex_string_end(&mut self, start: usize) -> Result<usize, LexError> {
        while let Some((index, character)) = self.chars.next() {
            if character == '"' {
                return Ok(index + character.len_utf8());
            }

            if character == '\\' {
                let Some((_, escaped)) = self.chars.next() else {
                    return Err(LexError::new(
                        "unterminated string escape",
                        Span::new(start, self.source.len()),
                    ));
                };

                match escaped {
                    '"' | '\\' | 'n' | 't' => {}
                    other => {
                        return Err(LexError::new(
                            format!("unsupported escape sequence `\\{other}`"),
                            Span::new(index, index + other.len_utf8()),
                        ));
                    }
                }
            }
        }

        Err(LexError::new(
            "unterminated string literal",
            Span::new(start, self.source.len()),
        ))
    }

    fn decode_string_literal(&self, raw: &str) -> Result<String, LexError> {
        let mut lexeme = String::new();
        let mut chars = raw.char_indices();

        let Some((_, opening_quote)) = chars.next() else {
            return Ok(lexeme);
        };
        debug_assert_eq!(opening_quote, '"');

        while let Some((index, character)) = chars.next() {
            if character == '"' {
                return Ok(lexeme);
            }

            if character == '\\' {
                let Some((_, escaped)) = chars.next() else {
                    return Err(LexError::new(
                        "unterminated string escape",
                        Span::new(0, raw.len()),
                    ));
                };

                match escaped {
                    '"' => lexeme.push('"'),
                    '\\' => lexeme.push('\\'),
                    'n' => lexeme.push('\n'),
                    't' => lexeme.push('\t'),
                    other => {
                        return Err(LexError::new(
                            format!("unsupported escape sequence `\\{other}`"),
                            Span::new(index, index + other.len_utf8()),
                        ));
                    }
                }
            } else {
                lexeme.push(character);
            }
        }

        Err(LexError::new(
            "unterminated string literal",
            Span::new(0, raw.len()),
        ))
    }

    fn lex_punctuation(&mut self, start: usize, first: char) -> Result<LexedToken, LexError> {
        let mut end = start + first.len_utf8();

        if matches!(
            (first, self.peek_character()),
            (':', Some(':'))
                | ('-', Some('>'))
                | ('=', Some('='))
                | ('!', Some('='))
                | ('<', Some('<'))
                | ('<', Some('='))
                | ('>', Some('>'))
                | ('>', Some('='))
                | ('&', Some('&'))
                | ('|', Some('|'))
        ) {
            if let Some((next_index, next)) = self.chars.next() {
                end = next_index + next.len_utf8();
            }
        }

        let lexeme = &self.source[start..end];
        let Some(kind) = PunctuationKind::from_lexeme(lexeme).map(TokenKind::Punctuation) else {
            return Err(LexError::new(
                format!("unsupported punctuation `{lexeme}`"),
                Span::new(start, end),
            ));
        };

        Ok(LexedToken {
            kind,
            span: Span::new(start, end),
        })
    }

    fn peek_character(&mut self) -> Option<char> {
        self.chars.peek().map(|(_, character)| *character)
    }

}

#[cfg(test)]
mod tests {
    use super::Lexer;
    use omega_tokens::{
        CommentKind, KeywordKind, NumericLiteralKind, PunctuationKind, TokenKind,
    };

    fn semantic_kinds(source: &str) -> Vec<TokenKind> {
        Lexer::new(source)
            .tokenize()
            .expect("tokenization should succeed")
            .iter()
            .filter(|token| !token.is_non_semantic())
            .map(|token| token.kind)
            .collect()
    }

    #[test]
    fn tokenizes_keywords_and_identifiers_distinctly() {
        assert_eq!(
            semantic_kinds("machine game entry self true false custom"),
            vec![
                TokenKind::Keyword(KeywordKind::Machine),
                TokenKind::Identifier,
                TokenKind::Keyword(KeywordKind::Entry),
                TokenKind::Keyword(KeywordKind::SelfValue),
                TokenKind::Keyword(KeywordKind::True),
                TokenKind::Keyword(KeywordKind::False),
                TokenKind::Identifier,
            ]
        );
    }

    #[test]
    fn tokenizes_multi_character_punctuation() {
        assert_eq!(
            semantic_kinds(":: -> == != << <= >> >= && ||"),
            vec![
                TokenKind::Punctuation(PunctuationKind::ColonColon),
                TokenKind::Punctuation(PunctuationKind::Arrow),
                TokenKind::Punctuation(PunctuationKind::EqualEqual),
                TokenKind::Punctuation(PunctuationKind::ExclamationEqual),
                TokenKind::Punctuation(PunctuationKind::LessLess),
                TokenKind::Punctuation(PunctuationKind::LessEqual),
                TokenKind::Punctuation(PunctuationKind::GreaterGreater),
                TokenKind::Punctuation(PunctuationKind::GreaterEqual),
                TokenKind::Punctuation(PunctuationKind::AndAnd),
                TokenKind::Punctuation(PunctuationKind::PipePipe),
            ]
        );
    }

    #[test]
    fn skips_line_comments_and_whitespace() {
        let tokens = Lexer::new("let // comment\n value")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].kind, TokenKind::Keyword(KeywordKind::Let));
        assert!(matches!(tokens[1].kind, TokenKind::Whitespace(_)));
        assert!(matches!(tokens[2].kind, TokenKind::Comment(CommentKind::Line)));
        assert!(matches!(tokens[3].kind, TokenKind::Whitespace(_)));
        assert_eq!(tokens[4].kind, TokenKind::Identifier);
        assert_eq!(tokens[4].lexeme.as_str(), "value");
    }

    #[test]
    fn skips_block_comments_and_whitespace() {
        let tokens = Lexer::new("let /* comment */ value")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].kind, TokenKind::Keyword(KeywordKind::Let));
        assert!(matches!(tokens[1].kind, TokenKind::Whitespace(_)));
        assert!(matches!(tokens[2].kind, TokenKind::Comment(CommentKind::Block)));
        assert!(matches!(tokens[3].kind, TokenKind::Whitespace(_)));
        assert_eq!(tokens[4].kind, TokenKind::Identifier);
        assert_eq!(tokens[4].lexeme.as_str(), "value");
    }

    #[test]
    fn skips_nested_block_comments() {
        let tokens = Lexer::new("let /* outer /* inner */ still outer */ value")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].kind, TokenKind::Keyword(KeywordKind::Let));
        assert!(matches!(tokens[1].kind, TokenKind::Whitespace(_)));
        assert!(matches!(tokens[2].kind, TokenKind::Comment(CommentKind::Block)));
        assert!(matches!(tokens[3].kind, TokenKind::Whitespace(_)));
        assert_eq!(tokens[4].kind, TokenKind::Identifier);
        assert_eq!(tokens[4].lexeme.as_str(), "value");
    }

    #[test]
    fn errors_on_unterminated_block_comment() {
        let error = Lexer::new("let /* comment")
            .tokenize()
            .expect_err("tokenization should fail");

        assert_eq!(error.message, "unterminated block comment");
    }

    #[test]
    fn tokenizes_string_literals_with_escapes() {
        let tokens = Lexer::new("\"line\\nvalue\"")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
        assert_eq!(tokens[0].lexeme.as_str(), "line\nvalue");
    }

    #[test]
    fn tokenizes_integer_and_float_literals() {
        let tokens = Lexer::new("42 3.14f")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(tokens.len(), 3);
        assert_eq!(
            tokens[0].kind,
            TokenKind::NumericLiteral(NumericLiteralKind::Integer)
        );
        assert!(matches!(tokens[1].kind, TokenKind::Whitespace(_)));
        assert_eq!(
            tokens[2].kind,
            TokenKind::NumericLiteral(NumericLiteralKind::Float)
        );
    }
}
