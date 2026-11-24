//! Match each closed public method to the body that was prepared for its arguments.

use super::{Program, services};
use crate::source_state::machine::Machine;
use crate::source_state::signature::StateSignature;
use crate::source_state::trait_definition::AppliedTraitScope;
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_check_tables::Checked;

impl<'program> Program<'program> {
    pub(super) fn conformance_method_bodies(
        &mut self,
        context: MachineId,
        application: &AppliedTraitScope<'program>,
        required: &'program StateSignature,
        supplied: &'program Machine,
    ) -> Checked<Vec<(&'program StateSignature, &'program Machine)>> {
        let typed = self.typed;
        if required.type_parameters.is_empty() {
            let header = typed
                .applied_callable_signature(application, required.symbol)
                .or_else(|| {
                    application.keeps_declared_signatures(typed).then_some(required)
                })
                .or_else(|| typed.closed_requirement_in_scope(application, required.symbol))
                .ok_or_else(|| {
                    services::unavailable(context, "conformance lost its prepared requirement header")
                })?;
            return Ok(vec![(header, supplied)]);
        }
        if !self.generic_templates.contains(&supplied.symbol) {
            self.generic_templates.push(supplied.symbol);
        }
        let unavailable = |reason| services::unavailable(context, reason);
        let mut members = Vec::new();
        for receipt in typed
            .machine_specializations
            .iter()
            .filter(|receipt| receipt.template == supplied.symbol)
        {
            let Some(closed) =
                self.provider_method_requirement(context, receipt.instance, required.symbol)?
            else {
                continue;
            };
            if !typed.requirement_application_matches_contract_scope(closed.symbol, application) {
                return Err(unavailable(
                    "the selected method belongs to another trait application",
                ));
            }
            let body = typed
                .machine_by_symbol(receipt.instance)
                .ok_or_else(|| unavailable("the selected method instance lost its body"))?;
            members.push((closed, body));
        }
        members.sort_unstable_by_key(|(header, _)| {
            (header.symbol.arena_index(), header.symbol.generation())
        });
        if members
            .windows(2)
            .any(|pair| pair[0].0.symbol == pair[1].0.symbol)
        {
            return Err(unavailable(
                "the selected method has two bodies for the same public arguments",
            ));
        }
        Ok(members)
    }

    /// Read the public header with the implementation's type parameters.
    /// Each declaration owns its binder names. Their order and declared bounds
    /// determine which parameter in the body stands for a public parameter.
    pub(super) fn with_method_type_parameters<T>(
        &mut self,
        context: MachineId,
        member: &Machine,
        required: &StateSignature,
        read: impl FnOnce(&mut Self) -> Checked<T>,
    ) -> Checked<T> {
        use crate::source_state::data::TypeParameterKind;
        let typed = self.typed;
        let parameters = typed.state_signature_type_parameters(required);
        if !parameters
            .iter()
            .any(|parameter| parameter.kind == TypeParameterKind::Type)
        {
            return read(self);
        }
        let member_parameters = typed.machine_type_parameters(member);
        if parameters.len() != member_parameters.len() {
            return Err(services::unavailable(
                context,
                "the requirement and its implementation declare different numbers of static parameters",
            ));
        }
        let mut bindings = Vec::new();
        let mut sources = Vec::new();
        for (public, private) in parameters.iter().zip(member_parameters.iter()) {
            if public.kind != TypeParameterKind::Type {
                continue;
            }
            if private.kind != TypeParameterKind::Type
                || crate::source_state::data::type_parameter_demands_stronger_properties(
                    public, private,
                )
            {
                return Err(services::unavailable(
                    context,
                    "the method implementation gives a public type parameter a different contract",
                ));
            }
            let value = self
                .scope
                .iter()
                .find_map(|(symbol, ty)| (*symbol == private.symbol).then_some(*ty))
                .ok_or_else(|| {
                    services::unavailable(context, "the implementation lost its type parameter")
                })?;
            bindings.push((public.symbol, value));
            if let Some(reference) = typed
                .type_reference_table
                .find_named_type_reference(private.symbol)
            {
                sources.push((public.symbol, reference));
            }
        }
        let outer = self.extended_source_scope(sources);
        let previous = std::mem::replace(&mut self.source_scope, outer);
        let previous_len = self.scope.len();
        self.scope.extend(bindings);
        let result = read(self);
        self.scope.truncate(previous_len);
        self.source_scope = previous;
        result
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
        if !typed
            .expression_table
            .static_machine_application_is_valid(receipt.binding_application)
        {
            return Err(unavailable("the selected method lost its static arguments"));
        }
        let actual = typed
            .expression_table
            .static_machine_application(receipt.binding_application);
        let Some(closed) = typed.closed_requirement_application(actual.registration_operation)
        else {
            return Ok(None);
        };
        typed
            .specialization_binding_declaration(receipt)
            .map_err(unavailable)?;
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

    /// The body a named call enters for a member its conformance supplies.
    /// A generic member (`machine keep<T>`) is entered at the instance stage
    /// 04 prepared for the call's own closed requirement application,
    /// `Keep::keep<u8>` for `Local::keep::<u8>(x)`; any other member is its
    /// own body. Without that prepared instance the generic body is left as
    /// selected, and reading it refuses the call.
    pub(super) fn selected_member_instance(
        &self,
        id: MachineId,
        call: crate::source_state::references::StaticArgumentCall<'_>,
        member: SymbolHandle,
        requirement: SymbolHandle,
    ) -> Checked<SymbolHandle> {
        let typed = self.typed;
        let generic = typed
            .machine_by_symbol(member)
            .is_some_and(|machine| !machine.type_parameters.is_empty());
        let target = match call {
            crate::source_state::references::StaticArgumentCall::Expression(_, call) => {
                call.target_symbol
            }
            crate::source_state::references::StaticArgumentCall::Statement(call) => {
                call.target_symbol
            }
        };
        if !generic || typed.closed_requirement_application(target).is_none() {
            return Ok(member);
        }
        let mut selected = None;
        for receipt in typed
            .machine_specializations
            .iter()
            .filter(|receipt| receipt.template == member)
        {
            if self
                .provider_method_requirement(id, receipt.instance, requirement)?
                .is_some_and(|closed| closed.symbol == target)
            {
                if selected.is_some_and(|known| known != receipt.instance) {
                    return Err(services::unavailable(
                        id,
                        "the selected member has two bodies for the same public arguments",
                    ));
                }
                selected = Some(receipt.instance);
            }
        }
        Ok(selected.unwrap_or(member))
    }
}
