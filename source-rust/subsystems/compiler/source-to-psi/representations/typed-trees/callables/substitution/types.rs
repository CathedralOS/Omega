use super::{
    CallableApplication, CallableLifetimeBinding, CallableLifetimeOrigin, CopiedCallableBody,
    SymbolHandle, TypedTrees, data, lifetime_parameter_identity, signature, types,
};

impl TypedTrees {
    pub(crate) fn apply_refinement_postconditions(
        &mut self,
        receiving: &mut signature::StateSignature,
        declared: &signature::StateSignature,
        environment: &crate::source_state::trait_definition::TraitApplicationEnvironment,
    ) -> Result<(), &'static str> {
        let parameters = self.state_signature_parameters(declared);
        let actual_parameters = self.state_signature_parameters(receiving);
        let type_parameters = self.state_signature_type_parameters(declared);
        let actual_types = self.state_signature_type_parameters(receiving);
        if parameters.len() != actual_parameters.len()
            || type_parameters.len() != actual_types.len()
            || declared.lifetime_parameters.len() != receiving.lifetime_parameters.len()
        {
            return Err("refinement postcondition lost its inherited callable telescope");
        }
        let symbols = std::iter::once((declared.symbol, receiving.symbol))
            .chain(
                parameters
                    .iter()
                    .zip(actual_parameters)
                    .map(|(formal, actual)| (formal.symbol, actual.symbol)),
            )
            .chain(
                type_parameters
                    .iter()
                    .zip(actual_types)
                    .map(|(formal, actual)| (formal.symbol, actual.symbol)),
            )
            .collect();
        let source_binders = parameters
            .iter()
            .map(|parameter| (parameter.name.clone(), parameter.symbol))
            .chain(
                type_parameters
                    .iter()
                    .map(|parameter| (parameter.name.clone(), parameter.symbol)),
            )
            .collect();
        let lifetimes = declared
            .lifetime_parameters
            .iter()
            .zip(&receiving.lifetime_parameters)
            .enumerate()
            .map(|(ordinal, (parameter, argument))| CallableLifetimeBinding {
                parameter: parameter.clone(),
                argument: argument.clone(),
                region: crate::source_state::type_identity::LifetimeParameterIdentity {
                    declaration: receiving.symbol,
                    ordinal,
                },
                origin: CallableLifetimeOrigin::Rigid,
            })
            .collect();
        let mut application = self.materialize_trait_application(environment)?;
        application.region_scope = declared.symbol;
        application.trait_region_application =
            Some((environment.clone(), environment.frame_count()));
        application.static_argument_owner = environment.argument_owner();
        application.symbols = symbols;
        application.source_binders = source_binders;
        application.lifetimes = lifetimes;
        let contracts = self.state_signature_contracts(receiving).to_vec();
        let additions = self.state_signature_contracts(declared).to_vec();
        receiving.contracts = Default::default();
        for contract in contracts {
            self.signature_contracts
                .append_to_span(&mut receiving.contracts, contract);
        }
        for mut contract in additions {
            if contract.kind != signature::SignatureContractKind::Ensures {
                return Err("refinement postconditions cannot add caller obligations");
            }
            contract.facts = application.facts(self, contract.facts)?;
            self.signature_contracts
                .append_to_span(&mut receiving.contracts, contract);
        }
        Ok(())
    }

    pub fn apply_callable_result_type(
        &mut self,
        original: types::TypeReferenceHandle,
        lifetimes: &[(
            crate::source_state::name::Identifier,
            Option<crate::source_state::name::Identifier>,
        )],
        declaration: SymbolHandle,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        let mut application = CallableApplication {
            region_scope: declaration,
            ..CallableApplication::default()
        };
        for (parameter, argument) in lifetimes {
            if let Some(argument) = argument {
                application.lifetimes.push(CallableLifetimeBinding {
                    parameter: parameter.clone(),
                    argument: argument.clone(),
                    region: Default::default(),
                    origin: CallableLifetimeOrigin::Lexical,
                });
            } else {
                application.elided_lifetimes.push(parameter.clone());
            }
        }
        application.type_reference(self, original)
    }

    pub fn apply_callable_type(
        &mut self,
        original: types::TypeReferenceHandle,
        substitutions: &[(SymbolHandle, types::TypeReferenceHandle)],
        lifetimes: &[(
            crate::source_state::name::Identifier,
            crate::source_state::name::Identifier,
        )],
        declaration: SymbolHandle,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        self.apply_callable_type_in_scope(
            original,
            substitutions,
            lifetimes,
            declaration,
            declaration,
            &[],
        )
    }

    /// The declaration owns the formal regions; the supplier owns actual
    /// regions. Supplied type operands retain their independent existing owner.
    pub fn apply_callable_type_in_scope(
        &mut self,
        original: types::TypeReferenceHandle,
        substitutions: &[(SymbolHandle, types::TypeReferenceHandle)],
        lifetimes: &[(
            crate::source_state::name::Identifier,
            crate::source_state::name::Identifier,
        )],
        declaration: SymbolHandle,
        supplier: SymbolHandle,
        supplied_regions: &[crate::source_state::type_identity::LifetimeParameterIdentity],
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        if !supplied_regions.is_empty() && supplied_regions.len() != lifetimes.len() {
            return Err("applied callable type has an incomplete supplied region roster");
        }
        let lifetimes = lifetimes.iter().enumerate().map(|(ordinal, (parameter, argument))| {
            let region = if supplied_regions.is_empty() {
                lifetime_parameter_identity(self, supplier, argument.as_str())
                    .ok_or("applied callable type region lost its exact supplying declaration")?
            } else {
                let region = supplied_regions[ordinal];
                if crate::source_state::type_identity::lifetime_parameter_name(self, region) != Some(argument) {
                    return Err("applied callable type region differs from its retained declaration ordinal");
                }
                region
            };
            Ok(CallableLifetimeBinding {
                parameter: parameter.clone(), argument: argument.clone(), region,
                origin: CallableLifetimeOrigin::Lexical,
            })
        }).collect::<Result<Vec<_>, &'static str>>()?;
        CallableApplication {
            region_scope: declaration,
            static_argument_owner: supplier,
            types: substitutions.to_vec(),
            lifetimes,
            ..CallableApplication::default()
        }
        .type_reference(self, original)
    }

    /// Close the whole invocation signature in input order. Elided inputs
    /// introduce private caller binders once; the result uses the same policy
    /// as callable schemas, before any recursive value construction occurs.
    pub fn instantiate_invocation_signature(
        &mut self,
        parameters: &[types::TypeReferenceHandle],
        result: types::TypeReferenceHandle,
        substitutions: &[(SymbolHandle, types::TypeReferenceHandle)],
        declaration: crate::source_state::callable_signature_shapes::CallableDeclaration<'_>,
        caller: &mut crate::source_state::machine::Machine,
    ) -> Result<(Vec<types::TypeReferenceHandle>, types::TypeReferenceHandle), &'static str> {
        let declared = declaration.lifetime_parameters(self)?.to_vec();
        if declared != caller.lifetime_parameters {
            return Err("generated caller has a different lifetime telescope");
        }
        let positions = self.machine_positions_for_symbol(caller.symbol);
        let [position] = positions.as_slice() else {
            return Err("invocation lost its unique caller");
        };
        let mut application = CallableApplication {
            region_scope: declaration.symbol(),
            types: substitutions.to_vec(),
            lifetimes: declared
                .iter()
                .enumerate()
                .map(|(ordinal, name)| CallableLifetimeBinding {
                    parameter: name.clone(),
                    argument: name.clone(),
                    region: crate::source_state::type_identity::LifetimeParameterIdentity {
                        declaration: caller.symbol,
                        ordinal,
                    },
                    origin: CallableLifetimeOrigin::Lexical,
                })
                .collect(),
            invocation_elision: Some((
                caller.symbol,
                *position as usize,
                crate::source_state::type_identity::identity_context::SignatureElision::inputs(
                    declared.len(),
                ),
            )),
            ..Default::default()
        };
        let parameters = parameters
            .iter()
            .map(|reference| application.type_reference(self, *reference))
            .collect::<Result<Vec<_>, _>>()?;
        application
            .invocation_elision
            .as_ref()
            .unwrap()
            .2
            .enter_result(declared.len(), 0);
        let result = application.type_reference(self, result)?;
        caller.lifetime_parameters = self
            .machine_by_symbol(caller.symbol)
            .ok_or("invocation lost its current caller")?
            .lifetime_parameters
            .clone();
        Ok((parameters, result))
    }

    /// A selected member's lifetime parameters belong to its enclosing data
    /// application. Apply that tuple before using the member in another scope;
    /// an omitted tuple keeps ordinary lifetime elision.
    pub fn instantiate_data_member_type(
        &mut self,
        original: types::TypeReferenceHandle,
        application: types::TypeReferenceHandle,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        let definition = self
            .closed_data_definition(application)
            .ok_or("input member lost its closed data application")?;
        let declaration = definition.symbol;
        let parameters = definition.lifetime_parameters.clone();
        if parameters.is_empty() {
            // The retained member already has its static arguments applied.
            // With no enclosing lifetimes to bind, keep that occurrence.
            return Ok(original);
        }
        let application = self
            .type_reference_table
            .unconstrained_type_reference(application);
        let supplied = match self.type_reference_node(application) {
            types::TypeReferenceNode::Generic {
                lifetime_arguments, ..
            } => lifetime_arguments.clone(),
            types::TypeReferenceNode::Named { .. } => Vec::new(),
            _ => return Err("input member has no nominal enclosing application"),
        };
        if supplied.is_empty() {
            return self.apply_callable_result_type(
                original,
                &parameters
                    .into_iter()
                    .map(|parameter| (parameter, None))
                    .collect::<Vec<_>>(),
                declaration,
            );
        }
        if supplied.len() != parameters.len() {
            return Err("input member lost its complete enclosing lifetime tuple");
        }
        let owner = self
            .type_reference_table
            .operand_lifetime_owner(application);
        let retained = self.type_reference_table.operand_lifetimes(application);
        let lifetimes = parameters
            .into_iter()
            .zip(supplied)
            .enumerate()
            .map(|(slot, (parameter, argument))| {
                let region = match retained.get(slot).copied() {
                    Some(region) if region.declaration.is_valid() => Some(region),
                    Some(_) => {
                        return Err("input member lifetime still has an unmaterialized elision");
                    }
                    None => lifetime_parameter_identity(self, owner, argument.as_str()),
                }
                .ok_or("input member lifetime lost its actual supplying declaration")?;
                Ok(CallableLifetimeBinding {
                    parameter,
                    argument,
                    region,
                    origin: CallableLifetimeOrigin::Lexical,
                })
            })
            .collect::<Result<Vec<_>, &'static str>>()?;
        CallableApplication {
            region_scope: declaration,
            lifetimes,
            ..CallableApplication::default()
        }
        .type_reference(self, original)
    }

    /// `original` with each type parameter in `substitutions` replaced by
    /// its type, wherever it is written.
    pub fn substitute_type_parameters(
        &mut self,
        original: types::TypeReferenceHandle,
        declaration: SymbolHandle,
        substitutions: &[(SymbolHandle, types::TypeReferenceHandle)],
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        CallableApplication {
            region_scope: declaration,
            types: substitutions.to_vec(),
            ..CallableApplication::default()
        }
        .type_reference(self, original)
    }

    /// The proposition a closed application of `definition`, generic over
    /// types, selects: each binder replaced by its type and each parameter
    /// a fresh symbol, wherever the parameters and the formula name them.
    /// Only a proposition declared as one boolean formula is copied.
    pub fn copy_proposition_application(
        &mut self,
        definition: &crate::source_state::proposition::PropositionDefinition,
        instance: SymbolHandle,
        name: crate::source_state::name::Identifier,
        substitutions: &[(SymbolHandle, types::TypeReferenceHandle)],
    ) -> Result<crate::source_state::proposition::PropositionDefinition, &'static str> {
        use crate::source_state::proposition::{PropositionBody, PropositionFormula};
        let PropositionBody::Transparent {
            proposition: PropositionFormula::BooleanExpression(formula),
        } = &definition.body
        else {
            return Err("a generic proposition is copied only from one boolean formula");
        };
        let mut application = CallableApplication {
            region_scope: definition.symbol,
            types: substitutions.to_vec(),
            ..CallableApplication::default()
        };
        // What the formula writes, its static applications among them, was
        // written in the generated proposition.
        application.symbols.push((definition.symbol, instance));
        let declared = self
            .state_parameters
            .span_or_empty(definition.parameters)
            .to_vec();
        for parameter in &declared {
            let kind = self.symbols.get(parameter.symbol).kind;
            let fresh = self.symbols.insert_generated_root_from(
                parameter.symbol,
                kind,
                parameter.name.as_str(),
            );
            application.symbols.push((parameter.symbol, fresh));
        }
        let mut parameters = arena::HandleSpan::empty();
        for mut parameter in declared {
            parameter.symbol = application.symbol(parameter.symbol);
            parameter.type_reference =
                application.type_reference(self, parameter.type_reference)?;
            self.state_parameters
                .append_to_span(&mut parameters, parameter);
        }
        let formula = application.expression(self, *formula)?;
        Ok(crate::source_state::proposition::PropositionDefinition {
            symbol: instance,
            name,
            is_public: false,
            binders: arena::HandleSpan::empty(),
            parameters,
            transparent_formula_source_span: definition.transparent_formula_source_span,
            body: PropositionBody::Transparent {
                proposition: PropositionFormula::BooleanExpression(formula),
            },
        })
    }

    /// Copies every form of callable contract fact under the same declaration
    /// and static bindings as its entry. No fact becomes checking evidence here.
    pub fn copy_callable_contract(
        &mut self,
        original: &signature::SignatureContract,
        symbols: crate::source_state::SymbolRebinding<'_>,
        substitutions: &[(SymbolHandle, types::TypeReferenceHandle)],
        declaration: SymbolHandle,
    ) -> Result<signature::SignatureContract, &'static str> {
        let mut application = CallableApplication {
            region_scope: declaration,
            types: substitutions.to_vec(),
            symbols: symbols.iter().copied().collect(),
            ..CallableApplication::default()
        };
        let mut copied = original.clone();
        copied.facts = application.facts(self, original.facts)?;
        Ok(copied)
    }

    /// A copied callable keeps its invocation permissions and substituted types.
    /// Header targets, local annotations, patterns, static operands and expression
    /// metadata all use the same declaration map; applying a domain also rebuilds
    /// its semantic identity from the substituted arguments.
    pub fn apply_copied_callable_interface<'state>(
        &mut self,
        entries: impl IntoIterator<
            Item = (
                SymbolHandle,
                &'state mut crate::source_state::machine::Machine,
                &'state mut crate::source_state::state::State,
                &'state CopiedCallableBody,
            ),
        >,
        symbols: crate::source_state::SymbolRebinding<'_>,
        substitutions: &[(SymbolHandle, types::TypeReferenceHandle)],
    ) -> Result<(), &'static str> {
        let mut application = CallableApplication {
            types: substitutions.to_vec(),
            ..CallableApplication::default()
        };
        for &(source, target) in symbols.iter() {
            application
                .declaration_symbols
                .insert_execution(source, target);
        }
        for (declaration, machine, state, body) in entries {
            application.region_scope = declaration;
            machine.service_reach_row =
                application.applied_reach_row(self, machine.service_reach_row)?;
            application.copy_authored_reach(self, declaration, machine.symbol)?;
            machine.invokes = application.invocations(self, machine.invokes)?;
            // Definitions belong to the copied declaration just like its
            // written contract. Apply both sides through the same type copier.
            if !machine.declared_type_equations.is_empty() {
                let original = self
                    .declared_type_equations
                    .span_or_empty(machine.declared_type_equations)
                    .to_vec();
                let mut equations = Vec::with_capacity(original.len());
                for equation in original {
                    equations.push(crate::source_state::machine::DeclaredTypeEquation {
                        parameter: application.type_reference(self, equation.parameter)?,
                        definition: application.type_reference(self, equation.definition)?,
                    });
                }
                machine.declared_type_equations =
                    self.declared_type_equations.insert_many(equations);
            }
            let rows = self
                .expression_table
                .iter_expressions_from(body.first_expression)
                .take(body.expression_count)
                .map(|(handle, _)| handle)
                .collect::<Vec<_>>();
            state.return_type = application.type_reference(self, state.return_type)?;
            for parameter in state.parameters.handles() {
                let original = self.state_parameters.get(parameter).type_reference;
                let applied = application.type_reference(self, original)?;
                self.state_parameters.get_mut(parameter).type_reference = applied;
            }
            for &handle in &body.statements {
                match self.statement_table.statement(handle).clone() {
                    crate::source_state::statement::StatementNode::LocalData(mut local) => {
                        local.type_reference =
                            application.type_reference(self, local.type_reference)?;
                        *self.statement_table.statement_mut(handle) =
                            crate::source_state::statement::StatementNode::LocalData(local);
                    }
                    crate::source_state::statement::StatementNode::Call(mut call) => {
                        if let Some(runtime) = &mut call.runtime_call {
                            runtime.contract =
                                application.runtime_contract(self, runtime.contract)?;
                        }
                        if let Some(request) = &mut call.installed_interrupt_operation {
                            request.prepared = None;
                        }
                        application.static_arguments(self, &mut call.machine_arguments)?;
                        application.apply_call_application(
                            self,
                            crate::source_state::references::StaticArgumentCall::Statement(&call),
                        )?;
                        *self.statement_table.statement_mut(handle) =
                            crate::source_state::statement::StatementNode::Call(call);
                    }
                    _ => {}
                }
            }
            let call_targets = application.expression_call_targets(self, &rows)?;
            application.apply_expression_rows(self, &rows, call_targets)?;
        }
        Ok(())
    }

    pub fn instantiate_nominal_machine_parameter_contracts(
        &mut self,
        first_parameter: usize,
    ) -> Result<(), &'static str> {
        use data::{MachineParameterContract::Nominal, TypeParameterKind::Machine};

        let mut pending = Vec::new();
        for (handle, parameter) in self.data_type_parameters.iter_from(first_parameter) {
            let Machine {
                contract:
                    Nominal {
                        receiving_trait,
                        requirement,
                        receiving_arguments,
                        instantiated: None,
                        ..
                    },
            } = &parameter.kind
            else {
                continue;
            };
            // A lexical helper captures the supplying declaration's binder. Its
            // requirement is the same application only for the same receiver
            // and argument tuple. Find that row before changing the indexed table.
            let earlier = self.with_type_parameter_declarations(parameter.symbol, |rows| {
                rows.iter().copied().take_while(|&row| row != handle).find(|&row| {
                    matches!(&self.data_type_parameters.get(row).kind, Machine {
                        contract: Nominal {
                            receiving_trait: receiver, requirement: member, receiving_arguments: arguments, ..
                        },
                    } if receiver == receiving_trait && member == requirement && arguments == receiving_arguments)
                })
            });
            pending.push((
                handle,
                earlier,
                parameter.symbol,
                *receiving_trait,
                *requirement,
                receiving_arguments.clone(),
            ));
        }
        for (handle, earlier, binder, receiver, requirement, arguments) in pending {
            let kind = earlier.map(|row| &self.data_type_parameters.get(row).kind);
            let kind = if let Some(
                prepared @ Machine {
                    contract:
                        Nominal {
                            instantiated: Some(_),
                            ..
                        },
                },
            ) = kind
            {
                prepared.clone()
            } else {
                let contract = CallableApplication {
                    region_scope: self.declaration_scope_parent(binder),
                    ..CallableApplication::default()
                }
                .nominal_contract(self, receiver, requirement, arguments)?;
                Machine { contract }
            };
            self.data_type_parameters.get_mut(handle).kind = kind;
        }
        Ok(())
    }
}
