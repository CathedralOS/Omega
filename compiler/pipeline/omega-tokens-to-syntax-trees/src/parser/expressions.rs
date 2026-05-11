use super::*;

impl Parser<'_, '_> {
    pub(super) fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        if self.consume("let") {
            return self.parse_local_data();
        }

        if self.check_kind(TokenKind::IntegerLiteral)
            || self.check_kind(TokenKind::FloatLiteral)
            || self.check_kind(TokenKind::StringLiteral)
        {
            let expression = self.parse_expression()?;

            if self.check("}") {
                return Ok(Statement::Expression(expression));
            }

            self.expect(";")?;
            return Ok(Statement::Expression(expression));
        }

        let mut path = vec![self.expect_value_name_segment()?];

        while self.consume(".") {
            path.push(self.expect_member_name_segment()?);
        }

        if self.brace_starts_struct_literal() && path.len() == 1 {
            let expression = self.parse_struct_literal(
                path.into_iter()
                    .next()
                    .expect("struct literal type path should have one member"),
            )?;

            if self.check("}") {
                return Ok(Statement::Expression(expression));
            }

            self.expect(";")?;
            return Ok(Statement::Expression(expression));
        }

        if self.consume("=") {
            let value = self.parse_expression()?;
            self.expect(";")?;
            return Ok(Statement::Assignment(Assignment {
                target: Expression::Name(path.into()),
                value,
            }));
        }

        if self.check("(") {
            self.expect("(")?;
            let arguments = self.parse_call_arguments()?;
            let (receiver, target) = split_call_path(path);
            return Ok(Statement::Call(Call {
                receiver,
                target,
                arguments,
            }));
        }

        if self.check("}") {
            return Ok(Statement::Expression(Expression::Name(path.into())));
        }

        let target = self.parse_reference_tail(Expression::Name(path.into()))?;

