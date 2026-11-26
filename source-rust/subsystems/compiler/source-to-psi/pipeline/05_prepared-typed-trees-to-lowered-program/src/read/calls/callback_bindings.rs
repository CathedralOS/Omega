//! Bind a supplied static callback to its declared requirement and source occurrence.
//! A generic checking frame may inspect a selection authored by a different machine.

use super::{
    AuthoredDeclarationSelectionOccurrenceId, CallTarget, Checked, Construct, Expression,
    ExpressionKind, MachineId, Program, Signature, StaticMachineArgument, StaticMachineTarget,
    SymbolHandle, SymbolKind, Unserved, refuse,
};

struct RequiredCallback<'a> {
    parameter: SymbolHandle,
    source: &'a crate::source_state::signature::StateSignature,
    signature: Signature,
}

struct CallbackConformance {
    selected: MachineId,
    arguments: Vec<terminal_psi::static_reach::StaticReachArgument>,
    ordinal: u32,
}

impl Program<'_> {
    /// A call through a callable parameter is selected by the enclosing
    /// specialization. The authored call still names the parameter itself.
    pub(in super::super) fn static_machine_target(
        &self,
        id: MachineId,
        target: SymbolHandle,
        binder: SymbolHandle,
    ) -> Checked<(SymbolHandle, Option<StaticMachineTarget>)> {
        if !binder.is_valid() {
            return Ok((target, None));
        }
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Call(what));
        let owner = self
            .symbols
            .get(id)
            .ok_or_else(|| unserved("static call has no owner"))?;
        let (position, argument) = self
            .typed
            .static_machine_binding(*owner, binder, target)
            .map_err(unserved)?
            .ok_or_else(|| unserved("static call target is not selected by its binder"))?;
        let occurrence = self.static_argument_selection(id, argument)?;
        Ok((
            binder,
            Some(StaticMachineTarget {
                position,
                target,
                occurrence,
            }),
        ))
    }

    pub(in crate::read) fn static_argument_selections(
        &self,
        id: MachineId,
        argument: &StaticMachineArgument,
    ) -> Checked<()> {
        if self
            .typed
            .symbols
            .symbols()
            .nodes()
            .is_valid(argument.symbol)
            && matches!(
                self.typed.symbols.get(argument.symbol).kind,
                SymbolKind::Machine | SymbolKind::State
            )
        {
            self.static_argument_selection(id, argument)?;
        }
        if let Some(application) = &argument.application {
            for argument in &application.arguments {
                self.static_argument_selections(id, argument)?;
            }
            if application.member.is_valid() {
                let member = self
                    .typed
                    .expression_table
                    .static_member(application.member)
                    .ok_or_else(|| {
                        refuse(
                            id,
                            Construct::Machine,
                            Unserved::Call("static argument lost its selected member"),
                        )
                    })?;
                self.static_argument_selections(id, member)?;
            }
        }
        Ok(())
    }

    fn static_argument_selection(
        &self,
        id: MachineId,
        argument: &StaticMachineArgument,
    ) -> Checked<AuthoredDeclarationSelectionOccurrenceId> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Call(what));
        let selection = argument
            .authored_selection
            .and_then(|occurrence| self.typed.authored_declaration_selections().get(occurrence))
            .ok_or_else(|| unserved("static call argument lost its authored selection"))?;
        use crate::declaration_selection::AuthoredDeclarationSelectionKind as Kind;
        if !matches!(
            selection.kind(),
            Kind::StaticArgument | Kind::InstalledStaticArgument
        ) || selection.target().as_resolved().is_none_or(|selected| {
            let selected = selected.selected_symbol();
            !self.typed.symbols.symbols().nodes().is_valid(selected)
                || (selected != argument.symbol
                    && !self.instantiates_callable(argument.symbol, selected)
                    && self.entered_machine(selected) != self.entered_machine(argument.symbol))
        }) {
            return Err(unserved(
                "static call argument changed its authored selection",
            ));
        }
        let selection_source = selection.source_span();
        let file = self
            .typed
            .symbols
            .source_file(selection_source)
            .ok_or_else(|| unserved("static call selection has no source file"))?;
        if selection_source.span.start >= selection_source.span.end
            || file
                .source
                .get(selection_source.span.start..selection_source.span.end)
                .is_none()
        {
            return Err(unserved(
                "static call selection has no authored source coordinate",
            ));
        }
        let target = if self
            .typed
            .trait_machine_signature(argument.symbol)
            .is_some()
        {
            argument.symbol
        } else {
            self.call_target_symbol(id, argument.symbol)?
        };
        self.visible_to_from(id, target, Some(*selection))?;
        Ok(selection.occurrence_id())
    }

    pub(in super::super) fn retain_static_reach(
        &self,
        id: MachineId,
        mut call: Expression,
        binding: Option<StaticMachineTarget>,
    ) -> Checked<Expression> {
        if let Some(binding) = binding {
            let ExpressionKind::Call {
                callee: CallTarget::Machine(_),
                static_reach_binding,
                ..
            } = &mut call.kind
            else {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Call("static callback did not retain its exact machine invocation"),
                ));
            };
            *static_reach_binding = Some(binding.position);
        }
        Ok(call)
    }

    pub(super) fn callable_application_arguments(
        &mut self,
        caller: MachineId,
        callee: MachineId,
        lexical_owner: SymbolHandle,
        arguments: &[(SymbolHandle, &StaticMachineArgument)],
    ) -> Checked<Vec<terminal_psi::static_reach::StaticReachArgument>> {
        let mut bindings = Vec::with_capacity(arguments.len());
        for &(parameter, argument) in arguments {
            let required = self
                .typed
                .retained_static_machine_contract(parameter)
                .ok_or_else(|| {
                    refuse(
                        caller,
                        Construct::Machine,
                        Unserved::Call("callable argument lost its originating contract"),
                    )
                })?
                .signature();
            let scope = self.scopes.get(callee).cloned().ok_or_else(|| {
                refuse(
                    caller,
                    Construct::Machine,
                    Unserved::Call("callable argument lost its receiving type scope"),
                )
            })?;
            let source_scope = self.source_scopes.get(callee).cloned().ok_or_else(|| {
                refuse(
                    caller,
                    Construct::Machine,
                    Unserved::Call("callable argument lost its receiving source scope"),
                )
            })?;
            let outer = std::mem::replace(&mut self.scope, scope);
            let outer_source = std::mem::replace(&mut self.source_scope, source_scope);
            let read = self.binder_requirement(parameter, required, &[]);
            self.source_scope = outer_source;
            self.scope = outer;
            let requirement = read?;
            let signature = self.signatures.get(requirement).cloned().ok_or_else(|| {
                refuse(
                    caller,
                    Construct::Machine,
                    Unserved::Call("callable argument lost its required header"),
                )
            })?;
            let checked = self.callback_conformance(
                caller,
                RequiredCallback {
                    parameter,
                    source: required,
                    signature,
                },
                argument,
                lexical_owner,
            )?;
            let position = self.static_reach_parameter(callee, parameter, requirement)?;
            bindings.push(terminal_psi::static_reach::StaticReachArgument {
                parameter: position,
                requirement,
                selected: checked.selected,
                arguments: checked.arguments,
                conformance: checked.ordinal,
            });
        }
        Ok(bindings)
    }

    /// Closing a generic application does not discharge its callback contracts.
    /// Read their full applied headers and ask the ordinary refinement checker.
    pub(in super::super) fn closed_callback_requirements(
        &mut self,
        caller: MachineId,
        symbol: SymbolHandle,
    ) -> Checked<()> {
        let unavailable = |what| refuse(caller, Construct::Machine, Unserved::Call(what));
        let Some(crate::source_state::MachineInstantiation::Specialization(receipt)) = self
            .typed
            .machine_instantiation(symbol)
            .map_err(unavailable)?
        else {
            return Ok(());
        };
        self.typed
            .specialization_binding_declaration(receipt)
            .map_err(unavailable)?;
        let application = receipt.binding_application;
        let invocation = self
            .typed
            .expression_table
            .static_machine_application(application);
        let lexical_owner = invocation.lexical_owner;
        let contracts = self
            .typed
            .static_callback_requirements(application)
            .map_err(unavailable)?;
        for contract in contracts {
            let signature = self.callback_requirement_signature(caller, contract.required)?;
            self.callback_conformance(
                caller,
                RequiredCallback {
                    parameter: contract.parameter,
                    source: contract.required,
                    signature,
                },
                contract.argument,
                lexical_owner,
            )?;
        }
        Ok(())
    }

    fn callback_conformance(
        &mut self,
        caller: MachineId,
        required: RequiredCallback<'_>,
        argument: &StaticMachineArgument,
        lexical_owner: SymbolHandle,
    ) -> Checked<CallbackConformance> {
        let RequiredCallback {
            parameter,
            source: required,
            signature,
        } = required;
        let unavailable = |reason| refuse(caller, Construct::Machine, Unserved::Call(reason));
        let supplier = self
            .typed
            .static_operand_region_supplier(argument, lexical_owner)
            .map_err(unavailable)?;
        let (implementation, selected, nested) = if self
            .typed
            .symbols
            .symbols()
            .nodes()
            .is_valid(argument.symbol)
            && self.typed.symbols.get(argument.symbol).kind == SymbolKind::MachineParameter
        {
            if argument.application.is_some() {
                return Err(unavailable(
                    "a forwarded callback parameter carries another application",
                ));
            }
            let supplied = self
                .symbols
                .get(caller)
                .and_then(|owner| self.typed.machine_by_symbol(*owner))
                .and_then(|owner| {
                    self.typed
                        .machine_parameter_signature_in(owner, argument.symbol)
                })
                .ok_or_else(|| {
                    unavailable("forwarded callback is outside its caller's lexical context")
                })?;
            let projection = self.binder_requirement(argument.symbol, supplied, &[])?;
            let position = self.static_reach_parameter(caller, argument.symbol, projection)?;
            let parameter = terminal_psi::static_reach::StaticReachParameter {
                owner: caller,
                parameter: position,
                requirement: projection,
            };
            (
                crate::callables::static_contracts::SourceImplementation::Parameter(parameter),
                projection,
                Vec::new(),
            )
        } else {
            self.static_argument_selection(caller, argument)?;
            let (body, nested) = self.selected_callback_body(caller, argument, supplier)?;
            (
                crate::callables::static_contracts::SourceImplementation::Machine(body),
                body,
                nested,
            )
        };
        let application = self
            .typed
            .static_operand_identity(
                crate::source_state::type_identity::StaticOperandIdentityRequest {
                    operand:
                        crate::source_state::type_identity::IndexIdentityArgument::StaticArgument(
                            argument,
                        ),
                    lexical_owner: supplier,
                    substitutions: &[],
                },
            )
            .and_then(|identity| self.typed.type_identity_text(identity))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| {
                refuse(
                    caller,
                    Construct::Machine,
                    Unserved::Call("callable argument lost its exact application identity"),
                )
            })?;
        let envelope = self.static_envelope(caller, required, false)?;
        let conformance =
            u32::try_from(self.static_contracts.conformances.len()).map_err(|_| {
                refuse(
                    caller,
                    Construct::Machine,
                    Unserved::Call("too many callback conformance questions"),
                )
            })?;
        self.static_contracts.conformances.push(
            crate::callables::static_contracts::SourceConformance {
                declaration: parameter,
                requirement: required.symbol,
                application,
                implementation,
                signature,
                envelope,
                provider: None,
            },
        );
        Ok(CallbackConformance {
            selected,
            arguments: nested,
            ordinal: conformance,
        })
    }

    fn selected_callback_body(
        &mut self,
        caller: MachineId,
        argument: &StaticMachineArgument,
        supplier: SymbolHandle,
    ) -> Checked<(
        MachineId,
        Vec<terminal_psi::static_reach::StaticReachArgument>,
    )> {
        let target = self.entered_machine(argument.symbol).ok_or_else(|| {
            refuse(
                caller,
                Construct::Machine,
                Unserved::Call("supplied callable has no declared machine body"),
            )
        })?;
        let actual = if let Some(applied) = &argument.application {
            if applied.member.is_valid() || !applied.lifetime_arguments.is_empty() {
                return Err(refuse(
                    caller,
                    Construct::Machine,
                    Unserved::Call(
                        "supplied callable has an unsupported member or lifetime application",
                    ),
                ));
            }
            Some(self.application_arguments(
                caller,
                supplier,
                target,
                argument.authored_machine_application,
                &applied.arguments,
            )?)
        } else {
            self.inherited_application_arguments(caller, target)?
        };
        let (body, nested) = if let Some(actual) = actual {
            let body = self.application(
                caller,
                target,
                super::ApplicationTypes::Machine(actual.types, actual.type_sources),
            )?;
            let nested = self.callable_application_arguments(
                caller,
                body,
                actual.lexical_owner,
                &actual.callables,
            )?;
            // A selected callable with scalar consts needs a fully applied
            // required header; it cannot discard those values as metadata.
            if !actual.values.is_empty() {
                return Err(refuse(
                    caller,
                    Construct::Machine,
                    Unserved::Call("supplied callable retains scalar const inputs"),
                ));
            }
            (body, nested)
        } else {
            (self.machine(target)?, Vec::new())
        };
        self.fixed_contracts
            .push(super::super::FixedContract::Interface(body));
        Ok((body, nested))
    }

    /// The source binder and its full applied requirement travel together.
    /// A shared body can then substitute each call's actual callback without
    /// learning an effect from the current set of callers.
    pub(super) fn static_reach_parameter(
        &mut self,
        owner: MachineId,
        parameter: SymbolHandle,
        requirement: MachineId,
    ) -> Checked<u32> {
        let unavailable = |what| refuse(owner, Construct::Machine, Unserved::Call(what));
        let source = self
            .symbols
            .get(owner)
            .copied()
            .ok_or_else(|| unavailable("static callback lost its source owner"))?;
        let position = self
            .typed
            .declaration_type_parameters(source)
            .iter()
            .position(|declared| declared.symbol == parameter)
            .and_then(|position| u32::try_from(position).ok())
            .ok_or_else(|| unavailable("static callback lost its exact parameter position"))?;
        let row = terminal_psi::static_reach::StaticReachParameter {
            owner,
            parameter: position,
            requirement,
        };
        match self.static_reach_parameters.binary_search(&row) {
            Ok(_) => {}
            Err(position) => self.static_reach_parameters.insert(position, row),
        }
        Ok(position)
    }
}
