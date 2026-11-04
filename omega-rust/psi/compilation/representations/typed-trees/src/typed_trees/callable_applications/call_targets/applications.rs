//! Keep each helper call's complete arguments and supplying scope.
//! Configured helpers and forwarded universal binders retain their own parameter owners.
use super::super::CallableApplication;
use crate::{TypedTrees, expression, name::Identifier};
use symbols::SymbolHandle;

impl CallableApplication {
    /// The selected helper owns these parameters. Its configured arguments come
    /// from the supplier and must not be substituted through the copied contract.
    pub(super) fn retain_selected_call_application(
        &self,
        program: &mut TypedTrees,
        call: &mut expression::TableCallExpression,
        target: SymbolHandle,
        name: &Identifier,
        supplied: &expression::StaticMachineArgument,
    ) -> Result<(), &'static str> {
        use crate::static_applications::{
            StaticMachineBinding, StaticMachineBindingValue, StaticMachineLifetimeBinding,
        };
        if !self.static_argument_owner.is_valid() {
            return Err("selected contract helper lost its supplying declaration");
        }
        let configured = supplied.application.as_deref();
        if configured.is_some_and(|application| application.member.is_valid()) {
            return Err("selected contract helper needs its separately retained member selection");
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
                crate::data::TypeParameterKind::Machine { .. } => {
                    StaticMachineBindingValue::Machine(argument.clone())
                }
                crate::data::TypeParameterKind::Const { .. } => StaticMachineBindingValue::Const(
                    program.static_operand_type_reference_in_scope(
                        argument,
                        self.static_argument_owner,
                    )?,
                ),
                crate::data::TypeParameterKind::Value { .. } => {
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
        if !explicit_lifetimes.is_empty() && explicit_lifetimes.len() != lifetime_parameters.len() {
            return Err("selected contract helper has an incomplete lifetime application");
        }
        let lifetimes = lifetime_parameters
            .iter()
            .zip(explicit_lifetimes)
            .map(|(parameter, argument)| {
                crate::type_identity::lifetime_parameter_identity(
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
    pub(super) fn retain_prepared_call_application(
        &self,
        program: &mut TypedTrees,
        call: &mut expression::TableCallExpression,
        provider: crate::static_applications::StaticMachineApplicationHandle,
    ) -> Result<(), &'static str> {
        let source = program.static_machine_application(provider).clone();
        let bindings = program
            .static_machine_application_bindings(provider)
            .to_vec();
        if program
            .machine_parameter_signature(source.binding_declaration)
            .is_some()
        {
            // A forwarded function still has its universal parameters. Its
            // provider record selects a binder, not one configured instance.
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
            return Err("determined contract helper cannot replace a universal call application");
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
        application: crate::static_applications::StaticMachineApplicationHandle,
    ) -> Result<Vec<expression::StaticMachineArgument>, &'static str> {
        use crate::static_applications::StaticMachineBindingValue;
        let declaration = program
            .static_machine_application(application)
            .binding_declaration;
        let bindings = program.static_machine_application_bindings(application);
        let declared_parameters = if let Some(machine) = program.machine_by_symbol(declaration) {
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
                | StaticMachineBindingValue::Conformance(argument) => argument.clone(),
                StaticMachineBindingValue::RuntimeValue { .. } => {
                    return Err("prepared contract helper has an unclosed runtime operand");
                }
            });
        }
        Ok(arguments)
    }
}
