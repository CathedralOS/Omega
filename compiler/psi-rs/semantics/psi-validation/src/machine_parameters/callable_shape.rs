use super::contract_facts::validate_contract_facts;
use super::type_refinement::{BinderBinding, TypeBinding, required_type_matches};
use super::{
    MachineBlockingRow, MachineSuspensionRow, machine_and_state, machine_parameter_contract,
    machine_parameter_signature,
};
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{TypeParameter, TypeParameterKind};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::{SignatureContract, StateParameter};
use psi_typed_trees::state::State;
use psi_typed_trees::types::TypeReferenceHandle;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_selected_callable_shape(
    program: &TypedTrees,
    suspensions: &[MachineSuspensionRow],
    blockings: &[MachineBlockingRow],
    service_reaches: &psi_effects::ServiceReachInferencePlan,
    invocations: &psi_effects::InvocationInferencePlan,
    generic_call: &str,
    parameter: &TypeParameter,
    requirement: &psi_typed_trees::signature::StateSignature,
    selected_symbol: SymbolHandle,
    selected_name: &str,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some((actual_machine, actual_state)) = machine_and_state(program, selected_symbol) {
        validate_callable_shape(
            program,
            suspensions,
            blockings,
            service_reaches,
            invocations,
            generic_call,
            parameter,
            requirement,
            actual_machine,
            actual_state,
            generic_types,
            bindings,
            diagnostics,
        );
        return;
    }

    if let Some((actual_parameter, actual_signature)) =
        machine_parameter_contract(program, selected_symbol)
    {
        let label = format!(
            "machine parameter `{}` forwarded into `{generic_call}`",
            actual_parameter.name
        );
        validate_callable_parts(
            program,
            &label,
            parameter,
            requirement,
            program.state_signature_type_parameters(actual_signature),
            program.state_signature_parameters(actual_signature),
            actual_signature.return_type,
            program
                .service_reach_rows
                .services(actual_signature.service_reach_row),
            &psi_effects::declared_signature_invocations(program, actual_signature),
            actual_signature.suspends,
            actual_signature.blocks,
            actual_signature
                .termination_guarantee
                .promises_termination(),
            program.state_signature_contracts(actual_signature),
            generic_types,
            bindings,
            &mut Vec::new(),
            diagnostics,
        );
        return;
    }

    diagnostics.push(Diagnostic::error(format!(
        "static machine argument `{selected_name}` does not name a callable machine entry or an in-scope machine parameter"
    )));
}

#[allow(clippy::too_many_arguments)]
fn validate_callable_shape(
    program: &TypedTrees,
    suspensions: &[MachineSuspensionRow],
    blockings: &[MachineBlockingRow],
    service_reaches: &psi_effects::ServiceReachInferencePlan,
    invocations: &psi_effects::InvocationInferencePlan,
    generic_call: &str,
    parameter: &TypeParameter,
    requirement: &psi_typed_trees::signature::StateSignature,
    actual_machine: &Machine,
    actual_state: &State,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let label = format!(
        "machine argument `{}` for `{generic_call}`",
        actual_machine.name
    );
    let inferred_suspension = suspensions
        .iter()
        .find(|row| row.symbol == actual_machine.symbol);
    let inferred_blocking = blockings
        .iter()
        .find(|row| row.symbol == actual_machine.symbol);
    let actual_services = service_reaches
        .for_machine(actual_machine.symbol)
        .map(|summary| service_reaches.services(summary.effective))
        .unwrap_or_else(|| {
            program
                .service_reach_rows
                .services(actual_machine.service_reach_row)
        });
    let actual_may_suspend = inferred_suspension
        .map(|row| row.transitive_may_suspend)
        .unwrap_or(actual_machine.suspends);
    let actual_may_block = inferred_blocking
        .map(|row| row.transitive_may_block)
        .unwrap_or(actual_machine.blocks);
    let actual_invocations = invocations
        .for_machine(actual_machine.symbol)
        .map(|summary| summary.effective.as_slice())
        .unwrap_or_default();
    validate_callable_parts(
        program,
        &label,
        parameter,
        requirement,
        program.machine_type_parameters(actual_machine),
        program.state_parameters(actual_state),
        actual_state.return_type,
        actual_services,
        actual_invocations,
        actual_may_suspend,
        actual_may_block,
        matches!(
            &actual_machine.termination_plan.interface,
            psi_language_semantics::TerminationInterface::Published(
                psi_language_semantics::TerminationGuarantee::Terminates { .. }
            )
        ),
        program.machine_contracts(actual_machine),
        generic_types,
        bindings,
        &mut Vec::new(),
        diagnostics,
    );
}

