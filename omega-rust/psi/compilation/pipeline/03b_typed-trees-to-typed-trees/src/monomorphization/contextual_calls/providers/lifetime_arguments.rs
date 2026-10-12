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
    expected: &typed_trees::signature::StateSignature,
    applied_expected_types: &[typed_trees::types::TypeReferenceHandle],
) -> Result<Vec<StaticMachineLifetimeBinding>, Vec<Diagnostic>> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == selected)
        })
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "nested lifetime application lost its selected declaration",
            )]
        })?;
    let explicit = selection
        .application
        .as_ref()
        .filter(|application| !application.lifetime_arguments.is_empty());
    if let Some(explicit) = explicit {
        if machine.lifetime_parameters.len() != explicit.lifetime_arguments.len() {
            return Err(vec![Diagnostic::error(
                "nested machine lifetime arguments do not match its complete declared telescope",
            )]);
        }
    }
    let application_owner = program
        .expression_table
        .static_machine_application(invocation)
        .type_identity_owner(program)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "nested machine lifetime arguments lost their original caller capture",
            )]
        })?;
    if explicit.is_none() && !expected.lifetime_parameters.is_empty() {
        // A universally quantified expected telescope asks the selected source
        // declaration to refine fresh invocation binders. It supplies no actual
        // anonymous operand and must not instantiate those binders from caller
        // storage. The existing callable region judgment owns that universal map.
        return Ok(Vec::new());
    }
    if explicit.is_none() {
        let state = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == selected)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "nested lifetime application lost its selected state",
                )]
            })?;
        let mut original_bindings = Vec::new();
        let mut original_conflict = false;
        for (actual, required) in program
            .state_parameters(state)
            .iter()
            .zip(program.state_signature_parameters(expected))
        {
            crate::conformance::conformance_application_lifetimes::collect_lifetime_arguments(
                program,
                actual.type_reference,
                required.type_reference,
                &machine.lifetime_parameters,
                &mut original_bindings,
                &mut original_conflict,
            );
        }
        let mut bindings = Vec::new();
        let mut conflict = false;
        for (actual, required) in program
            .state_parameters(state)
            .iter()
            .zip(applied_expected_types)
        {
            crate::conformance::conformance_application_lifetimes::collect_lifetime_arguments(
                program,
                actual.type_reference,
                *required,
                &machine.lifetime_parameters,
                &mut bindings,
                &mut conflict,
            );
        }
        // Named operands read from an expected contract remain in that contract's
        // original declaration namespace. Equal spelling in the invocation owner
        // cannot donate a region. Anonymous reference operands have no named scope.
        for binding in &bindings {
            if let Some(argument) = binding.argument.named() {
                let expected_owner = program.lifetime_declaration_owner(expected.symbol);
                let expected_region = program
                    .original_declaration_lifetime_parameter(expected_owner, argument.as_str());
                let actual_region = match application_owner {
                    typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(owner) => {
                        program.original_declaration_lifetime_parameter(
                            program.lifetime_declaration_owner(owner),
                            argument.as_str(),
                        )
                    }
                    typed_trees::type_identity::AppliedTypeIdentityOwner::Captured(capture) => {
                        capture.owner().original_declaration_lifetime_parameter(
                            capture.symbol(),
                            argument.as_str(),
                        )
                    }
                };
                let original_argument = original_bindings
                    .iter()
                    .find(|original| original.parameter == binding.parameter)
                    .and_then(|original| original.argument.named());
                let invocation_handle = invocation;
                let invocation = program
                    .expression_table
                    .static_machine_application(invocation_handle);
                let projected_region = original_argument.filter(|_| !original_conflict).and_then(
                    |original_argument| {
                        let original_region = program.original_declaration_lifetime_parameter(
                            expected_owner,
                            original_argument.as_str(),
                        )?;
                        program
                            .expression_table
                            .static_machine_application_lifetimes(invocation_handle)
                            .iter()
                            .find(|caller| {
                                program.original_declaration_lifetime_parameter(
                                    invocation.binding_declaration,
                                    caller.parameter.as_str(),
                                ) == Some(original_region)
                                    && caller.argument.named() == Some(argument)
                            })
                            .and(actual_region)
                    },
                );
                if actual_region.is_none()
                    || (expected_region != actual_region && projected_region != actual_region)
                {
                    return Err(vec![Diagnostic::error(
                        "inferred nested machine lifetime has no original caller region correspondence",
                    )]);
                }
            }
        }
        if conflict
            || machine.lifetime_parameters.iter().any(|parameter| {
                !bindings
                    .iter()
                    .any(|binding| binding.parameter == *parameter)
            })
        {
            return Err(vec![Diagnostic::error(
                "nested machine lifetime application has no complete determined correspondence",
            )]);
        }
        return Ok(machine
            .lifetime_parameters
            .iter()
            .filter_map(|parameter| {
                bindings
                    .iter()
                    .find(|binding| binding.parameter == *parameter)
                    .cloned()
            })
            .collect());
    }
    let Some(explicit) = explicit else {
        return Err(vec![Diagnostic::error(
            "nested lifetime application lost its supplied tuple",
        )]);
    };
    if explicit
        .lifetime_arguments
        .iter()
        .any(|argument| match application_owner {
            typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(owner) => program
                .original_declaration_lifetime_parameter(
                    program.lifetime_declaration_owner(owner),
                    argument.as_str(),
                )
                .is_none(),
            typed_trees::type_identity::AppliedTypeIdentityOwner::Captured(capture) => capture
                .owner()
                .original_declaration_lifetime_parameter(capture.symbol(), argument.as_str())
                .is_none(),
        })
    {
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
            argument: argument.clone().into(),
        })
        .collect())
}
