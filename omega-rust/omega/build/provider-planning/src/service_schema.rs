//! Derive provider service schemas from checked declaration surfaces.

use effects::provider_plan::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceProgressEstablishmentRoute,
    ServiceProgressEstablishmentRouteKind, ServiceProgressPremise, ServiceProgressSubject,
    ServiceResultClaim, ServiceSchema,
};

/// PRV2: reify a typed boundary trait's callable surface. `None` for a
/// non-boundary trait (only boundary traits have service schemas).
pub fn from_typed(
    program: &typed_trees::TypedTrees,
    trait_definition: &typed_trees::trait_definition::TraitDefinition,
) -> Option<ServiceSchema> {
    from_typed_instance(program, trait_definition, &[])
}

/// Reify one concrete generic boundary instance. The argument tuple is
/// semantic input only for resolving evaluated calling-plan identity;
/// policy type/source names remain absent from the published schema.
pub fn from_typed_instance(
    program: &typed_trees::TypedTrees,
    trait_definition: &typed_trees::trait_definition::TraitDefinition,
    boundary_arguments: &[typed_trees::types::TypeReferenceHandle],
) -> Option<ServiceSchema> {
    if !trait_definition.is_boundary {
        return None;
    }
    let mut methods = Vec::new();
    let mut visited = Vec::new();
    collect_service_methods(
        program,
        trait_definition,
        trait_definition.symbol,
        boundary_arguments,
        &mut visited,
        &mut methods,
    );
    Some(ServiceSchema {
        trait_name: trait_definition.name.as_str().to_owned(),
        trait_package_identity: program
            .symbols
            .symbol_package_identity(trait_definition.symbol),
        methods,
    })
}

