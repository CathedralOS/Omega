use crate::parse_error::ParseError;
use omega_abstract_syntax_tree::expression::{
    BinaryExpression, BinaryOperator, Expression, IndexedExpression, StructLiteral,
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
use std::sync::Arc;

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
    parse_file_with_optional_source(file_id, None, tokens)
}

pub fn parse_file_with_source(
    file_id: FileId,
    source: Arc<str>,
    tokens: &[Token<'_>],
) -> Result<AstFile, ParseError> {
    parse_file_with_optional_source(file_id, Some(source), tokens)
}

fn parse_file_with_optional_source(
    file_id: FileId,
    source: Option<Arc<str>>,
    tokens: &[Token<'_>],
) -> Result<AstFile, ParseError> {
    Parser {
        file_id,
        source,
        tokens,
        index: 0,
    }
    .parse_file()
}

struct Parser<'tokens, 'source> {
    file_id: FileId,
    source: Option<Arc<str>>,
    tokens: &'tokens [Token<'source>],
    index: usize,
}

impl Parser<'_, '_> {
    fn parse_file(&mut self) -> Result<AstFile, ParseError> {
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

        merge_machine_items(&mut items);

        let tables = AstTables::from_items(&items);

        Ok(AstFile {
            file_id: self.file_id,
            items,
            tables,
        })
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
        let (name, path) = if self.check_kind(TokenKind::String) {
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
        let path = self.parse_path()?;

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
        let name = self.expect_identifier()?;

        if name == "host" {
            Ok(TrustLevel::Host)
        } else {
            Ok(TrustLevel::Named(name))
        }
    }

    fn parse_path(&mut self) -> Result<IdentifierPath, ParseError> {
        let mut path = vec![self.expect_identifier()?];

        while self.consume("::") {
            path.push(self.expect_identifier()?);
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
                    Identifier::generated("self"),
                    TypeReference::named("Self"),
                    false,
                    true,
                    true,
                )
            } else {
                let mut is_mutable = self.consume("mut");
                let name = self.expect_identifier()?;
                self.expect(":")?;
                let is_const = self.consume("const");
                if self.consume("&") {
                    self.expect("mut")?;
                    is_mutable = true;
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

    fn parse_machine(&mut self) -> Result<Machine, ParseError> {
        let _ = self.consume("for");
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

            while self.consume(".") {
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

        while self.consume(".") {
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
                target: Expression::Name(vec![first_name].into()),
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
            return Ok(Statement::Expression(Expression::Name(
                vec![first_name].into(),
            )));
        }

        self.expect(".")?;
        let second_name = self.expect_identifier()?;

        if !self.check("(") {
            let target =
                self.parse_reference_tail(Expression::Name(vec![first_name, second_name].into()))?;

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

        let file_id = self.file_id;
        let source = self.source.clone();
        if let Some(token) = self.advance() {
            match token.kind {
                TokenKind::Integer => token
                    .lexeme
                    .as_str()
                    .parse::<i64>()
                    .map(Expression::Integer)
                    .map_err(|_| ParseError::at_span("invalid integer literal", token.span)),
                TokenKind::Float => Ok(Expression::Float(source_text_from_token(
                    file_id,
                    source.as_ref(),
                    token,
                ))),
                TokenKind::Identifier => {
                    if token.lexeme.as_str() == "true" {
                        return Ok(Expression::Boolean(true));
                    }

                    if token.lexeme.as_str() == "false" {
                        return Ok(Expression::Boolean(false));
                    }

                    let mut path = vec![identifier_from_token(file_id, source.as_ref(), token)];

                    while self.consume(".") || self.consume("::") {
                        path.push(self.expect_identifier()?);
                    }

                    if self.check("{") && path.len() == 1 {
                        self.parse_struct_literal(
                            path.into_iter()
                                .next()
                                .expect("struct literal type path should have one member"),
                        )
                    } else {
                        self.parse_reference_tail(Expression::Name(path.into()))
                    }
                }
                TokenKind::String => Ok(Expression::String(SourceText::generated(
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
        let source = self.source.clone();
        let Some(token) = self.advance() else {
            return Err(self.error_here("expected identifier"));
        };

        if token.kind == TokenKind::Identifier {
            Ok(identifier_from_token(file_id, source.as_ref(), token))
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

fn merge_machine_items(items: &mut Vec<Item>) {
    let mut merged = Vec::with_capacity(items.len());

    for item in items.drain(..) {
        match item {
            Item::Machine(machine) => {
                if let Some(Item::Machine(existing)) = merged.iter_mut().find(|existing_item| {
                    matches!(existing_item, Item::Machine(existing) if existing.name == machine.name)
                }) {
                    existing.contains.extend(machine.contains);
                    existing.owned_data.extend(machine.owned_data);
                    existing.states.extend(machine.states);
                } else {
                    merged.push(Item::Machine(machine));
                }
            }
            other => merged.push(other),
        }
    }

    *items = merged;
}

fn identifier_from_token(
    file_id: FileId,
    source: Option<&Arc<str>>,
    token: &Token<'_>,
) -> Identifier {
    let source_span = omega_core::source::SourceSpan::new(file_id, token.span);

    if let Some(source) = source {
        Identifier::source(Arc::clone(source), source_span)
    } else {
        Identifier::new(token.lexeme.as_str(), source_span)
    }
}

fn source_text_from_token(
    file_id: FileId,
    source: Option<&Arc<str>>,
    token: &Token<'_>,
) -> SourceText {
    let source_span = omega_core::source::SourceSpan::new(file_id, token.span);

    if let Some(source) = source {
        SourceText::source(Arc::clone(source), source_span)
    } else {
        SourceText::generated(token.lexeme.as_str())
    }
}

fn binary_expression(left: Expression, operator: BinaryOperator, right: Expression) -> Expression {
    Expression::Binary(Box::new(BinaryExpression {
        left,
        operator,
        right,
    }))
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
}
