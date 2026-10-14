//! Derive provider service schemas from checked declaration surfaces.
//!
//! The caller selects a Build or product declaration context. The assembled
//! tree may retain both checked copies of a dependency; equal package and
//! signature identities do not authorize substituting one for the other.
//! Portable requirement names are complete module paths projected from the
//! exact symbol, paired with package identity. An authored leaf is not unique
//! even inside one package; replay must use the same projection as derivation.

mod applied_receivers;

use crate::ProviderPlanningContext;
use crate::calling_policy_plans::BoundaryCallingPlanRealization;
use checked_trees::typed_trees::trait_definition::{
    AppliedTraitScope, AppliedTypeReference, TraitApplicationArguments,
};
use effects::provider_plan::{
    ProviderIdentityTable, ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod,
    ServiceProgressEstablishmentRoute, ServiceProgressEstablishmentRouteKind,
    ServiceProgressPremise, ServiceProgressSubject, ServiceResultClaim, ServiceSchema,
};

/// Whether retained declaration provenance belongs to the product frontier.
/// Build copies cannot supply final product plans even when package and names
/// match. Source-free declarations retain their explicitly unscoped behavior.
pub fn is_product_declaration(
    program: &checked_trees::typed_trees::TypedTrees,
    symbol: symbols::SymbolHandle,
) -> bool {
    ProviderPlanningContext::new(program, source::DependencyScope::Product).contains(symbol)
}

/// PRV2: reify a typed boundary trait's callable surface. `None` for a
/// non-boundary trait (only boundary traits have service schemas).
/// Calling applications come from the requested realization; source trees
/// never contain target ABI identities. Build-only schema derivation supplies
/// no native applications.
pub fn from_typed(
    context: ProviderPlanningContext<'_>,
    calling_plans: &[BoundaryCallingPlanRealization],
    trait_definition: &checked_trees::typed_trees::trait_definition::TraitDefinition,
) -> Option<ServiceSchema> {
    from_typed_instance(context, calling_plans, trait_definition, &[])
}

/// Reify one concrete boundary application. Canonical argument identities pin
/// its receiver views, including phantom arguments; source spellings do not.
/// Evaluated calling-plan commitments remain separate realization evidence.
pub fn from_typed_instance(
    context: ProviderPlanningContext<'_>,
    calling_plans: &[BoundaryCallingPlanRealization],
    trait_definition: &checked_trees::typed_trees::trait_definition::TraitDefinition,
    boundary_arguments: &[checked_trees::typed_trees::types::TypeReferenceHandle],
) -> Option<ServiceSchema> {
    let program = context.typed;
    if !trait_definition.is_boundary || !context.contains(trait_definition.symbol) {
        return None;
    }
    let root = if boundary_arguments.is_empty() {
        AppliedTraitScope::abstract_scope(trait_definition)
    } else {
        AppliedTraitScope::new(
            program,
            trait_definition,
            TraitApplicationArguments {
                // This legacy schema entry admits only region-free receivers.
                // An occurrence-owned applied view must precede region-bearing selection.
                lexical_owner: symbols::SymbolHandle::invalid(),
                enclosing_data_applications: &[],
                arguments: boundary_arguments,
                lifetimes: &[],
            },
        )?
    };
    from_typed_application(context, calling_plans, root, boundary_arguments)
}