        self.expect("=")?;
        let value = self.parse_expression()?;
        self.expect(";")?;
        Ok(Statement::Assignment(Assignment { target, value }))
    }

    fn parse_local_data(&mut self) -> Result<Statement, ParseError> {
        let name = self.expect_binding_name()?;
        self.expect(":")?;
        let type_reference = self.parse_type_reference()?;
        let initial_value = if self.consume("=") {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.expect(";")?;

        Ok(Statement::LocalData(LocalData {
            name,
            type_reference,
            initial_value,
        }))
    }

    fn parse_call_arguments(&mut self) -> Result<Vec<Expression>, ParseError> {
        let arguments = self.parse_arguments_after_open_paren()?;
        self.expect(";")?;

        Ok(arguments)
    }

    pub(super) fn parse_arguments_after_open_paren(
        &mut self,
    ) -> Result<Vec<Expression>, ParseError> {
        let mut arguments = Vec::new();

        if !self.check(")") {
            loop {
                arguments.push(self.parse_expression()?);

                if !self.consume(",") {
                    break;
                }

                if self.check(")") {
                    break;
                }
            }
        }

        self.expect(")")?;

        Ok(arguments)
    }

    pub(super) fn parse_expression(&mut self) -> Result<Expression, ParseError> {
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
        let mut expression = self.parse_shift_expression()?;

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
            let right = self.parse_shift_expression()?;
            expression = binary_expression(expression, operator, right);
        }

        Ok(expression)
    }

    fn parse_shift_expression(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_add_expression()?;

        loop {
            let operator = if self.consume("<<") {
                BinaryOperator::ShiftLeft
            } else if self.consume(">>") {
                BinaryOperator::ShiftRight
            } else {
                break;
            };
            let right = self.parse_add_expression()?;
            expression = binary_expression(expression, operator, right);
        }

        Ok(expression)
    }

    pub(super) fn parse_add_expression(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_multiply_expression()?;

        loop {
            let operator = if self.consume("+") {
                BinaryOperator::Add
            } else if self.consume("-") {
                BinaryOperator::Subtract
            } else {
                break;
            };
            let right = self.parse_multiply_expression()?;
            expression = binary_expression(expression, operator, right);
        }

        Ok(expression)
    }

    fn parse_multiply_expression(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_postfix_expression()?;

        loop {
            let operator = if self.consume("*") {
                BinaryOperator::Multiply
            } else if self.consume("/") {
                BinaryOperator::Divide
            } else if self.consume("%") {
                BinaryOperator::Modulo
            } else {
                break;
            };
            let right = self.parse_postfix_expression()?;
            expression = binary_expression(expression, operator, right);
        }

        Ok(expression)
    }

    fn parse_postfix_expression(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_primary_expression()?;

        loop {
            if self.consume("[") {
                let index = self.parse_expression()?;
                self.expect("]")?;
                expression = Expression::Indexed(Box::new(IndexedExpression {
                    collection: expression,
                    index,
                }));
                continue;
            }

            if self.consume("(") {
                let arguments = self.parse_arguments_after_open_paren()?;
                expression = expression_to_call(expression, arguments)?;
                continue;
            }

            if self.consume(".") || self.consume("::") {
                let member = self.expect_member_name_segment()?;

                if self.consume("(") {
                    let arguments = self.parse_arguments_after_open_paren()?;
                    expression = Expression::Call(Box::new(CallExpression {
                        receiver: Some(Box::new(expression)),
                        target: member,
                        arguments,
                    }));
                } else {
                    expression = match expression {
                        Expression::Name(mut path) => {
                            path.push(member);
                            Expression::Name(path)
                        }
                        other => Expression::Member(Box::new(
                            omega_syntax_trees::expression::MemberExpression {
                                receiver: other,
                                member,
                            },
                        )),
                    };
                }

                continue;
            }

            if self.consume("as") {
                let target_type = self.parse_path()?;
                expression =
                    Expression::Cast(Box::new(omega_syntax_trees::expression::CastExpression {
                        value: expression,
                        target_type,
                    }));
                continue;
            }

            break;
        }

        Ok(expression)
    }

    fn parse_primary_expression(&mut self) -> Result<Expression, ParseError> {
        if self.consume("&") {
            let is_mutable = self.consume("mut");
            let expression = self.parse_expression()?;
            return if is_mutable {
                Ok(Expression::Mutable(Box::new(expression)))
            } else {
                Ok(expression)
            };
        }

        if self.consume("mut") {
            return Ok(Expression::Mutable(Box::new(self.parse_expression()?)));
        }

        if self.consume("[") {
            return self.parse_array_literal();
        }

        if self.consume("(") {
            let expression = self.parse_expression()?;
            self.expect(")")?;
            return Ok(expression);
        }

        let file_id = self.file_id;
        if let Some(token) = self.advance() {
            match token.kind {
                TokenKind::IntegerLiteral => token
                    .lexeme
                    .as_str()
                    .parse::<i64>()
                    .map(Expression::Integer)
                    .map_err(|_| ParseError::at_span("invalid integer literal", token.span)),
                TokenKind::FloatLiteral => {
                    Ok(Expression::Float(source_text_from_token(file_id, token)))
                }
                TokenKind::Identifier => {
                    let mut path = vec![identifier_from_token(file_id, token)];

                    while self.consume(".") || self.consume("::") {
                        path.push(self.expect_member_name_segment()?);
                    }
                    if self.brace_starts_struct_literal() && path.len() == 1 {
                        self.parse_struct_literal(
                            path.into_iter()
                                .next()
                                .expect("struct literal type path should have one member"),
                        )
                    } else {
                        Ok(Expression::Name(path.into()))
                    }
                }
                TokenKind::Keyword(KeywordKind::True) => Ok(Expression::Boolean(true)),
                TokenKind::Keyword(KeywordKind::False) => Ok(Expression::Boolean(false)),
                TokenKind::Keyword(KeywordKind::SelfValue) => Ok(Expression::Name(
                    vec![identifier_from_token(file_id, token)].into(),
                )),
                TokenKind::Keyword(KeywordKind::State) => Ok(Expression::Name(
                    vec![identifier_from_token(file_id, token)].into(),
                )),
                TokenKind::Keyword(KeywordKind::Target) => Ok(Expression::Name(
                    vec![identifier_from_token(file_id, token)].into(),
                )),
                TokenKind::StringLiteral => Ok(Expression::String(SourceText::generated(
                    token.lexeme.as_str(),
                ))),
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
                let member = self.expect_member_name_segment()?;
                expression = match expression {
                    Expression::Name(mut path) => {
                        path.push(member);
                        Expression::Name(path)
                    }
                    other => Expression::Member(Box::new(
                        omega_syntax_trees::expression::MemberExpression {
                            receiver: other,
                            member,
                        },
                    )),
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

    fn parse_struct_literal(&mut self, type_name: Identifier) -> Result<Expression, ParseError> {
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
}

fn split_call_path(mut path: Vec<Identifier>) -> (Option<IdentifierPath>, Identifier) {
    let target = path
        .pop()
        .expect("call path should contain at least one member");
    let receiver = (!path.is_empty()).then(|| IdentifierPath::new(path));
    (receiver, target)
}

fn expression_to_call(
    expression: Expression,
    arguments: Vec<Expression>,
) -> Result<Expression, ParseError> {
    match expression {
        Expression::Name(path) => {
            let mut members = path.as_slice().to_vec();
            let target = members
                .pop()
                .expect("call path should contain at least one member");
            let receiver = (!members.is_empty())
                .then(|| Box::new(Expression::Name(IdentifierPath::new(members))));
            Ok(Expression::Call(Box::new(CallExpression {
                receiver,
                target,
                arguments,
            })))
        }
        other => Err(ParseError::new(format!(
            "cannot call non-name expression `{}`",
            other.display_name()
        ))),
    }
}
