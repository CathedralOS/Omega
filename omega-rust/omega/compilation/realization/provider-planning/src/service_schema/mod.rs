//! Derive provider service schemas from checked declaration surfaces.
//!
//! The caller selects a Build or product declaration context. The assembled
//! tree may retain both checked copies of a dependency; equal package and
//! signature identities do not authorize substituting one for the other.
//! Portable requirement names are complete module paths projected from the
//! exact symbol, paired with package identity. An authored leaf is not unique
//! even inside one package; replay must use the same projection as derivation.

use crate::planning_context::ProviderPlanningContext;
use effects::provider_plan::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceProgressEstablishmentRoute,
    ServiceProgressEstablishmentRouteKind, ServiceProgressPremise, ServiceProgressSubject,
    ServiceResultClaim, ServiceSchema,
};

/// Whether retained declaration provenance belongs to the product frontier.
/// Build copies cannot supply final product plans even when package and names
/// match. Source-free declarations retain their explicitly unscoped behavior.
pub fn is_product_declaration(
    program: &typed_trees::TypedTrees,
    symbol: symbols::SymbolHandle,
) -> bool {
    ProviderPlanningContext::new(program, source::DependencyScope::Product).contains(symbol)
}

/// PRV2: reify a typed boundary trait's callable surface. `None` for a
/// non-boundary trait (only boundary traits have service schemas).
pub fn from_typed(
    context: ProviderPlanningContext<'_>,
    trait_definition: &typed_trees::trait_definition::TraitDefinition,
) -> Option<ServiceSchema> {
    from_typed_instance(context, trait_definition, &[])
}

