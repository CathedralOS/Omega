pub(crate) mod applications {
    use super::super::CallableApplication;
    use crate::source_state::{TypedTrees, expression, name::Identifier};
    use symbols::SymbolHandle;

    impl CallableApplication {
        pub(crate) fn retain_selected_call_application(
            &self,
            program: &mut TypedTrees,
            call: &mut expression::TableCallExpression,
            target: SymbolHandle,
            name: &Identifier,
            supplied: &expression::StaticMachineArgument,
        ) -> Result<(), &'static str> {
            use crate::source_state::static_applications::{
                StaticMachineBinding, StaticMachineBindingValue, StaticMachineLifetimeBinding,
            };
            if !self.static_argument_owner.is_valid() {
                return Err("selected contract helper lost its supplying declaration");
            }
            let configured = supplied.application.as_deref();
            if configured.is_some_and(|application| application.member.is_valid()) {
                return Err(
                    "selected contract helper needs its separately retained member selection",
                );
            }
            let explicit = configured.map_or(&[][..], |application| application.arguments.as_ref());
            let local_arguments = if call.authored_machine_application.is_valid()
                && !program
                    .static_machine_application_bindings(call.authored_machine_application)
                    .is_empty()
            {
                Self::retained_application_arguments(program, call.authored_machine_application)?
            } else if call.authored_machine_application.is_valid()
                && !program
                    .static_machine_application_arguments(call.authored_machine_application)
                    .is_empty()
            {
                program
                    .static_machine_application_arguments(call.authored_machine_application)
                    .to_vec()
            } else {
                call.machine_arguments.to_vec()
            };
            if !explicit.is_empty() && !local_arguments.is_empty() {
                return Err("selected contract helper has two argument lists for one declaration");
            }
            let arguments = if explicit.is_empty() {
                local_arguments
            } else {
                explicit.to_vec()
            };
            let (declaration, parameters, evidence, lifetime_parameters) = if let Some(machine) =
                program
                    .machine_holding_state(target)
                    .or_else(|| program.machine_by_symbol(target))
            {
                (
                    machine.symbol,
                    program.machine_type_parameters(machine).to_vec(),
                    machine.conformance_bounds.clone(),
                    machine.lifetime_parameters.clone(),
                )
            } else if let Some((_, signature)) = program.machine_parameter_signature(target) {
                (
                    signature.symbol,
                    program.state_signature_type_parameters(signature).to_vec(),
                    signature.conformance_bounds.clone(),
                    signature.lifetime_parameters.clone(),
                )
            } else {
                return Err("selected contract helper lost its parameter declaration");
            };
            if arguments.len() != parameters.len() + evidence.len() {
                return Err("selected contract helper needs its complete configured arguments");
            }
            let mut bindings = Vec::with_capacity(arguments.len());
            for (parameter, argument) in parameters.iter().zip(&arguments) {
                let value = match parameter.kind {
                    crate::source_state::data::TypeParameterKind::Machine { .. } => {
                        StaticMachineBindingValue::Machine(argument.clone())
                    }
                    crate::source_state::data::TypeParameterKind::Proof { .. } => {
                        StaticMachineBindingValue::Proof(argument.clone())
                    }
                    crate::source_state::data::TypeParameterKind::Const { .. } => {
                        StaticMachineBindingValue::Const(
                            program.static_operand_type_reference_in_scope(
                                argument,
                                self.static_argument_owner,
                            )?,
                        )
                    }
                    crate::source_state::data::TypeParameterKind::Value { .. } => {
                        return Err("selected contract helper has an unclosed runtime operand");
                    }
                    _ => StaticMachineBindingValue::Type(
                        program.static_operand_type_reference_in_scope(
                            argument,
                            self.static_argument_owner,
                        )?,
                    ),
                };
                bindings.push(StaticMachineBinding {
                    parameter: parameter.symbol,
                    value,
                });
            }
            for (bound, argument) in evidence.iter().zip(&arguments[parameters.len()..]) {
                bindings.push(StaticMachineBinding {
                    parameter: bound.binder,
                    value: StaticMachineBindingValue::Conformance(argument.clone()),
                });
            }
            let explicit_lifetimes = configured.map_or(&[][..], |application| {
                application.lifetime_arguments.as_ref()
            });
            if !explicit_lifetimes.is_empty()
                && explicit_lifetimes.len() != lifetime_parameters.len()
            {
                return Err("selected contract helper has an incomplete lifetime application");
            }
            let lifetimes = lifetime_parameters
                .iter()
                .zip(explicit_lifetimes)
                .map(|(parameter, argument)| {
                    crate::source_state::type_identity::lifetime_parameter_identity(
                        program,
                        self.static_argument_owner,
                        argument.as_str(),
                    )
                    .ok_or("selected contract helper lifetime has no supplying declaration")?;
                    Ok(StaticMachineLifetimeBinding {
                        parameter: parameter.clone(),
                        argument: argument.clone(),
                    })
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
            let application = program.expression_table.retain_static_machine_application(
                target,
                name.clone(),
                &arguments,
            );
            let row = program
                .expression_table
                .static_machine_application_mut(application);
            row.lexical_owner = self.static_argument_owner;
            row.binding_declaration = declaration;
            program
                .expression_table
                .set_static_machine_application_bindings(application, &bindings);
            program
                .expression_table
                .set_static_machine_application_lifetimes(application, declaration, &lifetimes);
            call.machine_arguments = arguments.into();
            call.authored_machine_application = application;
            Ok(())
        }
    }

    impl CallableApplication {
        pub(crate) fn retain_prepared_call_application(
            &self,
            program: &mut TypedTrees,
            call: &mut expression::TableCallExpression,
            provider: crate::source_state::static_applications::StaticMachineApplicationHandle,
        ) -> Result<(), &'static str> {
            let source = program.static_machine_application(provider).clone();
            let bindings = program
                .static_machine_application_bindings(provider)
                .to_vec();
            if program
                .machine_parameter_signature(source.binding_declaration)
                .is_some()
            {
                if !bindings.is_empty() {
                    return Err(
                        "forwarded contract helper unexpectedly contains configured parameters",
                    );
                }
                let supplied = expression::StaticMachineArgument {
                    symbol: source.registration_operation,
                    path: Box::from([source.target.clone()]),
                    ..Default::default()
                };
                return self.retain_selected_call_application(
                    program,
                    call,
                    source.registration_operation,
                    &source.target,
                    &supplied,
                );
            }

            let lifetimes = program
                .static_machine_application_lifetimes(provider)
                .to_vec();
            let arguments = Self::retained_application_arguments(program, provider)?;
            if !call.machine_arguments.is_empty() {
                return Err(
                    "determined contract helper cannot replace a universal call application",
                );
            }
            let application = program.expression_table.retain_static_machine_application(
                source.registration_operation,
                source.target,
                &arguments,
            );
            let row = program
                .expression_table
                .static_machine_application_mut(application);
            row.lexical_owner = source.lexical_owner;
            row.binding_declaration = source.binding_declaration;
            row.conformance_selection = source.conformance_selection;
            program
                .expression_table
                .set_static_machine_application_bindings(application, &bindings);
            program
                .expression_table
                .set_static_machine_application_lifetimes(
                    application,
                    source.lifetime_binding_declaration,
                    &lifetimes,
                );
            call.machine_arguments = arguments.into();
            call.authored_machine_application = application;
            Ok(())
        }
    }

    impl CallableApplication {
        fn retained_application_arguments(
            program: &TypedTrees,
            application: crate::source_state::static_applications::StaticMachineApplicationHandle,
        ) -> Result<Vec<expression::StaticMachineArgument>, &'static str> {
            use crate::source_state::static_applications::StaticMachineBindingValue;
            let declaration = program
                .static_machine_application(application)
                .binding_declaration;
            let bindings = program.static_machine_application_bindings(application);
            let declared_parameters = if let Some(machine) = program.machine_by_symbol(declaration)
            {
                program
                    .machine_type_parameters(machine)
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .chain(machine.conformance_bounds.iter().map(|bound| bound.binder))
                    .collect::<Vec<_>>()
            } else if let Some((_, signature)) = program.machine_parameter_signature(declaration) {
                program
                    .state_signature_type_parameters(signature)
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .chain(
                        signature
                            .conformance_bounds
                            .iter()
                            .map(|bound| bound.binder),
                    )
                    .collect::<Vec<_>>()
            } else {
                return Err("prepared contract helper lost its receiving declaration");
            };
            let mut arguments = Vec::with_capacity(declared_parameters.len());
            for parameter in declared_parameters {
                let binding = bindings
                    .iter()
                    .find(|binding| binding.parameter == parameter)
                    .ok_or("prepared contract helper lost one configured parameter")?;
                arguments.push(match &binding.value {
                    StaticMachineBindingValue::Type(reference)
                    | StaticMachineBindingValue::Const(reference) => {
                        program.static_operand_from_type_reference(*reference)?
                    }
                    StaticMachineBindingValue::Machine(argument)
                    | StaticMachineBindingValue::Proof(argument)
                    | StaticMachineBindingValue::Conformance(argument) => argument.clone(),
                    StaticMachineBindingValue::RuntimeValue { .. } => {
                        return Err("prepared contract helper has an unclosed runtime operand");
                    }
                });
            }
            Ok(arguments)
        }
    }
}

