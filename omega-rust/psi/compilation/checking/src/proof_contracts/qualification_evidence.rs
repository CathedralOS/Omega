use crate::value_custody::places::unwrapped_type_reference;
use crate::value_custody::type_references::type_references_match;
use checked_trees::BoundaryQualificationAuthorization;
use diagnostics::Diagnostic;
use language_semantics::DomainEstablishmentRoute;
use language_semantics::MachineSupplyMode;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::SignatureContractKind;
use typed_trees::signature::StateSignature;
use typed_trees::types::TypeReferenceHandle;

/// An admission obligation is not a body assumption or a direct-call guarantee.
/// The subject retains the exact realization formal, or the normal result.
pub struct BoundaryQualificationObligation {
    pub requirement: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub domain: SymbolHandle,
    pub parameter: SymbolHandle,
}

/// Resolve the same applied requirement for named maps and attached edges.
/// Only authorized membership conclusions enter this refutation-only roster;
/// ordinary contract refinement continues to own all other obligations.
pub fn boundary_provider_qualification_obligations(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Vec<BoundaryQualificationObligation> {
    use typed_trees::trait_definition::AppliedTraitScope;
    let mut obligations = Vec::new();
    let mut append = |application: &AppliedTraitScope<'_>,
                      requirement: &StateSignature,
                      state: &typed_trees::state::State| {
        let definition = application.declaration;
        if !definition.is_boundary {
            return;
        }
        let requirement = match program.applied_callable_signature(application, requirement.symbol)
        {
            Some(applied) => applied,
            None if requirement.type_parameters.is_empty()
                && application.substitutions.is_empty()
                && definition.lifetime_parameters.is_empty() =>
            {
                requirement
            }
            // Signature refinement diagnoses a missing prepared application.
            None => return,
        };
        let Some(actual) =
            crate::inherited_satisfier_parameters(program, definition, requirement, state)
        else {
            return;
        };
        let formal = program.state_signature_parameters(requirement);
        for contract in program.state_signature_contracts(requirement) {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if boundary_qualification_authorization(
                    program,
                    definition.symbol,
                    requirement,
                    contract.kind.clone(),
                    fact,
                )
                .is_none()
                {
                    continue;
                }
                let Some(membership) = fact.as_membership() else {
                    continue;
                };
                if !membership.domain_symbol.is_valid() {
                    continue;
                }
                let parameter = if program
                    .expression_table
                    .names_reserved_result(membership.value)
                {
                    SymbolHandle::invalid()
                } else {
                    let Some(position) = formal.iter().position(|parameter| {
                        program
                            .expression_table
                            .names_parameter(membership.value, parameter)
                    }) else {
                        continue;
                    };
                    let Some(parameter) = actual.get(position) else {
                        continue;
                    };
                    parameter.symbol
                };
                obligations.push(BoundaryQualificationObligation {
                    requirement: requirement.symbol,
                    realization_state: state.symbol,
                    domain: membership.domain_symbol,
                    parameter,
                });
            }
        }
    };
    for conformance in program.conformances() {
        if !conformance
            .rows
            .iter()
            .any(|row| row.realization_machine == machine.symbol)
        {
            continue;
        }
        let Some(root) = AppliedTraitScope::from_conformance(program.typed_program(), conformance)
        else {
            continue;
        };
        for row in conformance
            .rows
            .iter()
            .filter(|row| row.realization_machine == machine.symbol)
        {
            let Some(application) =
                root.declaring_scope(program.typed_program(), row.declaring_trait)
            else {
                continue;
            };
            let Some(requirement) =
                program.trait_machine_signature_by_symbol(application.declaration, row.requirement)
            else {
                continue;
            };
            let Some(state) = program.state_in_machine(machine, row.realization_state) else {
                continue;
            };
            append(&application, requirement, state);
        }
    }
    if let Some(entry) = program.machine_states(machine).first() {
        for conformance in program.machine_trait_conformances(machine) {
            let Some(application) = AppliedTraitScope::from_machine_conformance(
                program.typed_program(),
                machine,
                conformance,
            ) else {
                continue;
            };
            for requirement in program.trait_machine_signatures(application.declaration) {
                if conformance.requirement.as_ref() == Some(&requirement.name) {
                    append(&application, requirement, entry);
                }
            }
        }
    }
    obligations
}

pub(crate) fn validate_qualification_authorization(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_boundary_requirements(program, diagnostics);
    validate_external_machine_claims(program, diagnostics);
}

