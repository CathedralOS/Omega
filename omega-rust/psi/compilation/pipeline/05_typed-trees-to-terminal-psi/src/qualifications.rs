//! Publish the declared meaning of each qualification once, and intern the
//! complete sets used by lowering. Source handles are construction coordinates;
//! only exact application identities and module-local domain IDs leave this stage.

use semantic_vocabulary::{
    DomainSemanticId, MachineId, ScalarDomainId, ScalarQualificationSetId, ScalarType,
    StructuralDomainId, StructuralTypeId,
};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{
    NominalDeclarationIdentity, ScalarDomainDeclaration, ScalarDomainEstablishmentRoute,
    ScalarDomainPredicate, ScalarQualificationCatalog, ScalarQualificationSet,
    StructuralDomainDeclaration, StructuralDomainPredicate, StructuralEstablishmentRoute,
    StructuralQualificationChange, TerminalIdentityIndex, TerminalIdentityTable,
    TerminalIdentityTableBuilder,
};

use crate::body::{DomainInstance, DomainSet, Policy, Qualification, Regions, ValueType};
use crate::{Unserved, refuse};

pub(crate) fn domain_identity(
    typed: &typed_trees::TypedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    instance: DomainInstance,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<terminal_psi::TerminalIdentityIndex, &'static str> {
    let declaration = crate::declare::nominal(typed, sources, instance.declaration, identities)?;
    let exact = identities
        .text(declaration)
        .ok_or("domain application lost its retained declaration identity")?;
    let identity = typed.domain_application_identity(
        instance.declaration,
        instance.identity,
        exact,
        sources,
    )?;
    Ok(identities.intern(&identity))
}

#[derive(Default)]
pub(crate) struct Qualifications {
    pub catalog: ScalarQualificationCatalog,
    pub identities: TerminalIdentityTableBuilder,
    domains: Vec<(DomainInstance, ScalarDomainId)>,
    policies: Vec<(ScalarType, Policy, ScalarDomainId)>,
    issuers: Vec<(MachineId, ScalarDomainId, u32)>,
    /// The module's structural domain catalog, in identity order.
    pub structural: Vec<StructuralDomainDeclaration>,
    /// Each used record domain by its exact application and catalog identity.
    structural_domains: Vec<(DomainInstance, StructuralDomainId)>,
    structural_issuers: Vec<(MachineId, StructuralDomainId, u32)>,
    /// Explicit structural casts, in machine and edge order.
    pub structural_changes: Vec<StructuralQualificationChange>,
    /// The module's scalar applications. Every machine's lowering shares
    /// it, so one layout question about one row is one application however
    /// many bodies ask it (`layout_observation`).
    pub scalar_applications: terminal_psi::ScalarApplications,
}

impl Qualifications {
    /// The application observing `measurement` of `subject` under the
    /// selected target's validated geometry, published once per module.
    pub(crate) fn layout_observation(
        &mut self,
        subject: semantic_vocabulary::StructuralTypeId,
        measurement: terminal_psi::LayoutMeasurement,
        scalar_type: ScalarType,
    ) -> Result<semantic_vocabulary::ScalarApplicationId, &'static str> {
        let declaration = terminal_psi::ScalarApplication::LayoutObservation {
            subject,
            measurement,
            semantics_version: terminal_psi::LayoutMeasurement::SEMANTICS_VERSION,
            scalar_type,
        };
        if let Some(position) = self
            .scalar_applications
            .declarations()
            .iter()
            .position(|existing| *existing == declaration)
        {
            return semantic_vocabulary::ScalarApplicationId::new(position as u64 + 1)
                .ok_or("scalar application namespace is exhausted");
        }
        self.scalar_applications.insert(declaration)
    }
}

/// Which declared route an issuer restriction names.
#[derive(Clone, Copy)]
enum RouteKind {
    ExactMachine,
    CheckedRequirement,
    BoundaryRequirement,
}

