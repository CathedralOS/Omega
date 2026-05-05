use crate::ast::expression::Expression;
use crate::ast::item::{
    CommandParameter, CommandSignature, Contains, Item, Machine, Platform, State, UseItem,
};
use crate::ast::statement::{Assignment, CommandCall, Statement, Transition, TransitionTarget};
use crate::ast::types::TypeReference;
use crate::lexer::{Token, TokenKind};
use crate::parser::parse_error::ParseError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstFile {
    pub items: Vec<Item>,
}

pub fn parse_file(tokens: &[Token]) -> Result<AstFile, ParseError> {
    Parser { tokens, index: 0 }.parse_file()
}

pub fn parse_items(tokens: &[Token]) -> Result<Vec<Item>, ParseError> {
    parse_file(tokens).map(|file| file.items)
}

struct Parser<'a> {
    tokens: &'a [Token],
    index: usize,
}

impl Parser<'_> {
    fn parse_file(&mut self) -> Result<AstFile, ParseError> {
        let mut items = Vec::new();

        while !self.is_at_end() {
            if self.consume("use") {
                items.push(Item::Use(self.parse_use()?));
            } else if self.consume("platform") {
                items.push(Item::Platform(self.parse_platform()?));
            } else if self.consume("machine") {
                items.push(Item::Machine(self.parse_machine()?));
            } else {
                return Err(self.error_here("expected top-level item"));
            }
        }

        Ok(AstFile { items })
    }

    fn parse_use(&mut self) -> Result<UseItem, ParseError> {
        let mut path = vec![self.expect_identifier()?];

        while self.consume("::") {
            path.push(self.expect_identifier()?);
        }

        self.expect(";")?;

        Ok(UseItem { path })
    }

    fn parse_platform(&mut self) -> Result<Platform, ParseError> {
        let name = self.expect_identifier()?;
        self.expect("{")?;

        let mut commands = Vec::new();

        while !self.consume("}") {
            self.expect("command")?;
            let name = self.expect_identifier()?;
            let parameters = self.parse_command_parameters()?;
            self.expect(";")?;
            commands.push(CommandSignature { name, parameters });
        }

        Ok(Platform { name, commands })
    }

    fn parse_command_parameters(&mut self) -> Result<Vec<CommandParameter>, ParseError> {
        self.expect("(")?;

        let mut parameters = Vec::new();

        if self.consume(")") {
            return Ok(parameters);
        }

        loop {
            let is_mutable = self.consume("mut");
            let name = self.expect_identifier()?;
            self.expect(":")?;
            let type_reference = TypeReference {
                name: self.expect_identifier()?,
            };

            parameters.push(CommandParameter {
                name,
                type_reference,
                is_mutable,
            });

            if self.consume(")") {
                break;
            }

            self.expect(",")?;
        }

        Ok(parameters)
    }

    fn parse_machine(&mut self) -> Result<Machine, ParseError> {
        let name = self.expect_identifier()?;
        self.expect("{")?;

        let mut contains = Vec::new();
        let mut states = Vec::new();

        while !self.consume("}") {
            if self.consume("contains") {
                contains.push(self.parse_contains()?);
            } else if self.consume("owns") {
                self.skip_until_semicolon()?;
            } else if self.consume("state") {
                states.push(self.parse_state()?);
            } else if self.consume("command") {
                self.skip_command_declaration()?;
            } else {
                return Err(self.error_here("expected machine item"));
            }
        }

        Ok(Machine {
            name,
            contains,
            states,
        })
    }

    fn parse_contains(&mut self) -> Result<Contains, ParseError> {
        let name = self.expect_identifier()?;
        self.expect(":")?;
        let type_name = self.expect_identifier()?;
        self.expect(";")?;

        Ok(Contains { name, type_name })
    }

    fn parse_state(&mut self) -> Result<State, ParseError> {
        let name = self.expect_identifier()?;
        self.expect("{")?;

        let mut statements = Vec::new();

        while !self.consume("}") {
            if self.consume("->") {
                statements.push(self.parse_transition()?);
            } else {
                statements.push(self.parse_statement()?);
            }
        }

        Ok(State { name, statements })
    }

    fn parse_transition(&mut self) -> Result<Statement, ParseError> {
        let target = self.parse_transition_target()?;
        let continuation = if self.consume("->") {
            Some(self.parse_transition_target()?)
        } else {
            None
        };
        let condition = if self.consume("when") {
            Some(self.collect_condition_until_semicolon()?)
        } else {
            self.expect(";")?;
            None
        };

        Ok(Statement::Transition(Transition {
            target,
            continuation,
            condition,
        }))
    }

    fn parse_transition_target(&mut self) -> Result<TransitionTarget, ParseError> {
        if self.consume("self") {
            return Ok(TransitionTarget::SelfTarget);
        }

        if self.consume("return") {
            return Ok(TransitionTarget::ReturnToCaller);
        }

        let mut path = vec![self.expect_identifier()?];

        while self.consume(".") {
            path.push(self.expect_identifier()?);
        }

        Ok(TransitionTarget::Named(path))
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        let first_name = self.expect_identifier()?;

        if self.consume("=") {
            let value = self.parse_expression()?;
            self.expect(";")?;
            return Ok(Statement::Assignment(Assignment {
                target: vec![first_name],
                value,
            }));
        }

        if self.check("(") {
            self.expect("(")?;
            let arguments = self.parse_call_arguments()?;
            return Ok(Statement::CommandCall(CommandCall {
                receiver: None,
                command: first_name,
                arguments,
            }));
        }

        self.expect(".")?;
        let second_name = self.expect_identifier()?;

        if !self.check("(") {
            let mut target = vec![first_name, second_name];

            while self.consume(".") {
                target.push(self.expect_identifier()?);
            }

            self.expect("=")?;
            let value = self.parse_expression()?;
            self.expect(";")?;
            return Ok(Statement::Assignment(Assignment { target, value }));
        }

        self.expect("(")?;
        let arguments = self.parse_call_arguments()?;

        Ok(Statement::CommandCall(CommandCall {
            receiver: Some(first_name),
            command: second_name,
            arguments,
        }))
    }

    fn parse_call_arguments(&mut self) -> Result<Vec<Expression>, ParseError> {
        let mut arguments = Vec::new();

        if !self.check(")") {
            loop {
                arguments.push(self.parse_expression()?);

                if !self.consume(",") {
                    break;
                }
            }
        }

        self.expect(")")?;
        self.expect(";")?;

        Ok(arguments)
    }

    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        if self.consume("mut") {
            return Ok(Expression::Mutable(Box::new(self.parse_expression()?)));
        }

        if let Some(token) = self.advance() {
            match token.kind {
                TokenKind::Integer => token
                    .lexeme
                    .parse::<i64>()
                    .map(Expression::Integer)
                    .map_err(|_| ParseError::new("invalid integer literal")),
                TokenKind::Identifier => {
                    let mut path = vec![token.lexeme.clone()];

                    while self.consume(".") || self.consume("::") {
                        path.push(self.expect_identifier()?);
                    }

                    Ok(Expression::Name(path))
                }
                TokenKind::String => Ok(Expression::String(token.lexeme.clone())),
                _ => Err(ParseError::new("expected expression")),
            }
        } else {
            Err(ParseError::new("expected expression"))
        }
    }

    fn skip_command_declaration(&mut self) -> Result<(), ParseError> {
        self.expect_identifier()?;
        self.skip_balanced_parens()?;

        if self.consume(";") {
            return Ok(());
        }

        if self.consume("when") {
            while !self.check("{") {
                self.advance()
                    .ok_or_else(|| ParseError::new("unterminated command guard"))?;
            }
        }

        self.skip_balanced_braces()
    }

    fn skip_balanced_parens(&mut self) -> Result<(), ParseError> {
        self.expect("(")?;
        let mut depth = 1;

        while depth > 0 {
            let token = self
                .advance()
                .ok_or_else(|| ParseError::new("unterminated parentheses"))?;

            if token.lexeme == "(" {
                depth += 1;
            } else if token.lexeme == ")" {
                depth -= 1;
            }
        }

        Ok(())
    }

    fn skip_balanced_braces(&mut self) -> Result<(), ParseError> {
        self.expect("{")?;
        let mut depth = 1;

        while depth > 0 {
            let token = self
                .advance()
                .ok_or_else(|| ParseError::new("unterminated block"))?;

            if token.lexeme == "{" {
                depth += 1;
            } else if token.lexeme == "}" {
                depth -= 1;
            }
        }

        Ok(())
    }

    fn skip_until_semicolon(&mut self) -> Result<(), ParseError> {
        while !self.consume(";") {
            self.advance()
                .ok_or_else(|| ParseError::new("expected semicolon"))?;
        }

        Ok(())
    }

    fn collect_condition_until_semicolon(&mut self) -> Result<String, ParseError> {
        let mut parts = Vec::new();

        while !self.consume(";") {
            let token = self
                .advance()
                .ok_or_else(|| ParseError::new("expected transition condition"))?;
            parts.push(token.lexeme.clone());
        }

        if parts.is_empty() {
            Err(ParseError::new("expected transition condition"))
        } else {
            Ok(parts.join(" "))
        }
    }

    fn consume(&mut self, lexeme: &str) -> bool {
        if self.check(lexeme) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn check(&self, lexeme: &str) -> bool {
        self.peek().is_some_and(|token| token.lexeme == lexeme)
    }

    fn expect(&mut self, lexeme: &str) -> Result<(), ParseError> {
        if self.consume(lexeme) {
            Ok(())
        } else {
            Err(self.error_here(format!("expected `{lexeme}`")))
        }
    }

    fn expect_identifier(&mut self) -> Result<String, ParseError> {
        let token = self
            .advance()
            .ok_or_else(|| ParseError::new("expected identifier"))?;

        if token.kind == TokenKind::Identifier {
            Ok(token.lexeme.clone())
        } else {
            Err(ParseError::new("expected identifier"))
        }
    }

    fn advance(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.index)?;
        self.index += 1;
        Some(token)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn is_at_end(&self) -> bool {
        self.index >= self.tokens.len()
    }

    fn error_here(&self, message: impl Into<String>) -> ParseError {
        ParseError::new(message)
    }
}
