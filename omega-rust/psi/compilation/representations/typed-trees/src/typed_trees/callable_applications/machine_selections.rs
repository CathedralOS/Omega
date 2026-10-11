//! Occurrence-owned checking views for completely determined symbolic providers.
//! The declaration keeps its universal telescope. A view consumes only its exact
//! supplied tuple, with the invocation and provider lexical scopes kept separate.
//! Bounds and effects still owe ordinary admission; no executable cache is made.
//! One selected tuple can face several member-local demands. The demanding
//! binder owns its view; invocation and ordinal alone cannot identify a contract.

mod queries;

use super::{
    AppliedCallableOwner, AppliedCallableSignature, CallableApplication, CallableSubstitution,
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
        let application = self.expression_table.static_machine_application(provider);
        let selected = application.registration_operation;
        let machine = self
            .machine_by_symbol(application.binding_declaration)
            .ok_or("applied callable lost its original machine declaration")?;
        let state = self
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == selected)
            .ok_or("applied callable lost its original entry state")?;
        let substitutions = self
            .expression_table
            .static_machine_application_bindings(provider)
            .iter()
            .filter_map(|binding| match binding.value {
                StaticMachineBindingValue::Type(actual)
                | StaticMachineBindingValue::Const(actual) => Some(Ok((binding.parameter, actual))),
                StaticMachineBindingValue::Conformance(_) => None,
                _ => Some(Err(
                    "applied machine callable has an unclosed machine or runtime operand",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let static_arguments = self
            .expression_table
            .static_machine_application_bindings(provider)
            .iter()
            .filter_map(|binding| match &binding.value {
                StaticMachineBindingValue::Conformance(argument) => {
                    Some((binding.parameter, argument.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let lifetimes = self
            .expression_table
            .static_machine_application_lifetimes(provider)
            .iter()
            .map(|binding| (binding.parameter.clone(), binding.argument.clone()))
            .collect::<Vec<_>>();
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
                !substitutions
                    .iter()
                    .any(|(symbol, _)| *symbol == parameter.symbol)
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
        let declared = signature::StateSignature {
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
                language_semantics::TerminationInterface::Published(guarantee) => guarantee.clone(),
                _ => language_semantics::TerminationGuarantee::NoGuarantee,
            },
            ..signature::StateSignature::default()
        };
        let signature = CallableApplication {
            region_scope: application.binding_declaration,
            lifetime_declaration: selected,
            lifetimes,
            types: substitutions,
            static_arguments,
            ..CallableApplication::default()
        }
        .signature(self, &declared)?;
        let source_required = required.clone();
        let caller_lifetimes = caller_lifetime_bindings
            .iter()
            .map(|binding| (binding.parameter.clone(), binding.argument.clone()))
            .collect::<Vec<_>>();
        let required = CallableApplication {
            lifetimes: caller_lifetimes,
            types: caller_substitutions.to_vec(),
            ..CallableApplication::default()
        }
        .signature(self, required)?;
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
        let prepared = AppliedCallableSignature {
            owner: AppliedCallableOwner::MachineSelection {
                invocation,
                ordinal,
                demand,
                selected: selected_entry,
                provider,
                required,
                source_required,
                caller_operands,
                caller_lifetimes,
            },
            signature,
        };
        let previous = self.applied_callable_signatures.iter().find_map(|(handle, previous)| {
            matches!(&previous.owner, AppliedCallableOwner::MachineSelection { invocation: source, ordinal: position, demand: previous_demand, .. } if *source == invocation && *position == ordinal && *previous_demand == demand).then_some(handle)
        });
        if let Some(previous) = previous {
            *self.applied_callable_signatures.get_mut(previous) = prepared;
        } else {
            self.applied_callable_signatures.insert(prepared);
        }
        Ok(())
    }
}
