//! PRV1 (design-ruled 2026-07-17): the typed **ProviderPlan** policy
//! carrier -- one value per (provider type, service schema, target), unifying
//! checked `satisfies` closures and irreducible external leaves with the
//! remaining built-in platform-lowering tables. CONSTRUCTION IS FREE: any code can build a plan; PRV2
//! validates coverage/signatures/identity, PRV3 admits semantic claims
//! through the chapter-10 grant/receipt carrier and selects by a
//! slot-owner capability. PRV4 retired authored `provides`/populate tables:
//! ordinary target packages now own checked adapters and `via Binding` leaves.
//! Trust classification is ADMISSION OUTPUT,
//! never author-selected plan data -- which is why no trust field exists
//! on these types.

use super::foreign_locator::NormalizedForeignLocator;
pub use psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment;
use sha2::{Digest, Sha256};

/// Collision-resistant identity of one exact normalized provider plan.
///
/// Construction remains crate-owned. Compact plan fingerprints are retained
/// for existing reports and coordinates, but admission joins should retain
/// this digest or the complete [`ProviderPlan`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderPlanDigest([u8; 32]);

impl ProviderPlanDigest {
    const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// The service schema a plan serves: a boundary trait's callable surface,
/// reified from the typed `TraitDefinition` (today that read is scattered
/// -- parameter-count walks in the compiler pipeline, Console detection in
/// the interpreter; the schema type is the one honest carrier).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceSchema {
    /// The boundary trait's name (`Console`, `FilesystemHost`).
    pub trait_name: String,
    /// Exact package owning the selected boundary trait/operator declaration.
    /// `None` is explicit for toolchain, standalone, or source-free trees.
    pub trait_package_identity: Option<psi_core::PackageKeyIdentity>,
    /// One entry per trait machine, in declaration order.
    pub methods: Vec<ServiceMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceMethod {
    pub name: String,
    /// Semantic owner of the exact requirement. This differs from the
    /// enclosing schema when a target root inherits a stable core requirement
    /// and refines only its calling plan.
    pub requirement_owner: String,
    /// Exact package owning `requirement_owner`. Inherited requirements may
    /// differ from the selected schema owner. `None` is never repaired from
    /// the readable owner or overload identity.
    pub requirement_owner_package_identity: Option<psi_core::PackageKeyIdentity>,
    /// Stable named-callable identity of the exact requirement overload.
    /// Provider schemas are never name-only, including singleton schemas.
    pub requirement_identity: String,
    /// Declared parameter count (excluding any receiver) -- the same count
    /// the vtable-field encoder compares against call operands.
    pub parameter_count: usize,
    /// Positional semantic identities of the declared parameter types,
    /// excluding any receiver. Domain qualifications and carry permissions
    /// are part of these identities, so a provider plan cannot be replayed
    /// after an authority-bearing parameter is weakened or replaced.
    pub parameter_type_identities: Vec<String>,
    /// Linear routed qualifications accepted at this boundary entry. These
    /// are structured separately from the complete type identity so provider
    /// admission, carry planning, predicate discharge, and authority-flow
    /// artifacts do not have to parse a display string to recover a source
    /// obligation.
    pub entry_claims: Vec<ServiceEntryClaim>,
    /// Whether the method declares a return type.
    pub has_result: bool,
    /// Semantic identity of the declared result type. `None` denotes no
    /// result, not a unit-shaped result.
    pub result_type_identity: Option<String>,
    /// Linear routed qualifications established on this exact result. These
    /// are retained separately from the complete result type so runtime
    /// transition receipts can bind a concrete subject without parsing a
    /// normalized type display.
    pub result_claims: Vec<ServiceResultClaim>,
    /// EFX: normalized boundary-service identities rendered from the
    /// symbol-resolved service table. This includes the containing boundary
    /// trait and any explicit additional reach (with parent closure).
    pub service_reach: Vec<String>,
    /// Direct synchronous boundary bindings this method may enter before it
    /// returns, rendered as the selected binding's boundary-trait identity.
    /// This remains a direct edge set; it is never replaced by reach closure.
    pub synchronous_invocations: Vec<String>,
    /// Independent authored operational ceilings. These never derive from
    /// service reach and participate directly in provider schema identity.
    pub may_suspend: bool,
    pub may_block: bool,
    /// Existing public bodyless requirement guarantee. Progress-profile
    /// premises are retained separately below; private ranking evidence stays
    /// outside provider identity.
    pub terminates_guarantee: bool,
    /// Exact normalized premise schemas. The root distinguishes the installed
    /// provider receiver from caller parameters; projections and profile use
    /// semantic paths.
    pub termination_premises: Vec<ServiceProgressPremise>,
    /// Compact report coordinate for the canonical validated
    /// `BoundaryEntryPlan` selected by a concrete `Calling<C>` relationship.
    pub calling_plan_report_fingerprint: Option<u64>,
    /// Domain-separated commitment to that exact boundary calling plan.
    /// This is present exactly when the report coordinate is present.
    pub calling_plan_commitment: Option<BoundaryCallingPlanCommitment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceProgressPremise {
    pub profile: String,
    pub subject: ServiceProgressSubject,
    pub subject_projections: Vec<String>,
    /// Exact owner-authored relationships permitted to establish this
    /// profile. A selected provider plan retains these declarations for
    /// final composition, but none of them is itself an establishment
    /// receipt.
    pub establishment_routes: Vec<ServiceProgressEstablishmentRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceProgressEstablishmentRoute {
    pub kind: ServiceProgressEstablishmentRouteKind,
    pub requirement_identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServiceProgressEstablishmentRouteKind {
    CheckedRequirement,
    BoundaryRequirement,
}

impl ServiceProgressEstablishmentRouteKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CheckedRequirement => "checked_requirement",
            Self::BoundaryRequirement => "boundary_requirement",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServiceProgressSubject {
    /// The service/provider capability itself; composition supplies the exact
    /// installed provider occurrence.
    ProviderReceiver,
    /// One ordinary caller-visible parameter, excluding the receiver.
    Parameter(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceEntryAuthorityFlow {
    /// The selected provider accepts a caller/external-world claim at entry.
    Accepts,
}

impl ServiceEntryAuthorityFlow {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepts => "accepts",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceEntryClaim {
    /// Positional parameter ordinal after excluding any receiver.
    pub parameter_index: usize,
    /// Canonical normalized identity of the carrier qualified by this routed
    /// domain. Consumers must not recover it by parsing the complete parameter
    /// type or substitute an authored display spelling.
    pub carrier_identity: String,
    /// Carrier-aware normalized semantic-domain identity retained by the typed
    /// constraint. This is not the authored short spelling.
    pub domain: String,
    /// Whether the routed qualification also carries predicates that must be
    /// proved at the concrete installed occurrence. Bodyless claims may flow
    /// through the generic external-root acknowledgement path; predicate
    /// claims require a specialized installer that discharges them first.
    pub predicate_body: psi_language_semantics::DomainPredicateBody,
    /// Accepted resource claims are born maximally strict. Exact positive
    /// carry permissions remain separate constrained-type facts.
    pub effective_carry: psi_language_semantics::CarryPolicy,
    pub authority_flow: ServiceEntryAuthorityFlow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceResultClaim {
    pub domain: String,
    pub effective_carry: psi_language_semantics::CarryPolicy,
}

/// How one method binds on one target -- the Binding sum's union with the
/// platform tables' mechanisms. Aligned with the host-ABI plan's
/// `HostBindingMechanism` so PRV4's relocation is a rename. Instructions are
/// checked `asm` bodies whose catalog contracts contribute their obligations;
/// they are deliberately not a second, bodiless provider-binding mechanism.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderBinding {
    /// One evaluated, target-validated physical foreign locator. Its atomic
    /// byte coordinates remain sealed together through selection and opaque
    /// executable accounting.
    Import { locator: NormalizedForeignLocator },
    /// Temporary source `via Binding::DllImport("library", "symbol")` bridge.
    ///
    /// This is intentionally distinct from [`Self::Import`]: string pairs are
    /// not normalized evaluated binding data and cannot silently enter the new
    /// locator path. Remove it with the source evaluator join.
    StringBackedImportBootstrap { library: String, symbol: String },
    /// Direct system call by number.
    Syscall { number: i64 },
    /// A compiler-known operation furnished by the selected target package.
    /// This is the exact normalized realization-machine overload identity;
    /// target catalog selection is derived separately from the checked
    /// requirement and selected target.
    CompilerIntrinsic { machine: String },
    /// COM/UEFI slot dispatch: callee address read from the receiver.
    VtableSlot { index: i64 },
    /// Field-model vtable dispatch: the fn-ptr field of a named table
    /// struct; the byte offset resolves from the layout plan downstream.
    VtableField { table: String, field: String },
    /// UEFI service-table function (the boot-services shape).
    TableFunction { table: String, field: String },
    /// An ORDINARY CHECKED MACHINE realizing the requirement (the ruling's
    /// composite form: lowering sequences and argument adaptation are
    /// checked Omega code with an explicit satisfies edge, never authored
    /// rows). Admission checks the adapter as a REFINEMENT: its transitive
    /// effects must fit inside the satisfied requirement's declared
    /// ceiling.
    CheckedAdapter {
        /// Canonical typed machine-overload identity, never a short name.
        machine_identity: String,
        /// Exact package owning the checked adapter. `None` is retained only
        /// for toolchain, standalone, and focused source-free programs.
        machine_package_identity: Option<psi_core::PackageKeyIdentity>,
    },
}

/// One method's normalized provider binding. Composite argument adaptation is
/// checked Omega code, so plan rows carry only irreducible leaf mechanisms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlanRow {
    pub method: String,
    /// Exact requirement overload supplied by this row. It is always nonempty;
    /// a method spelling is display/debug data, not dispatch identity.
    pub requirement_identity: String,
    pub binding: ProviderBinding,
}

/// The PRV1 carrier: one provider type's plan for one service schema on one
/// target. `origin_package_identity` is exact compiler-derived provenance
/// INPUT to admission (a package can never self-grant); the legacy readable
/// `origin_package` is display data only. The admission verdict itself lives
/// in the chapter-10 receipts, never here.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderPlan {
    /// The plan's own name (`omega::host::standard::console`, the future
    /// slot-selection key).
    pub name: String,
    /// The nominal provider type whose explicit conformance closure produced
    /// this plan. Empty only for a free external leaf; checked adapters belong
    /// to nominal provider types. Slot overrides select this identity, never
    /// individual rows.
    pub provider_type: String,
    /// Exact package owning `provider_type`. Free external leaves and
    /// toolchain/standalone/source-free trees retain `None`; consumers must not
    /// infer this from `provider_type` or the realizing machine's package.
    pub provider_type_package_identity: Option<psi_core::PackageKeyIdentity>,
    /// The target this plan serves (`windows_x64`; empty = every target).
    pub target: String,
    /// The schema served.
    pub schema: ServiceSchema,
    /// One row per bound method.
    pub rows: Vec<ProviderPlanRow>,
    /// Exact package that authored the realizing machine(s). `None` means the
    /// source was not package-owned (for example toolchain or standalone
    /// source); consumers must not infer ownership from any readable name.
    pub origin_package_identity: Option<psi_core::PackageKeyIdentity>,
    /// Legacy readable origin label. This is diagnostic data, not admission
    /// provenance, and must never repair a missing exact package identity.
    pub origin_package: String,
}

impl ServiceSchema {
    /// PRV2: reify a typed boundary trait's callable surface. `None` for a
    /// non-boundary trait (only boundary traits have service schemas).
    pub fn from_typed(
        program: &psi_typed_trees::TypedTrees,
        trait_definition: &psi_typed_trees::trait_definition::TraitDefinition,
    ) -> Option<Self> {
        Self::from_typed_instance(program, trait_definition, &[])
    }

    /// Reify one concrete generic boundary instance. The argument tuple is
    /// semantic input only for resolving evaluated calling-plan identity;
    /// policy type/source names remain absent from the published schema.
    pub fn from_typed_instance(
        program: &psi_typed_trees::TypedTrees,
        trait_definition: &psi_typed_trees::trait_definition::TraitDefinition,
        boundary_arguments: &[psi_typed_trees::types::TypeReferenceHandle],
    ) -> Option<Self> {
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
        Some(Self {
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
        program: &psi_typed_trees::TypedTrees,
        operator: &psi_typed_trees::operator::OperatorDefinition,
    ) -> Option<Self> {
        operator.is_boundary.then(|| Self {
            trait_name: psi_typed_trees::operator::boundary_operator_requirement_identity(
                program, operator,
            ),
            trait_package_identity: program.symbols.symbol_package_identity(operator.symbol),
            methods: vec![ServiceMethod {
                name: "realize".to_owned(),
                requirement_owner:
                    psi_typed_trees::operator::boundary_operator_requirement_identity(
                        program, operator,
                    ),
                requirement_owner_package_identity: program
                    .symbols
                    .symbol_package_identity(operator.symbol),
                requirement_identity:
                    psi_typed_trees::operator::boundary_operator_requirement_identity(
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
}

fn collect_service_methods(
    program: &psi_typed_trees::TypedTrees,
    trait_definition: &psi_typed_trees::trait_definition::TraitDefinition,
    policy_owner: psi_symbols::SymbolHandle,
    boundary_arguments: &[psi_typed_trees::types::TypeReferenceHandle],
    visited: &mut Vec<psi_symbols::SymbolHandle>,
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
    program: &psi_typed_trees::TypedTrees,
    signature: &psi_typed_trees::signature::StateSignature,
) -> Vec<ServiceProgressPremise> {
    let psi_language_semantics::TerminationGuarantee::Terminates { premises } =
        &signature.termination_guarantee
    else {
        return Vec::new();
    };
    let parameters = program.state_signature_parameters(signature);
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
    program: &psi_typed_trees::TypedTrees,
    profile: psi_language_semantics::SemanticDomainId,
) -> Vec<ServiceProgressEstablishmentRoute> {
    let domain = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.semantic_id == profile)
        .expect("normalized progress premise must name one declared profile domain");
    debug_assert_eq!(
        domain.classification,
        Some(psi_language_semantics::DomainClassification::ProgressProfile)
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
                psi_language_semantics::DomainEstablishmentRoute::CheckedRequirement { .. } => {
                    ServiceProgressEstablishmentRouteKind::CheckedRequirement
                }
                psi_language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                    ..
                } => ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
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
    program: &psi_typed_trees::TypedTrees,
    trait_definition: &psi_typed_trees::trait_definition::TraitDefinition,
    signature: &psi_typed_trees::signature::StateSignature,
) -> Vec<ServiceEntryClaim> {
    let mut claims = Vec::new();
    for (parameter_index, parameter) in program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
    {
        if program.type_multiplicity(parameter.type_reference)
            != psi_language_semantics::Multiplicity::Linear
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
    program: &psi_typed_trees::TypedTrees,
    trait_definition: &psi_typed_trees::trait_definition::TraitDefinition,
    signature: &psi_typed_trees::signature::StateSignature,
) -> Vec<ServiceResultClaim> {
    if !signature.return_type.is_valid()
        || program.type_multiplicity(signature.return_type)
            != psi_language_semantics::Multiplicity::Linear
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
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    boundary_trait: psi_symbols::SymbolHandle,
    requirement: psi_symbols::SymbolHandle,
    claims: &mut Vec<ServiceResultClaim>,
) {
    use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceNode};

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
                    && domain.predicate_body
                        == psi_language_semantics::DomainPredicateBody::Bodyless
                    && domain.establishment_routes.iter().any(|route| {
                        matches!(
                            route,
                            psi_language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
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
                        effective_carry: psi_language_semantics::CarryPolicy::STRICT,
                    });
                }
            }
        }
        _ => {}
    }
}

fn append_routed_entry_claims(
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    parameter_index: usize,
    boundary_trait: psi_symbols::SymbolHandle,
    requirement: psi_symbols::SymbolHandle,
    claims: &mut Vec<ServiceEntryClaim>,
) {
    use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceNode};

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
                            psi_language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
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
                        effective_carry: psi_language_semantics::CarryPolicy::STRICT,
                        authority_flow: ServiceEntryAuthorityFlow::Accepts,
                    });
                }
            }
        }
        _ => {}
    }
}

fn service_reach_names(
    program: &psi_typed_trees::TypedTrees,
    trait_definition: &psi_typed_trees::trait_definition::TraitDefinition,
    signature: &psi_typed_trees::signature::StateSignature,
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

fn synchronous_invocation_names(
    program: &psi_typed_trees::TypedTrees,
    signature: &psi_typed_trees::signature::StateSignature,
) -> Vec<String> {
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let mut names = psi_effects::declared_signature_invocations(program, signature)
        .into_iter()
        .filter_map(|target| match target {
            psi_effects::InvocationTarget::Parameter(index) => parameters
                .get(index as usize)
                .map(|parameter| parameter.type_reference)
                .and_then(|type_reference| boundary_trait_name_for_type(program, type_reference)),
            psi_effects::InvocationTarget::Service(symbol) => program
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
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
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

fn push_package_identity(
    rendered: &mut String,
    label: &str,
    identity: Option<psi_core::PackageKeyIdentity>,
) {
    rendered.push('\n');
    rendered.push_str(label);
    rendered.push(':');
    match identity {
        Some(identity) => {
            for byte in identity.digest() {
                rendered.push_str(&format!("{byte:02x}"));
            }
        }
        None => rendered.push_str("<unbound>"),
    }
}

impl ProviderPlan {
    /// Domain-separated SHA-256 commitment to the complete normalized plan
    /// structure. Unlike [`Self::identity_fingerprint`], this includes exact
    /// normalized foreign-locator coordinates and is suitable for retained
    /// evidence identity.
    pub fn identity_digest(&self) -> ProviderPlanDigest {
        let mut encoder = ProviderPlanDigestEncoder::new();
        encoder.string(&self.name);
        encoder.string(&self.provider_type);
        encoder.package_identity(self.provider_type_package_identity);
        encoder.string(&self.target);
        encoder.string(&self.schema.trait_name);
        encoder.package_identity(self.schema.trait_package_identity);

        let mut methods = self.schema.methods.iter().collect::<Vec<_>>();
        methods.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.requirement_identity.cmp(&right.requirement_identity))
        });
        encoder.len(methods.len());
        for method in methods {
            encoder.service_method(method);
        }

        let mut rows = self.rows.iter().collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            left.method
                .cmp(&right.method)
                .then_with(|| left.requirement_identity.cmp(&right.requirement_identity))
        });
        encoder.len(rows.len());
        for row in rows {
            encoder.string(&row.method);
            encoder.string(&row.requirement_identity);
            encoder.provider_binding(&row.binding);
        }
        encoder.package_identity(self.origin_package_identity);
        encoder.finish()
    }

    /// PRV2: the plan's NORMALIZED IDENTITY -- an FNV-1a fingerprint over
    /// the canonical rendering (name, exact package provenance, target,
    /// schema surface, rows in method order). Two plans with the same
    /// fingerprint are treated as the same execution policy by the current
    /// provider pipeline; presentation (row order, whitespace) is excluded.
    /// This 64-bit compatibility key is not collision-resistant package
    /// admission evidence.
    pub fn identity_fingerprint(&self) -> u64 {
        let mut rendered = format!(
            "{}\n{}\n{}\n{}",
            self.name, self.provider_type, self.target, self.schema.trait_name
        );
        push_package_identity(
            &mut rendered,
            "provider-type-package",
            self.provider_type_package_identity,
        );
        push_package_identity(
            &mut rendered,
            "schema-package",
            self.schema.trait_package_identity,
        );
        rendered.push_str("\npackage:");
        match self.origin_package_identity {
            Some(identity) => {
                for byte in identity.digest() {
                    rendered.push_str(&format!("{byte:02x}"));
                }
            }
            None => rendered.push_str("<unbound>"),
        }
        let mut methods: Vec<&ServiceMethod> = self.schema.methods.iter().collect();
        methods.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.requirement_identity.cmp(&right.requirement_identity))
        });
        for method in methods {
            push_package_identity(
                &mut rendered,
                "requirement-owner-package",
                method.requirement_owner_package_identity,
            );
            rendered.push_str(&format!(
                "\nm:{}/{}/{}/{}/services:{}/invokes:{}/suspend:{}/block:{}/terminates:{}",
                method.name,
                method.requirement_identity,
                method.parameter_count,
                method.has_result,
                method.service_reach.join("+"),
                method.synchronous_invocations.join("+"),
                method.may_suspend,
                method.may_block,
                method.terminates_guarantee,
            ));
            let mut premises = method.termination_premises.iter().collect::<Vec<_>>();
            premises.sort_by(|left, right| {
                left.profile
                    .cmp(&right.profile)
                    .then_with(|| left.subject.cmp(&right.subject))
                    .then_with(|| left.subject_projections.cmp(&right.subject_projections))
            });
            for premise in premises {
                rendered.push_str(&format!(
                    "\nmt:{}/{}/{}",
                    premise.profile,
                    match premise.subject {
                        ServiceProgressSubject::ProviderReceiver => "self".to_owned(),
                        ServiceProgressSubject::Parameter(index) => format!("parameter:{index}"),
                    },
                    premise.subject_projections.join("::")
                ));
                let mut establishment_routes =
                    premise.establishment_routes.iter().collect::<Vec<_>>();
                establishment_routes.sort();
                for route in establishment_routes {
                    rendered.push_str(&format!(
                        "\nmtr:{}/{}",
                        route.kind.as_str(),
                        route.requirement_identity,
                    ));
                }
            }
            for parameter in &method.parameter_type_identities {
                rendered.push_str("\nmp:");
                rendered.push_str(parameter);
            }
            let mut entry_claims = method.entry_claims.iter().collect::<Vec<_>>();
            entry_claims.sort_by(|left, right| {
                left.parameter_index
                    .cmp(&right.parameter_index)
                    .then_with(|| left.carrier_identity.cmp(&right.carrier_identity))
                    .then_with(|| left.domain.cmp(&right.domain))
            });
            for claim in entry_claims {
                rendered.push_str(&format!(
                    "\nmc:{}/{}/{}/{}/{}/{}",
                    claim.parameter_index,
                    claim.carrier_identity,
                    claim.domain,
                    claim.predicate_body.as_str(),
                    claim.authority_flow.as_str(),
                    claim.effective_carry,
                ));
            }
            if let Some(result) = &method.result_type_identity {
                rendered.push_str("\nmr:");
                rendered.push_str(result);
            }
            let mut result_claims = method.result_claims.iter().collect::<Vec<_>>();
            result_claims.sort_by(|left, right| left.domain.cmp(&right.domain));
            for claim in result_claims {
                rendered.push_str(&format!("\nmrc:{}/{}", claim.domain, claim.effective_carry,));
            }
            if let Some(fingerprint) = method.calling_plan_report_fingerprint {
                rendered.push_str(&format!("/calling:{fingerprint:016x}"));
            }
        }
        let mut rows: Vec<&ProviderPlanRow> = self.rows.iter().collect();
        rows.sort_by(|left, right| {
            left.method
                .cmp(&right.method)
                .then_with(|| left.requirement_identity.cmp(&right.requirement_identity))
        });
        for row in rows {
            let binding_identity = match &row.binding {
                ProviderBinding::Import { locator } => {
                    format!("NormalizedImport:{:016x}", locator.normalized_identity(),)
                }
                ProviderBinding::StringBackedImportBootstrap { library, symbol } => {
                    format!("StringBackedImportBootstrap:{library:?}/{symbol:?}")
                }
                ProviderBinding::CompilerIntrinsic { machine, .. } => {
                    format!("CompilerIntrinsic {{ machine: {machine:?} }}")
                }
                ProviderBinding::CheckedAdapter {
                    machine_identity,
                    machine_package_identity,
                } => {
                    let mut identity = format!("CheckedAdapter:{machine_identity}");
                    push_package_identity(
                        &mut identity,
                        "checked-adapter-package",
                        *machine_package_identity,
                    );
                    identity
                }
                binding => format!("{binding:?}"),
            };
            rendered.push_str(&format!(
                "\nr:{}/{}/{}",
                row.method, row.requirement_identity, binding_identity
            ));
        }
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in rendered.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    /// PRV2: full structural validation against the schema -- every method
    /// bound exactly once and no stray rows. Returns NAMED errors; empty =
    /// structurally valid.
    pub fn validate_against_schema(&self) -> Vec<String> {
        let mut errors = self.validate_candidate_against_schema();
        for method in &self.schema.methods {
            let count = self
                .rows
                .iter()
                .filter(|row| self.schema.row_binds_method(row, method))
                .count();
            if count == 0 {
                errors.push(format!(
                    "plan `{}` does not bind `{}::{}`",
                    self.name, self.schema.trait_name, method.name
                ));
            }
        }
        errors
    }

    /// Validate one candidate before coverage/selection. Partial candidates
    /// are legitimate, but a candidate cannot duplicate a requirement, name a
    /// callable row outside its schema. This is the additive-only conformance check;
    /// selection decides whether the surviving candidate covers the complete
    /// slot.
    pub fn validate_candidate_against_schema(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.name.is_empty() {
            errors.push("provider plan has no exact selection name".to_owned());
        }
        if self.schema.trait_name.is_empty() {
            errors.push(format!(
                "plan `{}` schema has no exact boundary-slot identity",
                self.name
            ));
        }
        for (method_index, method) in self.schema.methods.iter().enumerate() {
            if method.calling_plan_report_fingerprint.is_some()
                != method.calling_plan_commitment.is_some()
            {
                errors.push(format!(
                    "plan `{}` schema method `{}::{}` must retain its calling-plan report coordinate and strong commitment together",
                    self.name, self.schema.trait_name, method.name,
                ));
            }
            if method
                .calling_plan_commitment
                .is_some_and(BoundaryCallingPlanCommitment::is_zero)
            {
                errors.push(format!(
                    "plan `{}` schema method `{}::{}` retains an empty calling-plan commitment",
                    self.name, self.schema.trait_name, method.name,
                ));
            }
            if method.name.is_empty() {
                errors.push(format!(
                    "plan `{}` schema `{}` has a method with no readable drift name",
                    self.name, self.schema.trait_name,
                ));
            }
            if method.requirement_owner.is_empty() {
                errors.push(format!(
                    "plan `{}` schema method `{}::{}` has no exact requirement owner",
                    self.name, self.schema.trait_name, method.name
                ));
            }
            if method.requirement_identity.is_empty() {
                errors.push(format!(
                    "plan `{}` schema method `{}::{}` has no exact requirement identity",
                    self.name, self.schema.trait_name, method.name
                ));
            } else if let Some(previous_index) = self.schema.methods[..method_index]
                .iter()
                .position(|previous| previous.requirement_identity == method.requirement_identity)
            {
                errors.push(format!(
                    "plan `{}` schema methods at indexes {previous_index} and {method_index} repeat exact requirement identity `{}`",
                    self.name, method.requirement_identity,
                ));
            }
            if method.parameter_type_identities.len() != method.parameter_count {
                errors.push(format!(
                    "plan `{}` schema method `{}::{}` declares {} parameters but retains {} exact parameter type identities",
                    self.name,
                    self.schema.trait_name,
                    method.name,
                    method.parameter_count,
                    method.parameter_type_identities.len(),
                ));
            }
            for (parameter_index, identity) in method.parameter_type_identities.iter().enumerate() {
                if identity.is_empty() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` parameter {} has no exact semantic type identity",
                        self.name,
                        self.schema.trait_name,
                        method.name,
                        parameter_index,
                    ));
                }
            }
            for premise in &method.termination_premises {
                if !method.terminates_guarantee {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` retains a progress premise without a termination guarantee",
                        self.name, self.schema.trait_name, method.name,
                    ));
                }
                if premise.profile.is_empty() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` has a progress premise with no exact profile identity",
                        self.name, self.schema.trait_name, method.name,
                    ));
                }
                if premise.establishment_routes.is_empty() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` progress premise `{}` has no authorized establishment route",
                        self.name,
                        self.schema.trait_name,
                        method.name,
                        premise.profile,
                    ));
                }
                if premise
                    .establishment_routes
                    .iter()
                    .any(|route| route.requirement_identity.is_empty())
                {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` progress premise `{}` has an establishment route with no exact requirement identity",
                        self.name,
                        self.schema.trait_name,
                        method.name,
                        premise.profile,
                    ));
                }
                if premise.establishment_routes.iter().any(|route| {
                    route.kind != ServiceProgressEstablishmentRouteKind::BoundaryRequirement
                }) {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` progress premise `{}` has a non-boundary establishment route",
                        self.name,
                        self.schema.trait_name,
                        method.name,
                        premise.profile,
                    ));
                }
                let mut normalized_routes = premise.establishment_routes.clone();
                normalized_routes.sort();
                normalized_routes.dedup();
                if normalized_routes.len() != premise.establishment_routes.len() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` progress premise `{}` repeats an establishment route",
                        self.name,
                        self.schema.trait_name,
                        method.name,
                        premise.profile,
                    ));
                }
                if let ServiceProgressSubject::Parameter(index) = premise.subject
                    && index >= method.parameter_count
                {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` progress premise names out-of-range parameter {index} of {}",
                        self.name,
                        self.schema.trait_name,
                        method.name,
                        method.parameter_count,
                    ));
                }
                if premise
                    .subject_projections
                    .iter()
                    .any(|projection| projection.is_empty())
                {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` progress premise has an empty subject projection identity",
                        self.name, self.schema.trait_name, method.name,
                    ));
                }
            }
            for claim in &method.entry_claims {
                if claim.parameter_index >= method.parameter_count {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` entry claim names out-of-range parameter {} of {}",
                        self.name,
                        self.schema.trait_name,
                        method.name,
                        claim.parameter_index,
                        method.parameter_count,
                    ));
                }
                if claim.domain.is_empty() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` entry claim for parameter {} has no exact semantic domain identity",
                        self.name,
                        self.schema.trait_name,
                        method.name,
                        claim.parameter_index,
                    ));
                }
                if claim.carrier_identity.is_empty() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` entry claim for parameter {} has no exact carrier identity",
                        self.name,
                        self.schema.trait_name,
                        method.name,
                        claim.parameter_index,
                    ));
                }
                if claim.effective_carry != psi_language_semantics::CarryPolicy::STRICT {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` entry claim for parameter {} and domain `{}` is not born-strict",
                        self.name,
                        self.schema.trait_name,
                        method.name,
                        claim.parameter_index,
                        claim.domain,
                    ));
                }
            }
            for (index, pair) in method.entry_claims.windows(2).enumerate() {
                let left = (
                    pair[0].parameter_index,
                    pair[0].carrier_identity.as_str(),
                    pair[0].domain.as_str(),
                );
                let right = (
                    pair[1].parameter_index,
                    pair[1].carrier_identity.as_str(),
                    pair[1].domain.as_str(),
                );
                if left >= right {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` entry claims are not strictly increasing at indexes {index} and {}",
                        self.name,
                        self.schema.trait_name,
                        method.name,
                        index + 1,
                    ));
                }
            }
            if method.has_result != method.result_type_identity.is_some() {
                errors.push(format!(
                    "plan `{}` schema method `{}::{}` result presence disagrees with its exact result type identity",
                    self.name, self.schema.trait_name, method.name,
                ));
            }
            if method
                .result_type_identity
                .as_ref()
                .is_some_and(|identity| identity.is_empty())
            {
                errors.push(format!(
                    "plan `{}` schema method `{}::{}` result has no exact semantic type identity",
                    self.name, self.schema.trait_name, method.name,
                ));
            }
            for claim in &method.result_claims {
                if !method.has_result || method.result_type_identity.is_none() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` retains a result claim without a real result",
                        self.name, self.schema.trait_name, method.name,
                    ));
                }
                if claim.domain.is_empty() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` result claim has no exact semantic domain identity",
                        self.name, self.schema.trait_name, method.name,
                    ));
                }
                if claim.effective_carry != psi_language_semantics::CarryPolicy::STRICT {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` result claim for domain `{}` is not born-strict",
                        self.name, self.schema.trait_name, method.name, claim.domain,
                    ));
                }
            }
            for (index, pair) in method.result_claims.windows(2).enumerate() {
                if pair[0].domain.as_str() >= pair[1].domain.as_str() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` result claims are not strictly increasing at indexes {index} and {}",
                        self.name,
                        self.schema.trait_name,
                        method.name,
                        index + 1,
                    ));
                }
            }
            for (axis, identities) in [
                ("service-reach", method.service_reach.as_slice()),
                (
                    "synchronous-invocation",
                    method.synchronous_invocations.as_slice(),
                ),
            ] {
                for (index, identity) in identities.iter().enumerate() {
                    if identity.is_empty() {
                        errors.push(format!(
                            "plan `{}` schema method `{}::{}` {axis} identity at index {index} is empty",
                            self.name, self.schema.trait_name, method.name,
                        ));
                    }
                }
                for (index, pair) in identities.windows(2).enumerate() {
                    if pair[0] >= pair[1] {
                        errors.push(format!(
                            "plan `{}` schema method `{}::{}` {axis} identities are not strictly increasing at indexes {index} and {}",
                            self.name,
                            self.schema.trait_name,
                            method.name,
                            index + 1,
                        ));
                    }
                }
            }
        }
        for row in &self.rows {
            if row.requirement_identity.is_empty() {
                errors.push(format!(
                    "plan `{}` row `{}` has no exact requirement identity",
                    self.name, row.method
                ));
            }
            match &row.binding {
                ProviderBinding::Import { locator } => {
                    if self.target != locator.target().target_name() {
                        errors.push(format!(
                            "plan `{}` row `{}` normalized import targets `{}`, but the provider plan targets `{}`",
                            self.name,
                            row.method,
                            locator.target().target_name(),
                            self.target,
                        ));
                    }
                }
                ProviderBinding::StringBackedImportBootstrap { library, symbol } => {
                    if library.is_empty() {
                        errors.push(format!(
                            "plan `{}` row `{}` import has no exact library identity",
                            self.name, row.method,
                        ));
                    }
                    if symbol.is_empty() {
                        errors.push(format!(
                            "plan `{}` row `{}` import has no exact symbol identity",
                            self.name, row.method,
                        ));
                    }
                }
                ProviderBinding::Syscall { number } => {
                    if u32::try_from(*number).is_err() {
                        errors.push(format!(
                            "provider binding `{}::{}` has syscall number {number}, but the target syscall plan requires a value in 0..={}",
                            self.schema.trait_name,
                            row.method,
                            u32::MAX,
                        ));
                    }
                }
                ProviderBinding::CompilerIntrinsic { machine } => {
                    if machine.is_empty() {
                        errors.push(format!(
                            "plan `{}` row `{}` compiler intrinsic has no exact realization-machine identity",
                            self.name, row.method,
                        ));
                    }
                }
                ProviderBinding::VtableSlot { index } => {
                    if *index < 0 {
                        errors.push(format!(
                            "plan `{}` row `{}` vtable slot index {index} is negative",
                            self.name, row.method,
                        ));
                    }
                }
                ProviderBinding::VtableField { table, field }
                | ProviderBinding::TableFunction { table, field } => {
                    if table.is_empty() {
                        errors.push(format!(
                            "external leaf for `{}::{}` uses a table field without an attached provider data type; declare it as `machine TableType::leaf(...) satisfies {}::{} via Binding::...`",
                            self.schema.trait_name,
                            row.method,
                            self.schema.trait_name,
                            row.method,
                        ));
                    } else if self.provider_type.is_empty() {
                        errors.push(format!(
                            "plan `{}` row `{}` table binding has no nominal provider type",
                            self.name, row.method,
                        ));
                    } else if table != &self.provider_type {
                        errors.push(format!(
                            "plan `{}` row `{}` table owner `{table}` does not match nominal provider type `{}`",
                            self.name, row.method, self.provider_type,
                        ));
                    }
                    if field.is_empty() {
                        errors.push(format!(
                            "plan `{}` row `{}` table binding has no exact field identity",
                            self.name, row.method,
                        ));
                    }
                }
                ProviderBinding::CheckedAdapter {
                    machine_identity,
                    machine_package_identity,
                } => {
                    if machine_identity.is_empty() {
                        errors.push(format!(
                            "plan `{}` row `{}` checked adapter has no exact machine identity",
                            self.name, row.method,
                        ));
                    }
                    if self.provider_type.is_empty() {
                        errors.push(format!(
                            "checked adapter `{machine_identity}` for `{}::{}` has no nominal provider type; attach it as an ordinary checked machine satisfying {}::{}` and select that provider for the boundary slot",
                            self.schema.trait_name,
                            row.method,
                            self.schema.trait_name,
                            row.method,
                        ));
                    }
                    if *machine_package_identity != self.origin_package_identity {
                        errors.push(format!(
                            "plan `{}` row `{}` checked-adapter package identity does not match the realizing package",
                            self.name, row.method,
                        ));
                    }
                }
            }
        }
        for method in &self.schema.methods {
            let count = self
                .rows
                .iter()
                .filter(|row| self.schema.row_binds_method(row, method))
                .count();
            if count > 1 {
                errors.push(format!(
                    "plan `{}` binds `{}::{}` {count} times; one row per method",
                    self.name, self.schema.trait_name, method.name
                ));
            }
        }
        for row in &self.rows {
            if !self
                .schema
                .methods
                .iter()
                .any(|method| self.schema.row_binds_method(row, method))
            {
                errors.push(format!(
                    "plan `{}` binds `{}`, which is not a `{}` method",
                    self.name, row.method, self.schema.trait_name
                ));
            }
        }
        errors
    }

    /// PRV2 preview (the cheapest structural fact, used by tests today):
    /// every schema method has exactly one row and every row names a
    /// schema method.
    pub fn covers_schema(&self) -> bool {
        self.schema.methods.iter().all(|method| {
            self.rows
                .iter()
                .filter(|row| self.schema.row_binds_method(row, method))
                .count()
                == 1
        }) && self.rows.iter().all(|row| {
            self.schema
                .methods
                .iter()
                .any(|method| self.schema.row_binds_method(row, method))
        })
    }
}

