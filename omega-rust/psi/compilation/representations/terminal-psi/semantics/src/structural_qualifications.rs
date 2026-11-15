//! A structural value's domains are part of its declared type, as a scalar's
//! are (`scalar_qualifications`). Source lowering, the lowered-form checker and
//! artifact ingress share this one judgment, so a domain named on a record
//! parameter, result or field is a contract the lowered form must keep, not a
//! label only the reader checked.
//!
//! Every root place declares its whole-value domains and the projected
//! domains of the fields beneath it (`StructuralPathQualification`). A place
//! that is not a root has the memberships its root declares beneath its path.
//! Each typed transfer (a call argument, a call result, a successor argument,
//! a normal return, a copy or move, a field initializer or store) delivers
//! exactly the memberships its destination declares
//! (`wiki/spec/language/domains.md`: "the supplied expression's static type
//! must carry exactly the domains the destination declares"). Membership
//! changes only at a `StructuralQualificationChange`: an explicit cast that
//! strictly adds or strictly erases the domains a value displays (its
//! whole-value domains, or a reference's domains on its referent), naming
//! the issuer route of every added routed domain. A cast that adds a domain
//! with predicates continues in a block whose invariant states them of its
//! parameter, so the edge into it proves them. No other operation produces
//! a qualified value, except provisioning that adds only free domains.

use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, ContentPlaceVersion, EdgeId, IntegerType, MachineId,
    PlaceId, ScalarTerm, ScalarType, StructuralCaseId, StructuralDomainId, StructuralFieldId,
    ValueId,
};
use terminal_psi::{
    OperationKind, StructuralArgument, StructuralDomainDeclaration, StructuralEstablishmentRoute,
    StructuralPathQualification, StructuralPathSegment, StructuralQualificationChange,
    StructuralTypeShape, TargetMachineCallable, TerminalMachine, TerminalModule, Terminator,
};

use crate::scalar_qualifications::{IssuerRoute, issuer_route_authorizes};

/// The one rule a lowered structural value breaks, and in which machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralQualificationError {
    pub machine: Option<MachineId>,
    pub detail: &'static str,
}

type Judged<T> = Result<T, StructuralQualificationError>;

fn malformed<T>(machine: Option<MachineId>, detail: &'static str) -> Judged<T> {
    Err(StructuralQualificationError { machine, detail })
}

/// The memberships of one place: its whole-value domains and the domains of
/// the places beneath it, each list sorted and free of repeats.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Memberships {
    pub whole: Vec<StructuralDomainId>,
    pub projected: Vec<StructuralPathQualification>,
}

impl Memberships {
    fn of(whole: &[StructuralDomainId], projected: &[StructuralPathQualification]) -> Self {
        let mut result = Self {
            whole: whole.to_vec(),
            projected: projected.to_vec(),
        };
        result.whole.sort_unstable();
        result.whole.dedup();
        result.projected.sort_unstable();
        result.projected.dedup();
        result
    }

    fn is_empty(&self) -> bool {
        self.whole.is_empty() && self.projected.is_empty()
    }

    /// The memberships of the place `path` beneath this one: the rows whose
    /// path is exactly `path` become whole-value domains, the rows beneath it
    /// keep the rest of their path.
    pub fn beneath(&self, path: &[StructuralPathSegment]) -> Self {
        let (whole, projected) =
            StructuralPathQualification::select(&self.whole, &self.projected, path);
        Self { whole, projected }
    }
}

struct Declared<'a> {
    machine: &'a TerminalMachine,
    /// Every root place that declares memberships, by place identity.
    declared: Vec<(PlaceId, Memberships)>,
}

