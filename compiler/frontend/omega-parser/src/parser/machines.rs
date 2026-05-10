use super::*;

impl Parser<'_, '_> {
    pub(super) fn parse_machine(&mut self) -> Result<Machine, ParseError> {
        let (name, entry_name, machine_return_type) = if self.consume("for") {
            (self.expect_identifier()?, None, None)
        } else {
            let path = self.parse_path()?;
            let machine_return_type = self.parse_optional_return_type()?;

            if path.len() > 1 {
                let target_name = Identifier::generated(
                    path.as_slice()[..path.len() - 1]
                        .iter()
                        .map(|member: &Identifier| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::"),
                );
                let entry_name = path
                    .as_slice()
                    .last()
                    .cloned()
                    .expect("machine path with multiple members should have a tail");
                (target_name, Some(entry_name), machine_return_type)
            } else {
                let machine_name = path
                    .as_slice()
                    .first()
                    .cloned()
                    .expect("machine path should contain at least one member");
                (machine_name, None, machine_return_type)
            }
        };
        self.expect("{")?;

        let mut contains = Vec::new();
        let mut owned_data = Vec::new();
        let mut states = Vec::new();

        while !self.consume("}") {
            if self.consume("contains") {
                contains.push(self.parse_contains()?);
            } else if self.consume("owns") {
                owned_data.push(self.parse_owned_data()?);
            } else if self.consume("pub") {
                self.expect_callable_keyword()?;
                states.push(self.parse_state_with_entry_support()?);
            } else if self.consume_callable_keyword() {
                states.push(self.parse_state_with_entry_support()?);
            } else if self.consume_state_or_fn_keyword() {
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

        if let Some(entry_name) = entry_name {
            if let Some(state) = states.iter_mut().find(|state| state.name == "entry") {
                state.name = entry_name;
            }
        }

        if let Some(return_type) = &machine_return_type {
            for state in &mut states {
                if state.return_type.is_none() {
                    state.return_type = Some(return_type.clone());
                }
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
        self.parse_named_state_body(name)
    }

    fn parse_state_with_entry_support(&mut self) -> Result<State, ParseError> {
        let name = if self.check("(") {
            Identifier::generated("entry")
        } else {
            self.expect_identifier()?
        };

        self.parse_named_state_body(name)
    }

    fn parse_named_state_body(&mut self, name: Identifier) -> Result<State, ParseError> {
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
            } else if self.consume("transition") {
                statements.extend(self.parse_transition_block()?);
            } else if self.consume("match") {
                statements.extend(self.parse_transition_block()?);
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

    fn parse_transition_block(&mut self) -> Result<Vec<Statement>, ParseError> {
        let subject = if self.consume("{") {
            None
        } else {
            let subject = self.parse_transition_subject_expression()?;
            self.expect("{")?;
            Some(subject)
        };

        let mut statements = Vec::new();

        while !self.consume("}") {
            let guard = if self.consume("_") {
                TransitionGuard::Always
            } else {
                let pattern = self.parse_expression()?;

                if let Some(subject) = &subject {
                    TransitionGuard::When(binary_expression(
                        subject.clone(),
                        BinaryOperator::Equal,
                        pattern,
                    ))
                } else {
                    TransitionGuard::When(pattern)
                }
            };

            self.expect("->")?;

            let target = if self.consume("{") {
                self.expect("}")?;
                TransitionTarget::Terminal
            } else {
                self.parse_transition_target()?
            };

            statements.push(Statement::Transition(Transition {
                target,
                continuation: None,
                guard,
            }));
        }

        Ok(statements)
    }

    fn parse_transition_subject_expression(&mut self) -> Result<Expression, ParseError> {
        if self.transition_subject_is_bare_name() {
            let mut path = vec![self.expect_identifier()?];

            while self.consume(".") || self.consume("::") {
                path.push(self.expect_identifier()?);
            }

            return Ok(Expression::Name(path.into()));
        }

        self.parse_expression()
    }

    fn parse_transition_target(&mut self) -> Result<TransitionTarget, ParseError> {
        if self.check_kind(TokenKind::Integer)
            || self.check_kind(TokenKind::Float)
            || self.check_kind(TokenKind::String)
            || self.check("[")
            || self.check("(")
            || self.check("&")
            || self.check("mut")
            || self.check("true")
            || self.check("false")
        {
            return Ok(TransitionTarget::Value(self.parse_expression()?));
        }

        if self.consume("self") {
            if !self.check(".") {
                if self.consume("(") {
                    let arguments = self.parse_arguments_after_open_paren()?;
                    if !arguments.is_empty() {
                        return Err(self.error_here("self transition does not accept arguments"));
                    }
                }

                return Ok(TransitionTarget::SelfTarget);
            }

            let mut path = vec![Identifier::generated("self")];

            while self.consume(".") || self.consume("::") {
                path.push(self.expect_identifier()?);
            }

            let arguments = if self.consume("(") {
                self.parse_arguments_after_open_paren()?
            } else {
                Vec::new()
            };

            return Ok(TransitionTarget::Named {
                path: path.into(),
                arguments,
            });
        }

        let mut path = vec![self.expect_identifier()?];

        while self.consume(".") || self.consume("::") {
            path.push(self.expect_identifier()?);
        }

        let arguments = if self.consume("(") {
            self.parse_arguments_after_open_paren()?
        } else {
            Vec::new()
        };

        Ok(TransitionTarget::Named {
            path: path.into(),
            arguments,
        })
    }
}
