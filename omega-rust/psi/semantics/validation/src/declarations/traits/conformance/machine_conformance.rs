//! Validating a machine's trait conformances: top-level requirements,
//! operator and single-requirement conformance, named evidence contracts
//! and forwarded trait arguments.

use crate::declarations::traits::conformance::signature_matching::{
    TraitTypeBinding, TraitTypeBindingTarget, parameter_shape_label,
    type_references_match_with_trait_bindings,
    validate_machine_state_satisfies_trait_signature_with_arguments,
};
use crate::declarations::traits::conformance::trait_applications::validate_trait_application_obligations;
use crate::declarations::traits::trait_definition_by_symbol;
use crate::value_custody::type_references::type_reference_label;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::domain::ProofFact;
use typed_trees::machine::Machine;
use typed_trees::proposition::{ProofSubstitutions, PropositionLabels};
use typed_trees::signature::{SignatureContractKind, StateSignature};
use typed_trees::state::State;
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_machine_trait_conformances(
    program: &TypedTrees,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    machine: &Machine,
    symbols: &crate::declarations::symbols::TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for conformance in program.machine_trait_conformances(machine) {
        if let Some(requirement) = program.machines().iter().find(|requirement| {
            requirement.symbol == conformance.symbol
                && requirement.supply_mode
                    == language_semantics::MachineSupplyMode::TopLevelRequirement
        }) {
            validate_machine_top_level_requirement_conformance(
                program,
                service_reaches,
                machine,
                requirement,
                conformance,
                diagnostics,
            );
            continue;
        }
        let Some(trait_definition) = trait_definition_by_symbol(program, conformance.symbol) else {
            if validate_machine_operator_conformance(
                program,
                machine,
                conformance,
                symbols,
                diagnostics,
            ) {
                continue;
            }
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies unknown trait `{}`",
                machine.name, conformance.name
            )));
            continue;
        };
        let expected_lifetime_arguments = trait_definition.lifetime_parameters.len();
        if conformance.trait_lifetime_arguments.len() != expected_lifetime_arguments {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` conformance to trait `{}` expects {expected_lifetime_arguments} target-trait lifetime argument(s), got {}",
                machine.name,
                trait_definition.name,
                conformance.trait_lifetime_arguments.len(),
            )));
            continue;
        }
        if conformance.trait_lifetime_arguments.iter().any(|ordinal| {
            usize::try_from(*ordinal)
                .map_or(true, |ordinal| ordinal >= machine.lifetime_parameters.len())
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` conformance to trait `{}` retains a target-trait lifetime outside its machine telescope",
                machine.name, trait_definition.name,
            )));
            continue;
        }
        let explicit_type_arguments = program
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        let expected_type_arguments = program.trait_type_parameters(trait_definition).len();
        if explicit_type_arguments.len() != expected_type_arguments {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` conformance to trait `{}` expects {expected_type_arguments} generic argument(s), got {}",
                machine.name,
                trait_definition.name,
                explicit_type_arguments.len()
            )));
            continue;
        }

        validate_trait_application_obligations(
            program,
            trait_definition,
            explicit_type_arguments,
            &machine.conformance_bounds,
            &format!(
                "machine `{}` conformance to trait `{}`",
                machine.name, trait_definition.name
            ),
            diagnostics,
        );

        let Some(requirement_name) = conformance.requirement.as_ref() else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has a malformed conformance to `{}` without an exact requirement",
                machine.name, trait_definition.name
            )));
            continue;
        };
        validate_machine_single_requirement(
            program,
            service_reaches,
            machine,
            trait_definition,
            requirement_name,
            conformance.requirement_symbol,
            conformance.alias.as_ref().map(|alias| alias.as_str()),
            &conformance.trait_lifetime_arguments,
            explicit_type_arguments,
            diagnostics,
        );
    }
}

fn validate_machine_top_level_requirement_conformance(
    program: &TypedTrees,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    machine: &Machine,
    requirement: &Machine,
    conformance: &typed_trees::machine::TraitConformance,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let requirement_identity = requirement.name.as_str();
    let label = format!(
        "provider machine `{}` for top-level boundary requirement `{requirement_identity}`",
        machine.name
    );
    if !conformance.trait_lifetime_arguments.is_empty() {
        diagnostics.push(Diagnostic::error(format!(
            "{label} supplies target-trait lifetime arguments to a non-trait requirement"
        )));
        return;
    }
    if conformance.requirement_symbol != requirement.symbol {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not retain the exact selected requirement symbol"
        )));
        return;
    }
    if !program
        .type_reference_table
        .type_reference_handles(conformance.arguments)
        .is_empty()
    {
        diagnostics.push(Diagnostic::error(format!(
            "{label} supplies trait-style arguments; a top-level requirement owns its own static telescope"
        )));
        return;
    }
    let Some(requirement_entry) = program.machine_states(requirement).first() else {
        diagnostics.push(Diagnostic::error(format!(
            "top-level boundary requirement `{requirement_identity}` has no entry signature"
        )));
        return;
    };
    let Some(provider_entry) = program.machine_states(machine).first() else {
        diagnostics.push(Diagnostic::error(format!("{label} has no entry signature")));
        return;
    };

    if requirement.lifetime_parameters.len() != machine.lifetime_parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "{label} has {} lifetime parameter(s); the requirement declares {}",
            machine.lifetime_parameters.len(),
            requirement.lifetime_parameters.len(),
        )));
    }

    let required_type_parameters = program.machine_type_parameters(requirement);
    let actual_type_parameters = program.machine_type_parameters(machine);
    if required_type_parameters.len() != actual_type_parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "{label} has {} static parameter(s); the requirement declares {}",
            actual_type_parameters.len(),
            required_type_parameters.len(),
        )));
        return;
    }
    for (index, (required, actual)) in required_type_parameters
        .iter()
        .zip(actual_type_parameters)
        .enumerate()
    {
        let same_kind = matches!(
            (&required.kind, &actual.kind),
            (TypeParameterKind::Type, TypeParameterKind::Type)
                | (
                    TypeParameterKind::Const { .. },
                    TypeParameterKind::Const { .. }
                )
                | (
                    TypeParameterKind::Value { .. },
                    TypeParameterKind::Value { .. }
                )
                | (
                    TypeParameterKind::Machine { .. },
                    TypeParameterKind::Machine { .. }
                )
                | (
                    TypeParameterKind::Proposition { .. },
                    TypeParameterKind::Proposition { .. }
                )
        );
        if !same_kind {
            diagnostics.push(Diagnostic::error(format!(
                "{label} static parameter {index} has a different kind"
            )));
        }
    }
    let generic_types = required_type_parameters.iter().collect::<Vec<_>>();
    crate::machine_calls::machine_parameters::validate_trait_callable_parameter_refinement(
        program,
        &label,
        required_type_parameters,
        actual_type_parameters,
        &generic_types,
        diagnostics,
    );
    for (index, (required, actual)) in required_type_parameters
        .iter()
        .zip(actual_type_parameters)
        .enumerate()
    {
        if matches!(
            (&required.kind, &actual.kind),
            (TypeParameterKind::Type, TypeParameterKind::Type)
        ) && typed_trees::data::type_parameter_demands_stronger_properties(required, actual)
        {
            diagnostics.push(Diagnostic::error(format!(
                "{label} static parameter {index} demands stronger type properties"
            )));
        }
    }
    let mut type_bindings = required_type_parameters
        .iter()
        .zip(actual_type_parameters)
        .filter(|&(required, actual)| {
            matches!(
                (&required.kind, &actual.kind),
                (TypeParameterKind::Type, TypeParameterKind::Type)
                    | (
                        TypeParameterKind::Const { .. },
                        TypeParameterKind::Const { .. }
                    )
                    | (
                        TypeParameterKind::Value { .. },
                        TypeParameterKind::Value { .. }
                    )
            )
        })
        .map(|(required, actual)| TraitTypeBinding {
            parameter_symbol: required.symbol,
            parameter_name: required.name.as_str().to_owned(),
            target: TraitTypeBindingTarget::Parameter(actual.symbol),
        })
        .collect::<Vec<_>>();
    for (index, (required, actual)) in required_type_parameters
        .iter()
        .zip(actual_type_parameters)
        .enumerate()
    {
        let ((
            TypeParameterKind::Const {
                type_reference: required_type,
            },
            TypeParameterKind::Const {
                type_reference: actual_type,
            },
        )
        | (
            TypeParameterKind::Value {
                type_reference: required_type,
            },
            TypeParameterKind::Value {
                type_reference: actual_type,
            },
        )) = (&required.kind, &actual.kind)
        else {
            continue;
        };
        if !type_references_match_with_trait_bindings(
            program,
            *actual_type,
            *required_type,
            required_type_parameters,
            &mut type_bindings,
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "{label} const static parameter {index} has a different type"
            )));
        }
    }

    let required_parameters = program.state_parameters(requirement_entry);
    let actual_parameters = program.state_parameters(provider_entry);
    if required_parameters.len() != actual_parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "{label} has {} value parameter(s); the requirement declares {}",
            actual_parameters.len(),
            required_parameters.len(),
        )));
        return;
    }
    for (index, (required, actual)) in required_parameters
        .iter()
        .zip(actual_parameters)
        .enumerate()
    {
        if required.is_const != actual.is_const || required.is_mutable != actual.is_mutable {
            diagnostics.push(Diagnostic::error(format!(
                "{label} parameter {} has shape `{}`, but the requirement declares `{}`",
                index,
                parameter_shape_label(program, actual),
                parameter_shape_label(program, required),
            )));
            continue;
        }
        if required.is_self {
            let carrier_matches = requirement.attached_data_symbol.is_valid()
                && nominal_type_symbol(program, actual.type_reference)
                    == Some(requirement.attached_data_symbol);
            let shape_matches = !actual.is_self
                && type_references_match_with_trait_bindings(
                    program,
                    actual.type_reference,
                    required.type_reference,
                    required_type_parameters,
                    &mut type_bindings,
                );
            if !carrier_matches || !shape_matches {
                diagnostics.push(Diagnostic::error(format!(
                    "{label} parameter {} must realize the requirement receiver as exact carrier `{}` with the same reference shape; got `{}`",
                    index,
                    requirement
                        .attached_data
                        .as_ref()
                        .map_or("<missing carrier>", |name| name.as_str()),
                    parameter_shape_label(program, actual),
                )));
            }
            continue;
        }
        if actual.is_self
            || !type_references_match_with_trait_bindings(
                program,
                actual.type_reference,
                required.type_reference,
                required_type_parameters,
                &mut type_bindings,
            )
        {
            diagnostics.push(Diagnostic::error(format!(
                "{label} parameter {} expects `{}`, got `{}`",
                index,
                parameter_shape_label(program, required),
                parameter_shape_label(program, actual),
            )));
        }
    }

    let return_matches = type_references_match_with_trait_bindings(
        program,
        provider_entry.return_type,
        requirement_entry.return_type,
        required_type_parameters,
        &mut type_bindings,
    );
    let dispatch_matches = program.normalized_result_dispatch_set(provider_entry.return_type)
        == program.normalized_result_dispatch_set(requirement_entry.return_type);
    if !return_matches || !dispatch_matches {
        diagnostics.push(Diagnostic::error(format!(
            "{label} returns `{}`, but the requirement returns `{}` with its exact result dispatch",
            type_reference_label(program, provider_entry.return_type),
            type_reference_label(program, requirement_entry.return_type),
        )));
    }

    validate_top_level_requirement_effect_ceiling(
        program,
        service_reaches,
        machine,
        requirement,
        &label,
        diagnostics,
    );
    // An external leaf (`via` binding or compiler intrinsic) states no checked
    // contract of its own: the requirement's contract remains the public
    // contract and the binding's sealed catalog or foreign policy carries the
    // realization, exactly as for a boundary-operator satisfier. Only a
    // checked body refines the requirement's contract.
    if conformance.external_binding.is_none() && !conformance.via_expression.is_valid() {
        crate::machine_calls::machine_parameters::validate_callable_contract_refinement(
            program,
            &label,
            requirement_identity,
            program.machine_contracts(requirement),
            program.machine_contracts(machine),
            required_parameters,
            actual_parameters,
            diagnostics,
        );
    }
}

/// Recheck one exact top-level requirement realization from retained typed
/// custody. Later compiler-owned consumers use this after successful checking
/// so a substituted requirement or provider telescope cannot inherit the
/// earlier verdict.
pub fn revalidate_top_level_requirement_realization(
    program: &TypedTrees,
    machine: &Machine,
    requirement: &Machine,
    conformance: &typed_trees::machine::TraitConformance,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let operational = crate::infer_operational_may(program);
    let service_reaches = crate::infer_service_reaches(program, &operational);
    if requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
        || conformance.symbol != requirement.symbol
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` does not retain one exact top-level boundary requirement realization",
            machine.name
        )));
    } else {
        validate_machine_top_level_requirement_conformance(
            program,
            &service_reaches,
            machine,
            requirement,
            conformance,
            &mut diagnostics,
        );
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn nominal_type_symbol(program: &TypedTrees, handle: TypeReferenceHandle) -> Option<SymbolHandle> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => nominal_type_symbol(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            nominal_type_symbol(program, *base_type)
        }
        TypeReferenceNode::Named { symbol, .. } => symbol.is_valid().then_some(*symbol),
        TypeReferenceNode::Generic { base_symbol, .. } => {
            base_symbol.is_valid().then_some(*base_symbol)
        }
        _ => None,
    }
}

