//! Check functions supplied to a generic call against its declared requirements.
//! Prepare all supplied functions before copying guarantees that mention them.
//!
//! Each checking view keeps the exact invocation, selected function, arguments
//! and supplying scopes. The source declaration remains universal. These views
//! do not grant admission or create executable instances.

mod queries;
#[cfg(test)]
mod record_tests;

use super::{
    AppliedCallableOwner, AppliedCallableSignature, CallableApplication, CallableLifetimeBinding,
    CallableLifetimeOrigin, CallableSubstitution,
};
use crate::{
    TypedTrees, signature,
    static_applications::{StaticMachineApplicationHandle, StaticMachineBindingValue},
    types::TypeReferenceHandle,
};
use symbols::SymbolHandle;

impl TypedTrees {
    pub fn prepare_applied_machine_callable(
        &mut self,
        invocation: StaticMachineApplicationHandle,
        ordinal: usize,
        provider: StaticMachineApplicationHandle,
        selected_entry: SymbolHandle,
        demand: SymbolHandle,
        required: &signature::StateSignature,
        caller_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        caller_lifetime_bindings: &[crate::static_applications::StaticMachineLifetimeBinding],
    ) -> Result<(), &'static str> {
        let application = self.static_machine_application(provider);
        let selected = application.registration_operation;
        let machine = self.machine_by_symbol(application.binding_declaration);
        let forwarded = if machine.is_none() {
            self.machine_parameter_signature(selected)
                .map(|(_, signature)| signature.clone())
        } else {
            None
        };
        if machine.is_none() && forwarded.is_none() {
            return Err("applied callable lost its original declaration or Machine binder");
        }
        let substitutions = self
            .static_machine_application_bindings(provider)
            .iter()
            .filter_map(|binding| match binding.value {
                StaticMachineBindingValue::Type(actual)
                | StaticMachineBindingValue::Const(actual) => Some(Ok((binding.parameter, actual))),
                StaticMachineBindingValue::Conformance(_)
                | StaticMachineBindingValue::Machine(_) => None,
                StaticMachineBindingValue::RuntimeValue { .. } => Some(Err(
                    "applied machine callable has an unclosed runtime operand",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let static_arguments = self
            .static_machine_application_bindings(provider)
            .iter()
            .filter_map(|binding| match &binding.value {
                StaticMachineBindingValue::Conformance(argument)
                | StaticMachineBindingValue::Machine(argument) => {
                    Some((binding.parameter, argument.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let lifetimes = self
            .static_application_lifetimes_for_declaration(
                provider,
                application.binding_declaration,
            )?
            .iter()
            .map(|binding| (binding.parameter.clone(), binding.argument.clone()))
            .collect::<Vec<_>>();
        let declared = if let Some(machine) = machine {
            let state = self
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == selected)
                .ok_or("applied callable lost its original entry state")?;
            if !lifetimes.is_empty()
                && (lifetimes.len() != machine.lifetime_parameters.len()
                    || machine
                        .lifetime_parameters
                        .iter()
                        .zip(&lifetimes)
                        .any(|(parameter, (supplied, _))| parameter != supplied))
            {
                return Err("applied machine callable has an incomplete lifetime tuple");
            }
            if self
                .machine_type_parameters(machine)
                .iter()
                .any(|parameter| {
                    let supplied = match &parameter.kind {
                        crate::data::TypeParameterKind::Machine { .. } => {
                            static_arguments.iter().any(|(symbol, argument)| {
                                *symbol == parameter.symbol && argument.symbol.is_valid()
                            })
                        }
                        _ => substitutions
                            .iter()
                            .any(|(symbol, _)| *symbol == parameter.symbol),
                    };
                    !supplied
                })
            {
                return Err("applied machine callable has an incomplete provider tuple");
            }
            if machine.conformance_bounds.iter().any(|bound| {
                !static_arguments
                    .iter()
                    .any(|(symbol, _)| *symbol == bound.binder)
            }) {
                return Err("applied machine callable has an incomplete evidence tuple");
            }
            // Every provider binder is supplied; remaining nested telescopes are
            // copied by the same deep application used for nominal requirements.
            signature::StateSignature {
                symbol: selected,
                name: state.name.clone(),
                lifetime_parameters: machine.lifetime_parameters.clone(),
                parameters: state.parameters,
                return_type: state.return_type,
                contracts: machine.contracts,
                invokes: machine.invokes,
                service_reach_row: machine.service_reach_row,
                service_reach_is_installation_bound: machine.service_reach_is_installation_bound,
                suspends: machine.suspends,
                blocks: machine.blocks,
                termination_guarantee: match &machine.termination_plan.interface {
                    language_semantics::TerminationInterface::Published(guarantee) => {
                        guarantee.clone()
                    }
                    _ => language_semantics::TerminationGuarantee::NoGuarantee,
                },
                ..signature::StateSignature::default()
            }
        } else {
            if !substitutions.is_empty() || !static_arguments.is_empty() || !lifetimes.is_empty() {
                return Err("forwarded callable provider has an unexpected declaration tuple");
            }
            forwarded.ok_or("applied callable lost its forwarded contract")?
        };
        let provider_lifetime_declaration = application.lifetime_binding_declaration;
        let provider_lifetime_owner = application.lexical_owner;
        let signature = CallableApplication {
            region_scope: application.binding_declaration,
            lifetime_declaration: selected,
            lifetimes: lifetimes
                .into_iter()
                .map(|(parameter, argument)| {
                    let region = crate::type_identity::lifetime_parameter_identity(
                        self,
                        application.lexical_owner,
                        argument.as_str(),
                    )
                    .ok_or("applied provider region lost its supplying declaration")?;
                    Ok(CallableLifetimeBinding {
                        parameter,
                        argument,
                        region,
                        origin: CallableLifetimeOrigin::Lexical,
                    })
                })
                .collect::<Result<Vec<_>, &'static str>>()?,
            types: substitutions,
            static_arguments,
            static_argument_owner: application.lexical_owner,
            ..CallableApplication::default()
        }
        .signature(self, &declared)?;
        let provider_region_rows = self.static_machine_application_lifetimes(provider).to_vec();
        let provider_lifetimes = self
            .callable_lifetime_substitutions
            .insert_many(provider_region_rows);
        let invocation_source = self.static_machine_application(invocation).clone();
        let caller_lifetime_declaration = invocation_source.lifetime_binding_declaration;
        let caller_lifetime_owner = invocation_source.lexical_owner;
        let source_required = required.clone();
        // Save the exact inputs now. Required contracts are copied after all
        // providers for this invocation are available, including later binders.
        let caller_snapshot = self
            .expression_table
            .snapshot_static_machine_bindings(invocation);
        let caller_operands =
            self.callable_substitutions
                .insert_many(caller_substitutions.iter().map(|(parameter, actual)| {
                    CallableSubstitution {
                        parameter: *parameter,
                        actual: *actual,
                    }
                }));
        let caller_lifetimes = self
            .callable_lifetime_substitutions
            .insert_many(caller_lifetime_bindings.iter().cloned());
        let previous = self
            .applied_callable_signatures
            .iter()
            .find_map(|(handle, previous)| {
                matches!(&previous.owner, AppliedCallableOwner::MachineSelection {
                invocation: source,
                ordinal: position,
                demand: previous_demand,
                selected: previous_entry,
                source_required: previous_required,
                ..
            } if *source == invocation
                && *position == ordinal
                && *previous_demand == demand
                && *previous_entry == selected_entry
                && previous_required == &source_required)
                .then_some(handle)
            });
        let prepared = AppliedCallableSignature {
            owner: AppliedCallableOwner::MachineSelection {
                invocation,
                ordinal,
                demand,
                selected: selected_entry,
                provider,
                required: None,
                source_required,
                caller_operands,
                caller_bindings: caller_snapshot,
                caller_lifetimes,
                caller_lifetime_declaration,
                caller_lifetime_owner,
                provider_lifetime_declaration,
                provider_lifetime_owner,
                provider_lifetimes,
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
}

impl TypedTrees {
    /// Copy the invocation's guarantees after its whole provider roster exists.
    /// A guarantee may call a helper supplied before or after its checker.
    pub fn prepare_selected_machine_contracts(
        &mut self,
        invocation: StaticMachineApplicationHandle,
    ) -> Result<(), &'static str> {
        let selected = self
            .expression_table
            .static_machine_application_selected_machines(invocation);
        let mut providers = Vec::new();
        for (_, prepared) in self.applied_callable_signatures.iter() {
            let AppliedCallableOwner::MachineSelection {
                invocation: source,
                ordinal,
                selected: entry,
                demand,
                provider,
                ..
            } = &prepared.owner
            else {
                continue;
            };
            if *source != invocation
                || selected
                    .get(*ordinal)
                    .is_none_or(|argument| argument.symbol != *entry)
                || !self.machine_selection_lifetimes_are_current(&prepared.owner)
            {
                continue;
            }
            if let Some((_, previous)) = providers.iter().find(|(parameter, _)| parameter == demand)
            {
                if !self.same_selected_provider(*previous, *provider) {
                    return Err(
                        "current contract helper has conflicting retained provider applications",
                    );
                }
            } else {
                providers.push((*demand, *provider));
            }
        }
        let pending = self
            .applied_callable_signatures
            .iter()
            .filter_map(|(handle, prepared)| match &prepared.owner {
                AppliedCallableOwner::MachineSelection {
                    invocation: source,
                    required: None,
                    ordinal,
                    selected: entry,
                    ..
                } if *source == invocation
                    && selected
                        .get(*ordinal)
                        .is_some_and(|argument| argument.symbol == *entry)
                    && self.machine_selection_lifetimes_are_current(&prepared.owner) =>
                {
                    Some(handle)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for handle in pending {
            let AppliedCallableOwner::MachineSelection {
                source_required,
                caller_operands,
                caller_bindings,
                caller_lifetimes,
                caller_lifetime_owner,
                ..
            } = self.applied_callable_signatures.get(handle).owner.clone()
            else {
                unreachable!()
            };
            let arguments = self
                .expression_table
                .static_machine_binding_snapshot(caller_bindings)
                .iter()
                .filter_map(|binding| match &binding.value {
                    StaticMachineBindingValue::Machine(argument)
                    | StaticMachineBindingValue::Conformance(argument) => {
                        Some((binding.parameter, argument.clone()))
                    }
                    _ => None,
                })
                .collect();
            let lifetimes = self
                .callable_lifetime_substitutions
                .span_or_empty(caller_lifetimes)
                .iter()
                .map(|binding| {
                    let region = crate::type_identity::lifetime_parameter_identity(
                        self,
                        caller_lifetime_owner,
                        binding.argument.as_str(),
                    )
                    .ok_or("applied requirement region lost its supplying declaration")?;
                    Ok(CallableLifetimeBinding {
                        parameter: binding.parameter.clone(),
                        argument: binding.argument.clone(),
                        region,
                        origin: CallableLifetimeOrigin::Lexical,
                    })
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
            let types = self
                .callable_substitutions
                .span_or_empty(caller_operands)
                .iter()
                .map(|operand| (operand.parameter, operand.actual))
                .collect();
            let required = CallableApplication {
                lifetimes,
                types,
                static_arguments: arguments,
                static_argument_owner: caller_lifetime_owner,
                selected_providers: providers.clone(),
                ..CallableApplication::default()
            }
            .signature(self, &source_required)?;
            self.applied_callable_signatures
                .set_required(handle, required)?;
        }
        Ok(())
    }

    pub fn applied_machine_provider(
        &self,
        invocation: StaticMachineApplicationHandle,
        ordinal: usize,
        selected: SymbolHandle,
        demand: SymbolHandle,
        expected: &signature::StateSignature,
    ) -> Option<StaticMachineApplicationHandle> {
        self.applied_callable_signatures
            .iter()
            .find_map(|(_, prepared)| match &prepared.owner {
                AppliedCallableOwner::MachineSelection {
                    invocation: source,
                    ordinal: position,
                    selected: entry,
                    demand: parameter,
                    source_required,
                    provider,
                    ..
                } if *source == invocation
                    && *position == ordinal
                    && *entry == selected
                    && *parameter == demand
                    && source_required == expected =>
                {
                    Some(*provider)
                }
                _ => None,
            })
    }
}