/// Reify an occurrence-owned application without discarding its simultaneous
/// type and region scopes. Build still chooses a declaration; this query keeps
/// the application that source checking supplied, and grants no selection.
pub fn from_typed_application(
    context: ProviderPlanningContext<'_>,
    calling_plans: &[BoundaryCallingPlanRealization],
    root: AppliedTraitScope<'_>,
    boundary_arguments: &[checked_trees::typed_trees::types::TypeReferenceHandle],
) -> Option<ServiceSchema> {
    let program = context.typed;
    let trait_definition = root.declaration;
    if !trait_definition.is_boundary || !context.contains(trait_definition.symbol) {
        return None;
    }
    let mut methods = Vec::new();
    let mut identities = ProviderIdentityTable::default();
    let scopes = applied_receivers::collect_scopes(program, root)?;
    for scope in scopes.iter().rev() {
        collect_service_methods(
            program,
            calling_plans,
            scope,
            trait_definition.symbol,
            boundary_arguments,
            &mut methods,
            &mut identities,
        );
    }
    let receiver_applications =
        applied_receivers::receiver_applications(program, &scopes, &mut identities)?;
    Some(ServiceSchema {
        trait_name: identities.intern(&program.trait_declaration_path(trait_definition)),
        trait_package_identity: program
            .symbols
            .symbol_package_identity(trait_definition.symbol),
        identities,
        receiver_applications,
        methods,
    })
}

