//! Publish the declared meaning of a scalar qualification once, and intern the
//! complete sets used by lowering. Source handles are construction coordinates;
//! only exact declaration identities and module-local domain IDs leave this stage.

use semantic_vocabulary::{
    DomainSemanticId, MachineId, ScalarDomainId, ScalarQualificationSetId, ScalarType,
};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{
    ScalarDomainDeclaration, ScalarDomainEstablishmentRoute, ScalarDomainPredicate,
    ScalarQualificationCatalog, ScalarQualificationSet, TerminalIdentityTable,
    TerminalIdentityTableBuilder,
};

use crate::body::{DomainSet, Policy, Qualification, ValueType};
use crate::{Unserved, refuse};

#[derive(Default)]
pub(crate) struct Qualifications {
    pub catalog: ScalarQualificationCatalog,
    pub identities: TerminalIdentityTableBuilder,
    semantic_base: u64,
    domains: Vec<(SymbolHandle, ScalarDomainId)>,
    policies: Vec<(ScalarType, Policy, ScalarDomainId)>,
    issuers: Vec<(MachineId, ScalarDomainId, u32)>,
}

impl Qualifications {
    pub fn declare(
        diagnostic: MachineId,
        source: Option<&crate::read::Program<'_>>,
        identities: TerminalIdentityTable,
        semantic_base: u64,
    ) -> Checked<Self> {
        let mut result = Self {
            identities: identities.into_builder(),
            semantic_base,
            ..Self::default()
        };
        let Some(source) = source else {
            return Ok(result);
        };
        let typed = source.typed;
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
        let builder = &mut result.identities;
        let unavailable = |detail| {
            refuse(
                diagnostic,
                Construct::Machine,
                Unserved::Declaration(detail),
            )
        };
        for &(symbol, carrier) in &source.declared_domains {
            let ValueType::Scalar(carrier, _) = carrier else {
                continue;
            };
            let id = ScalarDomainId::new(result.catalog.domains.len() as u64 + 1)
                .ok_or_else(|| unavailable("scalar domain identity overflow"))?;
            let definition = typed
                .domain_definition(symbol)
                .ok_or_else(|| unavailable("a used scalar domain lost its declaration"))?;
            let identity =
                crate::declare::nominal(typed, &sources, symbol, builder).map_err(unavailable)?;
            let clauses = source
                .scalar_domain_predicates
                .iter()
                .find(|(owner, scalar, _)| *owner == symbol && *scalar == carrier)
                .map(|(_, _, clauses)| clauses.clone())
                .ok_or_else(|| unavailable("scalar domain lost its prepared source predicates"))?;
            let mut predicate = if clauses.is_empty() {
                None
            } else {
                Some(ScalarDomainPredicate {
                    subject: semantic_vocabulary::ValueId::new(1).expect("one is a subject"),
                    proposition: crate::canonical::conjunction(clauses, diagnostic)?,
                })
            };
            let establishment_routes: Option<Vec<ScalarDomainEstablishmentRoute>> = match definition
                .issuance()
            {
                typed_trees::domain::DomainIssuance::Unrestricted => None,
                typed_trees::domain::DomainIssuance::Planned(_) => {
                    return Err(unavailable(
                        "scalar domain access plan has no closed issuance declaration",
                    ));
                }
                typed_trees::domain::DomainIssuance::Only(routes) => {
                    let mut retained = Vec::with_capacity(routes.len());
                    for route in routes {
                        match route {
                            language_semantics::DomainEstablishmentRoute::ExactMachine {
                                machine,
                            } => {
                                let ordinal = u32::try_from(retained.len()).map_err(|_| {
                                    unavailable("scalar domain issuer ordinal overflow")
                                })?;
                                result.issuers.extend(source.symbols.iter().filter_map(
                                    |(body, symbol)| {
                                        (*symbol == *machine).then_some((body, id, ordinal))
                                    },
                                ));
                                let declaration =
                                    crate::declare::semantic_identity(typed, *machine, builder)
                                        .map_err(unavailable)?;
                                let machine =
                                    typed.machine_by_symbol(*machine).ok_or_else(|| {
                                        unavailable("scalar domain issuer has no declaration")
                                    })?;
                                let callable = typed
                                    .machine_callable_identity_text(machine)
                                    .filter(|text| !text.is_empty())
                                    .ok_or_else(|| {
                                        unavailable("scalar domain issuer has no callable identity")
                                    })?;
                                retained.push(ScalarDomainEstablishmentRoute::ExactMachine {
                                    declaration: terminal_psi::NominalDeclarationIdentity::new(
                                        declaration,
                                    ),
                                    machine_identity: builder.intern(&callable),
                                });
                            }
                            language_semantics::DomainEstablishmentRoute::CheckedRequirement {
                                trait_definition,
                                requirement,
                            }
                            | language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                                boundary_trait: trait_definition,
                                requirement,
                            } => {
                                let boundary = matches!(route, language_semantics::DomainEstablishmentRoute::BoundaryRequirement { .. });
                                let (owner, declared) = typed.trait_machine_signature(*requirement)
                                    .filter(|(owner, _)| owner.symbol == *trait_definition && owner.is_boundary == boundary)
                                    .ok_or_else(|| unavailable("scalar domain route lost its exact requirement declaration"))?;
                                let callable =
                                    typed.trait_requirement_callable_identity_text(owner, declared);
                                if callable.is_empty() {
                                    return Err(unavailable(
                                        "scalar domain requirement has no callable identity",
                                    ));
                                }
                                let declaration = terminal_psi::NominalDeclarationIdentity::new(
                                    crate::declare::nominal(typed, &sources, *requirement, builder)
                                        .map_err(unavailable)?,
                                );
                                let requirement_identity = builder.intern(&callable);
                                if boundary {
                                    retained.push(
                                        ScalarDomainEstablishmentRoute::BoundaryRequirement {
                                            declaration,
                                            requirement_identity,
                                        },
                                    );
                                } else {
                                    let ordinal = u32::try_from(retained.len()).map_err(|_| {
                                        unavailable("scalar domain issuer ordinal overflow")
                                    })?;
                                    for (body, symbol) in source.symbols.iter() {
                                        if typed.machine_by_symbol(*symbol).is_some_and(|machine| {
                                            typed.machine_trait_conformances(machine).iter().any(
                                                |edge| {
                                                    edge.symbol == *trait_definition
                                                        && edge.requirement_symbol == *requirement
                                                },
                                            )
                                        }) {
                                            result.issuers.push((body, id, ordinal));
                                        }
                                    }
                                    retained.push(
                                        ScalarDomainEstablishmentRoute::CheckedRequirement {
                                            declaration,
                                            requirement_identity,
                                        },
                                    );
                                }
                            }
                        }
                    }
                    let mut ordered: Vec<_> = retained.into_iter().enumerate().collect();
                    ordered.sort_unstable_by(|(_, left), (_, right)| left.cmp(right));
                    if ordered.windows(2).any(|pair| pair[0].1 == pair[1].1) {
                        return Err(unavailable("scalar domain repeats an establishment route"));
                    }
                    for (_, _, ordinal) in result
                        .issuers
                        .iter_mut()
                        .filter(|(_, domain, _)| *domain == id)
                    {
                        *ordinal = u32::try_from(
                            ordered
                                .iter()
                                .position(|(original, _)| *original == *ordinal as usize)
                                .ok_or_else(|| {
                                    unavailable("scalar domain issuer order lost its route")
                                })?,
                        )
                        .map_err(|_| unavailable("scalar domain issuer ordinal overflow"))?;
                    }
                    Some(ordered.into_iter().map(|(_, route)| route).collect())
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
                semantic_domain: semantic_base
                    .checked_add(id.get())
                    .and_then(DomainSemanticId::new)
                    .ok_or_else(|| unavailable("scalar semantic domain identity overflow"))?,
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
                        semantic_domain: self
                            .semantic_base
                            .checked_add(id.get())
                            .and_then(DomainSemanticId::new)
                            .ok_or_else(|| {
                                unavailable("scalar policy semantic identity overflow")
                            })?,
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
