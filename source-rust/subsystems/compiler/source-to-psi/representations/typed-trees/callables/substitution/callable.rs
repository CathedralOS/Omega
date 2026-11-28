use super::{
    CallableApplication, CallableLifetimeOrigin, ScopedCallableLifetime, SymbolHandle, TypedTrees,
    data, lifetime_parameter_identity, signature, types,
};

impl CallableApplication {
    pub fn for_type_parameters<'a>(
        declaration: SymbolHandle,
        bindings: impl IntoIterator<
            Item = (
                &'a crate::source_state::data::TypeParameter,
                types::TypeReferenceHandle,
            ),
        >,
        subject: Option<(SymbolHandle, SymbolHandle)>,
    ) -> Self {
        let mut application = Self {
            region_scope: declaration,
            lifetime_declaration: declaration,
            symbols: subject.into_iter().collect(),
            ..Default::default()
        };
        for (parameter, argument) in bindings {
            application.types.push((parameter.symbol, argument));
            application
                .source_binders
                .push((parameter.name.clone(), parameter.symbol));
        }
        application
    }

    pub(super) fn runtime_contract(
        &mut self,
        program: &mut TypedTrees,
        source: crate::source_state::callable_values::RuntimeCallableContractHandle,
    ) -> Result<crate::source_state::callable_values::RuntimeCallableContractHandle, &'static str>
    {
        if let Some((_, destination)) = self
            .active_runtime_contracts
            .iter()
            .rev()
            .find(|(active, _)| *active == source)
        {
            return Ok(*destination);
        }
        if !program.runtime_callable_contracts.is_valid(source) {
            return Err("callable application lost its runtime contract");
        }
        let declared = program.runtime_callable_contracts.get(source).clone();
        let own_symbols = std::iter::once(declared.signature.symbol)
            .chain(
                program
                    .state_signature_type_parameters(&declared.signature)
                    .iter()
                    .map(|parameter| parameter.symbol),
            )
            .chain(
                program
                    .state_signature_parameters(&declared.signature)
                    .iter()
                    .map(|parameter| parameter.symbol),
            )
            .collect::<Vec<_>>();
        // The callable's formals bind inside the surrounding application. Keep
        // outer substitutions, but give these formals their own fresh frame.
        let previous_types = std::mem::take(&mut self.types);
        self.types.extend(
            previous_types
                .iter()
                .copied()
                .filter(|(symbol, _)| !own_symbols.contains(symbol)),
        );
        let previous_static = std::mem::take(&mut self.static_arguments);
        self.static_arguments.extend(
            previous_static
                .iter()
                .filter(|(symbol, _)| !own_symbols.contains(symbol))
                .cloned(),
        );
        let previous_symbols = std::mem::take(&mut self.symbols);
        self.symbols.extend(
            previous_symbols
                .iter()
                .copied()
                .filter(|(symbol, _)| !own_symbols.contains(symbol)),
        );
        let previous_runtime = std::mem::take(&mut self.runtime_values);
        self.runtime_values.extend(
            previous_runtime
                .iter()
                .copied()
                .filter(|symbol| !own_symbols.contains(symbol)),
        );
        let previous_owned = self.owned_signature;
        let owner = program.symbols.insert_generated_root_from(
            declared.signature.symbol,
            symbols::SymbolKind::CallableContract,
            "$callable",
        );
        self.symbols.push((declared.signature.symbol, owner));
        self.owned_signature = owner;
        let destination = program.runtime_callable_contracts.insert(
            crate::source_state::callable_values::RuntimeCallableContract {
                lexical_owner: self.symbol(declared.lexical_owner),
                ..Default::default()
            },
        );
        self.active_runtime_contracts.push((source, destination));
        let applied = self.signature(program, &declared.signature);
        self.active_runtime_contracts.pop();
        self.types = previous_types;
        self.static_arguments = previous_static;
        self.symbols = previous_symbols;
        self.runtime_values = previous_runtime;
        self.owned_signature = previous_owned;
        program
            .runtime_callable_contracts
            .get_mut(destination)
            .signature = applied?;
        Ok(destination)
    }

    pub(super) fn invocation_region(
        &self,
        program: &mut TypedTrees,
    ) -> Result<ScopedCallableLifetime, &'static str> {
        let (owner, position, elision) = self
            .invocation_elision
            .as_ref()
            .ok_or("invocation has no elision scope")?;
        let ordinal = elision
            .region()
            .ok_or("invocation result has ambiguous lifetime elision")?;
        if !program
            .machines()
            .get(*position)
            .is_some_and(|machine| machine.symbol == *owner)
        {
            return Err("invocation lost its unique lifetime declaration");
        }
        if program.machines().at(*position).lifetime_parameters.len() == ordinal {
            program.machine_mut(*position).lifetime_parameters.push(
                crate::source_state::name::Identifier::generated(format!(
                    "@invocation-region-{ordinal}"
                )),
            );
        }
        let name = program
            .machines()
            .at(*position)
            .lifetime_parameters
            .get(ordinal)
            .cloned()
            .ok_or("invocation elision skipped a declaration ordinal")?;
        Ok(ScopedCallableLifetime {
            name,
            region: crate::source_state::type_identity::LifetimeParameterIdentity {
                declaration: *owner,
                ordinal,
            },
            origin: CallableLifetimeOrigin::Lexical,
        })
    }

    pub(super) fn scoped_lifetime(
        &mut self,
        program: &TypedTrees,
        reference: types::TypeReferenceHandle,
        slot: usize,
        original: &crate::source_state::name::Identifier,
    ) -> Result<ScopedCallableLifetime, &'static str> {
        if let Some(site) = self.specialized_regions.front()
            && site.reference == reference
            && site.slot == slot
        {
            let site = self.specialized_regions.pop_front().unwrap();
            return Ok(ScopedCallableLifetime {
                name: site.name,
                region: site.region,
                origin: CallableLifetimeOrigin::Specialized,
            });
        }
        if let Some(region) = program
            .type_reference_table
            .operand_lifetimes(reference)
            .get(slot)
            .copied()
            .filter(|region| region.declaration.is_valid())
        {
            if let Some(binding) = self.lifetimes.iter().rev().find(|binding| {
                lifetime_parameter_identity(program, self.region_scope, binding.parameter.as_str())
                    == Some(region)
            }) {
                return Ok(ScopedCallableLifetime {
                    name: binding.argument.clone(),
                    region: binding.region,
                    origin: binding.origin,
                });
            }
            let (region, substituted) =
                if let Some((environment, preceding_frames)) = &self.trait_region_application {
                    environment
                        .resolved_lifetime_identity(program, region, *preceding_frames)
                        .ok_or("resolved callable region lost its exact supplying frame")?
                } else {
                    (region, false)
                };
            let name = crate::source_state::type_identity::lifetime_parameter_name(program, region)
                .ok_or("resolved callable region lost its declaration ordinal")?
                .clone();
            return Ok(ScopedCallableLifetime {
                name,
                region,
                origin: if substituted {
                    CallableLifetimeOrigin::Captured
                } else {
                    CallableLifetimeOrigin::Lexical
                },
            });
        }
        if let Some(binding) = self
            .lifetimes
            .iter()
            .rev()
            .find(|binding| binding.parameter == *original)
        {
            return Ok(ScopedCallableLifetime {
                name: binding.argument.clone(),
                region: binding.region,
                origin: binding.origin,
            });
        }
        let Some((environment, preceding_frames)) = &self.trait_region_application else {
            return Ok(ScopedCallableLifetime {
                name: original.clone(),
                region: Default::default(),
                origin: CallableLifetimeOrigin::Lexical,
            });
        };
        let (region, substituted) = environment
            .lifetime_identity(
                program,
                reference,
                *preceding_frames,
                original.as_str(),
                self.region_scope,
            )
            .ok_or("applied trait operand has no exact supplying region")?;
        let name = crate::source_state::type_identity::lifetime_parameter_name(program, region)
            .ok_or("applied trait operand lost its region declaration ordinal")?
            .clone();
        Ok(ScopedCallableLifetime {
            name,
            region,
            origin: if substituted {
                CallableLifetimeOrigin::Captured
            } else {
                CallableLifetimeOrigin::Lexical
            },
        })
    }

    pub(crate) fn lifetime(
        &self,
        original: &crate::source_state::name::Identifier,
    ) -> crate::source_state::name::Identifier {
        self.lifetimes
            .iter()
            .rev()
            .find(|binding| binding.parameter == *original)
            .map_or_else(|| original.clone(), |binding| binding.argument.clone())
    }

    pub(crate) fn symbol(&self, original: SymbolHandle) -> SymbolHandle {
        self.symbols
            .iter()
            .rev()
            .find(|(symbol, _)| *symbol == original)
            .map(|(_, replacement)| *replacement)
            .or_else(|| self.declaration_symbols.execution_symbol(original))
            .unwrap_or(original)
    }

    pub(super) fn nominal_contract(
        &mut self,
        program: &mut TypedTrees,
        selected_trait: SymbolHandle,
        requirement: SymbolHandle,
        mut arguments: Vec<types::TypeReferenceHandle>,
    ) -> Result<data::MachineParameterContract, &'static str> {
        use crate::source_state::trait_definition::{AppliedTraitScope, TraitApplicationArguments};

        let receiving_arguments = arguments.clone();
        let (declaration, signature) = program
            .trait_machine_signature(requirement)
            .ok_or("nominal requirement lost its declaring trait")?;
        let declaring_trait = declaration.symbol;
        let mut clauses = Vec::new();
        let environment = if declaring_trait == selected_trait {
            None
        } else {
            let selected = program
                .trait_definition_by_symbol(selected_trait)
                .ok_or("nominal requirement lost its selected trait")?;
            let scope = AppliedTraitScope::new(
                program,
                selected,
                TraitApplicationArguments {
                    lexical_owner: self.region_scope,
                    enclosing_data_applications: &[],
                    arguments: &arguments,
                    lifetimes: &[],
                },
            )
            .ok_or("inherited nominal requirement has invalid selected trait arguments")?;
            let declaring = scope
                .declaring_scope(program, declaring_trait)
                .ok_or("inherited nominal requirement has no unique declaring trait application")?;
            clauses = declaring.callable_refinement_clauses(program, signature.name.as_str());
            Some(declaring.environment())
        };
        let previous_regions = self.trait_region_application.clone();
        if let Some(environment) = &environment {
            arguments = program.materialize_trait_application_arguments(environment)?;
            self.trait_region_application = Some((environment.clone(), environment.frame_count()));
        }
        let applied = if arguments.is_empty() && environment.is_none() {
            Ok(None)
        } else {
            self.nominal(program, declaring_trait, requirement, &arguments)
        };
        self.trait_region_application = previous_regions;
        let mut applied = applied?;
        if let Some(signature) = &mut applied {
            program.apply_callable_refinements(signature, &clauses)?;
        }
        Ok(data::MachineParameterContract::Nominal {
            receiving_trait: selected_trait,
            receiving_arguments,
            trait_definition: declaring_trait,
            requirement,
            trait_arguments: arguments,
            instantiated: applied.map(Box::new),
        })
    }

    pub(super) fn nominal(
        &mut self,
        program: &mut TypedTrees,
        trait_symbol: SymbolHandle,
        requirement: SymbolHandle,
        arguments: &[types::TypeReferenceHandle],
    ) -> Result<Option<signature::StateSignature>, &'static str> {
        let Some(definition) = program.trait_definition_by_symbol(trait_symbol) else {
            return Ok(None);
        };
        let parameters = program
            .trait_type_parameters(definition)
            .iter()
            .map(|parameter| (parameter.name.clone(), parameter.symbol))
            .collect::<Vec<_>>();
        if parameters.len() != arguments.len() {
            return Ok(None);
        }
        let Some(declared) = program
            .trait_machine_signature_by_symbol(definition, requirement)
            .cloned()
        else {
            return Ok(None);
        };
        if self
            .active_requirements
            .contains(&(trait_symbol, requirement))
        {
            return Err(
                "recursive nominal callable application is not yet represented by cycle-aware applied telescopes",
            );
        }
        let previous_types = self.types.len();
        let previous_binders = self.source_binders.len();
        self.source_binders.extend(parameters.iter().cloned());
        self.types.extend(
            parameters
                .into_iter()
                .map(|(_, symbol)| symbol)
                .zip(arguments.iter().copied()),
        );
        self.active_requirements.push((trait_symbol, requirement));
        let applied = self.signature(program, &declared);
        self.active_requirements.pop();
        self.types.truncate(previous_types);
        self.source_binders.truncate(previous_binders);
        applied.map(Some)
    }
}
