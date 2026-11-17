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
        .opaque_referents
        .iter()
        .all(|opaque| !location.contains(opaque));
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
            state.insert(fact, dependencies);
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

    fn invalidate(&mut self, writes: Option<&[DescriptorLocation]>) {
        self.0.retain(|fact, dependencies| match &fact.subject {
            Subject::Storage {
                version: ContentPlaceVersion::Current,
                ..
            } => writes.is_some_and(|writes| {
                writes.is_empty()
                    || dependencies.as_ref().is_some_and(|dependencies| {
                        writes.iter().all(|written| {
                            dependencies
                                .iter()
                                .all(|dependency| !dependency.location().overlaps(written))
                        })
                    })
            }),
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
    machine: MachineId,
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
            machine,
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
    let Some(inputs) = operation.declared_invocation() else {
        return Ok(Vec::new());
    };
    let (formal_scalar, formal_structural, rows) = match inputs.callee {
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
                header
                    .result
                    .scalar_ref()
                    .map(|value| (value.id, value.scalar_type)),
                header
                    .result
                    .structural()
                    .map(|value| (value.place, value.structural_type)),
                header.membership_ensures.as_slice(),
            )
        }
        TargetMachineCallable::Boundary(callee) => {
            let header = module.boundary_machine(callee).ok_or_else(|| {
                unavailable(machine.id, "membership call lost its boundary declaration")
            })?;
            let scalar = header
                .result
                .scalar()
                .map(|ty| {
                    ValueId::new(header.scalar_parameters.len() as u64 + 1).map(|id| (id, ty))
                })
                .flatten();
            let structural = match &header.result {
                terminal_psi::BoundaryMachineResult::Structural(value) => Some((
                    PlaceId::new(header.structural_parameters.len() as u64 + 1).ok_or_else(
                        || unavailable(machine.id, "boundary result place identity overflow"),
                    )?,
                    value.structural_type,
                )),
                _ => None,
            };
            (scalar, structural, header.membership_ensures.as_slice())
        }
    };
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let binding = match (&operation.result, formal_scalar, formal_structural) {
        (OperationResult::Scalar(actual), Some((formal, carrier)), None)
            if actual.scalar_type == carrier =>
        {
            ResultBinding::Scalar {
                formal,
                carrier,
                actual: actual.id,
            }
        }
        (OperationResult::Structural(actual), None, Some((formal, carrier)))
            if actual.structural_type == carrier =>
        {
            ResultBinding::Structural {
                formal,
                carrier,
                actual: DescriptorLocation::whole(actual.place),
            }
        }
        (OperationResult::Unit, None, None) => ResultBinding::Unit,
        _ => {
            return Err(unavailable(
                machine.id,
                "membership call result differs from its declared carrier",
            ));
        }
    };
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
            machine.id,
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

fn result_binding(machine: &TerminalMachine) -> ResultBinding {
    match &machine.result {
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

fn header_result_binding(
    machine: MachineId,
    result: &terminal_psi::TerminalMachineResult,
    actual: &ResultBinding,
) -> Judged<ResultBinding> {
    Ok(match (result, actual) {
        (terminal_psi::TerminalMachineResult::Unit, ResultBinding::Unit) => ResultBinding::Unit,
        (
            terminal_psi::TerminalMachineResult::Scalar(formal),
            ResultBinding::Scalar {
                carrier, actual, ..
            },
        ) if formal.scalar_type == *carrier => ResultBinding::Scalar {
            formal: formal.id,
            carrier: *carrier,
            actual: *actual,
        },
        (
            terminal_psi::TerminalMachineResult::Structural(formal),
            ResultBinding::Structural {
                carrier, actual, ..
            },
        ) if formal.structural_type == *carrier => ResultBinding::Structural {
            formal: formal.place,
            carrier: *carrier,
            actual: actual.clone(),
        },
        _ => {
            return Err(unavailable(
                machine,
                "membership public result differs from its selected body's carrier",
            ));
        }
    })
}

fn promised_outputs(
    module: &TerminalModule,
    machine: &TerminalMachine,
    actual: &ResultBinding,
) -> Judged<Vec<Fact>> {
    let mut goals = machine
        .contract
        .membership_ensures
        .iter()
        .map(|row| actual.membership(module, Some(machine.id), row))
        .collect::<Judged<Vec<_>>>()?;
    for header in module.declaration_contracts.iter().filter(|header| {
        header
            .selected_application
            .as_ref()
            .is_some_and(|selection| selection.realization == machine.id)
    }) {
        let binding = header_result_binding(machine.id, &header.result, actual)?;
        goals.extend(
            header
                .membership_ensures
                .iter()
                .map(|row| binding.membership(module, Some(machine.id), row))
                .collect::<Judged<Vec<_>>>()?,
        );
    }
    goals.sort();
    goals.dedup();
    Ok(goals)
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
                    crate::exactly_one(module.structural_domains.iter(), |row| row.id == *domain)
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
                        let case = crate::exactly_one(cases.iter(), |row| row.id == *case)
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
        let predicate = match row {
            RuntimeDomainMembership::Scalar { subject, domain } => {
                let domain = crate::scalar_qualifications::scalar_domain(module, *domain)
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
            RuntimeDomainMembership::Structural {
                root, path, domain, ..
            } => {
                let domain = module
                    .structural_domains
                    .iter()
                    .find(|row| row.id == *domain)
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
        };
        if let Some(predicate) = predicate
            && predicate != Proposition::Truth
            && !ensures
                .clone()
                .any(|clause| crate::scalar_qualifications::retains_conjunct(clause, &predicate))
        {
            return Err(unavailable(
                machine,
                "membership guarantee lost its independent result predicate",
            ));
        }
    }
    Ok(())
}

fn validate_headers(module: &TerminalModule) -> Judged<()> {
    validate_boundary_outputs(module)?;
    for machine in &module.machines {
        let _ = promised_outputs(module, machine, &result_binding(machine))?;
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
    for header in &module.declaration_contracts {
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
        machine: MachineId,
        edge: EdgeId,
        value: ValueId,
        domain: ScalarDomainId,
    ) -> bool {
        self.edges
            .get(&(machine, edge))
            .and_then(|position| self.before.get(&(machine, *position)))
            .is_some_and(|facts| {
                facts.0.contains_key(&Fact {
                    subject: Subject::Value(value),
                    domain: Domain::Scalar(domain),
                })
            })
    }

    pub(crate) fn structural_on_edge(
        &self,
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
                facts.0.contains_key(&Fact {
                    subject: Subject::Storage {
                        version: ContentPlaceVersion::Current,
                        location: location.clone(),
                    },
                    domain: Domain::Structural(domain),
                })
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
fn capture_return(
    state: &mut Facts,
    goals: &[Fact],
    source: &ResultBinding,
    destination: &ResultBinding,
) {
    let (
        ResultBinding::Structural { actual: source, .. },
        ResultBinding::Structural {
            actual: destination,
            ..
        },
    ) = (source, destination)
    else {
        return;
    };
    for goal in goals {
        if let Some(Some(dependencies)) = state.0.get_mut(goal) {
            *dependencies = dependencies
                .iter()
                .map(|dependency| dependency.transported(source, destination))
                .collect();
        }
    }
}

pub(crate) fn analyze(module: &TerminalModule) -> Judged<MembershipFacts> {
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
    {
        return Ok(result);
    }
    validate_headers(module)?;
    let graphs =
        terminal_reference_graph_from(module, module.machines.iter().map(|machine| machine.id))
            .map_err(|_| MembershipError {
                machine: None,
                detail: "runtime membership has no complete storage transfer graph",
            })?;
    let mut storage = DescriptorAnalysis::with_reference_slots(&module.structural_types, &graphs);
    for machine in &module.machines {
        let graph = graphs
            .iter()
            .find(|graph| graph.id == machine.id)
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
            result
                .edges
                .insert((machine.id, block.terminator.edge()), cut);
            for (edge, _) in block.terminator.successor_edges() {
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
                for goal in promised_outputs(module, machine, binding)? {
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
            let transferred = incoming.transferred(&step.value_transfers);
            let mut after = incoming.clone();
            if let Some(binding) = returns.get(&position) {
                capture_return(&mut after, &goals, binding, &result_binding(machine));
            }
            let writes = storage
                .storage_writes(machine.id, position)
                .map_err(|detail| unavailable(machine.id, detail))?;
            after.invalidate(writes.as_deref());
            after.invalidate(Some(&step.definitions));
            if let Some(defined) = graph.payload_scalar_definitions.get(&position) {
                after.0.retain(|fact, _| !matches!(fact.subject, Subject::Value(value) if defined.contains(&value)));
            }
            for (fact, dependencies) in transferred {
                after.insert(fact, dependencies);
            }
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
                for (fact, dependencies) in transported {
                    next.insert(fact, dependencies);
                }
                if let Some(bindings) = graph
                    .payload_scalar_bindings
                    .get(&position)
                    .and_then(|rows| rows.get(edge_ordinal))
                {
                    for (fact, dependencies) in transport_scalar_bindings(&after, bindings) {
                        next.insert(fact, dependencies);
                    }
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
            if goals.iter().any(|goal| !state.0.contains_key(goal)) {
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
    fn unknown_writes_invalidate_storage_membership() {
        let mut facts = Facts::default();
        facts.insert(fact(location(1, &[])), None);
        facts.invalidate(Some(&[]));
        assert_eq!(facts.0.len(), 1);
        facts.invalidate(None);
        assert!(facts.0.is_empty());
    }
}