fn validate_top_level_requirement_effect_ceiling(
    program: &TypedTrees,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    machine: &Machine,
    requirement: &Machine,
    label: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let allowed_services = program
        .service_reach_rows
        .services(requirement.service_reach_row);
    // Checked providers contribute their whole call closure. Comparing only
    // authored clauses would let an annotation-free helper evade this bound.
    let Some(summary) = service_reaches.for_machine(machine.symbol) else {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` has no inferred service-reach summary for requirement checking",
            machine.name,
        )));
        return;
    };
    for service in service_reaches.services(summary.effective) {
        if !allowed_services.contains(service) {
            let service_name = program
                .service_reaches
                .definition(*service)
                .map(|definition| definition.name.as_str())
                .unwrap_or("<unknown canonical service>");
            diagnostics.push(Diagnostic::error(format!(
                "{label} reaches service `{service_name}` outside the requirement ceiling"
            )));
        }
    }
    if machine.suspends && !requirement.suspends {
        diagnostics.push(Diagnostic::error(format!(
            "{label} declares `suspends;` beyond the requirement ceiling"
        )));
    }
    if machine.blocks && !requirement.blocks {
        diagnostics.push(Diagnostic::error(format!(
            "{label} declares `blocks;` beyond the requirement ceiling"
        )));
    }

    let allowed_invocations = program.machine_invokes(requirement);
    for invocation in program.machine_invokes(machine) {
        if !allowed_invocations
            .iter()
            .any(|allowed| allowed.target == invocation.target)
        {
            diagnostics.push(Diagnostic::error(format!(
                "{label} invokes `{}` outside the requirement ceiling",
                invocation.name,
            )));
        }
    }

    let required_termination = requirement
        .termination_plan
        .interface
        .published()
        .unwrap_or(&requirement.termination_plan.checked_summary);
    let actual_termination = machine
        .termination_plan
        .interface
        .published()
        .unwrap_or(&machine.termination_plan.checked_summary);
    if required_termination.promises_termination() && !actual_termination.promises_termination() {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not provide the requirement's termination guarantee"
        )));
    }
}

/// Operator requirements share the ordinary machine `satisfies` spelling with
/// trait requirements, but resolve by exact overloaded signature rather than
/// a trait symbol. Boundary leaves retain their admitted-binding rule; checked
/// software providers for either boundary or ordinary operators must cover the
/// selected declaration's supported contract.
fn validate_machine_operator_conformance(
    program: &TypedTrees,
    machine: &Machine,
    conformance: &typed_trees::machine::TraitConformance,
    symbols: &crate::declarations::symbols::TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some(requirement) = conformance.requirement.as_ref() else {
        return false;
    };
    let namespace = conformance.name.as_str();
    let requirement_name = requirement.as_str();
    let names_operator = program.operators().iter().any(|operator| {
        let path = program.operator_path_members(operator.name);
        matches!(path, [owner, member]
            if owner.as_str() == namespace
                && member.as_str() == requirement_name)
    });
    if !names_operator {
        return false;
    }

    if !conformance.trait_lifetime_arguments.is_empty() {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` supplies target-trait lifetime arguments to operator requirement `{}::{}`",
            machine.name, namespace, requirement_name,
        )));
        return true;
    }

    if !program
        .type_reference_table
        .type_reference_handles(conformance.arguments)
        .is_empty()
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` supplies type arguments to operator requirement `{}::{}`; the exact overloaded operator is selected from the machine signature",
            machine.name, namespace, requirement_name,
        )));
        return true;
    }

    let operator = if let Some(operator) = typed_trees::operator::resolve_satisfied_checked_operator(
        program,
        machine,
        namespace,
        requirement_name,
    ) {
        operator
    } else {
        let retained = typed_trees::operator::resolve_specialized_checked_operator_application(
            program,
            machine,
            namespace,
            requirement_name,
        );
        let Some((operator, retained)) = retained else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` does not match one exact overload of operator requirement `{}::{}`; its entry parameter and result types must equal one declared requirement signature",
                machine.name, namespace, requirement_name,
            )));
            return true;
        };
        if let Err(diagnostic) =
            crate::declarations::operators::validate_closed_operator_application(
                program,
                symbols,
                operator,
                &retained.arguments,
            )
        {
            diagnostics.push(diagnostic);
            return true;
        }
        operator
    };
    if conformance.requirement_symbol != operator.symbol {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` operator realization `{}::{}` does not retain the exact selected overload",
            machine.name, namespace, requirement_name,
        )));
        return true;
    }

    if conformance.external_binding.is_none() && !conformance.via_expression.is_valid() {
        crate::proof_contracts::contract_entailment::check_operator_contract_conformance(
            program,
            machine,
            operator,
            diagnostics,
        );
    }
    true
}

/// Conform THIS machine to ONE trait requirement (the machine-by-machine
/// carrier model): the machine's ENTRY signature must match the requirement's
/// (with `Self` binding to the carrier type on first use). LAW requirements
/// (an `ensures` on the requirement) additionally demand a proven-ensures |=
/// declared-law match (rung B: contract_entailment::check_law_conformance).
fn validate_machine_single_requirement(
    program: &TypedTrees,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    machine: &Machine,
    trait_definition: &TraitDefinition,
    requirement_name: &typed_trees::name::Identifier,
    settled_requirement_symbol: symbols::SymbolHandle,
    conformance_alias: Option<&str>,
    trait_lifetime_arguments: &[u32],
    explicit_type_arguments: &[TypeReferenceHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    // The machine's conforming signature is its ENTRY state (the first state:
    // an implicit entry always parses first, and single-entry proof machines
    // are the shape this mode serves).
    let Some(entry_state) = program.machine_states(machine).first() else {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` satisfies `{}::{}` but has no states",
            machine.name, trait_definition.name, requirement_name
        )));
        return;
    };
    let implementation_dispatch = program.normalized_result_dispatch_set(entry_state.return_type);
    let named_requirements = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .filter(|requirement| requirement.name == *requirement_name)
        .collect::<Vec<_>>();
    if named_requirements.is_empty() {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` satisfies `{}::{}`, but trait `{}` has no requirement named `{}`",
            machine.name,
            trait_definition.name,
            requirement_name,
            trait_definition.name,
            requirement_name
        )));
        return;
    }
    let matching_requirements = if named_requirements.len() == 1 {
        named_requirements
    } else {
        named_requirements
            .into_iter()
            .filter(|requirement| {
                program.normalized_result_dispatch_set(requirement.return_type)
                    == implementation_dispatch
            })
            .collect::<Vec<_>>()
    };
    let [requirement] = matching_requirements.as_slice() else {
        let dispatch = if implementation_dispatch.is_empty() {
            "<empty>".to_owned()
        } else {
            implementation_dispatch.identity()
        };
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` satisfies `{}::{}`, but its entry result dispatch set `{dispatch}` selects {} matching requirement overload(s); exactly one is required",
            machine.name,
            trait_definition.name,
            requirement_name,
            matching_requirements.len(),
        )));
        return;
    };
    if requirement.symbol != settled_requirement_symbol {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` satisfies `{}::{}` but its retained exact requirement does not match overload resolution",
            machine.name, trait_definition.name, requirement_name,
        )));
        return;
    }

    validate_machine_state_satisfies_trait_signature_with_arguments(
        program,
        service_reaches,
        machine,
        entry_state,
        trait_definition,
        requirement,
        Some(trait_lifetime_arguments),
        explicit_type_arguments,
        diagnostics,
    );

    validate_concrete_named_evidence_contract_conformance(
        program,
        machine,
        entry_state,
        trait_definition,
        requirement,
        explicit_type_arguments,
        diagnostics,
    );

    // A LAW requirement (ensures on the requirement) demands the satisfier
    // PROVE the law: proven-ensures |= declared-law, forall-to-forall.
    crate::proof_contracts::contract_entailment::check_law_conformance(
        program,
        machine,
        conformance_alias,
        trait_definition,
        requirement,
        explicit_type_arguments,
        diagnostics,
    );
}

/// Validate the first deliberately closed named-witness conformance surface.
///
/// Incoming names are satisfier-local aliases, while outgoing names are the
/// requirement's public selectors. Both lanes otherwise retain exact order,
/// proposition application, and evidence-interface identity. Generic trait,
/// requirement, machine, and proposition telescopes remain fenced until the
/// conformance substitution carrier owns their complete identities.
#[allow(clippy::too_many_arguments)]
fn validate_concrete_named_evidence_contract_conformance(
    program: &TypedTrees,
    machine: &Machine,
    entry_state: &State,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    explicit_type_arguments: &[TypeReferenceHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let required_requires = named_contracts(
        program.state_signature_contracts(requirement),
        SignatureContractKind::Requires,
    );
    let required_ensures = named_contracts(
        program.state_signature_contracts(requirement),
        SignatureContractKind::Ensures,
    );
    let actual_requires = named_contracts(
        program.machine_contracts(machine),
        SignatureContractKind::Requires,
    );
    let actual_ensures = named_contracts(
        program.machine_contracts(machine),
        SignatureContractKind::Ensures,
    );
    if required_requires.is_empty()
        && required_ensures.is_empty()
        && actual_requires.is_empty()
        && actual_ensures.is_empty()
    {
        return;
    }

    if !explicit_type_arguments.is_empty()
        || !trait_definition.lifetime_parameters.is_empty()
        || !program.trait_type_parameters(trait_definition).is_empty()
        || !requirement.lifetime_parameters.is_empty()
        || !program
            .state_signature_type_parameters(requirement)
            .is_empty()
        || !machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(machine).is_empty()
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` satisfies `{}::{}` with named evidence, but named-witness conformance currently requires a concrete non-generic trait, requirement, and satisfier",
            machine.name, trait_definition.name, requirement.name,
        )));
        return;
    }

    validate_named_contract_lane(
        program,
        machine,
        entry_state,
        trait_definition,
        requirement,
        SignatureContractKind::Requires,
        &required_requires,
        &actual_requires,
        false,
        diagnostics,
    );
    validate_named_contract_lane(
        program,
        machine,
        entry_state,
        trait_definition,
        requirement,
        SignatureContractKind::Ensures,
        &required_ensures,
        &actual_ensures,
        true,
        diagnostics,
    );
}

