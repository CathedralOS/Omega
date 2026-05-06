use crate::ast::expression::{
    BinaryExpression, BinaryOperator, Expression, IndexedExpression, StructLiteral,
    StructLiteralField,
};
use crate::ast::item::{
    CommandDefinition, CommandParameter, CommandSignature, Contains, DataDefinition, DataField,
    DataMember, DataVariant, Item, Machine, OwnedData, Platform, State, UseItem,
};
use crate::ast::statement::{
    Assignment, CommandCall, LocalData, Statement, Transition, TransitionTarget,
};
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
            } else if self.consume("data") {
                items.push(Item::Data(self.parse_data_definition()?));
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

    fn parse_data_definition(&mut self) -> Result<DataDefinition, ParseError> {
        let name = self.expect_identifier()?;
        self.expect("{")?;

        let mut members = Vec::new();

        while !self.consume("}") {
            let member_name = self.expect_identifier()?;

            if self.consume(":") {
                let type_reference = self.parse_type_reference()?;
                self.expect(";")?;
                members.push(DataMember::Field(DataField {
                    name: member_name,
                    type_reference,
                }));
            } else {
                members.push(DataMember::Variant(DataVariant { name: member_name }));

                if !self.check("}") {
                    self.expect(",")?;
                }
            }
        }

        Ok(DataDefinition { name, members })
    }

    fn parse_platform(&mut self) -> Result<Platform, ParseError> {
        let name = self.expect_identifier()?;
        self.expect("{")?;

        let mut commands = Vec::new();

        while !self.consume("}") {
            self.expect("command")?;
            let signature = self.parse_command_signature()?;
            self.expect(";")?;
            commands.push(signature);
        }

        Ok(Platform { name, commands })
    }

    fn parse_command_signature(&mut self) -> Result<CommandSignature, ParseError> {
        let name = self.expect_identifier()?;
        let parameters = self.parse_command_parameters()?;

        Ok(CommandSignature { name, parameters })
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
            let type_reference = self.parse_type_reference()?;

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

    fn parse_type_reference(&mut self) -> Result<TypeReference, ParseError> {
        if self.consume("[") {
            let element_type = self.parse_type_reference()?;
            self.expect(";")?;
            let length = self.expect_integer_literal()?;
            self.expect("]")?;

            return Ok(TypeReference::FixedArray {
                element_type: Box::new(element_type),
                length,
            });
        }

        Ok(TypeReference::named(self.expect_identifier()?))
    }

    fn parse_machine(&mut self) -> Result<Machine, ParseError> {
        let name = self.expect_identifier()?;
        self.expect("{")?;

        let mut contains = Vec::new();
        let mut commands = Vec::new();
        let mut owned_data = Vec::new();
        let mut states = Vec::new();

        while !self.consume("}") {
            if self.consume("contains") {
                contains.push(self.parse_contains()?);
            } else if self.consume("owns") {
                owned_data.push(self.parse_owned_data()?);
            } else if self.consume("state") {
                states.push(self.parse_state()?);
            } else if self.consume("command") {
                commands.push(self.parse_command_definition()?);
            } else if self.consume("invariant") {
                self.expect_identifier()?;
                self.skip_balanced_braces()?;
            } else {
                return Err(self.error_here("expected machine item"));
            }
        }

        Ok(Machine {
            name,
            contains,
            commands,
            owned_data,
            states,
        })
    }

    fn parse_command_definition(&mut self) -> Result<CommandDefinition, ParseError> {
        let signature = self.parse_command_signature()?;

        if self.consume(";") {
            return Ok(CommandDefinition {
                signature,
                guard: None,
                statements: Vec::new(),
            });
        }

        let guard = if self.consume("when") {
            Some(self.collect_until("{")?)
        } else {
            None
        };

        let statements = self.parse_statement_block()?;

        Ok(CommandDefinition {
            signature,
            guard,
            statements,
        })
    }

    fn parse_contains(&mut self) -> Result<Contains, ParseError> {
        let name = self.expect_identifier()?;
        self.expect(":")?;
        let type_name = self.expect_identifier()?;
        self.expect(";")?;

        Ok(Contains { name, type_name })
    }

    fn parse_owned_data(&mut self) -> Result<OwnedData, ParseError> {
        let name = self.expect_identifier()?;
        self.expect(":")?;
        let type_reference = self.parse_type_reference()?;
        let initial_value = if self.consume("=") {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.expect(";")?;

        Ok(OwnedData {
            name,
            type_reference,
            initial_value,
        })
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
        if self.consume("let") {
            return self.parse_local_data();
        }

        let first_name = self.expect_identifier()?;

        if self.consume("=") {
            let value = self.parse_expression()?;
            self.expect(";")?;
            return Ok(Statement::Assignment(Assignment {
                target: Expression::Name(vec![first_name]),
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
            let target =
                self.parse_reference_tail(Expression::Name(vec![first_name, second_name]))?;

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

    fn parse_local_data(&mut self) -> Result<Statement, ParseError> {
        let name = self.expect_identifier()?;
        self.expect(":")?;
        let type_reference = self.parse_type_reference()?;
        self.expect(";")?;

        Ok(Statement::LocalData(LocalData {
            name,
            type_reference,
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
        self.parse_add_expression()
    }

    fn parse_add_expression(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_primary_expression()?;

        while self.consume("+") {
            let right = self.parse_primary_expression()?;
            expression = Expression::Binary(Box::new(BinaryExpression {
                left: expression,
                operator: BinaryOperator::Add,
                right,
            }));
        }

        Ok(expression)
    }

    fn parse_primary_expression(&mut self) -> Result<Expression, ParseError> {
        if self.consume("mut") {
            return Ok(Expression::Mutable(Box::new(self.parse_expression()?)));
        }

        if self.consume("[") {
            return self.parse_array_literal();
        }

        if let Some(token) = self.advance() {
            match token.kind {
                TokenKind::Integer => token
                    .lexeme
                    .parse::<i64>()
                    .map(Expression::Integer)
                    .map_err(|_| ParseError::at_span("invalid integer literal", token.span)),
                TokenKind::Identifier => {
                    let mut path = vec![token.lexeme.clone()];

                    while self.consume(".") || self.consume("::") {
                        path.push(self.expect_identifier()?);
                    }

                    if self.check("{") && path.len() == 1 {
                        self.parse_struct_literal(path.remove(0))
                    } else {
                        self.parse_reference_tail(Expression::Name(path))
                    }
                }
                TokenKind::String => Ok(Expression::String(token.lexeme.clone())),
                _ => Err(ParseError::at_span("expected expression", token.span)),
            }
        } else {
            Err(self.error_here("expected expression"))
        }
    }

    fn parse_reference_tail(
        &mut self,
        mut expression: Expression,
    ) -> Result<Expression, ParseError> {
        loop {
            if self.consume("[") {
                let index = self.parse_expression()?;
                self.expect("]")?;
                expression = Expression::Indexed(Box::new(IndexedExpression {
                    collection: expression,
                    index,
                }));
            } else if self.consume(".") {
                let name = self.expect_identifier()?;
                expression = match expression {
                    Expression::Name(mut path) => {
                        path.push(name);
                        Expression::Name(path)
                    }
                    _ => {
                        return Err(self.error_here(
                            "field access after a complex expression is not supported yet",
                        ));
                    }
                };
            } else {
                break;
            }
        }

        Ok(expression)
    }

    fn parse_array_literal(&mut self) -> Result<Expression, ParseError> {
        let mut values = Vec::new();

        if self.consume("]") {
            return Ok(Expression::ArrayLiteral(values));
        }

        loop {
            values.push(self.parse_expression()?);

            if self.consume("]") {
                break;
            }

            self.expect(",")?;

            if self.consume("]") {
                break;
            }
        }

        Ok(Expression::ArrayLiteral(values))
    }

    fn parse_struct_literal(&mut self, type_name: String) -> Result<Expression, ParseError> {
        self.expect("{")?;
        let mut fields = Vec::new();

        while !self.consume("}") {
            let name = self.expect_identifier()?;
            self.expect(":")?;
            let value = self.parse_expression()?;
            fields.push(StructLiteralField { name, value });

            if !self.check("}") {
                self.expect(",")?;
            }
        }

        Ok(Expression::StructLiteral(StructLiteral {
            type_name,
            fields,
        }))
    }

    fn parse_statement_block(&mut self) -> Result<Vec<Statement>, ParseError> {
        self.expect("{")?;
        let mut statements = Vec::new();

        while !self.consume("}") {
            if self.consume("->") {
                statements.push(self.parse_transition()?);
            } else {
                statements.push(self.parse_statement()?);
            }
        }

        Ok(statements)
    }

    fn skip_balanced_braces(&mut self) -> Result<(), ParseError> {
        self.expect("{")?;
        let mut depth = 1;

        while depth > 0 {
            let Some(token) = self.advance() else {
                return Err(self.error_here("unterminated block"));
            };

            if token.lexeme == "{" {
                depth += 1;
            } else if token.lexeme == "}" {
                depth -= 1;
            }
        }

        Ok(())
    }

    fn collect_until(&mut self, lexeme: &str) -> Result<String, ParseError> {
        let mut parts = Vec::new();

        while !self.check(lexeme) {
            let Some(token) = self.advance() else {
                return Err(self.error_here(format!("expected `{lexeme}`")));
            };
            parts.push(token.lexeme.clone());
        }

        Ok(parts.join(" "))
    }

    fn collect_condition_until_semicolon(&mut self) -> Result<String, ParseError> {
        let mut parts = Vec::new();

        while !self.consume(";") {
            let Some(token) = self.advance() else {
                return Err(self.error_here("expected transition condition"));
            };
            parts.push(token.lexeme.clone());
        }

        if parts.is_empty() {
            Err(self.error_here("expected transition condition"))
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
        let Some(token) = self.advance() else {
            return Err(self.error_here("expected identifier"));
        };

        if token.kind == TokenKind::Identifier {
            Ok(token.lexeme.clone())
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
            .parse::<usize>()
            .map_err(|_| ParseError::at_span("invalid integer literal", token.span))
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
        if let Some(token) = self.peek() {
            ParseError::at_span(message, token.span)
        } else {
            ParseError::new(message)
        }
    }
}