impl<'a> Declared<'a> {
    fn new(machine: &'a TerminalMachine) -> Judged<Self> {
        let mut declared = Vec::new();
        for parameter in machine.structural_parameters.iter().chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        ) {
            declared.push((
                parameter.place,
                Memberships::of(
                    &parameter.qualifications,
                    &parameter.projected_qualifications,
                ),
            ));
        }
        if let Some(result) = machine.result.structural() {
            declared.push((
                result.place,
                Memberships::of(&result.qualifications, &result.projected_qualifications),
            ));
        }
        for operation in machine.operations() {
            if let Some(result) = operation.result.structural() {
                declared.push((
                    result.place,
                    Memberships::of(&result.qualifications, &result.projected_qualifications),
                ));
            }
        }
        declared.sort_unstable_by_key(|(place, _)| *place);
        if declared.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return malformed(
                Some(machine.id),
                "a structural place declares its memberships twice",
            );
        }
        Ok(Self { machine, declared })
    }

    /// What `place` declares. A root no declaration covers (a literal, an
    /// attachment, a trivial local) holds no membership.
    fn root(&self, place: PlaceId) -> Memberships {
        self.declared
            .binary_search_by_key(&place, |(id, _)| *id)
            .map(|index| self.declared[index].1.clone())
            .unwrap_or_default()
    }

    fn at(&self, place: PlaceId, path: &[StructuralPathSegment]) -> Memberships {
        self.root(place).beneath(path)
    }

    fn argument(&self, argument: &StructuralArgument) -> Memberships {
        self.at(argument.place, &argument.path)
    }

    fn machine(&self) -> Option<MachineId> {
        Some(self.machine.id)
    }
}

fn domain(module: &TerminalModule, id: StructuralDomainId) -> Option<&StructuralDomainDeclaration> {
    module
        .structural_domains
        .binary_search_by_key(&id, |domain| domain.id)
        .ok()
        .map(|index| &module.structural_domains[index])
}

/// The path segment that names `field` beneath a value of `structural_type`,
/// and beneath `case` for a payload field.
fn field_segments(
    module: &TerminalModule,
    structural_type: semantic_vocabulary::StructuralTypeId,
    case: Option<semantic_vocabulary::StructuralCaseId>,
    field: StructuralFieldId,
) -> Option<Vec<StructuralPathSegment>> {
    let declaration = module.structural_type(structural_type)?;
    let (fields, prefix) = match (&declaration.shape, case) {
        (StructuralTypeShape::Record { fields, .. }, None)
        | (StructuralTypeShape::Mixed { fields, .. }, None) => (fields.as_slice(), Vec::new()),
        (
            StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. },
            Some(id),
        ) => {
            let case = cases.iter().find(|case| case.id == id)?;
            (
                case.fields.as_slice(),
                vec![StructuralPathSegment::Case(id)],
            )
        }
        _ => return None,
    };
    let declared = fields.iter().find(|candidate| candidate.id == field)?;
    let mut path = prefix;
    path.push(StructuralPathSegment::Field(declared.identity));
    Some(path)
}

/// A byte or content domain, which its own judgment governs (`bytes`,
/// `content_transformations`).
fn judged_elsewhere(module: &TerminalModule, id: StructuralDomainId) -> bool {
    domain(module, id).is_some_and(|domain| {
        domain.byte_predicate.is_some() || domain.content_projection.is_some()
    })
}

/// A domain a field of zero-provisioned storage may hold: no issuer
/// restriction and no prerequisite. Its predicates, if any, are clauses of the
/// owning type, which the proof kernel owes at the provisioning operation
/// exactly as for a scalar field.
fn zero_admits(module: &TerminalModule, row: &StructuralPathQualification) -> bool {
    domain(module, row.domain).is_some_and(|domain| {
        domain.byte_predicate.is_none()
            && domain.content_projection.is_none()
            && domain.establishment_routes.is_none()
            && domain.has_no_required_memberships()
            // A domain of every element has no owning record field whose
            // default-domain clauses could prove its predicate at zero.
            && (row.path.last() != Some(&StructuralPathSegment::AllElements)
                || domain.predicate.is_none())
    })
}

fn require_exact(
    places: &Declared<'_>,
    actual: &Memberships,
    expected: &Memberships,
    detail: &'static str,
) -> Judged<()> {
    if actual == expected {
        Ok(())
    } else {
        malformed(places.machine(), detail)
    }
}

