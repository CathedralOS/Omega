use super::{AppliedCallableOwner, AppliedCallableSignature, CallableApplication};
use crate::source_state::type_identity;
use crate::source_state::{TypedTrees, data, signature, static_applications, types};
use arena::HandleSpan;
use symbols::SymbolHandle;
use type_identity::applied_type_identity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum RequirementLocalArgument {
    Static(types::TypeReferenceHandle),
    RuntimeValue { carrier: types::TypeReferenceHandle },
}

impl Default for RequirementLocalArgument {
    fn default() -> Self {
        Self::Static(types::TypeReferenceHandle::invalid())
    }
}

impl RequirementLocalArgument {
    pub fn type_reference(self) -> types::TypeReferenceHandle {
        match self {
            Self::Static(reference) | Self::RuntimeValue { carrier: reference } => reference,
        }
    }

    pub fn static_operand(self) -> Option<types::TypeReferenceHandle> {
        match self {
            Self::Static(reference) => Some(reference),
            Self::RuntimeValue { .. } => None,
        }
    }

    fn portable_identity(
        self,
        program: &TypedTrees,
        kind: &data::TypeParameterKind,
        binding: &static_applications::StaticMachineBindingValue,
        lexical_owner: SymbolHandle,
        ordinal: usize,
    ) -> Option<type_identity::NormalizedTypeIdentity> {
        if let (
            data::TypeParameterKind::Const { type_reference }
            | data::TypeParameterKind::Value { type_reference },
            static_applications::StaticMachineBindingValue::Const(_),
        ) = (kind, binding)
        {
            let argument = program
                .static_operand_from_type_reference(self.type_reference())
                .ok()?;
            if program.is_closed_aggregate_operand(*type_reference, &argument) {
                let value = program
                    .canonical_aggregate_argument(*type_reference, &argument)
                    .ok()?;
                return Some(
                    program
                        .type_identities
                        .intern(&value.application_identity()?),
                );
            }
        }
        // A selected machine and its entry state invoke the same callable.
        // Keep its complete checked application rather than its type spelling.
        if matches!(
            binding,
            static_applications::StaticMachineBindingValue::Machine(_)
        ) && matches!(kind, data::TypeParameterKind::Machine { contract }
                if !matches!(contract, data::MachineParameterContract::RequirementIdentity))
        {
            type_identity::portable_applied_index_identity(
                program,
                type_identity::IndexIdentityArgument::TypeReference(self.type_reference()),
                lexical_owner,
                &[],
            )
        } else if matches!(
            (kind, binding),
            (
                data::TypeParameterKind::Type,
                static_applications::StaticMachineBindingValue::Type(_)
            )
        ) {
            type_identity::portable_bound_type_argument_identity(
                program,
                self.type_reference(),
                lexical_owner,
                ordinal,
            )
        } else {
            type_identity::portable_applied_type_identity(
                program,
                self.type_reference(),
                lexical_owner,
                &[],
            )
        }
    }
}

#[derive(Clone, Copy)]
pub struct ClosedRequirementApplication<'program> {
    pub declaring_trait: SymbolHandle,
    pub template: SymbolHandle,
    pub trait_application_identity: type_identity::NormalizedTypeIdentity,
    pub portable_trait_application_identity: type_identity::NormalizedTypeIdentity,
    pub portable_local_arguments: arena::RootRows<'program, type_identity::NormalizedTypeIdentity>,
    pub trait_scope: crate::source_state::trait_definition::RetainedTraitScope,
    pub trait_arguments: &'program [types::TypeReferenceHandle],
    pub local_arguments: arena::RootRows<'program, RequirementLocalArgument>,
    pub invocation_contract_parameters: arena::RootRows<'program, data::TypeParameter>,
    pub application: static_applications::StaticMachineApplicationHandle,
    pub signature: &'program signature::StateSignature,
}