/// Reify one exact overloaded boundary-operator requirement as a
/// single-row provider slot. `trait_name` is the legacy field name on the
/// shared carrier; operator slots use their stable signature identity so
/// f32/f64 overloads can never collide or be selected as one another.
pub fn from_typed_operator(
    context: ProviderPlanningContext<'_>,
    operator: &checked_trees::typed_trees::operator::OperatorDefinition,
) -> Option<ServiceSchema> {
    let program = context.typed;
    if !context.contains(operator.symbol) || !operator.is_boundary {
        return None;
    }
    let mut identities = ProviderIdentityTable::default();
    let requirement_identity = identities.intern(
        &checked_trees::typed_trees::operator::boundary_operator_requirement_identity(
            program, operator,
        ),
    );
    Some(ServiceSchema {
        trait_name: requirement_identity,
        receiver_applications: Vec::new(),
        trait_package_identity: program.symbols.symbol_package_identity(operator.symbol),
        methods: vec![ServiceMethod {
            completion:
                checked_trees::typed_trees::boundary_completions::boundary_requirement_completion(
                    program,
                    operator.symbol,
                ),
            name: "realize".to_owned(),
            requirement_owner:
                checked_trees::typed_trees::operator::boundary_operator_requirement_identity(
                    program, operator,
                ),
            requirement_owner_package_identity: program
                .symbols
                .symbol_package_identity(operator.symbol),
            requirement_identity,
            parameter_count: program.operator_parameters(operator).len(),
            parameter_type_identities: program
                .operator_parameters(operator)
                .iter()
                .map(|parameter| {
                    identities
                        .intern(&program.normalized_type_identity_text(parameter.type_reference))
                })
                .collect(),
            entry_claims: Vec::new(),
            has_result: operator.return_type.is_valid(),
            result_type_identity: operator
                .return_type
                .is_valid()
                .then(|| {
                    identities.intern(&program.normalized_type_identity_text(operator.return_type))
                })
                .unwrap_or_default(),
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
        identities,
    })
}

/// Whether a service schema binds exactly this boundary-operator coordinate.
///
/// [`checked_trees::typed_trees::operator::boundary_operator_requirement_identity`] is
/// deliberately package-blind: two packages may declare same-spelled,
/// same-signature boundary operators whose canonical identities are equal.
/// Every replay join must therefore compare the retained package identity
/// too, or a foreign declaration could substitute for the selected
/// coordinate.
pub fn schema_binds_exact_boundary_operator(
    context: ProviderPlanningContext<'_>,
    schema: &ServiceSchema,
    operator: &checked_trees::typed_trees::operator::OperatorDefinition,
) -> bool {
    let program = context.typed;
    context.contains(operator.symbol)
        && operator.is_boundary
        && checked_trees::typed_trees::operator::boundary_operator_requirement_identity(
            program, operator,
        ) == schema.identity_text(schema.trait_name)
        && program.symbols.symbol_package_identity(operator.symbol) == schema.trait_package_identity
}

/// Whether a service schema binds exactly this boundary trait declaration.
/// Trait names share the same package-blind replay hazard as operator
/// requirement identities.
pub fn schema_binds_exact_boundary_trait(
    context: ProviderPlanningContext<'_>,
    schema: &ServiceSchema,
    trait_definition: &checked_trees::typed_trees::trait_definition::TraitDefinition,
) -> bool {
    let program = context.typed;
    context.contains(trait_definition.symbol)
        && trait_definition.is_boundary
        && program.trait_declaration_path(trait_definition)
            == schema.identity_text(schema.trait_name)
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
    requirement: &checked_trees::typed_trees::machine::Machine,
) -> bool {
    let program = context.typed;
    context.contains(requirement.symbol)
        && requirement.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        && program.symbols.symbol_package_identity(requirement.symbol)
            == schema.trait_package_identity
        && from_typed_boundary_requirement(context, requirement).is_some_and(|candidate| {
            // Ids only carry meaning inside the table that minted them;
            // the candidate interns in its own table, so compare the
            // resolved text, never the raw ids.
            candidate.identity_text(candidate.trait_name) == schema.identity_text(schema.trait_name)
        })
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
    requirement: &checked_trees::typed_trees::machine::Machine,
) -> Option<ServiceSchema> {
    if !context.contains(requirement.symbol)
        || !requirement.symbol.is_valid()
        || requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
        || requirement.body_is_present
    {
        return None;
    }
    project_boundary_machine_schema(context, requirement)
}

/// A generated scalar accessor has its own compiler-catalog declaration role.
/// It does not make ordinary bodyless boundary machines intrinsic candidates.
pub fn from_sealed_placed_accessor(
    context: ProviderPlanningContext<'_>,
    accessor: &crate::SealedPlacedAccessor,
) -> Result<ServiceSchema, diagnostics::Diagnostic> {
    if crate::SealedPlacedAccessor::authenticate(context, accessor.machine())?.as_ref()
        != Some(accessor)
    {
        return Err(diagnostics::Diagnostic::error(
            "sealed placed accessor schema substituted its authenticated declaration",
        ));
    }
    let machine = context.typed.machine(accessor.machine()).ok_or_else(|| {
        diagnostics::Diagnostic::error("sealed placed accessor schema lost its machine")
    })?;
    project_boundary_machine_schema(context, machine).ok_or_else(|| {
        diagnostics::Diagnostic::error(
            "sealed placed accessor has no complete published callable schema",
        )
    })
}

fn project_boundary_machine_schema(
    context: ProviderPlanningContext<'_>,
    requirement: &checked_trees::typed_trees::machine::Machine,
) -> Option<ServiceSchema> {
    let program = context.typed;
    let [entry] = program.machine_states(requirement) else {
        return None;
    };
    let requirement_path = program.symbols.display_path(requirement.symbol, "::");
    let (requirement_owner, method_name) = requirement_path.rsplit_once("::")?;
    if requirement_owner.is_empty() || method_name.is_empty() {
        return None;
    }
    let requirement_identity = program.machine_callable_identity_text(requirement)?;
    let parameters = program.state_parameters(entry);
    let published_termination = requirement.termination_plan.interface.published()?;
    let package_identity = program.symbols.symbol_package_identity(requirement.symbol);
    let mut identities = ProviderIdentityTable::default();
    let requirement_identity_id = identities.intern(&requirement_identity);
    let termination_premises = service_progress_premises_for_parameters(
        program,
        published_termination,
        parameters,
        &mut identities,
    );
    Some(ServiceSchema {
        // A token family contains separate overloaded requirement slots. Its
        // Build selection covers all coordinates atomically, but each row
        // installs one exact callable signature, not whichever overload shares
        // the path. Untokened requirements retain their ordinary named slot.
        trait_name: if requirement.spelling.is_some() {
            requirement_identity_id
        } else {
            identities.intern(&requirement_path)
        },
        trait_package_identity: package_identity,
        receiver_applications: Vec::new(),
        methods: vec![ServiceMethod {
            completion:
                checked_trees::typed_trees::boundary_completions::boundary_requirement_completion(
                    program,
                    requirement.symbol,
                ),
            name: method_name.to_owned(),
            requirement_owner: requirement_owner.to_owned(),
            requirement_owner_package_identity: package_identity,
            requirement_identity: requirement_identity_id,
            parameter_count: parameters.iter().count(),
            parameter_type_identities: parameters
                .iter()
                .map(|parameter| {
                    identities
                        .intern(&program.normalized_type_identity_text(parameter.type_reference))
                })
                .collect(),
            entry_claims: Vec::new(),
            has_result: entry.return_type.is_valid(),
            result_type_identity: entry
                .return_type
                .is_valid()
                .then(|| {
                    identities.intern(&program.normalized_type_identity_text(entry.return_type))
                })
                .unwrap_or_default(),
            result_claims: Vec::new(),
            service_reach: service_reach_row_names(program, requirement.service_reach_row),
            synchronous_invocations: machine_synchronous_invocation_names(program, requirement),
            may_suspend: requirement.suspends,
            may_block: requirement.blocks,
            terminates_guarantee: published_termination.promises_termination(),
            termination_premises,
            calling_plan_report_fingerprint: None,
            calling_plan_commitment: None,
        }],
        identities,
    })
}

fn collect_service_methods(
    program: &checked_trees::typed_trees::TypedTrees,
    calling_plans: &[BoundaryCallingPlanRealization],
    scope: &AppliedTraitScope<'_>,
    policy_owner: symbols::SymbolHandle,
    boundary_arguments: &[checked_trees::typed_trees::types::TypeReferenceHandle],
    methods: &mut Vec<ServiceMethod>,
    identities: &mut ProviderIdentityTable,
) {
    let trait_definition = scope.declaration;
    for signature in program.trait_machine_signatures(trait_definition) {
        let requirement_identity = identities
            .intern(&program.trait_requirement_callable_identity_text(trait_definition, signature));
        if methods
            .iter()
            .any(|method| method.requirement_identity == requirement_identity)
        {
            continue;
        }
        let calling_plan = calling_plans.iter().find(|application| {
            application.boundary_trait == policy_owner
                && application.boundary_arguments == boundary_arguments
                && application.requirement_machine == signature.symbol
        });
        methods.push(ServiceMethod {
            completion:
                checked_trees::typed_trees::boundary_completions::boundary_requirement_completion(
                    program,
                    signature.symbol,
                ),
            name: signature.name.as_str().to_owned(),
            requirement_owner: program.trait_declaration_path(trait_definition),
            requirement_owner_package_identity: program
                .symbols
                .symbol_package_identity(trait_definition.symbol),
            requirement_identity,
            parameter_count: program.state_signature_parameters(signature).iter().count(),
            parameter_type_identities: program
                .state_signature_parameters(signature)
                .iter()
                .map(|parameter| {
                    identities.intern(
                        &program
                            .type_identity_text(
                                scope.type_identity(program, parameter.type_reference),
                            )
                            .unwrap_or_default(),
                    )
                })
                .collect(),
            entry_claims: service_entry_claims(program, scope, signature, identities),
            has_result: signature.return_type.is_valid(),
            result_type_identity: signature
                .return_type
                .is_valid()
                .then(|| {
                    identities.intern(
                        &program
                            .type_identity_text(scope.type_identity(program, signature.return_type))
                            .unwrap_or_default(),
                    )
                })
                .unwrap_or_default(),
            result_claims: service_result_claims(program, scope, signature, identities),
            service_reach: service_reach_names(program, trait_definition, signature),
            synchronous_invocations: synchronous_invocation_names(program, signature),
            may_suspend: signature.suspends,
            may_block: signature.blocks,
            terminates_guarantee: signature.termination_guarantee.promises_termination(),
            termination_premises: service_progress_premises(program, signature, identities),
            calling_plan_report_fingerprint: calling_plan
                .map(|identity| identity.report_fingerprint),
            calling_plan_commitment: calling_plan.map(|identity| identity.commitment),
        });
    }
}

fn service_progress_premises(
    program: &checked_trees::typed_trees::TypedTrees,
    signature: &checked_trees::typed_trees::signature::StateSignature,
    identities: &mut ProviderIdentityTable,
) -> Vec<ServiceProgressPremise> {
    service_progress_premises_for_parameters(
        program,
        &signature.termination_guarantee,
        program.state_signature_parameters(signature),
        identities,
    )
}

fn service_progress_premises_for_parameters(
    program: &checked_trees::typed_trees::TypedTrees,
    guarantee: &language_semantics::TerminationGuarantee,
    parameters: &[checked_trees::typed_trees::signature::StateParameter],
    identities: &mut ProviderIdentityTable,
) -> Vec<ServiceProgressPremise> {
    let language_semantics::TerminationGuarantee::Terminates { premises } = guarantee else {
        return Vec::new();
    };
    premises
        .iter()
        .map(|premise| {
            let position = checked_trees::typed_trees::signature::state_parameter_position(
                parameters,
                premise.subject.root,
            )
            .expect("normalized public progress premise must be parameter-rooted");
            let subject = ServiceProgressSubject::Parameter(position);
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
                    identities,
                ),
            }
        })
        .collect()
}