/// A domain's issuer restriction in canonical order, and each body that may
/// issue through it with the ordinal of its route there. `None` is no
/// restriction; `Some` of nothing permits nobody. One builder serves both
/// domain families, so a scalar and a record domain written the same way
/// have the same routes.
fn establishment_routes<R: Ord>(
    typed: &typed_trees::TypedTrees,
    source: &crate::read::Program<'_>,
    sources: &[(source::SourceId, [u8; 32])],
    builder: &mut TerminalIdentityTableBuilder,
    definition: &typed_trees::domain::DomainDefinition,
    unavailable: &impl Fn(&'static str) -> terminal_check_tables::Refusal,
    make: impl Fn(RouteKind, NominalDeclarationIdentity, TerminalIdentityIndex) -> R,
) -> Checked<Option<(Vec<R>, Vec<(MachineId, u32)>)>> {
    let routes = match definition.issuance() {
        typed_trees::domain::DomainIssuance::Unrestricted => return Ok(None),
        typed_trees::domain::DomainIssuance::Planned(_) => {
            return Err(unavailable(
                "domain access plan has no closed issuance declaration",
            ));
        }
        typed_trees::domain::DomainIssuance::Only(routes) => routes,
    };
    let mut retained = Vec::with_capacity(routes.len());
    let mut issuers = Vec::new();
    for route in routes {
        let ordinal = u32::try_from(retained.len())
            .map_err(|_| unavailable("domain issuer ordinal overflow"))?;
        match route {
            language_semantics::DomainEstablishmentRoute::ExactMachine { machine } => {
                issuers.extend(
                    source.symbols.iter().filter_map(|(body, symbol)| {
                        (*symbol == *machine).then_some((body, ordinal))
                    }),
                );
                let declaration = crate::declare::semantic_identity(typed, *machine, builder)
                    .map_err(unavailable)?;
                let machine = typed
                    .machine_by_symbol(*machine)
                    .ok_or_else(|| unavailable("domain issuer has no declaration"))?;
                let callable = typed
                    .machine_callable_identity_text(machine)
                    .filter(|text| !text.is_empty())
                    .ok_or_else(|| unavailable("domain issuer has no callable identity"))?;
                retained.push(make(
                    RouteKind::ExactMachine,
                    NominalDeclarationIdentity::new(declaration),
                    builder.intern(&callable),
                ));
            }
            language_semantics::DomainEstablishmentRoute::CheckedRequirement {
                trait_definition,
                requirement,
            }
            | language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                boundary_trait: trait_definition,
                requirement,
            } => {
                let boundary = matches!(
                    route,
                    language_semantics::DomainEstablishmentRoute::BoundaryRequirement { .. }
                );
                let (owner, declared) = typed
                    .trait_machine_signature(*requirement)
                    .filter(|(owner, _)| {
                        owner.symbol == *trait_definition && owner.is_boundary == boundary
                    })
                    .ok_or_else(|| {
                        unavailable("domain route lost its exact requirement declaration")
                    })?;
                let callable = typed.trait_requirement_callable_identity_text(owner, declared);
                if callable.is_empty() {
                    return Err(unavailable("domain requirement has no callable identity"));
                }
                let declaration = NominalDeclarationIdentity::new(
                    crate::declare::nominal(typed, sources, *requirement, builder)
                        .map_err(unavailable)?,
                );
                let requirement_identity = builder.intern(&callable);
                if boundary {
                    retained.push(make(
                        RouteKind::BoundaryRequirement,
                        declaration,
                        requirement_identity,
                    ));
                } else {
                    for (body, symbol) in source.symbols.iter() {
                        if typed.machine_by_symbol(*symbol).is_some_and(|machine| {
                            typed
                                .machine_trait_conformances(machine)
                                .iter()
                                .any(|edge| {
                                    edge.symbol == *trait_definition
                                        && edge.requirement_symbol == *requirement
                                })
                        }) {
                            issuers.push((body, ordinal));
                        }
                    }
                    retained.push(make(
                        RouteKind::CheckedRequirement,
                        declaration,
                        requirement_identity,
                    ));
                }
            }
        }
    }
    let mut ordered: Vec<_> = retained.into_iter().enumerate().collect();
    ordered.sort_unstable_by(|(_, left), (_, right)| left.cmp(right));
    if ordered.windows(2).any(|pair| pair[0].1 == pair[1].1) {
        return Err(unavailable("domain repeats an establishment route"));
    }
    for (_, ordinal) in &mut issuers {
        *ordinal = u32::try_from(
            ordered
                .iter()
                .position(|(original, _)| *original == *ordinal as usize)
                .ok_or_else(|| unavailable("domain issuer order lost its route"))?,
        )
        .map_err(|_| unavailable("domain issuer ordinal overflow"))?;
    }
    Ok(Some((
        ordered.into_iter().map(|(_, route)| route).collect(),
        issuers,
    )))
}

