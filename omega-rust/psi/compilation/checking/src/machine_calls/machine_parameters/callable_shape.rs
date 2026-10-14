use super::contract_facts::validate_contract_facts;
use super::type_refinement::{BinderBinding, TypeBinding, required_type_matches};
use super::{
    MachineBlockingRow, MachineStateIndex, MachineSuspensionRow, machine_and_state,
    machine_parameter_contract, machine_parameter_signature,
};
use crate::machine_calls::effect_inference::full_parameter_invocation;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContract, StateParameter};
use typed_trees::state::State;
use typed_trees::typed_trees::AppliedMachineCallable;
use typed_trees::types::TypeReferenceHandle;

mod nominal_inputs;

/// The shape one callable presents for refinement against a machine
/// parameter's requirement: a machine entry, a forwarded machine parameter or
/// a nested machine contract all reduce to these parts.
pub(super) struct CallableParts<'a> {
    pub(super) lexical_owner: SymbolHandle,
    pub(super) lifetime_parameters: &'a [typed_trees::name::Identifier],
    pub(super) type_parameters: &'a [TypeParameter],
    pub(super) parameters: &'a [StateParameter],
    pub(super) return_type: TypeReferenceHandle,
    pub(super) terminates: bool,
    pub(super) contracts: &'a [SignatureContract],
}