use super::CallableApplication;
use crate::source_state::{TypedTrees, expression, name::Identifier};
use symbols::{SymbolHandle, SymbolKind};

pub(crate) struct AppliedCallTarget {
    parameter: SymbolHandle,
    target: SymbolHandle,
    name: Identifier,
    attached_data: Option<(SymbolHandle, Identifier)>,
    supplied: Option<expression::StaticMachineArgument>,
    provider: Option<crate::source_state::static_applications::StaticMachineApplicationHandle>,
}

impl CallableApplication {
    pub(crate) fn selected_call_target(
        &self,
        program: &TypedTrees,
        parameter: SymbolHandle,
    ) -> Result<Option<AppliedCallTarget>, &'static str> {
        if !parameter.is_valid()
            || program.symbols.get(parameter).kind != SymbolKind::MachineParameter
        {
            return Ok(None);
        }
        let supplied = self
            .static_arguments
            .iter()
            .rev()
            .find(|(symbol, _)| *symbol == parameter)
            .map(|(_, argument)| argument.clone());
        let provider = self
            .selected_providers
            .iter()
            .find(|(binder, _)| *binder == parameter)
            .map(|(_, provider)| *provider);
        let (selected, name) = if let Some(provider) = provider {
            let record = program.static_machine_application(provider);
            (record.registration_operation, record.target.clone())
        } else if let Some(argument) = &supplied {
            let name = argument
                .path
                .last()
                .cloned()
                .ok_or("selected contract helper lost its diagnostic name")?;
            (argument.symbol, name)
        } else {
            let Some((_, reference)) = self
                .types
                .iter()
                .rev()
                .find(|(symbol, _)| *symbol == parameter)
            else {
                return Ok(None);
            };
            let crate::source_state::types::TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(*reference)
            else {
                return Err("applied contract call needs an exact named callable declaration");
            };
            (*symbol, name.clone())
        };
        if !selected.is_valid()
            || !matches!(
                program.symbols.get(selected).kind,
                SymbolKind::Machine | SymbolKind::State | SymbolKind::MachineParameter
            )
        {
            return Err("applied contract call lost its selected callable declaration");
        }
        if selected == parameter
            && supplied
                .as_ref()
                .is_none_or(|argument| argument.application.is_none())
        {
            return Ok(None);
        }
        let machine = program
            .machine_by_symbol(selected)
            .or_else(|| program.machine_holding_state(selected));
        let target = if let Some(machine) = machine
            && machine.symbol == selected
        {
            program
                .machine_states(machine)
                .first()
                .ok_or("applied contract call selected a machine without its entry")?
                .symbol
        } else {
            selected
        };
        let attached_data = match machine {
            Some(machine) if machine.attached_data_symbol.is_valid() => Some((
                machine.attached_data_symbol,
                machine.attached_data.clone().ok_or(
                    "applied contract call lost its attached declaration's diagnostic name",
                )?,
            )),
            _ => None,
        };
        Ok(Some(AppliedCallTarget {
            parameter,
            target,
            name,
            attached_data,
            supplied,
            provider,
        }))
    }

    pub(crate) fn apply_call_target(
        &self,
        program: &mut TypedTrees,
        call: &mut expression::TableCallExpression,
        selected: AppliedCallTarget,
    ) -> Result<(), &'static str> {
        if !call.static_machine_parameter.is_valid() {
            call.static_machine_parameter = self.symbol(selected.parameter);
        }
        if let Some(provider) = selected.provider {
            self.retain_prepared_call_application(program, call, provider)?;
        } else if let Some(argument) = &selected.supplied {
            self.retain_selected_call_application(
                program,
                call,
                selected.target,
                &selected.name,
                argument,
            )?;
        }
        call.target_symbol = selected.target;
        call.target = selected.name;
        if let Some((symbol, name)) = selected.attached_data {
            call.receiver = program.expression_table.insert_name_path(
                &expression::NamePath::resolved_from_iter([name], symbol, symbol),
            );
        }
        Ok(())
    }
}