/// Reify one exact overloaded boundary-operator requirement as a
/// single-row provider slot. `trait_name` is the legacy field name on the
/// shared carrier; operator slots use their stable signature identity so
/// f32/f64 overloads can never collide or be selected as one another.
pub fn from_typed_operator(
    program: &typed_trees::TypedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Option<ServiceSchema> {
    operator.is_boundary.then(|| ServiceSchema {
        trait_name: typed_trees::operator::boundary_operator_requirement_identity(
            program, operator,
        ),
        trait_package_identity: program.symbols.symbol_package_identity(operator.symbol),
        methods: vec![ServiceMethod {
            name: "realize".to_owned(),
            requirement_owner: typed_trees::operator::boundary_operator_requirement_identity(
                program, operator,
            ),
            requirement_owner_package_identity: program
                .symbols
                .symbol_package_identity(operator.symbol),
            requirement_identity: typed_trees::operator::boundary_operator_requirement_identity(
                program, operator,
            ),
            parameter_count: program.operator_parameters(operator).len(),
            parameter_type_identities: program
                .operator_parameters(operator)
                .iter()
                .map(|parameter| {
                    program
                        .normalized_type_identity(parameter.type_reference)
                        .into_string()
                })
                .collect(),
            entry_claims: Vec::new(),
            has_result: operator.return_type.is_valid(),
            result_type_identity: operator.return_type.is_valid().then(|| {
                program
                    .normalized_type_identity(operator.return_type)
                    .into_string()
            }),
            result_claims: Vec::new(),
            service_reach: Vec::new(),
            synchronous_invocations: Vec::new(),
            may_suspend: false,
            may_block: false,
            terminates_guarantee: false,
            termination_premises: Vec::new(),
            calling_plan_report_fingerprint: None,
            calling_plan_commitment: None,
        }],
    })
}

/// Reify one exact explicit top-level boundary requirement as a
/// single-row provider slot. This first planning rung is deliberately
/// closed over a non-generic callable: lifetime and static generic
/// telescopes remain outside provider selection until their application
/// identity has an equally exact carrier.
pub fn from_typed_boundary_requirement(
    program: &typed_trees::TypedTrees,
    requirement: &typed_trees::machine::Machine,
) -> Option<ServiceSchema> {
    if !requirement.symbol.is_valid()
        || !requirement.is_public
        || requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
        || requirement.body_is_present
        || !requirement.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(requirement).is_empty()
    {
        return None;
    }
    let [entry] = program.machine_states(requirement) else {
        return None;
    };
    let (requirement_owner, method_name) = requirement.name.as_str().rsplit_once("::")?;
    if requirement_owner.is_empty() || method_name.is_empty() {
        return None;
    }
    let requirement_identity = program
        .normalized_machine_overload_identity(requirement)?
        .identity();
    let parameters = program.state_parameters(entry);
    let published_termination = requirement.termination_plan.interface.published()?;
    let package_identity = program.symbols.symbol_package_identity(requirement.symbol);

    Some(ServiceSchema {
        trait_name: requirement.name.as_str().to_owned(),
        trait_package_identity: package_identity,
        methods: vec![ServiceMethod {
            name: method_name.to_owned(),
            requirement_owner: requirement_owner.to_owned(),
            requirement_owner_package_identity: package_identity,
            requirement_identity,
            parameter_count: parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count(),
            parameter_type_identities: parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .map(|parameter| {
                    program
                        .normalized_type_identity(parameter.type_reference)
                        .into_string()
                })
                .collect(),
            entry_claims: Vec::new(),
            has_result: entry.return_type.is_valid(),
            result_type_identity: entry.return_type.is_valid().then(|| {
                program
                    .normalized_type_identity(entry.return_type)
                    .into_string()
            }),
            result_claims: Vec::new(),
            service_reach: service_reach_row_names(program, requirement.service_reach_row),
            synchronous_invocations: machine_synchronous_invocation_names(program, requirement),
            may_suspend: requirement.suspends,
            may_block: requirement.blocks,
            terminates_guarantee: published_termination.promises_termination(),
            termination_premises: service_progress_premises_for_parameters(
                program,
                published_termination,
                parameters,
            ),
            calling_plan_report_fingerprint: None,
            calling_plan_commitment: None,
        }],
    })
}

fn collect_service_methods(
    program: &typed_trees::TypedTrees,
    trait_definition: &typed_trees::trait_definition::TraitDefinition,
    policy_owner: symbols::SymbolHandle,
    boundary_arguments: &[typed_trees::types::TypeReferenceHandle],
    visited: &mut Vec<symbols::SymbolHandle>,
    methods: &mut Vec<ServiceMethod>,
) {
    if visited.contains(&trait_definition.symbol) {
        return;
    }
    visited.push(trait_definition.symbol);

    for requirement in program.trait_requirements(trait_definition) {
        let Some(parent) = program
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == requirement.symbol)
        else {
            continue;
        };
        collect_service_methods(
            program,
            parent,
            policy_owner,
            boundary_arguments,
            visited,
            methods,
        );
    }

    for signature in program.trait_machine_signatures(trait_definition) {
        let requirement_identity = program
            .normalized_trait_requirement_overload_identity(trait_definition, signature)
            .identity();
        if methods
            .iter()
            .any(|method| method.requirement_identity == requirement_identity)
        {
            continue;
        }
        let calling_plan = program.boundary_calling_plan_identity_for_arguments(
            policy_owner,
            boundary_arguments,
            signature.symbol,
        );
        methods.push(ServiceMethod {
            name: signature.name.as_str().to_owned(),
            requirement_owner: trait_definition.name.as_str().to_owned(),
            requirement_owner_package_identity: program
                .symbols
                .symbol_package_identity(trait_definition.symbol),
            requirement_identity,
            parameter_count: program
                .state_signature_parameters(signature)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count(),
            parameter_type_identities: program
                .state_signature_parameters(signature)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .map(|parameter| {
                    program
                        .normalized_type_identity(parameter.type_reference)
                        .into_string()
                })
                .collect(),
            entry_claims: service_entry_claims(program, trait_definition, signature),
            has_result: signature.return_type.is_valid(),
            result_type_identity: signature.return_type.is_valid().then(|| {
                program
                    .normalized_type_identity(signature.return_type)
                    .into_string()
            }),
            result_claims: service_result_claims(program, trait_definition, signature),
            service_reach: service_reach_names(program, trait_definition, signature),
            synchronous_invocations: synchronous_invocation_names(program, signature),
            may_suspend: signature.suspends,
            may_block: signature.blocks,
            terminates_guarantee: signature.termination_guarantee.promises_termination(),
            termination_premises: service_progress_premises(program, signature),
            calling_plan_report_fingerprint: calling_plan
                .map(|identity| identity.report_fingerprint),
            calling_plan_commitment: calling_plan.map(|identity| identity.commitment),
        });
    }
}