impl<'a> CallableParts<'a> {
    /// The parts a state signature declares: a machine parameter's contract
    /// presents exactly what it authored.
    fn of_signature(
        program: &'a dyn crate::CheckingInput,
        signature: &'a typed_trees::signature::StateSignature,
    ) -> Self {
        Self {
            lexical_owner: crate::borrow_contracts::lexical_region_owner(program, signature.symbol),
            lifetime_parameters: &signature.lifetime_parameters,
            type_parameters: program.state_signature_type_parameters(signature),
            parameters: program.state_signature_parameters(signature),
            return_type: signature.return_type,
            terminates: signature.termination_guarantee.promises_termination(),
            contracts: program.state_signature_contracts(signature),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_selected_callable_shape(
    program: &dyn crate::CheckingInput,
    index: &MachineStateIndex,
    generic_call: &str,
    parameter: &TypeParameter,
    requirement: &typed_trees::signature::StateSignature,
    selected_symbol: SymbolHandle,
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
    ordinal: usize,
    selected_name: &str,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some((actual_machine, actual_state)) = machine_and_state(program, index, selected_symbol)
    {
        if let Some(applied) = program.applied_machine_callable(
            application,
            ordinal,
            selected_symbol,
            parameter.symbol,
            requirement,
        ) {
            let label = format!(
                "machine argument `{}` for `{generic_call}`",
                actual_machine.name
            );
            validate_callable_formation(
                program,
                &label,
                parameter,
                applied.required,
                &CallableParts {
                    lexical_owner: applied.lexical_owner,
                    ..CallableParts::of_signature(program, applied.selected)
                },
                Some(&applied),
                generic_types,
                bindings,
                &mut Vec::new(),
                diagnostics,
            );
            return;
        }
        if program.has_applied_machine_selection(application, ordinal, selected_symbol) {
            diagnostics.push(Diagnostic::error(format!(
                "machine argument `{selected_name}` for `{generic_call}` lost its exact applied demand for `{}`",
                parameter.name
            )));
            return;
        }
        validate_callable_shape(
            program,
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
        let applied = program.applied_machine_callable(
            application,
            ordinal,
            selected_symbol,
            parameter.symbol,
            requirement,
        );
        if program.has_applied_machine_selection(application, ordinal, selected_symbol)
            && applied.is_none()
        {
            diagnostics.push(Diagnostic::error(
                "forwarded callable lost its exact applied demand",
            ));
            return;
        }
        let (required, selected) = applied
            .as_ref()
            .map_or((requirement, actual_signature), |view| {
                (view.required, view.selected)
            });
        validate_callable_parts(
            program,
            &label,
            parameter,
            required,
            &CallableParts::of_signature(program, selected),
            selected,
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
    program: &dyn crate::CheckingInput,
    generic_call: &str,
    parameter: &TypeParameter,
    requirement: &typed_trees::signature::StateSignature,
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
    validate_callable_formation(
        program,
        &label,
        parameter,
        requirement,
        &CallableParts {
            lexical_owner: actual_machine.symbol,
            lifetime_parameters: &actual_machine.lifetime_parameters,
            type_parameters: program.machine_type_parameters(actual_machine),
            parameters: program.state_parameters(actual_state),
            return_type: actual_state.return_type,
            terminates: matches!(
                &actual_machine.termination_plan.interface,
                language_semantics::TerminationInterface::Published(
                    language_semantics::TerminationGuarantee::Terminates { .. }
                )
            ),
            contracts: program.machine_contracts(actual_machine),
        },
        None,
        generic_types,
        bindings,
        &mut Vec::new(),
        diagnostics,
    );
}

#[allow(clippy::too_many_arguments)]
fn validate_callable_parts(
    program: &dyn crate::CheckingInput,
    label: &str,
    parameter: &TypeParameter,
    requirement: &typed_trees::signature::StateSignature,
    actual: &CallableParts<'_>,
    actual_signature: &typed_trees::signature::StateSignature,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    binder_bindings: &mut Vec<BinderBinding>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_callable_formation(
        program,
        label,
        parameter,
        requirement,
        actual,
        None,
        generic_types,
        bindings,
        binder_bindings,
        diagnostics,
    );
    let actual_operational =
        crate::machine_calls::effect_inference::signature_operational(program, actual_signature);
    validate_callable_effects(
        program,
        label,
        parameter,
        requirement,
        actual.parameters,
        &crate::machine_calls::effect_inference::signature_service_reach(program, actual_signature),
        &crate::declared_signature_invocations(program, actual_signature),
        actual_operational.may_suspend,
        actual_operational.may_block,
        diagnostics,
    );
}

#[allow(clippy::too_many_arguments)]
fn validate_callable_formation(
    program: &dyn crate::CheckingInput,
    label: &str,
    parameter: &TypeParameter,
    requirement: &typed_trees::signature::StateSignature,
    actual: &CallableParts<'_>,
    applied: Option<&AppliedMachineCallable<'_>>,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    binder_bindings: &mut Vec<BinderBinding>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let formation_diagnostic_count = diagnostics.len();
    let actual_parameters = actual.parameters;
    validate_callable_type_parameters(
        program,
        label,
        parameter,
        requirement,
        actual.type_parameters,
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

    let mut formation_binders = binder_bindings.clone();
    formation_binders.extend(required_parameters.iter().zip(actual_parameters).map(
        |(required, actual)| BinderBinding {
            required: required.symbol,
            actual: actual.symbol,
        },
    ));

    for (index, (required, actual)) in required_parameters
        .iter()
        .zip(actual_parameters)
        .enumerate()
    {
        if required.is_mutable != actual.is_mutable
            || required.is_const != actual.is_const
            || required.relevance != actual.relevance
        {
            diagnostics.push(Diagnostic::error(format!(
                "{label} does not refine `{}`: parameter {} has a different calling mode",
                parameter.name, index
            )));
            continue;
        }
        if !complete_callable_type_matches(
            program,
            actual.type_reference,
            required.type_reference,
            generic_types,
            bindings,
            &formation_binders,
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

    if !complete_callable_type_matches(
        program,
        actual.return_type,
        requirement.return_type,
        generic_types,
        bindings,
        &formation_binders,
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: expected return `{}`, got `{}`",
            parameter.name,
            program.display_type_reference(requirement.return_type),
            program.display_type_reference(actual.return_type)
        )));
    }

    // Complete formation precedes implication. Carrier-only matching can
    // infer generic substitutions, but cannot erase declared qualifications
    // and then use contract refinement to legitimize a different signature.
    if diagnostics.len() != formation_diagnostic_count {
        return;
    }

    let substitutions = bindings
        .iter()
        .map(|binding| (binding.symbol, binding.actual))
        .collect::<Vec<_>>();
    if let Err(message) = validate_callable_regions(
        program,
        requirement,
        actual,
        applied,
        &substitutions,
        &formation_binders,
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: {message}",
            parameter.name
        )));
        return;
    }

    if requirement.termination_guarantee.promises_termination() && !actual.terminates {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: the requirement guarantees termination",
            parameter.name
        )));
    }

    validate_contract_facts(
        program,
        label,
        parameter,
        requirement,
        actual.contracts,
        required_parameters,
        actual_parameters,
        diagnostics,
    );
}

