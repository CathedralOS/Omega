//! Match each closed public method to the body that was prepared for its arguments.

use super::{Program, services};
use crate::source_state::machine::Machine;
use crate::source_state::signature::StateSignature;
use crate::source_state::trait_definition::{AppliedTraitScope, Conformance};
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_check_tables::Checked;

impl<'program> Program<'program> {
    pub(super) fn conformance_method_bodies(
        &mut self,
        context: MachineId,
        application: &AppliedTraitScope<'program>,
        declaration: &'program Conformance,
        required: &'program StateSignature,
        supplied: &'program Machine,
    ) -> Checked<Vec<(&'program StateSignature, &'program Machine)>> {
        if !application.declaration.is_boundary || required.type_parameters.is_empty() {
            return Ok(vec![(required, supplied)]);
        }
        let typed = self.typed;
        let mut members = Vec::new();
        // The authored member still owes its universal body and refinement
        // checks when no concrete call reaches this installation.
        if !self.generic_templates.contains(&supplied.symbol) {
            self.generic_templates.push(supplied.symbol);
        }
        self.generic_conformance_witnesses(context, declaration)?;
        for closed in typed.closed_requirement_applications().filter(|closed| {
            closed.template == required.symbol
                && typed.requirement_application_matches_contract_scope(
                    closed.signature.symbol,
                    application,
                )
        }) {
            let mut instances = typed.machine_specializations.iter().filter(|receipt| {
                receipt.template == supplied.symbol
                    && typed
                        .expression_table
                        .static_machine_application(receipt.binding_application)
                        .registration_operation
                        == closed.signature.symbol
            });
            let (Some(instance), None) = (instances.next(), instances.next()) else {
                return Err(services::unavailable(
                    context,
                    "the selected method needs one implementation for the public arguments",
                ));
            };
            let body = typed.machine_by_symbol(instance.instance).ok_or_else(|| {
                services::unavailable(context, "the selected method instance lost its body")
            })?;
            if self
                .provider_method_requirement(context, body.symbol, required.symbol)?
                .map(|signature| signature.symbol)
                != Some(closed.signature.symbol)
            {
                return Err(services::unavailable(
                    context,
                    "the selected method instance has another public requirement",
                ));
            }
            members.push((closed.signature, body));
        }
        Ok(members)
    }

    /// Recover the public arguments used to specialize a selected provider.
    /// The requirement and implementation declare different binder symbols.
    pub(super) fn provider_method_requirement(
        &self,
        id: MachineId,
        machine: SymbolHandle,
        template: SymbolHandle,
    ) -> Checked<Option<&'program StateSignature>> {
        let typed = self.typed;
        let unavailable = |reason| super::services::unavailable(id, reason);
        let Some(crate::source_state::MachineInstantiation::Specialization(receipt)) =
            typed.machine_instantiation(machine).map_err(unavailable)?
        else {
            return Ok(None);
        };
        typed
            .specialization_binding_declaration(receipt)
            .map_err(unavailable)?;
        let actual = typed
            .expression_table
            .static_machine_application(receipt.binding_application);
        let Some(closed) = typed.closed_requirement_application(actual.registration_operation)
        else {
            return Ok(None);
        };
        if closed.template != template || actual.binding_declaration != receipt.template {
            return Err(unavailable(
                "the selected method belongs to another requirement or implementation",
            ));
        }
        typed
            .require_closed_requirement_call_application(
                closed.signature.symbol,
                closed.application,
            )
            .map_err(unavailable)?;
        let public = typed
            .expression_table
            .static_machine_application(closed.application);
        let arguments = typed
            .expression_table
            .static_machine_application_arguments(closed.application);
        let bindings = typed
            .expression_table
            .static_machine_application_bindings(receipt.binding_application);
        if bindings.len() != arguments.len() {
            return Err(unavailable(
                "the selected method and its public requirement take different numbers of arguments",
            ));
        }
        for (binding, argument) in bindings.iter().zip(arguments) {
            let parameter = typed
                .data_type_parameter_by_symbol(binding.parameter)
                .ok_or_else(|| unavailable("the selected method lost its declared parameter"))?;
            if !typed.static_binding_matches_argument(
                &parameter.kind,
                &binding.value,
                argument,
                public.lexical_owner,
                actual.lexical_owner,
            ) {
                return Err(unavailable(
                    "the selected method has a different argument for a public requirement parameter",
                ));
            }
        }
        Ok(Some(closed.signature))
    }
}
