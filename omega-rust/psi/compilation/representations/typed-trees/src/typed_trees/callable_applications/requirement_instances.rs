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

    /// Rejoin a finite signature's declaration-scoped tuple to a complete
    /// source invocation. The signature keeps invocation lifetimes universal;
    /// those caller loans and an inherited presentation do not belong to the
    /// synthetic tuple. They remain in the original complete application view.
    pub fn closed_requirement_application_matches_call(
        &self,
        instance: SymbolHandle,
        application: static_applications::StaticMachineApplicationHandle,
    ) -> bool {
        use crate::type_identity::{
            AppliedTypeIdentityOwner, AppliedTypeIdentityScope, IndexIdentityArgument,
            TypeIdentityQualification, applied_static_parameter_identity,
        };
        if !self
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return false;
        }
        let mut instances = self
            .closed_requirement_applications()
            .filter(|row| row.signature.symbol == instance);
        let Some(closed) = instances.next() else {
            return false;
        };
        if instances.next().is_some()
            || !self
                .expression_table
                .static_machine_application_is_valid(closed.application)
        {
            return false;
        }
        let Ok(actual) = static_applications::StaticApplicationView::new(self, application) else {
            return false;
        };
        if actual.declaration().symbol() != closed.template
            || actual.application().registration_operation != closed.template
        {
            return false;
        }
        let Some((owner, signature)) = self.trait_machine_signature(closed.template) else {
            return false;
        };
        if owner.symbol != closed.declaring_trait || !owner.lifetime_parameters.is_empty() {
            return false;
        }
        let selected = if actual.application().selected_interface.is_valid() {
            actual.application().selected_interface
        } else {
            owner.symbol
        };
        let Some(selected) = self.trait_definition_by_symbol(selected) else {
            return false;
        };
        if !selected.lifetime_parameters.is_empty() {
            return false;
        }
        let parameters = self.trait_type_parameters(owner);
        let local_parameters = self.state_signature_type_parameters(signature);
        if parameters.len() != closed.trait_arguments.len()
            || local_parameters.len() != closed.local_arguments.len()
            || owner.conformance_bounds.iter().any(|bound| {
                !parameters
                    .iter()
                    .any(|parameter| parameter.symbol == bound.binder)
            })
        {
            return false;
        }
        let Some(lexical_owner) = actual.application().type_identity_owner(self) else {
            return false;
        };
        let actual_arguments = actual
            .bindings()
            .iter()
            .map(|binding| {
                let argument = match &binding.value {
                    static_applications::StaticMachineBindingValue::Type(reference) => {
                        IndexIdentityArgument::TypeArgument(*reference)
                    }
                    static_applications::StaticMachineBindingValue::Const(reference) => {
                        IndexIdentityArgument::TypeReference(*reference)
                    }
                    static_applications::StaticMachineBindingValue::Machine(argument)
                    | static_applications::StaticMachineBindingValue::Conformance(argument) => {
                        IndexIdentityArgument::StaticArgument(argument)
                    }
                    static_applications::StaticMachineBindingValue::RuntimeValue { .. } => {
                        return None;
                    }
                };
                Some((binding.parameter, argument))
            })
            .collect::<Option<Vec<_>>>();
        let Some(actual_arguments) = actual_arguments else {
            return false;
        };
        let lifetimes = actual
            .lifetimes()
            .iter()
            .map(|(formal, binding)| (formal.name.to_string(), binding.argument.scope_argument()))
            .collect::<Vec<_>>();
        let expected_arguments = parameters
            .iter()
            .zip(closed.trait_arguments)
            .map(|(parameter, reference)| {
                let argument = if matches!(parameter.kind, data::TypeParameterKind::Type) {
                    IndexIdentityArgument::TypeArgument(*reference)
                } else {
                    IndexIdentityArgument::TypeReference(*reference)
                };
                (parameter.symbol, argument)
            })
            .collect::<Vec<_>>();
        let expected_scopes = [AppliedTypeIdentityScope {
            region_role: crate::type_identity::AppliedTypeIdentityRegionRole::Supplied,
            declaration: owner.symbol,
            arguments: &expected_arguments,
            lifetimes: &[],
        }];
        let expected_local_arguments = local_parameters
            .iter()
            .zip(closed.local_arguments)
            .map(|(parameter, reference)| {
                let argument = if matches!(parameter.kind, data::TypeParameterKind::Type) {
                    IndexIdentityArgument::TypeArgument(*reference)
                } else {
                    IndexIdentityArgument::TypeReference(*reference)
                };
                (parameter.symbol, argument)
            })
            .collect::<Vec<_>>();
        let expected_local_scopes = [
            expected_scopes[0],
            AppliedTypeIdentityScope {
                region_role: crate::type_identity::AppliedTypeIdentityRegionRole::Supplied,
                declaration: closed.template,
                arguments: &expected_local_arguments,
                lifetimes: &[],
            },
        ];
        let expected_owner = AppliedTypeIdentityOwner::Declaration(closed.template);
        let expected_outer = parameters
            .iter()
            .zip(closed.trait_arguments)
            .map(|(parameter, reference)| {
                let argument = if matches!(parameter.kind, data::TypeParameterKind::Type) {
                    IndexIdentityArgument::TypeArgument(*reference)
                } else {
                    IndexIdentityArgument::TypeReference(*reference)
                };
                applied_static_parameter_identity(
                    self,
                    parameter,
                    argument,
                    expected_owner,
                    &expected_scopes,
                    TypeIdentityQualification::Ordinary,
                )
            })
            .collect::<Option<Vec<_>>>();
        let expected_local = local_parameters
            .iter()
            .zip(closed.local_arguments)
            .map(|(parameter, reference)| {
                let argument = if matches!(parameter.kind, data::TypeParameterKind::Type) {
                    IndexIdentityArgument::TypeArgument(*reference)
                } else {
                    IndexIdentityArgument::TypeReference(*reference)
                };
                applied_static_parameter_identity(
                    self,
                    parameter,
                    argument,
                    expected_owner,
                    &expected_local_scopes,
                    TypeIdentityQualification::Ordinary,
                )
            })
            .collect::<Option<Vec<_>>>();
        let (Some(expected_outer), Some(expected_local)) = (expected_outer, expected_local) else {
            return false;
        };
        let mut found = false;
        let mut agrees = true;
        crate::trait_definition::AppliedTraitScope::abstract_scope(selected).visit(
            self,
            &mut |scope| {
                if scope.declaration.symbol != owner.symbol {
                    return;
                }
                found = true;
                // Keep every original parent telescope simultaneous. No parent
                // operand is interpreted in the telescope it is replacing.
                let groups = scope
                    .substitution_scope_starts
                    .iter()
                    .enumerate()
                    .map(|(ordinal, start)| {
                        let end = scope
                            .substitution_scope_starts
                            .get(ordinal + 1)
                            .copied()
                            .unwrap_or(scope.substitutions.len());
                        scope.substitutions[*start..end]
                            .iter()
                            .map(|(parameter, reference)| {
                                let parameter = self.data_type_parameter_by_symbol(*parameter)?;
                                let argument =
                                    if matches!(parameter.kind, data::TypeParameterKind::Type) {
                                        IndexIdentityArgument::TypeArgument(*reference)
                                    } else {
                                        IndexIdentityArgument::TypeReference(*reference)
                                    };
                                Some((parameter.symbol, argument))
                            })
                            .collect::<Option<Vec<_>>>()
                    })
                    .collect::<Option<Vec<_>>>();
                let Some(groups) = groups else {
                    agrees = false;
                    return;
                };
                let mut scopes = vec![AppliedTypeIdentityScope {
                    region_role: crate::type_identity::AppliedTypeIdentityRegionRole::Supplied,
                    declaration: closed.template,
                    arguments: &actual_arguments,
                    lifetimes: &lifetimes,
                }];
                for group in &groups {
                    let Some((parameter, _)) = group.first() else {
                        continue;
                    };
                    let declaration = self.symbols.get(*parameter).parent;
                    if self
                        .trait_definition_by_symbol(declaration)
                        .is_none_or(|owner| !owner.lifetime_parameters.is_empty())
                    {
                        agrees = false;
                        return;
                    }
                    scopes.push(AppliedTypeIdentityScope {
                        region_role: crate::type_identity::AppliedTypeIdentityRegionRole::Supplied,
                        declaration,
                        arguments: group,
                        lifetimes: &[],
                    });
                }
                for (parameter, expected) in parameters
                    .iter()
                    .zip(&expected_outer)
                    .chain(local_parameters.iter().zip(&expected_local))
                {
                    let identity = applied_static_parameter_identity(
                        self,
                        parameter,
                        IndexIdentityArgument::Subject(parameter.symbol),
                        lexical_owner,
                        &scopes,
                        TypeIdentityQualification::Ordinary,
                    );
                    if identity.as_ref() != Some(expected) {
                        agrees = false;
                    }
                }
            },
        );
        found && agrees
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
            return self.owned_requirement_application_matches_scope(requirement, scope);
        };
        let Some(declaration) = self.trait_definition_by_symbol(application.declaring_trait) else {
            return false;
        };
        scope
            .declaring_scope(self, declaration.symbol)
            .and_then(|actual| actual.application_identity(self))
            == Some(application.trait_application_identity)
    }

    /// Compare a produced public header's original selected-interface telescope
    /// with an independently retained provider scope. The method application remains in
    /// the header identity; only its exact outer tuple selects a provider schema.
    /// The shared full application key retains supplying scopes and regions;
    /// erased or missing operands cannot license a correspondence.
    fn owned_requirement_application_matches_scope(
        &self,
        requirement: SymbolHandle,
        scope: &crate::trait_definition::AppliedTraitScope<'_>,
    ) -> bool {
        let mut headers = self
            .instantiated_requirement_signatures
            .iter()
            .filter(|(_, row)| {
                self.trait_machine_signatures.get(row.signature).symbol == requirement
            });
        let Some((_, header)) = headers.next() else {
            return false;
        };
        if headers.next().is_some() {
            return false;
        }
        let Some((declaration, original)) =
            self.trait_machine_signature(header.requirement_template)
        else {
            return false;
        };
        let Some(selected) = self
            .selected_public_trait_requirement_signature(requirement, header.binding_application)
        else {
            return false;
        };
        if selected.declaration.symbol != original.symbol
            || selected.owner.symbol != declaration.symbol
            || header.declaring_trait != declaration.symbol
        {
            return false;
        }
        let interface = scope.declaration;
        if interface.conformance_bounds.iter().any(|bound| {
            !self
                .trait_type_parameters(interface)
                .iter()
                .any(|parameter| parameter.symbol == bound.binder)
        }) || static_applications::requirement_provider_application_identity(
            self,
            requirement,
            interface.symbol,
        )
        .is_err()
        {
            return false;
        }
        let Some(roster) = self
            .requirement_interface_applications
            .span(header.interface_applications)
        else {
            return false;
        };
        let mut rows = roster
            .iter()
            .filter(|row| row.interface == interface.symbol);
        let Some(row) = rows.next() else {
            return false;
        };
        if rows.next().is_some() {
            return false;
        }
        self.requirement_interface_application_matches_scope(
            original.symbol,
            row.binding_application,
            scope,
        )
    }

    /// A caller-owned logical contract and a produced public header use the
    /// same full original interface application comparison. This authenticates
    /// identity only; it does not choose a provider or admit a body.
    pub fn requirement_call_application_matches_scope(
        &self,
        requirement: SymbolHandle,
        application: static_applications::StaticMachineApplicationHandle,
        scope: &crate::trait_definition::AppliedTraitScope<'_>,
    ) -> bool {
        let Some(public) =
            self.selected_public_trait_requirement_signature(requirement, application)
        else {
            return false;
        };
        if !application.is_valid() {
            return public.signature.symbol == public.declaration.symbol
                && static_applications::original_requirement_is_complete(
                    self,
                    scope.declaration.symbol,
                    public.declaration.symbol,
                )
                && scope.declaring_scope(self, public.owner.symbol).is_some();
        }
        if public.signature.symbol != public.declaration.symbol {
            // A produced executable header must retain its own complete
            // interface roster. A logical call cannot repair a missing row.
            return self.requirement_application_matches_scope(requirement, scope);
        }
        // A logical caller contract can precede an executable header. Its own
        // complete selected-interface tuple is the input, never guessed values
        // from the callee or an unrelated interface's application roster.
        let original = self
            .expression_table
            .static_machine_application(application);
        let interface = if original.selected_interface.is_valid() {
            original.selected_interface
        } else {
            public.owner.symbol
        };
        interface == scope.declaration.symbol
            && self.requirement_interface_application_matches_scope(
                public.declaration.symbol,
                application,
                scope,
            )
    }

    fn requirement_interface_application_matches_scope(
        &self,
        requirement_template: SymbolHandle,
        application_handle: static_applications::StaticMachineApplicationHandle,
        scope: &crate::trait_definition::AppliedTraitScope<'_>,
    ) -> bool {
        use crate::type_identity::{
            AppliedTypeIdentityScope, IndexIdentityArgument, TypeIdentityQualification,
            applied_lifetime_identity, applied_static_parameter_identity,
            local_trait_application_identity,
        };
        let interface = scope.declaration;
        if interface.conformance_bounds.iter().any(|bound| {
            !self
                .trait_type_parameters(interface)
                .iter()
                .any(|parameter| parameter.symbol == bound.binder)
        }) {
            return false;
        }
        let Ok(application) =
            static_applications::StaticApplicationView::new(self, application_handle)
        else {
            return false;
        };
        if application.declaration().symbol() != requirement_template {
            return false;
        }
        let Some(owner) = application.application().type_identity_owner(self) else {
            return false;
        };
        let Some(arguments) = application
            .bindings()
            .iter()
            .filter(|binding| {
                self.trait_type_parameters(interface)
                    .iter()
                    .any(|parameter| parameter.symbol == binding.parameter)
                    || interface
                        .conformance_bounds
                        .iter()
                        .any(|bound| bound.binder == binding.parameter)
            })
            .map(|binding| {
                let value = match &binding.value {
                    static_applications::StaticMachineBindingValue::Type(reference) => {
                        IndexIdentityArgument::TypeArgument(*reference)
                    }
                    static_applications::StaticMachineBindingValue::Const(reference) => {
                        IndexIdentityArgument::TypeReference(*reference)
                    }
                    static_applications::StaticMachineBindingValue::Machine(argument)
                    | static_applications::StaticMachineBindingValue::Conformance(argument) => {
                        IndexIdentityArgument::StaticArgument(argument)
                    }
                    static_applications::StaticMachineBindingValue::RuntimeValue { .. } => {
                        return None;
                    }
                };
                Some((binding.parameter, value))
            })
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        let Some(captured) = application
            .lifetimes()
            .get(..interface.lifetime_parameters.len())
        else {
            return false;
        };
        if captured
            .iter()
            .zip(&interface.lifetime_parameters)
            .any(|((formal, _), parameter)| formal.name != parameter)
        {
            return false;
        }
        let lifetimes = captured
            .iter()
            .map(|(formal, binding)| (formal.name.to_string(), binding.argument.scope_argument()))
            .collect::<Vec<_>>();
        let scopes = [AppliedTypeIdentityScope {
            region_role: crate::type_identity::AppliedTypeIdentityRegionRole::Supplied,
            declaration: interface.symbol,
            arguments: &arguments,
            lifetimes: &lifetimes,
        }];
        let Some(expected) = self
            .trait_type_parameters(interface)
            .iter()
            .map(|parameter| {
                applied_static_parameter_identity(
                    self,
                    parameter,
                    IndexIdentityArgument::Subject(parameter.symbol),
                    owner,
                    &scopes,
                    TypeIdentityQualification::Ordinary,
                )
            })
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        let Some(regions) = interface
            .lifetime_parameters
            .iter()
            .map(|parameter| applied_lifetime_identity(self, parameter.as_str(), owner, &scopes))
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        let Some(expected) =
            local_trait_application_identity(self, interface.symbol, &expected, &regions)
        else {
            return false;
        };
        scope.application_identity(self) == Some(expected)
    }

    /// Join a selected public call to its genuine runtime receiver.
    /// The finite producer retains its declaring scope; an owned public header
    /// retains complete reachable interface applications. Receiver regions and
    /// operand categories come from the original type payload and lexical owner,
    /// never from a declaration-only scope or the copied runtime ABI signature.
    pub fn requirement_application_matches_receiver<'program>(
        &'program self,
        requirement: SymbolHandle,
        call_application: static_applications::StaticMachineApplicationHandle,
        scope: &crate::trait_definition::AppliedTraitScope<'program>,
        receiver: types::TypeReferenceHandle,
        lexical_owner: crate::type_identity::AppliedTypeIdentityOwner<'_>,
    ) -> bool {
        let Some(selected) =
            self.selected_public_trait_requirement_signature(requirement, call_application)
        else {
            return false;
        };
        if let Some(application) = self.closed_requirement_application(requirement) {
            // Finite tuples intentionally have no erased region payload. Do
            // not let ordinary scope equality erase a region-bearing receiver.
            let Some(actual) = scope.declaring_scope(self, application.declaring_trait) else {
                return false;
            };
            if !scope.declaration.lifetime_parameters.is_empty()
                || !actual.declaration.lifetime_parameters.is_empty()
                || actual
                    .region_free_parameter_identities(
                        self,
                        crate::type_identity::TypeIdentityQualification::Ordinary,
                    )
                    .is_none()
            {
                return false;
            }
            return self.requirement_application_matches_scope(requirement, scope);
        }
        let Some(executable) = self.public_trait_requirement_signature(requirement) else {
            return false;
        };
        if scope.declaring_scope(self, selected.owner.symbol).is_none() {
            return false;
        }
        let expected = if executable.application.is_valid() {
            // Produced headers keep their exact inherited interface roster.
            // The call tuple authenticated above cannot borrow another header.
            static_applications::requirement_provider_application_identity(
                self,
                requirement,
                scope.declaration.symbol,
            )
        } else {
            // A complete logical contract can precede body specialization.
            // Its own original call, not a fabricated executable header,
            // supplies the complete selected-interface tuple and regions.
            static_applications::requirement_call_interface_identity(
                self,
                requirement,
                call_application,
                scope.declaration.symbol,
            )
        };
        let Ok(expected) = expected else {
            return false;
        };
        let Ok(actual) =
            static_applications::trait_type_application_identity(self, receiver, lexical_owner)
        else {
            return false;
        };
        actual == expected
    }

    /// Join a finite requirement coordinate to the exact realization receipt.
    /// These canonical tuple identities are retained by specialization; equal
    /// ABI types and readable const spellings cannot donate another tuple.
    pub fn requirement_realization_specialization(
        &self,
        requirement: SymbolHandle,
        realization: SymbolHandle,
        provider: SymbolHandle,
    ) -> Option<&crate::typed_trees::commitments::MachineSpecialization> {
        let application = self.closed_requirement_application(requirement)?;
        let tuple = self
            .expression_table
            .static_machine_application_arguments(application.application)
            .iter()
            .map(|argument| self.static_const_argument_identity(argument))
            .collect::<Option<Vec<_>>>()?;
        let mut matches = self.machine_specializations.iter().filter(|receipt| {
            receipt.template == realization
                && receipt.instance == provider
                && receipt.is_const_only()
                && receipt.const_argument_identities == tuple
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
            .static_machine_application(application)
            .clone();
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
        if receiving_scope.argument_owner != lexical_owner {
            return Err("closed requirement source application has a different supplying owner");
        }
        // Closure consumes the same source environment that defines the key.
        // Discarding it here either rejects retained regions or licenses a
        // later consumer to rebuild a different application from erased types.
        self.require_receiving_trait_operands(&receiving_scope)?;
        self.require_receiving_application_operands(&local_arguments, &receiving_scope)?;
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
                    crate::type_identity::AppliedTypeIdentityOwner::Declaration(lexical_owner),
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
                                        .type_identity_owner(self);
                                    existing_owner.and_then(|owner| {
                                        crate::type_identity::applied_type_identity(
                                            self, *left, owner, &[],
                                            crate::type_identity::TypeIdentityQualification::Ordinary,
                                        )
                                    })
                                    .zip(source.type_identity_owner(self).and_then(|owner| {
                                        crate::type_identity::applied_type_identity(
                                            self, *right, owner, &[],
                                            crate::type_identity::TypeIdentityQualification::Ordinary,
                                        )
                                    }))
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