impl TypedTrees {
    pub fn closed_requirement_applications(
        &self,
    ) -> impl Iterator<Item = ClosedRequirementApplication<'_>> {
        self.applied_callable_signatures
            .iter()
            .filter_map(|(_, prepared)| self.closed_requirement_record(prepared))
    }

    pub fn closed_requirement_application(
        &self,
        instance: SymbolHandle,
    ) -> Option<ClosedRequirementApplication<'_>> {
        self.closed_requirement_record(self.closed_requirement_signature(instance)?)
    }

    fn closed_requirement_record<'program>(
        &'program self,
        prepared: &'program AppliedCallableSignature,
    ) -> Option<ClosedRequirementApplication<'program>> {
        let AppliedCallableOwner::RequirementInstance {
            declaration,
            template,
            trait_application_identity,
            portable_trait_application_identity,
            portable_local_arguments,
            trait_scope,
            trait_arguments,
            local_arguments,
            invocation_contract_parameters,
            application,
        } = prepared.owner
        else {
            return None;
        };
        Some(ClosedRequirementApplication {
            declaring_trait: declaration,
            template,
            trait_application_identity,
            portable_trait_application_identity,
            portable_local_arguments: self
                .callable_argument_identities
                .span_or_empty(portable_local_arguments),
            trait_scope,
            trait_arguments: self.type_reference_handles(trait_arguments),
            local_arguments: self.callable_arguments.span_or_empty(local_arguments),
            invocation_contract_parameters: self
                .data_type_parameters
                .span_or_empty(invocation_contract_parameters),
            application,
            signature: &prepared.signature,
        })
    }

    pub fn requirement_application_matches_scope<'program>(
        &'program self,
        requirement: SymbolHandle,
        scope: &crate::source_state::trait_definition::AppliedTraitScope<'program>,
    ) -> bool {
        let Some(application) = self.closed_requirement_application(requirement) else {
            return false;
        };
        let Some(declaration) = self.trait_definition_by_symbol(application.declaring_trait) else {
            return false;
        };
        scope
            .declaring_scope(self, declaration.symbol)
            .and_then(|actual| actual.application_identity(self))
            == Some(application.trait_application_identity)
    }

    pub fn requirement_contract_scopes_match(
        &self,
        left: &crate::source_state::trait_definition::AppliedTraitScope<'_>,
        right: &crate::source_state::trait_definition::AppliedTraitScope<'_>,
    ) -> bool {
        left.callable_contract_key(self)
            .zip(right.callable_contract_key(self))
            .is_some_and(|(left, right)| left == right)
    }

    pub fn requirement_application_matches_contract_scope<'program>(
        &'program self,
        requirement: SymbolHandle,
        scope: &crate::source_state::trait_definition::AppliedTraitScope<'program>,
    ) -> bool {
        self.requirement_application_matches_scope(requirement, scope)
            && self
                .closed_requirement_application(requirement)
                .and_then(|application| application.trait_scope.scope(self))
                .is_some_and(|retained| self.requirement_contract_scopes_match(&retained, scope))
    }

    fn requirement_argument_handles_are_live(
        &self,
        argument: &crate::source_state::expression::StaticMachineArgument,
    ) -> bool {
        (!argument.symbol.is_valid() || self.symbols.symbols().nodes().is_valid(argument.symbol))
            && (!argument.type_reference.is_valid()
                || self.contains_type_reference(argument.type_reference))
    }

    fn requirement_application_bindings_match(
        &self,
        left_application: static_applications::StaticMachineApplicationHandle,
        right_application: static_applications::StaticMachineApplicationHandle,
    ) -> bool {
        let lexical_owner = self
            .static_machine_application(right_application)
            .lexical_owner;
        let bindings = self.static_machine_application_bindings(right_application);
        self.static_machine_application(left_application)
            .lifetime_binding_declaration
            == self
                .static_machine_application(right_application)
                .lifetime_binding_declaration
            && self.static_machine_application_lifetimes(left_application)
                == self.static_machine_application_lifetimes(right_application)
            && (self
                .static_machine_application_lifetimes(right_application)
                .is_empty()
                || self
                    .static_machine_application(left_application)
                    .lexical_owner
                    == lexical_owner)
            && self
                .static_machine_application_bindings(left_application)
                .len()
                == bindings.len()
            && self
                .static_machine_application_bindings(left_application)
                .iter()
                .zip(bindings)
                .enumerate()
                .all(|(ordinal, (left, right))| {
                    let left_parameter = left.parameter;
                    let left_value = &left.value;
                    left.parameter == right.parameter
                        && match (&left.value, &right.value) {
                            (
                                static_applications::StaticMachineBindingValue::Type(left),
                                static_applications::StaticMachineBindingValue::Type(right),
                            ) => {
                                let existing_owner = self
                                    .static_machine_application(left_application)
                                    .lexical_owner;
                                applied_type_identity(self, *left, existing_owner, &[])
                                    .zip(applied_type_identity(self, *right, lexical_owner, &[]))
                                    .is_some_and(|(left, right)| left == right)
                            }
                            (
                                static_applications::StaticMachineBindingValue::Const(
                                    left_reference,
                                ),
                                static_applications::StaticMachineBindingValue::Const(
                                    right_reference,
                                ),
                            ) => self
                                .data_type_parameter_by_symbol(left_parameter)
                                .is_some_and(|parameter| {
                                    let identity = |reference, bound: &_, owner| {
                                        RequirementLocalArgument::Static(reference)
                                            .portable_identity(
                                                self,
                                                &parameter.kind,
                                                bound,
                                                owner,
                                                ordinal,
                                            )
                                    };
                                    identity(
                                        *left_reference,
                                        &left.value,
                                        self.static_machine_application(left_application)
                                            .lexical_owner,
                                    )
                                    .zip(identity(*right_reference, &right.value, lexical_owner))
                                    .is_some_and(|(left, right)| left == right)
                                }),
                            (
                                static_applications::StaticMachineBindingValue::RuntimeValue {
                                    type_reference: left,
                                    ..
                                },
                                static_applications::StaticMachineBindingValue::RuntimeValue {
                                    type_reference: right,
                                    ..
                                },
                            ) => applied_type_identity(
                                self,
                                *left,
                                self.static_machine_application(left_application)
                                    .lexical_owner,
                                &[],
                            )
                            .zip(applied_type_identity(self, *right, lexical_owner, &[]))
                            .is_some_and(|(left, right)| left == right),
                            // Without this arm two calls selecting one operand
                            // each get their own instance under one canonical
                            // identity, which lowering rejects as a duplicate.
                            (
                                static_applications::StaticMachineBindingValue::Machine(_),
                                static_applications::StaticMachineBindingValue::Machine(right),
                            ) => self
                                .data_type_parameter_by_symbol(left_parameter)
                                .is_some_and(|parameter| {
                                    self.static_binding_matches_argument(
                                        &parameter.kind,
                                        left_value,
                                        right,
                                        lexical_owner,
                                        self.static_machine_application(left_application)
                                            .lexical_owner,
                                    )
                                }),
                            (
                                static_applications::StaticMachineBindingValue::Conformance(left),
                                static_applications::StaticMachineBindingValue::Conformance(right),
                            ) => self.same_static_machine_operand(
                                left,
                                self.static_machine_application(left_application)
                                    .lexical_owner,
                                right,
                                lexical_owner,
                            ),
                            (left, right) => left == right,
                        }
                })
    }

    /// Authenticate the retained application of a closed requirement call.
    /// Equivalent source applications may share one closed signature, so
    /// application handles alone do not identify the supplied argument tuple.
    pub fn require_closed_requirement_call_application(
        &self,
        instance: SymbolHandle,
        application: static_applications::StaticMachineApplicationHandle,
    ) -> Result<(), &'static str> {
        let table = &self.expression_table;
        if !self.symbols.symbols().nodes().is_valid(instance)
            || !table.static_machine_application_is_valid(application)
        {
            return Err("closed requirement call has a stale declaration or application");
        }
        let closed = self
            .closed_requirement_application(instance)
            .ok_or("closed requirement call has no prepared signature")?;
        if !table.static_machine_application_is_valid(closed.application) {
            return Err("closed requirement call has a stale prepared application");
        }
        if !self.symbols.symbols().nodes().is_valid(closed.template)
            || !self
                .symbols
                .symbols()
                .nodes()
                .is_valid(closed.declaring_trait)
        {
            return Err("closed requirement call lost its declaration owners");
        }
        let actual = table.static_machine_application(application);
        if actual.registration_operation != closed.template
            || actual.binding_declaration != closed.template
        {
            return Err("closed requirement call applies another declaration");
        }
        for source in [closed.application, application] {
            let source_row = self.static_machine_application(source);
            if source_row.registration_operation != closed.template
                || source_row.binding_declaration != closed.template
            {
                return Err("closed requirement call applies another declaration");
            }
            if !source_row.selected_machine_arguments.is_empty()
                || !source_row.input_selections.is_empty()
                || source_row.conformance_selection.is_some()
                || source_row.callable_contract_source.is_valid()
            {
                return Err(
                    "closed requirement call has an unrepresented static application feature",
                );
            }
            if source_row.lifetime_binding_declaration.is_valid()
                && !self
                    .symbols
                    .symbols()
                    .nodes()
                    .is_valid(source_row.lifetime_binding_declaration)
            {
                return Err("closed requirement call has a stale lifetime declaration");
            }
            if !self
                .symbols
                .symbols()
                .nodes()
                .is_valid(source_row.lexical_owner)
            {
                return Err("closed requirement call lost its supplying declaration");
            }
            for binding in self.static_machine_application_bindings(source) {
                if !self.symbols.symbols().nodes().is_valid(binding.parameter) {
                    return Err("closed requirement call has a stale local binder");
                }
                use static_applications::StaticMachineBindingValue;
                match &binding.value {
                    StaticMachineBindingValue::Type(reference)
                    | StaticMachineBindingValue::Const(reference) => {
                        if !self.contains_type_reference(*reference) {
                            return Err("closed requirement call has a stale operand input");
                        }
                    }
                    StaticMachineBindingValue::Machine(argument)
                    | StaticMachineBindingValue::Conformance(argument) => {
                        if !self.requirement_argument_handles_are_live(argument) {
                            return Err("closed requirement call has a stale operand input");
                        }
                    }
                    StaticMachineBindingValue::Proof(_) => {
                        return Err(
                            "closed requirement call has no logical evidence admission receipt",
                        );
                    }
                    StaticMachineBindingValue::RuntimeValue { .. } => {
                        return Err(
                            "closed requirement call has no runtime value argument admission receipt",
                        );
                    }
                }
            }
            for argument in self.static_machine_application_arguments(source) {
                if !self.requirement_argument_handles_are_live(argument) {
                    return Err("closed requirement call has a stale operand input");
                }
            }
        }
        if closed
            .trait_arguments
            .iter()
            .copied()
            .chain(
                closed
                    .local_arguments
                    .iter()
                    .map(|local| local.type_reference()),
            )
            .any(|reference| !self.contains_type_reference(reference))
        {
            return Err("closed requirement call has a stale operand input");
        }
        let (owner, declared) = self
            .trait_machine_signature(closed.template)
            .filter(|(owner, signature)| {
                owner.symbol == closed.declaring_trait && signature.symbol == closed.template
            })
            .ok_or("closed requirement call lost its authored signature")?;
        let scope = closed
            .trait_scope
            .scope(self)
            .ok_or("closed requirement call lost its declaring trait scope")?;
        if scope.declaration.symbol != owner.symbol
            || scope.application_identity(self) != Some(closed.trait_application_identity)
            || scope.portable_application_identity(self)
                != Some(closed.portable_trait_application_identity)
            || !self.requirement_application_matches_contract_scope(instance, &scope)
        {
            return Err("closed requirement call lost its complete declaring application");
        }
        let parameters = self.state_signature_type_parameters(declared);
        let arguments = self.static_machine_application_arguments(application);
        let bindings = self.static_machine_application_bindings(application);
        let prepared_bindings = self.static_machine_application_bindings(closed.application);
        if parameters.len() != arguments.len()
            || parameters.len() != bindings.len()
            || parameters.len() != prepared_bindings.len()
            || parameters.len() != closed.local_arguments.len()
            || parameters.len() != closed.portable_local_arguments.len()
        {
            return Err("closed requirement call has an incomplete bound argument tuple");
        }
        let prepared_owner = self
            .static_machine_application(closed.application)
            .lexical_owner;
        for (ordinal, parameter) in parameters.iter().enumerate() {
            let binding = &bindings[ordinal];
            let prepared = &prepared_bindings[ordinal];
            if binding.parameter != parameter.symbol || prepared.parameter != parameter.symbol {
                return Err("closed requirement call binds another local parameter");
            }
            if !self.static_binding_matches_argument(
                &parameter.kind,
                &binding.value,
                &arguments[ordinal],
                actual.lexical_owner,
                actual.lexical_owner,
            ) {
                return Err("closed requirement call argument differs from its retained binding");
            }
            if closed.local_arguments[ordinal].portable_identity(
                self,
                &parameter.kind,
                &prepared.value,
                prepared_owner,
                ordinal,
            ) != Some(closed.portable_local_arguments[ordinal])
            {
                return Err("closed requirement call lost its portable local operand identity");
            }
        }
        if !self.requirement_application_bindings_match(closed.application, application) {
            return Err("closed requirement call selects another bound application");
        }
        Ok(())
    }

    pub fn instantiate_requirement_application(
        &mut self,
        environment: &crate::source_state::trait_definition::TraitApplicationEnvironment,
        template: SymbolHandle,
        trait_arguments: &[types::TypeReferenceHandle],
        application: static_applications::StaticMachineApplicationHandle,
    ) -> Result<SymbolHandle, &'static str> {
        if !application.is_valid() {
            return Err("closed requirement has no retained source application");
        }
        let source = self.static_machine_application(application);
        if source.registration_operation != template || source.binding_declaration != template {
            return Err("closed requirement application names a different declaration");
        }
        let lexical_owner = source.lexical_owner;
        let bindings = self
            .static_machine_application_bindings(application)
            .to_vec();
        let local_arguments = bindings
                .iter()
                .map(|binding| match &binding.value {
                    static_applications::StaticMachineBindingValue::Type(actual)
                    | static_applications::StaticMachineBindingValue::Const(actual) => self
                        .static_operand_type_reference_in_scope(
                            &crate::source_state::expression::StaticMachineArgument {
                                type_reference: *actual,
                                ..Default::default()
                            },
                            lexical_owner,
                        )
                        .map(RequirementLocalArgument::Static),
                    static_applications::StaticMachineBindingValue::Machine(actual)
                    | static_applications::StaticMachineBindingValue::Conformance(actual) => self
                        .static_operand_type_reference_in_scope(actual, lexical_owner)
                        .map(RequirementLocalArgument::Static),
                    static_applications::StaticMachineBindingValue::Proof(_) => Err(
                        "logical evidence needs its complete implication, not a static requirement operand",
                    ),
                    static_applications::StaticMachineBindingValue::RuntimeValue {
                        type_reference,
                        ..
                    } => Ok(RequirementLocalArgument::RuntimeValue {
                        carrier: *type_reference,
                    }),
                })
                .collect::<Result<Vec<_>, _>>()?;
        let receiving_scope = environment
            .scope(self)
            .ok_or("closed requirement lost its declaring trait scope")?;
        let declaring_trait = receiving_scope.declaration.symbol;
        let refinements =
            receiving_scope.callable_refinement_clauses(self, self.symbols.name(template));
        self.require_receiving_trait_operands(&receiving_scope)?;
        self.require_owned_application_operands(
            &local_arguments
                .iter()
                .map(|argument| argument.type_reference())
                .collect::<Vec<_>>(),
            lexical_owner,
        )?;
        let trait_application_identity = receiving_scope
            .application_identity(self)
            .ok_or("closed requirement has no complete declaring application identity")?;
        let portable_trait_application_identity = receiving_scope
            .portable_application_identity(self)
            .ok_or("closed requirement has no complete portable declaring application")?;
        let portable_local_arguments = local_arguments
            .iter()
            .zip(&bindings)
            .enumerate()
            .map(|(ordinal, (argument, binding))| {
                let parameter = self
                    .data_type_parameter_by_symbol(binding.parameter)
                    .ok_or("closed requirement lost its declared local parameter")?;
                argument
                    .portable_identity(
                        self,
                        &parameter.kind,
                        &binding.value,
                        lexical_owner,
                        ordinal,
                    )
                    .ok_or("closed requirement has no complete portable local operand")
            })
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(existing) = self.closed_requirement_applications().find(|existing| {
            existing.declaring_trait == declaring_trait
                && existing.template == template
                && self.requirement_application_matches_contract_scope(
                    existing.signature.symbol,
                    &receiving_scope,
                )
                && existing.trait_application_identity == trait_application_identity
                && existing.portable_trait_application_identity
                    == portable_trait_application_identity
                && existing
                    .portable_local_arguments
                    .iter()
                    .eq(portable_local_arguments.iter())
                && existing
                    .local_arguments
                    .iter()
                    .zip(&local_arguments)
                    .all(|(left, right)| {
                        std::mem::discriminant(left) == std::mem::discriminant(right)
                    })
                && self.requirement_application_bindings_match(existing.application, application)
        }) {
            return Ok(existing.signature.symbol);
        }
        let mut declared = self
            .trait_machine_signature(template)
            .filter(|(owner, _)| owner.symbol == declaring_trait)
            .map(|(_, signature)| signature.clone())
            .ok_or("closed requirement lost its authored signature")?;
        self.apply_callable_refinements(&mut declared, &refinements)?;
        let outer_parameters = self
            .trait_definition_by_symbol(declaring_trait)
            .map(|owner| self.trait_type_parameters(owner).to_vec())
            .ok_or("closed requirement lost its declaring trait")?;
        if outer_parameters.len() != trait_arguments.len() {
            return Err("closed requirement has an incomplete trait application");
        }
        let parameters = self.state_signature_type_parameters(&declared).to_vec();
        if parameters.len() != bindings.len()
            || parameters
                .iter()
                .zip(&bindings)
                .any(|(parameter, binding)| parameter.symbol != binding.parameter)
        {
            return Err(
                "closed requirement needs exactly one binding per authored local parameter",
            );
        }
        let mut substitutions = CallableApplication {
            region_scope: template,
            static_argument_owner: lexical_owner,
            trait_region_application: Some((environment.clone(), environment.frame_count())),
            source_binders: outer_parameters
                .iter()
                .map(|parameter| (parameter.name.clone(), parameter.symbol))
                .collect(),
            types: outer_parameters
                .iter()
                .zip(trait_arguments)
                .map(|(parameter, argument)| (parameter.symbol, *argument))
                .collect(),
            ..CallableApplication::default()
        };
        for (parameter, binding) in parameters.iter().zip(&bindings) {
            match (&parameter.kind, &binding.value) {
                (
                    data::TypeParameterKind::Type,
                    static_applications::StaticMachineBindingValue::Type(actual),
                )
                | (
                    data::TypeParameterKind::Const { .. } | data::TypeParameterKind::Value { .. },
                    static_applications::StaticMachineBindingValue::Const(actual),
                ) if actual.is_valid() => {
                    substitutions.types.push((parameter.symbol, *actual));
                }
                (
                    data::TypeParameterKind::Machine { .. },
                    static_applications::StaticMachineBindingValue::Machine(actual),
                )
                | (
                    data::TypeParameterKind::Implementation,
                    static_applications::StaticMachineBindingValue::Conformance(actual),
                ) => {
                    substitutions
                        .static_arguments
                        .push((parameter.symbol, actual.clone()));
                }
                (
                    data::TypeParameterKind::Value { .. },
                    static_applications::StaticMachineBindingValue::RuntimeValue { .. },
                ) => substitutions.runtime_values.push(parameter.symbol),
                _ => {
                    return Err(
                        "closed requirement binding has no represented complete static operand",
                    );
                }
            }
        }
        let instance = self.symbols.insert_generated_root_from(
            template,
            self.symbols.get(template).kind,
            declared.name.as_str(),
        );
        substitutions.symbols.push((template, instance));
        substitutions.owned_signature = instance;
        let mut closed = substitutions.signature(self, &declared)?;
        let invocation_contract_parameters = closed.type_parameters;
        closed.type_parameters = HandleSpan::empty();
        let trait_arguments = self
            .type_reference_table
            .insert_type_reference_handles(trait_arguments.iter().copied());
        let local_arguments = self.callable_arguments.insert_many(local_arguments);
        let trait_scope = environment.retain(self)?;
        let portable_local_arguments = self
            .callable_argument_identities
            .insert_many(portable_local_arguments);
        self.applied_callable_signatures
            .insert(AppliedCallableSignature {
                owner: AppliedCallableOwner::RequirementInstance {
                    declaration: declaring_trait,
                    template,
                    trait_application_identity,
                    portable_trait_application_identity,
                    portable_local_arguments,
                    trait_scope,
                    trait_arguments,
                    local_arguments,
                    invocation_contract_parameters,
                    application,
                },
                signature: closed,
            });
        Ok(instance)
    }
}

#[cfg(test)]
#[path = "../tests/requirement_instances.rs"]
mod tests;