impl Qualifications {
    pub fn declare(
        diagnostic: MachineId,
        source: Option<&crate::read::Program<'_>>,
        identities: TerminalIdentityTable,
    ) -> Checked<Self> {
        let mut result = Self {
            identities: identities.into_builder(),
            ..Self::default()
        };
        let Some(source) = source else {
            return Ok(result);
        };
        let typed = source.typed();
        let sources = match source::source_identity::toolchain_source_identities(
            typed.symbols.source_files(),
        ) {
            Ok(sources) => sources,
            // An invocation-local identity is not a portable claim about
            // anonymous source. `nominal` retains that exact local scope.
            Err(_) if result.identities.is_invocation_scoped() => Vec::new(),
            Err(detail) => {
                return Err(terminal_check_tables::Refusal {
                    reason: terminal_check_tables::RefusalReason::Unsupported,
                    machine: diagnostic,
                    concerns: Construct::Machine,
                    source_span: None,
                    detail: format!("scalar domain source identity: {detail}"),
                });
            }
        };
        let unavailable = |detail| {
            refuse(
                diagnostic,
                Construct::Machine,
                Unserved::Declaration(detail),
            )
        };
        for &(symbol, carrier) in &source.declared_domains {
            let carrier = match carrier {
                ValueType::Scalar(carrier, _) => carrier,
                ValueType::Record(carrier, _, regions) => {
                    result.declare_structural(
                        diagnostic, source, &sources, symbol, carrier, regions,
                    )?;
                    continue;
                }
                ValueType::Rigid(_) => {
                    return Err(unavailable("domain retains an unresolved carrier"));
                }
            };
            let builder = &mut result.identities;
            let id = ScalarDomainId::new(result.catalog.domains.len() as u64 + 1)
                .ok_or_else(|| unavailable("scalar domain identity overflow"))?;
            let definition = source.domain_theory(diagnostic, symbol)?;
            let identity =
                domain_identity(typed, &sources, symbol, builder).map_err(unavailable)?;
            let domains = DomainSet::EMPTY
                .with(symbol)
                .ok_or_else(|| unavailable("scalar domain subject"))?;
            let (clauses, _) = source.cast_predicates(
                diagnostic,
                Qualification {
                    policy: None,
                    domains,
                },
                carrier,
            )?;
            let mut predicate = if clauses.is_empty() {
                None
            } else {
                Some(ScalarDomainPredicate {
                    subject: semantic_vocabulary::ValueId::new(1).expect("one is a subject"),
                    proposition: crate::canonical::conjunction(clauses, diagnostic)?,
                })
            };
            let establishment_routes = match establishment_routes(
                typed,
                source,
                &sources,
                builder,
                definition,
                &unavailable,
                |kind, declaration, identity| match kind {
                    RouteKind::ExactMachine => ScalarDomainEstablishmentRoute::ExactMachine {
                        declaration,
                        machine_identity: identity,
                    },
                    RouteKind::CheckedRequirement => {
                        ScalarDomainEstablishmentRoute::CheckedRequirement {
                            declaration,
                            requirement_identity: identity,
                        }
                    }
                    RouteKind::BoundaryRequirement => {
                        ScalarDomainEstablishmentRoute::BoundaryRequirement {
                            declaration,
                            requirement_identity: identity,
                        }
                    }
                },
            )? {
                None => None,
                Some((routes, issuers)) => {
                    result.issuers.extend(
                        issuers
                            .into_iter()
                            .map(|(body, ordinal)| (body, id, ordinal)),
                    );
                    Some(routes)
                }
            };
            if predicate.is_none()
                && establishment_routes
                    .as_ref()
                    .is_some_and(|routes| !routes.is_empty())
            {
                predicate = Some(ScalarDomainPredicate {
                    subject: semantic_vocabulary::ValueId::new(1).expect("one is a subject"),
                    proposition: semantic_vocabulary::Proposition::Truth,
                });
            }
            result.catalog.domains.push(ScalarDomainDeclaration {
                required_memberships: Some(Vec::new()),
                predicate,
                id,
                semantic_domain: DomainSemanticId::new(id.get())
                    .expect("a domain identity is nonzero"),
                identity,
                carrier,
                establishment_routes,
            });
            result.domains.push((symbol, id));
        }
        Ok(result)
    }

