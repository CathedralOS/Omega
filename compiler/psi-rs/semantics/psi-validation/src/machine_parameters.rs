//! MP2b: admission of compile-time machine-symbol arguments.
//!
//! Static selections are checked at the generic call edge. The selected
//! machine must be concrete, match the authored callable shape, stay within
//! the required service and operational ceilings, and conservatively refine
//! conjunctive requires/ensures facts. This pass never invents a callback
//! contract.

use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{TypeParameter, TypeParameterKind};
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{ExpressionNode, StaticMachineArgument};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use psi_typed_trees::state::State;
use psi_typed_trees::statement::StatementNode;
use psi_typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_static_machine_arguments(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let operational = psi_effects::infer_operational_may(program);
    let service_reaches = psi_effects::infer_service_reaches(program, &operational);
    let invocations = psi_effects::infer_synchronous_invocations(program);
    for (_, expression) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            validate_call_selection(
                program,
                &operational,
                &service_reaches,
                &invocations,
                call.target_symbol,
                call.target.as_str(),
                &call.machine_arguments,
                diagnostics,
            );
        }
    }

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Call(call) = statement {
                    validate_call_selection(
                        program,
                        &operational,
                        &service_reaches,
                        &invocations,
                        call.target_symbol,
                        call.target.as_str(),
                        &call.machine_arguments,
                        diagnostics,
                    );
                }
            }
        }
    }
}