fn service_progress_premises(
    program: &typed_trees::TypedTrees,
    signature: &typed_trees::signature::StateSignature,
) -> Vec<ServiceProgressPremise> {
    service_progress_premises_for_parameters(
        program,
        &signature.termination_guarantee,
        program.state_signature_parameters(signature),
    )
}

fn service_progress_premises_for_parameters(
    program: &typed_trees::TypedTrees,
    guarantee: &language_semantics::TerminationGuarantee,
    parameters: &[typed_trees::signature::StateParameter],
) -> Vec<ServiceProgressPremise> {
    let language_semantics::TerminationGuarantee::Terminates { premises } = guarantee else {
        return Vec::new();
    };
    premises
        .iter()
        .map(|premise| {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.symbol == premise.subject.root)
                .expect("normalized public progress premise must be parameter-rooted");
            let subject = if parameter.is_self {
                ServiceProgressSubject::ProviderReceiver
            } else {
                ServiceProgressSubject::Parameter(
                    parameters
                        .iter()
                        .filter(|candidate| !candidate.is_self)
                        .position(|candidate| candidate.symbol == parameter.symbol)
                        .expect("non-receiver premise root must be an ordinary parameter"),
                )
            };
            let profile = program
                .semantic_domains
                .name(premise.profile)
                .expect("normalized progress premise must name a registered profile")
                .to_owned();
            ServiceProgressPremise {
                profile,
                subject,
                subject_projections: premise
                    .subject
                    .projections
                    .iter()
                    .map(|symbol| program.symbols.display_path(*symbol, "::"))
                    .collect(),
                establishment_routes: service_progress_establishment_routes(
                    program,
                    premise.profile,
                ),
            }
        })
        .collect()
}

fn service_progress_establishment_routes(
    program: &typed_trees::TypedTrees,
    profile: language_semantics::SemanticDomainId,
) -> Vec<ServiceProgressEstablishmentRoute> {
    let domain = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.semantic_id == profile)
        .expect("normalized progress premise must name one declared profile domain");
    debug_assert_eq!(
        domain.classification,
        Some(language_semantics::DomainClassification::ProgressProfile)
    );
    let mut routes = domain
        .establishment_routes
        .iter()
        .map(|route| {
            let owner = program
                .traits()
                .iter()
                .find(|owner| owner.symbol == route.source_symbol())
                .expect("normalized progress establishment route must name one trait owner");
            let requirement = program
                .trait_machine_signatures(owner)
                .iter()
                .find(|requirement| requirement.symbol == route.requirement_symbol())
                .expect("normalized progress establishment route must name one requirement");
            let kind = match route {
                language_semantics::DomainEstablishmentRoute::CheckedRequirement { .. } => {
                    ServiceProgressEstablishmentRouteKind::CheckedRequirement
                }
                language_semantics::DomainEstablishmentRoute::BoundaryRequirement { .. } => {
                    ServiceProgressEstablishmentRouteKind::BoundaryRequirement
                }
            };
            ServiceProgressEstablishmentRoute {
                kind,
                requirement_identity: program
                    .normalized_trait_requirement_overload_identity(owner, requirement)
                    .identity(),
            }
        })
        .collect::<Vec<_>>();
    routes.sort();
    routes.dedup();
    routes
}

