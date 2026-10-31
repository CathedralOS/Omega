//! Derive provider service schemas from checked declaration surfaces.
//!
//! The caller selects a Build or product declaration context. The assembled
//! tree may retain both checked copies of a dependency; equal package and
//! signature identities do not authorize substituting one for the other.
//! Portable requirement names are complete module paths projected from the
//! exact symbol, paired with package identity. An authored leaf is not unique
//! even inside one package; replay must use the same projection as derivation.

mod applied_receivers;
mod correspondence;

use super::ProviderSchemaDeclaration;
use language_semantics::service_surface::identities::ProviderIdentityTable;
use std::sync::Arc;

/// One source projection retains method correspondence at the point it is known.
/// Its symbols belong only to the producing program. Complete simultaneous
/// frames are consumed while projecting types, not copied into another ledger.
pub struct ProjectedServiceSurface<'program> {
    program: &'program crate::TypedTrees,
    pub declaration: Arc<ServiceSurface>,
    pub schema: ProviderSchemaDeclaration,
    pub argument_owner: symbols::SymbolHandle,
    pub requirements: Vec<ServiceRequirementSource>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ServiceRequirementSource {
    pub declaring_trait: symbols::SymbolHandle,
    pub symbol: symbols::SymbolHandle,
}

impl ProjectedServiceSurface<'_> {
    /// Record a deferred catalog obligation during source formation. Ordinary
    /// surface projection does not reject a dormant unsupported declaration.
    pub(super) fn validate_correspondence(&self) -> Result<(), diagnostics::Diagnostic> {
        correspondence::validate(self.program, self)
    }
}

use crate::provider_declarations::ProviderPlanningContext;
use crate::trait_definition::{AppliedTraitScope, AppliedTypeReference};
use language_semantics::service_surface::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceProgressEstablishmentRoute,
    ServiceProgressEstablishmentRouteKind, ServiceProgressPremise, ServiceProgressSubject,
    ServiceResultClaim, ServiceSurface,
};

/// Whether retained declaration provenance belongs to the product frontier.
/// Build copies cannot supply final product plans even when package and names
/// match. Source-free declarations retain their explicitly unscoped behavior.
pub fn is_product_declaration(program: &crate::TypedTrees, symbol: symbols::SymbolHandle) -> bool {
    ProviderPlanningContext::new(program, source::DependencyScope::Product).contains(symbol)
}

/// Project an abstract boundary declaration without target calling evidence.
pub fn from_typed<'program>(
    context: ProviderPlanningContext<'program>,
    trait_definition: &'program crate::trait_definition::TraitDefinition,
) -> Option<ProjectedServiceSurface<'program>> {
    from_typed_application(context, AppliedTraitScope::abstract_scope(trait_definition))
}

/// Reify an occurrence-owned application without discarding its simultaneous
/// type and region scopes. Build still chooses a declaration; this query keeps
/// the application that source checking supplied, and grants no selection.
pub fn from_typed_application<'program>(
    context: ProviderPlanningContext<'program>,
    root: AppliedTraitScope<'program>,
) -> Option<ProjectedServiceSurface<'program>> {
    let program = context.typed;
    let trait_definition = root.declaration;
    if !trait_definition.is_boundary || !context.contains(trait_definition.symbol) {
        return None;
    }
    let mut methods = Vec::new();
    let mut requirements = Vec::new();
    let mut identities = ProviderIdentityTable::default();
    let argument_owner = if program.trait_type_parameters(trait_definition).is_empty() {
        trait_definition.symbol
    } else {
        root.argument_owner
    };
    let scopes = applied_receivers::collect_scopes(program, root)?;
    for scope in scopes.iter().rev() {
        collect_service_methods(
            program,
            scope,
            &mut requirements,
            &mut methods,
            &mut identities,
        );
    }
    let receiver_applications =
        applied_receivers::receiver_applications(program, &scopes, &mut identities)?;
    Some(ProjectedServiceSurface {
        program,
        schema: ProviderSchemaDeclaration::BoundaryTrait(trait_definition.symbol),
        argument_owner,
        requirements,
        declaration: Arc::new(ServiceSurface {
            trait_name: identities.intern(&program.trait_declaration_path(trait_definition)),
            trait_package_identity: program
                .symbols
                .symbol_package_identity(trait_definition.symbol),
            identities,
            receiver_applications,
            methods,
        }),
    })
}