fn named_contracts(
    contracts: &[typed_trees::signature::SignatureContract],
    kind: SignatureContractKind,
) -> Vec<&typed_trees::signature::SignatureContract> {
    contracts
        .iter()
        .filter(|contract| contract.kind == kind && contract.binding.is_some())
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn validate_named_contract_lane(
    program: &TypedTrees,
    machine: &Machine,
    entry_state: &State,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    kind: SignatureContractKind,
    required: &[&typed_trees::signature::SignatureContract],
    actual: &[&typed_trees::signature::SignatureContract],
    permits_strengthening: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let lane = match kind {
        SignatureContractKind::Requires => "requires",
        SignatureContractKind::Ensures => "ensures",
        _ => unreachable!("named conformance lane is requires or ensures"),
    };
    let cardinality_matches = if permits_strengthening {
        actual.len() >= required.len()
    } else {
        actual.len() == required.len()
    };
    if !cardinality_matches {
        let expectation = if permits_strengthening {
            format!("at least {}", required.len())
        } else {
            required.len().to_string()
        };
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` satisfies `{}::{}` but its named {lane} lane has {} row(s); the requirement owns {expectation}",
            machine.name,
            trait_definition.name,
            requirement.name,
            actual.len(),
        )));
        return;
    }

    let parameter_substitutions = program
        .state_signature_parameters(requirement)
        .iter()
        .zip(program.state_parameters(entry_state))
        .map(|(required, actual)| {
            (
                required.symbol,
                required.name.as_str().to_owned(),
                actual.name.as_str().to_owned(),
            )
        })
        .collect::<Vec<_>>();

    for (lane_position, (required, actual)) in required.iter().zip(actual).enumerate() {
        let required_name = required
            .binding
            .as_ref()
            .expect("named requirement row has a binding");
        let actual_name = actual
            .binding
            .as_ref()
            .expect("named satisfier row has a binding");
        if kind == SignatureContractKind::Ensures && required_name != actual_name {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}` but named ensures lane {} renames public selector `{required_name}` to `{actual_name}`",
                machine.name,
                trait_definition.name,
                requirement.name,
                lane_position,
            )));
            continue;
        }

        let required_facts = program.proof_facts.span_or_empty(required.facts);
        let actual_facts = program.proof_facts.span_or_empty(actual.facts);
        let (
            [ProofFact::Proposition(required_application)],
            [ProofFact::Proposition(actual_application)],
        ) = (required_facts, actual_facts)
        else {
            // Named-contract validation owns the malformed-row diagnostics.
            continue;
        };
        if !required_application.binder_arguments.is_empty()
            || !actual_application.binder_arguments.is_empty()
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}` but named {lane} lane {} uses a generic proposition telescope; this first conformance rung is concrete only",
                machine.name,
                trait_definition.name,
                requirement.name,
                lane_position,
            )));
            continue;
        }
        let argument_labels = program
            .expression_table
            .expression_handles(required_application.arguments)
            .iter()
            .map(|argument| {
                program.render_proof_expression(
                    *argument,
                    ProofSubstitutions::ByParameter(&parameter_substitutions),
                )
            })
            .collect::<Vec<_>>();
        let expected = program.normalize_nominal_proposition_application(
            required_application,
            Some(PropositionLabels {
                binder_labels: &[],
                argument_labels: &argument_labels,
            }),
        );
        let observed = program.normalize_nominal_proposition_application(actual_application, None);
        if expected != observed {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}` but named {lane} lane {} does not retain the requirement's exact proposition and evidence interface",
                machine.name,
                trait_definition.name,
                requirement.name,
                lane_position,
            )));
        }
    }
}

/// Compose a parent edge such as `Forwarded<U>: Sink<U>` with the concrete
/// arguments of the child instance. The returned handles are existing nodes;
/// exact forwarded parameters need no allocation and cover the canonical
/// parent-binding form while preserving concrete/composite arguments as-is.
pub fn compose_forwarded_trait_arguments(
    program: &TypedTrees,
    source_trait: &TraitDefinition,
    source_arguments: &[TypeReferenceHandle],
    parent_arguments: &[TypeReferenceHandle],
) -> Vec<TypeReferenceHandle> {
    let source_parameters = program.trait_type_parameters(source_trait);
    parent_arguments
        .iter()
        .map(|argument| {
            let TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(*argument)
            else {
                return *argument;
            };
            source_parameters
                .iter()
                .zip(source_arguments.iter())
                .find(|(parameter, _)| {
                    (parameter.symbol.is_valid() && parameter.symbol == *symbol)
                        || parameter.name.as_str() == name.as_str()
                })
                .map(|(_, concrete)| *concrete)
                .unwrap_or(*argument)
        })
        .collect()
}
