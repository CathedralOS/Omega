//! Membership facts follow exact runtime values independently of their
//! declared types. Storage transfers and invalidations come from the shared
//! storage graph; descriptor origins alone do not establish membership.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{
    ContentPlaceVersion, EdgeId, MachineId, PlaceId, ScalarDomainId, ScalarTerm, ScalarType,
    StructuralDomainId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Operation, OperationResult, RuntimeDomainMembership, StructuralParameterDeclaration,
    StructuralPathQualification, TargetMachineCallable, TerminalMachine, TerminalModule,
    ValueDeclaration,
};

use crate::descriptors::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPathSegment, DescriptorPosition, canonical_descriptor_location,
    descriptor_location,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Domain {
    Scalar(ScalarDomainId),
    Structural(StructuralDomainId),
}

fn required_domains(module: &TerminalModule, domain: Domain) -> Option<Vec<Domain>> {
    match domain {
        Domain::Scalar(id) => crate::scalar_qualifications::scalar_domain(module, id)
            .and_then(|domain| {
                domain.required_domain_closure(&module.scalar_qualifications.domains)
            })
            .map(|closure| {
                closure
                    .into_iter()
                    .map(|domain| Domain::Scalar(domain.id))
                    .collect()
            }),
        Domain::Structural(id) => module
            .structural_domains
            .iter()
            .find(|domain| domain.id == id)
            .and_then(|domain| domain.required_domain_closure(&module.structural_domains))
            .map(|closure| {
                closure
                    .into_iter()
                    .map(|domain| Domain::Structural(domain.id))
                    .collect()
            }),
    }
}

fn domain_supplies(module: &TerminalModule, actual: Domain, required: Domain) -> bool {
    actual == required
        || required_domains(module, actual).is_some_and(|closure| closure.contains(&required))
}

/// A declared scalar field or primitive contributes its existing type facts.
/// Missing metadata contributes nothing; it cannot supply a new membership.
fn declared_scalar_domains<'module>(
    module: &'module TerminalModule,
    carrier: StructuralTypeId,
    location: &DescriptorLocation,
) -> Option<&'module [ScalarDomainId]> {
    let path = crate::descriptor_lifetimes::descriptor_path(
        &module.structural_types,
        carrier,
        &location.path,
    )?;
    let set = if let Some(field) =
        crate::runtime_structural_field(module.structural_types.iter(), carrier, &path)
    {
        crate::scalar_qualifications::field_qualifications(module, field.id)
    } else {
        let tip =
            crate::runtime_structural_path_tip(module.structural_types.iter(), carrier, &path)?;
        crate::scalar_qualifications::primitive_qualifications(module, tip.id)
    };
    crate::scalar_qualifications::qualification_domains(module, set)
}

fn observe(
    module: &TerminalModule,
    machine: &TerminalMachine,
    storage: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
    position: DescriptorPosition,
    mut fact: Fact,
) -> Option<(Fact, Dependencies)> {
    let Subject::Storage {
        version: ContentPlaceVersion::Current,
        location,
    } = &mut fact.subject
    else {
        return Some((fact, Some(BTreeSet::new())));
    };
    *location = storage.location(machine.id, position, location)?;
    for (ordinal, step) in location.path.iter().enumerate() {
        if let DescriptorPathSegment::Case(case) = step {
            let subject = DescriptorLocation {
                root: location.root,
                path: location.path[..ordinal].to_vec(),
            };
            if !storage.case_is_established(
                machine.id,
                position,
                &crate::descriptor_results::DescriptorCasePredicate {
                    subject: subject.into(),
                    case: *case,
                },
            ) {
                return None;
            }
        }
    }
    let root_type = machine.structural_place_type(location.root)?;
    let Some(inventory) = crate::descriptor_results::DescriptorSlotKind::References.inventory(
        &module.structural_types,
        location.root,
        root_type,
    ) else {
        return Some((fact, None));
    };
    let mut dependencies = BTreeSet::from([Dependency::ValueImage(location.clone())]);
    let mut complete = inventory
        .unknown_contents
        .iter()
        .all(|opaque| !location.contains(&opaque.location));
    for mut slot in inventory
        .slots
        .into_iter()
        .filter(|slot| location.contains(slot))
    {
        slot.path.push(DescriptorPathSegment::Referent);
        match storage.location(machine.id, position, &slot) {
            Some(backing) => {
                dependencies.insert(Dependency::Referent(backing));
            }
            None => complete = false,
        }
    }
    Some((fact, complete.then_some(dependencies)))
}

