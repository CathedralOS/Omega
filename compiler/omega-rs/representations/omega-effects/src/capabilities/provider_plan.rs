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

/// The service schema a plan serves: a boundary trait's callable surface,
/// reified from the typed `TraitDefinition` (today that read is scattered
/// -- parameter-count walks in the compiler pipeline, Console detection in
/// the interpreter; the schema type is the one honest carrier).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceSchema {
    /// The boundary trait's name (`Console`, `FilesystemHost`).
    pub trait_name: String,
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
    /// Canonical validated `BoundaryEntryPlan` identity selected by a concrete
    /// `Calling<C>` relationship. Policy type/source identity is excluded.
    pub calling_plan_fingerprint: Option<u64>,
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
    /// Dynamic-library import (`DllImport { module, symbol }`).
    Import { library: String, symbol: String },
    /// Direct system call by number.
    Syscall { number: u32 },
    /// A compiler-known operation furnished by the selected target package.
    /// The name is canonical `BoundaryTrait::method` identity; ABI planning
    /// validates that the target already owns the matching lowering.
    CompilerIntrinsic { name: String },
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
    CheckedAdapter { machine: String },
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
/// target. `origin_package` is provenance INPUT to admission (a package
/// can never self-grant); the admission verdict itself lives in the
/// chapter-10 receipts, never here.
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
    /// The target this plan serves (`windows_x64`; empty = every target).
    pub target: String,
    /// The schema served.
    pub schema: ServiceSchema,
    /// One row per bound method.
    pub rows: Vec<ProviderPlanRow>,
    /// Where the plan came from -- admission provenance input.
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
            methods: vec![ServiceMethod {
                name: "realize".to_owned(),
                requirement_owner:
                    psi_typed_trees::operator::boundary_operator_requirement_identity(
                        program, operator,
                    ),
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
                calling_plan_fingerprint: None,
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
        methods.push(ServiceMethod {
            name: signature.name.as_str().to_owned(),
            requirement_owner: trait_definition.name.as_str().to_owned(),
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
            calling_plan_fingerprint: program.boundary_calling_plan_fingerprint_for_arguments(
                policy_owner,
                boundary_arguments,
                signature.symbol,
            ),
        });
    }
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
            .then_with(|| left.domain.cmp(&right.domain))
    });
    claims.dedup_by(|left, right| {
        left.parameter_index == right.parameter_index && left.domain == right.domain
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

impl ProviderPlan {
    /// PRV2: the plan's NORMALIZED IDENTITY -- an FNV-1a fingerprint over
    /// the canonical rendering (name, target, schema surface, rows in
    /// method order). Two plans with the same fingerprint are the same
    /// policy; presentation (row order, whitespace) is excluded.
    pub fn identity_fingerprint(&self) -> u64 {
        let mut rendered = format!(
            "{}\n{}\n{}\n{}",
            self.name, self.provider_type, self.target, self.schema.trait_name
        );
        let mut methods: Vec<&ServiceMethod> = self.schema.methods.iter().collect();
        methods.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.requirement_identity.cmp(&right.requirement_identity))
        });
        for method in methods {
            rendered.push_str(&format!(
                "\nm:{}/{}/{}/{}/services:{}/invokes:{}/suspend:{}/block:{}",
                method.name,
                method.requirement_identity,
                method.parameter_count,
                method.has_result,
                method.service_reach.join("+"),
                method.synchronous_invocations.join("+"),
                method.may_suspend,
                method.may_block,
            ));
            for parameter in &method.parameter_type_identities {
                rendered.push_str("\nmp:");
                rendered.push_str(parameter);
            }
            let mut entry_claims = method.entry_claims.iter().collect::<Vec<_>>();
            entry_claims.sort_by(|left, right| {
                left.parameter_index
                    .cmp(&right.parameter_index)
                    .then_with(|| left.domain.cmp(&right.domain))
            });
            for claim in entry_claims {
                rendered.push_str(&format!(
                    "\nmc:{}/{}/{}/{}/{}",
                    claim.parameter_index,
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
            if let Some(fingerprint) = method.calling_plan_fingerprint {
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
            rendered.push_str(&format!(
                "\nr:{}/{}/{:?}",
                row.method, row.requirement_identity, row.binding
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
        for method in &self.schema.methods {
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

    /// The built-in Console lowering, spelled as a ProviderPlan value --
    /// the PRV4 relocation target (windows.rs insert_platform_lowering's
    /// rows as data). Construction is free; nothing consumes this yet.
    fn windows_console_plan() -> ProviderPlan {
        let schema = ServiceSchema {
            trait_name: "Console".to_owned(),
            methods: vec![
                ServiceMethod {
                    name: "write_line".to_owned(),
                    requirement_owner: "Console".to_owned(),
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
                    calling_plan_fingerprint: None,
                },
                ServiceMethod {
                    name: "read_byte".to_owned(),
                    requirement_owner: "Console".to_owned(),
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
                    calling_plan_fingerprint: None,
                },
                ServiceMethod {
                    name: "exit_process".to_owned(),
                    requirement_owner: "Console".to_owned(),
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
                    calling_plan_fingerprint: None,
                },
            ],
        };
        ProviderPlan {
            name: "omega::host::standard::console".to_owned(),
            provider_type: "StandardConsole".to_owned(),
            target: "windows_x64".to_owned(),
            schema,
            rows: vec![
                ProviderPlanRow {
                    method: "write_line".to_owned(),
                    requirement_identity: "Console::write_line".to_owned(),
                    binding: ProviderBinding::Import {
                        library: "kernel32.dll".to_owned(),
                        symbol: "WriteFile".to_owned(),
                    },
                },
                ProviderPlanRow {
                    method: "read_byte".to_owned(),
                    requirement_identity: "Console::read_byte".to_owned(),
                    binding: ProviderBinding::Import {
                        library: "kernel32.dll".to_owned(),
                        symbol: "ReadFile".to_owned(),
                    },
                },
                ProviderPlanRow {
                    method: "exit_process".to_owned(),
                    requirement_identity: "Console::exit_process".to_owned(),
                    binding: ProviderBinding::Import {
                        library: "kernel32.dll".to_owned(),
                        symbol: "ExitProcess".to_owned(),
                    },
                },
            ],
            origin_package: "omega::language::std".to_owned(),
        }
    }

    #[test]
    fn evaluated_calling_plan_is_published_provider_identity() {
        let mut first = windows_console_plan();
        let baseline = first.identity_fingerprint();
        first.schema.methods[0].calling_plan_fingerprint = Some(0x1234);
        assert_ne!(baseline, first.identity_fingerprint());

        let mut refactored = first.clone();
        refactored.schema.methods[0].calling_plan_fingerprint = Some(0x1234);
        assert_eq!(
            first.identity_fingerprint(),
            refactored.identity_fingerprint()
        );
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
        assert_ne!(
            suspending.identity_fingerprint(),
            blocking.identity_fingerprint()
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
    fn schema_validation_rejects_malformed_qualification_subjects_independently() {
        let valid_claim = ServiceEntryClaim {
            parameter_index: 0,
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
}
