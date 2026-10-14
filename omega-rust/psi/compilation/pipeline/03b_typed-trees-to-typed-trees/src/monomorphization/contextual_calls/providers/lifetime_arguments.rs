//! Retain erased regions as exact declaration-to-caller mappings, never
//! runtime tuple operands. Even an unused provider lifetime consumes one
//! authored argument; no borrow occurrence may guess away that telescope.

use super::super::super::{Diagnostic, StaticMachineArgument, SymbolHandle, TypedTrees};
use typed_trees::static_applications::{
    StaticMachineApplicationHandle, StaticMachineLifetimeBinding,
};

pub(super) fn bind(
    program: &TypedTrees,
    selected: SymbolHandle,
    selection: &StaticMachineArgument,
    invocation: StaticMachineApplicationHandle,
) -> Result<Vec<StaticMachineLifetimeBinding>, Vec<Diagnostic>> {
    let Some(explicit) = selection
        .application
        .as_ref()
        .filter(|application| !application.lifetime_arguments.is_empty())
    else {
        return Ok(Vec::new());
    };
    let machine = program
        .machines()
        .iter()
        .find(|machine| program.machine_or_state_symbol(machine, selected))
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "nested lifetime application lost its selected declaration",
            )]
        })?;
    if machine.lifetime_parameters.len() != explicit.lifetime_arguments.len() {
        return Err(vec![Diagnostic::error(
            "nested machine lifetime arguments do not match its complete declared telescope",
        )]);
    }
    let owner = program
        .expression_table
        .static_machine_application(invocation)
        .lexical_owner;
    let owner = program.machine_by_symbol(owner).ok_or_else(|| {
        vec![Diagnostic::error(
            "nested machine lifetime arguments have no caller declaration custody",
        )]
    })?;
    if explicit.lifetime_arguments.iter().any(|argument| {
        !owner
            .lifetime_parameters
            .iter()
            .any(|parameter| parameter == argument)
    }) {
        return Err(vec![Diagnostic::error(
            "nested machine lifetime argument is not declared in its lexical owner",
        )]);
    }
    Ok(machine
        .lifetime_parameters
        .iter()
        .zip(&explicit.lifetime_arguments)
        .map(|(parameter, argument)| StaticMachineLifetimeBinding {
            parameter: parameter.clone(),
            argument: argument.clone(),
        })
        .collect())
}
