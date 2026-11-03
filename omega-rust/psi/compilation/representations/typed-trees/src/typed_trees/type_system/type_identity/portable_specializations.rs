//! Review identifies a generic body by its declaration and arguments, not the
//! temporary name allocated while compiling it. Executable lookup is unchanged.

use super::IndexIdentityArgument;
use super::constraint_identity::{atom, byte_atom, compound};
use super::coordinates::CoordinateTag;
use super::identity_context::{TypeIdentityContext, TypeIdentityQualification};
use crate::TypedTrees;
use crate::data::TypeParameterKind;
use crate::static_applications::StaticMachineBindingValue;
use language_semantics::type_identity::vocabulary::{Label, NameForm};
use std::cell::{Cell, RefCell};
use symbols::SymbolHandle;

/// Describe one retained specialization for a portable review record. Ordinary
/// declarations return `None`; a malformed specialization never falls back to
/// its generated name. The ordinary operand encoder visits nested applications.
pub fn portable_machine_specialization_identity(
    program: &TypedTrees,
    symbol: SymbolHandle,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<Option<String>, String> {
    let projection = PortableSpecializations::default();
    let invalid = Cell::new(false);
    let context = TypeIdentityContext {
        portable_specializations: Some(&projection),
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
pub(crate) struct PortableSpecializations {
    active: RefCell<Vec<SymbolHandle>>,
    error: RefCell<Option<String>>,
}

impl PortableSpecializations {
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
        let mut receipts = program
            .machine_specializations
            .iter()
            .filter(|receipt| receipt.instance == machine.symbol);
        let Some(receipt) = receipts.next() else {
            return Ok(None);
        };
        if receipts.next().is_some() {
            return Err("specialized callable has multiple retained applications".into());
        }
        if self.active.borrow().contains(&machine.symbol) {
            return Err("specialized callable has a cyclic static argument identity".into());
        }
        self.active.borrow_mut().push(machine.symbol);
        let result = self.application(program, receipt, context);
        self.active.borrow_mut().pop();
        let identity = result?;
        if symbol == machine.symbol {
            return Ok(Some(identity));
        }
        let states = program.machine_states(machine);
        let selected = states
            .iter()
            .find(|state| state.symbol == symbol)
            .ok_or("specialized callable lost its selected nested machine")?;
        if states.first().is_some_and(|entry| entry.symbol == symbol) {
            Ok(Some(identity))
        } else {
            let original = program
                .machine_by_symbol(receipt.template)
                .ok_or("specialized callable lost its original nested declaration owner")?;
            let origin = program
                .symbols
                .symbol_provenance_source_span(selected.symbol)
                .ok_or("specialized nested callable lost its declaration provenance")?;
            let mut matches = program.machine_states(original).iter().filter(|state| {
                state.name == selected.name
                    && program.symbols.symbol_provenance_source_span(state.symbol) == Some(origin)
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
        // The template may also be the first executable instance. Its original
        // overload is retained separately, so naming it must not recurse.
        let declaration_context = TypeIdentityContext {
            portable_specializations: None,
            ..*context
        };
        let declaration = declaration_context.name(program, receipt.template, "");
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
            let (kind, operand) = match (&parameter.kind, &binding.value) {
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
                (
                    TypeParameterKind::Machine { .. },
                    StaticMachineBindingValue::Machine(argument),
                ) => ("machine", IndexIdentityArgument::StaticArgument(argument)),
                (
                    TypeParameterKind::Implementation,
                    StaticMachineBindingValue::Conformance(argument),
                ) => (
                    "conformance",
                    IndexIdentityArgument::StaticArgument(argument),
                ),
                _ => {
                    return Err(
                        "specialized callable argument differs from its original parameter kind"
                            .into(),
                    );
                }
            };
            let identity = super::substitution::index_argument(program, operand, &argument_context);
            let mut fields = vec![atom(Label::Kind, kind), identity.clone()];
            if matches!(binding.value, StaticMachineBindingValue::Conformance(_)) {
                let closed = conformances
                    .next()
                    .ok_or("specialized callable lost a checked conformance application")?;
                if conformance_declarations.next() != Some(&closed.declaration)
                    || closed.identity_encoding.is_empty()
                {
                    return Err(
                        "specialized callable has inconsistent checked conformance selection"
                            .into(),
                    );
                }
                let selected = crate::expression::StaticMachineArgument {
                    symbol: closed.declaration,
                    application: Some(Box::new(crate::expression::StaticSymbolApplication {
                        member: Default::default(),
                        lifetime_arguments: closed
                            .lifetime_arguments
                            .iter()
                            .map(crate::name::Identifier::generated)
                            .collect(),
                        arguments: closed.arguments.clone(),
                    })),
                    ..Default::default()
                };
                let selected_identity = super::substitution::index_argument(
                    program,
                    IndexIdentityArgument::StaticArgument(&selected),
                    &argument_context,
                );
                if identity != selected_identity {
                    return Err("specialized callable conformance binding differs from its checked application".into());
                }
                // Keep both the full own telescope above and the checked
                // selection's complete encoding; a compact hash is not enough.
                fields.push(byte_atom(
                    CoordinateTag::Declaration,
                    &closed.identity_encoding,
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
                atom(
                    CoordinateTag::Template,
                    &receipt.normalized_template_identity,
                ),
                compound(CoordinateTag::Arguments, arguments),
                compound(CoordinateTag::Lifetimes, lifetimes),
            ],
        ))
    }
}
