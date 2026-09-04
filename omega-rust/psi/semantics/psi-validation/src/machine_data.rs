use crate::expression_types::{
    argument_matches_type_reference_handle, expression_type_name_handle,
};
use crate::symbols::TopLevelSymbols;
use crate::type_references::{
    TypeReferenceOwner, validate_type_reference_handle_with_type_parameters,
};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::types::TypeReferenceHandle;
use std::fmt;

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
            &machine.lifetime_parameters,
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