/// Reify one concrete generic boundary instance. The argument tuple is
/// semantic input only for resolving evaluated calling-plan identity;
/// policy type/source names remain absent from the published schema.
pub fn from_typed_instance(
    context: ProviderPlanningContext<'_>,
    trait_definition: &typed_trees::trait_definition::TraitDefinition,
    boundary_arguments: &[typed_trees::types::TypeReferenceHandle],
) -> Option<ServiceSchema> {
    let program = context.typed;
    if !trait_definition.is_boundary || !context.contains(trait_definition.symbol) {
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
        trait_name: program.trait_declaration_path(trait_definition),
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
    context: ProviderPlanningContext<'_>,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Option<ServiceSchema> {
    let program = context.typed;
    if !context.contains(operator.symbol) {
        return None;
    }
    operator.is_boundary.then(|| ServiceSchema {
        trait_name: typed_trees::operator::boundary_operator_requirement_identity(
            program, operator,
        ),
        trait_package_identity: program.symbols.symbol_package_identity(operator.symbol),
        methods: vec![ServiceMethod {
            completion: validation::boundary_requirement_completion(program, operator.symbol),
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

/// Whether a service schema binds exactly this boundary-operator coordinate.
///
/// [`typed_trees::operator::boundary_operator_requirement_identity`] is
/// deliberately package-blind: two packages may declare same-spelled,
/// same-signature boundary operators whose canonical identities are equal.
/// Every replay join must therefore compare the retained package identity
/// too, or a foreign declaration could substitute for the selected
/// coordinate.
pub fn schema_binds_exact_boundary_operator(
    context: ProviderPlanningContext<'_>,
    schema: &ServiceSchema,
    operator: &typed_trees::operator::OperatorDefinition,
) -> bool {
    let program = context.typed;
    context.contains(operator.symbol)
        && operator.is_boundary
        && typed_trees::operator::boundary_operator_requirement_identity(program, operator)
            == schema.trait_name
        && program.symbols.symbol_package_identity(operator.symbol) == schema.trait_package_identity
}

/// Whether a service schema binds exactly this boundary trait declaration.
/// Trait names share the same package-blind replay hazard as operator
/// requirement identities.
pub fn schema_binds_exact_boundary_trait(
    context: ProviderPlanningContext<'_>,
    schema: &ServiceSchema,
    trait_definition: &typed_trees::trait_definition::TraitDefinition,
) -> bool {
    let program = context.typed;
    context.contains(trait_definition.symbol)
        && trait_definition.is_boundary
        && program.trait_declaration_path(trait_definition) == schema.trait_name
        && program
            .symbols
            .symbol_package_identity(trait_definition.symbol)
            == schema.trait_package_identity
}

/// Whether a service schema binds exactly this top-level boundary
/// requirement machine.
pub fn schema_binds_exact_boundary_requirement(
    context: ProviderPlanningContext<'_>,
    schema: &ServiceSchema,
    requirement: &typed_trees::machine::Machine,
) -> bool {
    let program = context.typed;
    context.contains(requirement.symbol)
        && requirement.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        && program.symbols.symbol_package_identity(requirement.symbol)
            == schema.trait_package_identity
        && from_typed_boundary_requirement(context, requirement)
            .is_some_and(|candidate| candidate.trait_name == schema.trait_name)
}

/// Reify one exact explicit top-level boundary requirement as a
/// single-row provider slot. This first planning rung admits an erased
/// lifetime telescope (`Owner::name<'a>(...)`): the conformance edge
/// carries no lifetime arguments because the realizing machine's own
/// telescope is the identity application, and the erased names never
/// enter the schema's canonical identity axes. A generic token-machine
/// template projects its authored schema — the normalized identity pins
/// the telescope exactly, and the selected-requirement specialization
/// roster is the exact carrier joining that template to each closed
/// application identity Terminal Psi mints.
pub fn from_typed_boundary_requirement(
    context: ProviderPlanningContext<'_>,
    requirement: &typed_trees::machine::Machine,
) -> Option<ServiceSchema> {
    let program = context.typed;
    if !context.contains(requirement.symbol)
        || !requirement.symbol.is_valid()
        || requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
        || requirement.body_is_present
    {
        return None;
    }
    let [entry] = program.machine_states(requirement) else {
        return None;
    };
    let requirement_path = program.symbols.display_path(requirement.symbol, "::");
    let (requirement_owner, method_name) = requirement_path.rsplit_once("::")?;
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
        // A token family contains separate overloaded requirement slots. Its
        // Build selection covers all coordinates atomically, but each row
        // installs one exact callable signature, not whichever overload shares
        // the path. Untokened requirements retain their ordinary named slot.
        trait_name: if requirement.spelling.is_some() {
            requirement_identity.clone()
        } else {
            requirement_path.clone()
        },
        trait_package_identity: package_identity,
        methods: vec![ServiceMethod {
            completion: validation::boundary_requirement_completion(program, requirement.symbol),
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
            completion: validation::boundary_requirement_completion(program, signature.symbol),
            name: signature.name.as_str().to_owned(),
            requirement_owner: program.trait_declaration_path(trait_definition),
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
        // Progress profiles project only requirement routes; exact-machine
        // routes have no progress-projection representation yet and are
        // ineligible for admitted boundary progress profiles.
        .filter(|route| {
            !matches!(
                route,
                language_semantics::DomainEstablishmentRoute::ExactMachine { .. }
            )
        })
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
                language_semantics::DomainEstablishmentRoute::ExactMachine { .. } => {
                    unreachable!("exact-machine routes are filtered above")
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
                .map(|definition| program.trait_declaration_path(definition)),
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
                .map(|definition| program.trait_declaration_path(definition)),
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
        .map(|definition| program.trait_declaration_path(definition))
}

#[cfg(test)]
mod tests {
    use crate::evaluated_via_bindings::evaluate_via_bindings;
    use crate::planning_context::ProviderPlanningContext;
    use crate::provider_planning::{
        DerivedProviderPlan, ProviderPlanDerivation, ProviderPlanProvenance,
        derive_satisfies_plans, validate_derived_provider_plan_candidates,
    };
    use semantic_vocabulary::PackageKeyIdentity;
    use source::{DependencyScope, SourceMap, SourceOrigin, SourceResolutionStratum};
    use std::path::PathBuf;
    use std::sync::Arc;
    use symbols::SymbolKind;
    use typed_trees::TypedTrees;

    fn scoped_provider_copies() -> TypedTrees {
        let text = r#"
            boundary trait Surface {
                machine invoke();
            }
            data Provider {}
            machine Provider::invoke() satisfies Surface::invoke {}
        "#;
        let package = PackageKeyIdentity::from_digest([7; 32]).expect("nonzero package identity");
        let mut sources = SourceMap::default();
        let mut syntax = syntax_trees::SyntaxTrees::default();
        let tokens = source_files_to_tokens::Lexer::new(text)
            .tokenize()
            .expect("tokenize provider fixture");
        for scope in [DependencyScope::Build, DependencyScope::Product] {
            let source_id = sources
                .add_checked_instance(
                    PathBuf::from("package/shared.omg"),
                    text.to_owned(),
                    PathBuf::from("package"),
                    Some(package),
                    SourceOrigin::User,
                    SourceResolutionStratum::Base,
                    scope,
                )
                .source_id;
            let parsed = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
                .expect("parse scoped provider fixture");
            syntax.extend_from(&parsed);
        }
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax,
                sources: Some(Arc::new(sources)),
                top_level_bindings: Vec::new(),
            },
        )
        .expect("resolve both provider contexts independently");
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type both provider contexts")
    }

    #[test]
    fn derived_provider_replay_rejects_other_scope_provenance_with_identical_portable_plan() {
        let typed = scoped_provider_copies();
        let contexts = [DependencyScope::Build, DependencyScope::Product]
            .map(|scope| ProviderPlanningContext::new(&typed, scope));
        let plans = contexts.map(|context| {
            let derived =
                derive_satisfies_plans(ProviderPlanDerivation::unevaluated(context, None));
            let [plan] = derived.as_slice() else {
                panic!("one provider plan per scope, got {derived:?}");
            };
            assert_eq!(plan.plan.rows.len(), 1);
            plan.clone()
        });
        assert_eq!(
            plans[0].plan, plans[1].plan,
            "scope is not portable plan identity"
        );
        assert_ne!(plans[0].provenance, plans[1].provenance);

        for (context, plan, other) in [
            (contexts[0], &plans[0], &plans[1]),
            (contexts[1], &plans[1], &plans[0]),
        ] {
            let bindings = evaluate_via_bindings(context, None, None)
                .expect("checked adapters need no via evaluation");
            let diagnostics = validate_derived_provider_plan_candidates(
                context,
                &bindings,
                std::slice::from_ref(plan),
            );
            assert!(diagnostics.is_empty(), "{diagnostics:?}");

            // Keep the semantic plan and table fixed. Each retained coordinate
            // must reject its equal-looking declaration from the other purpose.
            for (role, provenance) in [
                (
                    "schema",
                    ProviderPlanProvenance {
                        schema: other.provenance.schema,
                        ..plan.provenance.clone()
                    },
                ),
                (
                    "provider",
                    ProviderPlanProvenance {
                        provider_type: other.provenance.provider_type,
                        ..plan.provenance.clone()
                    },
                ),
                (
                    "requirement",
                    ProviderPlanProvenance {
                        row_requirements: other.provenance.row_requirements.clone(),
                        ..plan.provenance.clone()
                    },
                ),
                (
                    "realization",
                    ProviderPlanProvenance {
                        row_realizations: other.provenance.row_realizations.clone(),
                        ..plan.provenance.clone()
                    },
                ),
            ] {
                let substituted = DerivedProviderPlan {
                    plan: plan.plan.clone(),
                    provenance,
                };
                let diagnostics =
                    validate_derived_provider_plan_candidates(context, &bindings, &[substituted]);
                assert!(
                    diagnostics.iter().any(|diagnostic| {
                        diagnostic.message.contains(&format!("retained {role}"))
                            && diagnostic.message.contains("planning context")
                    }),
                    "other-scope {role} must reject in {:?}: {diagnostics:?}",
                    context.scope,
                );
            }
        }
    }

    #[test]
    fn empty_evaluated_binding_table_retains_its_scope_through_derivation_and_replay() {
        let typed = scoped_provider_copies();
        let target = target::TargetProfile::WindowsX64;
        for (scope, other_scope) in [
            (DependencyScope::Build, DependencyScope::Product),
            (DependencyScope::Product, DependencyScope::Build),
        ] {
            let context = ProviderPlanningContext::new(&typed, scope);
            let other_context = ProviderPlanningContext::new(&typed, other_scope);
            let bindings = evaluate_via_bindings(context, Some(target), None)
                .expect("evaluate empty binding table");
            assert!(bindings.rows().is_empty());
            assert_eq!(bindings.scope(), scope);
            assert_eq!(bindings.target(), Some(target));
            let derivation = ProviderPlanDerivation::evaluated(
                context,
                Some(target.target_name()),
                &bindings,
                &[],
            )
            .expect("same-scope empty table admits derivation");
            let plans = derive_satisfies_plans(derivation);
            assert_eq!(plans.len(), 1);
            let diagnostics = validate_derived_provider_plan_candidates(context, &bindings, &plans);
            assert!(diagnostics.is_empty(), "{diagnostics:?}");

            let diagnostics = bindings
                .validate_against_typed(other_context)
                .expect_err("an empty table still belongs to its original scope");
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains("different dependency purpose")
                }),
                "{diagnostics:?}"
            );
            assert!(
                ProviderPlanDerivation::evaluated(
                    other_context,
                    Some(target.target_name()),
                    &bindings,
                    &[],
                )
                .is_err()
            );
            // No candidate rows can cause this rejection: the table's own
            // retained purpose must survive even when both populations are empty.
            let diagnostics =
                validate_derived_provider_plan_candidates(other_context, &bindings, &[]);
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains("different dependency purpose")
                }),
                "{diagnostics:?}"
            );
        }
    }

    #[test]
    fn declaration_context_follows_generated_provenance_and_preserves_source_free_behavior() {
        let mut typed = scoped_provider_copies();
        let [build, product] = [DependencyScope::Build, DependencyScope::Product].map(|scope| {
            let context = ProviderPlanningContext::new(&typed, scope);
            typed
                .traits()
                .iter()
                .find(|definition| context.contains(definition.symbol))
                .expect("one trait per checked purpose")
                .symbol
        });
        let generated_build = typed.symbols.insert_generated_root_from(
            build,
            SymbolKind::Trait,
            "GeneratedBuildService",
        );
        let generated_product = typed.symbols.insert_generated_root_from(
            product,
            SymbolKind::Trait,
            "GeneratedProductService",
        );
        let build_context = ProviderPlanningContext::new(&typed, DependencyScope::Build);
        let product_context = ProviderPlanningContext::new(&typed, DependencyScope::Product);

        assert!(build_context.contains(generated_build));
        assert!(!build_context.contains(generated_product));
        assert!(product_context.contains(generated_product));
        assert!(!product_context.contains(generated_build));
        assert!(
            typed
                .symbols
                .symbol_provenance_source_span(typed.symbols.root())
                .is_none()
        );
        assert!(build_context.contains(typed.symbols.root()));
        assert!(product_context.contains(typed.symbols.root()));
    }
}