fn service_entry_claims(
    program: &typed_trees::TypedTrees,
    trait_definition: &typed_trees::trait_definition::TraitDefinition,
    signature: &typed_trees::signature::StateSignature,
) -> Vec<ServiceEntryClaim> {
    let mut claims = Vec::new();
    for (parameter_index, parameter) in program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
    {
        if program.type_multiplicity(parameter.type_reference)
            != language_semantics::Multiplicity::Linear
        {
            continue;
        }
        append_routed_entry_claims(
            program,
            parameter.type_reference,
            parameter_index,
            trait_definition.symbol,
            signature.symbol,
            &mut claims,
        );
    }
    claims.sort_by(|left, right| {
        left.parameter_index
            .cmp(&right.parameter_index)
            .then_with(|| left.carrier_identity.cmp(&right.carrier_identity))
            .then_with(|| left.domain.cmp(&right.domain))
    });
    claims.dedup_by(|left, right| {
        left.parameter_index == right.parameter_index
            && left.carrier_identity == right.carrier_identity
            && left.domain == right.domain
    });
    claims
}

fn service_result_claims(
    program: &typed_trees::TypedTrees,
    trait_definition: &typed_trees::trait_definition::TraitDefinition,
    signature: &typed_trees::signature::StateSignature,
) -> Vec<ServiceResultClaim> {
    if !signature.return_type.is_valid()
        || program.type_multiplicity(signature.return_type)
            != language_semantics::Multiplicity::Linear
    {
        return Vec::new();
    }
    let mut claims = Vec::new();
    append_bodyless_result_claims(
        program,
        signature.return_type,
        trait_definition.symbol,
        signature.symbol,
        &mut claims,
    );
    claims.sort_by(|left, right| left.domain.cmp(&right.domain));
    claims.dedup_by(|left, right| left.domain == right.domain);
    claims
}

fn append_bodyless_result_claims(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    boundary_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
    claims: &mut Vec<ServiceResultClaim>,
) {
    use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            append_bodyless_result_claims(program, *referee, boundary_trait, requirement, claims);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            append_bodyless_result_claims(program, *base_type, boundary_trait, requirement, claims);
            for constraint in program.type_reference_table.constraints(*constraints) {
                let TypeConstraintNode::Domain(domain) = constraint else {
                    continue;
                };
                if domain.symbol.is_valid()
                    && domain.predicate_body == language_semantics::DomainPredicateBody::Bodyless
                    && domain.establishment_routes.iter().any(|route| {
                        matches!(
                            route,
                            language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                                boundary_trait: route_trait,
                                requirement: route_requirement,
                            } if *route_trait == boundary_trait && *route_requirement == requirement
                        )
                    })
                {
                    claims.push(ServiceResultClaim {
                        domain: domain
                            .semantic_id
                            .is_valid()
                            .then(|| program.semantic_domains.name(domain.semantic_id))
                            .flatten()
                            .unwrap_or_else(|| domain.name.as_str())
                            .to_owned(),
                        effective_carry: language_semantics::CarryPolicy::STRICT,
                    });
                }
            }
        }
        _ => {}
    }
}

