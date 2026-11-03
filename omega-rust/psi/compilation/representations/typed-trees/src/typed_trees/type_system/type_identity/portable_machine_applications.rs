//! Review identifies a generic body by its declaration and arguments, not the
//! temporary name allocated while compiling it. Executable lookup is unchanged.

use super::IndexIdentityArgument;
mod captures;
mod conformances;

use super::constraint_identity::{atom, compound};
use super::coordinates::CoordinateTag;
use super::identity_context::{TypeIdentityContext, TypeIdentityQualification};
use crate::TypedTrees;
use crate::data::TypeParameterKind;
use crate::static_applications::StaticMachineBindingValue;
use crate::typed_trees::callable_applications::MachineInstantiation;
use language_semantics::type_identity::vocabulary::{Label, NameForm};
use std::cell::{Cell, RefCell};
use symbols::SymbolHandle;

/// Describe one retained callable application for a portable review record.
/// Ordinary declarations return `None`; a malformed application never falls back to
/// its generated name. The ordinary operand encoder visits nested applications.
pub fn portable_machine_application_identity(
    program: &TypedTrees,
    symbol: SymbolHandle,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<Option<String>, String> {
    let projection = PortableMachineApplications::default();
    let invalid = Cell::new(false);
    let context = TypeIdentityContext {
        portable_machine_applications: Some(&projection),
        qualification: TypeIdentityQualification::PackageQualified,
        exact_toolchain_sources,
        missing_exact_nominal_owner: Some(&invalid),
        invalid_application: Some(&invalid),
        invalid_index_argument: Some(&invalid),
        ..Default::default()
    };
    let result = projection.identity(program, symbol, &context)?;
    if let Some(reason) = projection.error.into_inner() {
        return Err(reason);
    }
    if invalid.get() {
        return Err(
            "specialization argument lost its exact owner, static operand, or lifetime scope"
                .into(),
        );
    }
    Ok(result)
}

#[derive(Default)]
pub(crate) struct PortableMachineApplications {
    active: RefCell<Vec<SymbolHandle>>,
    error: RefCell<Option<String>>,
}

impl PortableMachineApplications {
    pub(super) fn nominal_name(
        &self,
        program: &TypedTrees,
        symbol: SymbolHandle,
        context: &TypeIdentityContext<'_>,
    ) -> Option<String> {
        match self.identity(program, symbol, context) {
            Ok(identity) => identity,
            Err(reason) => {
                self.error
                    .borrow_mut()
                    .get_or_insert_with(|| reason.clone());
                Some(atom(NameForm::InvalidDerivedOrigin, &reason))
            }
        }
    }

    fn identity(
        &self,
        program: &TypedTrees,
        symbol: SymbolHandle,
        context: &TypeIdentityContext<'_>,
    ) -> Result<Option<String>, String> {
        let Some(machine) = program
            .machine_by_symbol(symbol)
            .or_else(|| program.machine_holding_state(symbol))
        else {
            return Ok(None);
        };
        let Some(receipt) = program
            .machine_instantiation(machine.symbol)
            .map_err(|reason| format!("callable `{}`: {reason}", machine.name))?
        else {
            return Ok(None);
        };
        if self.active.borrow().contains(&machine.symbol) {
            return Err("specialized callable has a cyclic static argument identity".into());
        }
        self.active.borrow_mut().push(machine.symbol);
        let result = match receipt {
            MachineInstantiation::Specialization(specialization) => {
                self.application(program, specialization, context)
            }
            MachineInstantiation::Capture(capture) => self.capture(program, capture, context),
        };
        self.active.borrow_mut().pop();
        let identity = result.map_err(|reason| format!("callable `{}`: {reason}", machine.name))?;
        if symbol == machine.symbol {
            return Ok(Some(identity));
        }
        let states = program.machine_states(machine);
        if states.first().is_some_and(|entry| entry.symbol == symbol) {
            Ok(Some(identity))
        } else {
            let original = program
                .machine_by_symbol(receipt.template())
                .ok_or("specialized callable lost its original nested declaration owner")?;
            let mut matches = program.machine_states(original).iter().filter(|state| {
                program.instantiated_machine_state_matches(
                    machine.symbol,
                    receipt.template(),
                    state.symbol,
                    symbol,
                )
            });
            let declaration = matches
                .next()
                .ok_or("specialized nested callable does not rejoin its original declaration")?;
            if matches.next().is_some() {
                return Err(
                    "specialized nested callable has ambiguous original declarations".into(),
                );
            }
            Ok(Some(compound(
                CoordinateTag::NestedMachine,
                [
                    identity,
                    atom(CoordinateTag::Name, declaration.name.as_str()),
                ],
            )))
        }
    }

    fn origin(
        &self,
        program: &TypedTrees,
        instance: SymbolHandle,
        template: SymbolHandle,
        normalized_template_identity: &str,
        context: &TypeIdentityContext<'_>,
    ) -> Result<String, String> {
        if instance != template
            && let Some(origin) = self.identity(program, template, context)?
        {
            // A member invocation follows its implementation capture. The
            // captured body's temporary overload name must not reappear here.
            return Ok(origin);
        }
        if normalized_template_identity.is_empty() {
            return Err("callable application lost its original overload identity".into());
        }
        let declaration_context = TypeIdentityContext {
            portable_machine_applications: None,
            ..*context
        };
        Ok(compound(
            CoordinateTag::NamedCallable,
            [
                declaration_context.name(program, template, ""),
                atom(CoordinateTag::Template, normalized_template_identity),
            ],
        ))
    }

    fn application(
        &self,
        program: &TypedTrees,
        receipt: &crate::typed_trees::MachineSpecialization,
        context: &TypeIdentityContext<'_>,
    ) -> Result<String, String> {
        if !receipt.binding_application.is_valid()
            || receipt.normalized_template_identity.is_empty()
        {
            return Err(
                "specialized callable lost its original declaration or binding application".into(),
            );
        }
        let application = program.static_machine_application(receipt.binding_application);
        if application.binding_declaration != receipt.template {
            return Err("specialized callable bindings belong to another declaration".into());
        }
        let template = program
            .machine_by_symbol(receipt.template)
            .ok_or("specialized callable lost its original machine declaration")?;
        let parameters = program
            .data_type_parameters
            .span(receipt.template_parameters)
            .ok_or("specialized callable lost its original static parameters")?;
        let bindings = program.static_machine_application_bindings(receipt.binding_application);
        if parameters.len() != bindings.len() {
            return Err("specialized callable has an incomplete static argument list".into());
        }
        let declaration = self.origin(
            program,
            receipt.instance,
            receipt.template,
            &receipt.normalized_template_identity,
            context,
        )?;
        let argument_context = TypeIdentityContext {
            application_owner: application.lexical_owner,
            application_argument_owner: application.lexical_owner,
            ..*context
        };
        let mut arguments = Vec::with_capacity(parameters.len());
        let mut conformances = receipt.conformance_applications.iter();
        let mut conformance_declarations = receipt.conformance_arguments.iter();
        for parameter in parameters {
            let mut matches = bindings
                .iter()
                .filter(|binding| binding.parameter == parameter.symbol);
            let binding = matches
                .next()
                .ok_or("specialized callable is missing an original parameter binding")?;
            if matches.next().is_some() {
                return Err("specialized callable repeats an original parameter binding".into());
            }
            let (kind, operand) = static_binding_operand(&parameter.kind, &binding.value)?;
            let mut fields = vec![atom(Label::Kind, kind)];
            if let StaticMachineBindingValue::Conformance(argument) = &binding.value {
                let closed = conformances
                    .next()
                    .ok_or("specialized callable lost a checked conformance application")?;
                if conformance_declarations.next() != Some(&closed.declaration) {
                    return Err(
                        "specialized callable has inconsistent checked conformance selection"
                            .into(),
                    );
                }
                fields.push(conformances::project(
                    program,
                    closed,
                    argument,
                    &argument_context,
                )?);
            } else {
                fields.push(super::substitution::index_argument(
                    program,
                    operand,
                    &argument_context,
                ));
            }
            arguments.push(compound(CoordinateTag::Parameter, fields));
        }
        if conformances.next().is_some() || conformance_declarations.next().is_some() {
            return Err(
                "specialized callable retains a conformance without an original parameter".into(),
            );
        }
        let bindings = program
            .static_application_lifetimes_for_declaration(
                receipt.binding_application,
                receipt.template,
            )
            .map_err(str::to_owned)?;
        if bindings.len() != template.lifetime_parameters.len() {
            return Err("specialized callable has an incomplete lifetime argument list".into());
        }
        let mut lifetimes = Vec::with_capacity(bindings.len());
        for parameter in &template.lifetime_parameters {
            let binding = bindings
                .iter()
                .find(|binding| &binding.parameter == parameter)
                .ok_or("specialized callable lost an original lifetime argument")?;
            lifetimes.push(argument_context.lifetime(program, binding.argument.as_str()));
        }
        Ok(compound(
            CoordinateTag::SpecializedCallable,
            [
                declaration,
                compound(CoordinateTag::Arguments, arguments),
                compound(CoordinateTag::Lifetimes, lifetimes),
            ],
        ))
    }
}

fn static_binding_operand<'a>(
    kind: &TypeParameterKind,
    value: &'a StaticMachineBindingValue,
) -> Result<(&'static str, IndexIdentityArgument<'a>), String> {
    Ok(match (kind, value) {
        (TypeParameterKind::Type, StaticMachineBindingValue::Type(reference)) => {
            ("type", IndexIdentityArgument::TypeArgument(*reference))
        }
        (TypeParameterKind::Const { .. }, StaticMachineBindingValue::Const(reference)) => {
            ("const", IndexIdentityArgument::TypeReference(*reference))
        }
        (TypeParameterKind::Value { .. }, StaticMachineBindingValue::Const(reference)) => {
            ("value", IndexIdentityArgument::TypeReference(*reference))
        }
        (
            TypeParameterKind::Value { .. },
            StaticMachineBindingValue::RuntimeValue {
                type_reference,
                subject,
                expression,
            },
        ) if !subject.is_valid() && !expression.is_valid() => (
            "runtime-carrier",
            IndexIdentityArgument::TypeArgument(*type_reference),
        ),
        (TypeParameterKind::Machine { .. }, StaticMachineBindingValue::Machine(argument)) => {
            ("machine", IndexIdentityArgument::StaticArgument(argument))
        }
        (TypeParameterKind::Implementation, StaticMachineBindingValue::Conformance(argument)) => (
            "conformance",
            IndexIdentityArgument::StaticArgument(argument),
        ),
        _ => {
            return Err(
                "specialized callable argument differs from its original parameter kind".into(),
            );
        }
    })
}