    /// A cast's predicate/issuer completion and a result's forwarding retain
    /// their exact values and edges. The checker judges them against the
    /// declared theory and actual callable, independently of the reader.
    pub fn complete(
        &mut self,
        machine: &terminal_psi::TerminalMachine,
        coercions_start: usize,
    ) -> Checked<()> {
        use terminal_psi::{
            ScalarReturnOrigin, ScalarReturnQualification, ScalarReturnSite, Terminator,
        };
        let mut returns = Vec::new();
        let mut values: Vec<_> =
            terminal_semantics::scalar_qualifications::values(machine).collect();
        values.sort_unstable_by_key(|value| value.id);
        let value = |id| {
            values
                .binary_search_by_key(&id, |value| value.id)
                .ok()
                .map(|index| values[index])
        };
        let coercions = self
            .catalog
            .coercions
            .get(coercions_start..)
            .ok_or_else(|| {
                refuse(
                    machine.id,
                    Construct::Machine,
                    Unserved::Type("qualification slice is outside its producer"),
                )
            })?;
        for coercion in coercions {
            if coercion.machine != machine.id {
                return Err(refuse(
                    machine.id,
                    Construct::Edge(coercion.edge),
                    Unserved::Type("qualification slice contains another machine"),
                ));
            }
            let source = value(coercion.source).ok_or_else(|| {
                refuse(
                    machine.id,
                    Construct::Edge(coercion.edge),
                    Unserved::Type("qualification source"),
                )
            })?;
            let destination = value(coercion.destination).ok_or_else(|| {
                refuse(
                    machine.id,
                    Construct::Edge(coercion.edge),
                    Unserved::Type("qualification destination"),
                )
            })?;
            let before = self.members(machine.id, source.qualifications)?;
            for id in self
                .members(machine.id, destination.qualifications)?
                .iter()
                .filter(|id| !before.contains(id))
            {
                let domain = self
                    .catalog
                    .domains
                    .iter()
                    .find(|domain| domain.id == *id)
                    .ok_or_else(|| {
                        refuse(
                            machine.id,
                            Construct::Edge(coercion.edge),
                            Unserved::Type("qualification domain"),
                        )
                    })?;
                let origin = if domain.establishment_routes.is_some() {
                    let route = self
                        .issuers
                        .iter()
                        .find(|(issuer, declared, _)| *issuer == machine.id && *declared == *id)
                        .map(|(_, _, route)| *route)
                        .ok_or_else(|| {
                            refuse(
                                machine.id,
                                Construct::Edge(coercion.edge),
                                Unserved::Type("qualification has no retained issuer association"),
                            )
                        })?;
                    ScalarReturnOrigin::Established { route }
                } else if domain.predicate.is_some() {
                    ScalarReturnOrigin::Proved
                } else {
                    continue;
                };
                returns.push(ScalarReturnQualification {
                    site: ScalarReturnSite::Completion {
                        argument_ordinal: coercion.argument_ordinal,
                    },
                    machine: machine.id,
                    edge: coercion.edge,
                    source: source.id,
                    result: destination.id,
                    domain: *id,
                    origins: vec![origin],
                });
            }
        }
        if let Some(result) = machine.result.scalar_ref() {
            for domain in
                self.catalog
                    .sets
                    .iter()
                    .filter(|set| set.id == result.qualifications)
                    .flat_map(|set| &set.domains)
                    .filter(|id| {
                        self.catalog.domains.iter().any(|domain| {
                            domain.id == **id && domain.establishment_routes.is_some()
                        })
                    })
            {
                for block in &machine.blocks {
                    if let Terminator::Return { edge, value, .. } = block.terminator {
                        returns.push(ScalarReturnQualification {
                            site: ScalarReturnSite::NormalExit,
                            machine: machine.id,
                            edge,
                            source: value,
                            result: result.id,
                            domain: *domain,
                            origins: vec![ScalarReturnOrigin::Forwarded],
                        });
                    }
                }
            }
        }
        returns.sort_unstable_by_key(|row| (row.machine, row.edge, row.domain));
        self.catalog.returns.extend(returns);
        Ok(())
    }

