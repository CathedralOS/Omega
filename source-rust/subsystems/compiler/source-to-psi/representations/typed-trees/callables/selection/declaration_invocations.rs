use super::{
    AppliedCallableOwner, AppliedCallableSignature, CallableApplication, CallableLifetimeBinding,
    CallableLifetimeOrigin,
};
use crate::source_state::{
    TypedTrees,
    signature::StateSignature,
    static_applications::{
        StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
    },
};
use symbols::SymbolHandle;

impl TypedTrees {
    /// A bodyless declaration publishes a contract, not an executable
    /// copy. Close that contract at its actual source application while
    /// types are still mutable; later readers borrow the retained result.
    pub fn prepare_bodyless_callable_application(
        &mut self,
        application: StaticMachineApplicationHandle,
        target: SymbolHandle,
    ) -> Result<bool, &'static str> {
        let Some((declaration, source)) = self.preparable_bodyless_declaration(target)? else {
            return Ok(false);
        };
        if !self
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return Err("bodyless call has no retained source application");
        }
        let invocation = self.static_machine_application(application).clone();
        if invocation.binding_declaration != declaration
            || !self.same_registered_machine_operation(invocation.registration_operation, target)
            || !self
                .symbols
                .symbols()
                .nodes()
                .is_valid(invocation.lexical_owner)
            || !invocation.input_selections.is_empty()
            || !invocation.selected_machine_arguments.is_empty()
            || invocation.conformance_selection.is_some()
            || invocation.callable_contract_source.is_valid()
        {
            return Err("bodyless call lost its exact declaration or supplying application");
        }
        if self
            .bodyless_callable_signature(application, target)?
            .is_some()
        {
            // The lookup authenticates the complete current occurrence.
            // An unchanged read must not mint new type or region rows.
            return Ok(true);
        }
        let parameters = self.state_signature_type_parameters(&source).to_vec();
        if parameters.iter().any(|parameter| {
            parameter.kind != crate::source_state::data::TypeParameterKind::Type
                || parameter.bounds != crate::source_state::data::DataProperties::default()
        }) || !source.conformance_bounds.is_empty()
        {
            return Err("bodyless call needs its static value, evidence or eligibility judgment");
        }
        let arguments = self
            .static_machine_application_arguments(application)
            .to_vec();
        if arguments.len() != parameters.len() || invocation.arguments.len() != arguments.len() {
            return Err("bodyless call needs exactly one argument per declared type parameter");
        }
        let mut bindings = Vec::with_capacity(parameters.len());
        for (parameter, argument) in parameters.iter().zip(&arguments) {
            let actual =
                self.static_operand_type_reference_in_scope(argument, invocation.lexical_owner)?;
            if !self.type_reference_table.contains_type_reference(actual) {
                return Err("bodyless call type argument has no live retained occurrence");
            }
            bindings.push(StaticMachineBinding {
                parameter: parameter.symbol,
                value: StaticMachineBindingValue::Type(actual),
            });
        }
        let types = bindings
            .iter()
            .map(|binding| match binding.value {
                StaticMachineBindingValue::Type(actual) => actual,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        self.require_declared_application_operands(&types, invocation.lexical_owner)?;
        if self
            .require_owned_application_operands(&types, invocation.lexical_owner)
            .is_err()
        {
            // A declared outer binder is still open. Its generated
            // callers prepare their own exact closed applications.
            return Ok(false);
        }
        let argument_identities = arguments
                .iter()
                .map(|argument| {
                    self.declared_static_operand_identity(
                        crate::source_state::type_identity::DeclaredStaticOperandIdentityRequest {
                            operand: crate::source_state::type_identity::IndexIdentityArgument::StaticArgument(
                                argument,
                            ),
                            lexical_owner: invocation.lexical_owner,
                            binders: &[],
                            lifetime_binders: &[],
                            exact_toolchain_sources: &[],
                        },
                    )
                    .ok_or("bodyless call argument has no complete declared identity")
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
        self.expression_table
            .set_static_machine_application_bindings(application, &bindings);
        let lifetime_rows = self
            .static_application_lifetimes_for_declaration(application, declaration)?
            .to_vec();
        let lifetimes = lifetime_rows
            .iter()
            .map(|binding| {
                let region = crate::source_state::type_identity::lifetime_parameter_identity(
                    self,
                    invocation.lexical_owner,
                    binding.argument.as_str(),
                )
                .ok_or("bodyless call lifetime has no exact supplying declaration")?;
                Ok(CallableLifetimeBinding {
                    parameter: binding.parameter.clone(),
                    argument: binding.argument.clone(),
                    region,
                    origin: CallableLifetimeOrigin::Lexical,
                })
            })
            .collect::<Result<Vec<_>, &'static str>>()?;
        let mut existing = self
            .applied_callable_signatures
            .declaration_rows(application, invocation.lexical_owner);
        let previous = existing.next().map(|(handle, row)| (handle, row.clone()));
        if existing.next().is_some() {
            return Err("bodyless call has more than one prepared declaration contract");
        }
        drop(existing);
        if let Some((_, previous)) = &previous
            && self.bodyless_application_matches(previous, declaration, &source, application)
        {
            return Ok(true);
        }
        let mut declared = source.clone();
        // All original type binders were supplied above, including
        // unused ones; no generic eligibility premise was discarded.
        declared.type_parameters = Default::default();
        let signature = CallableApplication {
            region_scope: declaration,
            lifetime_declaration: source.symbol,
            lifetimes,
            types: parameters
                .iter()
                .zip(types)
                .map(|(parameter, actual)| (parameter.symbol, actual))
                .collect(),
            static_argument_owner: invocation.lexical_owner,
            ..Default::default()
        }
        .signature(self, &declared)?;
        let snapshot = self
            .expression_table
            .snapshot_static_machine_bindings(application);
        let lifetimes = self
            .callable_lifetime_substitutions
            .insert_many(lifetime_rows);
        let arguments = self
            .callable_argument_identities
            .insert_many(argument_identities);
        let prepared = AppliedCallableSignature {
            owner: AppliedCallableOwner::RequirementInvocation {
                declaration,
                application,
                lexical_owner: invocation.lexical_owner,
                source,
                bindings: snapshot,
                lifetimes,
                arguments,
            },
            signature,
        };
        if let Some((previous, _)) = previous {
            self.applied_callable_signatures
                .replace(previous, prepared)?;
        } else {
            self.applied_callable_signatures.insert(prepared);
        }
        Ok(true)
    }

    /// A selected body and its public requirement have distinct binders.
    /// Close the public header from the exact retained body application,
    /// then keep that occurrence beside the ordinary closed requirement.
    pub fn prepare_selected_requirement_application(
        &mut self,
        application: StaticMachineApplicationHandle,
        dispatch: &crate::source_state::StaticRequirementDispatch,
        original: SymbolHandle,
    ) -> Result<(), &'static str> {
        use crate::source_state::static_applications::{
            StaticMachineBinding, StaticMachineBindingValue,
        };
        if !self
            .expression_table
            .static_machine_application_is_valid(application)
            || !self.symbols.symbols().nodes().is_valid(original)
            || !self
                .symbols
                .symbols()
                .nodes()
                .is_valid(dispatch.evidence_parameter)
        {
            return Err("selected public application has stale source coordinates");
        }
        let invocation = self.static_machine_application(application).clone();
        let receipt = self
            .machine_instantiation(invocation.lexical_owner)?
            .ok_or("selected public application lost its supplying instance")?;
        let bound = self
            .evidence_bound_requirement(receipt.template(), dispatch.evidence_parameter, original)
            .map_err(|_| "selected public application lost its exact bound")?
            .ok_or("selected public application has no requirement bound")?;
        if bound.signature.symbol != dispatch.requirement
            || bound.application.declaration.symbol != dispatch.declaring_trait
        {
            return Err("selected public application changed its requirement");
        }
        let source = self
            .applied_callable_signature(&bound.application, bound.signature.symbol)
            .or_else(|| {
                bound
                    .application
                    .keeps_declared_signatures(self)
                    .then_some(bound.signature)
            })
            .ok_or("selected public application lost its applied source header")?
            .clone();
        let environment = bound.application.environment();
        let required = self.state_signature_type_parameters(&source).to_vec();
        if required.is_empty() {
            return Ok(());
        }
        let (_, declaration) = self
            .trait_machine_signature(source.symbol)
            .ok_or("selected public application lost its authored requirement")?;
        let authored = self.state_signature_type_parameters(declaration).to_vec();
        let provider = self
            .machine_by_symbol(invocation.binding_declaration)
            .ok_or("selected public application lost its binding declaration")?;
        if provider.symbol != dispatch.realization_machine {
            return Err("selected public application binds another realization");
        }
        let supplied = self.machine_callable_parameters(provider);
        let mut bindings = self
            .static_machine_application_bindings(application)
            .to_vec();
        if required.len() != authored.len()
            || required.len() != supplied.len()
            || supplied.len() != bindings.len()
            || !source.lifetime_parameters.is_empty()
            || !invocation.lifetime_bindings.is_empty()
        {
            return Err("selected public application needs its complete invocation telescope");
        }
        bindings.sort_unstable_by_key(|binding| {
            (
                binding.parameter.arena_index(),
                binding.parameter.generation(),
            )
        });
        if bindings
            .windows(2)
            .any(|pair| pair[0].parameter == pair[1].parameter)
        {
            return Err("selected realization repeats an invocation binder");
        }
        let mut public_bindings = Vec::with_capacity(required.len());
        let mut public_arguments = Vec::with_capacity(required.len());
        for ((required, authored), supplied) in required.iter().zip(authored).zip(supplied) {
            // A `const` or value binder; the conformance check already
            // matched the requirement's binder kinds with the member's.
            let (crate::source_state::data::TypeParameterKind::Const {
                type_reference: expected,
            }
            | crate::source_state::data::TypeParameterKind::Value {
                type_reference: expected,
            }) = required.kind
            else {
                return Err("selected public application has an unserved binder kind");
            };
            let (crate::source_state::data::TypeParameterKind::Const {
                type_reference: actual,
            }
            | crate::source_state::data::TypeParameterKind::Value {
                type_reference: actual,
            }) = supplied.kind
            else {
                return Err("selected realization has another binder kind");
            };
            if std::mem::discriminant(&required.kind) != std::mem::discriminant(&supplied.kind) {
                return Err("selected realization has another binder kind");
            }
            let primitive = self
                .primitive_type_reference(expected)
                .filter(|expected| self.primitive_type_reference(actual) == Some(*expected))
                .ok_or("selected realization value binder has another carrier")?;
            let Ok(position) = bindings.binary_search_by_key(
                &(supplied.symbol.arena_index(), supplied.symbol.generation()),
                |binding| {
                    (
                        binding.parameter.arena_index(),
                        binding.parameter.generation(),
                    )
                },
            ) else {
                return Err("selected realization binder has no unique argument");
            };
            let binding = &bindings[position];
            let StaticMachineBindingValue::Const(reference) = binding.value else {
                return Err("selected realization binder has no static value");
            };
            let argument = self.static_operand_from_type_reference(reference)?;
            self.prepare_callable_scalar_argument(primitive, &argument)?;
            public_bindings.push(StaticMachineBinding {
                parameter: authored.symbol,
                value: StaticMachineBindingValue::Const(reference),
            });
            public_arguments.push(argument);
        }
        let mut existing = self
            .applied_callable_signatures
            .declaration_rows(application, invocation.lexical_owner);
        let previous = existing.next().map(|(handle, _)| handle);
        if existing.next().is_some() {
            return Err("selected call has several prepared public applications");
        }
        drop(existing);
        let arguments = self
                .static_machine_application_arguments(application)
                .iter()
                .map(|argument| {
                    self.declared_static_operand_identity(
                        crate::source_state::type_identity::DeclaredStaticOperandIdentityRequest {
                            operand: crate::source_state::type_identity::IndexIdentityArgument::StaticArgument(
                                argument,
                            ),
                            lexical_owner: invocation.lexical_owner,
                            binders: &[],
                            lifetime_binders: &[],
                            exact_toolchain_sources: &[],
                        },
                    )
                    .ok_or("selected call argument has no exact declared identity")
                })
                .collect::<Result<Vec<_>, _>>()?;
        let public = self.expression_table.retain_static_machine_application(
            source.symbol,
            source.name.clone(),
            &public_arguments,
        );
        let row = self.expression_table.static_machine_application_mut(public);
        row.lexical_owner = invocation.lexical_owner;
        row.binding_declaration = source.symbol;
        self.expression_table
            .set_static_machine_application_bindings(public, &public_bindings);
        let trait_arguments = self.materialize_trait_application_arguments(&environment)?;
        let closed = self.instantiate_requirement_application(
            &environment,
            source.symbol,
            &trait_arguments,
            public,
        )?;
        let signature = self
            .closed_requirement_application(closed)
            .ok_or("selected public application lost its closed requirement")?
            .signature
            .clone();
        let bindings = self
            .expression_table
            .snapshot_static_machine_bindings(application);
        let arguments = self.callable_argument_identities.insert_many(arguments);
        let prepared = AppliedCallableSignature {
            owner: AppliedCallableOwner::RequirementInvocation {
                declaration: source.symbol,
                application,
                lexical_owner: invocation.lexical_owner,
                source,
                bindings,
                lifetimes: Default::default(),
                arguments,
            },
            signature,
        };
        if let Some(previous) = previous {
            self.applied_callable_signatures
                .replace(previous, prepared)?;
        } else {
            self.applied_callable_signatures.insert(prepared);
        }
        Ok(())
    }

    pub(crate) fn selected_requirement_invocation_signature(
        &self,
        owner: SymbolHandle,
        application: StaticMachineApplicationHandle,
        source: &StateSignature,
    ) -> Result<Option<&StateSignature>, &'static str> {
        if !application.is_valid() {
            return Ok(None);
        }
        let mut rows = self
            .applied_callable_signatures
            .declaration_rows(application, owner);
        let Some((_, prepared)) = rows.next() else {
            return Ok(None);
        };
        if rows.next().is_some()
            || !self.bodyless_application_matches(prepared, source.symbol, source, application)
        {
            return Err("selected public application changed after preparation");
        }
        Ok(Some(&prepared.signature))
    }

    /// Look up this occurrence, not another call with equal rendered
    /// arguments. Mutated source bindings cannot reuse an earlier header.
    pub fn bodyless_callable_signature(
        &self,
        application: StaticMachineApplicationHandle,
        target: SymbolHandle,
    ) -> Result<Option<&StateSignature>, &'static str> {
        let Some((declaration, source)) = self.preparable_bodyless_declaration(target)? else {
            return Ok(None);
        };
        if !self
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return Err("bodyless call has no live source application");
        }
        let invocation = self.static_machine_application(application);
        if invocation.binding_declaration != declaration
            || !self.same_registered_machine_operation(invocation.registration_operation, target)
            || !self
                .symbols
                .symbols()
                .nodes()
                .is_valid(invocation.lexical_owner)
            || !invocation.input_selections.is_empty()
            || !invocation.selected_machine_arguments.is_empty()
            || invocation.conformance_selection.is_some()
            || invocation.callable_contract_source.is_valid()
        {
            return Err("bodyless call does not belong to its prepared declaration");
        }
        let mut rows = self
            .applied_callable_signatures
            .declaration_rows(application, invocation.lexical_owner);
        let Some((_, prepared)) = rows.next() else {
            return Ok(None);
        };
        if rows.next().is_some() {
            return Err("bodyless call has ambiguous prepared declaration contracts");
        }
        if !self.bodyless_application_matches(prepared, declaration, &source, application) {
            return Err("bodyless call source or bindings changed after contract preparation");
        }
        Ok(Some(&prepared.signature))
    }

    fn bodyless_application_matches(
        &self,
        prepared: &AppliedCallableSignature,
        actual_declaration: SymbolHandle,
        actual_source: &StateSignature,
        actual_application: StaticMachineApplicationHandle,
    ) -> bool {
        let AppliedCallableOwner::RequirementInvocation {
            declaration,
            application,
            lexical_owner,
            source,
            bindings,
            lifetimes,
            arguments,
        } = &prepared.owner
        else {
            return false;
        };
        let invocation = self.static_machine_application(actual_application);
        let actual_arguments = self.static_machine_application_arguments(actual_application);
        let actual_bindings = self.static_machine_application_bindings(actual_application);
        let Some(known_arguments) = self.callable_argument_identities.span(*arguments) else {
            return false;
        };
        let Some(known_lifetimes) = self.callable_lifetime_substitutions.span(*lifetimes) else {
            return false;
        };
        *declaration == actual_declaration
                && *application == actual_application
                && self.symbols.symbols().nodes().is_valid(*lexical_owner)
                && *lexical_owner == invocation.lexical_owner
                && source == actual_source
                && (prepared.signature.symbol == source.symbol
                    || self
                        .closed_requirement_application(prepared.signature.symbol)
                        .is_some_and(|closed| closed.template == source.symbol))
                && bindings.len() == actual_bindings.len()
                && invocation.bindings.len() == actual_bindings.len()
                && source.type_parameters.len() == actual_bindings.len()
                && self
                    .expression_table
                    .static_machine_binding_snapshot(*bindings)
                    == actual_bindings
                && invocation.lifetime_bindings.len() == known_lifetimes.len()
                && known_lifetimes
                    .iter()
                    .eq(self.static_machine_application_lifetimes(actual_application))
                && invocation.arguments.len() == actual_arguments.len()
                && known_arguments.len() == actual_arguments.len()
                && known_arguments
                    .iter()
                    .zip(actual_arguments)
                    .all(|(known, actual)| {
                        self.declared_static_operand_identity(
                            crate::source_state::type_identity::DeclaredStaticOperandIdentityRequest {
                                operand:
                                    crate::source_state::type_identity::IndexIdentityArgument::StaticArgument(
                                        actual,
                                    ),
                                lexical_owner: *lexical_owner,
                                binders: &[],
                                lifetime_binders: &[],
                                exact_toolchain_sources: &[],
                            },
                        ) == Some(*known)
                    })
    }

    pub fn bodyless_declaration(
        &self,
        target: SymbolHandle,
    ) -> Result<Option<(SymbolHandle, StateSignature)>, &'static str> {
        if !self.symbols.symbols().nodes().is_valid(target) {
            return Err("bodyless call target has no live declaration");
        }
        let Some(machine) = self
            .machine_by_symbol(target)
            .or_else(|| self.machine_holding_state(target))
        else {
            return Ok(None);
        };
        if machine.supply_mode != source_declarations::MachineSupplyMode::Boundary
            || !machine.has_bodyless_signature()
        {
            return Ok(None);
        }
        self.bodyless_machine_signature_for(machine, target)
            .map(Some)
    }

    /// Close the complete header of an imported boundary or a compiler-supplied
    /// native borrow. Compiler-provided operations never become provider slots.
    pub fn preparable_bodyless_declaration(
        &self,
        target: SymbolHandle,
    ) -> Result<Option<(SymbolHandle, StateSignature)>, &'static str> {
        if let Some(source) = self.bodyless_declaration(target)? {
            return Ok(Some(source));
        }
        let Some(machine) = self
            .machine_by_symbol(target)
            .or_else(|| self.machine_holding_state(target))
        else {
            return Ok(None);
        };
        if !machine.has_bodyless_signature()
            || !matches!(
                self.symbols
                    .sealed_declaration(machine.symbol, machine.name.as_str()),
                Some(
                    language_core::sealed_intrinsics::SealedIntrinsic::CompilerPrimitive(
                        language_core::sealed_intrinsics::CompilerPrimitive::NativeBorrow(_)
                    )
                )
            )
        {
            return Ok(None);
        }
        self.compiler_primitive_schema(machine.symbol)
            .map_err(|_| "invalid native borrow declaration")?
            .ok_or("native borrow has no checked declaration")?;
        self.bodyless_machine_signature_for(machine, target)
            .map(Some)
    }

    /// Read the complete actual machine header without granting call
    /// eligibility. Boundary lookup and sealed intrinsic lookup establish
    /// their own permitted supply roles before using this shared header.
    pub fn bodyless_machine_signature(
        &self,
        target: SymbolHandle,
    ) -> Result<Option<(SymbolHandle, StateSignature)>, &'static str> {
        if !self.symbols.symbols().nodes().is_valid(target) {
            return Err("bodyless call target has no live declaration");
        }
        let Some(machine) = self
            .machine_by_symbol(target)
            .or_else(|| self.machine_holding_state(target))
        else {
            return Ok(None);
        };
        if !machine.has_bodyless_signature() {
            return Ok(None);
        }
        self.bodyless_machine_signature_for(machine, target)
            .map(Some)
    }

    fn bodyless_machine_signature_for(
        &self,
        machine: &crate::source_state::machine::Machine,
        target: SymbolHandle,
    ) -> Result<(SymbolHandle, StateSignature), &'static str> {
        let states = self
            .machine_states
            .span(machine.states)
            .ok_or("bodyless declaration has a stale state roster")?;
        let Some([state]) = states.as_array() else {
            return Err("bodyless declaration needs one exact entry state");
        };
        if (target != machine.symbol && target != state.symbol)
            || !state.statement_nodes.is_empty()
            || !state.contracts.is_empty()
            || machine.structural_type_equations_pending
            || !machine.declared_type_equations.is_empty()
        {
            return Err("bodyless declaration needs its complete entry and structural contract");
        }
        if self
            .data_type_parameters
            .span(machine.type_parameters)
            .is_none()
            || self.state_parameters.span(state.parameters).is_none()
            || self.signature_contracts.span(machine.contracts).is_none()
        {
            return Err("bodyless declaration has a stale parameter or contract roster");
        }
        Ok((
            machine.symbol,
            StateSignature {
                // The machine authored the contracts and effect rows;
                // its generated entry supplies only the runtime telescope.
                // Keeping the entry as owner loses the machine's written
                // reach (and mis-scopes reserved result/region bindings).
                symbol: machine.symbol,
                name: machine.name.clone(),
                spelling: machine.spelling,
                lifetime_parameters: machine.lifetime_parameters.clone(),
                type_parameters: machine.type_parameters,
                conformance_bounds: machine.conformance_bounds.clone(),
                parameters: state.parameters,
                return_type: state.return_type,
                invokes: machine.invokes,
                service_reach_row: machine.service_reach_row,
                service_reach_is_installation_bound: machine.service_reach_is_installation_bound,
                suspends_keyword_source_spans: machine.suspends_keyword_source_spans.clone(),
                blocks_keyword_source_spans: machine.blocks_keyword_source_spans.clone(),
                suspends: machine.suspends,
                blocks: machine.blocks,
                contracts: machine.contracts,
                termination_guarantee: match &machine.termination_plan.interface {
                    source_declarations::TerminationInterface::Published(guarantee) => {
                        guarantee.clone()
                    }
                    _ => source_declarations::TerminationGuarantee::NoGuarantee,
                },
                declares_no_crash_route: machine.declares_no_crash_route,
                ..Default::default()
            },
        ))
    }
}