/// Every domain a declaration names exists and names the carrier at the place
/// it qualifies.
fn validate_declared(module: &TerminalModule, places: &Declared<'_>) -> Judged<()> {
    let machine = places.machine;
    for (place, memberships) in &places.declared {
        let Some(root) = machine.structural_place_type(*place) else {
            return malformed(
                places.machine(),
                "a qualified structural place has no declared type",
            );
        };
        let rows = memberships.whole.iter().map(|id| (&[][..], *id)).chain(
            memberships
                .projected
                .iter()
                .map(|row| (row.path.as_slice(), row.domain)),
        );
        for (path, id) in rows {
            let Some(declared) = domain(module, id) else {
                return malformed(
                    places.machine(),
                    "a structural place names an absent domain",
                );
            };
            if judged_elsewhere(module, id) {
                continue;
            }
            let carrier =
                crate::qualification_path_carrier(module.structural_types.iter(), root, path);
            if carrier != Some(declared.carrier) {
                return malformed(
                    places.machine(),
                    "a structural place's domain has a different carrier",
                );
            }
        }
    }
    Ok(())
}

/// The domains a place displays on its value, each with whether its `self`
/// stands beneath the place's `Referent` step: its whole-value domains, and
/// a reference's domains on its referent, the memberships beneath that one
/// step (`wiki/spec/language/domains.md`, "Mutable views": "A cast applies
/// to a reference by accounting for the domains displayed on its
/// referent"). Beside them, every other projected membership, which a
/// change passes unchanged.
fn displayed(
    memberships: &Memberships,
) -> (
    Vec<(StructuralDomainId, bool)>,
    Vec<&StructuralPathQualification>,
) {
    let mut shown: Vec<_> = memberships.whole.iter().map(|id| (*id, false)).collect();
    let mut beneath = Vec::new();
    for row in &memberships.projected {
        if row.path == [StructuralPathSegment::Referent] {
            shown.push((row.domain, true));
        } else {
            beneath.push(row);
        }
    }
    (shown, beneath)
}

/// A structural domain's predicate, written over its subject's plain
/// carrier, restated over the place `root` a cast establishes it on, and
/// beneath that place's `Referent` step when `referent`.
struct Rerooted {
    subject: PlaceId,
    root: PlaceId,
    referent: bool,
}

impl Rerooted {
    fn path(
        &self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<Vec<CanonicalStructuralPathSegment>, &'static str> {
        if root != self.subject {
            return Err("a domain predicate observes storage outside its subject");
        }
        let mut rerooted = Vec::with_capacity(path.len() + 1);
        if self.referent {
            rerooted.push(CanonicalStructuralPathSegment::Referent);
        }
        rerooted.extend_from_slice(path);
        Ok(rerooted)
    }
}

impl crate::substitution::clauses::ClauseBindings for Rerooted {
    type Error = &'static str;

    fn value(&mut self, _: ValueId, _: ScalarType) -> Result<ScalarTerm, Self::Error> {
        Err("a structural domain predicate names a value")
    }

    fn field(
        &mut self,
        version: ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        let path = self.path(root, path)?;
        let root = self.root;
        Ok(match scalar_type {
            ScalarType::Integer(scalar_type) => ScalarTerm::IntegerField {
                version,
                root,
                path,
                scalar_type,
            },
            ScalarType::Boolean => ScalarTerm::BooleanField {
                version,
                root,
                path,
            },
            ScalarType::IeeeFloat(format) => ScalarTerm::IeeeFloatField {
                version,
                root,
                path,
                format,
            },
            ScalarType::Callable(_) => {
                return Err("a structural domain predicate observes a callable field");
            }
        })
    }

    fn extent(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: IntegerType,
    ) -> Result<ScalarTerm, Self::Error> {
        Ok(ScalarTerm::ViewExtent {
            path: self.path(root, path)?,
            root: self.root,
            scalar_type,
        })
    }

    fn case(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: StructuralCaseId,
    ) -> Result<bool, Self::Error> {
        Err("a structural cast's predicate tests a case")
    }
}

