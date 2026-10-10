//! Occurrence-owned checking views for completely determined symbolic providers.
//! The declaration keeps its universal telescope. A view consumes only its exact
//! supplied tuple, with the invocation and provider lexical scopes kept separate.
//! Bounds and effects still owe ordinary admission; no executable cache is made.

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
        required: &signature::StateSignature,
        caller_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
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
            types: substitutions,
            static_arguments,
            ..CallableApplication::default()
        }
        .signature(self, &declared)?;
        let source_required = required.clone();
        let required = CallableApplication {
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
        let prepared = AppliedCallableSignature {
            owner: AppliedCallableOwner::MachineSelection {
                invocation,
                ordinal,
                selected: selected_entry,
                provider,
                required,
                source_required,
                caller_operands,
            },
            signature,
        };
        let previous = self.applied_callable_signatures.iter().find_map(|(handle, previous)| {
            matches!(&previous.owner, AppliedCallableOwner::MachineSelection { invocation: source, ordinal: position, .. } if *source == invocation && *position == ordinal).then_some(handle)
        });
        if let Some(previous) = previous {
            *self.applied_callable_signatures.get_mut(previous) = prepared;
        } else {
            self.applied_callable_signatures.insert(prepared);
        }
        Ok(())
    }
}