/// Reify one exact overloaded boundary-operator requirement as a
/// single-row provider slot. `trait_name` is the legacy field name on the
/// shared carrier; operator slots use their stable signature identity so
/// f32/f64 overloads can never collide or be selected as one another.
pub fn from_typed_operator<'program>(
    context: ProviderPlanningContext<'program>,
    operator: &crate::operator::OperatorDefinition,
) -> Option<ProjectedServiceSurface<'program>> {
    let program = context.typed;
    if !context.contains(operator.symbol) || !operator.is_boundary {
        return None;
    }
    let mut identities = ProviderIdentityTable::default();
    let requirement_identity = identities.intern(
        &crate::operator::boundary_operator_requirement_identity(program, operator),
    );
    Some(ProjectedServiceSurface {
        program,
        schema: ProviderSchemaDeclaration::BoundaryOperator(operator.symbol),
        argument_owner: operator.symbol,
        requirements: vec![ServiceRequirementSource {
            declaring_trait: symbols::SymbolHandle::invalid(),
            symbol: operator.symbol,
        }],
        declaration: Arc::new(ServiceSurface {
            trait_name: requirement_identity,
            receiver_applications: Vec::new(),
            trait_package_identity: program.symbols.symbol_package_identity(operator.symbol),
            methods: vec![ServiceMethod {
                completion: crate::boundary_completions::boundary_requirement_completion(
                    program,
                    operator.symbol,
                ),
                name: "realize".to_owned(),
                requirement_owner: crate::operator::boundary_operator_requirement_identity(
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
                        identities.intern(
                            &program.normalized_type_identity_text(parameter.type_reference),
                        )
                    })
                    .collect(),
                entry_claims: Vec::new(),
                has_result: operator.return_type.is_valid(),
                result_type_identity: operator
                    .return_type
                    .is_valid()
                    .then(|| {
                        identities
                            .intern(&program.normalized_type_identity_text(operator.return_type))
                    })
                    .unwrap_or_default(),
                result_claims: Vec::new(),
                service_reach: Vec::new(),
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
            }],
            identities,
        }),
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
pub fn from_typed_boundary_requirement<'program>(
    context: ProviderPlanningContext<'program>,
    requirement: &crate::machine::Machine,
) -> Option<ProjectedServiceSurface<'program>> {
    if !context.contains(requirement.symbol)
        || !requirement.symbol.is_valid()
        || requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
        || requirement.body_is_present
    {
        return None;
    }
    project_boundary_machine_schema(
        context,
        requirement,
        ProviderSchemaDeclaration::BoundaryRequirement(requirement.symbol),
    )
}

pub fn project_boundary_machine_schema<'program>(
    context: ProviderPlanningContext<'program>,
    requirement: &crate::machine::Machine,
    schema: ProviderSchemaDeclaration,
) -> Option<ProjectedServiceSurface<'program>> {
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
    Some(ProjectedServiceSurface {
        program,
        schema,
        argument_owner: requirement.symbol,
        requirements: vec![ServiceRequirementSource {
            declaring_trait: symbols::SymbolHandle::invalid(),
            symbol: requirement.symbol,
        }],
        declaration: Arc::new(ServiceSurface {
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
                completion: crate::boundary_completions::boundary_requirement_completion(
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
                        identities.intern(
                            &program.normalized_type_identity_text(parameter.type_reference),
                        )
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
            }],
            identities,
        }),
    })
}

fn collect_service_methods(
    program: &crate::TypedTrees,
    scope: &AppliedTraitScope<'_>,
    requirements: &mut Vec<ServiceRequirementSource>,
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
        requirements.push(ServiceRequirementSource {
            declaring_trait: trait_definition.symbol,
            symbol: signature.symbol,
        });
        methods.push(ServiceMethod {
            completion: crate::boundary_completions::boundary_requirement_completion(
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
            result_claims: service_result_claims(program, scope, signature),
            service_reach: service_reach_names(program, trait_definition, signature),
            synchronous_invocations: synchronous_invocation_names(program, signature),
            may_suspend: signature.suspends,
            may_block: signature.blocks,
            terminates_guarantee: signature.termination_guarantee.promises_termination(),
            termination_premises: service_progress_premises(program, signature, identities),
        });
    }
}