fn validate_callable_regions(
    program: &dyn crate::CheckingInput,
    requirement: &typed_trees::signature::StateSignature,
    actual: &CallableParts<'_>,
    applied: Option<&AppliedMachineCallable<'_>>,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    binder_bindings: &[BinderBinding],
) -> Result<(), &'static str> {
    use crate::borrow_contracts::CallableBorrowContract;
    let binders = binder_bindings
        .iter()
        .enumerate()
        .flat_map(|(ordinal, binding)| {
            let name = format!("$formation{ordinal}");
            [(binding.required, name.clone()), (binding.actual, name)]
        })
        .collect::<Vec<_>>();
    let mut required = CallableBorrowContract {
        lexical_owner: crate::borrow_contracts::lexical_region_owner(program, requirement.symbol),
        lifetime_parameters: &requirement.lifetime_parameters,
        parameters: program.state_signature_parameters(requirement),
        return_type: requirement.return_type,
        substitutions,
        substitution_scope_starts: &[0],
        application_scopes: &[],
        binders: &binders,
        lifetime_bindings: &[],
        argument_owner: actual.lexical_owner,
        supplied_telescope: false,
    };
    let mut selected = CallableBorrowContract {
        lexical_owner: actual.lexical_owner,
        lifetime_parameters: actual.lifetime_parameters,
        parameters: actual.parameters,
        return_type: actual.return_type,
        substitutions,
        substitution_scope_starts: &[0],
        application_scopes: &[],
        binders: &binders,
        lifetime_bindings: &[],
        argument_owner: actual.lexical_owner,
        supplied_telescope: false,
    };
    let caller_substitutions;
    let provider_substitutions;
    if let Some(applied) = applied {
        // Applied carriers serve full formation. Region proof instead uses
        // the source declarations and separately scoped simultaneous operands.
        let source = program
            .expression_table
            .static_machine_application(applied.provider);
        let machine = program
            .machine_by_symbol(source.binding_declaration)
            .ok_or("applied callable lost its original machine declaration")?;
        let state = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == source.registration_operation)
            .ok_or("applied callable lost its original entry state")?;
        caller_substitutions = applied
            .caller_operands
            .iter()
            .map(|operand| (operand.parameter, operand.actual))
            .collect::<Vec<_>>();
        provider_substitutions = program
            .expression_table
            .static_machine_application_bindings(applied.provider)
            .iter()
            .filter_map(|binding| match binding.value {
                typed_trees::static_applications::StaticMachineBindingValue::Type(reference)
                | typed_trees::static_applications::StaticMachineBindingValue::Const(reference) => {
                    Some((binding.parameter, reference))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        required = CallableBorrowContract {
            lexical_owner: crate::borrow_contracts::lexical_region_owner(
                program,
                applied.source_required.symbol,
            ),
            lifetime_parameters: &applied.source_required.lifetime_parameters,
            parameters: program.state_signature_parameters(applied.source_required),
            return_type: applied.source_required.return_type,
            substitutions: &caller_substitutions,
            substitution_scope_starts: &[0],
            application_scopes: &[],
            binders: &binders,
            lifetime_bindings: applied.caller_lifetimes,
            argument_owner: applied.lexical_owner,
            supplied_telescope: false,
        };
        selected = CallableBorrowContract {
            lexical_owner: machine.symbol,
            lifetime_parameters: &machine.lifetime_parameters,
            parameters: program.state_parameters(state),
            return_type: state.return_type,
            substitutions: &provider_substitutions,
            substitution_scope_starts: &[0],
            application_scopes: &[],
            binders: &binders,
            lifetime_bindings: program
                .expression_table
                .static_machine_application_lifetimes(applied.provider),
            argument_owner: applied.lexical_owner,
            supplied_telescope: true,
        };
    }
    crate::borrow_contracts::validate_callable_borrow_refinement(program, required, selected)
}

#[allow(clippy::too_many_arguments)]
fn complete_callable_type_matches(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    binder_bindings: &[BinderBinding],
) -> bool {
    if !required_type_matches(
        program,
        actual,
        required,
        generic_types,
        bindings,
        binder_bindings,
    ) {
        return false;
    }
    if !actual.is_valid() || !required.is_valid() {
        return actual.is_valid() == required.is_valid();
    }
    let mut positional = Vec::with_capacity(binder_bindings.len() * 2);
    for (position, binding) in binder_bindings.iter().enumerate() {
        let label = format!("$formation{position}");
        positional.push((binding.required, label.clone()));
        positional.push((binding.actual, label));
    }
    let substitutions = bindings
        .iter()
        .map(|binding| (binding.symbol, binding.actual))
        .collect::<Vec<_>>();
    let identity = |reference| {
        program.type_identity(typed_trees::type_identity::TypeIdentityRequest {
            binders: &positional,
            substitutions: &substitutions,
            ..typed_trees::type_identity::TypeIdentityRequest::ordinary(reference)
        })
    };
    identity(required) == identity(actual)
}

#[allow(clippy::too_many_arguments)]
fn validate_proposition_parameter_formation(
    program: &dyn crate::CheckingInput,
    label: &str,
    required: &typed_trees::data::PropositionParameterSignature,
    actual: &typed_trees::data::PropositionParameterSignature,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    binder_bindings: &[BinderBinding],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let required = program.state_parameters.span_or_empty(required.parameters);
    let actual = program.state_parameters.span_or_empty(actual.parameters);
    if required.len() != actual.len() {
        diagnostics.push(Diagnostic::error(format!(
            "{label}: proposition parameter signatures have different arity"
        )));
        return;
    }
    let mut formation_binders = binder_bindings.to_vec();
    formation_binders.extend(
        required
            .iter()
            .zip(actual)
            .map(|(required, actual)| BinderBinding {
                required: required.symbol,
                actual: actual.symbol,
            }),
    );
    for (position, (required, actual)) in required.iter().zip(actual).enumerate() {
        if required.is_mutable != actual.is_mutable
            || required.is_const != actual.is_const
            || required.relevance != actual.relevance
            || !complete_callable_type_matches(
                program,
                actual.type_reference,
                required.type_reference,
                generic_types,
                bindings,
                &formation_binders,
            )
        {
            diagnostics.push(Diagnostic::error(format!(
                "{label}: proposition parameter {position} has a different complete calling shape"
            )));
        }
    }
}

/// Only this judgment needs the cleanup-complete concrete dependency graph.
/// Shape, nominal identity, and predicate formation have already been judged.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_selected_callable_effects(
    program: &dyn crate::CheckingInput,
    suspensions: &[MachineSuspensionRow],
    blockings: &[MachineBlockingRow],
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    invocations: &flow_effects::InvocationInferencePlan,
    label: &str,
    parameter: &TypeParameter,
    requirement: &typed_trees::signature::StateSignature,
    actual_machine: &Machine,
    actual_state: &State,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let services = service_reaches
        .for_machine(actual_machine.symbol)
        .map(|summary| service_reaches.services(summary.effective))
        .unwrap_or_else(|| {
            program
                .service_reach_rows
                .services(actual_machine.service_reach_row)
        });
    let actual_invocations = invocations
        .for_machine(actual_machine.symbol)
        .map(|summary| summary.effective.as_slice())
        .unwrap_or_default();
    let may_suspend = suspensions
        .iter()
        .find(|row| row.symbol == actual_machine.symbol)
        .map(|row| row.transitive_may_suspend)
        .unwrap_or(actual_machine.suspends);
    let may_block = blockings
        .iter()
        .find(|row| row.symbol == actual_machine.symbol)
        .map(|row| row.transitive_may_block)
        .unwrap_or(actual_machine.blocks);
    validate_callable_effects(
        program,
        label,
        parameter,
        requirement,
        program.state_parameters(actual_state),
        services,
        actual_invocations,
        may_suspend,
        may_block,
        diagnostics,
    );
}