fn introduce(
    module: &TerminalModule,
    machine: &TerminalMachine,
    storage: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
    position: DescriptorPosition,
    facts: impl IntoIterator<Item = Fact>,
    state: &mut Facts,
) {
    for fact in facts {
        if let Some((fact, dependencies)) = observe(module, machine, storage, position, fact) {
            state.insert_with_requirements(module, fact, dependencies);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Subject {
    Value(ValueId),
    Storage {
        version: ContentPlaceVersion,
        location: DescriptorLocation,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Fact {
    subject: Subject,
    domain: Domain,
}

impl Fact {
    fn needs_projection_evidence(&self) -> bool {
        matches!(&self.subject, Subject::Storage { location, .. }
            if location.path.iter().any(|step| !matches!(step, DescriptorPathSegment::Field(_))))
    }

    fn has_case_projection(&self) -> bool {
        matches!(&self.subject, Subject::Storage { location, .. }
            if location.path.iter().any(|step| matches!(step, DescriptorPathSegment::Case(_))))
    }
}

/// None retains an unknown indirect dependency: any possible write
/// invalidates it. Known dependencies include the subject's storage and the
/// actual storage reached through its contained references.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Dependency {
    ValueImage(DescriptorLocation),
    Referent(DescriptorLocation),
}

impl Dependency {
    fn location(&self) -> &DescriptorLocation {
        match self {
            Self::ValueImage(location) | Self::Referent(location) => location,
        }
    }
    fn transported(&self, source: &DescriptorLocation, destination: &DescriptorLocation) -> Self {
        match self {
            Self::ValueImage(location) => Self::ValueImage(
                location
                    .transported(source, destination)
                    .unwrap_or_else(|| location.clone()),
            ),
            Self::Referent(location) => Self::Referent(location.clone()),
        }
    }
}

type Dependencies = Option<BTreeSet<Dependency>>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Facts(BTreeMap<Fact, Dependencies>);

fn combine_dependencies(left: &mut Dependencies, right: &Dependencies) {
    match (left, right) {
        (Some(left), Some(right)) => left.extend(right.iter().cloned()),
        (left, None) => *left = None,
        (None, Some(_)) => {}
    }
}

impl Facts {
    /// Keep implied memberships on the same value before predecessor states
    /// meet, so a stronger domain on one path does not hide a shared fact.
    fn insert_with_requirements(
        &mut self,
        module: &TerminalModule,
        fact: Fact,
        dependencies: Dependencies,
    ) {
        if let Some(required) = required_domains(module, fact.domain) {
            for domain in required {
                self.insert(
                    Fact {
                        subject: fact.subject.clone(),
                        domain,
                    },
                    dependencies.clone(),
                );
            }
        }
        self.insert(fact, dependencies);
    }

    fn insert(&mut self, fact: Fact, dependencies: Dependencies) {
        match self.0.entry(fact) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(dependencies);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                combine_dependencies(entry.get_mut(), &dependencies)
            }
        }
    }

    /// Every copy reads the same pre-operation state. Copying a subtree
    /// transports only memberships of that subtree, never its ancestors.
    fn transferred(
        &self,
        transfers: &[(DescriptorLocation, DescriptorLocation)],
    ) -> Vec<(Fact, Dependencies)> {
        transfers
            .iter()
            .flat_map(|(source, destination)| {
                self.0.iter().filter_map(move |(fact, dependencies)| {
                    let Subject::Storage {
                        version: ContentPlaceVersion::Current,
                        location,
                    } = &fact.subject
                    else {
                        return None;
                    };
                    let dependencies = dependencies.as_ref().map(|dependencies| {
                        dependencies
                            .iter()
                            .map(|dependency| dependency.transported(source, destination))
                            .collect()
                    });
                    Some((
                        Fact {
                            subject: Subject::Storage {
                                version: ContentPlaceVersion::Current,
                                location: location.transported(source, destination)?,
                            },
                            domain: fact.domain,
                        },
                        dependencies,
                    ))
                })
            })
            .collect()
    }

    /// The copy creates its destination image, but simultaneous writes still
    /// invalidate facts that depend on storage reached through references.
    fn accept_transfers(
        &mut self,
        transfers: Vec<(Fact, Dependencies)>,
        writes: Option<&[DescriptorLocation]>,
    ) {
        for (fact, dependencies) in transfers {
            let survives = writes.is_some_and(<[DescriptorLocation]>::is_empty)
                || dependencies.as_ref().is_some_and(|dependencies| {
                    dependencies.iter().all(|dependency| match dependency {
                        Dependency::ValueImage(_) => true,
                        Dependency::Referent(referent) => writes.is_some_and(|writes| {
                            writes.iter().all(|written| !referent.overlaps(written))
                        }),
                    })
                });
            if survives {
                self.insert(fact, dependencies);
            }
        }
    }

    fn scalar_read(
        &self,
        source: &DescriptorLocation,
        destination: ValueId,
    ) -> Vec<(Fact, Dependencies)> {
        self.0
            .keys()
            .filter_map(|fact| {
                let (
                    Subject::Storage {
                        version: ContentPlaceVersion::Current,
                        location,
                    },
                    Domain::Scalar(domain),
                ) = (&fact.subject, fact.domain)
                else {
                    return None;
                };
                (location == source).then_some((
                    Fact {
                        subject: Subject::Value(destination),
                        domain: Domain::Scalar(domain),
                    },
                    Some(BTreeSet::new()),
                ))
            })
            .collect()
    }

    fn scalar_store(
        &self,
        source: ValueId,
        destination: &DescriptorLocation,
    ) -> Vec<(Fact, Dependencies)> {
        self.0
            .keys()
            .filter_map(|fact| {
                let (Subject::Value(value), Domain::Scalar(domain)) = (&fact.subject, fact.domain)
                else {
                    return None;
                };
                (*value == source).then_some((
                    Fact {
                        subject: Subject::Storage {
                            version: ContentPlaceVersion::Current,
                            location: destination.clone(),
                        },
                        domain: Domain::Scalar(domain),
                    },
                    Some(BTreeSet::from([Dependency::ValueImage(
                        destination.clone(),
                    )])),
                ))
            })
            .collect()
    }

    fn proves(&self, module: &TerminalModule, query: &Fact) -> bool {
        if self.0.contains_key(query) {
            return true;
        }
        let first = match query.domain {
            Domain::Scalar(_) => {
                Domain::Scalar(ScalarDomainId::new(1).expect("domain IDs start at one"))
            }
            Domain::Structural(_) => {
                Domain::Structural(StructuralDomainId::new(1).expect("domain IDs start at one"))
            }
        };
        let first = Fact {
            subject: query.subject.clone(),
            domain: first,
        };
        self.0
            .range(first..)
            .take_while(|(fact, _)| fact.subject == query.subject)
            .any(|(fact, _)| domain_supplies(module, fact.domain, query.domain))
    }

    fn invalidate(&mut self, writes: Option<&[DescriptorLocation]>) {
        self.0.retain(|fact, dependencies| match &fact.subject {
            Subject::Storage {
                version: ContentPlaceVersion::Current,
                ..
            } => {
                dependencies.as_ref().is_some_and(BTreeSet::is_empty)
                    || writes.is_some_and(|writes| {
                        writes.is_empty()
                            || dependencies.as_ref().is_some_and(|dependencies| {
                                writes.iter().all(|written| {
                                    dependencies
                                        .iter()
                                        .all(|dependency| !dependency.location().overlaps(written))
                                })
                            })
                    })
            }
            Subject::Storage {
                version: ContentPlaceVersion::Entry,
                ..
            }
            | Subject::Value(_) => true,
        });
    }

    fn intersect(&mut self, incoming: &Self) -> bool {
        let before = self.clone();
        self.0.retain(|fact, dependencies| {
            let Some(incoming) = incoming.0.get(fact) else {
                return false;
            };
            combine_dependencies(dependencies, incoming);
            true
        });
        *self != before
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MembershipError {
    pub machine: Option<MachineId>,
    pub detail: &'static str,
}

type Judged<T> = Result<T, MembershipError>;

fn unavailable(machine: impl Into<Option<MachineId>>, detail: &'static str) -> MembershipError {
    MembershipError {
        machine: machine.into(),
        detail,
    }
}

/// The declared result and the exact value delivered on this completion.
/// No input or neighboring declaration can stand in for that result.
enum ResultBinding {
    Unit,
    Scalar {
        formal: ValueId,
        carrier: ScalarType,
        actual: ValueId,
    },
    Structural {
        formal: PlaceId,
        carrier: StructuralTypeId,
        actual: DescriptorLocation,
    },
}

impl ResultBinding {
    fn membership(
        &self,
        module: &TerminalModule,
        machine: Option<MachineId>,
        row: &RuntimeDomainMembership,
    ) -> Judged<Fact> {
        let invalid = || {
            unavailable(
                machine,
                "membership promise does not name its exact normal result",
            )
        };
        let location = |root,
                        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
                        version| {
            let Self::Structural {
                formal,
                carrier,
                actual,
            } = self
            else {
                return Err(invalid());
            };
            if root != *formal || version != ContentPlaceVersion::Current {
                return Err(invalid());
            }
            let selected =
                canonical_descriptor_location(&module.structural_types, *formal, *carrier, path)
                    .ok_or_else(|| {
                        unavailable(
                            machine,
                            "membership result projection has no exact carrier path",
                        )
                    })?;
            selected
                .transported(&DescriptorLocation::whole(*formal), actual)
                .ok_or_else(invalid)
        };
        match row {
            RuntimeDomainMembership::Scalar {
                subject: ScalarTerm::Value { id, scalar_type },
                domain,
            } => {
                let Self::Scalar {
                    formal,
                    carrier,
                    actual,
                } = self
                else {
                    return Err(invalid());
                };
                let declared = crate::scalar_qualifications::scalar_domain(module, *domain)
                    .ok_or_else(|| {
                        unavailable(machine, "membership result names an absent scalar domain")
                    })?;
                if id != formal || scalar_type != carrier || declared.carrier != *carrier {
                    return Err(invalid());
                }
                Ok(Fact {
                    subject: Subject::Value(*actual),
                    domain: Domain::Scalar(*domain),
                })
            }
            RuntimeDomainMembership::Scalar { subject, domain } => {
                let (root, path, version) = match subject {
                    ScalarTerm::BooleanField {
                        root,
                        path,
                        version,
                    }
                    | ScalarTerm::IntegerField {
                        root,
                        path,
                        version,
                        ..
                    }
                    | ScalarTerm::IeeeFloatField {
                        root,
                        path,
                        version,
                        ..
                    } => (*root, path, *version),
                    _ => return Err(invalid()),
                };
                let Self::Structural { carrier, .. } = self else {
                    return Err(invalid());
                };
                let declared = crate::scalar_qualifications::scalar_domain(module, *domain)
                    .ok_or_else(|| {
                        unavailable(machine, "membership result names an absent scalar domain")
                    })?;
                if declared.carrier != subject.scalar_type()
                    || crate::canonical_primitive_read_type(
                        module.structural_types.iter(),
                        *carrier,
                        path,
                    ) != Some(subject.scalar_type())
                {
                    return Err(invalid());
                }
                Ok(Fact {
                    subject: Subject::Storage {
                        version,
                        location: location(root, path, version)?,
                    },
                    domain: Domain::Scalar(*domain),
                })
            }
            RuntimeDomainMembership::Structural {
                root,
                path,
                version,
                domain,
            } => {
                let Self::Structural { carrier, .. } = self else {
                    return Err(invalid());
                };
                let declared = module
                    .structural_domains
                    .iter()
                    .find(|row| row.id == *domain)
                    .ok_or_else(|| {
                        unavailable(
                            machine,
                            "membership result names an absent structural domain",
                        )
                    })?;
                if crate::canonical_structural_path_tip(
                    module.structural_types.iter(),
                    *carrier,
                    path,
                )
                .map(|row| row.id)
                    != Some(declared.carrier)
                {
                    return Err(invalid());
                }
                Ok(Fact {
                    subject: Subject::Storage {
                        version: *version,
                        location: location(*root, path, *version)?,
                    },
                    domain: Domain::Structural(*domain),
                })
            }
        }
    }
}

fn scalar_declarations(module: &TerminalModule, values: &[ValueDeclaration]) -> Judged<Vec<Fact>> {
    let mut facts = Vec::new();
    for value in values {
        let domains =
            crate::scalar_qualifications::qualification_domains(module, value.qualifications)
                .ok_or(MembershipError {
                    machine: None,
                    detail: "membership source lost its declared scalar domain set",
                })?;
        facts.extend(domains.iter().map(|domain| Fact {
            subject: Subject::Value(value.id),
            domain: Domain::Scalar(*domain),
        }));
    }
    Ok(facts)
}

fn structural_declaration(
    module: &TerminalModule,
    machine: Option<MachineId>,
    root: PlaceId,
    ty: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
) -> Judged<Vec<Fact>> {
    let mut facts = whole
        .iter()
        .map(|domain| Fact {
            subject: Subject::Storage {
                version: ContentPlaceVersion::Current,
                location: DescriptorLocation::whole(root),
            },
            domain: Domain::Structural(*domain),
        })
        .collect::<Vec<_>>();
    for row in projected {
        // An all-elements default is not an observation of one concrete
        // element. Its existing default-domain judgment remains responsible
        // for indexed uses and restoration.
        if row
            .path
            .iter()
            .any(|step| matches!(step, terminal_psi::StructuralPathSegment::AllElements))
        {
            continue;
        }
        facts.push(Fact {
            subject: Subject::Storage {
                version: ContentPlaceVersion::Current,
                location: descriptor_location(&module.structural_types, root, ty, &row.path, None)
                    .ok_or_else(|| {
                        unavailable(
                            machine,
                            "declared membership lost its concrete structural path",
                        )
                    })?,
            },
            domain: Domain::Structural(row.domain),
        });
    }
    Ok(facts)
}

fn parameter_declarations(
    module: &TerminalModule,
    machine: MachineId,
    parameters: &[StructuralParameterDeclaration],
) -> Judged<Vec<Fact>> {
    let mut facts = Vec::new();
    for parameter in parameters {
        facts.extend(structural_declaration(
            module,
            Some(machine),
            parameter.place,
            parameter.structural_type,
            &parameter.qualifications,
            &parameter.projected_qualifications,
        )?);
    }
    Ok(facts)
}

fn operation_call(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Judged<Vec<Fact>> {
    let (formal, rows) = if let Some(inputs) = operation.declared_invocation() {
        match inputs.callee {
            TargetMachineCallable::Machine(callee) => {
                let header = module
                    .invocation_machine_contract(callee, inputs.required_contract)
                    .ok_or_else(|| {
                        unavailable(
                            machine.id,
                            "membership call lost its exact selected contract",
                        )
                    })?;
                (
                    result_binding(&header.result),
                    header.membership_ensures.as_slice(),
                )
            }
            TargetMachineCallable::Boundary(callee) => {
                let boundary = module.boundary_machine(callee).ok_or_else(|| {
                    unavailable(machine.id, "membership call lost its boundary declaration")
                })?;
                (
                    boundary_binding(Some(machine.id), boundary)?,
                    boundary.membership_ensures.as_slice(),
                )
            }
        }
    } else if let Some(inputs) = operation.kind.callable_invocation() {
        let contract = module.callables.contract(inputs.contract).ok_or_else(|| {
            unavailable(
                machine.id,
                "membership call lost its exact callable contract",
            )
        })?;
        (
            result_binding(&contract.proof_result),
            contract.proof.membership_ensures.as_slice(),
        )
    } else {
        return Ok(Vec::new());
    };
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let actual = match &operation.result {
        OperationResult::Unit => ResultBinding::Unit,
        OperationResult::Scalar(value) => ResultBinding::Scalar {
            formal: value.id,
            carrier: value.scalar_type,
            actual: value.id,
        },
        OperationResult::Structural(value) => ResultBinding::Structural {
            formal: value.place,
            carrier: value.structural_type,
            actual: DescriptorLocation::whole(value.place),
        },
    };
    let binding = bind_result(Some(machine.id), formal, &actual)?;
    rows.iter()
        .map(|row| binding.membership(module, Some(machine.id), row))
        .collect()
}

#[derive(Default)]
struct Event {
    produced: Vec<Fact>,
    declared: Vec<Fact>,
}

fn operation_event(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Judged<Event> {
    let declared = match &operation.result {
        OperationResult::Unit => Vec::new(),
        OperationResult::Scalar(value) => scalar_declarations(module, std::slice::from_ref(value))?,
        OperationResult::Structural(value) => structural_declaration(
            module,
            Some(machine.id),
            value.place,
            value.structural_type,
            &value.qualifications,
            &value.projected_qualifications,
        )?,
    };
    Ok(Event {
        produced: operation_call(module, machine, operation)?,
        declared,
    })
}

fn result_binding(result: &terminal_psi::TerminalMachineResult) -> ResultBinding {
    match result {
        terminal_psi::TerminalMachineResult::Unit => ResultBinding::Unit,
        terminal_psi::TerminalMachineResult::Scalar(result) => ResultBinding::Scalar {
            formal: result.id,
            carrier: result.scalar_type,
            actual: result.id,
        },
        terminal_psi::TerminalMachineResult::Structural(result) => ResultBinding::Structural {
            formal: result.place,
            carrier: result.structural_type,
            actual: DescriptorLocation::whole(result.place),
        },
    }
}

fn bind_result(
    machine: Option<MachineId>,
    formal: ResultBinding,
    actual: &ResultBinding,
) -> Judged<ResultBinding> {
    Ok(match (formal, actual) {
        (ResultBinding::Unit, ResultBinding::Unit) => ResultBinding::Unit,
        (
            ResultBinding::Scalar {
                formal, carrier, ..
            },
            ResultBinding::Scalar {
                carrier: actual_type,
                actual,
                ..
            },
        ) if carrier == *actual_type => ResultBinding::Scalar {
            formal,
            carrier,
            actual: *actual,
        },
        (
            ResultBinding::Structural {
                formal, carrier, ..
            },
            ResultBinding::Structural {
                carrier: actual_type,
                actual,
                ..
            },
        ) if carrier == *actual_type => ResultBinding::Structural {
            formal,
            carrier,
            actual: actual.clone(),
        },
        _ => {
            return Err(unavailable(
                machine,
                "membership contract result differs from its checked realization",
            ));
        }
    })
}

/// Join each published promise to its checked body once. A provider's normal
/// guarantees are owed even when its implementation omits an explicit clause.
fn prepare_promises(module: &TerminalModule) -> Judged<BTreeMap<MachineId, Vec<Fact>>> {
    // Pure predicates are still ordinary postconditions. Retain their header
    // rows, but do not invent an additional issuer obligation for their bodies.
    let predicates = module
        .scalar_qualifications
        .domains
        .iter()
        .filter(|domain| {
            domain.establishment_routes.is_none() && domain.has_no_required_memberships()
        })
        .map(|domain| Domain::Scalar(domain.id))
        .chain(
            module
                .structural_domains
                .iter()
                .filter(|domain| domain.is_pure_predicate() && domain.byte_predicate.is_none())
                .map(|domain| Domain::Structural(domain.id)),
        )
        .collect::<BTreeSet<_>>();
    let mut promises = BTreeMap::<MachineId, Vec<Fact>>::new();
    let mut append = |machine: &TerminalMachine,
                      binding: ResultBinding,
                      rows: &[RuntimeDomainMembership]|
     -> Judged<()> {
        for row in rows {
            let fact = binding.membership(module, Some(machine.id), row)?;
            // Cases, indices and dereferences still owe formation evidence;
            // their conditional field types cannot establish that themselves.
            if fact.needs_projection_evidence() || !predicates.contains(&fact.domain) {
                promises.entry(machine.id).or_default().push(fact);
            }
        }
        Ok(())
    };
    for machine in &module.machines {
        append(
            machine,
            result_binding(&machine.result),
            &machine.contract.membership_ensures,
        )?;
    }
    for header in &module.declaration_contracts {
        if header.membership_ensures.is_empty() {
            continue;
        }
        let Some(selected) = &header.selected_application else {
            continue;
        };
        let machine = module.machine(selected.realization).ok_or_else(|| {
            unavailable(
                selected.realization,
                "membership public contract has no checked realization",
            )
        })?;
        let binding = bind_result(
            Some(machine.id),
            result_binding(&header.result),
            &result_binding(&machine.result),
        )?;
        append(machine, binding, &header.membership_ensures)?;
    }
    for candidate in &module.provider_candidates {
        let boundary = module.boundary_machine(candidate.boundary).ok_or_else(|| {
            unavailable(
                candidate.candidate,
                "membership provider has no boundary requirement",
            )
        })?;
        if boundary.membership_ensures.is_empty() {
            continue;
        }
        let machine = module.machine(candidate.candidate).ok_or_else(|| {
            unavailable(
                candidate.candidate,
                "membership provider has no checked realization",
            )
        })?;
        let binding = bind_result(
            Some(machine.id),
            boundary_binding(Some(machine.id), boundary)?,
            &result_binding(&machine.result),
        )?;
        append(machine, binding, &boundary.membership_ensures)?;
    }
    for descriptor in &module.callables.descriptors {
        let contract = module
            .callables
            .contract(descriptor.contract)
            .ok_or_else(|| {
                unavailable(
                    descriptor.machine,
                    "membership descriptor has no exact callable contract",
                )
            })?;
        if contract.proof.membership_ensures.is_empty() {
            continue;
        }
        let machine = module.machine(descriptor.machine).ok_or_else(|| {
            unavailable(
                descriptor.machine,
                "membership descriptor has no checked realization",
            )
        })?;
        let binding = bind_result(
            Some(machine.id),
            result_binding(&contract.proof_result),
            &result_binding(&machine.result),
        )?;
        append(machine, binding, &contract.proof.membership_ensures)?;
    }
    for rows in promises.values_mut() {
        rows.sort();
        rows.dedup();
    }
    Ok(promises)
}

pub(crate) fn normal_membership_refinement(
    module: &TerminalModule,
    required_result: &terminal_psi::TerminalMachineResult,
    required: &[RuntimeDomainMembership],
    selected_result: &terminal_psi::TerminalMachineResult,
    selected: &[RuntimeDomainMembership],
) -> Result<(), &'static str> {
    if required.is_empty() {
        return Ok(());
    }
    validate_roster(None, required).map_err(|error| error.detail)?;
    validate_roster(None, selected).map_err(|error| error.detail)?;
    let selected_binding = result_binding(selected_result);
    let required_binding = bind_result(None, result_binding(required_result), &selected_binding)
        .map_err(|error| error.detail)?;
    let mut provided = Facts::default();
    for row in selected {
        provided.insert(
            selected_binding
                .membership(module, None, row)
                .map_err(|error| error.detail)?,
            Some(BTreeSet::new()),
        );
    }
    let declared = match selected_result {
        terminal_psi::TerminalMachineResult::Unit => Vec::new(),
        terminal_psi::TerminalMachineResult::Scalar(value) => {
            scalar_declarations(module, std::slice::from_ref(value))
                .map_err(|error| error.detail)?
        }
        terminal_psi::TerminalMachineResult::Structural(value) => structural_declaration(
            module,
            None,
            value.place,
            value.structural_type,
            &value.qualifications,
            &value.projected_qualifications,
        )
        .map_err(|error| error.detail)?,
    };
    for fact in declared
        .into_iter()
        .filter(|fact| !fact.has_case_projection())
    {
        provided.insert(fact, Some(BTreeSet::new()));
    }
    for row in required {
        let query = required_binding
            .membership(module, None, row)
            .map_err(|error| error.detail)?;
        if let (
            Domain::Scalar(_),
            Subject::Storage { location, .. },
            terminal_psi::TerminalMachineResult::Structural(value),
        ) = (query.domain, &query.subject, selected_result)
            // A case field's type applies only when that case is active.
            // Only an explicit selected guarantee can establish that promise.
            && !query.has_case_projection()
            && let Some(domains) = declared_scalar_domains(module, value.structural_type, location)
        {
            for domain in domains {
                provided.insert(
                    Fact {
                        subject: query.subject.clone(),
                        domain: Domain::Scalar(*domain),
                    },
                    Some(BTreeSet::new()),
                );
            }
        }
        if !provided.proves(module, &query) {
            return Err(
                "selected callable contract does not provide its required normal-result membership",
            );
        }
    }
    Ok(())
}

fn bind_return_fact(machine: MachineId, binding: &ResultBinding, fact: &Fact) -> Judged<Fact> {
    let subject = match (binding, &fact.subject) {
        (ResultBinding::Scalar { formal, actual, .. }, Subject::Value(value))
            if value == formal =>
        {
            Subject::Value(*actual)
        }
        (
            ResultBinding::Structural { formal, actual, .. },
            Subject::Storage {
                version: ContentPlaceVersion::Current,
                location,
            },
        ) => Subject::Storage {
            version: ContentPlaceVersion::Current,
            location: location
                .transported(&DescriptorLocation::whole(*formal), actual)
                .ok_or_else(|| {
                    unavailable(
                        machine,
                        "membership guarantee lost its exact returned value",
                    )
                })?,
        },
        _ => {
            return Err(unavailable(
                machine,
                "membership guarantee lost its exact returned value",
            ));
        }
    };
    Ok(Fact {
        subject,
        domain: fact.domain,
    })
}

fn boundary_binding(
    machine: Option<MachineId>,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
) -> Judged<ResultBinding> {
    Ok(match &boundary.result {
        terminal_psi::BoundaryMachineResult::Unit => ResultBinding::Unit,
        terminal_psi::BoundaryMachineResult::Scalar(value) => {
            let formal =
                ValueId::new(boundary.scalar_parameters.len() as u64 + 1).ok_or_else(|| {
                    unavailable(machine, "boundary membership result identity overflow")
                })?;
            ResultBinding::Scalar {
                formal,
                carrier: value.scalar_type,
                actual: formal,
            }
        }
        terminal_psi::BoundaryMachineResult::Structural(value) => {
            let formal =
                PlaceId::new(boundary.structural_parameters.len() as u64 + 1).ok_or_else(|| {
                    unavailable(
                        machine,
                        "boundary membership result place identity overflow",
                    )
                })?;
            ResultBinding::Structural {
                formal,
                carrier: value.structural_type,
                actual: DescriptorLocation::whole(formal),
            }
        }
    })
}

fn validate_roster(machine: Option<MachineId>, rows: &[RuntimeDomainMembership]) -> Judged<()> {
    if rows.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(unavailable(
            machine,
            "normal membership guarantee roster is not canonical",
        ));
    }
    Ok(())
}

/// Join published normal-result facts to the complete authored service method.
/// Recomputing a header digest cannot authorize a different guarantee.
pub fn validate_boundary_normal_memberships(
    module: &TerminalModule,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    method: &language_semantics::service_surface::ServiceMethod,
) -> Result<(), String> {
    use language_semantics::service_surface::{
        ServiceNormalResultMembership, ServiceResultProjection,
    };
    use semantic_vocabulary::CanonicalStructuralPathSegment as Path;
    validate_roster(None, &boundary.membership_ensures).map_err(|error| error.detail)?;
    let binding = boundary_binding(None, boundary).map_err(|error| error.detail)?;
    let identity = |index| {
        module
            .identities
            .text(index)
            .filter(|text| !text.is_empty())
            .ok_or("normal-result membership lost its exact semantic identity")
    };
    let mut actual = Vec::with_capacity(boundary.membership_ensures.len());
    for row in &boundary.membership_ensures {
        let _ = binding
            .membership(module, None, row)
            .map_err(|error| error.detail)?;
        let (path, domain) = match row {
            RuntimeDomainMembership::Scalar { subject, domain } => {
                let path = match subject {
                    ScalarTerm::Value { .. } => &[][..],
                    ScalarTerm::BooleanField { path, .. }
                    | ScalarTerm::IntegerField { path, .. }
                    | ScalarTerm::IeeeFloatField { path, .. } => path.as_slice(),
                    _ => return Err("normal-result membership has no exact result selector".into()),
                };
                let domain = crate::scalar_qualifications::scalar_domain(module, *domain)
                    .ok_or("normal-result membership names an absent scalar domain")?;
                (path, identity(domain.identity)?)
            }
            RuntimeDomainMembership::Structural { path, domain, .. } => {
                let domain =
                    crate::exactly_one::exactly_one(module.structural_domains.iter(), |row| {
                        row.id == *domain
                    })
                    .ok_or("normal-result membership names no unique structural domain")?;
                (path.as_slice(), identity(domain.identity)?)
            }
        };
        let mut portable = Vec::with_capacity(path.len());
        if !path.is_empty() {
            let ResultBinding::Structural { carrier, .. } = &binding else {
                return Err("normal-result projection has no structural result".into());
            };
            for (ordinal, step) in path.iter().enumerate() {
                portable.push(match step {
                    Path::Field(field) => {
                        let field = crate::observed_field_declaration(
                            module.structural_types.iter(),
                            *carrier,
                            &path[..ordinal],
                            *field,
                        )
                        .ok_or("normal-result membership field has no exact declaration")?;
                        ServiceResultProjection::Field(identity(field.identity)?.to_owned())
                    }
                    Path::Case(case) => {
                        let tip = crate::canonical_structural_path_tip(
                            module.structural_types.iter(),
                            *carrier,
                            &path[..ordinal],
                        )
                        .ok_or("normal-result membership case has no exact carrier")?;
                        let cases = tip
                            .shape
                            .sum_cases()
                            .ok_or("normal-result membership case is not a sum")?;
                        let case =
                            crate::exactly_one::exactly_one(cases.iter(), |row| row.id == *case)
                                .ok_or("normal-result membership case has no unique declaration")?;
                        ServiceResultProjection::Case(identity(case.identity)?.to_owned())
                    }
                    Path::FixedIndex(index) => ServiceResultProjection::FixedIndex(*index),
                    Path::Referent => continue,
                });
            }
        }
        actual.push(ServiceNormalResultMembership {
            path: portable,
            domain: domain.to_owned(),
        });
    }
    actual.sort();
    if actual.windows(2).any(|pair| pair[0] == pair[1])
        || method
            .normal_result_memberships
            .iter()
            .any(|row| !row.has_exact_identity())
        || method
            .normal_result_memberships
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || actual != method.normal_result_memberships
    {
        return Err(
            "native requirement changes its authored normal-result membership guarantees".into(),
        );
    }
    Ok(())
}

fn validate_boundary_outputs(module: &TerminalModule) -> Judged<()> {
    let diagnostic = None;
    for boundary in &module.boundary_machines {
        if boundary.membership_ensures.is_empty() {
            continue;
        }
        validate_roster(diagnostic, &boundary.membership_ensures)?;
        let binding = boundary_binding(diagnostic, boundary)?;
        for row in &boundary.membership_ensures {
            let _ = binding.membership(module, diagnostic, row)?;
        }
        validate_predicates(
            module,
            diagnostic,
            &boundary.membership_ensures,
            boundary.scalar_ensures.iter(),
        )?;
    }
    Ok(())
}

/// A nominal promise does not replace its numeric or structural predicates.
/// They remain ordinary guarantees, checked against actual result values.
fn validate_predicates<'a>(
    module: &TerminalModule,
    machine: Option<MachineId>,
    memberships: &[RuntimeDomainMembership],
    ensures: impl Clone + Iterator<Item = &'a semantic_vocabulary::Proposition>,
) -> Judged<()> {
    use semantic_vocabulary::Proposition;
    for row in memberships {
        let promised = match row {
            RuntimeDomainMembership::Scalar { domain, .. } => Domain::Scalar(*domain),
            RuntimeDomainMembership::Structural { domain, .. } => Domain::Structural(*domain),
        };
        let required = required_domains(module, promised).ok_or_else(|| {
            unavailable(
                machine,
                "membership guarantee lost its complete domain requirements",
            )
        })?;
        for domain in required {
            let predicate = match (row, domain) {
                (RuntimeDomainMembership::Scalar { subject, .. }, Domain::Scalar(domain)) => {
                    let domain = crate::scalar_qualifications::scalar_domain(module, domain)
                        .ok_or_else(|| {
                            unavailable(machine, "membership guarantee lost its scalar domain")
                        })?;
                    domain.predicate.as_ref().map(|predicate| {
                        crate::substitution::substitute_proposition_values_with(
                            &predicate.proposition,
                            &|id| (id == predicate.subject).then_some(subject),
                        )
                    })
                }
                (
                    RuntimeDomainMembership::Structural { root, path, .. },
                    Domain::Structural(domain),
                ) => {
                    let domain = module
                        .structural_domains
                        .iter()
                        .find(|row| row.id == domain)
                        .ok_or_else(|| {
                            unavailable(machine, "membership guarantee lost its structural domain")
                        })?;
                    let replacement = (*root, path.clone());
                    domain.predicate.as_ref().map(|predicate| {
                        crate::substitution::substitute_proposition_structural_places_with(
                            &predicate.proposition,
                            &|id| (id == predicate.subject).then_some(&replacement),
                        )
                    })
                }
                _ => {
                    return Err(unavailable(
                        machine,
                        "membership guarantee changes its domain carrier role",
                    ));
                }
            };
            if let Some(predicate) = predicate
                && predicate != Proposition::Truth
                && !ensures.clone().any(|clause| {
                    crate::scalar_qualifications::retains_conjunct(clause, &predicate)
                })
            {
                return Err(unavailable(
                    machine,
                    "membership guarantee lost its independent result predicate",
                ));
            }
        }
    }
    Ok(())
}

fn validate_headers(module: &TerminalModule) -> Judged<()> {
    validate_boundary_outputs(module)?;
    for machine in &module.machines {
        validate_roster(Some(machine.id), &machine.contract.membership_ensures)?;
        let binding = result_binding(&machine.result);
        for row in &machine.contract.membership_ensures {
            let _ = binding.membership(module, Some(machine.id), row)?;
        }
        validate_predicates(
            module,
            Some(machine.id),
            &machine.contract.membership_ensures,
            machine
                .contract
                .ensures
                .iter()
                .map(|clause| &clause.proposition),
        )?;
    }
    for contract in &module.callables.contracts {
        validate_roster(None, &contract.proof.membership_ensures)?;
        let binding = result_binding(&contract.proof_result);
        for row in &contract.proof.membership_ensures {
            let _ = binding.membership(module, None, row)?;
        }
        validate_predicates(
            module,
            None,
            &contract.proof.membership_ensures,
            contract
                .proof
                .ensures
                .iter()
                .map(|clause| &clause.proposition),
        )?;
    }
    for header in &module.declaration_contracts {
        let machine = header
            .selected_application
            .as_ref()
            .map(|selected| selected.realization);
        validate_roster(machine, &header.membership_ensures)?;
        let binding = result_binding(&header.result);
        for row in &header.membership_ensures {
            let _ = binding.membership(module, machine, row)?;
        }
        validate_predicates(
            module,
            header
                .selected_application
                .as_ref()
                .map(|selected| selected.realization),
            &header.membership_ensures,
            header.ensures.iter().map(|clause| &clause.proposition),
        )?;
    }
    Ok(())
}

fn seed_entry(module: &TerminalModule, machine: &TerminalMachine) -> Judged<Vec<Fact>> {
    let mut entry = scalar_declarations(module, &machine.parameters)?;
    entry.extend(parameter_declarations(
        module,
        machine.id,
        &machine.structural_parameters,
    )?);
    Ok(entry)
}

fn transport_scalar_bindings(
    state: &Facts,
    bindings: &[(ValueId, ScalarTerm)],
) -> Vec<(Fact, Dependencies)> {
    bindings
        .iter()
        .flat_map(|(destination, source)| {
            state.0.iter().filter_map(move |(fact, dependencies)| {
                let (Subject::Value(source_value), ScalarTerm::Value { id, .. }) =
                    (&fact.subject, source)
                else {
                    return None;
                };
                (*source_value == *id).then_some((
                    Fact {
                        subject: Subject::Value(*destination),
                        domain: fact.domain,
                    },
                    dependencies.clone(),
                ))
            })
        })
        .collect()
}

/// Prepared once for a module's qualification judgment. These are checking
/// facts, not declared defaults or runtime custody owners.
#[derive(Default)]
pub(crate) struct MembershipFacts {
    before: BTreeMap<(MachineId, DescriptorPosition), Facts>,
    edges: BTreeMap<(MachineId, EdgeId), DescriptorPosition>,
    cast_sources: BTreeMap<(MachineId, EdgeId, u32, bool), Option<DescriptorLocation>>,
}

impl MembershipFacts {
    pub(crate) fn scalar_on_edge(
        &self,
        module: &TerminalModule,
        machine: MachineId,
        edge: EdgeId,
        value: ValueId,
        domain: ScalarDomainId,
    ) -> bool {
        self.edges
            .get(&(machine, edge))
            .and_then(|position| self.before.get(&(machine, *position)))
            .is_some_and(|facts| {
                facts.proves(
                    module,
                    &Fact {
                        subject: Subject::Value(value),
                        domain: Domain::Scalar(domain),
                    },
                )
            })
    }

    pub(crate) fn structural_on_edge(
        &self,
        module: &TerminalModule,
        change: &terminal_psi::StructuralQualificationChange,
        referent: bool,
        domain: StructuralDomainId,
    ) -> bool {
        let Some(Some(location)) = self.cast_sources.get(&(
            change.machine,
            change.edge,
            change.argument_ordinal,
            referent,
        )) else {
            return false;
        };
        self.edges
            .get(&(change.machine, change.edge))
            .and_then(|position| self.before.get(&(change.machine, *position)))
            .is_some_and(|facts| {
                facts.proves(
                    module,
                    &Fact {
                        subject: Subject::Storage {
                            version: ContentPlaceVersion::Current,
                            location: location.clone(),
                        },
                        domain: Domain::Structural(domain),
                    },
                )
            })
    }
}

fn declared_roots(rows: impl IntoIterator<Item = Fact>, roots: &mut BTreeMap<PlaceId, Vec<Fact>>) {
    for fact in rows {
        if let Subject::Storage { location, .. } = &fact.subject {
            roots.entry(location.root).or_default().push(fact);
        }
    }
}

fn observed_defaults(
    module: &TerminalModule,
    machine: &TerminalMachine,
    storage: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
    position: DescriptorPosition,
    location: &DescriptorLocation,
    declarations: &BTreeMap<PlaceId, Vec<Fact>>,
    state: &mut Facts,
) {
    let Some(rows) = declarations.get(&location.root) else {
        return;
    };
    introduce(module, machine, storage, position, rows.iter().filter(|fact| {
        matches!(&fact.subject, Subject::Storage { location: subject, .. } if location.contains(subject))
    }).cloned(), state);
}

fn returned_binding(
    machine: &TerminalMachine,
    terminator: &terminal_psi::Terminator,
) -> Judged<Option<ResultBinding>> {
    Ok(match (terminator, &machine.result) {
        (
            terminal_psi::Terminator::Return { value, .. },
            terminal_psi::TerminalMachineResult::Scalar(result),
        ) => Some(ResultBinding::Scalar {
            formal: result.id,
            carrier: result.scalar_type,
            actual: *value,
        }),
        (
            terminal_psi::Terminator::ReturnStructural { source, .. },
            terminal_psi::TerminalMachineResult::Structural(result),
        ) => Some(ResultBinding::Structural {
            formal: result.place,
            carrier: result.structural_type,
            actual: DescriptorLocation::whole(*source),
        }),
        (
            terminal_psi::Terminator::ReturnUnit { .. },
            terminal_psi::TerminalMachineResult::Unit,
        ) => Some(ResultBinding::Unit),
        (
            terminal_psi::Terminator::Return { .. }
            | terminal_psi::Terminator::ReturnStructural { .. }
            | terminal_psi::Terminator::ReturnUnit { .. },
            _,
        ) => {
            return Err(unavailable(
                machine.id,
                "membership completion differs from its declared result",
            ));
        }
        _ => None,
    })
}

/// A return captures the value before cleanup. Its own image moves to the
/// result, while contained references keep depending on their original storage.
fn capture_return(state: &mut Facts, source: &ResultBinding) {
    let ResultBinding::Structural { actual: source, .. } = source else {
        return;
    };
    for (fact, dependencies) in &mut state.0 {
        let Subject::Storage {
            version: ContentPlaceVersion::Current,
            location,
        } = &fact.subject
        else {
            continue;
        };
        if !source.contains(location) {
            continue;
        }
        if let Some(dependencies) = dependencies {
            dependencies.retain(|dependency| !matches!(dependency, Dependency::ValueImage(location) if source.contains(location)));
        }
    }
}

pub(crate) fn analyze(module: &TerminalModule) -> Judged<MembershipFacts> {
    let bodies = module
        .machines
        .iter()
        .map(|machine| machine.id)
        .collect::<Vec<_>>();
    analyze_bodies(module, &bodies)
}

/// Check every header, but prove normal completion only for these exact bodies.
/// A check form retains abstract requirements as callable contracts, not bodies.
pub(crate) fn analyze_bodies(
    module: &TerminalModule,
    bodies: &[MachineId],
) -> Judged<MembershipFacts> {
    use crate::descriptor_results::{DescriptorAnalysis, terminal_reference_graph_from};
    let mut result = MembershipFacts::default();
    if !module
        .machines
        .iter()
        .any(|machine| !machine.contract.membership_ensures.is_empty())
        && !module
            .declaration_contracts
            .iter()
            .any(|header| !header.membership_ensures.is_empty())
        && !module
            .boundary_machines
            .iter()
            .any(|header| !header.membership_ensures.is_empty())
        && !module
            .callables
            .contracts
            .iter()
            .any(|contract| !contract.proof.membership_ensures.is_empty())
    {
        return Ok(result);
    }
    validate_headers(module)?;
    let promises = prepare_promises(module)?;
    let graphs =
        terminal_reference_graph_from(module, module.machines.iter().map(|machine| machine.id))
            .map_err(|_| MembershipError {
                machine: None,
                detail: "runtime membership has no complete storage transfer graph",
            })?;
    let mut graphs_by_id = graphs.iter().collect::<Vec<_>>();
    graphs_by_id.sort_unstable_by_key(|graph| graph.id);
    if graphs_by_id.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return Err(unavailable(
            None,
            "runtime membership has duplicate machine storage graphs",
        ));
    }
    let bodies = bodies.iter().copied().collect::<BTreeSet<_>>();
    if bodies.iter().any(|id| {
        graphs_by_id
            .binary_search_by_key(id, |graph| graph.id)
            .is_err()
    }) {
        return Err(unavailable(
            None,
            "membership checking names an absent body",
        ));
    }
    let mut storage = DescriptorAnalysis::with_reference_slots(&module.structural_types, &graphs);
    for machine in &module.machines {
        if !bodies.contains(&machine.id) {
            continue;
        }
        let graph = graphs_by_id
            .binary_search_by_key(&machine.id, |graph| graph.id)
            .ok()
            .map(|index| graphs_by_id[index])
            .ok_or_else(|| {
                unavailable(
                    machine.id,
                    "runtime membership lost its machine's storage graph",
                )
            })?;
        let flow = storage.flow(machine.id).map_err(|_| {
            unavailable(
                machine.id,
                "runtime membership storage aliases could not be resolved",
            )
        })?;
        let mut events = BTreeMap::new();
        let mut arrivals = BTreeMap::new();
        let mut returns = BTreeMap::new();
        let mut declarations = BTreeMap::new();
        declared_roots(seed_entry(module, machine)?, &mut declarations);
        for block in &machine.blocks {
            let mut declared = scalar_declarations(module, &block.parameters)?;
            declared.extend(parameter_declarations(
                module,
                machine.id,
                &block.structural_parameters,
            )?);
            declared_roots(declared.iter().cloned(), &mut declarations);
            arrivals.insert((block.id, 0), declared);
            for (ordinal, operation) in block.operations.iter().enumerate() {
                let ordinal = u32::try_from(ordinal).map_err(|_| {
                    unavailable(machine.id, "membership operation ordinal overflow")
                })?;
                let event = operation_event(module, machine, operation)?;
                declared_roots(event.declared.iter().cloned(), &mut declarations);
                events.insert((block.id, ordinal), event);
            }
            let cut = (
                block.id,
                u32::try_from(block.operations.len()).map_err(|_| {
                    unavailable(machine.id, "membership terminator ordinal overflow")
                })?,
            );
            for edge in block.terminator.edges() {
                result.edges.insert((machine.id, edge), cut);
            }
            if let Some(binding) = returned_binding(machine, &block.terminator)? {
                returns.insert(cut, binding);
            }
        }
        let mut entry = Facts::default();
        introduce(
            module,
            machine,
            &mut storage,
            graph.entry,
            seed_entry(module, machine)?,
            &mut entry,
        );
        let mut before = BTreeMap::from([(graph.entry, entry)]);
        let mut effective = BTreeMap::new();
        let mut completed = BTreeMap::new();
        let mut pending = BTreeSet::from([graph.entry]);
        while let Some(position) = pending.pop_first() {
            let step = flow
                .get(&position)
                .ok_or_else(|| unavailable(machine.id, "membership point has no storage event"))?;
            let mut incoming = before.get(&position).cloned().ok_or_else(|| {
                unavailable(machine.id, "membership point lost its incoming facts")
            })?;
            if let Some(declared) = arrivals.get(&position) {
                introduce(
                    module,
                    machine,
                    &mut storage,
                    position,
                    declared.iter().cloned(),
                    &mut incoming,
                );
            }
            for source in step
                .observed_inputs()
                .chain(step.value_transfers.iter().map(|(source, _)| source))
            {
                observed_defaults(
                    module,
                    machine,
                    &mut storage,
                    position,
                    source,
                    &declarations,
                    &mut incoming,
                );
            }
            for edge in &step.successors {
                for (source, _) in edge.derivations() {
                    observed_defaults(
                        module,
                        machine,
                        &mut storage,
                        position,
                        source,
                        &declarations,
                        &mut incoming,
                    );
                }
            }
            let mut goals = Vec::new();
            if let Some(binding) = returns.get(&position) {
                if let ResultBinding::Structural { actual, .. } = binding {
                    observed_defaults(
                        module,
                        machine,
                        &mut storage,
                        position,
                        actual,
                        &declarations,
                        &mut incoming,
                    );
                }
                for goal in promises.get(&machine.id).into_iter().flatten() {
                    let goal = bind_return_fact(machine.id, binding, goal)?;
                    if let (Domain::Scalar(_), Subject::Storage { location, .. }) =
                        (goal.domain, &goal.subject)
                        && let Some(carrier) = machine.structural_place_type(location.root)
                        && let Some(domains) = declared_scalar_domains(module, carrier, location)
                    {
                        introduce(
                            module,
                            machine,
                            &mut storage,
                            position,
                            domains.iter().map(|domain| Fact {
                                subject: goal.subject.clone(),
                                domain: Domain::Scalar(*domain),
                            }),
                            &mut incoming,
                        );
                    }
                    match observe(module, machine, &mut storage, position, goal) {
                        Some((goal, _)) => goals.push(goal),
                        None => {
                            return Err(unavailable(
                                machine.id,
                                "normal membership guarantee has no exact returned storage observation",
                            ));
                        }
                    }
                }
            }
            effective.insert(position, incoming.clone());
            let mut transferred = incoming.transferred(&step.value_transfers);
            if let Some(reads) = graph.scalar_payload_reads.get(&position) {
                for (source, destination) in reads {
                    let ScalarTerm::Value { id, .. } = destination else {
                        continue;
                    };
                    if let Some(source) = storage.location(machine.id, position, source) {
                        transferred.extend(incoming.scalar_read(&source, *id));
                    }
                }
            }
            for (destination, source) in &step.scalar_value_stores {
                transferred.extend(incoming.scalar_store(*source, destination));
            }
            let mut after = incoming.clone();
            if let Some(binding) = returns.get(&position) {
                capture_return(&mut after, binding);
            }
            let writes = storage
                .storage_writes(machine.id, position)
                .map_err(|detail| unavailable(machine.id, detail))?;
            after.invalidate(writes.as_deref());
            after.invalidate(Some(&step.definitions));
            if let Some(defined) = graph.payload_scalar_definitions.get(&position) {
                after.0.retain(|fact, _| !matches!(fact.subject, Subject::Value(value) if defined.contains(&value)));
            }
            after.accept_transfers(transferred, writes.as_deref());
            if !goals.is_empty() {
                completed.insert(position, (goals, after.clone()));
            }
            for (edge_ordinal, edge) in step.successors.iter().enumerate() {
                let transfers = edge
                    .derivations()
                    .filter_map(|(source, destination)| {
                        Some((
                            storage.location(machine.id, position, source)?,
                            storage.location(machine.id, edge.target, destination)?,
                        ))
                    })
                    .collect::<Vec<_>>();
                let transported = after.transferred(&transfers);
                let mut next = after.clone();
                let mut defined = edge.definitions.clone();
                defined.extend(edge.parameters.iter().cloned());
                next.invalidate(Some(&defined));
                if let Some(parameters) = (edge.target.1 == 0)
                    .then(|| graph.payload_scalar_parameters.get(&edge.target.0))
                    .flatten()
                {
                    next.0.retain(|fact, _| !matches!(fact.subject, Subject::Value(value) if parameters.contains(&value)));
                }
                next.accept_transfers(transported, Some(&defined));
                if let Some(bindings) = graph
                    .payload_scalar_bindings
                    .get(&position)
                    .and_then(|rows| rows.get(edge_ordinal))
                {
                    next.accept_transfers(
                        transport_scalar_bindings(&after, bindings),
                        Some(&defined),
                    );
                }
                if let Some(event) = events.get(&position) {
                    introduce(
                        module,
                        machine,
                        &mut storage,
                        edge.target,
                        event.declared.iter().chain(&event.produced).cloned(),
                        &mut next,
                    );
                }
                let changed = match before.get_mut(&edge.target) {
                    Some(previous) => previous.intersect(&next),
                    None => {
                        before.insert(edge.target, next);
                        true
                    }
                };
                if changed {
                    pending.insert(edge.target);
                }
            }
        }
        for (_, (goals, state)) in completed {
            if goals.iter().any(|goal| !state.proves(module, goal)) {
                return Err(unavailable(
                    machine.id,
                    "normal completion does not establish its promised runtime membership",
                ));
            }
        }
        for change in module
            .structural_qualification_changes
            .iter()
            .filter(|change| change.machine == machine.id)
        {
            let Some(position) = result.edges.get(&(machine.id, change.edge)) else {
                continue;
            };
            for referent in [false, true] {
                let mut source = DescriptorLocation::whole(change.source);
                if referent {
                    source.path.push(DescriptorPathSegment::Referent);
                }
                let canonical = storage.location(machine.id, *position, &source);
                result.cast_sources.insert(
                    (machine.id, change.edge, change.argument_ordinal, referent),
                    canonical,
                );
            }
        }
        result.before.extend(
            effective
                .into_iter()
                .map(|(position, facts)| ((machine.id, position), facts)),
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::StructuralFieldId;

    fn location(place: u64, fields: &[u64]) -> DescriptorLocation {
        DescriptorLocation {
            root: PlaceId::new(place).unwrap(),
            path: fields
                .iter()
                .map(|field| DescriptorPathSegment::Field(StructuralFieldId::new(*field).unwrap()))
                .collect(),
        }
    }
    fn fact(location: DescriptorLocation) -> Fact {
        Fact {
            subject: Subject::Storage {
                version: ContentPlaceVersion::Current,
                location,
            },
            domain: Domain::Structural(StructuralDomainId::new(1).unwrap()),
        }
    }

    #[test]
    fn copying_a_value_keeps_its_actual_referent_dependency() {
        let source = location(1, &[]);
        let destination = location(2, &[]);
        let backing = location(3, &[1]);
        let mut original = Facts::default();
        original.insert(
            fact(source.clone()),
            Some(BTreeSet::from([
                Dependency::ValueImage(source.clone()),
                Dependency::Referent(backing.clone()),
            ])),
        );
        let mut copied = Facts::default();
        for (fact, dependencies) in original.transferred(&[(source.clone(), destination.clone())]) {
            copied.insert(fact, dependencies);
        }
        copied.invalidate(Some(&[source]));
        assert!(
            copied.0.contains_key(&fact(destination.clone())),
            "the new value owns its image independently"
        );
        copied.invalidate(Some(&[location(3, &[2])]));
        assert!(
            copied.0.contains_key(&fact(destination.clone())),
            "a disjoint sibling is unchanged"
        );
        copied.invalidate(Some(&[backing]));
        assert!(
            copied.0.is_empty(),
            "the reference still depends on the original backing"
        );
    }

    #[test]
    fn a_copy_cannot_restore_a_fact_after_a_simultaneous_referent_write() {
        let source = location(1, &[]);
        let destination = location(2, &[]);
        let backing = location(3, &[]);
        let mut original = Facts::default();
        original.insert(
            fact(source.clone()),
            Some(BTreeSet::from([
                Dependency::ValueImage(source.clone()),
                Dependency::Referent(backing.clone()),
            ])),
        );
        let transfers = original.transferred(&[(source, destination.clone())]);
        let mut copied = Facts::default();
        copied.accept_transfers(transfers.clone(), Some(&[destination.clone(), backing]));
        assert!(
            copied.0.is_empty(),
            "the copy cannot revive a changed referent"
        );
        copied.accept_transfers(transfers.clone(), None);
        assert!(
            copied.0.is_empty(),
            "unknown writes can change the referent"
        );
        copied.accept_transfers(transfers, Some(std::slice::from_ref(&destination)));
        assert!(
            copied.0.contains_key(&fact(destination)),
            "the copy creates its own destination image"
        );
    }

    #[test]
    fn copying_a_field_does_not_qualify_the_enclosing_record() {
        let source = location(1, &[1]);
        let destination = location(2, &[2]);
        let mut original = Facts::default();
        original.insert(
            fact(location(1, &[])),
            Some(BTreeSet::from([Dependency::ValueImage(location(1, &[]))])),
        );
        original.insert(
            fact(source.clone()),
            Some(BTreeSet::from([Dependency::ValueImage(source.clone())])),
        );
        let copied = original.transferred(&[(source, destination.clone())]);
        assert_eq!(copied.len(), 1);
        assert_eq!(copied[0].0, fact(destination));
        assert_ne!(copied[0].0, fact(location(2, &[])));
    }

    #[test]
    fn scalar_reads_capture_the_proven_value_and_stores_name_only_the_written_field() {
        let field = location(1, &[1]);
        let scalar = ValueId::new(8).unwrap();
        let domain = Domain::Scalar(ScalarDomainId::new(1).unwrap());
        let mut facts = Facts::default();
        facts.insert(
            Fact {
                subject: Subject::Storage {
                    version: ContentPlaceVersion::Current,
                    location: field.clone(),
                },
                domain,
            },
            Some(BTreeSet::from([Dependency::ValueImage(field.clone())])),
        );
        for (fact, dependencies) in facts.scalar_read(&field, scalar) {
            facts.insert(fact, dependencies);
        }
        facts.invalidate(Some(&[field]));
        assert!(facts.0.contains_key(&Fact {
            subject: Subject::Value(scalar),
            domain
        }));
        let stored = facts.scalar_store(scalar, &location(2, &[2]));
        assert_eq!(stored.len(), 1);
        assert_eq!(
            stored[0].0.subject,
            Subject::Storage {
                version: ContentPlaceVersion::Current,
                location: location(2, &[2])
            }
        );
        assert!(
            facts
                .scalar_store(ValueId::new(9).unwrap(), &location(2, &[2]))
                .is_empty(),
            "an equal but distinct scalar is not supplied evidence"
        );
    }

    #[test]
    fn unused_public_headers_still_owe_their_exact_result_subject() {
        let carrier = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 32)
                .unwrap(),
        );
        let result = ValueId::new(2).unwrap();
        let domain = ScalarDomainId::new(1).unwrap();
        let mut module = TerminalModule::declarations();
        module.identities = vec![String::new(), "exact-domain-application".into()].into();
        module
            .scalar_qualifications
            .domains
            .push(terminal_psi::ScalarDomainDeclaration {
                required_memberships: Some(Vec::new()),
                predicate: None,
                id: domain,
                semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
                identity: terminal_psi::TerminalIdentityIndex(1),
                carrier,
                establishment_routes: Some(Vec::new()),
            });
        let mut header =
            terminal_psi::MachineContract::empty(semantic_vocabulary::ContractId::new(1).unwrap());
        header.parameters.push(ValueDeclaration {
            id: ValueId::new(1).unwrap(),
            scalar_type: carrier,
            qualifications: Default::default(),
        });
        header
            .parameter_order
            .push(terminal_psi::BoundaryParameterKind::Scalar);
        header.result = terminal_psi::TerminalMachineResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type: carrier,
            qualifications: Default::default(),
        });
        header
            .membership_ensures
            .push(RuntimeDomainMembership::Scalar {
                subject: ScalarTerm::value(result, carrier),
                domain,
            });
        module.declaration_contracts.push(header);
        assert!(analyze(&module).is_ok());
        module.declaration_contracts[0].membership_ensures[0] = RuntimeDomainMembership::Scalar {
            subject: ScalarTerm::value(ValueId::new(1).unwrap(), carrier),
            domain,
        };
        let error = match analyze(&module) {
            Ok(_) => panic!("an unused header cannot rename an input as its normal result"),
            Err(error) => error,
        };
        assert_eq!(
            error.detail,
            "membership promise does not name its exact normal result"
        );
    }

    #[test]
    fn refinement_uses_exact_result_membership_and_its_declared_closure() {
        let carrier = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 32)
                .unwrap(),
        );
        let mut module = TerminalModule::declarations();
        for id in 1..=3 {
            module
                .scalar_qualifications
                .domains
                .push(terminal_psi::ScalarDomainDeclaration {
                    required_memberships: Some(if id == 2 {
                        vec![semantic_vocabulary::DomainSemanticId::new(1).unwrap()]
                    } else {
                        Vec::new()
                    }),
                    predicate: None,
                    id: ScalarDomainId::new(id).unwrap(),
                    semantic_domain: semantic_vocabulary::DomainSemanticId::new(id).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(id as u32),
                    carrier,
                    establishment_routes: Some(Vec::new()),
                });
        }
        let set = semantic_vocabulary::ScalarQualificationSetId::new(1);
        module
            .scalar_qualifications
            .sets
            .push(terminal_psi::ScalarQualificationSet {
                id: set,
                domains: vec![ScalarDomainId::new(2).unwrap()],
            });
        let required = terminal_psi::TerminalMachineResult::Scalar(ValueDeclaration {
            id: ValueId::new(4).unwrap(),
            scalar_type: carrier,
            qualifications: Default::default(),
        });
        let selected = terminal_psi::TerminalMachineResult::Scalar(ValueDeclaration {
            id: ValueId::new(9).unwrap(),
            scalar_type: carrier,
            qualifications: set,
        });
        let promise = |domain| RuntimeDomainMembership::Scalar {
            subject: ScalarTerm::value(ValueId::new(4).unwrap(), carrier),
            domain: ScalarDomainId::new(domain).unwrap(),
        };
        normal_membership_refinement(&module, &required, &[promise(1)], &selected, &[]).unwrap();
        assert_eq!(
            normal_membership_refinement(&module, &required, &[promise(3)], &selected, &[])
                .unwrap_err(),
            "selected callable contract does not provide its required normal-result membership"
        );
        let mut facts = Facts::default();
        facts.insert(
            Fact {
                subject: Subject::Value(ValueId::new(9).unwrap()),
                domain: Domain::Scalar(ScalarDomainId::new(2).unwrap()),
            },
            Some(BTreeSet::new()),
        );
        assert!(
            !facts.proves(
                &module,
                &Fact {
                    subject: Subject::Value(ValueId::new(8).unwrap()),
                    domain: Domain::Scalar(ScalarDomainId::new(1).unwrap())
                }
            ),
            "the domain closure cannot change the value it names"
        );

        let mut left = Facts::default();
        left.insert_with_requirements(
            &module,
            Fact {
                subject: Subject::Value(ValueId::new(9).unwrap()),
                domain: Domain::Scalar(ScalarDomainId::new(2).unwrap()),
            },
            Some(BTreeSet::new()),
        );
        let mut right = Facts::default();
        let shared = Fact {
            subject: Subject::Value(ValueId::new(9).unwrap()),
            domain: Domain::Scalar(ScalarDomainId::new(1).unwrap()),
        };
        right.insert_with_requirements(&module, shared.clone(), Some(BTreeSet::new()));
        left.intersect(&right);
        assert!(
            left.0.contains_key(&shared),
            "both predecessors justify the weaker domain on this value"
        );
        assert!(!left.proves(
            &module,
            &Fact {
                domain: Domain::Scalar(ScalarDomainId::new(2).unwrap()),
                ..shared
            }
        ));

        use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, StructuralCaseId};
        use terminal_psi::{
            StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
            StructuralMultiplicity, StructuralResultDeclaration, StructuralTypeDeclaration,
            StructuralTypeShape, TerminalIdentityIndex, TerminalMachineResult,
        };
        let ty = StructuralTypeId::new(1).unwrap();
        let case = StructuralCaseId::new(1).unwrap();
        let field = StructuralFieldId::new(1).unwrap();
        module.structural_types.push(StructuralTypeDeclaration {
            declaration_identity: TerminalIdentityIndex(1),
            quotient_carrier: None,
            id: ty,
            identity: TerminalIdentityIndex(1),
            owns_linear_claim: false,
            multiplicity: StructuralMultiplicity::Unrestricted,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
            requires: Vec::new(),
            shape: StructuralTypeShape::Sum {
                cases: vec![
                    StructuralCaseDeclaration {
                        declaration_identity: TerminalIdentityIndex(2),
                        id: case,
                        identity: TerminalIdentityIndex(2),
                        fields: vec![StructuralFieldDeclaration {
                            declaration_identity: TerminalIdentityIndex(3),
                            inline_carrier: None,
                            id: field,
                            identity: TerminalIdentityIndex(3),
                            relevance: language_core::BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(carrier),
                            storage: None,
                        }],
                    },
                    StructuralCaseDeclaration {
                        declaration_identity: TerminalIdentityIndex(6),
                        id: StructuralCaseId::new(2).unwrap(),
                        identity: TerminalIdentityIndex(6),
                        fields: Vec::new(),
                    },
                ],
            },
        });
        module
            .scalar_qualifications
            .fields
            .push(terminal_psi::ScalarFieldQualification {
                field,
                value_type: semantic_vocabulary::QualifiedScalarType {
                    scalar_type: carrier,
                    qualifications: set,
                },
            });
        let result = TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: PlaceId::new(11).unwrap(),
            structural_type: ty,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: Vec::new(),
        });
        assert_eq!(
            declared_scalar_domains(
                &module,
                ty,
                &DescriptorLocation {
                    root: PlaceId::new(11).unwrap(),
                    path: vec![
                        DescriptorPathSegment::Case(case),
                        DescriptorPathSegment::Field(field)
                    ],
                }
            ),
            Some(&[ScalarDomainId::new(2).unwrap()][..]),
            "the case field really has the stronger declared domain"
        );
        let promise = RuntimeDomainMembership::Scalar {
            subject: ScalarTerm::IntegerField {
                root: PlaceId::new(11).unwrap(),
                path: vec![Path::Case(case), Path::Field(field)],
                version: ContentPlaceVersion::Current,
                scalar_type: carrier.as_integer().unwrap(),
            },
            domain: ScalarDomainId::new(1).unwrap(),
        };
        assert_eq!(
            normal_membership_refinement(&module, &result, &[promise], &result, &[]).unwrap_err(),
            "selected callable contract does not provide its required normal-result membership",
            "a qualified case field does not prove that the normal result has that case"
        );

        // The same formation rule applies to structural projected defaults.
        let nested = StructuralTypeId::new(2).unwrap();
        let mut nested_type = module.structural_types[0].clone();
        nested_type.id = nested;
        nested_type.identity = TerminalIdentityIndex(4);
        nested_type.shape = StructuralTypeShape::Record {
            fields: Vec::new(),
            storage: terminal_psi::StructuralRecordStorage::Packed,
        };
        module.structural_types.push(nested_type);
        let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
            unreachable!()
        };
        cases[0].fields[0].field_type = StructuralFieldType::Structural(nested);
        let domain = StructuralDomainId::new(1).unwrap();
        module
            .structural_domains
            .push(terminal_psi::StructuralDomainDeclaration {
                required_memberships: Some(Vec::new()),
                complete_predicate_theory: true,
                predicate: None,
                id: domain,
                semantic_domain: semantic_vocabulary::DomainSemanticId::new(11).unwrap(),
                identity: TerminalIdentityIndex(5),
                carrier: nested,
                subject_identity: TerminalIdentityIndex(4),
                content_projection: None,
                byte_predicate: None,
                establishment_routes: Some(Vec::new()),
                access_policy: None,
            });
        let TerminalMachineResult::Structural(mut selected) = result.clone() else {
            unreachable!()
        };
        selected
            .projected_qualifications
            .push(terminal_psi::StructuralPathQualification {
                path: vec![
                    terminal_psi::StructuralPathSegment::Case(case),
                    terminal_psi::StructuralPathSegment::Field(TerminalIdentityIndex(3)),
                ],
                domain,
            });
        assert_eq!(
            structural_declaration(
                &module,
                None,
                selected.place,
                ty,
                &[],
                &selected.projected_qualifications
            )
            .unwrap()
            .len(),
            1
        );
        let promise = RuntimeDomainMembership::Structural {
            root: selected.place,
            path: vec![Path::Case(case), Path::Field(field)],
            version: ContentPlaceVersion::Current,
            domain,
        };
        assert_eq!(
            normal_membership_refinement(
                &module,
                &result,
                &[promise],
                &TerminalMachineResult::Structural(selected),
                &[]
            )
            .unwrap_err(),
            "selected callable contract does not provide its required normal-result membership"
        );

        // Required domains contribute their predicates to the same promise.
        module.scalar_qualifications.domains[0].predicate =
            Some(terminal_psi::ScalarDomainPredicate {
                subject: ValueId::new(1).unwrap(),
                proposition: semantic_vocabulary::Proposition::Falsehood,
            });
        let promise = RuntimeDomainMembership::Scalar {
            subject: ScalarTerm::value(ValueId::new(4).unwrap(), carrier),
            domain: ScalarDomainId::new(2).unwrap(),
        };
        assert_eq!(
            validate_predicates(
                &module,
                None,
                std::slice::from_ref(&promise),
                std::iter::empty()
            )
            .unwrap_err()
            .detail,
            "membership guarantee lost its independent result predicate"
        );
        validate_predicates(
            &module,
            None,
            &[promise],
            [&semantic_vocabulary::Proposition::Falsehood].into_iter(),
        )
        .unwrap();
    }

    #[test]
    fn return_capture_freezes_the_image_but_keeps_live_referent_dependencies() {
        let image = location(1, &[]);
        let backing = location(3, &[]);
        let plain = fact(image.clone());
        let mut borrowed = plain.clone();
        borrowed.domain = Domain::Structural(StructuralDomainId::new(2).unwrap());
        let mut state = Facts::default();
        state.insert(
            plain.clone(),
            Some(BTreeSet::from([Dependency::ValueImage(image.clone())])),
        );
        state.insert(
            borrowed.clone(),
            Some(BTreeSet::from([
                Dependency::ValueImage(image.clone()),
                Dependency::Referent(backing),
            ])),
        );
        capture_return(
            &mut state,
            &ResultBinding::Structural {
                formal: image.root,
                carrier: StructuralTypeId::new(1).unwrap(),
                actual: image,
            },
        );
        state.invalidate(None);
        assert!(state.0.contains_key(&plain));
        assert!(!state.0.contains_key(&borrowed));
    }

    #[test]
    fn unknown_writes_invalidate_storage_membership() {
        let mut facts = Facts::default();
        facts.insert(fact(location(1, &[])), None);
        facts.invalidate(Some(&[]));
        assert_eq!(facts.0.len(), 1);
        facts.invalidate(None);
        assert!(facts.0.is_empty());
    }
}