#[allow(clippy::too_many_arguments)]
fn validate_callable_parts(
    program: &TypedTrees,
    label: &str,
    parameter: &TypeParameter,
    requirement: &psi_typed_trees::signature::StateSignature,
    actual_type_parameters: &[TypeParameter],
    actual_parameters: &[StateParameter],
    actual_return_type: TypeReferenceHandle,
    actual_services: &[psi_language_semantics::ServiceReachId],
    actual_invocations: &[psi_effects::InvocationTarget],
    actual_may_suspend: bool,
    actual_may_block: bool,
    actual_terminates: bool,
    actual_contracts: &[SignatureContract],
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    binder_bindings: &mut Vec<BinderBinding>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_callable_type_parameters(
        program,
        label,
        parameter,
        requirement,
        actual_type_parameters,
        generic_types,
        bindings,
        binder_bindings,
        diagnostics,
    );

    let required_parameters = program.state_signature_parameters(requirement);
    if required_parameters.len() != actual_parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: expected {} parameter(s), got {}",
            parameter.name,
            required_parameters.len(),
            actual_parameters.len()
        )));
        return;
    }

    for (index, (required, actual)) in required_parameters
        .iter()
        .zip(actual_parameters)
        .enumerate()
    {
        if required.is_self != actual.is_self
            || required.is_mutable != actual.is_mutable
            || required.is_const != actual.is_const
        {
            diagnostics.push(Diagnostic::error(format!(
                "{label} does not refine `{}`: parameter {} has a different calling mode",
                parameter.name, index
            )));
            continue;
        }
        if !required_type_matches(
            program,
            actual.type_reference,
            required.type_reference,
            generic_types,
            bindings,
            binder_bindings,
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "{label} does not refine `{}`: parameter {} expects `{}`, got `{}`",
                parameter.name,
                index,
                program.display_type_reference(required.type_reference),
                program.display_type_reference(actual.type_reference)
            )));
        }
    }

    if !required_type_matches(
        program,
        actual_return_type,
        requirement.return_type,
        generic_types,
        bindings,
        binder_bindings,
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: expected return `{}`, got `{}`",
            parameter.name,
            program.display_type_reference(requirement.return_type),
            program.display_type_reference(actual_return_type)
        )));
    }

    let allowed_services = program
        .service_reach_rows
        .services(requirement.service_reach_row);
    for service in actual_services {
        if allowed_services.contains(service) {
            continue;
        }
        let name = program
            .service_reaches
            .definition(*service)
            .map(|definition| definition.name.as_str())
            .unwrap_or("<unknown boundary service>");
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: service reach `{name}` exceeds its authored ceiling",
            parameter.name
        )));
    }

    let allowed_invocations = psi_effects::declared_signature_invocations(program, requirement);
    for invocation in actual_invocations {
        if allowed_invocations.contains(invocation) {
            continue;
        }
        let name = match invocation {
            psi_effects::InvocationTarget::Parameter(index) => required_parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .nth(*index as usize)
                .map(|parameter| parameter.name.as_str())
                .unwrap_or("<unknown binding>"),
            psi_effects::InvocationTarget::Service(symbol) => program
                .traits()
                .iter()
                .find(|definition| definition.symbol == *symbol)
                .map(|definition| definition.name.as_str())
                .unwrap_or("<unknown boundary service>"),
        };
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: synchronous invocation `{name}` exceeds its authored `invokes` ceiling",
            parameter.name
        )));
    }

    if actual_may_suspend && !requirement.suspends {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: it may suspend, but the requirement omits `suspends;`",
            parameter.name
        )));
    }
    if actual_may_block && !requirement.blocks {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: it may block, but the requirement omits `blocks;`",
            parameter.name
        )));
    }

    if requirement.termination_guarantee.promises_termination() && !actual_terminates {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: the requirement guarantees termination",
            parameter.name
        )));
    }

    validate_contract_facts(
        program,
        &label,
        parameter,
        requirement,
        actual_contracts,
        required_parameters,
        actual_parameters,
        diagnostics,
    );
}