    /// Retain the generic predicate over the domain's own carrier for
    /// declaration formation. Field sites separately prove the same clauses
    /// at their declared storage paths (`RecordType::requires`).
    fn declare_structural(
        &mut self,
        diagnostic: MachineId,
        source: &crate::read::Program<'_>,
        sources: &[(source::SourceId, [u8; 32])],
        symbol: DomainInstance,
        carrier: StructuralTypeId,
        regions: Regions,
    ) -> Checked<()> {
        let typed = source.typed();
        let unavailable = |detail| {
            refuse(
                diagnostic,
                Construct::Machine,
                Unserved::Declaration(detail),
            )
        };
        let definition = source.domain_theory(diagnostic, symbol)?;
        let byte_predicate =
            typed_trees::byte_predicates::byte_slice_domain_predicate(typed, definition)
                .map(|(_, predicate)| predicate);
        let predicate = if byte_predicate.is_none() && crate::read::has_predicates(definition) {
            let domains = DomainSet::EMPTY
                .with(symbol)
                .ok_or_else(|| unavailable("record domain subject"))?;
            let qualification = Qualification {
                policy: None,
                domains,
            };
            let clauses = source.field_predicates(
                diagnostic,
                &[],
                ValueType::Record(carrier, qualification, regions),
            )?;
            Some(StructuralDomainPredicate {
                subject: crate::body::self_place()
                    .ok_or_else(|| unavailable("record domain subject identity"))?,
                proposition: crate::canonical::conjunction(clauses, diagnostic)?,
            })
        } else {
            None
        };
        let id = StructuralDomainId::new(self.structural.len() as u64 + 1)
            .ok_or_else(|| unavailable("record domain identity overflow"))?;
        let identity =
            domain_identity(typed, sources, symbol, &mut self.identities).map_err(unavailable)?;
        // A governed domain's access plan decides who may issue it. An
        // evaluated plan reads as its routes below; a plan nobody has run
        // shows no machine to be an issuer, so the lowered form permits
        // nobody. Its values still pass whole through formals, results and
        // fields.
        let planned = matches!(
            definition.issuance(),
            typed_trees::domain::DomainIssuance::Planned(_)
        );
        let establishment_routes = if planned {
            Some(Vec::new())
        } else {
            match establishment_routes(
                typed,
                source,
                sources,
                &mut self.identities,
                definition,
                &unavailable,
                |kind, declaration, identity| match kind {
                    RouteKind::ExactMachine => StructuralEstablishmentRoute::ExactMachine {
                        declaration,
                        machine_identity: identity,
                    },
                    RouteKind::CheckedRequirement => {
                        StructuralEstablishmentRoute::CheckedRequirement {
                            declaration,
                            requirement_identity: identity,
                        }
                    }
                    RouteKind::BoundaryRequirement => {
                        StructuralEstablishmentRoute::BoundaryRequirement {
                            declaration,
                            requirement_identity: identity,
                        }
                    }
                },
            )? {
                None => None,
                Some((routes, issuers)) => {
                    self.structural_issuers.extend(
                        issuers
                            .into_iter()
                            .map(|(body, ordinal)| (body, id, ordinal)),
                    );
                    Some(routes)
                }
            }
        };
        self.structural.push(StructuralDomainDeclaration {
            required_memberships: Some(Vec::new()),
            complete_predicate_theory: byte_predicate.is_some(),
            predicate,
            id,
            semantic_domain: DomainSemanticId::new(id.get()).expect("a domain identity is nonzero"),
            identity,
            carrier,
            content_projection: None,
            byte_predicate,
            establishment_routes,
        });
        self.structural_domains.push((symbol, id));
        Ok(())
    }

