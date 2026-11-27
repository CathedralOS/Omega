//! Match each closed public method to the body that was prepared for its arguments.

use super::Program;
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
                    application
                        .keeps_declared_signatures(typed)
                        .then_some(required)
                })
                .ok_or_else(|| {
                    crate::read::unavailable(
                        context,
                        "conformance lost its prepared requirement header",
                    )
                })?;
            return Ok(vec![(header, supplied)]);
        }
        if !self.generic_templates.contains(&supplied.symbol) {
            self.generic_templates.push(supplied.symbol);
        }
        let unavailable = |reason| crate::read::unavailable(context, reason);
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

    /// Read the public header in the implementation's static parameter scope.
    /// Each declaration owns its binder names. Their order and declared bounds
    /// determine which parameter in the body stands for a public parameter.
    pub(super) fn with_method_parameters<T>(
        &mut self,
        context: MachineId,
        member: &Machine,
        required: &StateSignature,
        read: impl FnOnce(&mut Self) -> Checked<T>,
    ) -> Checked<T> {
        use crate::source_state::data::TypeParameterKind;
        let typed = self.typed;
        let parameters = typed.state_signature_type_parameters(required);
        if !parameters.iter().any(|parameter| {
            matches!(
                parameter.kind,
                TypeParameterKind::Type | TypeParameterKind::Machine { .. }
            )
        }) {
            return read(self);
        }
        let member_parameters = typed.machine_type_parameters(member);
        if parameters.len() != member_parameters.len() {
            return Err(crate::read::unavailable(
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
                return Err(crate::read::unavailable(
                    context,
                    "the method implementation gives a public type parameter a different contract",
                ));
            }
            let value = self
                .scope
                .iter()
                .find_map(|(symbol, ty)| (*symbol == private.symbol).then_some(*ty))
                .ok_or_else(|| {
                    crate::read::unavailable(context, "the implementation lost its type parameter")
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
        let result = (|| {
            for (public, private) in parameters.iter().zip(member_parameters.iter()) {
                match (&public.kind, &private.kind) {
                    (
                        TypeParameterKind::Machine { contract: required },
                        TypeParameterKind::Machine {
                            contract: implementation,
                        },
                    ) => {
                        self.match_method_callback(
                            context,
                            public.symbol,
                            required,
                            implementation,
                        )?;
                    }
                    (TypeParameterKind::Machine { .. }, _)
                    | (_, TypeParameterKind::Machine { .. }) => {
                        return Err(crate::read::unavailable(
                            context,
                            "the method and its requirement disagree on a callable parameter",
                        ));
                    }
                    _ => {}
                }
            }
            read(self)
        })();
        self.scope.truncate(previous_len);
        self.source_scope = previous;
        result
    }

    /// A provider's body can use only the callback contract available to
    /// everyone calling the public requirement, including its effect ceiling.
    fn match_method_callback(
        &mut self,
        context: MachineId,
        parameter: SymbolHandle,
        required: &'program crate::source_state::data::MachineParameterContract,
        implementation: &'program crate::source_state::data::MachineParameterContract,
    ) -> Checked<()> {
        use crate::source_state::data::MachineParameterContract;
        if !matches!(
            (required, implementation),
            (
                MachineParameterContract::Structural(_),
                MachineParameterContract::Structural(_)
            )
        ) && required != implementation
        {
            return Err(crate::read::unavailable(
                context,
                "the method and its requirement need the same nominal callback selection",
            ));
        }
        let signature = |contract| {
            self.typed
                .machine_parameter_contract_view(contract)
                .map(|view| view.signature())
        };
        let required = signature(required).ok_or_else(|| {
            crate::read::unavailable(context, "the public callback has no checked contract")
        })?;
        let implementation = signature(implementation).ok_or_else(|| {
            crate::read::unavailable(
                context,
                "the implementation callback has no checked contract",
            )
        })?;
        let public_signature = self.callback_requirement_signature(context, required)?;
        let private_signature = self.callback_requirement_signature(context, implementation)?;
        if public_signature != private_signature
            || self.static_envelope(context, required, false)?
                != self.static_envelope(context, implementation, false)?
        {
            return Err(crate::read::unavailable(
                context,
                "the method requires a different callback contract from its public requirement",
            ));
        }
        self.binder_requirement(parameter, required, &[])?;
        Ok(())
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
        let unavailable = |reason| crate::read::unavailable(id, reason);
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
                    return Err(crate::read::unavailable(
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