/// Check an authored callable generic parameter list against a trait
/// requirement. Trait conformance needs the same recursive machine-contract
/// judgment as a concrete static selection; a kind-only comparison would let
/// a provider silently change `machine Target(...)` beneath an otherwise
/// matching requirement.
pub(crate) fn validate_trait_callable_parameter_refinement(
    program: &TypedTrees,
    label: &str,
    requirement_parameters: &[TypeParameter],
    actual_parameters: &[TypeParameter],
    generic_types: &[&TypeParameter],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut bindings = Vec::new();
    let mut binder_bindings = requirement_parameters
        .iter()
        .zip(actual_parameters)
        .map(|(required, provider)| BinderBinding {
            required: required.symbol,
            actual: provider.symbol,
        })
        .collect::<Vec<_>>();

    for (required, actual) in requirement_parameters.iter().zip(actual_parameters) {
        let (
            TypeParameterKind::Machine {
                contract: required_contract,
            },
            TypeParameterKind::Machine {
                contract: actual_contract,
            },
        ) = (&required.kind, &actual.kind)
        else {
            continue;
        };
        let required_contract = machine_parameter_signature(program, required_contract);
        let actual_contract = machine_parameter_signature(program, actual_contract);
        let nested_label = format!("machine parameter `{}` of {label}", actual.name);
        validate_callable_parts(
            program,
            &nested_label,
            required,
            required_contract,
            program.state_signature_type_parameters(actual_contract),
            program.state_signature_parameters(actual_contract),
            actual_contract.return_type,
            program
                .service_reach_rows
                .services(actual_contract.service_reach_row),
            &psi_effects::declared_signature_invocations(program, actual_contract),
            actual_contract.suspends,
            actual_contract.blocks,
            actual_contract.termination_guarantee.promises_termination(),
            program.state_signature_contracts(actual_contract),
            generic_types,
            &mut bindings,
            &mut binder_bindings,
            diagnostics,
        );

        // Provider conformance publishes one exact higher-order slot. The
        // ordinary selection refinement above rejects a wider provider
        // contract; these reverse checks reject a narrower one.
        if required_contract.suspends && !actual_contract.suspends {
            diagnostics.push(Diagnostic::error(format!(
                "{nested_label} narrows `{}` by omitting `suspends;`",
                required.name
            )));
        }
        if required_contract.blocks && !actual_contract.blocks {
            diagnostics.push(Diagnostic::error(format!(
                "{nested_label} narrows `{}` by omitting `blocks;`",
                required.name
            )));
        }
        let actual_services = program
            .service_reach_rows
            .services(actual_contract.service_reach_row);
        for service in program
            .service_reach_rows
            .services(required_contract.service_reach_row)
        {
            if !actual_services.contains(service) {
                diagnostics.push(Diagnostic::error(format!(
                    "{nested_label} narrows `{}` by omitting an admitted service reach",
                    required.name
                )));
            }
        }
        if required_contract.termination_guarantee != actual_contract.termination_guarantee {
            diagnostics.push(Diagnostic::error(format!(
                "{nested_label} changes `{}` termination requirements",
                required.name
            )));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_callable_type_parameters(
    program: &TypedTrees,
    label: &str,
    parameter: &TypeParameter,
    requirement: &psi_typed_trees::signature::StateSignature,
    actual_parameters: &[TypeParameter],
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    binder_bindings: &mut Vec<BinderBinding>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let required_parameters = program.state_signature_type_parameters(requirement);
    if required_parameters.len() != actual_parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: its callable signature expects {} generic parameter(s), got {}",
            parameter.name,
            required_parameters.len(),
            actual_parameters.len()
        )));
        return;
    }

    // Establish every positional binder mapping before descending into any
    // one nested requirement. Sibling contracts may mention one another, and
    // their authored names are intentionally irrelevant to refinement.
    for (required, actual) in required_parameters.iter().zip(actual_parameters) {
        binder_bindings.push(BinderBinding {
            required: required.symbol,
            actual: actual.symbol,
        });
    }

    for (index, (required, actual)) in required_parameters
        .iter()
        .zip(actual_parameters)
        .enumerate()
    {
        match (&required.kind, &actual.kind) {
            (TypeParameterKind::Type, TypeParameterKind::Type) => {
                if (actual.bounds.multiplicity
                    == psi_language_semantics::Multiplicity::Unrestricted
                    && required.bounds.multiplicity
                        != psi_language_semantics::Multiplicity::Unrestricted)
                    || actual.bounds.carry.is_some() && required.bounds.carry != actual.bounds.carry
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "{label} does not refine `{}`: generic parameter {} demands stronger type properties",
                        parameter.name, index
                    )));
                }
            }
            (
                TypeParameterKind::Const {
                    type_reference: required_type,
                },
                TypeParameterKind::Const {
                    type_reference: actual_type,
                },
            ) => {
                if !required_type_matches(
                    program,
                    *actual_type,
                    *required_type,
                    generic_types,
                    bindings,
                    binder_bindings,
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{label} does not refine `{}`: const generic parameter {} has a different type",
                        parameter.name, index
                    )));
                }
            }
            (
                TypeParameterKind::Machine {
                    contract: required_contract,
                },
                TypeParameterKind::Machine {
                    contract: actual_contract,
                },
            ) => {
                let required_contract = machine_parameter_signature(program, required_contract);
                let actual_contract = machine_parameter_signature(program, actual_contract);
                let nested_label = format!("nested machine parameter `{}` of {label}", actual.name);
                validate_callable_parts(
                    program,
                    &nested_label,
                    required,
                    required_contract,
                    program.state_signature_type_parameters(actual_contract),
                    program.state_signature_parameters(actual_contract),
                    actual_contract.return_type,
                    program
                        .service_reach_rows
                        .services(actual_contract.service_reach_row),
                    &psi_effects::declared_signature_invocations(program, actual_contract),
                    actual_contract.suspends,
                    actual_contract.blocks,
                    actual_contract.termination_guarantee.promises_termination(),
                    program.state_signature_contracts(actual_contract),
                    generic_types,
                    bindings,
                    binder_bindings,
                    diagnostics,
                );
            }
            _ => diagnostics.push(Diagnostic::error(format!(
                "{label} does not refine `{}`: generic parameter {} has a different kind",
                parameter.name, index
            ))),
        }
    }
}