fn validate_boundary_requirements(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for trait_definition in program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
    {
        for signature in program.trait_machine_signatures(trait_definition) {
            for contract in program
                .state_signature_contracts(signature)
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Ensures)
            {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let Some(membership) = fact.as_membership() else {
                        continue;
                    };
                    if let Some(permission) = crate::proof_contracts::proof_facts::carry_permission(
                        program,
                        membership.domain,
                    ) {
                        if !program
                            .expression_table
                            .names_reserved_result(membership.value)
                            && !expression_is_mutable_parameter(
                                program,
                                signature,
                                membership.value,
                            )
                        {
                            diagnostics.push(Diagnostic::error(format!(
                                "boundary requirement `{}::{}` may admit carry permission `{permission}` only for its exact `result` or an exact mutable parameter",
                                trait_definition.name, signature.name,
                            )));
                        }
                        continue;
                    }
                    if !membership.domain_symbol.is_valid() {
                        continue;
                    }
                    let Some(domain) = program.domain_definition(membership.domain_symbol) else {
                        continue;
                    };

                    let subject = if program
                        .expression_table
                        .names_reserved_result(membership.value)
                    {
                        Some((signature.return_type, "result"))
                    } else {
                        mutable_parameter(program, signature, membership.value)
                            .map(|parameter| (parameter.type_reference, parameter.name.as_str()))
                    };
                    let Some((subject, subject_name)) = subject else {
                        diagnostics.push(Diagnostic::error(format!(
                            "boundary requirement `{}::{}` may admit domain `{}` only for its exact `result` or an exact mutable parameter; move the membership guarantee to `ensures result in {}` or name a mutable parameter",
                            trait_definition.name,
                            signature.name,
                            domain.name,
                            domain.name,
                        )));
                        continue;
                    };

                    let subject_carrier = unwrapped_type_reference(program, subject);
                    let domain_carrier = unwrapped_type_reference(program, domain.target_type);
                    if !matches!(
                        (subject_carrier, domain_carrier),
                        (Some(subject_carrier), Some(domain_carrier))
                            if type_references_match(program, subject_carrier, domain_carrier)
                                || lifetime_erased_nominal_carriers_match(
                                    program,
                                    subject_carrier,
                                    domain_carrier,
                                )
                    ) {
                        diagnostics.push(Diagnostic::error(format!(
                            "boundary requirement `{}::{}` cannot admit `{subject_name} in {}`: {} carrier `{}` does not match domain target `{}`",
                            trait_definition.name,
                            signature.name,
                            domain.name,
                            if language_core::is_reserved_result(&subject_name) {
                                "result"
                            } else {
                                "parameter"
                            },
                            program.display_type_reference_with_constraints(subject),
                            program.display_type_reference_with_constraints(domain.target_type),
                        )));
                    }
                }
            }
        }
    }
}

/// Domain declarations name their nominal carrier without a lifetime
/// telescope. A result may instantiate that same carrier's erased lifetime
/// parameters, but this must not widen ordinary generic matching: runtime type
/// arguments still require exact normalized identity.
fn lifetime_erased_nominal_carriers_match(
    program: &dyn crate::CheckingInput,
    application: typed_trees::types::TypeReferenceHandle,
    nominal: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let typed_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = program.type_reference_table.type_reference(application)
    else {
        return false;
    };
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(nominal)
    else {
        return false;
    };
    *base_symbol == *symbol
        && symbol.is_valid()
        && program
            .type_reference_table
            .type_reference_handles(*arguments)
            .is_empty()
        && crate::declarations::data_queries::data_definition_by_symbol(program, *base_symbol)
            .is_some_and(|definition| {
                !definition.lifetime_parameters.is_empty()
                    && definition.lifetime_parameters.len() == lifetime_arguments.len()
            })
}

fn validate_external_machine_claims(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines().iter().filter(|machine| {
        matches!(
            machine.supply_mode,
            MachineSupplyMode::AdmissionClaim
                | MachineSupplyMode::Requirement
                | MachineSupplyMode::ExternalRealization { .. }
        )
    }) {
        for contract in program
            .machine_contracts(machine)
            .iter()
            .chain(
                program
                    .machine_states(machine)
                    .iter()
                    .flat_map(|state| program.state_contracts(state)),
            )
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(membership) = fact.as_membership() else {
                    continue;
                };
                if let Some(permission) = crate::proof_contracts::proof_facts::carry_permission(
                    program,
                    membership.domain,
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "external machine `{}` cannot directly admit carry permission `{permission}`; publish it on an owner-authorized boundary trait requirement and satisfy that requirement through the provider",
                        machine.name,
                    )));
                    continue;
                }
                if !membership.domain_symbol.is_valid() {
                    continue;
                }
                let domain_name = program
                    .domain_definition(membership.domain_symbol)
                    .map(|domain| domain.name.as_str())
                    .unwrap_or("<unknown>");
                diagnostics.push(Diagnostic::error(format!(
                    "external machine `{}` cannot directly admit domain membership `{}`; publish `ensures result in {}` on an owner-authorized boundary trait requirement and satisfy that requirement through the provider",
                    machine.name, domain_name, domain_name,
                )));
            }
        }
    }
}