fn service_progress_establishment_routes(
    program: &checked_trees::typed_trees::TypedTrees,
    profile: language_semantics::SemanticDomainId,
    identities: &mut ProviderIdentityTable,
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
                .trait_definition_by_symbol(route.source_symbol())
                .expect("normalized progress establishment route must name one trait owner");
            let requirement = program
                .trait_machine_signature_by_symbol(owner, route.requirement_symbol())
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
                requirement_identity: identities
                    .intern(&program.trait_requirement_callable_identity_text(owner, requirement)),
            }
        })
        .collect::<Vec<_>>();
    // Interned ids mint in encounter order; the canonical premise order is
    // kind-then-text, which the digest encoder also enforces on encode.
    routes.sort_by(|left, right| {
        left.kind.cmp(&right.kind).then_with(|| {
            identities
                .text(left.requirement_identity)
                .cmp(identities.text(right.requirement_identity))
        })
    });
    routes.dedup();
    routes
}

fn service_entry_claims(
    program: &checked_trees::typed_trees::TypedTrees,
    scope: &AppliedTraitScope<'_>,
    signature: &checked_trees::typed_trees::signature::StateSignature,
    identities: &mut ProviderIdentityTable,
) -> Vec<ServiceEntryClaim> {
    let mut claims = Vec::new();
    for (parameter_index, parameter) in program
        .state_signature_parameters(signature)
        .iter()
        .enumerate()
    {
        let reference = scope.type_reference(program, parameter.type_reference);
        if reference.multiplicity(program) != language_semantics::Multiplicity::Linear {
            continue;
        }
        append_routed_entry_claims(
            program,
            reference,
            parameter_index,
            scope.declaration.symbol,
            signature.symbol,
            identities,
            &mut claims,
        );
    }
    claims.sort_by(|left, right| {
        left.parameter_index
            .cmp(&right.parameter_index)
            .then_with(|| {
                identities
                    .text(left.carrier_identity)
                    .cmp(identities.text(right.carrier_identity))
            })
            .then_with(|| left.domain.cmp(&right.domain))
    });
    claims.dedup_by(|left, right| {
        left.parameter_index == right.parameter_index
            && identities.text(left.carrier_identity) == identities.text(right.carrier_identity)
            && left.domain == right.domain
    });
    claims
}