    /// The whole-value domains a record type carries, sorted. A policy has no
    /// meaning on a record.
    pub fn structural(
        &self,
        machine: MachineId,
        carrier: StructuralTypeId,
        qualification: Qualification,
    ) -> Checked<Vec<StructuralDomainId>> {
        let unavailable = |detail| refuse(machine, Construct::Machine, Unserved::Type(detail));
        if qualification.policy.is_some() {
            return Err(unavailable("arithmetic policy on a data type"));
        }
        let mut domains = Vec::with_capacity(DomainSet::CAPACITY);
        for symbol in qualification.domains.members() {
            let id = self
                .structural_domains
                .iter()
                .find(|(declared, _)| *declared == symbol)
                .map(|(_, id)| *id)
                .ok_or_else(|| {
                    unavailable("data qualification has no retained domain declaration")
                })?;
            if self
                .structural
                .iter()
                .find(|domain| domain.id == id)
                .is_none_or(|domain| domain.carrier != carrier)
            {
                return Err(unavailable("data qualification has another carrier"));
            }
            domains.push(id);
        }
        domains.sort_unstable();
        Ok(domains)
    }

    /// The establishment row `machine` owes for each routed domain a cast in
    /// it adds: the route it issues by. A route-free domain needs none.
    pub fn structural_establishments(
        &self,
        machine: MachineId,
        added: &[StructuralDomainId],
    ) -> Checked<Vec<terminal_psi::ResultQualificationEstablishment>> {
        let mut rows = Vec::new();
        for id in added {
            let routed = self
                .structural
                .iter()
                .find(|domain| domain.id == *id)
                .is_some_and(|domain| domain.establishment_routes.is_some());
            if !routed {
                continue;
            }
            let route = self
                .structural_issuers
                .iter()
                .find(|(issuer, domain, _)| *issuer == machine && *domain == *id)
                .map(|(_, _, route)| *route)
                .ok_or_else(|| {
                    refuse(
                        machine,
                        Construct::Machine,
                        Unserved::Type("record cast has no retained issuer association"),
                    )
                })?;
            // A requirement route issues in a machine that declares the edge,
            // and the lowered form carries that edge only as a closed
            // conformance application of a selected call. Without one the
            // checker cannot authenticate the issuer, so the cast is refused
            // by name rather than published as a row it must reject.
            let requirement = self
                .structural
                .iter()
                .find(|domain| domain.id == *id)
                .and_then(|domain| domain.establishment_routes.as_deref())
                .and_then(|routes| routes.get(route as usize))
                .is_some_and(|route| {
                    matches!(
                        route,
                        StructuralEstablishmentRoute::CheckedRequirement { .. }
                    )
                });
            if requirement {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("record domain issued through a requirement route"),
                ));
            }
            rows.push(terminal_psi::ResultQualificationEstablishment { domain: *id, route });
        }
        rows.sort_unstable();
        Ok(rows)
    }

    pub fn members(
        &self,
        machine: MachineId,
        set: ScalarQualificationSetId,
    ) -> Checked<&[ScalarDomainId]> {
        if set.is_empty() {
            return Ok(&[]);
        }
        self.catalog
            .sets
            .iter()
            .find(|row| row.id == set)
            .map(|row| row.domains.as_slice())
            .ok_or_else(|| {
                refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("scalar qualification set has no declaration"),
                )
            })
    }

    pub fn scalar(
        &mut self,
        machine: MachineId,
        carrier: ScalarType,
        qualification: Qualification,
    ) -> Checked<ScalarQualificationSetId> {
        let unavailable = |detail| refuse(machine, Construct::Machine, Unserved::Type(detail));
        if qualification.is_none() {
            return Ok(ScalarQualificationSetId::ZERO);
        }
        let mut domains = Vec::with_capacity(DomainSet::CAPACITY + 1);
        for symbol in qualification.domains.members() {
            let id = self
                .domains
                .iter()
                .find(|(declared, _)| *declared == symbol)
                .map(|(_, id)| *id)
                .ok_or_else(|| {
                    unavailable("scalar qualification has no retained domain declaration")
                })?;
            if self
                .catalog
                .domains
                .iter()
                .find(|domain| domain.id == id)
                .is_none_or(|domain| domain.carrier != carrier)
            {
                return Err(unavailable("scalar qualification has another carrier"));
            }
            domains.push(id);
        }
        if let Some(policy) = qualification.policy {
            let id = match self
                .policies
                .iter()
                .find(|(ty, declared, _)| *ty == carrier && *declared == policy)
            {
                Some((_, _, id)) => *id,
                None => {
                    let ScalarType::Integer(_) = carrier else {
                        return Err(unavailable("arithmetic policy on a noninteger carrier"));
                    };
                    let id = ScalarDomainId::new(self.catalog.domains.len() as u64 + 1)
                        .ok_or_else(|| unavailable("scalar policy identity overflow"))?;
                    let arithmetic = match policy {
                        Policy::Wrapping => numerics::arithmetic::ArithmeticDomain::Wrapping,
                        Policy::Saturating => numerics::arithmetic::ArithmeticDomain::Saturating,
                        Policy::Trapping => {
                            return Err(unavailable("trapping arithmetic policy qualification"));
                        }
                    };
                    let identity =
                        terminal_semantics::scalar_qualifications::arithmetic_policy_identity(
                            arithmetic,
                        )
                        .ok_or_else(|| unavailable("arithmetic policy identity"))?;
                    self.catalog.domains.push(ScalarDomainDeclaration {
                        required_memberships: Some(Vec::new()),
                        predicate: None,
                        id,
                        semantic_domain: DomainSemanticId::new(id.get())
                            .expect("a policy identity is nonzero"),
                        identity: self.identities.intern(identity),
                        carrier,
                        establishment_routes: None,
                    });
                    self.policies.push((carrier, policy, id));
                    id
                }
            };
            domains.push(id);
        }
        domains.sort_unstable();
        if let Some(set) = self.catalog.sets.iter().find(|set| set.domains == domains) {
            return Ok(set.id);
        }
        let id = ScalarQualificationSetId::new(self.catalog.sets.len() as u64 + 1);
        self.catalog
            .sets
            .push(ScalarQualificationSet { id, domains });
        Ok(id)
    }
}
