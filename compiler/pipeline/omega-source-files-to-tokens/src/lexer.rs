use std::iter::Peekable;
use std::str::CharIndices;

use crate::LexError;
use omega_core::Span;
use omega_tokens::{
    KeywordKind, PunctuationKind, Token, TokenKind, TokenStream, TokenText,
};

pub struct Lexer<'source> {
    source: &'source str,
    chars: Peekable<CharIndices<'source>>,
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
        let source = self.source;

        while let Some((start, character)) = self.chars.next() {
            if character.is_whitespace() {
                continue;
            }

            if character == '/' && self.peek_character() == Some('/') {
                self.skip_line_comment();
                continue;
            }

            if character == '/' && self.peek_character() == Some('*') {
                self.skip_block_comment(start)?;
                continue;
            }

            if character.is_ascii_alphabetic() || character == '_' {
                let end = self.lex_identifier_end(start, character);
                let lexeme = &source[start..end];
                let kind = KeywordKind::from_lexeme(lexeme)
                    .map(TokenKind::Keyword)
                    .unwrap_or(TokenKind::Identifier);
                tokens.push(Token {
                    kind,
                    lexeme: TokenText::source(lexeme),
                    span: Span::new(start, end),
                });
                continue;
            }

            if character.is_ascii_digit() {
                let (kind, end) = self.lex_number(start, character);
                tokens.push(Token {
                    kind,
                    lexeme: TokenText::source(&source[start..end]),
                    span: Span::new(start, end),
                });
                continue;
            }

            if character == '"' {
                let (lexeme, span) = self.lex_string(start)?;
                tokens.push(Token {
                    kind: TokenKind::StringLiteral,
                    lexeme: TokenText::owned(lexeme),
                    span,
                });
                continue;
            }

            let end = self.lex_symbol_end(start, character);
            let lexeme = &source[start..end];
            let Some(kind) = PunctuationKind::from_lexeme(lexeme).map(TokenKind::Punctuation) else {
                return Err(LexError::new(
                    format!("unsupported punctuation `{lexeme}`"),
                    Span::new(start, end),
                ));
            };
            tokens.push(Token {
                kind,
                lexeme: TokenText::source(lexeme),
                span: Span::new(start, end),
            });
        }

        Ok(TokenStream::new(tokens))
    }

    fn skip_line_comment(&mut self) {
        for (_, character) in self.chars.by_ref() {
            if character == '\n' {
                break;
            }
        }
    }

    fn skip_block_comment(&mut self, start: usize) -> Result<(), LexError> {
        self.chars.next();
        let mut depth = 1usize;

        while let Some((_, character)) = self.chars.next() {
            match (character, self.peek_character()) {
                ('/', Some('*')) => {
                    self.chars.next();
                    depth += 1;
                }
                ('*', Some('/')) => {
                    self.chars.next();
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
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

    fn lex_identifier_end(&mut self, start: usize, first: char) -> usize {
        let mut end = start + first.len_utf8();

        while let Some((next_index, next)) = self.chars.peek().copied() {
            if next.is_ascii_alphanumeric() || next == '_' {
                end = next_index + next.len_utf8();
                self.chars.next();
            } else {
                break;
            }
        }

        end
    }

    fn lex_number(&mut self, start: usize, first: char) -> (TokenKind, usize) {
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

        (
            if is_float {
                TokenKind::FloatLiteral
            } else {
                TokenKind::IntegerLiteral
            },
            end,
        )
    }

    fn lex_string(&mut self, start: usize) -> Result<(String, Span), LexError> {
        let mut lexeme = String::new();

        while let Some((index, character)) = self.chars.next() {
            if character == '"' {
                return Ok((lexeme, Span::new(start, index + character.len_utf8())));
            }

            if character == '\\' {
                let Some((_, escaped)) = self.chars.next() else {
                    return Err(LexError::new(
                        "unterminated string escape",
                        Span::new(start, self.source.len()),
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
            Span::new(start, self.source.len()),
        ))
    }

    fn lex_symbol_end(&mut self, start: usize, first: char) -> usize {
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

        end
    }

    fn peek_character(&mut self) -> Option<char> {
        self.chars.peek().map(|(_, character)| *character)
    }
}

#[cfg(test)]
mod tests {
    use super::Lexer;
    use omega_tokens::{KeywordKind, PunctuationKind, TokenKind};

    #[test]
    fn tokenizes_keywords_and_identifiers_distinctly() {
        let tokens = Lexer::new("machine game entry self true false custom")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(tokens[0].kind, TokenKind::Keyword(KeywordKind::Machine));
        assert_eq!(tokens[1].kind, TokenKind::Identifier);
        assert_eq!(tokens[2].kind, TokenKind::Keyword(KeywordKind::Entry));
        assert_eq!(tokens[3].kind, TokenKind::Keyword(KeywordKind::SelfValue));
        assert_eq!(tokens[4].kind, TokenKind::Keyword(KeywordKind::True));
        assert_eq!(tokens[5].kind, TokenKind::Keyword(KeywordKind::False));
        assert_eq!(tokens[6].kind, TokenKind::Identifier);
    }

    #[test]
    fn tokenizes_multi_character_punctuation() {
        let tokens = Lexer::new(":: -> == != << <= >> >= && ||")
            .tokenize()
            .expect("tokenization should succeed");

        let kinds: Vec<_> = tokens.iter().map(|token| token.kind).collect();
        assert_eq!(
            kinds,
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

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Keyword(KeywordKind::Let));
        assert_eq!(tokens[1].kind, TokenKind::Identifier);
        assert_eq!(tokens[1].lexeme.as_str(), "value");
    }

    #[test]
    fn skips_block_comments_and_whitespace() {
        let tokens = Lexer::new("let /* comment */ value")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Keyword(KeywordKind::Let));
        assert_eq!(tokens[1].kind, TokenKind::Identifier);
        assert_eq!(tokens[1].lexeme.as_str(), "value");
    }

    #[test]
    fn skips_nested_block_comments() {
        let tokens = Lexer::new("let /* outer /* inner */ still outer */ value")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Keyword(KeywordKind::Let));
        assert_eq!(tokens[1].kind, TokenKind::Identifier);
        assert_eq!(tokens[1].lexeme.as_str(), "value");
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

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::IntegerLiteral);
        assert_eq!(tokens[1].kind, TokenKind::FloatLiteral);
    }
}
