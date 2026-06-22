use crate::expression_types::{
    argument_matches_type_reference_handle, expression_type_name_handle,
};
use crate::symbols::TopLevelSymbols;
use crate::type_references::{
    TypeReferenceOwner, validate_type_reference_handle_with_type_parameters,
};
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::ExpressionHandle;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::types::TypeReferenceHandle;
use std::fmt;

/// A version-scoped machine (`machine Counter::increment::v1`) attaches to the
/// historical shape `Counter::v1`. That shape only exists when the data
/// declaration carries a matching `version v1 { ... }` block; targeting an
/// undeclared version (or a version of unknown data) is a declared-history
/// contradiction.
pub(crate) fn validate_version_scoped_target(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(attached_data) = machine.attached_data.as_ref() else {
        return;
    };
    // Top-level data names are single identifiers, so a `::` in the attached
    // data name only appears for version-scoped machines.
    let Some((data_name, version_name)) = attached_data.as_str().split_once("::") else {
        return;
    };

    if program
        .data_definitions()
        .iter()
        .any(|definition| definition.name.as_str() == attached_data.as_str())
    {
        return;
    }

    if program
        .data_definitions()
        .iter()
        .any(|definition| definition.name.as_str() == data_name)
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` targets version `{version_name}` of data `{data_name}`, but `{data_name}` does not declare a `version {version_name}` block",
            machine.name
        )));
    } else {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` targets version `{version_name}` of unknown data `{data_name}`",
            machine.name
        )));
    }
}

pub(crate) fn validate_contained_types(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contained_object in program.machine_contained_objects(machine) {
        if !symbols.is_callable_receiver_type(&contained_object.type_name) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` contains `{}` with unknown type `{}`",
                machine.name, contained_object.name, contained_object.type_name
            )));
        }
    }
}

pub(crate) fn validate_owned_data(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for owned_data in program.machine_owned_data(machine) {
        validate_type_reference_handle_with_type_parameters(
            program,
            owned_data.type_reference,
            symbols,
            diagnostics,
            TypeReferenceOwner::MachineOwnedData {
                machine: machine.name.as_str(),
                data: owned_data.name.as_str(),
                generic_depth: 0,
            },
            program.machine_type_parameters(machine),
        );

        if owned_data.initial_value.is_valid() {
            validate_initial_value_handle(
                program,
                owned_data.type_reference,
                owned_data.initial_value,
                diagnostics,
                InitialValueOwner::MachineOwnedData {
                    machine: machine.name.as_str(),
                    data: owned_data.name.as_str(),
                },
            );
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum InitialValueOwner<'program> {
    MachineOwnedData {
        machine: &'program str,
        data: &'program str,
    },
}

impl fmt::Display for InitialValueOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MachineOwnedData { machine, data } => {
                write!(formatter, "machine `{machine}` owned data `{data}`")
            }
        }
    }
}

fn validate_initial_value_handle(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    initial_value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
    owner: InitialValueOwner<'_>,
) {
    if !argument_matches_type_reference_handle(program, initial_value, type_reference) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} initializer expects `{}`, got `{}`",
            program.display_type_reference_with_constraints(type_reference),
            expression_type_name_handle(program, initial_value)
        )));
    }
}