fn append_routed_entry_claims(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    parameter_index: usize,
    boundary_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
    claims: &mut Vec<ServiceEntryClaim>,
) {
    use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            append_routed_entry_claims(
                program,
                *referee,
                parameter_index,
                boundary_trait,
                requirement,
                claims,
            );
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            append_routed_entry_claims(
                program,
                *base_type,
                parameter_index,
                boundary_trait,
                requirement,
                claims,
            );
            for constraint in program.type_reference_table.constraints(*constraints) {
                let TypeConstraintNode::Domain(domain) = constraint else {
                    continue;
                };
                if domain.symbol.is_valid()
                    && domain.establishment_routes.iter().any(|route| {
                        matches!(
                            route,
                            language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                                boundary_trait: route_trait,
                                requirement: route_requirement,
                            } if *route_trait == boundary_trait && *route_requirement == requirement
                        )
                    })
                {
                    claims.push(ServiceEntryClaim {
                        parameter_index,
                        carrier_identity: program
                            .normalized_type_identity(*base_type)
                            .into_string(),
                        domain: domain
                            .semantic_id
                            .is_valid()
                            .then(|| program.semantic_domains.name(domain.semantic_id))
                            .flatten()
                            .unwrap_or_else(|| domain.name.as_str())
                            .to_owned(),
                        predicate_body: domain.predicate_body,
                        effective_carry: language_semantics::CarryPolicy::STRICT,
                        authority_flow: ServiceEntryAuthorityFlow::Accepts,
                    });
                }
            }
        }
        _ => {}
    }
}

fn service_reach_names(
    program: &typed_trees::TypedTrees,
    trait_definition: &typed_trees::trait_definition::TraitDefinition,
    signature: &typed_trees::signature::StateSignature,
) -> Vec<String> {
    let mut services = program
        .service_reach_rows
        .services(signature.service_reach_row)
        .to_vec();
    if trait_definition.is_boundary
        && let Some(service) = program
            .service_reaches
            .id_for_symbol(trait_definition.symbol)
    {
        program
            .service_reaches
            .extend_closure(service, &mut services);
    }
    services.sort_by_key(|service| service.0);
    services.dedup();
    let mut names = services
        .into_iter()
        .filter_map(|service| program.service_reaches.definition(service))
        .map(|definition| definition.name.clone())
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    names
}

fn service_reach_row_names(
    program: &typed_trees::TypedTrees,
    row: language_semantics::ServiceReachRowId,
) -> Vec<String> {
    let mut names = program
        .service_reach_rows
        .services(row)
        .iter()
        .filter_map(|service| program.service_reaches.definition(*service))
        .map(|definition| definition.name.clone())
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    names
}

fn synchronous_invocation_names(
    program: &typed_trees::TypedTrees,
    signature: &typed_trees::signature::StateSignature,
) -> Vec<String> {
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let mut names = validation::declared_signature_invocations(program, signature)
        .into_iter()
        .filter_map(|target| match target {
            flow_effects::InvocationTarget::Parameter(index) => parameters
                .get(index as usize)
                .map(|parameter| parameter.type_reference)
                .and_then(|type_reference| boundary_trait_name_for_type(program, type_reference)),
            flow_effects::InvocationTarget::Service(symbol) => program
                .traits()
                .iter()
                .find(|definition| definition.is_boundary && definition.symbol == symbol)
                .map(|definition| definition.name.as_str().to_owned()),
        })
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    names
}

fn machine_synchronous_invocation_names(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Vec<String> {
    let Some(entry) = program.machine_states(machine).first() else {
        return Vec::new();
    };
    let parameters = program
        .state_parameters(entry)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let mut names = validation::declared_machine_invocations(program, machine)
        .into_iter()
        .filter_map(|target| match target {
            flow_effects::InvocationTarget::Parameter(index) => parameters
                .get(index as usize)
                .map(|parameter| parameter.type_reference)
                .and_then(|type_reference| boundary_trait_name_for_type(program, type_reference)),
            flow_effects::InvocationTarget::Service(symbol) => program
                .traits()
                .iter()
                .find(|definition| definition.is_boundary && definition.symbol == symbol)
                .map(|definition| definition.name.as_str().to_owned()),
        })
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    names
}

fn boundary_trait_name_for_type(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<String> {
    let symbol = program
        .type_reference_table
        .type_reference(type_reference)
        .type_symbol(&program.type_reference_table);
    program
        .traits()
        .iter()
        .find(|definition| definition.is_boundary && definition.symbol == symbol)
        .map(|definition| definition.name.as_str().to_owned())
}