fn validate_change(
    module: &TerminalModule,
    places: &Declared<'_>,
    change: &StructuralQualificationChange,
    source: &StructuralArgument,
    destination: &Memberships,
    target: BlockId,
) -> Judged<Option<bool>> {
    let machine = places.machine();
    if !source.path.is_empty() || change.source != source.place {
        return malformed(
            machine,
            "a structural qualification change names another source",
        );
    }
    let before = places.argument(source);
    let (held, held_beneath) = displayed(&before);
    let (shown, shown_beneath) = displayed(destination);
    if held_beneath != shown_beneath {
        return malformed(
            machine,
            "a structural qualification change alters a projected membership",
        );
    }
    let added: Vec<_> = shown
        .iter()
        .filter(|id| !held.contains(id))
        .copied()
        .collect();
    let removed = held.iter().any(|id| !shown.contains(id));
    if added.is_empty() == !removed {
        return malformed(
            machine,
            "a structural qualification change must strictly add or strictly erase domains",
        );
    }
    if removed {
        if !change.establishments.is_empty() {
            return malformed(machine, "a structural erasure cannot establish a domain");
        }
        return Ok(None);
    }
    let declaration = places.machine.declaration.as_ref();
    let held: Vec<_> = held.iter().map(|(id, _)| *id).collect();
    for (id, referent) in &added {
        let Some(declared) = domain(module, *id) else {
            return malformed(
                machine,
                "a structural qualification change adds an absent domain",
            );
        };
        if declared.content_projection.is_some() {
            return malformed(
                machine,
                "a structural cast cannot prove a byte predicate or content projection",
            );
        }
        // `value as +D` proves the predicates of `D` of the operand on the
        // edge into the block the cast continues in: that block's invariant
        // states them of its parameter, as a scalar coercion's does
        // (`scalar_qualifications`).
        if let Some(predicate) = &declared.predicate
            && declared.byte_predicate.is_none()
        {
            let Ok(expected) = crate::substitution::clauses::substitute_clause(
                &predicate.proposition,
                &mut Rerooted {
                    subject: predicate.subject,
                    root: change.destination,
                    referent: *referent,
                },
            ) else {
                return malformed(
                    machine,
                    "a structural cast's domain predicate cannot be restated",
                );
            };
            let retained = module.scalar_block_invariants.iter().any(|invariant| {
                Some(invariant.machine) == machine
                    && invariant.header == target
                    && crate::scalar_qualifications::retains_conjunct(
                        &invariant.predicate,
                        &expected,
                    )
            });
            if !retained {
                return malformed(
                    machine,
                    "a structural cast lost its declared predicate arrival obligation",
                );
            }
        }
        let missing = declared
            .missing_membership_requirements(&module.structural_domains, &held)
            .is_none_or(|missing| missing.iter().any(|node| node.id != declared.id));
        if missing {
            return malformed(
                machine,
                "a structural cast adds a domain without its required memberships",
            );
        }
        let establishment = change.establishments.iter().find(|row| row.domain == *id);
        match (&declared.establishment_routes, establishment) {
            (None, None) => {}
            (None, Some(_)) => {
                return malformed(machine, "a structural cast names a route its domain lacks");
            }
            (Some(_), None) => {
                return malformed(
                    machine,
                    "a structural cast adds a routed domain without its issuer route",
                );
            }
            (Some(routes), Some(row)) => {
                let Some(declaration) = declaration else {
                    return malformed(machine, "a structural issuer lost its checked declaration");
                };
                let authorized = match routes.get(row.route as usize) {
                    Some(StructuralEstablishmentRoute::ExactMachine {
                        declaration: issuer,
                        machine_identity,
                    }) => issuer_route_authorizes(
                        module,
                        places.machine,
                        declaration,
                        IssuerRoute::ExactMachine {
                            issuer,
                            machine_identity: *machine_identity,
                        },
                    ),
                    Some(StructuralEstablishmentRoute::CheckedRequirement {
                        declaration: issuer,
                        requirement_identity,
                    }) => issuer_route_authorizes(
                        module,
                        places.machine,
                        declaration,
                        IssuerRoute::CheckedRequirement {
                            issuer,
                            requirement_identity: *requirement_identity,
                        },
                    ),
                    Some(StructuralEstablishmentRoute::BoundaryRequirement { .. }) | None => false,
                };
                if !authorized {
                    return malformed(
                        machine,
                        "a structural cast is not an authorized issuer of its domain",
                    );
                }
            }
        }
    }
    if change
        .establishments
        .iter()
        .any(|row| !added.iter().any(|(id, _)| *id == row.domain))
    {
        return malformed(
            machine,
            "a structural cast establishes a domain it does not add",
        );
    }
    let mut byte_domain = false;
    let mut established = true;
    for (id, referent) in &added {
        let declared = domain(module, *id).ok_or(StructuralQualificationError {
            machine,
            detail: "a structural qualification change adds an absent domain",
        })?;
        if declared.byte_predicate.is_none() {
            continue;
        }
        let root = places
            .machine
            .structural_place_type(change.destination)
            .ok_or(StructuralQualificationError {
                machine,
                detail: "a byte qualification change has no declared destination carrier",
            })?;
        let path = if *referent {
            &[StructuralPathSegment::Referent][..]
        } else {
            &[][..]
        };
        let carrier =
            crate::runtime_structural_path_tip(module.structural_types.iter(), root, path)
                .ok_or(StructuralQualificationError {
                    machine,
                    detail: "a byte qualification change has no declared displayed carrier",
                })?
                .id;
        if !crate::byte_predicate_carrier(module.structural_types.iter(), carrier) {
            return malformed(
                machine,
                "a byte qualification change has a non-byte carrier",
            );
        }
        let predicates = crate::byte_qualification_predicates_in(
            module.structural_domains.iter(),
            carrier,
            std::slice::from_ref(id),
        )
        .ok_or(StructuralQualificationError {
            machine,
            detail: "a byte qualification change is not a complete primitive byte domain",
        })?;
        let mut argument = source.clone();
        argument.path.extend_from_slice(path);
        let supplied = crate::byte_sequence_argument_predicates(module, places.machine, &argument)
            .ok_or(StructuralQualificationError {
                machine,
                detail: "a byte qualification change has no reconstructed source predicates",
            })?;
        byte_domain = true;
        established &= supplied.implies(predicates);
    }
    Ok(byte_domain.then_some(established))
}