/// An `ensures` subject that names an exact mutable non-self parameter: the
/// provider mints the membership into caller storage it borrows mutably, the
/// out-parameter direction of `ensures result in D`. Immutable parameters
/// stay caller premises — nothing may establish into them.
fn expression_is_mutable_parameter(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
    expression: ExpressionHandle,
) -> bool {
    mutable_parameter(program, signature, expression).is_some()
}

fn mutable_parameter<'a>(
    program: &'a dyn crate::CheckingInput,
    signature: &'a typed_trees::signature::StateSignature,
    expression: ExpressionHandle,
) -> Option<&'a typed_trees::signature::StateParameter> {
    program
        .state_signature_parameters(signature)
        .iter()
        .find(|parameter| {
            is_mutable_reference(program, parameter.type_reference)
                && program
                    .expression_table
                    .names_parameter(expression, parameter)
        })
}

/// Caller-visible mutability: only a `&mut` borrow (possibly under
/// constraints) lets the provider write caller storage. An owned `mut`
/// binding dies with the callee, so its ensures cannot mint caller claims.
fn is_mutable_reference(
    program: &dyn crate::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { access, .. } => {
            *access == language_core::ReferenceAccess::Mutable
        }
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            is_mutable_reference(program, *base_type)
        }
        _ => false,
    }
}

/// Classifies an owner-authorized membership at its exact boundary requirement.
/// This does not admit a call, prove domain predicates, or create a body premise.
/// Declaration refinement and occurrence evidence use the same classification;
/// only the selected admitted crossing may originate the resulting authority.
pub fn boundary_qualification_authorization(
    program: &dyn crate::CheckingInput,
    owner_symbol: SymbolHandle,
    signature: &StateSignature,
    contract_kind: SignatureContractKind,
    fact: &typed_trees::domain::ProofFact,
) -> Option<BoundaryQualificationAuthorization> {
    if contract_kind != SignatureContractKind::Ensures
        || !program
            .traits()
            .iter()
            .any(|definition| definition.symbol == owner_symbol && definition.is_boundary)
    {
        return None;
    }

    let membership = fact.as_membership()?;
    let subject_carrier = if program
        .expression_table
        .names_reserved_result(membership.value)
    {
        signature.return_type
    } else {
        mutable_parameter(program, signature, membership.value)?.type_reference
    };
    if crate::proof_contracts::proof_facts::carry_permission(program, membership.domain).is_some() {
        return Some(BoundaryQualificationAuthorization {
            requirement_symbol: owner_symbol,
            signature_symbol: signature.symbol,
        });
    }
    if !membership.domain_symbol.is_valid() {
        return None;
    }
    let domain = program.domain_definition(membership.domain_symbol)?;
    if membership.authored_domain_selection.is_none()
        || membership.domain_arguments.len() != domain.type_parameters.len()
    {
        return None;
    }
    if !domain.establishment_routes.iter().any(|route| {
        matches!(
            route,
            DomainEstablishmentRoute::BoundaryRequirement {
                boundary_trait,
                requirement,
            } if *boundary_trait == owner_symbol && *requirement == signature.symbol
        )
    }) {
        return None;
    }
    if !return_type_matches_domain_target(program, subject_carrier, domain.target_type) {
        return None;
    }

    Some(BoundaryQualificationAuthorization {
        requirement_symbol: owner_symbol,
        signature_symbol: signature.symbol,
    })
}

/// A boundary requirement's return type realizes the domain target. An
/// explicitly lifetime-parameterized carrier spells its result as
/// `PendingRead<'a>` while the domain target is the bare nominal; the lifetime
/// slot is erased from the semantic identity, so both sides compare by head
/// nominal when no runtime generic arguments remain.
fn return_type_matches_domain_target(
    program: &dyn crate::CheckingInput,
    return_type: TypeReferenceHandle,
    domain_target: TypeReferenceHandle,
) -> bool {
    if program
        .unwrapped_type_reference(return_type)
        .zip(program.unwrapped_type_reference(domain_target))
        .is_some_and(|(left, right)| {
            program.normalized_type_identity(left) == program.normalized_type_identity(right)
        })
    {
        return true;
    }
    nominal_head_symbol(program, return_type)
        .zip(nominal_head_symbol(program, domain_target))
        .is_some_and(|(left, right)| left == right)
}

fn nominal_head_symbol(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let unwrapped = program.unwrapped_type_reference(type_reference)?;
    program
        .type_reference_table
        .closed_nominal_symbol(unwrapped)
}