fn service_result_claims(
    program: &checked_trees::typed_trees::TypedTrees,
    scope: &AppliedTraitScope<'_>,
    signature: &checked_trees::typed_trees::signature::StateSignature,
    identities: &mut ProviderIdentityTable,
) -> Vec<ServiceResultClaim> {
    let reference = scope.type_reference(program, signature.return_type);
    if !reference.reference.is_valid()
        || reference.multiplicity(program) != language_semantics::Multiplicity::Linear
    {
        return Vec::new();
    }
    let mut claims = Vec::new();
    append_bodyless_result_claims(
        program,
        reference,
        scope.declaration.symbol,
        signature.symbol,
        identities,
        &mut claims,
    );
    claims.sort_by(|left, right| left.domain.cmp(&right.domain));
    claims.dedup_by(|left, right| left.domain == right.domain);
    claims
}

fn append_bodyless_result_claims(
    program: &checked_trees::typed_trees::TypedTrees,
    reference: AppliedTypeReference<'_>,
    boundary_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
    identities: &mut ProviderIdentityTable,
    claims: &mut Vec<ServiceResultClaim>,
) {
    use checked_trees::typed_trees::types::{TypeConstraintNode, TypeReferenceNode};

    match program
        .type_reference_table
        .type_reference(reference.reference)
    {
        TypeReferenceNode::Reference { referee, .. } => {
            append_bodyless_result_claims(
                program,
                reference.child(program, *referee),
                boundary_trait,
                requirement,
                identities,
                claims,
            );
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            append_bodyless_result_claims(
                program,
                reference.child(program, *base_type),
                boundary_trait,
                requirement,
                identities,
                claims,
            );
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
    program: &checked_trees::typed_trees::TypedTrees,
    reference: AppliedTypeReference<'_>,
    parameter_index: usize,
    boundary_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
    identities: &mut ProviderIdentityTable,
    claims: &mut Vec<ServiceEntryClaim>,
) {
    use checked_trees::typed_trees::types::{TypeConstraintNode, TypeReferenceNode};

    match program
        .type_reference_table
        .type_reference(reference.reference)
    {
        TypeReferenceNode::Reference { referee, .. } => {
            append_routed_entry_claims(
                program,
                reference.child(program, *referee),
                parameter_index,
                boundary_trait,
                requirement,
                identities,
                claims,
            );
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            append_routed_entry_claims(
                program,
                reference.child(program, *base_type),
                parameter_index,
                boundary_trait,
                requirement,
                identities,
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
                        carrier_identity: identities.intern(
                            &program
                                .type_identity_text(
                                    reference.child(program, *base_type).type_identity(program),
                                )
                                .unwrap_or_default(),
                        ),
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
    program: &checked_trees::typed_trees::TypedTrees,
    trait_definition: &checked_trees::typed_trees::trait_definition::TraitDefinition,
    signature: &checked_trees::typed_trees::signature::StateSignature,
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
    program: &checked_trees::typed_trees::TypedTrees,
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
    program: &checked_trees::typed_trees::TypedTrees,
    signature: &checked_trees::typed_trees::signature::StateSignature,
) -> Vec<String> {
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .collect::<Vec<_>>();
    boundary_invocation_names(
        program,
        checked_trees::typed_trees::invocations::declared_signature_invocations(program, signature),
        &parameters,
    )
}

fn machine_synchronous_invocation_names(
    program: &checked_trees::typed_trees::TypedTrees,
    machine: &checked_trees::typed_trees::machine::Machine,
) -> Vec<String> {
    let Some(entry) = program.machine_states(machine).first() else {
        return Vec::new();
    };
    let parameters = program.state_parameters(entry).iter().collect::<Vec<_>>();
    boundary_invocation_names(
        program,
        checked_trees::typed_trees::invocations::declared_machine_invocations(program, machine),
        &parameters,
    )
}

fn boundary_trait_name_for_type(
    program: &checked_trees::typed_trees::TypedTrees,
    type_reference: checked_trees::typed_trees::types::TypeReferenceHandle,
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

/// The sorted, deduplicated boundary-invocation names a signature or
/// machine entry declares: parameter targets resolve through the entry's
/// own parameter roster to their boundary-trait type name, and service
/// targets name the boundary trait directly.
fn boundary_invocation_names(
    program: &checked_trees::typed_trees::TypedTrees,
    targets: Vec<flow_effects::InvocationTarget>,
    parameters: &[&checked_trees::typed_trees::signature::StateParameter],
) -> Vec<String> {
    let mut names = targets
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