#[allow(clippy::too_many_arguments)]
fn validate_callable_effects(
    program: &dyn crate::CheckingInput,
    label: &str,
    parameter: &TypeParameter,
    requirement: &typed_trees::signature::StateSignature,
    actual_parameters: &[StateParameter],
    services: &[language_semantics::ServiceReachId],
    invocations: &[flow_effects::InvocationTarget],
    may_suspend: bool,
    may_block: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let required_parameters = program.state_signature_parameters(requirement);
    let allowed_services =
        crate::machine_calls::effect_inference::signature_service_reach(program, requirement);
    let operational =
        crate::machine_calls::effect_inference::signature_operational(program, requirement);
    for service in services {
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

    let allowed_invocations = crate::declared_signature_invocations(program, requirement)
        .iter()
        .filter_map(|target| full_parameter_invocation(*target, required_parameters))
        .collect::<Vec<_>>();
    for invocation in invocations {
        if full_parameter_invocation(*invocation, actual_parameters)
            .is_some_and(|target| allowed_invocations.contains(&target))
        {
            continue;
        }
        let name = match invocation {
            flow_effects::InvocationTarget::Parameter(index) => actual_parameters
                .get(*index as usize)
                .map(|parameter| parameter.name.as_str())
                .unwrap_or("<unknown binding>"),
            flow_effects::InvocationTarget::Service(symbol) => program
                .trait_definition_by_symbol(*symbol)
                .map(|definition| definition.name.as_str())
                .unwrap_or("<unknown boundary service>"),
        };
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: synchronous invocation `{name}` exceeds its authored `invokes` ceiling",
            parameter.name
        )));
    }

    if may_suspend && !operational.may_suspend {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: it may suspend, but the requirement omits `suspends;`",
            parameter.name
        )));
    }
    if may_block && !operational.may_block {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: it may block, but the requirement omits `blocks;`",
            parameter.name
        )));
    }
}