/// Run MP2b admission as a standalone pre-specialization gate. MP4 consumes
/// the static argument syntax, so its refinement proof must happen first.
pub fn validate_static_machine_selections(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    validate_static_machine_arguments(program, &mut diagnostics);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_call_selection(
    program: &TypedTrees,
    operational: &psi_effects::OperationalPlan,
    service_reaches: &psi_effects::ServiceReachInferencePlan,
    invocations: &psi_effects::InvocationInferencePlan,
    target_symbol: SymbolHandle,
    target_name: &str,
    arguments: &[StaticMachineArgument],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (requirements, generic_types): (Vec<_>, Vec<_>) = if let Some((callee, _)) =
        machine_and_state(program, target_symbol)
    {
        (
            program
                .machine_type_parameters(callee)
                .iter()
                .filter_map(|parameter| match &parameter.kind {
                    TypeParameterKind::Machine { contract } => Some((parameter, contract)),
                    _ => None,
                })
                .collect(),
            program
                .machine_type_parameters(callee)
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                .collect(),
        )
    } else if let Some((declaring_machine, signature)) =
        program.machine_parameter_signature(target_symbol)
    {
        (
            program
                .state_signature_type_parameters(signature)
                .iter()
                .filter_map(|parameter| match &parameter.kind {
                    TypeParameterKind::Machine { contract } => Some((parameter, contract)),
                    _ => None,
                })
                .collect(),
            program
                .machine_type_parameters(declaring_machine)
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                .collect(),
        )
    } else if let Some(signature) = program
        .traits()
        .iter()
        .flat_map(|definition| program.trait_machine_signatures(definition))
        .find(|signature| signature.symbol == target_symbol)
    {
        (
            program
                .state_signature_type_parameters(signature)
                .iter()
                .filter_map(|parameter| match &parameter.kind {
                    TypeParameterKind::Machine { contract } => Some((parameter, contract)),
                    _ => None,
                })
                .collect(),
            program
                .state_signature_type_parameters(signature)
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                .collect(),
        )
    } else {
        if !arguments.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                    "call `{target_name}` supplies static machine arguments, but its generic callee did not resolve"
                )));
        }
        return;
    };

    if arguments.len() != requirements.len() {
        diagnostics.push(Diagnostic::error(format!(
            "generic call `{target_name}` requires {} static machine argument(s), got {}",
            requirements.len(),
            arguments.len()
        )));
        return;
    }

    let mut bindings = Vec::new();

    for ((parameter, requirement), selected) in requirements.into_iter().zip(arguments) {
        let rendered = selected
            .path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        // A recursive generic body may forward its own authored machine
        // parameter (`map<F>(tail)`). This is not a concrete selection yet,
        // but it is already governed by exactly this requirement; the
        // eventual external selection is validated at the concrete call edge.
        if selected.symbol == parameter.symbol {
            continue;
        }
        if !selected.symbol.is_valid() {
            diagnostics.push(Diagnostic::error(format!(
                "static machine argument `{rendered}` for `{}` does not resolve to a concrete machine",
                parameter.name
            )));
            continue;
        }
        validate_selected_callable_shape(
            program,
            operational,
            service_reaches,
            invocations,
            target_name,
            parameter,
            requirement,
            selected.symbol,
            &rendered,
            &generic_types,
            &mut bindings,
            diagnostics,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_selected_callable_shape(
    program: &TypedTrees,
    operational: &psi_effects::OperationalPlan,
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
            operational,
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
            actual_signature.terminates_guarantee,
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
    operational: &psi_effects::OperationalPlan,
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
    let inferred = operational
        .machines()
        .iter()
        .find(|summary| summary.symbol == actual_machine.symbol);
    let actual_services = service_reaches
        .for_machine(actual_machine.symbol)
        .map(|summary| service_reaches.services(summary.effective))
        .unwrap_or_else(|| {
            program
                .service_reach_rows
                .services(actual_machine.service_reach_row)
        });
    let actual_may_suspend = inferred
        .map(|summary| summary.transitive_may_suspend)
        .unwrap_or(actual_machine.suspends);
    let actual_may_block = inferred
        .map(|summary| summary.transitive_may_block)
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

    if requirement.terminates_guarantee && !actual_terminates {
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
            actual_contract.terminates_guarantee,
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
        if required_contract.terminates_guarantee != actual_contract.terminates_guarantee {
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
                if (actual.bounds.copy && !required.bounds.copy)
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
                    actual_contract.terminates_guarantee,
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

/// N7 data-family admission uses the same refinement judgment as a generic
/// call, but its selected symbol is carried in a generic type argument rather
/// than an expression call node. Keeping this entry point here prevents proof
/// data from growing a weaker, shape-only callback check.
pub(crate) fn validate_data_machine_selection(
    program: &TypedTrees,
    family_name: &str,
    parameter: &TypeParameter,
    requirement: &psi_typed_trees::signature::StateSignature,
    selected_symbol: SymbolHandle,
    selected_name: &str,
    generic_types: &[&TypeParameter],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let operational = psi_effects::infer_operational_may(program);
    let service_reaches = psi_effects::infer_service_reaches(program, &operational);
    let invocations = psi_effects::infer_synchronous_invocations(program);
    validate_selected_callable_shape(
        program,
        &operational,
        &service_reaches,
        &invocations,
        family_name,
        parameter,
        requirement,
        selected_symbol,
        selected_name,
        generic_types,
        &mut Vec::new(),
        diagnostics,
    );
}

#[derive(Clone, Copy)]
struct TypeBinding {
    symbol: SymbolHandle,
    actual: TypeReferenceHandle,
}

#[derive(Clone, Copy)]
struct BinderBinding {
    required: SymbolHandle,
    actual: SymbolHandle,
}

fn required_type_matches(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    binder_bindings: &[BinderBinding],
) -> bool {
    if !actual.is_valid() || !required.is_valid() {
        return actual.is_valid() == required.is_valid();
    }
    if let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(required)
    {
        if let Some(binding) = binder_bindings
            .iter()
            .find(|binding| binding.required == *symbol)
        {
            return matches!(
                program.type_reference_table.type_reference(actual),
                TypeReferenceNode::Named { symbol, .. } if *symbol == binding.actual
            );
        }
        if let Some(parameter) = generic_types.iter().find(|parameter| {
            (parameter.symbol.is_valid() && parameter.symbol == *symbol)
                || parameter.name.as_str() == name.as_str()
        }) {
            if let Some(binding) = bindings
                .iter()
                .find(|binding| binding.symbol == parameter.symbol)
            {
                return required_type_matches(
                    program,
                    actual,
                    binding.actual,
                    &[],
                    &mut Vec::new(),
                    binder_bindings,
                );
            }
            bindings.push(TypeBinding {
                symbol: parameter.symbol,
                actual,
            });
            return true;
        }
    }

    match (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(required),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: actual_inner,
                is_mutable: actual_mutable,
                ..
            },
            TypeReferenceNode::Reference {
                referee: required_inner,
                is_mutable: required_mutable,
                ..
            },
        ) => {
            actual_mutable == required_mutable
                && required_type_matches(
                    program,
                    *actual_inner,
                    *required_inner,
                    generic_types,
                    bindings,
                    binder_bindings,
                )
        }
        (
            TypeReferenceNode::Constrained {
                base_type: actual_base,
                ..
            },
            TypeReferenceNode::Constrained {
                base_type: required_base,
                ..
            },
        ) => required_type_matches(
            program,
            *actual_base,
            *required_base,
            generic_types,
            bindings,
            binder_bindings,
        ),
        (
            TypeReferenceNode::FixedArray {
                element_type: actual_element,
                length: actual_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: required_element,
                length: required_length,
            },
        ) => {
            fixed_array_lengths_match(actual_length, required_length, binder_bindings)
                && required_type_matches(
                    program,
                    *actual_element,
                    *required_element,
                    generic_types,
                    bindings,
                    binder_bindings,
                )
        }
        (
            TypeReferenceNode::Slice {
                element_type: actual_element,
            },
            TypeReferenceNode::Slice {
                element_type: required_element,
            },
        ) => required_type_matches(
            program,
            *actual_element,
            *required_element,
            generic_types,
            bindings,
            binder_bindings,
        ),
        (
            TypeReferenceNode::Generic {
                base_symbol: actual_base,
                base_name: actual_name,
                arguments: actual_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: required_base,
                base_name: required_name,
                arguments: required_arguments,
                ..
            },
        ) => {
            let same_base = if actual_base.is_valid() && required_base.is_valid() {
                actual_base == required_base
            } else {
                actual_name == required_name
            };
            let actual_arguments = program
                .type_reference_table
                .type_reference_handles(*actual_arguments);
            let required_arguments = program
                .type_reference_table
                .type_reference_handles(*required_arguments);
            same_base
                && actual_arguments.len() == required_arguments.len()
                && actual_arguments
                    .iter()
                    .zip(required_arguments)
                    .all(|(actual, required)| {
                        required_type_matches(
                            program,
                            *actual,
                            *required,
                            generic_types,
                            bindings,
                            binder_bindings,
                        )
                    })
        }
        _ => crate::type_references::type_references_match(program, actual, required),
    }
}

fn fixed_array_lengths_match(
    actual: &FixedArrayLength,
    required: &FixedArrayLength,
    binder_bindings: &[BinderBinding],
) -> bool {
    match (actual, required) {
        (FixedArrayLength::Literal(actual), FixedArrayLength::Literal(required)) => {
            actual == required
        }
        (
            FixedArrayLength::ConstParameter { symbol: actual, .. },
            FixedArrayLength::ConstParameter {
                symbol: required, ..
            },
        ) => binder_bindings
            .iter()
            .find(|binding| binding.required == *required)
            .map_or(actual == required, |binding| binding.actual == *actual),
        (
            FixedArrayLength::ConstCall { name: actual },
            FixedArrayLength::ConstCall { name: required },
        ) => actual == required,
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_contract_facts(
    program: &TypedTrees,
    label: &str,
    parameter: &TypeParameter,
    requirement: &psi_typed_trees::signature::StateSignature,
    actual_contracts: &[SignatureContract],
    required_parameters: &[StateParameter],
    actual_parameters: &[StateParameter],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let required_contracts = program.state_signature_contracts(requirement);
    for kind in [
        SignatureContractKind::Requires,
        SignatureContractKind::Ensures,
        SignatureContractKind::Boundary,
    ] {
        let required = normalized_facts(program, required_contracts, kind, required_parameters);
        let actual = normalized_facts(program, actual_contracts, kind, actual_parameters);
        let valid = match kind {
            // Required preconditions may be stronger: callers of the generic
            // already prove them, so the selected implementation may demand a
            // subset. Selected postconditions must cover the requirement.
            SignatureContractKind::Requires => actual.iter().all(|fact| required.contains(fact)),
            SignatureContractKind::Ensures | SignatureContractKind::Boundary => {
                required.iter().all(|fact| actual.contains(fact))
            }
        };
        if !valid {
            diagnostics.push(Diagnostic::error(format!(
                "{label} does not refine `{}`: its {} facts are not a conservative refinement",
                parameter.name,
                contract_kind_name(kind)
            )));
        }
    }
}

fn normalized_facts(
    program: &TypedTrees,
    contracts: &[SignatureContract],
    kind: SignatureContractKind,
    parameters: &[StateParameter],
) -> Vec<String> {
    let mut facts = Vec::new();
    for contract in contracts.iter().filter(|contract| contract.kind == kind) {
        for fact in program.tables.proof_facts.span_or_empty(contract.facts) {
            if matches!(
                fact,
                ProofFact::Expression(expression)
                    if matches!(
                        program.expression_table.expression(*expression),
                        ExpressionNode::Boolean(true)
                    )
            ) {
                // `true` contributes no precondition or postcondition. Treat
                // it as the identity element so an implementation need not
                // redundantly republish a tautology to refine a slot.
                continue;
            }
            let raw = match fact {
                ProofFact::Expression(expression) => {
                    program.expression_table.display_name(*expression)
                }
                ProofFact::Membership(membership) => format!(
                    "{} in {}",
                    program.expression_table.display_name(membership.value),
                    program
                        .domain_path_members(membership.domain)
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::")
                ),
            };
            facts.push(alpha_normalize(&raw, parameters));
        }
    }
    facts.sort();
    facts.dedup();
    facts
}

fn alpha_normalize(text: &str, parameters: &[StateParameter]) -> String {
    let characters: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    while cursor < characters.len() {
        if !characters[cursor].is_ascii_alphanumeric() && characters[cursor] != '_' {
            output.push(characters[cursor]);
            cursor += 1;
            continue;
        }

        let start = cursor;
        while cursor < characters.len()
            && (characters[cursor].is_ascii_alphanumeric() || characters[cursor] == '_')
        {
            cursor += 1;
        }
        let word: String = characters[start..cursor].iter().collect();
        let previous = characters[..start]
            .iter()
            .rev()
            .copied()
            .find(|character| !character.is_whitespace());
        let next = characters[cursor..]
            .iter()
            .copied()
            .find(|character| !character.is_whitespace());
        // Replace value references, never field/case labels, member names,
        // call targets, or type constructors that merely share the spelling.
        let is_declaration_name =
            matches!(previous, Some('.' | ':')) || matches!(next, Some(':' | '(' | '{'));
        if !is_declaration_name
            && let Some(index) = parameters
                .iter()
                .position(|parameter| parameter.name.as_str() == word)
        {
            output.push('$');
            output.push_str(&index.to_string());
        } else {
            output.push_str(&word);
        }
    }
    output
}

fn contract_kind_name(kind: SignatureContractKind) -> &'static str {
    match kind {
        SignatureContractKind::Requires => "requires",
        SignatureContractKind::Ensures => "ensures",
        SignatureContractKind::Boundary => "boundary",
    }
}

fn machine_parameter_contract(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<(&TypeParameter, &psi_typed_trees::signature::StateSignature)> {
    for machine in program.machines() {
        if let Some(found) =
            machine_parameter_contract_in(program, program.machine_type_parameters(machine), symbol)
        {
            return Some(found);
        }
    }
    for data in program.data_definitions() {
        if let Some(found) =
            machine_parameter_contract_in(program, program.data_type_parameters(data), symbol)
        {
            return Some(found);
        }
    }
    None
}

fn machine_parameter_contract_in<'program>(
    program: &'program TypedTrees,
    parameters: &'program [TypeParameter],
    symbol: SymbolHandle,
) -> Option<(
    &'program TypeParameter,
    &'program psi_typed_trees::signature::StateSignature,
)> {
    for parameter in parameters {
        let TypeParameterKind::Machine { contract } = &parameter.kind else {
            continue;
        };
        if parameter.symbol == symbol || contract.symbol == symbol {
            return Some((parameter, contract));
        }
        if let Some(found) = machine_parameter_contract_in(
            program,
            program.state_signature_type_parameters(contract),
            symbol,
        ) {
            return Some(found);
        }
    }
    None
}

fn machine_and_state(
    program: &TypedTrees,
    selected_symbol: SymbolHandle,
) -> Option<(&Machine, &State)> {
    if !selected_symbol.is_valid() {
        return None;
    }
    program.machines().iter().find_map(|machine| {
        let states = program.machine_states(machine);
        states
            .iter()
            .find(|state| state.symbol == selected_symbol)
            .or_else(|| {
                (machine.symbol == selected_symbol)
                    .then(|| states.first())
                    .flatten()
            })
            .map(|state| (machine, state))
    })
}