struct ProviderPlanDigestEncoder(Sha256);

impl ProviderPlanDigestEncoder {
    fn new() -> Self {
        let mut digest = Sha256::new();
        digest.update(b"omega.provider-plan.sha256.v1\0");
        Self(digest)
    }

    fn finish(self) -> ProviderPlanDigest {
        ProviderPlanDigest::from_digest(self.0.finalize().into())
    }

    fn byte(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn bool(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    fn len(&mut self, value: usize) {
        self.0.update((value as u64).to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.update(value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.0.update(value.to_le_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.len(bytes.len());
        self.0.update(bytes);
    }

    fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn strings(&mut self, values: &[String]) {
        self.len(values.len());
        for value in values {
            self.string(value);
        }
    }

    fn package_identity(&mut self, identity: Option<psi_core::PackageKeyIdentity>) {
        match identity {
            Some(identity) => {
                self.byte(1);
                self.0.update(identity.digest());
            }
            None => self.byte(0),
        }
    }

    fn carry_policy(&mut self, policy: psi_language_semantics::CarryPolicy) {
        self.byte(match policy.suspension {
            psi_language_semantics::CarrySuspension::Forbidden => 0,
            psi_language_semantics::CarrySuspension::Allowed => 1,
        });
        self.byte(match policy.cpu {
            psi_language_semantics::CarryCpu::Origin => 0,
            psi_language_semantics::CarryCpu::Any => 1,
        });
        self.byte(match policy.host_thread {
            psi_language_semantics::CarryHostThread::Origin => 0,
            psi_language_semantics::CarryHostThread::Any => 1,
        });
        self.byte(match policy.address {
            psi_language_semantics::CarryAddress::Stable => 0,
            psi_language_semantics::CarryAddress::Movable => 1,
        });
    }

    fn service_method(&mut self, method: &ServiceMethod) {
        self.string(&method.name);
        self.string(&method.requirement_owner);
        self.package_identity(method.requirement_owner_package_identity);
        self.string(&method.requirement_identity);
        self.len(method.parameter_count);
        self.strings(&method.parameter_type_identities);

        let mut entry_claims = method.entry_claims.iter().collect::<Vec<_>>();
        entry_claims.sort_by(|left, right| {
            left.parameter_index
                .cmp(&right.parameter_index)
                .then_with(|| left.carrier_identity.cmp(&right.carrier_identity))
                .then_with(|| left.domain.cmp(&right.domain))
        });
        self.len(entry_claims.len());
        for claim in entry_claims {
            self.len(claim.parameter_index);
            self.string(&claim.carrier_identity);
            self.string(&claim.domain);
            self.byte(match claim.predicate_body {
                psi_language_semantics::DomainPredicateBody::Bodyless => 0,
                psi_language_semantics::DomainPredicateBody::Present => 1,
            });
            self.carry_policy(claim.effective_carry);
            self.byte(match claim.authority_flow {
                ServiceEntryAuthorityFlow::Accepts => 0,
            });
        }

        self.bool(method.has_result);
        match &method.result_type_identity {
            Some(identity) => {
                self.byte(1);
                self.string(identity);
            }
            None => self.byte(0),
        }
        let mut result_claims = method.result_claims.iter().collect::<Vec<_>>();
        result_claims.sort_by(|left, right| left.domain.cmp(&right.domain));
        self.len(result_claims.len());
        for claim in result_claims {
            self.string(&claim.domain);
            self.carry_policy(claim.effective_carry);
        }

        self.strings(&method.service_reach);
        self.strings(&method.synchronous_invocations);
        self.bool(method.may_suspend);
        self.bool(method.may_block);
        self.bool(method.terminates_guarantee);

        let mut premises = method.termination_premises.iter().collect::<Vec<_>>();
        premises.sort_by(|left, right| {
            left.profile
                .cmp(&right.profile)
                .then_with(|| left.subject.cmp(&right.subject))
                .then_with(|| left.subject_projections.cmp(&right.subject_projections))
        });
        self.len(premises.len());
        for premise in premises {
            self.string(&premise.profile);
            match premise.subject {
                ServiceProgressSubject::ProviderReceiver => self.byte(0),
                ServiceProgressSubject::Parameter(index) => {
                    self.byte(1);
                    self.len(index);
                }
            }
            self.strings(&premise.subject_projections);
            let mut routes = premise.establishment_routes.iter().collect::<Vec<_>>();
            routes.sort();
            self.len(routes.len());
            for route in routes {
                self.byte(match route.kind {
                    ServiceProgressEstablishmentRouteKind::CheckedRequirement => 0,
                    ServiceProgressEstablishmentRouteKind::BoundaryRequirement => 1,
                });
                self.string(&route.requirement_identity);
            }
        }

        match method.calling_plan_report_fingerprint {
            Some(fingerprint) => {
                self.byte(1);
                self.u64(fingerprint);
            }
            None => self.byte(0),
        }
        match method.calling_plan_commitment {
            Some(commitment) => {
                self.byte(1);
                self.bytes(&commitment.as_bytes());
            }
            None => self.byte(0),
        }
    }

    fn provider_binding(&mut self, binding: &ProviderBinding) {
        match binding {
            ProviderBinding::Import { locator } => {
                self.byte(0);
                self.string(locator.target().target_name());
                match locator.locator() {
                    omega_target::ForeignLocatorCandidate::PeByName { library, export } => {
                        self.byte(0);
                        self.bytes(library);
                        self.bytes(export);
                    }
                    omega_target::ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
                        self.byte(1);
                        self.bytes(library);
                        self.0.update(ordinal.to_le_bytes());
                    }
                    omega_target::ForeignLocatorCandidate::ElfVersioned {
                        object,
                        symbol,
                        version,
                    } => {
                        self.byte(2);
                        self.bytes(object);
                        self.bytes(symbol);
                        self.bytes(version);
                    }
                }
            }
            ProviderBinding::StringBackedImportBootstrap { library, symbol } => {
                self.byte(1);
                self.string(library);
                self.string(symbol);
            }
            ProviderBinding::Syscall { number } => {
                self.byte(2);
                self.i64(*number);
            }
            ProviderBinding::CompilerIntrinsic { machine } => {
                self.byte(3);
                self.string(machine);
            }
            ProviderBinding::VtableSlot { index } => {
                self.byte(4);
                self.i64(*index);
            }
            ProviderBinding::VtableField { table, field } => {
                self.byte(5);
                self.string(table);
                self.string(field);
            }
            ProviderBinding::TableFunction { table, field } => {
                self.byte(6);
                self.string(table);
                self.string(field);
            }
            ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } => {
                self.byte(7);
                self.string(machine_identity);
                self.package_identity(*machine_package_identity);
            }
        }
    }
}

impl ServiceSchema {
    /// Match a provider row to one exact requirement. The readable method name
    /// must agree as a drift check, but only the canonical overload identity
    /// selects the requirement.
    pub fn row_binds_method(&self, row: &ProviderPlanRow, method: &ServiceMethod) -> bool {
        !row.requirement_identity.is_empty()
            && row.method == method.name
            && row.requirement_identity == method.requirement_identity
    }

    pub fn method_for_row(&self, row: &ProviderPlanRow) -> Option<&ServiceMethod> {
        self.methods
            .iter()
            .find(|method| self.row_binds_method(row, method))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalized_windows_import(library: &[u8], export: &[u8]) -> ProviderBinding {
        ProviderBinding::Import {
            locator: crate::normalize_foreign_locator(
                crate::ForeignLocatorCandidate::PeByName {
                    library: library.to_vec(),
                    export: export.to_vec(),
                },
                omega_target::TargetProfile::WindowsX64,
            )
            .expect("valid normalized Windows import"),
        }
    }

    /// The built-in Console lowering, spelled as a ProviderPlan value --
    /// the PRV4 relocation target (windows.rs insert_platform_lowering's
    /// rows as data). Construction is free; nothing consumes this yet.
    fn windows_console_plan() -> ProviderPlan {
        let schema = ServiceSchema {
            trait_name: "Console".to_owned(),
            trait_package_identity: None,
            methods: vec![
                ServiceMethod {
                    name: "write_line".to_owned(),
                    requirement_owner: "Console".to_owned(),
                    requirement_owner_package_identity: None,
                    requirement_identity: "Console::write_line".to_owned(),
                    parameter_count: 1,
                    parameter_type_identities: vec!["String".to_owned()],
                    entry_claims: Vec::new(),
                    has_result: false,
                    result_type_identity: None,
                    result_claims: Vec::new(),
                    service_reach: vec!["Console".to_owned()],
                    synchronous_invocations: Vec::new(),
                    may_suspend: false,
                    may_block: false,
                    terminates_guarantee: false,
                    termination_premises: Vec::new(),
                    calling_plan_report_fingerprint: None,
                    calling_plan_commitment: None,
                },
                ServiceMethod {
                    name: "read_byte".to_owned(),
                    requirement_owner: "Console".to_owned(),
                    requirement_owner_package_identity: None,
                    requirement_identity: "Console::read_byte".to_owned(),
                    parameter_count: 0,
                    parameter_type_identities: Vec::new(),
                    entry_claims: Vec::new(),
                    has_result: true,
                    result_type_identity: Some("u8".to_owned()),
                    result_claims: Vec::new(),
                    service_reach: vec!["Console".to_owned()],
                    synchronous_invocations: Vec::new(),
                    may_suspend: true,
                    may_block: false,
                    terminates_guarantee: false,
                    termination_premises: Vec::new(),
                    calling_plan_report_fingerprint: None,
                    calling_plan_commitment: None,
                },
                ServiceMethod {
                    name: "exit_process".to_owned(),
                    requirement_owner: "Console".to_owned(),
                    requirement_owner_package_identity: None,
                    requirement_identity: "Console::exit_process".to_owned(),
                    parameter_count: 1,
                    parameter_type_identities: vec!["i32".to_owned()],
                    entry_claims: Vec::new(),
                    has_result: false,
                    result_type_identity: None,
                    result_claims: Vec::new(),
                    service_reach: vec!["Console".to_owned()],
                    synchronous_invocations: Vec::new(),
                    may_suspend: false,
                    may_block: true,
                    terminates_guarantee: false,
                    termination_premises: Vec::new(),
                    calling_plan_report_fingerprint: None,
                    calling_plan_commitment: None,
                },
            ],
        };
        ProviderPlan {
            name: "omega::host::standard::console".to_owned(),
            provider_type: "StandardConsole".to_owned(),
            provider_type_package_identity: None,
            target: "windows_x64".to_owned(),
            schema,
            rows: vec![
                ProviderPlanRow {
                    method: "write_line".to_owned(),
                    requirement_identity: "Console::write_line".to_owned(),
                    binding: ProviderBinding::StringBackedImportBootstrap {
                        library: "kernel32.dll".to_owned(),
                        symbol: "WriteFile".to_owned(),
                    },
                },
                ProviderPlanRow {
                    method: "read_byte".to_owned(),
                    requirement_identity: "Console::read_byte".to_owned(),
                    binding: ProviderBinding::StringBackedImportBootstrap {
                        library: "kernel32.dll".to_owned(),
                        symbol: "ReadFile".to_owned(),
                    },
                },
                ProviderPlanRow {
                    method: "exit_process".to_owned(),
                    requirement_identity: "Console::exit_process".to_owned(),
                    binding: ProviderBinding::StringBackedImportBootstrap {
                        library: "kernel32.dll".to_owned(),
                        symbol: "ExitProcess".to_owned(),
                    },
                },
            ],
            origin_package_identity: None,
            origin_package: "omega::language::std".to_owned(),
        }
    }

    #[test]
    fn evaluated_calling_plan_is_published_provider_identity() {
        let mut first = windows_console_plan();
        let baseline = first.identity_fingerprint();
        first.schema.methods[0].calling_plan_report_fingerprint = Some(0x1234);
        first.schema.methods[0].calling_plan_commitment =
            Some(psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest([1; 32]));
        assert_ne!(baseline, first.identity_fingerprint());

        let mut refactored = first.clone();
        refactored.schema.methods[0].calling_plan_report_fingerprint = Some(0x1234);
        assert_eq!(
            first.identity_fingerprint(),
            refactored.identity_fingerprint()
        );

        refactored.schema.methods[0].calling_plan_commitment =
            Some(psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest([2; 32]));
        assert_eq!(
            first.identity_fingerprint(),
            refactored.identity_fingerprint()
        );
        assert_ne!(first.identity_digest(), refactored.identity_digest());
    }

    #[test]
    fn provider_candidate_rejects_an_empty_calling_plan_commitment() {
        let mut plan = windows_console_plan();
        plan.schema.methods[0].calling_plan_report_fingerprint = Some(0x1234);
        plan.schema.methods[0].calling_plan_commitment =
            Some(BoundaryCallingPlanCommitment::from_digest([0; 32]));

        let diagnostics = plan.validate_candidate_against_schema();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("empty calling-plan commitment")),
            "unexpected diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn strong_provider_plan_identity_rejects_compact_equal_structural_substitution() {
        let original = windows_console_plan();
        let mut substituted = original.clone();
        substituted.schema.methods[0].requirement_owner = "OtherConsole".to_owned();

        assert_eq!(
            original.identity_fingerprint(),
            substituted.identity_fingerprint(),
            "the legacy compact renderer did not retain the readable requirement-owner field"
        );
        assert_ne!(original, substituted);
        assert_ne!(original.identity_digest(), substituted.identity_digest());
    }

    #[test]
    fn exact_package_provenance_enters_provider_identity_but_legacy_label_does_not() {
        let mut first = windows_console_plan();
        first.origin_package_identity = psi_core::PackageKeyIdentity::from_digest([1; 32]);
        let first_identity = first.identity_fingerprint();

        let mut renamed_label = first.clone();
        renamed_label.origin_package = "misleading display label".to_owned();
        assert_eq!(renamed_label.identity_fingerprint(), first_identity);

        let mut second = first;
        second.origin_package_identity = psi_core::PackageKeyIdentity::from_digest([2; 32]);
        assert_ne!(second.identity_fingerprint(), first_identity);

        let mut provider_type_owner = renamed_label.clone();
        provider_type_owner.provider_type_package_identity =
            psi_core::PackageKeyIdentity::from_digest([3; 32]);
        assert_ne!(provider_type_owner.identity_fingerprint(), first_identity);

        let mut schema_owner = renamed_label.clone();
        schema_owner.schema.trait_package_identity =
            psi_core::PackageKeyIdentity::from_digest([4; 32]);
        assert_ne!(schema_owner.identity_fingerprint(), first_identity);

        let mut requirement_owner = renamed_label;
        requirement_owner.schema.methods[0].requirement_owner_package_identity =
            psi_core::PackageKeyIdentity::from_digest([5; 32]);
        assert_ne!(requirement_owner.identity_fingerprint(), first_identity);

        let mut unbound_adapter = windows_console_plan();
        unbound_adapter.rows[0].binding = ProviderBinding::CheckedAdapter {
            machine_identity: "named-callable(path(ConsoleProvider::write))".to_owned(),
            machine_package_identity: None,
        };
        let unbound_identity = unbound_adapter.identity_fingerprint();
        let mut bound_adapter = unbound_adapter.clone();
        let adapter_package =
            psi_core::PackageKeyIdentity::from_digest([6; 32]).expect("nonzero package identity");
        bound_adapter.origin_package_identity = Some(adapter_package);
        bound_adapter.rows[0].binding = ProviderBinding::CheckedAdapter {
            machine_identity: "named-callable(path(ConsoleProvider::write))".to_owned(),
            machine_package_identity: Some(adapter_package),
        };
        assert_ne!(bound_adapter.identity_fingerprint(), unbound_identity);

        let mut other_overload = unbound_adapter;
        other_overload.rows[0].binding = ProviderBinding::CheckedAdapter {
            machine_identity: "named-callable(path(ConsoleProvider::write))#other".to_owned(),
            machine_package_identity: None,
        };
        assert_ne!(other_overload.identity_fingerprint(), unbound_identity);
    }

    #[test]
    fn independent_operational_ceilings_enter_provider_identity() {
        let baseline = windows_console_plan();
        let baseline_identity = baseline.identity_fingerprint();

        let mut suspending = baseline.clone();
        suspending.schema.methods[0].may_suspend = true;
        assert_ne!(suspending.identity_fingerprint(), baseline_identity);

        let mut blocking = baseline;
        blocking.schema.methods[0].may_block = true;
        assert_ne!(blocking.identity_fingerprint(), baseline_identity);

        let mut terminating = windows_console_plan();
        terminating.schema.methods[0].terminates_guarantee = true;
        assert_ne!(terminating.identity_fingerprint(), baseline_identity);

        let mut premised = terminating.clone();
        premised.schema.methods[0].termination_premises = vec![ServiceProgressPremise {
            profile: "SchedulerHandle::WeakFair".to_owned(),
            subject: ServiceProgressSubject::Parameter(0),
            subject_projections: Vec::new(),
            establishment_routes: vec![ServiceProgressEstablishmentRoute {
                kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                requirement_identity: "SchedulerAdmission::grant#exact".to_owned(),
            }],
        }];
        assert_ne!(
            premised.identity_fingerprint(),
            terminating.identity_fingerprint()
        );
        let mut changed_route = premised.clone();
        changed_route.schema.methods[0].termination_premises[0].establishment_routes[0]
            .requirement_identity = "SchedulerAdmission::grant_strong#exact".to_owned();
        assert_ne!(
            premised.identity_fingerprint(),
            changed_route.identity_fingerprint(),
            "the authorized establishment route must enter provider identity"
        );
        let mut two_routes = premised.clone();
        two_routes.schema.methods[0].termination_premises[0]
            .establishment_routes
            .push(ServiceProgressEstablishmentRoute {
                kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                requirement_identity: "SchedulerAdmission::accept_weak#exact".to_owned(),
            });
        let mut reversed_routes = two_routes.clone();
        reversed_routes.schema.methods[0].termination_premises[0]
            .establishment_routes
            .reverse();
        assert_eq!(
            two_routes.identity_fingerprint(),
            reversed_routes.identity_fingerprint(),
            "route declaration order is presentation, not provider identity"
        );
        assert_ne!(
            suspending.identity_fingerprint(),
            blocking.identity_fingerprint()
        );
        assert_ne!(
            terminating.identity_fingerprint(),
            suspending.identity_fingerprint()
        );
        assert_ne!(
            terminating.identity_fingerprint(),
            blocking.identity_fingerprint()
        );
    }

    #[test]
    fn progress_schema_rejects_missing_repeated_and_non_boundary_routes() {
        let mut plan = windows_console_plan();
        plan.schema.methods[0].terminates_guarantee = true;
        plan.schema.methods[0].termination_premises = vec![ServiceProgressPremise {
            profile: "SchedulerHandle::WeakFair".to_owned(),
            subject: ServiceProgressSubject::Parameter(0),
            subject_projections: Vec::new(),
            establishment_routes: Vec::new(),
        }];
        assert!(
            plan.validate_against_schema()
                .iter()
                .any(|error| error.contains("has no authorized establishment route"))
        );

        let route = ServiceProgressEstablishmentRoute {
            kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
            requirement_identity: "SchedulerAdmission::grant#exact".to_owned(),
        };
        plan.schema.methods[0].termination_premises[0].establishment_routes =
            vec![route.clone(), route];
        assert!(
            plan.validate_against_schema()
                .iter()
                .any(|error| error.contains("repeats an establishment route"))
        );

        plan.schema.methods[0].termination_premises[0].establishment_routes =
            vec![ServiceProgressEstablishmentRoute {
                kind: ServiceProgressEstablishmentRouteKind::CheckedRequirement,
                requirement_identity: "SchedulerAdmission::grant#exact".to_owned(),
            }];
        assert!(
            plan.validate_against_schema()
                .iter()
                .any(|error| error.contains("non-boundary establishment route"))
        );
    }

    #[test]
    fn normalized_parameter_and_result_types_enter_provider_identity() {
        let baseline = windows_console_plan();
        let baseline_identity = baseline.identity_fingerprint();

        let mut qualified_parameter = baseline.clone();
        qualified_parameter.schema.methods[0].parameter_type_identities[0] =
            "InterruptAcknowledgement in InterruptAcknowledgement::Pending".to_owned();
        assert_ne!(
            qualified_parameter.identity_fingerprint(),
            baseline_identity
        );

        let mut changed_result = baseline;
        changed_result.schema.methods[1].result_type_identity = Some("u16".to_owned());
        assert_ne!(changed_result.identity_fingerprint(), baseline_identity);
        assert_ne!(
            qualified_parameter.identity_fingerprint(),
            changed_result.identity_fingerprint()
        );
    }

    #[test]
    fn structured_entry_claims_enter_provider_identity() {
        let baseline = windows_console_plan();
        let mut accepted = baseline.clone();
        accepted.schema.methods[0].entry_claims = vec![ServiceEntryClaim {
            parameter_index: 0,
            carrier_identity: "named(name(InterruptAcknowledgement))".to_owned(),
            domain: "InterruptAcknowledgement::Pending".to_owned(),
            predicate_body: psi_language_semantics::DomainPredicateBody::Bodyless,
            effective_carry: psi_language_semantics::CarryPolicy::STRICT,
            authority_flow: ServiceEntryAuthorityFlow::Accepts,
        }];

        assert_ne!(
            accepted.identity_fingerprint(),
            baseline.identity_fingerprint(),
            "the receipt identity must bind structured accepted authority, not only display types"
        );

        let mut redirected_carrier = accepted.clone();
        redirected_carrier.schema.methods[0].entry_claims[0].carrier_identity =
            "named(name(OtherAcknowledgement))".to_owned();
        assert_ne!(
            accepted.identity_fingerprint(),
            redirected_carrier.identity_fingerprint(),
            "the routed qualification's exact carrier is provider-plan identity"
        );

        let mut relaxed = accepted.clone();
        relaxed.schema.methods[0].entry_claims[0].effective_carry =
            psi_language_semantics::CarryPolicy::PERMISSIVE;
        assert_ne!(
            accepted.identity_fingerprint(),
            relaxed.identity_fingerprint(),
            "the compiler-owned entry carry policy is receipt identity"
        );

        let mut predicate_bearing = accepted.clone();
        predicate_bearing.schema.methods[0].entry_claims[0].predicate_body =
            psi_language_semantics::DomainPredicateBody::Present;
        assert_ne!(
            accepted.identity_fingerprint(),
            predicate_bearing.identity_fingerprint(),
            "predicate discharge is part of the selected provider contract"
        );
    }

    #[test]
    fn console_plan_constructs_and_covers_its_schema() {
        let plan = windows_console_plan();
        assert!(plan.covers_schema());
    }

    #[test]
    fn validation_names_every_structural_defect() {
        // PRV2: missing binding, stray row, and duplicate binding each produce
        // a NAMED error.
        let mut plan = windows_console_plan();
        plan.rows.remove(0);
        plan.rows.push(ProviderPlanRow {
            method: "not_a_method".to_owned(),
            requirement_identity: "Console::not_a_method".to_owned(),
            binding: ProviderBinding::Syscall { number: 1 },
        });
        plan.rows.push(ProviderPlanRow {
            method: "exit_process".to_owned(),
            requirement_identity: "Console::exit_process".to_owned(),
            binding: ProviderBinding::Syscall { number: 0 },
        });
        let errors = plan.validate_against_schema();
        assert!(
            errors
                .iter()
                .any(|error| error.contains("does not bind `Console::write_line`")),
            "missing binding named: {errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("not a `Console` method")),
            "stray row named: {errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("binds `Console::exit_process` 2 times")),
            "duplicate named: {errors:?}"
        );
        assert!(windows_console_plan().validate_against_schema().is_empty());
    }

    #[test]
    fn coverage_detects_missing_and_stray_rows() {
        let mut plan = windows_console_plan();
        plan.rows.pop();
        assert!(
            plan.validate_candidate_against_schema().is_empty(),
            "a partial candidate is structurally valid before slot selection"
        );
        assert!(
            !plan.covers_schema(),
            "a missing method row must fail coverage"
        );

        let mut plan = windows_console_plan();
        plan.rows.push(ProviderPlanRow {
            method: "not_in_schema".to_owned(),
            requirement_identity: "Console::not_in_schema".to_owned(),
            binding: ProviderBinding::VtableSlot { index: 0 },
        });
        assert!(
            !plan.validate_candidate_against_schema().is_empty(),
            "a stray row is invalid even before coverage selection"
        );
        assert!(!plan.covers_schema(), "a stray row must fail coverage");
    }

    #[test]
    fn result_overloaded_requirements_bind_by_exact_identity() {
        let mut plan = windows_console_plan();
        let template = plan.schema.methods[0].clone();
        plan.schema.trait_name = "Convert".to_owned();
        plan.schema.methods = vec![
            ServiceMethod {
                name: "convert".to_owned(),
                requirement_owner: "Convert".to_owned(),
                requirement_identity: "Convert::convert(i32)->i32".to_owned(),
                ..template.clone()
            },
            ServiceMethod {
                name: "convert".to_owned(),
                requirement_owner: "Convert".to_owned(),
                requirement_identity: "Convert::convert(i32)->i32 in Saturating".to_owned(),
                ..template
            },
        ];
        plan.rows = vec![
            ProviderPlanRow {
                method: "convert".to_owned(),
                requirement_identity: plan.schema.methods[0].requirement_identity.clone(),
                binding: ProviderBinding::Syscall { number: 1 },
            },
            ProviderPlanRow {
                method: "convert".to_owned(),
                requirement_identity: plan.schema.methods[1].requirement_identity.clone(),
                binding: ProviderBinding::Syscall { number: 2 },
            },
        ];

        assert!(plan.covers_schema());
        assert!(plan.validate_against_schema().is_empty());

        plan.rows[1].requirement_identity = plan.rows[0].requirement_identity.clone();
        assert!(!plan.covers_schema());
        assert!(
            plan.validate_against_schema()
                .iter()
                .any(|error| error.contains("binds `Convert::convert` 2 times")),
            "duplicating one overload identity must not cover the other: {:?}",
            plan.validate_against_schema()
        );

        plan.rows[1].requirement_identity.clear();
        assert!(
            !plan.covers_schema(),
            "a human name cannot select an overload"
        );
    }

    #[test]
    fn schema_validation_requires_explicit_owner_without_using_it_for_selection() {
        let mut plan = windows_console_plan();
        plan.schema.trait_name = "DerivedConsole".to_owned();

        assert!(
            plan.validate_candidate_against_schema().is_empty(),
            "an inherited requirement owner may differ from its selected schema"
        );
        assert!(plan.covers_schema());

        plan.schema.methods[0].requirement_owner.clear();
        let errors = plan.validate_candidate_against_schema();
        assert!(errors.iter().any(|error| {
            error.contains(
                "schema method `DerivedConsole::write_line` has no exact requirement owner",
            )
        }));
        assert!(
            plan.covers_schema(),
            "readable owner metadata must not replace canonical overload selection"
        );

        plan.schema.methods[0].requirement_owner = "Console".to_owned();
        plan.schema.methods[0].requirement_identity.clear();
        assert!(!plan.covers_schema());
        let errors = plan.validate_candidate_against_schema();
        assert!(errors.iter().any(|error| {
            error.contains(
                "schema method `DerivedConsole::write_line` has no exact requirement identity",
            )
        }));
    }

    #[test]
    fn schema_validation_requires_unique_exact_requirement_identities() {
        let mut valid = windows_console_plan();
        valid.schema.methods[0].name = "operation".to_owned();
        valid.rows[0].method = "operation".to_owned();
        valid.schema.methods[0].requirement_owner = "BaseConsole".to_owned();
        valid.schema.methods[1].name = "operation".to_owned();
        valid.rows[1].method = "operation".to_owned();
        valid.schema.methods[1].requirement_owner = "DerivedConsole".to_owned();
        assert!(
            valid.validate_candidate_against_schema().is_empty(),
            "duplicate readable names and inherited differing owners remain valid when exact overload identities differ"
        );

        let mut same_label = windows_console_plan();
        same_label
            .schema
            .methods
            .push(same_label.schema.methods[0].clone());
        assert!(
            same_label.covers_schema(),
            "one row previously appeared to cover two identical schema methods"
        );
        assert!(
            same_label
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error
                    .contains("repeat exact requirement identity `Console::write_line`"))
        );

        let mut different_label = windows_console_plan();
        let mut duplicate_identity = different_label.schema.methods[0].clone();
        duplicate_identity.name = "renamed_operation".to_owned();
        duplicate_identity.requirement_owner = "OtherConsole".to_owned();
        different_label.schema.methods.push(duplicate_identity);
        assert!(
            different_label
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error
                    .contains("repeat exact requirement identity `Console::write_line`"))
        );
    }

    #[test]
    fn schema_validation_rejects_malformed_qualification_subjects_independently() {
        let valid_claim = ServiceEntryClaim {
            parameter_index: 0,
            carrier_identity: "named(name(Token))".to_owned(),
            domain: "Token::Granted".to_owned(),
            predicate_body: psi_language_semantics::DomainPredicateBody::Bodyless,
            effective_carry: psi_language_semantics::CarryPolicy::STRICT,
            authority_flow: ServiceEntryAuthorityFlow::Accepts,
        };
        let mut valid = windows_console_plan();
        valid.schema.methods[0].entry_claims = vec![valid_claim.clone()];
        assert!(valid.validate_candidate_against_schema().is_empty());

        let mut missing_parameter_type = valid.clone();
        missing_parameter_type.schema.methods[0]
            .parameter_type_identities
            .clear();
        assert!(
            missing_parameter_type
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("declares 1 parameters but retains 0 exact parameter"))
        );

        let mut out_of_range = valid.clone();
        out_of_range.schema.methods[0].entry_claims[0].parameter_index = 1;
        assert!(
            out_of_range
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("out-of-range parameter 1 of 1"))
        );