/// Check an authored callable generic parameter list against a trait
/// requirement. Trait conformance needs the same recursive machine-contract
/// judgment as a concrete static selection; a kind-only comparison would let
/// a provider silently change `machine Target(...)` beneath an otherwise
/// matching requirement.
/// Compare nested machine contracts under an exact positional binder mapping.
/// This is callable telescope formation, not trait supply or implementation
/// selection; callers separately establish roster shape and binder categories.
pub fn validate_trait_callable_parameter_refinement(
    program: &dyn crate::CheckingInput,
    label: &str,
    requirement_parameters: &[TypeParameter],
    actual_parameters: &[TypeParameter],
    generic_types: &[&TypeParameter],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut bindings = Vec::new();
    let binder_bindings = requirement_parameters
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
        validate_callable_input(
            program,
            &nested_label,
            required,
            required_contract,
            actual_contract,
            generic_types,
            &mut bindings,
            &binder_bindings,
            diagnostics,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_callable_type_parameters(
    program: &dyn crate::CheckingInput,
    label: &str,
    parameter: &TypeParameter,
    requirement: &typed_trees::signature::StateSignature,
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
            (TypeParameterKind::Implementation, TypeParameterKind::Implementation) => {
                // The binder selects code; its owner separately carries the
                // exact conformance obligations. It has no nested signature.
            }
            (
                TypeParameterKind::Proposition {
                    contract: required_contract,
                },
                TypeParameterKind::Proposition {
                    contract: actual_contract,
                },
            ) => {
                validate_proposition_parameter_formation(
                    program,
                    label,
                    required_contract,
                    actual_contract,
                    generic_types,
                    bindings,
                    binder_bindings,
                    diagnostics,
                );
            }
            (TypeParameterKind::Type, TypeParameterKind::Type) => {
                if typed_trees::data::type_parameter_demands_stronger_properties(required, actual) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{label} does not refine `{}`: generic parameter {} demands stronger type properties",
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
                validate_callable_input(
                    program,
                    &nested_label,
                    required,
                    required_contract,
                    actual_contract,
                    generic_types,
                    bindings,
                    binder_bindings,
                    diagnostics,
                );
            }
            _ => {
                if let Some((required_type, actual_type)) =
                    crate::value_custody::type_shapes::scalar_parameter_type_pair(
                        &required.kind,
                        &actual.kind,
                    )
                {
                    if !complete_callable_type_matches(
                        program,
                        actual_type,
                        required_type,
                        generic_types,
                        bindings,
                        binder_bindings,
                    ) {
                        diagnostics.push(Diagnostic::error(format!(
                            "{label} does not refine `{}`: const generic parameter {} has a different type",
                            parameter.name, index
                        )));
                    }
                } else {
                    diagnostics.push(Diagnostic::error(format!(
                        "{label} does not refine `{}`: generic parameter {} has a different kind",
                        parameter.name, index
                    )));
                }
            }
        }
    }
}

/// Callable inputs are contravariant: a supplied higher-order body must accept
/// every callable its requirement promises to the caller. Check the promised
/// input as a provider of the body's demanded input, reversing the exact
/// positional mapping as well. Each recursive input reverses again, so nominal
/// authority, predicates and operational ceilings alternate at every depth.
#[allow(clippy::too_many_arguments)]
fn validate_callable_input(
    program: &dyn crate::CheckingInput,
    label: &str,
    parameter: &TypeParameter,
    promised: &typed_trees::data::MachineParameterContract,
    demanded: &typed_trees::data::MachineParameterContract,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    binder_bindings: &[BinderBinding],
    diagnostics: &mut Vec<Diagnostic>,
) {
    nominal_inputs::validate(
        program,
        label,
        promised,
        demanded,
        generic_types,
        bindings,
        binder_bindings,
        diagnostics,
    );
    let promised = machine_parameter_signature(program, promised);
    let demanded = machine_parameter_signature(program, demanded);
    let mut reversed = binder_bindings
        .iter()
        .map(|binding| BinderBinding {
            required: binding.actual,
            actual: binding.required,
        })
        .collect::<Vec<_>>();
    validate_callable_parts(
        program,
        label,
        parameter,
        demanded,
        &CallableParts::of_signature(program, promised),
        promised,
        generic_types,
        bindings,
        &mut reversed,
        diagnostics,
    );
}
