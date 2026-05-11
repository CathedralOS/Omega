use super::*;

impl Parser<'_, '_> {
    pub(super) fn parse_items(&mut self) -> Result<Vec<Item>, ParseError> {
        let mut items = Vec::new();

        while !self.is_at_end() {
            if self.consume("use") {
                items.push(Item::Use(self.parse_use()?));
            } else if self.consume("target") {
                items.push(Item::Target(self.parse_target_definition()?));
            } else if self.consume("trust") {
                items.push(Item::TrustDefinition(self.parse_trust_definition()?));
            } else if self.consume("capability") {
                items.push(Item::Capability(self.parse_capability_definition()?));
            } else if self.consume("invariant") {
                items.push(Item::Invariant(self.parse_invariant_definition()?));
            } else if self.consume("library") {
                items.push(Item::Library(self.parse_library_definition()?));
            } else if self.consume("enum") {
                items.push(Item::Data(self.parse_enum_definition()?));
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

        Ok(items)
    }

    fn parse_use(&mut self) -> Result<UseItem, ParseError> {
        let path = self.parse_path()?;
        self.expect(";")?;

        Ok(UseItem { path })
    }

    fn parse_trust_definition(&mut self) -> Result<TrustDefinition, ParseError> {
        let name = self.expect_identifier()?;
        let token_count = self.skip_balanced_braces_with_count()?;

        Ok(TrustDefinition { name, token_count })
    }

    fn parse_library_definition(&mut self) -> Result<LibraryDefinition, ParseError> {
        let (name, path) = if self.check_kind(TokenKind::StringLiteral) {
            (None, self.expect_string_literal()?)
        } else {
            let name = self.expect_identifier()?;
            self.expect("=")?;
            (Some(name), self.expect_string_literal()?)
        };

        self.expect("calling_convention")?;
        let calling_convention = self.expect_identifier()?;
        self.expect("{")?;

        let mut functions = Vec::new();

        while !self.consume("}") {
            self.expect("fn")?;
            functions.push(self.parse_library_function()?);
        }

        Ok(LibraryDefinition {
            name,
            path,
            calling_convention,
            functions,
        })
    }

    fn parse_library_function(&mut self) -> Result<LibraryFunction, ParseError> {
        let signature = self.parse_state_signature()?;
        let mut symbol = None;
        let mut calling_convention = None;
        let mut trusts = Vec::new();

        while !self.check("fn") && !self.check("}") && !self.is_at_end() {
            if self.consume("trust") {
                trusts.push(self.parse_trust_level()?);
                let _ = self.consume(";");
            } else if self.consume("symbol") {
                symbol = Some(self.expect_string_literal()?);
                let _ = self.consume(";");
            } else if self.consume("calling_convention") {
                calling_convention = Some(self.expect_identifier()?);
                let _ = self.consume(";");
            } else if self.consume(";") {
                continue;
            } else {
                return Err(self.error_here("expected library function binding item"));
            }
        }

        Ok(LibraryFunction {
            signature,
            symbol,
            calling_convention,
            trusts,
        })
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
            if self.consume_state_or_fn_keyword() {
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
        let provider = self.parse_path()?;
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
        let path = if self.consume("host") {
            IdentifierPath::new(vec![Identifier::generated("host")])
        } else {
            self.parse_path()?
        };

        Ok(TrustPolicy { mode, path })
    }

    fn parse_capability_state(&mut self) -> Result<CapabilityState, ParseError> {
        let signature = self.parse_state_signature()?;
        let mut contracts = Vec::new();

        if self.consume("where") {
            self.expect("{")?;
            while !self.consume("}") {
                contracts.push(self.parse_capability_contract()?);
            }

            return Ok(CapabilityState {
                signature,
                contracts,
            });
        }

        if self.consume("{") {
            while !self.consume("}") {
                contracts.push(self.parse_capability_contract()?);
            }

            return Ok(CapabilityState {
                signature,
                contracts,
            });
        }

        while !self.check_state_or_fn_keyword() && !self.check("}") {
            contracts.push(self.parse_capability_contract()?);
        }

        Ok(CapabilityState {
            signature,
            contracts,
        })
    }

    fn parse_capability_contract(&mut self) -> Result<CapabilityContract, ParseError> {
        if self.consume("requires") {
            Ok(CapabilityContract {
                kind: CapabilityContractKind::Requires,
                token_count: self.skip_capability_contract_tokens(),
            })
        } else if self.consume("ensures") {
            Ok(CapabilityContract {
                kind: CapabilityContractKind::Ensures,
                token_count: self.skip_capability_contract_tokens(),
            })
        } else if self.consume("trust") || self.consume("trusted") {
            let trust_level = self.parse_trust_level()?;
            Ok(CapabilityContract {
                kind: CapabilityContractKind::Trusted(trust_level),
                token_count: 1,
            })
        } else {
            Err(self.error_here("expected capability contract"))
        }
    }

    fn parse_trust_level(&mut self) -> Result<TrustLevel, ParseError> {
        if self.consume("host") {
            return Ok(TrustLevel::Host);
        }

        let name = self.expect_identifier()?;

        if name == "host" {
            Ok(TrustLevel::Host)
        } else {
            Ok(TrustLevel::Named(name))
        }
    }

    pub(super) fn parse_path(&mut self) -> Result<IdentifierPath, ParseError> {
        let mut path = vec![self.expect_path_name_segment()?];

        while self.consume("::") {
            path.push(self.expect_path_name_segment()?);
        }

        Ok(path.into())
    }

    fn skip_capability_contract_tokens(&mut self) -> usize {
        let start = self.index;

        while !self.is_at_end()
            && !self.check("requires")
            && !self.check("ensures")
            && !self.check("trust")
            && !self.check("trusted")
            && !self.check_state_or_fn_keyword()
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
                let initial_value = if self.consume("=") {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                self.expect(";")?;
                members.push(DataMember::Field(DataField {
                    name: member_name,
                    type_reference,
                    initial_value,
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

    fn parse_enum_definition(&mut self) -> Result<DataDefinition, ParseError> {
        let name = self.expect_identifier()?;
        self.expect("{")?;

        let mut members = Vec::new();

        while !self.consume("}") {
            let name = self.expect_identifier()?;
            members.push(DataMember::Variant(DataVariant { name }));

            if !self.check("}") {
                self.expect(",")?;
            }
        }

        Ok(DataDefinition { name, members })
    }

    fn parse_platform(&mut self) -> Result<Platform, ParseError> {
        let name = self.expect_identifier()?;
        self.expect("{")?;

        let mut states = Vec::new();

        while !self.consume("}") {
            self.expect_state_or_fn_keyword()?;
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

    pub(super) fn parse_state_parameters(&mut self) -> Result<Vec<StateParameter>, ParseError> {
        self.expect("(")?;

        let mut parameters = Vec::new();

        if self.consume(")") {
            return Ok(parameters);
        }

        loop {
            let (name, type_reference, is_const, is_mutable, is_self) = if self.check("&")
                && self.tokens.get(self.index + 1).is_some_and(|token| {
                    token.lexeme.as_str() == "self" || token.lexeme.as_str() == "mut"
                }) {
                self.expect("&")?;
                let is_mutable = self.consume("mut");
                self.expect("self")?;
                (
                    Identifier::generated("self"),
                    TypeReference::named("Self"),
                    false,
                    is_mutable,
                    true,
                )
            } else {
                let mut is_mutable = self.consume("mut");
                let name = self.expect_binding_name()?;
                self.expect(":")?;
                let is_const = self.consume("const");
                if self.consume("&") {
                    is_mutable = self.consume("mut");
                }
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

    pub(super) fn parse_optional_return_type(
        &mut self,
    ) -> Result<Option<TypeReference>, ParseError> {
        if self.consume("->") {
            return Ok(Some(self.parse_type_reference()?));
        }

        Ok(None)
    }

    pub(super) fn parse_type_reference(&mut self) -> Result<TypeReference, ParseError> {
        if self.consume("&") {
            let _ = self.consume("mut");
            return self.parse_type_reference();
        }

        if self.consume("(") {
            self.expect(")")?;

            return Ok(TypeReference::Unit);
        }

        if self.consume("[") {
            let element_type = self.parse_type_reference()?;
            if self.consume(";") {
                let length = self.expect_integer_literal()?;
                self.expect("]")?;

                return Ok(TypeReference::FixedArray {
                    element_type: Box::new(element_type),
                    length,
                });
            }

            self.expect("]")?;

            return Ok(TypeReference::Slice {
                element_type: Box::new(element_type),
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
            TypeReference::Named(base_name)
        };

        if self.check("[") {
            type_reference = TypeReference::Constrained {
                base_type: Box::new(type_reference),
                constraints: self.parse_type_constraints()?,
            };
        }

        Ok(type_reference)
    }

    pub(super) fn parse_type_constraints(&mut self) -> Result<Vec<TypeConstraint>, ParseError> {
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

        if name.as_str() == "range" {
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
}
