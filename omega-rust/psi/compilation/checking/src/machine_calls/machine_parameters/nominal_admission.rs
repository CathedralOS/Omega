use super::type_refinement::{TypeBinding, required_type_matches_exact};
use super::{MachineStateIndex, machine_and_state, machine_parameter_contract_definition};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{MachineParameterContract, TypeParameter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AdmittedNominalSelection {
    pub(super) selected_machine: SymbolHandle,
    pub(super) selected_entry: SymbolHandle,
    pub(super) satisfaction_trait: SymbolHandle,
    pub(super) satisfaction_requirement: SymbolHandle,
    pub(super) canonical_requirement_overload: String,
}

pub(super) fn validate_nominal_machine_selection(
    program: &TypedTrees,
    index: &MachineStateIndex,
    generic_owner: &str,
    parameter: &TypeParameter,
    required_contract: &MachineParameterContract,
    selected_symbol: SymbolHandle,
    selected_name: &str,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Option<AdmittedNominalSelection>, ()> {
    let MachineParameterContract::Nominal {
        trait_definition: required_trait,
        requirement: required_requirement,
        trait_arguments: required_arguments,
        ..
    } = required_contract
    else {
        return Ok(None);
    };
    // The selection's trait application must equal the binder's under this
    // call's type bindings. Each candidate unifies on scratch bindings; only
    // the admitted one commits, so a rejected candidate binds nothing.
    let arguments_match = |actual: &[typed_trees::types::TypeReferenceHandle],
                           bindings: &mut Vec<TypeBinding>| {
        actual.len() == required_arguments.len()
            && actual
                .iter()
                .zip(required_arguments)
                .all(|(actual, required)| {
                    required_type_matches_exact(
                        program,
                        *actual,
                        *required,
                        generic_types,
                        bindings,
                    )
                })
    };

    // Forwarding another static binder is sound only when that binder carries
    // the same exact nominal authority. Structural coincidence does not
    // establish a named satisfaction row.
    if let Some((selected_parameter, selected_contract)) =
        machine_parameter_contract_definition(program, selected_symbol)
    {
        if let MachineParameterContract::Nominal {
            trait_definition,
            requirement,
            trait_arguments,
            ..
        } = selected_contract
            && trait_definition == required_trait
            && requirement == required_requirement
        {
            let mut scratch = bindings.clone();
            if arguments_match(trait_arguments, &mut scratch) {
                *bindings = scratch;
                return Ok(None);
            }
        }
        diagnostics.push(Diagnostic::error(format!(
            "machine parameter `{}` forwarded into `{generic_owner}` does not carry the exact nominal requirement of `{}`; matching callable structure establishes no satisfaction row",
            selected_parameter.name, parameter.name
        )));
        return Err(());
    }

    let Some((selected_machine, selected_state)) =
        machine_and_state(program, index, selected_symbol)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "static machine argument `{selected_name}` for nominal parameter `{}` does not resolve to a concrete machine entry",
            parameter.name
        )));
        return Err(());
    };
    let Some(entry_state) = program.machine_states(selected_machine).first() else {
        diagnostics.push(Diagnostic::error(format!(
            "static machine argument `{selected_name}` for nominal parameter `{}` has no callable entry",
            parameter.name
        )));
        return Err(());
    };
    if entry_state.symbol != selected_state.symbol {
        diagnostics.push(Diagnostic::error(format!(
            "static machine argument `{selected_name}` selects a non-entry state; nominal parameter `{}` requires the machine entry that owns its satisfaction row",
            parameter.name
        )));
        return Err(());
    }

    let view = program
        .machine_parameter_contract_view(required_contract)
        .expect("typed nominal contract must retain a valid exact requirement identity");
    let typed_trees::data::MachineParameterContractView::Nominal {
        trait_definition,
        requirement,
    } = view
    else {
        unreachable!("nominal contract projected as structural")
    };
    let mut admitted_bindings = None;
    let mut matching_rows = 0usize;
    for conformance in program.machine_trait_conformances(selected_machine) {
        if conformance.symbol != *required_trait
            || !conformance
                .requirement
                .as_ref()
                .is_some_and(|name| name.as_str() == requirement.name.as_str())
        {
            continue;
        }
        let mut scratch = bindings.clone();
        if arguments_match(
            program
                .type_reference_table
                .type_reference_handles(conformance.arguments),
            &mut scratch,
        ) {
            matching_rows += 1;
            admitted_bindings = Some(scratch);
        }
    }
    if matching_rows == 1
        && let Some(admitted) = admitted_bindings
    {
        *bindings = admitted;
    }
    if matching_rows != 1 {
        diagnostics.push(Diagnostic::error(format!(
            "static machine argument `{selected_name}` for nominal parameter `{}` retains {matching_rows} authored satisfaction row(s) for exact requirement `{}::{}`; exactly one is required and structural coincidence establishes none",
            parameter.name, trait_definition.name, requirement.name
        )));
        return Err(());
    }

    Ok(Some(AdmittedNominalSelection {
        selected_machine: selected_machine.symbol,
        selected_entry: entry_state.symbol,
        satisfaction_trait: *required_trait,
        satisfaction_requirement: *required_requirement,
        canonical_requirement_overload: program
            .trait_requirement_callable_identity_text(trait_definition, requirement),
    }))
}