fn validate_machine(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Judged<Vec<(EdgeId, bool)>> {
    let places = Declared::new(machine)?;
    let id = Some(machine.id);
    validate_declared(module, &places)?;
    let changes: Vec<_> = module
        .structural_qualification_changes
        .iter()
        .filter(|change| change.machine == machine.id)
        .collect();
    let mut used = vec![false; changes.len()];
    let mut byte_establishments = Vec::new();
    let find_block = |target| machine.blocks.iter().find(|block| block.id == target);
    for block in &machine.blocks {
        let edges = block.terminator.successor_edges();
        let arguments = block.terminator.successor_block_arguments();
        if edges.len() != arguments.len() {
            return malformed(id, "a successor's structural arguments lost their edge");
        }
        for ((edge, target), arguments) in edges.into_iter().zip(arguments) {
            let Some(target) = find_block(target) else {
                return malformed(id, "a structural successor names an absent block");
            };
            if target.structural_parameters.len() != arguments.len() {
                return malformed(
                    id,
                    "a structural successor has a different parameter roster",
                );
            }
            for (ordinal, (argument, parameter)) in arguments
                .iter()
                .zip(&target.structural_parameters)
                .enumerate()
            {
                let expected = Memberships::of(
                    &parameter.qualifications,
                    &parameter.projected_qualifications,
                );
                let change = changes.iter().position(|change| {
                    change.edge == edge && change.argument_ordinal as usize == ordinal
                });
                match change {
                    Some(index) => {
                        used[index] = true;
                        if changes[index].destination != parameter.place {
                            return malformed(
                                id,
                                "a structural qualification change names another destination",
                            );
                        }
                        if let Some(established) = validate_change(
                            module,
                            &places,
                            changes[index],
                            argument,
                            &expected,
                            target.id,
                        )? {
                            byte_establishments.push((edge, established));
                        }
                    }
                    None => require_exact(
                        &places,
                        &places.argument(argument),
                        &expected,
                        "a structural successor does not deliver its parameter's exact domains",
                    )?,
                }
            }
        }
        // A structural return in a machine with no structural result returns
        // an uninhabited value along a no-normal path (`inhabitation`); it
        // delivers no result, so it owes no result's domains.
        if let Terminator::ReturnStructural { source, .. } = &block.terminator
            && let Some(result) = machine.result.structural()
        {
            require_exact(
                &places,
                &places.root(*source),
                &Memberships::of(&result.qualifications, &result.projected_qualifications),
                "a structural return does not deliver its result's exact domains",
            )?;
        }
        if let Some((_, inputs)) = block.terminator.declared_invocation() {
            validate_arguments(module, &places, inputs, None)?;
        }
        for operation in &block.operations {
            let result = operation.result.structural();
            // Ordinary record casts establish membership at a checked edge.
            // A provider may establish membership when its boundary completes.
            if result.is_some_and(|result| !result.qualification_establishments.is_empty())
                && !matches!(operation.kind, OperationKind::BoundaryCall { .. })
            {
                return malformed(
                    id,
                    "a structural issuer claim on an operation requires a boundary completion",
                );
            }
            let produced = result.map(|result| {
                Memberships::of(&result.qualifications, &result.projected_qualifications)
            });
            if let Some(inputs) = operation.declared_invocation() {
                validate_arguments(module, &places, inputs, produced.as_ref())?;
                continue;
            }
            let Some(produced) = produced else {
                if let OperationKind::StoreStructuralField {
                    destination,
                    path,
                    field,
                    value,
                    ..
                } = &operation.kind
                {
                    let target = match field {
                        Some(field) => selected_field(module, machine, *destination, path, *field)?,
                        None => path.clone(),
                    };
                    require_exact(
                        &places,
                        &places.argument(value),
                        &places.at(*destination, &target),
                        "a structural store does not deliver its destination's exact domains",
                    )?;
                }
                continue;
            };
            let result_type = result.map(|result| result.structural_type);
            match &operation.kind {
                OperationKind::MoveStructuralField {
                    source,
                    path,
                    field: Some(field),
                } => {
                    let selected = selected_field(module, machine, *source, path, *field)?;
                    require_exact(
                        &places,
                        &places.at(*source, &selected),
                        &produced,
                        "a structural move does not keep its source's exact domains",
                    )?
                }
                OperationKind::StructuralLeafCopy { source, path }
                | OperationKind::MoveStructuralField {
                    source,
                    path,
                    field: None,
                } => require_exact(
                    &places,
                    &places.at(*source, path),
                    &produced,
                    "a structural copy does not keep its source's exact domains",
                )?,
                OperationKind::MoveStructuralValue { source } => require_exact(
                    &places,
                    &places.argument(source),
                    &produced,
                    "a structural move does not keep its source's exact domains",
                )?,
                OperationKind::EstablishRecord { fields }
                | OperationKind::EstablishStructuralCase { fields, .. } => {
                    let case = match &operation.kind {
                        OperationKind::EstablishStructuralCase { result_case, .. } => {
                            Some(*result_case)
                        }
                        _ => None,
                    };
                    if !produced.whole.is_empty() {
                        return malformed(id, "a structural literal cannot establish a domain");
                    }
                    let mut covered = Vec::new();
                    for initializer in fields {
                        let Some(segments) = result_type
                            .and_then(|ty| field_segments(module, ty, case, initializer.field))
                        else {
                            return malformed(id, "a structural literal names an absent field");
                        };
                        let expected = produced.beneath(&segments);
                        let actual = match initializer.value.as_structural() {
                            Some(argument) => places.argument(argument),
                            None => Memberships::default(),
                        };
                        require_exact(
                            &places,
                            &actual,
                            &expected,
                            "a structural literal's field does not deliver its declared domains",
                        )?;
                        covered.push(segments);
                    }
                    if produced.projected.iter().any(|row| {
                        !covered
                            .iter()
                            .any(|segments| row.path.starts_with(segments))
                    }) {
                        return malformed(
                            id,
                            "a structural literal declares a domain no field initializer supplies",
                        );
                    }
                }
                OperationKind::EstablishStructuralArray { elements } => {
                    if !produced.whole.is_empty()
                        || produced.projected.iter().any(|row| {
                            row.path.first() != Some(&StructuralPathSegment::AllElements)
                        })
                    {
                        return malformed(
                            id,
                            "a collection literal can declare only the domains of its elements",
                        );
                    }
                    let expected = produced.beneath(&[StructuralPathSegment::AllElements]);
                    for element in elements {
                        require_exact(
                            &places,
                            &places.argument(element),
                            &expected,
                            "a collection element does not deliver its declared domains",
                        )?;
                    }
                }
                // A reference carries the source's exact memberships at
                // the referent, including the source's whole-value domains.
                OperationKind::EstablishReference { source, .. } => {
                    let lent = places.argument(source);
                    let mut expected = Memberships::default();
                    expected
                        .projected
                        .extend(
                            lent.whole
                                .into_iter()
                                .map(|domain| StructuralPathQualification {
                                    path: vec![StructuralPathSegment::Referent],
                                    domain,
                                }),
                        );
                    for row in lent.projected {
                        let mut path = vec![StructuralPathSegment::Referent];
                        path.extend(row.path);
                        expected.projected.push(StructuralPathQualification {
                            path,
                            domain: row.domain,
                        });
                    }
                    let expected = Memberships::of(&expected.whole, &expected.projected);
                    require_exact(
                        &places,
                        &expected,
                        &produced,
                        "a reference does not keep its referent's exact domains",
                    )?;
                }
                // A reserved home holds nothing until a store fills it; the
                // store delivers its domains.
                OperationKind::EstablishStructuralLocal {} => {}
                OperationKind::EstablishZeroedStructural {} => {
                    let carrier = result_type.ok_or(StructuralQualificationError {
                        machine: id,
                        detail: "zero provisioning has no declared result carrier",
                    })?;
                    let byte_projected = produced.projected.iter().filter(|row| {
                        domain(module, row.domain)
                            .is_some_and(|domain| domain.byte_predicate.is_some())
                    });
                    let byte_defaults_hold =
                        crate::structural::zeroed_structural::zeroed_byte_qualifications_hold(
                            module.structural_types.iter(),
                            module.structural_domains.iter(),
                            carrier,
                            produced
                                .whole
                                .iter()
                                .map(|domain| (&[][..], *domain))
                                .chain(byte_projected.map(|row| (row.path.as_slice(), row.domain))),
                        );
                    if !byte_defaults_hold
                        || produced
                            .projected
                            .iter()
                            .filter(|row| {
                                domain(module, row.domain)
                                    .is_none_or(|domain| domain.byte_predicate.is_none())
                            })
                            .any(|row| !zero_admits(module, row))
                    {
                        return malformed(
                            id,
                            "zero provisioning cannot establish a structural domain",
                        );
                    }
                }
                _ if produced.is_empty() => {}
                // Byte and content domains have their own judgment
                // (`bytes`, `content_transformations`).
                _ if produced
                    .whole
                    .iter()
                    .chain(produced.projected.iter().map(|row| &row.domain))
                    .all(|domain| judged_elsewhere(module, *domain)) => {}
                _ => {
                    return malformed(
                        id,
                        "an ordinary structural operation cannot establish a declared domain",
                    );
                }
            }
        }
    }
    if used.iter().any(|used| !used) {
        return malformed(
            id,
            "a structural qualification change names no successor argument",
        );
    }
    Ok(byte_establishments)
}

/// The path to `field` of the value at `path` beneath `root`.
fn selected_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    root: PlaceId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
) -> Judged<Vec<StructuralPathSegment>> {
    let id = Some(machine.id);
    let Some(root_type) = machine.structural_place_type(root) else {
        return malformed(id, "a structural field access has no declared root");
    };
    let Some(owner) =
        crate::qualification_path_carrier(module.structural_types.iter(), root_type, path)
    else {
        return malformed(id, "a structural field access has no declared owner");
    };
    let Some(segments) = field_segments(module, owner, None, field) else {
        return malformed(id, "a structural field access names an absent field");
    };
    let mut selected = path.to_vec();
    selected.extend(segments);
    Ok(selected)
}