fn service_progress_premises(
    program: &crate::TypedTrees,
    signature: &crate::signature::StateSignature,
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
    program: &crate::TypedTrees,
    guarantee: &language_semantics::TerminationGuarantee,
    parameters: &[crate::signature::StateParameter],
    identities: &mut ProviderIdentityTable,
) -> Vec<ServiceProgressPremise> {
    let language_semantics::TerminationGuarantee::Terminates { premises } = guarantee else {
        return Vec::new();
    };
    premises
        .iter()
        .map(|premise| {
            let position =
                crate::signature::state_parameter_position(parameters, premise.subject.root)
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
    program: &crate::TypedTrees,
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
    program: &crate::TypedTrees,
    scope: &AppliedTraitScope<'_>,
    signature: &crate::signature::StateSignature,
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
    program: &crate::TypedTrees,
    scope: &AppliedTraitScope<'_>,
    signature: &crate::signature::StateSignature,
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
        &mut claims,
    );
    claims.sort_by(|left, right| left.domain.cmp(&right.domain));
    claims.dedup_by(|left, right| left.domain == right.domain);
    claims
}

/// Visit every domain constraint on a type reference's reference/constraint
/// spine that establishes `requirement` through `boundary_trait`. Entry and
/// result claims are two projections of one recognition, so the spine walk
/// exists once; `visit` receives the constrained subject its domain applies to.
fn for_each_establishing_domain<'scope, Visitor>(
    program: &crate::TypedTrees,
    reference: AppliedTypeReference<'scope>,
    boundary_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
    visit: &mut Visitor,
) where
    Visitor: FnMut(AppliedTypeReference<'scope>, &crate::types::DomainConstraint),
{
    use crate::types::{TypeConstraintNode, TypeReferenceNode};

    match program.type_reference_node(reference.reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            for_each_establishing_domain(
                program,
                reference.child(program, *referee),
                boundary_trait,
                requirement,
                visit,
            );
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let subject = reference.child(program, *base_type);
            for_each_establishing_domain(program, subject, boundary_trait, requirement, visit);
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
                    visit(subject, domain);
                }
            }
        }
        _ => {}
    }
}

/// The domain coordinate a claim publishes: the registered semantic domain's
/// name, or the authored spelling when the constraint carries no registration.
fn domain_claim_name(
    program: &crate::TypedTrees,
    domain: &crate::types::DomainConstraint,
) -> String {
    domain
        .semantic_id
        .is_valid()
        .then(|| program.semantic_domains.name(domain.semantic_id))
        .flatten()
        .unwrap_or_else(|| domain.name.as_str())
        .to_owned()
}

fn append_bodyless_result_claims(
    program: &crate::TypedTrees,
    reference: AppliedTypeReference<'_>,
    boundary_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
    claims: &mut Vec<ServiceResultClaim>,
) {
    for_each_establishing_domain(
        program,
        reference,
        boundary_trait,
        requirement,
        &mut |_, domain| {
            if domain.predicate_presence != language_semantics::DomainPredicatePresence::Absent {
                return;
            }
            claims.push(ServiceResultClaim {
                domain: domain_claim_name(program, domain),
                effective_carry: language_semantics::CarryPolicy::STRICT,
            });
        },
    );
}

fn append_routed_entry_claims(
    program: &crate::TypedTrees,
    reference: AppliedTypeReference<'_>,
    parameter_index: usize,
    boundary_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
    identities: &mut ProviderIdentityTable,
    claims: &mut Vec<ServiceEntryClaim>,
) {
    for_each_establishing_domain(
        program,
        reference,
        boundary_trait,
        requirement,
        &mut |subject, domain| {
            claims.push(ServiceEntryClaim {
                parameter_index,
                carrier_identity: identities.intern(
                    &program
                        .type_identity_text(subject.type_identity(program))
                        .unwrap_or_default(),
                ),
                domain: domain_claim_name(program, domain),
                predicate_presence: domain.predicate_presence,
                effective_carry: language_semantics::CarryPolicy::STRICT,
                authority_flow: ServiceEntryAuthorityFlow::Accepts,
            });
        },
    );
}

fn service_reach_names(
    program: &crate::TypedTrees,
    trait_definition: &crate::trait_definition::TraitDefinition,
    signature: &crate::signature::StateSignature,
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
    program: &crate::TypedTrees,
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
    program: &crate::TypedTrees,
    signature: &crate::signature::StateSignature,
) -> Vec<String> {
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .collect::<Vec<_>>();
    boundary_invocation_names(
        program,
        crate::invocations::declared_signature_invocations(program, signature),
        &parameters,
    )
}

fn machine_synchronous_invocation_names(
    program: &crate::TypedTrees,
    machine: &crate::machine::Machine,
) -> Vec<String> {
    let Some(entry) = program.machine_states(machine).first() else {
        return Vec::new();
    };
    let parameters = program.state_parameters(entry).iter().collect::<Vec<_>>();
    boundary_invocation_names(
        program,
        crate::invocations::declared_machine_invocations(program, machine),
        &parameters,
    )
}

fn boundary_trait_name_for_type(
    program: &crate::TypedTrees,
    type_reference: crate::types::TypeReferenceHandle,
) -> Option<String> {
    let symbol = program
        .type_reference_node(type_reference)
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
    program: &crate::TypedTrees,
    targets: Vec<flow_effects::InvocationTarget>,
    parameters: &[&crate::signature::StateParameter],
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