        let mut empty_entry_domain = valid.clone();
        empty_entry_domain.schema.methods[0].entry_claims[0]
            .domain
            .clear();
        assert!(
            empty_entry_domain
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error
                    .contains("entry claim for parameter 0 has no exact semantic domain"))
        );

        let mut empty_entry_carrier = valid.clone();
        empty_entry_carrier.schema.methods[0].entry_claims[0]
            .carrier_identity
            .clear();
        assert!(
            empty_entry_carrier
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("has no exact carrier identity"))
        );

        let mut result_presence_mismatch = valid.clone();
        result_presence_mismatch.schema.methods[0].has_result = true;
        assert!(
            result_presence_mismatch
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("result presence disagrees"))
        );

        let mut claim_without_result = valid.clone();
        claim_without_result.schema.methods[0].result_claims = vec![ServiceResultClaim {
            domain: "Token::Issued".to_owned(),
            effective_carry: psi_language_semantics::CarryPolicy::STRICT,
        }];
        assert!(
            claim_without_result
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("result claim without a real result"))
        );

        let mut empty_result_domain = valid;
        empty_result_domain.schema.methods[1].result_claims = vec![ServiceResultClaim {
            domain: String::new(),
            effective_carry: psi_language_semantics::CarryPolicy::STRICT,
        }];
        assert!(
            empty_result_domain
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("result claim has no exact semantic domain"))
        );
    }

    #[test]
    fn schema_validation_requires_nonempty_semantic_type_identities() {
        let mut valid = windows_console_plan();
        valid.schema.methods[0].entry_claims = vec![ServiceEntryClaim {
            parameter_index: 0,
            carrier_identity: "named(name(Token))".to_owned(),
            domain: "Token::Granted".to_owned(),
            predicate_body: psi_language_semantics::DomainPredicateBody::Bodyless,
            effective_carry: psi_language_semantics::CarryPolicy::STRICT,
            authority_flow: ServiceEntryAuthorityFlow::Accepts,
        }];
        valid.schema.methods[1].result_claims = vec![ServiceResultClaim {
            domain: "Token::Issued".to_owned(),
            effective_carry: psi_language_semantics::CarryPolicy::STRICT,
        }];
        assert!(
            valid.validate_candidate_against_schema().is_empty(),
            "exact type identities and independently retained domains are orthogonal"
        );

        let mut blank_parameter = valid.clone();
        blank_parameter.schema.methods[0].parameter_type_identities[0].clear();
        assert!(
            blank_parameter
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("parameter 0 has no exact semantic type identity"))
        );

        let mut blank_result = valid;
        blank_result.schema.methods[1]
            .result_type_identity
            .as_mut()
            .expect("read_byte has a real result")
            .clear();
        assert!(
            blank_result
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("result has no exact semantic type identity"))
        );
    }

    #[test]
    fn schema_validation_requires_canonical_born_strict_claims() {
        fn entry_claim(
            parameter_index: usize,
            domain: &str,
            predicate_body: psi_language_semantics::DomainPredicateBody,
        ) -> ServiceEntryClaim {
            ServiceEntryClaim {
                parameter_index,
                carrier_identity: "named(name(Token))".to_owned(),
                domain: domain.to_owned(),
                predicate_body,
                effective_carry: psi_language_semantics::CarryPolicy::STRICT,
                authority_flow: ServiceEntryAuthorityFlow::Accepts,
            }
        }

        let mut valid = windows_console_plan();
        valid.schema.methods[0].parameter_count = 2;
        valid.schema.methods[0].parameter_type_identities =
            vec!["FirstToken".to_owned(), "SecondToken".to_owned()];
        valid.schema.methods[0].entry_claims = vec![
            entry_claim(
                0,
                "Domain::Alpha",
                psi_language_semantics::DomainPredicateBody::Bodyless,
            ),
            entry_claim(
                0,
                "Domain::Beta",
                psi_language_semantics::DomainPredicateBody::Present,
            ),
            entry_claim(
                1,
                "Domain::Alpha",
                psi_language_semantics::DomainPredicateBody::Bodyless,
            ),
        ];
        valid.schema.methods[1].result_claims = vec![
            ServiceResultClaim {
                domain: "Domain::Alpha".to_owned(),
                effective_carry: psi_language_semantics::CarryPolicy::STRICT,
            },
            ServiceResultClaim {
                domain: "Domain::Beta".to_owned(),
                effective_carry: psi_language_semantics::CarryPolicy::STRICT,
            },
        ];
        assert!(
            valid.validate_candidate_against_schema().is_empty(),
            "canonical claims allow multiple domains per parameter and the same domain at different positions"
        );
        assert!(
            windows_console_plan()
                .validate_candidate_against_schema()
                .is_empty(),
            "empty claim vectors remain valid"
        );

        let mut permissive_entry = valid.clone();
        permissive_entry.schema.methods[0].entry_claims[0].effective_carry =
            psi_language_semantics::CarryPolicy::PERMISSIVE;
        assert!(
            permissive_entry
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("entry claim") && error.contains("not born-strict"))
        );

        let mut permissive_result = valid.clone();
        permissive_result.schema.methods[1].result_claims[0].effective_carry =
            psi_language_semantics::CarryPolicy::PERMISSIVE;
        assert!(
            permissive_result
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("result claim") && error.contains("not born-strict"))
        );

        let mut duplicate_entry = valid.clone();
        duplicate_entry.schema.methods[0].entry_claims[1] =
            duplicate_entry.schema.methods[0].entry_claims[0].clone();
        assert!(
            duplicate_entry
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("entry claims are not strictly increasing"))
        );

        let mut out_of_order_entry = valid.clone();
        out_of_order_entry.schema.methods[0].entry_claims.swap(0, 1);
        assert!(
            out_of_order_entry
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("entry claims are not strictly increasing"))
        );

        let mut duplicate_result = valid.clone();
        duplicate_result.schema.methods[1].result_claims[1] =
            duplicate_result.schema.methods[1].result_claims[0].clone();
        assert!(
            duplicate_result
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("result claims are not strictly increasing"))
        );

        let mut out_of_order_result = valid;
        out_of_order_result.schema.methods[1]
            .result_claims
            .swap(0, 1);
        assert!(
            out_of_order_result
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("result claims are not strictly increasing"))
        );
    }

    #[test]
    fn schema_validation_requires_canonical_independent_service_axes() {
        let mut valid = windows_console_plan();
        valid.schema.methods[0].service_reach = vec!["Console".to_owned(), "Storage".to_owned()];
        valid.schema.methods[0].synchronous_invocations =
            vec!["Clock".to_owned(), "TaskRuntime".to_owned()];
        valid.schema.methods[2].service_reach.clear();
        assert!(
            valid.validate_candidate_against_schema().is_empty(),
            "reach and direct invocation are distinct canonical sets, and either may be empty"
        );

        let mut empty_reach = valid.clone();
        empty_reach.schema.methods[0].service_reach[0].clear();
        assert!(
            empty_reach
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("service-reach identity at index 0 is empty"))
        );

        let mut duplicate_reach = valid.clone();
        duplicate_reach.schema.methods[0].service_reach =
            vec!["Console".to_owned(), "Console".to_owned()];
        assert!(duplicate_reach
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("service-reach identities are not strictly increasing")));

        let mut out_of_order_reach = valid.clone();
        out_of_order_reach.schema.methods[0].service_reach =
            vec!["Storage".to_owned(), "Console".to_owned()];
        assert!(out_of_order_reach
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("service-reach identities are not strictly increasing")));

        let mut empty_invocation = valid.clone();
        empty_invocation.schema.methods[0].synchronous_invocations[0].clear();
        assert!(
            empty_invocation
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("synchronous-invocation identity at index 0 is empty"))
        );

        let mut duplicate_invocation = valid.clone();
        duplicate_invocation.schema.methods[0].synchronous_invocations =
            vec!["Clock".to_owned(), "Clock".to_owned()];
        assert!(
            duplicate_invocation
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error
                    .contains("synchronous-invocation identities are not strictly increasing"))
        );

        let mut out_of_order_invocation = valid;
        out_of_order_invocation.schema.methods[0].synchronous_invocations =
            vec!["TaskRuntime".to_owned(), "Clock".to_owned()];
        assert!(
            out_of_order_invocation
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error
                    .contains("synchronous-invocation identities are not strictly increasing"))
        );
    }

    #[test]
    fn schema_validation_requires_selection_and_readable_names() {
        let mut valid = windows_console_plan();
        valid.target.clear();
        valid.provider_type.clear();
        valid.schema.trait_name = "DerivedConsole".to_owned();
        valid.schema.methods[0].requirement_owner = "BaseConsole".to_owned();
        valid.schema.methods[0].name = "operation".to_owned();
        valid.rows[0].method = "operation".to_owned();
        valid.schema.methods[1].name = "operation".to_owned();
        valid.rows[1].method = "operation".to_owned();
        assert!(
            valid.validate_candidate_against_schema().is_empty(),
            "free universal plans, inherited owners, and duplicate readable overload names remain valid"
        );
        assert!(
            valid.covers_schema(),
            "exact overload identity still selects"
        );

        let mut blank_plan = valid.clone();
        blank_plan.name.clear();
        assert!(
            blank_plan
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("no exact selection name"))
        );

        let mut blank_schema = valid.clone();
        blank_schema.schema.trait_name.clear();
        assert!(
            blank_schema
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("no exact boundary-slot identity"))
        );

        let mut blank_method = valid;
        blank_method.schema.methods[0].name.clear();
        blank_method.rows[0].method.clear();
        assert!(
            blank_method.covers_schema(),
            "a matching blank label still proves why a separate presence fence is required"
        );
        assert!(
            blank_method
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("method with no readable drift name"))
        );
    }

    #[test]
    fn schema_validation_requires_canonical_binding_payloads() {
        fn plan_with_binding(binding: ProviderBinding) -> ProviderPlan {
            let mut plan = windows_console_plan();
            plan.rows[0].binding = binding;
            plan
        }

        let valid_bindings = [
            normalized_windows_import(b"kernel32.dll", b"WriteFile"),
            ProviderBinding::StringBackedImportBootstrap {
                library: "kernel32.dll".to_owned(),
                symbol: "WriteFile".to_owned(),
            },
            ProviderBinding::Syscall { number: 0 },
            ProviderBinding::Syscall {
                number: i64::from(u32::MAX),
            },
            ProviderBinding::CompilerIntrinsic {
                machine: "Console::write_line".to_owned(),
            },
            ProviderBinding::VtableSlot { index: 0 },
            ProviderBinding::VtableField {
                table: "StandardConsole".to_owned(),
                field: "write_line".to_owned(),
            },
            ProviderBinding::TableFunction {
                table: "StandardConsole".to_owned(),
                field: "write_line".to_owned(),
            },
            ProviderBinding::CheckedAdapter {
                machine_identity: "write_line_adapter".to_owned(),
                machine_package_identity: None,
            },
        ];
        for binding in valid_bindings {
            assert!(
                plan_with_binding(binding)
                    .validate_candidate_against_schema()
                    .is_empty(),
                "every closed binding family accepts its exact canonical payload"
            );
        }

        let mut wrong_target =
            plan_with_binding(normalized_windows_import(b"kernel32.dll", b"WriteFile"));
        wrong_target.target = "linux_x64".to_owned();
        assert!(
            wrong_target
                .validate_candidate_against_schema()
                .iter()
                .any(|error| error.contains("normalized import targets `windows_x64`"))
        );

        for binding in [
            ProviderBinding::StringBackedImportBootstrap {
                library: "kernel32.dll".to_owned(),
                symbol: "WriteFile".to_owned(),
            },
            ProviderBinding::Syscall { number: 0 },
            ProviderBinding::CompilerIntrinsic {
                machine: "Console::write_line".to_owned(),
            },
            ProviderBinding::VtableSlot { index: 0 },
        ] {
            let mut free = plan_with_binding(binding);
            free.target.clear();
            free.provider_type.clear();
            assert!(
                free.validate_candidate_against_schema().is_empty(),
                "irreducible non-table leaves remain valid without a target or nominal provider type"
            );
        }

        let corruptions = [
            (
                ProviderBinding::StringBackedImportBootstrap {
                    library: String::new(),
                    symbol: "WriteFile".to_owned(),
                },
                "import has no exact library identity",
            ),
            (
                ProviderBinding::StringBackedImportBootstrap {
                    library: "kernel32.dll".to_owned(),
                    symbol: String::new(),
                },
                "import has no exact symbol identity",
            ),
            (ProviderBinding::Syscall { number: -1 }, "syscall number -1"),
            (
                ProviderBinding::Syscall {
                    number: i64::from(u32::MAX) + 1,
                },
                "target syscall plan requires a value in 0..=4294967295",
            ),
            (
                ProviderBinding::CompilerIntrinsic {
                    machine: String::new(),
                },
                "compiler intrinsic has no exact realization-machine identity",
            ),
            (
                ProviderBinding::VtableSlot { index: -1 },
                "vtable slot index -1 is negative",
            ),
            (
                ProviderBinding::VtableField {
                    table: String::new(),
                    field: "write_line".to_owned(),
                },
                "without an attached provider data type",
            ),
            (
                ProviderBinding::VtableField {
                    table: "StandardConsole".to_owned(),
                    field: String::new(),
                },
                "table binding has no exact field identity",
            ),
            (
                ProviderBinding::VtableField {
                    table: "OtherConsole".to_owned(),
                    field: "write_line".to_owned(),
                },
                "table owner `OtherConsole` does not match nominal provider type `StandardConsole`",
            ),
            (
                ProviderBinding::TableFunction {
                    table: String::new(),
                    field: "write_line".to_owned(),
                },
                "without an attached provider data type",
            ),
            (
                ProviderBinding::TableFunction {
                    table: "StandardConsole".to_owned(),
                    field: String::new(),
                },
                "table binding has no exact field identity",
            ),
            (
                ProviderBinding::TableFunction {
                    table: "OtherConsole".to_owned(),
                    field: "write_line".to_owned(),
                },
                "table owner `OtherConsole` does not match nominal provider type `StandardConsole`",
            ),
            (
                ProviderBinding::CheckedAdapter {
                    machine_identity: String::new(),
                    machine_package_identity: None,
                },
                "checked adapter has no exact machine identity",
            ),
        ];
        for (binding, expected) in corruptions {
            let errors = plan_with_binding(binding).validate_candidate_against_schema();
            assert!(
                errors.iter().any(|error| error.contains(expected)),
                "missing `{expected}` in {errors:?}"
            );
        }

        for binding in [
            ProviderBinding::VtableField {
                table: "StandardConsole".to_owned(),
                field: "write_line".to_owned(),
            },
            ProviderBinding::TableFunction {
                table: "StandardConsole".to_owned(),
                field: "write_line".to_owned(),
            },
        ] {
            let mut free_table = plan_with_binding(binding);
            free_table.provider_type.clear();
            let errors = free_table.validate_candidate_against_schema();
            assert!(
                errors
                    .iter()
                    .any(|error| error.contains("table binding has no nominal provider type")),
                "missing nominal table-owner rejection in {errors:?}"
            );
        }

        let mut free_adapter = plan_with_binding(ProviderBinding::CheckedAdapter {
            machine_identity: "write_line_adapter".to_owned(),
            machine_package_identity: None,
        });
        free_adapter.provider_type.clear();
        let errors = free_adapter.validate_candidate_against_schema();
        assert!(
            errors
                .iter()
                .any(|error| error.contains("has no nominal provider type")),
            "missing nominal-provider rejection in {errors:?}"
        );
    }

    #[test]
    fn normalized_import_identity_enters_provider_plan_identity_atomically() {
        let mut baseline = windows_console_plan();
        baseline.rows[0].binding = normalized_windows_import(b"kernel32.dll", b"WriteFile");
        let baseline_identity = baseline.identity_fingerprint();

        let mut changed_library = baseline.clone();
        changed_library.rows[0].binding =
            normalized_windows_import(b"kernelbase.dll", b"WriteFile");
        assert_ne!(baseline_identity, changed_library.identity_fingerprint());

        let mut changed_export = baseline.clone();
        changed_export.rows[0].binding = normalized_windows_import(b"kernel32.dll", b"ReadFile");
        assert_ne!(baseline_identity, changed_export.identity_fingerprint());
    }
}