fn validate_arguments(
    module: &TerminalModule,
    places: &Declared<'_>,
    inputs: terminal_psi::ModuleInvocationInputs<'_, TargetMachineCallable>,
    produced: Option<&Memberships>,
) -> Judged<()> {
    let id = places.machine();
    let (formals, result) = match inputs.callee {
        TargetMachineCallable::Machine(callee) => {
            let Some(contract) =
                module.invocation_machine_contract(callee, inputs.required_contract)
            else {
                return malformed(id, "a structural call lost its exact declared contract");
            };
            (
                contract.structural_parameters.as_slice(),
                contract.result.structural().map(|result| {
                    Memberships::of(&result.qualifications, &result.projected_qualifications)
                }),
            )
        }
        TargetMachineCallable::Boundary(boundary) => {
            let Some(boundary) = module.boundary_machine(boundary) else {
                return malformed(id, "a structural call lost its boundary declaration");
            };
            (
                boundary.structural_parameters.as_slice(),
                match &boundary.result {
                    terminal_psi::BoundaryMachineResult::Structural(result) => Some(
                        Memberships::of(&result.qualifications, &result.projected_qualifications),
                    ),
                    _ => None,
                },
            )
        }
    };
    if formals.len() != inputs.structural_arguments.len() {
        return malformed(id, "a structural call has a different formal roster");
    }
    for (argument, formal) in inputs.structural_arguments.iter().zip(formals) {
        require_exact(
            places,
            &places.argument(argument),
            &Memberships::of(&formal.qualifications, &formal.projected_qualifications),
            "a structural call argument does not deliver its formal's exact domains",
        )?;
    }
    if let Some(produced) = produced {
        require_exact(
            places,
            produced,
            &result.unwrap_or_default(),
            "a structural call result differs from its callee's declared domains",
        )?;
    }
    Ok(())
}

