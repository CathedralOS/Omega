//! Closed requirement applications retain their authored owner and telescope.
//!
//! A trait signature is not a machine body. Its instance borrows the same deep
//! callable application mechanism as an applied checking view, but has a fresh
//! executable symbol and an explicit complete source application. Equal ABI
//! carriers never collapse different tuples. Creation derives representation;
//! immutable checking still admits every operand and substituted predicate.
//! Invocation lifetimes remain universal on that signature. The source call's
//! parameter-position lifetime map describes caller loans, not a new public
//! requirement or permission to capture a region in a specialization operand.

use super::{AppliedCallableOwner, AppliedCallableSignature, CallableApplication};
use crate::{TypedTrees, data, signature, static_applications, types};
use arena::HandleSpan;
use symbols::SymbolHandle;

#[derive(Clone, Copy)]
pub struct ClosedRequirementApplication<'program> {
    pub declaring_trait: SymbolHandle,
    pub template: SymbolHandle,
    /// Complete declaring application, independent of the executable signature.
    pub trait_application_identity: crate::type_identity::NormalizedTypeIdentity,
    pub portable_trait_application_identity: crate::type_identity::NormalizedTypeIdentity,
    pub portable_local_arguments: &'program [crate::type_identity::NormalizedTypeIdentity],
    pub trait_scope: crate::trait_definition::RetainedTraitScope,
    pub trait_arguments: &'program [types::TypeReferenceHandle],
    pub local_arguments: &'program [types::TypeReferenceHandle],
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
            trait_arguments: self
                .type_reference_table
                .type_reference_handles(trait_arguments),
            local_arguments: self
                .type_reference_table
                .type_reference_handles(local_arguments),
            application,
            signature: &prepared.signature,
        })
    }

    /// Whether a checked satisfaction/map scope selects this closed requirement's
    /// exact declaring application. Inherited paths must agree; an equal local
    /// tuple cannot substitute another trait application or phantom argument.
    /// This rejoins derivation data and does not establish conformance authority.
    pub fn requirement_application_matches_scope<'program>(
        &'program self,
        requirement: SymbolHandle,
        scope: &crate::trait_definition::AppliedTraitScope<'program>,
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

    /// Join the complete public invocation to its exact realization receipt.
    /// Declaration-ordered typed bindings retain every type, const and machine
    /// operand; equal ABI carriers cannot donate another tuple.
    pub fn requirement_realization_specialization(
        &self,
        requirement: SymbolHandle,
        realization: SymbolHandle,
        provider: SymbolHandle,
    ) -> Option<&crate::typed_trees::commitments::MachineSpecialization> {
        let application = self.closed_requirement_application(requirement)?;
        let arguments = self
            .expression_table
            .static_machine_application_arguments(application.application);
        let source_owner = self
            .expression_table
            .static_machine_application(application.application)
            .lexical_owner;
        let mut matches = self.machine_specializations.iter().filter(|receipt| {
            if receipt.template != realization
                || receipt.instance != provider
                || !receipt.binding_application.is_valid()
            {
                return false;
            }
            let parameters = self
                .data_type_parameters
                .span_or_empty(receipt.template_parameters);
            let bindings = self
                .expression_table
                .static_machine_application_bindings(receipt.binding_application);
            let binding_owner = self
                .expression_table
                .static_machine_application(receipt.binding_application)
                .lexical_owner;
            let lifetimes = self
                .expression_table
                .static_machine_application_lifetimes(application.application);
            let same_regions = lifetimes
                == self
                    .expression_table
                    .static_machine_application_lifetimes(receipt.binding_application)
                && (lifetimes.is_empty() || source_owner == binding_owner);
            same_regions
                && parameters.len() == arguments.len()
                && bindings.len() == parameters.len()
                && parameters
                    .iter()
                    .zip(arguments)
                    .all(|(parameter, argument)| {
                        let mut actual = bindings
                            .iter()
                            .filter(|binding| binding.parameter == parameter.symbol);
                        let Some(binding) = actual.next() else {
                            return false;
                        };
                        actual.next().is_none()
                            && self.static_binding_matches_argument(
                                &parameter.kind,
                                &binding.value,
                                argument,
                                source_owner,
                                binding_owner,
                            )
                    })
        });
        let receipt = matches.next()?;
        matches.next().is_none().then_some(receipt)
    }

    /// Outer application and local executable substitution are separate axes.
    /// A requirement with no local operands needs its checked map/satisfaction
    /// scope, not a fabricated machine-specialization receipt. Consumers must
    /// separately rejoin that exact scope and original realization declaration.
    pub fn requirement_realization_matches(
        &self,
        requirement: SymbolHandle,
        realization: SymbolHandle,
        provider: SymbolHandle,
    ) -> bool {
        let Some(application) = self.closed_requirement_application(requirement) else {
            return false;
        };
        if application.local_arguments.is_empty() {
            return realization == provider
                && self
                    .machine(provider)
                    .is_some_and(|machine| self.machine_type_parameters(machine).is_empty());
        }
        self.requirement_realization_specialization(requirement, realization, provider)
            .is_some()
    }

    /// Close an exact declaring trait application and complete local telescope.
    /// The supplied row is derivation data, not an admitted selection receipt.
    pub fn instantiate_requirement_application(
        &mut self,
        environment: &crate::trait_definition::TraitApplicationEnvironment,
        template: SymbolHandle,
        trait_arguments: &[types::TypeReferenceHandle],
        application: static_applications::StaticMachineApplicationHandle,
    ) -> Result<SymbolHandle, &'static str> {
        if !application.is_valid() {
            return Err("closed requirement has no retained source application");
        }
        let source = self
            .expression_table
            .static_machine_application(application);
        if source.registration_operation != template || source.binding_declaration != template {
            return Err("closed requirement application names a different declaration");
        }
        let lexical_owner = source.lexical_owner;
        let bindings = self
            .expression_table
            .static_machine_application_bindings(application)
            .to_vec();
        let local_arguments = bindings
            .iter()
            .map(|binding| match &binding.value {
                static_applications::StaticMachineBindingValue::Type(actual)
                | static_applications::StaticMachineBindingValue::Const(actual) => Ok(*actual),
                static_applications::StaticMachineBindingValue::Machine(actual)
                | static_applications::StaticMachineBindingValue::Conformance(actual) => {
                    self.static_operand_type_reference(actual)
                }
                static_applications::StaticMachineBindingValue::RuntimeValue { .. } => {
                    Err("runtime operand does not close a requirement instance")
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let receiving_scope = environment
            .scope(self)
            .ok_or("closed requirement lost its declaring trait scope")?;
        let declaring_trait = receiving_scope.declaration.symbol;
        // Closure consumes the same source environment that defines the key.
        // Discarding it here either rejects retained regions or licenses a
        // later consumer to rebuild a different application from erased types.
        self.require_receiving_trait_operands(&receiving_scope)?;
        self.require_owned_application_operands(&local_arguments, lexical_owner)?;
        let trait_application_identity = receiving_scope
            .application_identity(self)
            .ok_or("closed requirement has no complete declaring application identity")?;
        let portable_trait_application_identity = receiving_scope
            .portable_application_identity(self)
            .ok_or("closed requirement has no complete portable declaring application")?;
        let portable_local_arguments = local_arguments
            .iter()
            .map(|argument| {
                crate::type_identity::portable_applied_type_identity(
                    self,
                    *argument,
                    lexical_owner,
                    &[],
                )
                .ok_or("closed requirement has no complete portable local operand")
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Signature and ABI equality are insufficient cache coordinates. The
        // outer application includes phantom arguments, and local Type/Const
        // operands retain their supplying region owner. Closure remains a
        // separate judgment above; this key does not admit an open operand.
        if let Some(existing) = self.closed_requirement_applications().find(|existing| {
            existing.declaring_trait == declaring_trait
                && existing.template == template
                && existing.trait_application_identity == trait_application_identity
                && existing.portable_trait_application_identity
                    == portable_trait_application_identity
                && existing.portable_local_arguments == portable_local_arguments
                && self
                    .expression_table
                    .static_machine_application_lifetimes(existing.application)
                    == self
                        .expression_table
                        .static_machine_application_lifetimes(application)
                && (self
                    .expression_table
                    .static_machine_application_lifetimes(application)
                    .is_empty()
                    || self
                        .expression_table
                        .static_machine_application(existing.application)
                        .lexical_owner
                        == lexical_owner)
                && self
                    .expression_table
                    .static_machine_application_bindings(existing.application)
                    .len()
                    == bindings.len()
                && self
                    .expression_table
                    .static_machine_application_bindings(existing.application)
                    .iter()
                    .zip(&bindings)
                    .all(|(left, right)| {
                        left.parameter == right.parameter
                            && match (&left.value, &right.value) {
                                (
                                    static_applications::StaticMachineBindingValue::Type(left),
                                    static_applications::StaticMachineBindingValue::Type(right),
                                )
                                | (
                                    static_applications::StaticMachineBindingValue::Const(left),
                                    static_applications::StaticMachineBindingValue::Const(right),
                                ) => {
                                    let existing_owner = self
                                        .expression_table
                                        .static_machine_application(existing.application)
                                        .lexical_owner;
                                    crate::type_identity::applied_type_identity(
                                        self,
                                        *left,
                                        existing_owner,
                                        &[],
                                    )
                                    .zip(crate::type_identity::applied_type_identity(
                                        self,
                                        *right,
                                        lexical_owner,
                                        &[],
                                    ))
                                    .is_some_and(|(left, right)| left == right)
                                }
                                (left, right) => left == right,
                            }
                    })
        }) {
            return Ok(existing.signature.symbol);
        }
        let declared = self
            .trait_machine_signature(template)
            .filter(|(owner, _)| owner.symbol == declaring_trait)
            .map(|(_, signature)| signature.clone())
            .ok_or("closed requirement lost its authored signature")?;
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
        let mut closed = substitutions.signature(self, &declared)?;
        closed.type_parameters = HandleSpan::empty();
        let trait_arguments = self
            .type_reference_table
            .insert_type_reference_handles(trait_arguments.iter().copied());
        let local_arguments = self
            .type_reference_table
            .insert_type_reference_handles(local_arguments);
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
                    application,
                },
                signature: closed,
            });
        Ok(instance)
    }
}
