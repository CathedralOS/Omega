mod expressions;
mod items;
mod machines;

use crate::parse_error::ParseError;
use omega_abstract_syntax_tree::expression::{
    BinaryExpression, BinaryOperator, CallExpression, Expression, IndexedExpression, StructLiteral,
    StructLiteralField,
};
use omega_abstract_syntax_tree::identifier::{Identifier, IdentifierPath};
use omega_abstract_syntax_tree::item::{
    CapabilityContract, CapabilityContractKind, CapabilityDefinition, CapabilityField,
    CapabilityMember, CapabilityState, Contains, DataDefinition, DataField, DataMember,
    DataVariant, InvariantDefinition, Item, LibraryDefinition, LibraryFunction, Machine, OwnedData,
    Platform, State, StateParameter, StateSignature, TargetDefinition, TargetHost,
    TargetHostSetting, TargetHostSettingValue, TrustDefinition, TrustLevel, TrustMode, TrustPolicy,
    UseItem,
};
use omega_abstract_syntax_tree::statement::{
    Assignment, Call, LocalData, Statement, Transition, TransitionGuard, TransitionTarget,
};
use omega_abstract_syntax_tree::tables::AstTables;
use omega_abstract_syntax_tree::types::{TypeConstraint, TypeReference};
use omega_core::source::{FileId, SourceText};
use omega_lexer::{Token, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstFile {
    pub file_id: FileId,
    pub items: Vec<Item>,
    pub tables: AstTables,
}

pub fn parse_file(tokens: &[Token<'_>]) -> Result<AstFile, ParseError> {
    parse_file_with_id(FileId::default(), tokens)
}

pub fn parse_file_with_id(file_id: FileId, tokens: &[Token<'_>]) -> Result<AstFile, ParseError> {
    parse_file_with_optional_source(file_id, tokens)
}

pub fn parse_file_with_source(
    file_id: FileId,
    _source: std::sync::Arc<str>,
    tokens: &[Token<'_>],
) -> Result<AstFile, ParseError> {
    parse_file_with_optional_source(file_id, tokens)
}

fn parse_file_with_optional_source(file_id: FileId, tokens: &[Token<'_>]) -> Result<AstFile, ParseError> {
    Parser {
        file_id,
        tokens,
        index: 0,
    }
    .parse_file()
}

struct Parser<'tokens, 'source> {
    file_id: FileId,
    tokens: &'tokens [Token<'source>],
    index: usize,
}

impl Parser<'_, '_> {
    fn skip_balanced_braces(&mut self) -> Result<(), ParseError> {
        self.expect("{")?;
        let mut depth = 1;

        while depth > 0 {
            let Some(token) = self.advance() else {
                return Err(self.error_here("unterminated block"));
            };

            if token.lexeme.as_str() == "{" {
                depth += 1;
            } else if token.lexeme.as_str() == "}" {
                depth -= 1;
            }
        }

        Ok(())
    }

    fn skip_balanced_braces_with_count(&mut self) -> Result<usize, ParseError> {
        self.expect("{")?;
        let mut depth = 1usize;
        let mut token_count = 0usize;

        while depth > 0 {
            let Some(token) = self.advance() else {
                return Err(self.error_here("unterminated block"));
            };

            if token.lexeme.as_str() == "{" {
                depth += 1;
            } else if token.lexeme.as_str() == "}" {
                depth -= 1;
            }

            if depth > 0 {
                token_count += 1;
            }
        }

        Ok(token_count)
    }

    fn skip_balanced_parentheses_after_open(&mut self) -> Result<usize, ParseError> {
        let mut depth = 1usize;
        let mut token_count = 0usize;

        while depth > 0 {
            let Some(token) = self.advance() else {
                return Err(self.error_here("unterminated parenthesized value"));
            };

            if token.lexeme.as_str() == "(" {
                depth += 1;
            } else if token.lexeme.as_str() == ")" {
                depth -= 1;
            }

            if depth > 0 {
                token_count += 1;
            }
        }

        Ok(token_count)
    }

    fn consume(&mut self, lexeme: &str) -> bool {
        if self.check(lexeme) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn consume_state_or_fn_keyword(&mut self) -> bool {
        self.consume("state") || self.consume("fn")
    }

    fn consume_callable_keyword(&mut self) -> bool {
        self.consume("entry")
    }

    fn check(&self, lexeme: &str) -> bool {
        self.peek()
            .is_some_and(|token| token.lexeme.as_str() == lexeme)
    }

    fn check_state_or_fn_keyword(&self) -> bool {
        self.check("state") || self.check("fn")
    }

    fn check_kind(&self, kind: TokenKind) -> bool {
        self.peek().is_some_and(|token| token.kind == kind)
    }

    fn transition_subject_is_bare_name(&self) -> bool {
        let mut cursor = self.index;
        let Some(token) = self.tokens.get(cursor) else {
            return false;
        };

        if token.kind != TokenKind::Identifier {
            return false;
        }

        cursor += 1;

        loop {
            let Some(separator) = self.tokens.get(cursor) else {
                return false;
            };

            if separator.lexeme.as_str() != "." && separator.lexeme.as_str() != "::" {
                return separator.lexeme.as_str() == "{";
            }

            cursor += 1;

            let Some(member) = self.tokens.get(cursor) else {
                return false;
            };

            if member.kind != TokenKind::Identifier {
                return false;
            }

            cursor += 1;
        }
    }

    fn brace_starts_struct_literal(&self) -> bool {
        if !self.check("{") {
            return false;
        }

        let Some(next) = self.tokens.get(self.index + 1) else {
            return false;
        };

        if next.lexeme.as_str() == "}" {
            return true;
        }

        next.kind == TokenKind::Identifier
            && self
                .tokens
                .get(self.index + 2)
                .is_some_and(|token| token.lexeme.as_str() == ":")
    }

    fn expect(&mut self, lexeme: &str) -> Result<(), ParseError> {
        if self.consume(lexeme) {
            Ok(())
        } else {
            Err(self.error_here(format!("expected `{lexeme}`")))
        }
    }

    fn expect_state_or_fn_keyword(&mut self) -> Result<(), ParseError> {
        if self.consume_state_or_fn_keyword() {
            Ok(())
        } else {
            Err(self.error_here("expected `state` or `fn`"))
        }
    }

    fn expect_callable_keyword(&mut self) -> Result<(), ParseError> {
        if self.consume_callable_keyword() {
            Ok(())
        } else {
            Err(self.error_here("expected `entry`"))
        }
    }

    fn expect_identifier(&mut self) -> Result<Identifier, ParseError> {
        let file_id = self.file_id;
        let Some(token) = self.advance() else {
            return Err(self.error_here("expected identifier"));
        };

        if token.kind == TokenKind::Identifier {
            Ok(identifier_from_token(file_id, token))
        } else {
            Err(ParseError::at_span("expected identifier", token.span))
        }
    }

    fn expect_integer_literal(&mut self) -> Result<usize, ParseError> {
        let Some(token) = self.advance() else {
            return Err(self.error_here("expected integer literal"));
        };

        if token.kind != TokenKind::Integer {
            return Err(ParseError::at_span("expected integer literal", token.span));
        }

        token
            .lexeme
            .as_str()
            .parse::<usize>()
            .map_err(|_| ParseError::at_span("invalid integer literal", token.span))
    }

    fn expect_string_literal(&mut self) -> Result<String, ParseError> {
        let Some(token) = self.advance() else {
            return Err(self.error_here("expected string literal"));
        };

        if token.kind == TokenKind::String {
            Ok(token.lexeme.as_str().to_owned())
        } else {
            Err(ParseError::at_span("expected string literal", token.span))
        }
    }

    fn advance(&mut self) -> Option<&Token<'_>> {
        let token = self.tokens.get(self.index)?;
        self.index += 1;
        Some(token)
    }

    fn peek(&self) -> Option<&Token<'_>> {
        self.tokens.get(self.index)
    }

    fn is_at_end(&self) -> bool {
        self.index >= self.tokens.len()
    }

    fn error_here(&self, message: impl Into<String>) -> ParseError {
        if let Some(token) = self.peek() {
            ParseError::at_span(message, token.span)
        } else {
            ParseError::new(message)
        }
    }
}

fn identifier_from_token(file_id: FileId, token: &Token<'_>) -> Identifier {
    let source_span = omega_core::source::SourceSpan::new(file_id, token.span);
    Identifier::new(token.lexeme.as_str(), source_span)
}

fn source_text_from_token(file_id: FileId, token: &Token<'_>) -> SourceText {
    let source_span = omega_core::source::SourceSpan::new(file_id, token.span);
    SourceText::new(token.lexeme.as_str(), source_span)
}

fn binary_expression(left: Expression, operator: BinaryOperator, right: Expression) -> Expression {
    Expression::Binary(Box::new(BinaryExpression {
        left,
        operator,
        right,
    }))
}

fn expression_to_transition_target(expression: Expression) -> TransitionTarget {
    match expression {
        Expression::Name(path) => {
            if path.len() == 1 && path.first().is_some_and(|name| name.as_str() == "self") {
                TransitionTarget::SelfTarget
            } else {
                TransitionTarget::Named {
                    path,
                    arguments: Vec::new(),
                }
            }
        }
        Expression::Call(call) => {
            let CallExpression {
                receiver,
                target,
                arguments,
            } = *call;

            if let Some(receiver) = receiver {
                match *receiver {
                    Expression::Name(mut path) => {
                        path.push(target);
                        TransitionTarget::Named { path, arguments }
                    }
                    other => TransitionTarget::Value(Expression::Call(Box::new(CallExpression {
                        receiver: Some(Box::new(other)),
                        target,
                        arguments,
                    }))),
                }
            } else {
                TransitionTarget::Named {
                    path: IdentifierPath::from(vec![target]),
                    arguments,
                }
            }
        }
        other => TransitionTarget::Value(other),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_file;
    use omega_lexer::Lexer;

    #[test]
    fn parses_machine_for_with_pub_entry_and_merges_blocks() {
        let tokens = Lexer::new(
            r#"
            data Game {
                seed: u64 = 1337;
            }

            machine for Game {
                pub entry new() -> Game {
                }
            }

            machine for Game {
                state ready(&mut self) {
                }
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");

        let parsed = parse_file(&tokens).expect("parse should succeed");

        assert_eq!(parsed.items.len(), 2);

        let omega_abstract_syntax_tree::item::Item::Machine(machine) = &parsed.items[1] else {
            panic!("expected merged machine item");
        };

        assert_eq!(machine.name, "Game");
        assert_eq!(machine.states.len(), 2);
        assert_eq!(machine.states[0].name, "new");
        assert_eq!(machine.states[1].name, "ready");
    }

    #[test]
    fn parses_unnamed_entry_as_entry_state() {
        let tokens = Lexer::new(
            r#"
            machine for main {
                pub entry(&mut self) -> i32 {
                    0
                }
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");

        let parsed = parse_file(&tokens).expect("parse should succeed");

        let omega_abstract_syntax_tree::item::Item::Machine(machine) = &parsed.items[0] else {
            panic!("expected machine item");
        };

        assert_eq!(machine.states.len(), 1);
        assert_eq!(machine.states[0].name, "entry");
    }

    #[test]
    fn parses_transition_blocks_as_ordered_transitions() {
        let tokens = Lexer::new(
            r#"
            machine for Game {
                pub entry run(&mut self) {
                    transition ready {
                        true -> done()
                        false -> wait()
                    }
                }

                state done(&mut self) {
                }

                state wait(&mut self) {
                }
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");

        let parsed = parse_file(&tokens).expect("parse should succeed");

        let omega_abstract_syntax_tree::item::Item::Machine(machine) = &parsed.items[0] else {
            panic!("expected machine item");
        };

        assert_eq!(machine.states[0].statements.len(), 2);
    }

    #[test]
    fn parses_machine_header_entry_surface_and_inherits_return_type() {
        let tokens = Lexer::new(
            r#"
            machine Game::new -> u64 {
                pub entry(seed: u64) {
                    -> ready(seed);
                }

                state ready(seed: u64) {
                    seed
                }
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");

        let parsed = parse_file(&tokens).expect("parse should succeed");

        let omega_abstract_syntax_tree::item::Item::Machine(machine) = &parsed.items[0] else {
            panic!("expected machine item");
        };

        assert_eq!(machine.name, "Game");
        assert_eq!(machine.states[0].name, "new");
        assert_eq!(
            machine.states[0].return_type,
            Some(omega_abstract_syntax_tree::types::TypeReference::named("u64"))
        );
        assert_eq!(
            machine.states[1].return_type,
            Some(omega_abstract_syntax_tree::types::TypeReference::named("u64"))
        );
    }

    #[test]
    fn parses_boolean_transition_block_with_recursive_call_target() {
        let tokens = Lexer::new(
            r#"
            machine for RoomFormatter {
                state append_exit(
                    &mut self,
                    exits: &[Exit],
                    exit_count: usize[positive],
                    out_line: &mut ConsoleLine,
                    index: IndexOf<exits>
                ) {
                    let next_index: usize = index + 1;

                    transition next_index < exit_count {
                        true -> append_exit(exits, exit_count, out_line, next_index)
                        false -> {}
                    }
                }
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");

        let parsed = parse_file(&tokens).expect("parse should succeed");

        let omega_abstract_syntax_tree::item::Item::Machine(machine) = &parsed.items[0] else {
            panic!("expected machine item");
        };

        assert_eq!(machine.states.len(), 1);
        assert_eq!(machine.states[0].statements.len(), 3);
    }
}