/// Check the exact membership transfers, domain theories and issuer routes.
/// Byte predicate truth is a separate establishment judgment at each cast edge.
pub fn validate_shape(module: &TerminalModule) -> Judged<()> {
    for machine in &module.machines {
        validate_machine(module, machine)?;
    }
    validate_change_owners(module)
}

fn validate_change_owners(module: &TerminalModule) -> Judged<()> {
    if module
        .structural_qualification_changes
        .iter()
        .any(|change| module.machine(change.machine).is_none())
    {
        return malformed(
            None,
            "a structural qualification change names an absent machine",
        );
    }
    Ok(())
}

/// Primitive byte casts use the actual incoming arguments, just as byte copies
/// and moves do. The lowered checker reports a false result as an unestablished
/// value; artifact ingress requires every result to be true.
pub fn byte_qualification_change_judgments(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Judged<Vec<(EdgeId, bool)>> {
    validate_machine(module, machine)
}

/// The complete structural membership judgment required at artifact ingress.
pub fn validate(module: &TerminalModule) -> Judged<()> {
    for machine in &module.machines {
        if byte_qualification_change_judgments(module, machine)?
            .iter()
            .any(|(_, established)| !established)
        {
            return malformed(
                Some(machine.id),
                "a byte transfer's source does not establish its primitive byte-domain qualifications",
            );
        }
    }
    validate_change_owners(module)
}
