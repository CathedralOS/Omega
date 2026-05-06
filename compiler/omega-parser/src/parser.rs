use crate::parse_error::ParseError;
use omega_ast::expression::{
    BinaryExpression, BinaryOperator, Expression, IndexedExpression, StructLiteral,
    StructLiteralField,
};
use omega_ast::item::{
    CapabilityContract, CapabilityContractKind, CapabilityDefinition, CapabilityField,
    CapabilityMember, CapabilityState, Contains, DataDefinition, DataField, DataMember,
    DataVariant, InvariantDefinition, Item, Machine, OwnedData, Platform, State, StateParameter,
    StateSignature, TargetDefinition, TargetHost, TargetHostSetting, TargetHostSettingValue,
    TrustLevel, TrustMode, TrustPolicy, UseItem,
};
use omega_ast::statement::{
    Assignment, Call, LocalData, Statement, Transition, TransitionGuard, TransitionTarget,
};
use omega_ast::types::{TypeConstraint, TypeReference};
use omega_lexer::{Token, TokenKind};

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
            } else if self.consume("target") {
                items.push(Item::Target(self.parse_target_definition()?));
            } else if self.consume("capability") {
                items.push(Item::Capability(self.parse_capability_definition()?));
            } else if self.consume("invariant") {
                items.push(Item::Invariant(self.parse_invariant_definition()?));
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

    fn parse_target_definition(&mut self) -> Result<TargetDefinition, ParseError> {
        let name = self.expect_identifier()?;
        self.expect("{")?;

        let mut host = None;
        let mut trust_policies = Vec::new();

        while !self.consume("}") {
            if self.consume("host") {
                host = Some(self.parse_target_host()?);
            } else if self.consume("trust") {
                trust_policies.push(self.parse_trust_policy()?);
            } else {
                return Err(self.error_here("expected target item"));
            }
        }

        Ok(TargetDefinition {
            name,
            host,
            trust_policies,
        })
    }

    fn parse_capability_definition(&mut self) -> Result<CapabilityDefinition, ParseError> {
        let name = self.expect_identifier()?;
        self.expect("{")?;

        let mut members = Vec::new();

        while !self.consume("}") {
            if self.consume("state") {
                members.push(CapabilityMember::State(self.parse_capability_state()?));
            } else {
                let field_name = self.expect_identifier()?;
                self.expect(":")?;
                let type_reference = self.parse_type_reference()?;
                members.push(CapabilityMember::Field(CapabilityField {
                    name: field_name,
                    type_reference,
                }));
            }
        }

        Ok(CapabilityDefinition { name, members })
    }

    fn parse_target_host(&mut self) -> Result<TargetHost, ParseError> {
        self.expect(":")?;
        let provider = self.expect_identifier()?;
        self.expect("{")?;

        let mut settings = Vec::new();

        while !self.consume("}") {
            let name = self.expect_identifier()?;
            self.expect("=")?;
            let value_name = self.expect_identifier()?;
            let value = if self.consume("(") {
                TargetHostSettingValue::Call {
                    name: value_name,
                    argument_tokens: self.skip_balanced_parentheses_after_open()?,
                }
            } else {
                TargetHostSettingValue::Named(value_name)
            };

            settings.push(TargetHostSetting { name, value });
        }

        Ok(TargetHost { provider, settings })
    }

    fn parse_trust_policy(&mut self) -> Result<TrustPolicy, ParseError> {
        let mode = if self.consume("unchecked") {
            TrustMode::Unchecked
        } else {
            TrustMode::Checked
        };
        let name = self.expect_identifier()?;

        Ok(TrustPolicy { mode, name })
    }

    fn parse_capability_state(&mut self) -> Result<CapabilityState, ParseError> {
        let signature = self.parse_state_signature()?;
        let mut contracts = Vec::new();

        while !self.check("state") && !self.check("}") {
            if self.consume("requires") {
                contracts.push(CapabilityContract {
                    kind: CapabilityContractKind::Requires,
                    token_count: self.skip_capability_contract_tokens(),
                });
            } else if self.consume("ensures") {
                contracts.push(CapabilityContract {
                    kind: CapabilityContractKind::Ensures,
                    token_count: self.skip_capability_contract_tokens(),
                });
            } else if self.consume("trusted") {
                let trust_level = self.parse_trust_level()?;
                contracts.push(CapabilityContract {
                    kind: CapabilityContractKind::Trusted(trust_level),
                    token_count: 1,
                });
            } else {
                return Err(self.error_here("expected capability contract"));
            }
        }

        Ok(CapabilityState {
            signature,
            contracts,
        })
    }

    fn parse_trust_level(&mut self) -> Result<TrustLevel, ParseError> {
        let name = self.expect_identifier()?;

        if name == "host" {
            Ok(TrustLevel::Host)
        } else {
            Ok(TrustLevel::Named(name))
        }
    }

    fn skip_capability_contract_tokens(&mut self) -> usize {
        let start = self.index;

        while !self.is_at_end()
            && !self.check("requires")
            && !self.check("ensures")
            && !self.check("trusted")
            && !self.check("state")
            && !self.check("}")
        {
            self.index += 1;
        }

        self.index - start
    }

    fn parse_invariant_definition(&mut self) -> Result<InvariantDefinition, ParseError> {
        let name = self.expect_identifier()?;
        self.expect("=")?;
        let constraints = self.parse_type_constraints()?;
        self.expect(";")?;

        Ok(InvariantDefinition { name, constraints })
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

        let mut states = Vec::new();

        while !self.consume("}") {
            self.expect("state")?;
            let signature = self.parse_state_signature()?;
            self.expect(";")?;
            states.push(signature);
        }

        Ok(Platform { name, states })
    }

    fn parse_state_signature(&mut self) -> Result<StateSignature, ParseError> {
        let name = self.expect_identifier()?;
        let parameters = self.parse_state_parameters()?;
        let return_type = self.parse_optional_return_type()?;

        Ok(StateSignature {
            name,
            parameters,
            return_type,
        })
    }

    fn parse_state_parameters(&mut self) -> Result<Vec<StateParameter>, ParseError> {
        self.expect("(")?;

        let mut parameters = Vec::new();

        if self.consume(")") {
            return Ok(parameters);
        }

        loop {
            let (name, type_reference, is_const, is_mutable, is_self) = if self.consume("&") {
                self.expect("mut")?;
                self.expect("self")?;
                (
                    String::from("self"),
                    TypeReference::named("Self"),
                    false,
                    true,
                    true,
                )
            } else {
                let is_mutable = self.consume("mut");
                let name = self.expect_identifier()?;
                self.expect(":")?;
                let is_const = self.consume("const");
                let type_reference = self.parse_type_reference()?;
                (name, type_reference, is_const, is_mutable, false)
            };

            parameters.push(StateParameter {
                name,
                type_reference,
                is_const,
                is_mutable,
                is_self,
            });

            if self.consume(")") {
                break;
            }

            self.expect(",")?;
        }

        Ok(parameters)
    }

    fn parse_optional_return_type(&mut self) -> Result<Option<TypeReference>, ParseError> {
        if self.consume("->") {
            return Ok(Some(self.parse_type_reference()?));
        }

        Ok(None)
    }

    fn parse_type_reference(&mut self) -> Result<TypeReference, ParseError> {
        if self.consume("(") {
            self.expect(")")?;

            return Ok(TypeReference::Unit);
        }

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

        let base_name = self.expect_identifier()?;
        let mut type_reference = if self.consume("<") {
            let mut arguments = Vec::new();

            if !self.check(">") {
                loop {
                    arguments.push(self.parse_type_reference()?);

                    if !self.consume(",") {
                        break;
                    }
                }
            }

            self.expect(">")?;

            TypeReference::Generic {
                base_name,
                arguments,
            }
        } else {
            TypeReference::named(base_name)
        };

        if self.check("[") {
            type_reference = TypeReference::Constrained {
                base_type: Box::new(type_reference),
                constraints: self.parse_type_constraints()?,
            };
        }

        Ok(type_reference)
    }

    fn parse_type_constraints(&mut self) -> Result<Vec<TypeConstraint>, ParseError> {
        self.expect("[")?;

        let mut constraints = Vec::new();

        if self.consume("]") {
            return Err(self.error_here("expected type constraints"));
        }

        loop {
            constraints.push(self.parse_type_constraint()?);

            if self.consume("]") {
                break;
            }

            self.expect(",")?;
        }

        Ok(constraints)
    }

    fn parse_type_constraint(&mut self) -> Result<TypeConstraint, ParseError> {
        let name = self.expect_identifier()?;

        if name == "range" {
            self.expect("<")?;
            let minimum = self.parse_range_bound_expression()?;
            self.expect(",")?;
            let maximum = self.parse_range_bound_expression()?;
            self.expect(">")?;

            Ok(TypeConstraint::Range { minimum, maximum })
        } else {
            Ok(TypeConstraint::Named(name))
        }
    }

    fn parse_range_bound_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_add_expression()
    }

    fn parse_machine(&mut self) -> Result<Machine, ParseError> {
        let name = self.expect_identifier()?;
        self.expect("{")?;

        let mut contains = Vec::new();
        let mut owned_data = Vec::new();
        let mut states = Vec::new();

        while !self.consume("}") {
            if self.consume("contains") {
                contains.push(self.parse_contains()?);
            } else if self.consume("owns") {
                owned_data.push(self.parse_owned_data()?);
            } else if self.consume("state") {
                states.push(self.parse_state()?);
            } else if self.consume("invariant") {
                self.expect_identifier()?;

                if self.consume("=") {
                    self.parse_type_constraints()?;
                    self.expect(";")?;
                } else {
                    self.skip_balanced_braces()?;
                }
            } else {
                return Err(self.error_here("expected machine item"));
            }
        }

        Ok(Machine {
            name,
            contains,
            owned_data,
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
        let parameters = if self.check("(") {
            self.parse_state_parameters()?
        } else {
            Vec::new()
        };
        let return_type = self.parse_optional_return_type()?;

        self.expect("{")?;

        let mut statements = Vec::new();

        while !self.consume("}") {
            if self.consume("->") {
                statements.push(self.parse_transition()?);
            } else {
                statements.push(self.parse_statement()?);
            }
        }

        Ok(State {
            name,
            parameters,
            return_type,
            statements,
        })
    }

    fn parse_transition(&mut self) -> Result<Statement, ParseError> {
        if self.consume("when") {
            return Ok(Statement::Transition(Transition {
                target: TransitionTarget::Terminal,
                continuation: None,
                guard: TransitionGuard::When(self.parse_expression()?),
            }));
        }

        if self.check("}") {
            return Ok(Statement::Transition(Transition {
                target: TransitionTarget::Terminal,
                continuation: None,
                guard: TransitionGuard::Always,
            }));
        }

        let target = self.parse_transition_target()?;
        let continuation = if self.consume("->") {
            Some(self.parse_transition_target()?)
        } else {
            None
        };
        let guard = if self.consume("when") {
            let guard = TransitionGuard::When(self.parse_expression()?);
            let _ = self.consume(";");
            guard
        } else {
            self.expect(";")?;
            TransitionGuard::Always
        };

        Ok(Statement::Transition(Transition {
            target,
            continuation,
            guard,
        }))
    }

    fn parse_transition_target(&mut self) -> Result<TransitionTarget, ParseError> {
        if self.consume("self") {
            if !self.check(".") {
                return Ok(TransitionTarget::SelfTarget);
            }

            let mut path = vec![String::from("self")];

            while self.consume(".") {
                path.push(self.expect_identifier()?);
            }

            let arguments = if self.consume("(") {
                self.parse_arguments_after_open_paren()?
            } else {
                Vec::new()
            };

            return Ok(TransitionTarget::Named { path, arguments });
        }

        let mut path = vec![self.expect_identifier()?];

        while self.consume(".") {
            path.push(self.expect_identifier()?);
        }

        let arguments = if self.consume("(") {
            self.parse_arguments_after_open_paren()?
        } else {
            Vec::new()
        };

        Ok(TransitionTarget::Named { path, arguments })
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        if self.consume("let") {
            return self.parse_local_data();
        }

        if self.check_kind(TokenKind::Integer)
            || self.check_kind(TokenKind::Float)
            || self.check_kind(TokenKind::String)
        {
            let expression = self.parse_expression()?;

            if self.check("}") {
                return Ok(Statement::Expression(expression));
            }

            self.expect(";")?;
            return Ok(Statement::Expression(expression));
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
            return Ok(Statement::Call(Call {
                receiver: None,
                target: first_name,
                arguments,
            }));
        }

        if self.check("}") {
            return Ok(Statement::Expression(Expression::Name(vec![first_name])));
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

        Ok(Statement::Call(Call {
            receiver: Some(first_name),
            target: second_name,
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
        let arguments = self.parse_arguments_after_open_paren()?;
        self.expect(";")?;

        Ok(arguments)
    }

    fn parse_arguments_after_open_paren(&mut self) -> Result<Vec<Expression>, ParseError> {
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

        Ok(arguments)
    }

    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_or_expression()
    }

    fn parse_or_expression(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_and_expression()?;

        while self.consume("||") {
            let right = self.parse_and_expression()?;
            expression = binary_expression(expression, BinaryOperator::Or, right);
        }

        Ok(expression)
    }

    fn parse_and_expression(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_equality_expression()?;

        while self.consume("&&") {
            let right = self.parse_equality_expression()?;
            expression = binary_expression(expression, BinaryOperator::And, right);
        }

        Ok(expression)
    }

    fn parse_equality_expression(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_comparison_expression()?;

        loop {
            let operator = if self.consume("==") {
                BinaryOperator::Equal
            } else if self.consume("!=") {
                BinaryOperator::NotEqual
            } else {
                break;
            };
            let right = self.parse_comparison_expression()?;
            expression = binary_expression(expression, operator, right);
        }

        Ok(expression)
    }

    fn parse_comparison_expression(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_add_expression()?;

        loop {
            let operator = if self.consume("<=") {
                BinaryOperator::LessOrEqual
            } else if self.consume(">=") {
                BinaryOperator::GreaterOrEqual
            } else if self.consume("<") {
                BinaryOperator::Less
            } else if self.consume(">") {
                BinaryOperator::Greater
            } else {
                break;
            };
            let right = self.parse_add_expression()?;
            expression = binary_expression(expression, operator, right);
        }

        Ok(expression)
    }

    fn parse_add_expression(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_primary_expression()?;

        while self.consume("+") {
            let right = self.parse_primary_expression()?;
            expression = binary_expression(expression, BinaryOperator::Add, right);
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
                TokenKind::Float => Ok(Expression::Float(token.lexeme.clone())),
                TokenKind::Identifier => {
                    if token.lexeme == "true" {
                        return Ok(Expression::Boolean(true));
                    }

                    if token.lexeme == "false" {
                        return Ok(Expression::Boolean(false));
                    }

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

    fn skip_balanced_parentheses_after_open(&mut self) -> Result<usize, ParseError> {
        let mut depth = 1usize;
        let mut token_count = 0usize;

        while depth > 0 {
            let Some(token) = self.advance() else {
                return Err(self.error_here("unterminated parenthesized value"));
            };

            if token.lexeme == "(" {
                depth += 1;
            } else if token.lexeme == ")" {
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

    fn check(&self, lexeme: &str) -> bool {
        self.peek().is_some_and(|token| token.lexeme == lexeme)
    }

    fn check_kind(&self, kind: TokenKind) -> bool {
        self.peek().is_some_and(|token| token.kind == kind)
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

fn binary_expression(left: Expression, operator: BinaryOperator, right: Expression) -> Expression {
    Expression::Binary(Box::new(BinaryExpression {
        left,
        operator,
        right,
    }))
}
