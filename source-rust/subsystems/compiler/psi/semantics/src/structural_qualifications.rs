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
//!
//! A value keeps a routed domain only while it is unchanged or while the
//! machine changing it is one the domain's routes name. A store beneath such
//! a value, or an exclusive loan of part of it, in any other machine is
//! refused (`DefaultRequirements::unauthorized_routed_write`): otherwise a
//! field write would keep evidence that only an issuer may establish.

use application_identity::type_identity::TypeIdentityPackageOwnerVisitor;
use application_identity::type_identity::TypeIdentityVisitError;
use application_identity::type_identity::visit_static_argument_package_owners;
use application_identity::type_identity::visit_type_identity_package_owners;
use application_identity::type_identity::vocabulary::OwnerForm;

use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, ContentPlaceVersion, EdgeId, IntegerType, MachineId,
    PlaceId, Proposition, ScalarTerm, ScalarType, StructuralCaseId, StructuralDomainId,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Operation, OperationKind, StructuralArgument, StructuralDomainDeclaration,
    StructuralEstablishmentRoute, StructuralPathQualification, StructuralPathSegment,
    StructuralQualificationChange, StructuralTypeDeclaration, StructuralTypeShape,
    TargetMachineCallable, TerminalMachine, TerminalModule, Terminator,
};

use crate::domain_issuance::{DeniedEstablishment, excludes_machine, issuer_route_authorizes};

/// The one rule a lowered structural value breaks, and in which machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralQualificationError {
    pub machine: Option<MachineId>,
    pub detail: &'static str,
    denied_establishment: Option<DeniedEstablishment>,
}

impl StructuralQualificationError {
    pub fn denied_establishment(&self) -> Option<DeniedEstablishment> {
        self.denied_establishment
    }
}

type Judged<T> = Result<T, StructuralQualificationError>;

fn malformed<T>(machine: Option<MachineId>, detail: &'static str) -> Judged<T> {
    Err(StructuralQualificationError {
        denied_establishment: None,
        machine,
        detail,
    })
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

/// Recognize boundary requirements already expressed by the input types.
/// This establishes no actual membership: the structural-call judgment must
/// still require each actual to deliver its formal's exact domains. Normalize
/// each formal once, then compare complete domain identities at exact paths.
pub fn formal_domains_cover_requirements(
    formals: &[terminal_psi::StructuralParameterDeclaration],
    requirements: &[terminal_psi::StructuralDomainRequirement],
) -> bool {
    if requirements.is_empty() {
        return true;
    }
    let memberships = formals
        .iter()
        .map(|formal| Memberships::of(&formal.qualifications, &formal.projected_qualifications))
        .collect::<Vec<_>>();
    requirements.iter().all(|requirement| {
        let Ok(position) = usize::try_from(requirement.argument_index) else {
            return false;
        };
        let Some(formal) = formals.get(position) else {
            return false;
        };
        if formal.position != requirement.argument_index {
            return false;
        }
        let declared = &memberships[position];
        if requirement.path.is_empty() {
            declared.whole.binary_search(&requirement.domain).is_ok()
        } else {
            declared
                .projected
                .binary_search_by(|membership| {
                    membership
                        .path
                        .cmp(&requirement.path)
                        .then_with(|| membership.domain.cmp(&requirement.domain))
                })
                .is_ok()
        }
    })
}

struct Declared<'a> {
    machine: &'a TerminalMachine,
    /// Every root place that declares memberships, by place identity.
    declared: Vec<(PlaceId, StructuralTypeId, Memberships)>,
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
                parameter.structural_type,
                Memberships::of(
                    &parameter.qualifications,
                    &parameter.projected_qualifications,
                ),
            ));
        }
        if let Some(result) = machine.result.structural() {
            declared.push((
                result.place,
                result.structural_type,
                Memberships::of(&result.qualifications, &result.projected_qualifications),
            ));
        }
        for operation in machine.operations() {
            if let Some(result) = operation.result.structural() {
                declared.push((
                    result.place,
                    result.structural_type,
                    Memberships::of(&result.qualifications, &result.projected_qualifications),
                ));
            }
        }
        declared.sort_unstable_by_key(|(place, _, _)| *place);
        if declared.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return malformed(
                Some(machine.id),
                "a structural place declares its memberships twice",
            );
        }
        // Additional physical roots (literal and trivial homes) have no
        // declaration memberships. Merge sorted actual rows once, without
        // repeated whole-machine place lookup or sparse-ID allocation.
        let mut extra = machine
            .structural_places
            .iter()
            .filter_map(|row| row.kind.produced_structural_type().map(|ty| (row.id, ty)))
            .collect::<Vec<_>>();
        extra.sort_unstable_by_key(|(place, _)| *place);
        if extra.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return malformed(
                Some(machine.id),
                "a physical structural root is declared twice",
            );
        }
        let mut merged = Vec::with_capacity(declared.len() + extra.len());
        let mut existing = declared.into_iter().peekable();
        let mut extra = extra.into_iter().peekable();
        while existing.peek().is_some() || extra.peek().is_some() {
            match (existing.peek(), extra.peek()) {
                (Some((left, _, _)), Some((right, _))) if left == right => {
                    let row = existing.next().expect("declared row");
                    let physical = extra.next().expect("physical row");
                    if row.1 != physical.1 {
                        return malformed(
                            Some(machine.id),
                            "a physical root has a different declared carrier",
                        );
                    }
                    merged.push(row);
                }
                (Some((left, _, _)), Some((right, _))) if left < right => {
                    merged.push(existing.next().expect("declared row"));
                }
                (Some(_), None) => merged.push(existing.next().expect("declared row")),
                _ => {
                    let (place, ty) = extra.next().expect("physical row");
                    merged.push((place, ty, Memberships::default()));
                }
            }
        }
        Ok(Self {
            machine,
            declared: merged,
        })
    }

    /// What `place` declares. A root no declaration covers (a literal, an
    /// attachment, a trivial local) holds no membership.
    fn root(&self, place: PlaceId) -> Memberships {
        self.declared
            .binary_search_by_key(&place, |(id, _, _)| *id)
            .map(|index| self.declared[index].2.clone())
            .unwrap_or_default()
    }

    fn row(&self, place: PlaceId) -> Option<&(PlaceId, StructuralTypeId, Memberships)> {
        self.declared
            .binary_search_by_key(&place, |(id, _, _)| *id)
            .ok()
            .map(|index| &self.declared[index])
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

/// One declaration-owned clause at one actual storage occurrence. This is a
/// reconstructed query result, not additional terminal evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultRequirement {
    pub root: PlaceId,
    pub path: Vec<StructuralPathSegment>,
    pub carrier: StructuralTypeId,
    origin: DefaultClause,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DefaultClause {
    Type(usize),
    Domain(StructuralDomainId),
}

impl DefaultRequirement {
    /// Compare declaration-owned clauses independently of their source alias.
    /// Consumers must separately compare resolved actual storage.
    pub fn same_clause(&self, other: &Self) -> bool {
        self.carrier == other.carrier && self.origin == other.origin
    }
}

/// A routed domain declared on a value that an operation changes in part.
/// The value at `path` beneath `root` keeps its declared domain across the
/// operation, so the operation's machine must be one the domain's routes
/// name (`wiki/spec/language/domains.md`: "Mutation invalidates
/// subject-bound facts unless preserved or re-established").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedOwner {
    pub root: PlaceId,
    pub path: Vec<StructuralPathSegment>,
    pub domain: StructuralDomainId,
}

/// Existing storage one operation changes, as each possible lender holds it.
enum Changed<'path> {
    /// Every value that contains `path` beneath `root` changes with it.
    Containing {
        root: PlaceId,
        path: Vec<StructuralPathSegment>,
    },
    /// The value at `path` beneath `root` is replaced through a display that
    /// may show fewer domains than its lender declares.
    Replaced {
        displayed_root: PlaceId,
        displayed_path: &'path [StructuralPathSegment],
        root: PlaceId,
        path: Vec<StructuralPathSegment>,
    },
}

/// Construction owes its clauses immediately. Writes and exclusive calls
/// can instead open a restoration window on existing storage.
pub fn operation_writes_default_storage(kind: &OperationKind) -> bool {
    kind.storage_uses()
        .iter()
        .any(|(_, effect)| writes_default_storage(*effect))
        || kind.structural_arguments().iter().any(|argument| {
            matches!(
                argument.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            )
        })
}

fn writes_default_storage(effect: Option<access_vocabulary::AccessOperation>) -> bool {
    effect.is_some_and(|effect| {
        matches!(
            effect,
            access_vocabulary::AccessOperation::Write
                | access_vocabulary::AccessOperation::Atomic(_)
        ) && effect.reads_and_writes().1
    })
}

/// The default clauses of actual declared storage. Additional entry facts and
/// the result placeholder are not storage declarations. Named memberships only
/// select what must be proved; they never supply predicate truth.
pub struct DefaultRequirements<'a> {
    types: &'a [StructuralTypeDeclaration],
    domains: &'a [StructuralDomainDeclaration],
    machine: &'a TerminalMachine,
    declared: Declared<'a>,
    type_defaults: Vec<bool>,
    /// Whether any domain restricts who may establish it.
    routed: bool,
}

fn push_routed(
    result: &mut Vec<RoutedOwner>,
    root: PlaceId,
    path: &[StructuralPathSegment],
    domain: StructuralDomainId,
) {
    let owner = RoutedOwner {
        root,
        path: path.to_vec(),
        domain,
    };
    if !result.contains(&owner) {
        result.push(owner);
    }
}

impl<'a> DefaultRequirements<'a> {
    pub fn new(
        types: &'a [StructuralTypeDeclaration],
        domains: &'a [StructuralDomainDeclaration],
        machine: &'a TerminalMachine,
    ) -> Judged<Self> {
        let mut memo = vec![None; types.len()];
        let mut active = vec![false; types.len()];
        for position in 0..types.len() {
            type_has_defaults(types, position, &mut memo, &mut active).map_err(|detail| {
                StructuralQualificationError {
                    denied_establishment: None,
                    machine: Some(machine.id),
                    detail,
                }
            })?;
        }
        Ok(Self {
            types,
            domains,
            machine,
            declared: Declared::new(machine)?,
            routed: domains
                .iter()
                .any(|domain| domain.establishment_routes.is_some()),
            type_defaults: memo
                .into_iter()
                .map(|value| value.expect("prepared type"))
                .collect(),
        })
    }

    fn error(&self, detail: &'static str) -> StructuralQualificationError {
        StructuralQualificationError {
            denied_establishment: None,
            machine: Some(self.machine.id),
            detail,
        }
    }

    fn carrier(&self, root: PlaceId, path: &[StructuralPathSegment]) -> Judged<StructuralTypeId> {
        let ty = self
            .declared
            .row(root)
            .map(|row| row.1)
            .ok_or_else(|| self.error("a default requirement has no actual storage root"))?;
        crate::runtime_structural_path_tip(self.types.iter(), ty, path)
            .map(|row| row.id)
            .ok_or_else(|| self.error("a default requirement has no structural carrier"))
    }

    fn declaration(&self, id: StructuralTypeId) -> Judged<&'a StructuralTypeDeclaration> {
        self.types
            .binary_search_by_key(&id, |row| row.id)
            .ok()
            .map(|position| &self.types[position])
            .ok_or_else(|| self.error("a default requirement names an absent carrier"))
    }

    fn at(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
        carrier: StructuralTypeId,
    ) -> Judged<Vec<DefaultRequirement>> {
        let mut result = Vec::new();
        for position in 0..self.declaration(carrier)?.requires.len() {
            result.push(DefaultRequirement {
                root,
                path: path.to_vec(),
                carrier,
                origin: DefaultClause::Type(position),
            });
        }
        let mut named = Vec::new();
        for id in self.declared.at(root, path).whole {
            named.extend(self.domain_clauses(id, carrier, path)?);
        }
        named.sort_unstable();
        named.dedup();
        result.extend(named.into_iter().map(|id| DefaultRequirement {
            root,
            path: path.to_vec(),
            carrier,
            origin: DefaultClause::Domain(id),
        }));
        Ok(result)
    }

    /// Predicate clauses owed by storage with a declared domain at this path.
    fn domain_clauses(
        &self,
        id: StructuralDomainId,
        carrier: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Judged<Vec<StructuralDomainId>> {
        let mut named = Vec::new();
        let row = self
            .domains
            .binary_search_by_key(&id, |row| row.id)
            .ok()
            .map(|position| &self.domains[position])
            .ok_or_else(|| self.error("a default requirement names an absent domain"))?;
        if row.carrier != carrier {
            return Err(self.error("a named default requirement has a different carrier"));
        }
        let closure = row
            .required_domain_closure(self.domains)
            .ok_or_else(|| self.error("a named default has incomplete membership requirements"))?;
        for domain in closure {
            if !domain.complete_predicate_theory || domain.content_projection.is_some() {
                return Err(self.error("a named default has no complete storage predicate theory"));
            }
            if domain.predicate.is_some() || domain.byte_predicate.is_some() {
                if path
                    .iter()
                    .any(|step| matches!(step, StructuralPathSegment::Referent))
                {
                    return Err(
                        self.error("a reference display is not a lender's declared default")
                    );
                }
                named.push(domain.id);
            }
        }
        Ok(named)
    }

    /// The complete declared defaults at one actual storage occurrence.
    /// These select obligations; callers must still check that the occurrence
    /// is initialized and outside every outstanding restoration window.
    pub fn storage(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Judged<Vec<DefaultRequirement>> {
        self.at(root, path, self.carrier(root, path)?)
    }

    /// Every clause used for snapshot demand, including nested type defaults.
    /// This inventory supplies no assumptions about those contents.
    pub fn declared(&self) -> Judged<Vec<DefaultRequirement>> {
        let mut result = Vec::new();
        for (place, ty, _) in &self.declared.declared {
            if self
                .machine
                .result
                .structural()
                .is_some_and(|r| r.place == *place)
            {
                continue;
            }
            self.walk(
                *place,
                &mut Vec::new(),
                *ty,
                false,
                &mut Vec::new(),
                &mut result,
            )?;
        }
        Ok(result)
    }

    fn type_has_defaults(&self, carrier: StructuralTypeId) -> Judged<bool> {
        self.types
            .binary_search_by_key(&carrier, |row| row.id)
            .ok()
            .map(|position| self.type_defaults[position])
            .ok_or_else(|| self.error("a default inventory names an absent carrier"))
    }

    fn projected_beneath(&self, root: PlaceId, path: &[StructuralPathSegment]) -> bool {
        self.declared.row(root).is_some_and(|row| {
            row.2.projected.iter().any(|qualified| {
                StructuralPathQualification::selected_suffix(&qualified.path, path).is_some()
            })
        })
    }

    fn walk(
        &self,
        root: PlaceId,
        path: &mut Vec<StructuralPathSegment>,
        carrier: StructuralTypeId,
        zero: bool,
        active: &mut Vec<StructuralTypeId>,
        result: &mut Vec<DefaultRequirement>,
    ) -> Judged<()> {
        if active.contains(&carrier) {
            return Err(self.error("recursive inline storage has no finite default inventory"));
        }
        active.push(carrier);
        result.extend(self.at(root, path, carrier)?);
        match &self.declaration(carrier)?.shape {
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } => {
                let mut all = self.type_has_defaults(*element)?;
                let mut selected = Vec::new();
                if let Some(row) = self.declared.row(root) {
                    for qualified in &row.2.projected {
                        let Some(suffix) =
                            StructuralPathQualification::selected_suffix(&qualified.path, path)
                        else {
                            continue;
                        };
                        match suffix.first() {
                            Some(StructuralPathSegment::FixedIndex(index)) if *index < *length => {
                                selected.push(*index);
                            }
                            Some(StructuralPathSegment::AllElements) => {
                                // A marker adds no storage predicate, regardless of array length.
                                let leaf = crate::qualification_path_carrier(
                                    self.types.iter(),
                                    *element,
                                    &suffix[1..],
                                )
                                .ok_or_else(|| {
                                    self.error("a default inventory names an absent element path")
                                })?;
                                all |= !self
                                    .domain_clauses(qualified.domain, leaf, &qualified.path)?
                                    .is_empty();
                            }
                            None => {}
                            _ => {
                                return Err(self.error(
                                    "a default inventory has no concrete array qualification path",
                                ));
                            }
                        }
                    }
                }
                selected.sort_unstable();
                selected.dedup();
                let visit = |index| {
                    path.push(StructuralPathSegment::FixedIndex(index));
                    let result = self.walk(root, path, *element, zero, active, result);
                    path.pop();
                    result
                };
                if all {
                    (0..*length).try_for_each(visit)?;
                } else {
                    selected.into_iter().try_for_each(visit)?;
                }
            }
            // A symbolic zero has no individual element coordinates. The
            // zero vocabulary check permits only ungated primitive elements,
            // so there is no nested default clause to inventory here.
            StructuralTypeShape::FixedArray {
                length: terminal_psi::ArrayExtent::Parameter { .. },
                ..
            } if zero => {}
            StructuralTypeShape::Record { fields, .. } => {
                self.fields(root, path, fields, zero, active, result)?;
            }
            StructuralTypeShape::Sum { cases } => {
                if zero && cases.is_empty() {
                    return Err(self.error("a zero sum declares no case"));
                }
                for case in cases.iter().take(if zero { 1 } else { cases.len() }) {
                    path.push(StructuralPathSegment::Case(case.id));
                    self.fields(root, path, &case.fields, zero, active, result)?;
                    path.pop();
                }
            }
            // References are independent storage, not inline descendants.
            // Their display never contributes the lender's default clauses.
            StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::AtomicCell(_)
            | StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::ElementView { .. } => {}
            _ if !zero => {}
            _ => return Err(self.error("zero storage has no supported default inventory")),
        }
        active.pop();
        Ok(())
    }

    fn fields(
        &self,
        root: PlaceId,
        path: &mut Vec<StructuralPathSegment>,
        fields: &[terminal_psi::StructuralFieldDeclaration],
        zero: bool,
        active: &mut Vec<StructuralTypeId>,
        result: &mut Vec<DefaultRequirement>,
    ) -> Judged<()> {
        for field in fields {
            if let terminal_psi::StructuralFieldType::Structural(nested) = field.field_type {
                path.push(StructuralPathSegment::Field(field.identity));
                if self.type_has_defaults(nested)? || self.projected_beneath(root, path) {
                    self.walk(root, path, nested, zero, active, result)?;
                }
                path.pop();
            }
        }
        Ok(())
    }

    /// Plain storage needs no descriptor graph. Prepare one only when this
    /// machine can write through borrowed storage or a borrowed successor.
    pub fn needs_borrowed_storage(&self) -> bool {
        let writes = self
            .machine
            .operations()
            .any(|operation| operation_writes_default_storage(&operation.kind));
        writes
            && (self.machine.blocks.iter().any(|block| {
                block
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.access != terminal_psi::StructuralAccess::Owned)
            }) || self.declared.declared.iter().any(|(_, ty, _)| {
                terminal_psi::structural_type_has_borrowed_storage_by(
                    |id| {
                        self.types
                            .binary_search_by_key(&id, |row| row.id)
                            .ok()
                            .map(|position| &self.types[position])
                    },
                    *ty,
                )
            }))
    }

    /// Reconstruct each operation's required clauses from its declared type
    /// and exact backing. The producer and checker use the same stable order.
    pub fn operation(
        &self,
        module: &TerminalModule,
        operation: &Operation,
        before: crate::descriptor_lifetimes::DescriptorPosition,
        references: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
    ) -> Judged<Vec<DefaultRequirement>> {
        let mut result = self.declared_operation(operation)?;
        self.changed_storage(module, operation, before, references, &mut |changed| {
            match changed {
                Changed::Containing { root, path } => {
                    self.append_unique(&mut result, self.owners(root, &path)?)
                }
                Changed::Replaced {
                    displayed_root,
                    displayed_path,
                    root,
                    path,
                } => self.append_lost(&mut result, displayed_root, displayed_path, root, &path)?,
            }
            Ok(())
        })?;
        Ok(result)
    }

    /// The routed domains of every value `operation` changes in part: the
    /// same displayed owners and actual lenders whose default clauses
    /// `operation` reconstructs. A store that replaces a whole qualified
    /// value is not among them, because its source must already deliver the
    /// destination's exact domains.
    pub fn routed_owners(
        &self,
        module: &TerminalModule,
        operation: &Operation,
        before: crate::descriptor_lifetimes::DescriptorPosition,
        references: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
    ) -> Judged<Vec<RoutedOwner>> {
        let mut result = Vec::new();
        if !self.routed || !operation_writes_default_storage(&operation.kind) {
            return Ok(result);
        }
        for (root, path) in self.displayed_containers(operation) {
            self.append_routed(&mut result, root, &path);
        }
        self.changed_storage(module, operation, before, references, &mut |changed| {
            match changed {
                Changed::Containing { root, path } => {
                    self.append_routed(&mut result, root, &path)
                }
                Changed::Replaced {
                    displayed_root,
                    displayed_path,
                    root,
                    path,
                } => {
                    // A reference shows its referent's domains one step
                    // beneath itself, so both count as displayed.
                    let mut shown = self.declared.at(displayed_root, displayed_path).whole;
                    let mut referent = displayed_path.to_vec();
                    referent.push(StructuralPathSegment::Referent);
                    shown.extend(self.declared.at(displayed_root, &referent).whole);
                    for domain in self.declared.at(root, &path).whole {
                        if self.is_routed(domain) && !shown.contains(&domain) {
                            push_routed(&mut result, root, &path, domain);
                        }
                    }
                }
            }
            Ok(())
        })?;
        Ok(result)
    }

    /// The first value with a routed domain that `operation` changes in part
    /// although none of the domain's routes names this machine. A boundary
    /// route issues when its call completes and names no checked body, so it
    /// authorizes no write.
    pub fn unauthorized_routed_write(
        &self,
        module: &TerminalModule,
        operation: &Operation,
        before: crate::descriptor_lifetimes::DescriptorPosition,
        references: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
    ) -> Judged<Option<RoutedOwner>> {
        for owner in self.routed_owners(module, operation, before, references)? {
            let routes = self
                .domains
                .binary_search_by_key(&owner.domain, |row| row.id)
                .ok()
                .and_then(|position| self.domains[position].establishment_routes.as_deref())
                .ok_or_else(|| self.error("a routed owner names a domain with no routes"))?;
            let authorized = self.machine.declaration.as_ref().is_some_and(|declaration| {
                routes.iter().any(|route| {
                    issuer_route_authorizes(module, self.machine, declaration, route)
                })
            });
            if !authorized {
                return Ok(Some(owner));
            }
        }
        Ok(None)
    }

    fn is_routed(&self, id: StructuralDomainId) -> bool {
        self.domains
            .binary_search_by_key(&id, |row| row.id)
            .is_ok_and(|position| self.domains[position].establishment_routes.is_some())
    }

    /// Routed domains of the value at `path` beneath `root` and of every
    /// value containing it, innermost first.
    fn append_routed(
        &self,
        result: &mut Vec<RoutedOwner>,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) {
        for length in (0..=path.len()).rev() {
            let path = &path[..length];
            // A case step selects a payload; the sum itself is the owner.
            if matches!(path.last(), Some(StructuralPathSegment::Case(_))) {
                continue;
            }
            for domain in self.declared.at(root, path).whole {
                if self.is_routed(domain) {
                    push_routed(result, root, path, domain);
                }
            }
        }
    }

    /// Visit the existing storage `operation` changes, resolved through each
    /// captured address to the lender that actually holds it.
    fn changed_storage<'operation>(
        &self,
        module: &TerminalModule,
        operation: &'operation Operation,
        before: crate::descriptor_lifetimes::DescriptorPosition,
        references: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
        visit: &mut dyn FnMut(Changed<'_>) -> Judged<()>,
    ) -> Judged<()> {
        use terminal_psi::StructuralStorageUse;
        // A plain view can still lend storage whose owner has a domain. Keep
        // that owner's requirements after resolving the captured address.
        for (used, effect) in operation.kind.storage_uses() {
            if !writes_default_storage(effect) {
                continue;
            }
            let (root, path, payload) = match used {
                StructuralStorageUse::Projection(projection) => {
                    (projection.root, projection.path.to_vec(), false)
                }
                StructuralStorageUse::Place(root) => (root, Vec::new(), true),
                StructuralStorageUse::Element { descriptor, path } => {
                    let selected = if descriptor.field.is_some() {
                        let ty = self.carrier(descriptor.root, &[])?;
                        let location = crate::descriptor_lifetimes::descriptor_location(
                            self.types,
                            descriptor.root,
                            ty,
                            descriptor.path,
                            descriptor.field,
                        )
                        .ok_or_else(|| {
                            self.error("an element write lost its declared descriptor field")
                        })?;
                        crate::descriptor_lifetimes::descriptor_path(self.types, ty, &location.path)
                            .ok_or_else(|| {
                                self.error("an element write lost its descriptor storage path")
                            })?
                    } else {
                        descriptor.path.to_vec()
                    };
                    for (root, mut selected) in
                        self.backings(module, descriptor.root, &selected, true, before, references)?
                    {
                        selected.extend_from_slice(path);
                        selected.pop(); // Only the containing owner carries the default.
                        visit(Changed::Containing {
                            root,
                            path: selected,
                        })?;
                    }
                    continue;
                }
                _ => return Err(self.error("a default write has no supported storage projection")),
            };
            let replaces_whole = matches!(
                operation.kind,
                OperationKind::WriteOnlyPrimitiveStore { .. }
                    | OperationKind::StoreStructuralField { field: None, .. }
                    | OperationKind::StructuralByteViewFieldStore { field: None, .. }
                    | OperationKind::AtomicAccess { .. }
            );
            for (owner, mut selected) in
                self.backings(module, root, &path, payload, before, references)?
            {
                if replaces_whole {
                    visit(Changed::Replaced {
                        displayed_root: root,
                        displayed_path: &path,
                        root: owner,
                        path: selected.clone(),
                    })?;
                    if selected.pop().is_none() {
                        continue;
                    }
                }
                visit(Changed::Containing {
                    root: owner,
                    path: selected,
                })?;
            }
        }
        for argument in operation.kind.structural_arguments() {
            if !matches!(
                argument.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) {
                continue;
            }
            for (root, mut path) in self.backings(
                module,
                argument.place,
                &argument.path,
                true,
                before,
                references,
            )? {
                // The callee maintains its displayed domains. Each possible
                // lender still owes its own hidden and enclosing rules.
                visit(Changed::Replaced {
                    displayed_root: argument.place,
                    displayed_path: &argument.path,
                    root,
                    path: path.clone(),
                })?;
                if path.pop().is_some() {
                    visit(Changed::Containing { root, path })?;
                }
            }
        }
        Ok(())
    }

    fn append_lost(
        &self,
        result: &mut Vec<DefaultRequirement>,
        displayed_root: PlaceId,
        displayed_path: &[StructuralPathSegment],
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Judged<()> {
        let displayed = self.declared.at(displayed_root, displayed_path);
        let lost = self
            .at(root, path, self.carrier(root, path)?)?
            .into_iter()
            .filter(|required| {
                matches!(required.origin, DefaultClause::Domain(domain)
                if !displayed.whole.contains(&domain))
            })
            .collect();
        self.append_unique(result, lost);
        Ok(())
    }

    fn append_unique(&self, result: &mut Vec<DefaultRequirement>, added: Vec<DefaultRequirement>) {
        for required in added {
            if !result.contains(&required) {
                result.push(required);
            }
        }
    }

    fn backings(
        &self,
        module: &TerminalModule,
        root: PlaceId,
        path: &[StructuralPathSegment],
        payload: bool,
        before: crate::descriptor_lifetimes::DescriptorPosition,
        references: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
    ) -> Judged<Vec<(PlaceId, Vec<StructuralPathSegment>)>> {
        use crate::descriptor_results::DescriptorCallOrigin;
        let ty = self
            .declared
            .row(root)
            .map(|row| row.1)
            .ok_or_else(|| self.error("a default write has no declared source"))?;
        let borrowed_arrival =
            crate::arrival_parameter(self.machine, root).is_some_and(|parameter| {
                parameter.access != terminal_psi::StructuralAccess::Owned
                    && matches!(
                        self.machine.structural_place(root).map(|place| place.kind),
                        Some(semantic_vocabulary::StructuralPlaceKind::BlockParameter { .. })
                    )
            });
        if !path.contains(&StructuralPathSegment::Referent)
            && !borrowed_arrival
            && !(payload
                && crate::descriptor_results::is_descriptor(
                    self.types.iter(),
                    self.carrier(root, path)?,
                ))
        {
            return Ok(vec![(root, path.to_vec())]);
        }
        // A provider-owned referent has displayed defaults but no enclosing
        // guest object. A mutable lender tied to an exclusive input is not
        // enough to establish this distinction.
        if let Some(crossing) = path
            .iter()
            .position(|step| *step == StructuralPathSegment::Referent)
            && path[crossing + 1..]
                .iter()
                .all(|step| matches!(step, StructuralPathSegment::Field(_)))
            && let Some(descriptor) = crate::descriptor_lifetimes::descriptor_location(
                self.types,
                root,
                ty,
                &path[..crossing],
                None,
            )
            && references
                .symbolic_source_origins(self.machine.id, before, &descriptor)
                .is_some_and(|origins| {
                    !origins.is_empty()
                        && origins.iter().all(|origin| {
                            matches!(
                                origin,
                                crate::descriptor_results::DescriptorSourceOrigin::Provider(_)
                            )
                        })
                })
            && matches!(
                crate::borrowed_loan_source(
                    module,
                    self.machine,
                    references,
                    before,
                    &descriptor,
                    std::iter::empty(),
                )
                .map_err(|detail| self.error(detail))?,
                crate::BorrowedLoanSource::Provider
            )
        {
            return Ok(Vec::new());
        }
        let split = path
            .iter()
            .position(|step| {
                matches!(
                    step,
                    StructuralPathSegment::RuntimeIndex { .. }
                        | StructuralPathSegment::FixedRange { .. }
                )
            })
            .unwrap_or(path.len());
        let (prefix, suffix) = path.split_at(split);
        if suffix.contains(&StructuralPathSegment::Referent) {
            return Err(self.error("default restoration crosses an unresolved indexed reference"));
        }
        let location =
            crate::descriptor_lifetimes::descriptor_location(self.types, root, ty, prefix, None)
                .ok_or_else(|| {
                    self.error("default restoration has no exact descriptor location")
                })?;
        let captured = references
            .captured_storage_family(self.machine.id, before, &location)
            .map_err(|detail| self.error(detail))?;
        captured
            .into_iter()
            .map(|origin| {
                let DescriptorCallOrigin::Argument { source, projection } = origin else {
                    return Err(self.error("default restoration has no exact captured lender"));
                };
                let (root, mut selected) = if let Some(path) = projection.captured_storage_path() {
                    (source, path)
                } else {
                    let unique = references
                        .location(self.machine.id, before, &location)
                        .ok_or_else(|| {
                            self.error("default restoration has no unique incoming lender")
                        })?;
                    let ty = self
                        .declared
                        .row(unique.root)
                        .map(|row| row.1)
                        .ok_or_else(|| {
                            self.error("default restoration lost its incoming carrier")
                        })?;
                    let path =
                        crate::descriptor_lifetimes::descriptor_path(self.types, ty, &unique.path)
                            .ok_or_else(|| {
                                self.error("default restoration lost its incoming storage path")
                            })?;
                    (unique.root, path)
                };
                selected.extend_from_slice(suffix);
                self.carrier(root, &selected)?;
                Ok((root, selected))
            })
            .collect()
    }

    fn declared_operation(&self, operation: &Operation) -> Judged<Vec<DefaultRequirement>> {
        let mut result = Vec::new();
        match &operation.kind {
            OperationKind::EstablishRecord { .. }
            | OperationKind::EstablishScalarCase { .. }
            | OperationKind::EstablishStructuralCase { .. } => {
                let produced = operation
                    .result
                    .structural()
                    .ok_or_else(|| self.error("a construction produces no storage"))?;
                return self.at(produced.place, &[], produced.structural_type);
            }
            OperationKind::EstablishZeroedStructural {} => {
                let produced = operation
                    .result
                    .structural()
                    .ok_or_else(|| self.error("a zero establishment produces no storage"))?;
                if !zero_defaults_supported(self.types, produced.structural_type, &mut Vec::new()) {
                    return Err(self.error("zero storage has no supported default inventory"));
                }
                self.walk(
                    produced.place,
                    &mut Vec::new(),
                    produced.structural_type,
                    true,
                    &mut Vec::new(),
                    &mut result,
                )?;
                return Ok(result);
            }
            _ => {}
        }
        for (root, path) in self.displayed_containers(operation) {
            result.extend(self.owners(root, &path)?);
        }
        Ok(result)
    }

    /// The displayed values that contain what a store or an exclusive loan
    /// changes: each is named by its path, and every prefix of that path is
    /// a containing value too. A store that replaces a whole value starts at
    /// that value's parent.
    fn displayed_containers(
        &self,
        operation: &Operation,
    ) -> Vec<(PlaceId, Vec<StructuralPathSegment>)> {
        let (root, mut path, strict) = match &operation.kind {
            OperationKind::StructuralScalarFieldStore {
                destination, path, ..
            }
            | OperationKind::StructuralByteViewFieldStore {
                destination,
                path,
                field: Some(_),
                ..
            }
            | OperationKind::StoreStructuralField {
                destination,
                path,
                field: Some(_),
                ..
            } => (*destination, path.clone(), false),
            OperationKind::WriteOnlyPrimitiveStore {
                destination, path, ..
            }
            | OperationKind::StoreStructuralField {
                destination,
                path,
                field: None,
                ..
            } => (*destination, path.clone(), true),
            kind => {
                // A callee may change an exclusively lent subplace. The
                // formal's own domains are restored at handback; each strict
                // containing owner owes its defaults at this call instead.
                return kind
                    .structural_arguments()
                    .iter()
                    .filter(|argument| {
                        matches!(
                            argument.access,
                            terminal_psi::StructuralAccess::MutableBorrow
                                | terminal_psi::StructuralAccess::WriteOnlyBorrow
                        )
                    })
                    .filter_map(|argument| {
                        let mut path = argument.path.clone();
                        path.pop().map(|_| (argument.place, path))
                    })
                    .collect();
            }
        };
        if strict && path.pop().is_none() {
            return Vec::new();
        }
        vec![(root, path)]
    }

    fn owners(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Judged<Vec<DefaultRequirement>> {
        let mut result = Vec::new();
        for length in (0..=path.len()).rev() {
            let path = &path[..length];
            if matches!(path.last(), Some(StructuralPathSegment::Case(_))) {
                continue;
            }
            let carrier = self.carrier(root, path)?;
            // Scalar leaves are not containing values. Existing store owners
            // already exclude the replaced leaf itself.
            if matches!(
                self.declaration(carrier)?.shape,
                StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::AtomicCell(_)
            ) {
                return Err(self.error("a default store owner crosses a scalar leaf"));
            }
            result.extend(self.at(root, path, carrier)?);
        }
        Ok(result)
    }

    /// Restate one clause over the common `self` coordinate. Domain declarations
    /// are used only as goals, never as evidence that a mutable value satisfies
    /// its own default.
    pub fn clause(&self, required: &DefaultRequirement) -> Judged<Proposition> {
        match required.origin {
            DefaultClause::Type(position) => self
                .declaration(required.carrier)?
                .requires
                .get(position)
                .cloned()
                .ok_or_else(|| self.error("a default type clause is absent")),
            DefaultClause::Domain(id) => {
                let row = self
                    .domains
                    .binary_search_by_key(&id, |row| row.id)
                    .ok()
                    .map(|position| &self.domains[position])
                    .filter(|row| row.carrier == required.carrier)
                    .ok_or_else(|| self.error("a default domain clause is absent"))?;
                default_domain_clause(self.types, row).map_err(|detail| self.error(detail))
            }
        }
    }
}

pub(crate) fn default_domain_clause(
    types: &[StructuralTypeDeclaration],
    domain: &StructuralDomainDeclaration,
) -> Result<Proposition, &'static str> {
    let self_place = PlaceId::new(1).expect("self has a nonzero coordinate");
    let mut clauses = Vec::new();
    if let Some(predicate) = &domain.predicate {
        let mut bindings = DefaultBindings(Rerooted {
            subject: predicate.subject,
            root: self_place,
            referent: false,
        });
        clauses.push(crate::substitution::clauses::substitute_clause(
            &predicate.proposition,
            &mut bindings,
        )?);
    }
    if let Some(predicate) = domain.byte_predicate {
        if !crate::byte_predicate_carrier(types.iter(), domain.carrier) {
            return Err("a named byte default has a nonbyte carrier");
        }
        clauses.push(Proposition::Equal(
            ScalarTerm::byte_sequence_predicate(predicate, self_place, Vec::new()),
            ScalarTerm::Boolean(true),
        ));
    }
    match clauses.len() {
        0 => Err("a named default has no predicate clause"),
        1 => Ok(clauses.pop().expect("one predicate")),
        _ => Ok(Proposition::Conjunction(clauses)),
    }
}

// Validate zero's old storage vocabulary independently of demand pruning:
// an unsupported zero cannot become supported because it owes no clauses.
fn zero_defaults_supported(
    types: &[StructuralTypeDeclaration],
    carrier: StructuralTypeId,
    active: &mut Vec<StructuralTypeId>,
) -> bool {
    if active.contains(&carrier) {
        return false;
    }
    let Ok(position) = types.binary_search_by_key(&carrier, |row| row.id) else {
        return false;
    };
    active.push(carrier);
    let fields = match &types[position].shape {
        StructuralTypeShape::PrimitiveScalar(_) => {
            active.pop();
            return true;
        }
        StructuralTypeShape::AtomicCell(cell) if cell.bounds().is_none() => {
            active.pop();
            return true;
        }
        shape
            if shape.is_shared_readable_byte_view()
                || matches!(
                    shape,
                    StructuralTypeShape::ElementView {
                        access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
                        ..
                    }
                ) =>
        {
            active.pop();
            return true;
        }
        StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } => {
            let supported = *length == 0 || zero_defaults_supported(types, *element, active);
            active.pop();
            return supported;
        }
        StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Parameter { .. },
        } => {
            let supported = types
                .binary_search_by_key(element, |row| row.id)
                .ok()
                .map(|position| &types[position])
                .is_some_and(|element| {
                    element.requires.is_empty()
                        && element.scalar_restriction.is_none()
                        && matches!(element.shape, StructuralTypeShape::PrimitiveScalar(_))
                });
            active.pop();
            return supported;
        }
        StructuralTypeShape::Record { fields, .. } => fields.as_slice(),
        StructuralTypeShape::Sum { cases } => {
            let Some(first) = cases.first() else {
                active.pop();
                return false;
            };
            &first.fields
        }
        _ => {
            active.pop();
            return false;
        }
    };
    let supported = fields.iter().all(|field| match &field.field_type {
        terminal_psi::StructuralFieldType::Scalar(_)
        | terminal_psi::StructuralFieldType::Atomic(_) => true,
        terminal_psi::StructuralFieldType::Structural(nested) => {
            zero_defaults_supported(types, *nested, active)
        }
        field => {
            let predicates_hold = match field {
                terminal_psi::StructuralFieldType::ByteSequence(_) => true,
                terminal_psi::StructuralFieldType::PredicateByteSequence { predicates, .. } => {
                    predicates.holds_for(&[])
                }
                _ => false,
            };
            predicates_hold
                && matches!(
                    field.byte_sequence_carrier(),
                    Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                    })
                )
        }
    });
    active.pop();
    supported
}

fn type_has_defaults(
    types: &[StructuralTypeDeclaration],
    position: usize,
    memo: &mut [Option<bool>],
    active: &mut [bool],
) -> Result<bool, &'static str> {
    if let Some(value) = memo[position] {
        return Ok(value);
    }
    if std::mem::replace(&mut active[position], true) {
        return Err("recursive inline storage has no finite default inventory");
    }
    let row = &types[position];
    let mut found = !row.requires.is_empty();
    let mut children = Vec::new();
    if let StructuralTypeShape::FixedArray { element, .. } = row.shape {
        children.push(element);
    }
    for (_, field) in row.shape.fields_with_case() {
        if let terminal_psi::StructuralFieldType::Structural(nested) = field.field_type {
            children.push(nested);
        }
    }
    for child in children {
        let child = types
            .binary_search_by_key(&child, |row| row.id)
            .map_err(|_| "an inline default inventory names an absent carrier")?;
        found |= type_has_defaults(types, child, memo, active)?;
    }
    active[position] = false;
    memo[position] = Some(found);
    Ok(found)
}

struct DefaultBindings(Rerooted);

impl crate::substitution::clauses::ClauseBindings for DefaultBindings {
    type Error = &'static str;
    fn value(&mut self, id: ValueId, ty: ScalarType) -> Result<ScalarTerm, Self::Error> {
        self.0.value(id, ty)
    }
    fn field(
        &mut self,
        version: ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        ty: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        if version != ContentPlaceVersion::Current {
            return Err("a default predicate observes invocation-entry contents");
        }
        self.0.field(version, root, path, ty)
    }
    fn extent(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        ty: IntegerType,
    ) -> Result<ScalarTerm, Self::Error> {
        self.0.extent(root, path, ty)
    }
    fn byte_predicate(
        &mut self,
        predicate: semantic_vocabulary::ByteSequencePredicate,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<ScalarTerm, Self::Error> {
        self.0.byte_predicate(predicate, root, path)
    }
    fn case(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: StructuralCaseId,
    ) -> Result<bool, Self::Error> {
        Err("a default case predicate requires symbolic rebasing")
    }
    fn case_predicate(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        case: StructuralCaseId,
    ) -> Result<Proposition, Self::Error> {
        Ok(Proposition::StructuralCaseMembership {
            subject: semantic_vocabulary::StructuralCaseSubject::new(
                self.0.root,
                self.0.path(root, path)?,
            ),
            case,
        })
    }
}

fn domain(module: &TerminalModule, id: StructuralDomainId) -> Option<&StructuralDomainDeclaration> {
    module
        .types.structural_domains
        .binary_search_by_key(&id, |domain| domain.id)
        .ok()
        .map(|index| &module.types.structural_domains[index])
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
            // An array has no record field to discharge a domain predicate
            // for each element. The empty theory is explicitly retained as Truth.
            && (row.path.last() != Some(&StructuralPathSegment::AllElements)
                || (domain.complete_predicate_theory
                    && domain.predicate.as_ref().is_some_and(|predicate| {
                        predicate.proposition == Proposition::Truth
                    })))
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
    for (_, root, memberships) in &places.declared {
        let root = *root;
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
            // Byte and content judgments check their predicates separately.
            // Every qualification must still name a valid storage path.
            let Some(carrier) =
                crate::qualification_path_carrier(module.types.structural_types.iter(), root, path)
            else {
                return malformed(
                    places.machine(),
                    "a structural place's domain names an absent path",
                );
            };
            if judged_elsewhere(module, id) {
                continue;
            }
            if carrier != declared.carrier {
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

    fn byte_predicate(
        &mut self,
        predicate: semantic_vocabulary::ByteSequencePredicate,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<ScalarTerm, Self::Error> {
        Ok(ScalarTerm::byte_sequence_predicate(
            predicate,
            self.root,
            self.path(root, path)?,
        ))
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
    facts: &crate::membership_facts::MembershipFacts,
) -> Judged<()> {
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
        if held.iter().any(|id| {
            !shown.contains(id)
                && domain(module, id.0)
                    .and_then(|row| row.access_policy.as_ref())
                    .is_some_and(|plan| plan.has_operation_rules())
        }) {
            return malformed(
                machine,
                "removing this domain would lose its memory access rules",
            );
        }
        if !change.establishments.is_empty() {
            return malformed(machine, "a structural erasure cannot establish a domain");
        }
        return Ok(());
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
            let retained = module.proofs.scalar_block_invariants.iter().any(|invariant| {
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
            .missing_membership_requirements(&module.types.structural_domains, &held)
            .is_none_or(|missing| missing.iter().any(|node| node.id != declared.id));
        let forwarded = facts.structural_on_edge(module, change, *referent, *id);
        if missing && !forwarded {
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
            (Some(_), None) if forwarded => {}
            (Some(routes), None) => {
                let mut error = StructuralQualificationError {
                    machine,
                    detail: "a structural cast lacks both an existing membership and an authorized issuer route",
                    denied_establishment: None,
                };
                // An unrun access plan leaves an empty issuer placeholder.
                // Only its checked, explicitly empty plan establishes that
                // nobody may issue. An allowed issuer missing its emitted
                // row remains a malformed product.
                let explicitly_empty = declared
                    .access_policy
                    .as_ref()
                    .is_some_and(|plan| plan.issuance.as_ref().is_some_and(Vec::is_empty));
                if (!routes.is_empty() || explicitly_empty)
                    && excludes_machine(module, places.machine, routes)
                {
                    error.denied_establishment = Some(DeniedEstablishment {
                        machine: places.machine.id,
                        edge: change.edge,
                        domain: declared.semantic_domain,
                    });
                }
                return Err(error);
            }
            (Some(routes), Some(row)) => {
                let Some(declaration) = declaration else {
                    return malformed(machine, "a structural issuer lost its checked declaration");
                };
                let authorized = routes.get(row.route as usize).is_some_and(|route| {
                    issuer_route_authorizes(module, places.machine, declaration, route)
                });
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
    Ok(())
}

fn byte_change_judgment(
    module: &TerminalModule,
    places: &Declared<'_>,
    change: &StructuralQualificationChange,
    source: &StructuralArgument,
    destination: &Memberships,
    mut incoming: Option<(
        &mut crate::descriptor_results::DescriptorAnalysis<'_>,
        crate::descriptor_results::DescriptorPosition,
    )>,
) -> Judged<Option<bool>> {
    let machine = places.machine();
    if !source.path.is_empty() || change.source != source.place {
        return malformed(machine, "a byte qualification change names another source");
    }
    let (held, _) = displayed(&places.argument(source));
    let (shown, _) = displayed(destination);
    let added = shown
        .into_iter()
        .filter(|domain| !held.contains(domain))
        .collect::<Vec<_>>();
    let mut byte_domain = false;
    let mut established = true;
    for (id, referent) in &added {
        let declared = domain(module, *id).ok_or(StructuralQualificationError {
            denied_establishment: None,
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
                denied_establishment: None,
                machine,
                detail: "a byte qualification change has no declared destination carrier",
            })?;
        let path = if *referent {
            &[StructuralPathSegment::Referent][..]
        } else {
            &[][..]
        };
        let carrier =
            crate::runtime_structural_path_tip(module.types.structural_types.iter(), root, path)
                .ok_or(StructuralQualificationError {
                    denied_establishment: None,
                    machine,
                    detail: "a byte qualification change has no declared displayed carrier",
                })?
                .id;
        if !crate::byte_predicate_carrier(module.types.structural_types.iter(), carrier) {
            return malformed(
                machine,
                "a byte qualification change has a non-byte carrier",
            );
        }
        let predicates = crate::byte_qualification_predicates_in(
            module.types.structural_domains.iter(),
            carrier,
            std::slice::from_ref(id),
        )
        .ok_or(StructuralQualificationError {
            denied_establishment: None,
            machine,
            detail: "a byte qualification change is not a complete primitive byte domain",
        })?;
        let mut argument = source.clone();
        argument.path.extend_from_slice(path);
        let mut supplied =
            crate::byte_sequence_argument_predicates(module, places.machine, &argument).ok_or(
                StructuralQualificationError {
                    denied_establishment: None,
                    machine,
                    detail: "a byte qualification change has no reconstructed source predicates",
                },
            )?;
        if let Some((analysis, position)) = incoming.as_mut()
            && let Some(facts) = crate::incoming_byte_sequence_argument_predicates(
                module,
                places.machine,
                analysis,
                *position,
                &argument,
            )
        {
            for predicate in facts.predicates() {
                supplied.insert(predicate);
            }
        }
        byte_domain = true;
        established &= supplied.implies(predicates);
    }
    Ok(byte_domain.then_some(established))
}

fn validate_machine(
    module: &TerminalModule,
    machine: &TerminalMachine,
    facts: &crate::membership_facts::MembershipFacts,
    mut incoming: Option<&mut crate::descriptor_results::DescriptorAnalysis<'_>>,
) -> Judged<Vec<(EdgeId, bool)>> {
    let places = Declared::new(machine)?;
    let id = Some(machine.id);
    validate_declared(module, &places)?;
    let changes: Vec<_> = module
        .types.structural_qualification_changes
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
                        let position = u32::try_from(block.operations.len()).map_err(|_| {
                            StructuralQualificationError {
                                denied_establishment: None,
                                machine: id,
                                detail: "a byte qualification use exceeds its descriptor graph",
                            }
                        })?;
                        validate_change(
                            module,
                            &places,
                            changes[index],
                            argument,
                            &expected,
                            target.id,
                            facts,
                        )?;
                        if let Some(established) = byte_change_judgment(
                            module,
                            &places,
                            changes[index],
                            argument,
                            &expected,
                            incoming
                                .as_deref_mut()
                                .map(|analysis| (analysis, (block.id, position))),
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
                        // Other cases retain their conditional field domains
                        // in the result type, but supply no storage in this
                        // literal. validate_declared checked their full paths.
                        let inactive_case = matches!(
                            (case, row.path.first()),
                            (Some(active), Some(StructuralPathSegment::Case(declared)))
                                if active != *declared
                        );
                        !inactive_case
                            && !covered
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
                OperationKind::EstablishElementView {
                    source,
                    region: terminal_psi::ElementViewRegion::WholeCollection,
                    ..
                }
                | OperationKind::EstablishMutableElementView {
                    source,
                    region: terminal_psi::ElementViewRegion::WholeCollection,
                    ..
                }
                | OperationKind::ElementViewSubslice { source, .. } => {
                    let mut expected = Memberships::default();
                    if let Some((_, root, memberships)) = places.row(source.place)
                        && !memberships.projected.is_empty()
                        && let Some(StructuralTypeShape::ElementView { element, .. }) = result_type
                            .and_then(|id| module.structural_type(id))
                            .map(|row| &row.shape)
                        && let Some(collection) = crate::runtime_structural_path_tip(
                            module.types.structural_types.iter(),
                            *root,
                            &source.path,
                        )
                        && crate::collection_backing(
                            module.types.structural_types.iter(),
                            collection.id,
                            *element,
                        )
                        .is_some()
                    {
                        // A window keeps the domains of each element. The
                        // backing's own domains do not describe the descriptor.
                        expected.projected = memberships.beneath(&source.path).projected;
                        expected
                            .projected
                            .retain(StructuralPathQualification::is_element_membership);
                    }
                    require_exact(
                        &places,
                        &expected,
                        &produced,
                        "an element view does not keep its source elements' exact domains",
                    )?;
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
                        denied_establishment: None,
                        machine: id,
                        detail: "zero provisioning has no declared result carrier",
                    })?;
                    let byte_projected = produced.projected.iter().filter(|row| {
                        domain(module, row.domain)
                            .is_some_and(|domain| domain.byte_predicate.is_some())
                    });
                    let byte_defaults_hold =
                        crate::structural::zeroed_structural::zeroed_byte_qualifications_hold(
                            module.types.structural_types.iter(),
                            module.types.structural_domains.iter(),
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
        crate::qualification_path_carrier(module.types.structural_types.iter(), root_type, path)
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

/// Read caller descriptions through the canonical identity grammar. The
/// unversioned toolchain name is descriptive: its retained source bytes must
/// match the exact revision on the module's declaration before it can link.
type CallerNominal = (OwnerForm, Option<[u8; 32]>, String);
#[derive(Default)]
struct MachineCallerDescription {
    depth: usize,
    remaining: usize,
    applications: usize,
    argument: u32,
    root_names: Vec<Option<String>>,
    static_arguments: Vec<String>,
    machine: Vec<CallerNominal>,
    entry: Vec<CallerNominal>,
}
impl TypeIdentityPackageOwnerVisitor for MachineCallerDescription {
    fn enter(&mut self) -> Result<(), TypeIdentityVisitError> {
        if self.depth == 128 {
            return Err(TypeIdentityVisitError::ResourceLimitExceeded);
        }
        self.depth += 1;
        Ok(())
    }
    fn leave(&mut self) {
        self.depth -= 1;
    }
    fn reserve(&mut self, bytes: usize) -> Result<(), TypeIdentityVisitError> {
        self.remaining = self
            .remaining
            .checked_sub(bytes)
            .ok_or(TypeIdentityVisitError::ResourceLimitExceeded)?;
        Ok(())
    }
    fn package_owner(&mut self, _: [u8; 32]) -> Result<(), TypeIdentityVisitError> {
        Ok(())
    }
    fn embedded_name(&mut self, _: &str) -> Result<(), TypeIdentityVisitError> {
        Ok(())
    }
    fn descriptive_toolchain_owner(&mut self) -> Result<(), TypeIdentityVisitError> {
        Ok(())
    }
    fn nominal_declaration(
        &mut self,
        owner: OwnerForm,
        digest: Option<[u8; 32]>,
        path: &str,
    ) -> Result<(), TypeIdentityVisitError> {
        let rows = match (self.applications, self.argument) {
            (0, _) | (1, 0) => &mut self.machine,
            (1, 1) => &mut self.entry,
            _ => return Ok(()),
        };
        self.remaining = self
            .remaining
            .checked_sub(path.len())
            .ok_or(TypeIdentityVisitError::ResourceLimitExceeded)?;
        rows.push((owner, digest, path.to_owned()));
        Ok(())
    }
    fn enter_static_application(&mut self) -> Result<(), TypeIdentityVisitError> {
        self.applications += 1;
        Ok(())
    }
    fn leave_static_application(&mut self) {
        self.applications -= 1;
    }
    fn enter_static_argument(&mut self, ordinal: u32) -> Result<(), TypeIdentityVisitError> {
        if self.applications == 1 {
            self.argument = ordinal;
            self.root_names.push(None);
        }
        Ok(())
    }
    fn static_argument(
        &mut self,
        ordinal: u32,
        identity: &str,
    ) -> Result<(), TypeIdentityVisitError> {
        if self.applications == 1 && ordinal >= 3 {
            self.remaining = self
                .remaining
                .checked_sub(identity.len())
                .ok_or(TypeIdentityVisitError::ResourceLimitExceeded)?;
            self.static_arguments.push(identity.to_owned());
        }
        Ok(())
    }
    fn static_name(&mut self, name: &str) -> Result<(), TypeIdentityVisitError> {
        if self.applications == 1 && self.argument < 3 {
            let row = self
                .root_names
                .last_mut()
                .ok_or(TypeIdentityVisitError::MalformedIdentity)?;
            if row.is_some() {
                return Err(TypeIdentityVisitError::MalformedIdentity);
            }
            self.remaining = self
                .remaining
                .checked_sub(name.len())
                .ok_or(TypeIdentityVisitError::ResourceLimitExceeded)?;
            *row = Some(name.to_owned());
        }
        Ok(())
    }
}

fn read_machine_caller_declaration(
    module: &TerminalModule,
    description: &schema_reflection::CallableDescription,
    declaration: &terminal_psi::NominalDeclarationIdentity,
    callable: terminal_psi::TerminalIdentityIndex,
) -> Judged<MachineCallerDescription> {
    let error = |detail| StructuralQualificationError {
        denied_establishment: None,
        machine: None,
        detail,
    };
    let applied = std::str::from_utf8(&description.1[1])
        .map_err(|_| error("machine caller lost its applied identity"))?;
    let mut described = MachineCallerDescription {
        remaining: applied.len().saturating_mul(3),
        ..Default::default()
    };
    visit_static_argument_package_owners(applied, &mut described)
        .map_err(|_| error("machine caller has an invalid complete application description"))?;
    if described.root_names.len() < 3
        || described.root_names[..3].iter().any(Option::is_none)
        || described.machine.len() != 1
        || described.entry.len() != 1
    {
        return Err(error(
            "machine caller lost its original machine, entry or signature",
        ));
    }
    let exact = module
        .identities
        .text(declaration.identity())
        .ok_or_else(|| error("machine caller has no exact source declaration"))?;
    let mut retained = MachineCallerDescription {
        remaining: exact.len().saturating_mul(3),
        ..Default::default()
    };
    visit_type_identity_package_owners(exact, &mut retained)
        .map_err(|_| error("machine caller declaration has an invalid canonical identity"))?;
    let [actual] = retained.machine.as_slice() else {
        return Err(error(
            "machine caller declaration has no unique nominal owner",
        ));
    };
    let declared = &described.machine[0];
    let revision = source_provenance::toolchain_source_revision_digest(&description.1[2]);
    let same_owner = match (actual.0, declared.0) {
        (OwnerForm::Package, OwnerForm::Package) => actual.1 == declared.1,
        (OwnerForm::ToolchainSource, OwnerForm::Toolchain) => actual.1 == Some(revision),
        (OwnerForm::ToolchainSource, OwnerForm::ToolchainSource) => {
            actual.1 == declared.1 && actual.1 == Some(revision)
        }
        _ => false,
    };
    if !same_owner || actual.2 != declared.2 {
        return Err(error(
            "machine caller describes another source declaration or revision",
        ));
    }
    // An applied caller additionally owes its checked static argument account
    // at the operation. The unspecialized identity already has no arguments
    // to hide, so its complete signature must match here too.
    if described.root_names.len() == 3
        && module.identities.text(callable) != described.root_names[2].as_deref()
    {
        return Err(error("machine caller route has another declared signature"));
    }
    Ok(described)
}

fn machine_caller_application_matches(
    module: &TerminalModule,
    current: &TerminalMachine,
    description: &schema_reflection::CallableDescription,
    described: &MachineCallerDescription,
) -> Judged<bool> {
    let error = |detail| StructuralQualificationError {
        denied_establishment: None,
        machine: Some(current.id),
        detail,
    };
    let revision = source_provenance::toolchain_source_revision_digest(&description.1[2]);
    let declaration = current
        .declaration
        .as_ref()
        .ok_or_else(|| error("current policy caller has no checked declaration"))?;
    let (entry, signature) = match &declaration.specialization {
        Some(application) => (&application.template_entry, application.template_callable),
        None => (&declaration.state, declaration.callable),
    };
    if described.root_names[2].as_deref() != module.identities.text(signature) {
        return Ok(false);
    }
    let text = module
        .identities
        .text(entry.identity())
        .ok_or_else(|| error("current caller has no exact original entry"))?;
    let mut expected = MachineCallerDescription {
        remaining: text.len().saturating_mul(3),
        ..Default::default()
    };
    visit_type_identity_package_owners(text, &mut expected)
        .map_err(|_| error("current caller's original entry has an invalid canonical identity"))?;
    let [entry] = expected.machine.as_slice() else {
        return Err(error("current caller has no unique original entry"));
    };
    let described_entry = &described.entry[0];
    let entry_owner_matches = match (entry.0, described_entry.0) {
        (OwnerForm::Package, OwnerForm::Package) => entry.1 == described_entry.1,
        (OwnerForm::ToolchainSource, OwnerForm::Toolchain) => entry.1 == Some(revision),
        (OwnerForm::ToolchainSource, OwnerForm::ToolchainSource) => {
            entry.1 == described_entry.1 && entry.1 == Some(revision)
        }
        _ => false,
    };
    if !entry_owner_matches || entry.2 != described_entry.2 {
        return Ok(false);
    }
    let arguments = &described.static_arguments;
    match &declaration.specialization {
        None => Ok(arguments.is_empty()),
        Some(specialization) => {
            let application = current
                .closed_reach_application
                .as_ref()
                .ok_or_else(|| error("current caller has no checked static argument account"))?;
            if specialization.capture.is_some() || application.capture.is_some() {
                return Err(error(
                    "current caller needs its complete checked capture argument account",
                ));
            }
            if specialization.commitment != application.specialization_commitment
                || specialization.template_semantic_declaration != application.template_identity
                || arguments.len() != application.telescope.len()
            {
                return Ok(false);
            }
            crate::calls::closed_reach::validate_machine_application_arguments(module, current)
                .map_err(error)?;
            for (argument, parameter) in arguments.iter().zip(&application.telescope) {
                match parameter {
                    terminal_psi::ClosedReachParameter::Type { argument: expected } => {
                        if argument != expected {
                            return Ok(false);
                        }
                    }
                    terminal_psi::ClosedReachParameter::Const { argument: expected } => {
                        if expected.index_identity().as_deref() != Some(argument.as_str()) {
                            return Ok(false);
                        }
                    }
                    terminal_psi::ClosedReachParameter::Machine(_)
                    | terminal_psi::ClosedReachParameter::Proposition { .. } => {
                        return Err(error(
                            "current caller needs its complete checked machine or proof argument account",
                        ));
                    }
                }
            }
            Ok(true)
        }
    }
}

/// Compare a description with this module's actual checked machine and its
/// complete entry/application. Matching a declaration supplies no invocation,
/// read, write or backing permission; those operation judgments remain separate.
pub fn caller_matches_machine(
    module: &TerminalModule,
    current: MachineId,
    caller: &(
        schema_reflection::CallableDescription,
        StructuralEstablishmentRoute,
    ),
) -> Judged<bool> {
    let current = module
        .machine(current)
        .ok_or(StructuralQualificationError {
            denied_establishment: None,
            machine: Some(current),
            detail: "policy caller is absent from the actual module",
        })?;
    let declaration = current
        .declaration
        .as_ref()
        .ok_or(StructuralQualificationError {
            denied_establishment: None,
            machine: Some(current.id),
            detail: "policy caller has no checked declaration",
        })?;
    let (description, route) = caller;
    let StructuralEstablishmentRoute::ExactMachine {
        declaration: owner,
        machine_identity,
    } = route
    else {
        return malformed(
            Some(current.id),
            "requirement caller needs its exact checked public application",
        );
    };
    if description.0 {
        return Ok(false);
    }
    let (expected_owner, semantic, callable, commitment) = match &declaration.specialization {
        Some(application) => (
            &application.template,
            application.template_semantic_declaration,
            application.template_callable,
            Some(&application.commitment),
        ),
        None => (
            &declaration.machine,
            declaration.semantic_declaration,
            declaration.callable,
            None,
        ),
    };
    let Some(signature) = module.identities.text(callable) else {
        return malformed(
            Some(current.id),
            "policy caller's checked signature is absent",
        );
    };
    let expected =
        terminal_psi::CheckedMachineDeclaration::callable_evidence_identity(signature, commitment);
    if owner != expected_owner
        || module.identities.text(*machine_identity) != Some(expected.as_str())
        || module.identities.text(semantic).map(str::as_bytes) != Some(description.1[0].as_slice())
    {
        return Ok(false);
    }
    let described = read_machine_caller_declaration(module, description, owner, *machine_identity)?;
    machine_caller_application_matches(module, current, description, &described)
}

/// Validate each rule against the source declarations retained by this module.
/// A lookalike record, case or field cannot satisfy a policy for another one.
pub fn validate_access_policy(
    module: &TerminalModule,
    domain: &StructuralDomainDeclaration,
) -> Judged<()> {
    let Some(plan) = &domain.access_policy else {
        return Ok(());
    };
    let error = |detail| StructuralQualificationError {
        denied_establishment: None,
        machine: None,
        detail,
    };
    let projected = plan.clone().try_bind(
        |(node, _)| Ok::<_, StructuralQualificationError>(node),
        |(description, _)| Ok(description),
    )?;
    let normalized = access_admission::normalize_access_plan(projected.clone()).map_err(|_| {
        error("domain access rules have invalid semantic coordinates or descriptions")
    })?;
    if normalized != projected {
        return Err(error("domain access rules are not in normalized order"));
    }
    let carrier = module
        .structural_type(domain.carrier)
        .ok_or_else(|| error("domain access rules have no carrier declaration"))?;
    let owner = module
        .identities
        .text(carrier.declaration_identity)
        .filter(|identity| !identity.is_empty())
        .ok_or_else(|| error("domain access subject has no exact source declaration"))?;
    let subject = module
        .identities
        .text(domain.subject_identity)
        .filter(|identity| !identity.is_empty())
        .ok_or_else(|| error("domain access subject has no exact applied identity"))?;
    if plan.schema.declaration().owner_identity != owner
        || plan.schema.declaration().application_identity.as_deref() != Some(subject)
    {
        return Err(error(
            "domain access schema describes another subject declaration or application",
        ));
    }
    let mut cases = carrier
        .shape
        .cases()
        .iter()
        .map(|case| {
            let identity = module
                .identities
                .text(case.declaration_identity)
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| error("domain access case has no exact source declaration"))?;
            Ok((identity, case))
        })
        .collect::<Judged<Vec<_>>>()?;
    cases.sort_unstable_by(|left, right| left.0.cmp(right.0));
    if cases.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(error(
            "domain access carrier repeats a source case declaration",
        ));
    }
    let described_cases = plan
        .schema
        .declaration()
        .members
        .iter()
        .filter_map(|member| match plan.schema.node(*member) {
            schema_reflection::SchemaNode::Case(case) => Some(case),
            _ => None,
        })
        .collect::<Vec<_>>();
    if described_cases.len() != cases.len()
        || described_cases.iter().any(|case| {
            cases
                .binary_search_by(|(identity, _)| identity.cmp(&case.member_identity.as_str()))
                .is_err()
        })
    {
        return Err(error(
            "domain access schema differs from its carrier's source cases",
        ));
    }
    let mut fields = carrier
        .shape
        .fields_with_case()
        .map(|(case, field)| {
            let name = module
                .identities
                .text(field.declaration_identity)
                .filter(|name| !name.is_empty())
                .ok_or_else(|| {
                    error("domain access schema has a field without a source declaration")
                })?;
            Ok((name, case, field))
        })
        .collect::<Judged<Vec<_>>>()?;
    fields.sort_unstable_by(|left, right| left.0.cmp(right.0));
    if fields.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(error(
            "domain access carrier repeats a source field declaration",
        ));
    }
    let governed = plan
        .schema
        .governed_fields()
        .map_err(|_| error("domain access schema lost its complete subject field roster"))?;
    if governed.len() != fields.len()
        || governed.iter().any(|(_, field)| {
            fields
                .binary_search_by(|(name, _, _)| name.cmp(&field.member_identity.as_str()))
                .is_err()
        })
    {
        return Err(error(
            "domain access schema differs from its carrier's source fields",
        ));
    }
    // Every described field keeps its source owner, even when no rule names
    // it. A later policy cannot borrow an unvalidated case or erased field.
    for (_, description) in &governed {
        let index = fields
            .binary_search_by(|(name, _, _)| name.cmp(&description.member_identity.as_str()))
            .map_err(|_| error("domain access schema lost its source field"))?;
        let (_, case, field) = fields[index];
        let described_case = match plan.schema.node(description.owner_case) {
            schema_reflection::SchemaNode::Case(case) if description.owner_case.is_valid() => {
                Some(case.member_identity.as_str())
            }
            _ if !description.owner_case.is_valid() => None,
            _ => return Err(error("domain access field has no valid schema case owner")),
        };
        let actual_case = case
            .map(|case| {
                module
                    .identities
                    .text(case.declaration_identity)
                    .filter(|identity| !identity.is_empty())
                    .ok_or_else(|| error("domain access field lost its source case owner"))
            })
            .transpose()?;
        if described_case != actual_case {
            return Err(error("domain access field belongs to another source case"));
        }
        if description.erased != field.relevance.is_erased() {
            return Err(error("domain access field changed its runtime relevance"));
        }
    }
    let region = |(node, path): &(
        schema_reflection::SchemaNodeHandle,
        Option<Vec<StructuralPathSegment>>,
    )|
     -> Judged<()> {
        if *node == plan.schema.root() {
            return if path.as_deref() == Some(&[]) {
                Ok(())
            } else {
                Err(error("whole-subject access rule has another storage path"))
            };
        }
        let index = governed
            .binary_search_by_key(node, |(node, _)| *node)
            .map_err(|_| error("domain access rule names a field outside its subject"))?;
        let description = governed[index].1;
        let index = fields
            .binary_search_by(|(name, _, _)| name.cmp(&description.member_identity.as_str()))
            .map_err(|_| error("domain access rule lost its source field"))?;
        let (_, case, field) = fields[index];
        if description.erased {
            return if path.is_none() {
                Ok(())
            } else {
                Err(error("erased access field has a fabricated storage path"))
            };
        }
        let matches = match (case, path.as_deref()) {
            (None, Some([StructuralPathSegment::Field(identity)])) => *identity == field.identity,
            (
                Some(case),
                Some(
                    [
                        StructuralPathSegment::Case(selected),
                        StructuralPathSegment::Field(identity),
                    ],
                ),
            ) => *selected == case.id && *identity == field.identity,
            _ => false,
        };
        if matches {
            Ok(())
        } else {
            Err(error(
                "domain access field has another declaration's storage path",
            ))
        }
    };
    for (coordinate, _) in plan.readers.iter().chain(&plan.writers) {
        region(coordinate)?;
    }
    for (coordinate, _) in &plan.observations {
        region(coordinate)?;
    }
    for (description, route) in plan.issuance.iter().flatten().chain(
        plan.readers
            .iter()
            .chain(&plan.writers)
            .flat_map(|(_, callers)| callers),
    ) {
        if let StructuralEstablishmentRoute::ExactMachine {
            declaration,
            machine_identity,
        } = route
        {
            read_machine_caller_declaration(module, description, declaration, *machine_identity)?;
        }
        if description.0 != !matches!(route, StructuralEstablishmentRoute::ExactMachine { .. })
            || module
                .identities
                .text(route.identity())
                .is_none_or(str::is_empty)
            || module
                .identities
                .text(route.declaration().identity())
                .is_none_or(str::is_empty)
        {
            return Err(error(
                "domain access caller lost its complete declared route",
            ));
        }
    }
    Ok(())
}

/// Each module numbers its cases and fields independently. Compare the rules
/// through their resolved source declarations after validating each module's
/// links, so matching numbers alone cannot make different policies equal.
pub fn same_access_policy(
    left: &TerminalModule,
    source: &StructuralDomainDeclaration,
    right: &TerminalModule,
    target: &StructuralDomainDeclaration,
) -> bool {
    fn semantic_plan(
        module: &TerminalModule,
        domain: &StructuralDomainDeclaration,
    ) -> Option<
        access_admission::AccessPlan<
            schema_reflection::SchemaNodeHandle,
            (schema_reflection::CallableDescription, (u8, String, String)),
        >,
    > {
        validate_access_policy(module, domain).ok()?;
        domain
            .access_policy
            .clone()?
            .try_bind(
                |(node, _)| Ok::<_, ()>(node),
                |(description, route)| {
                    let kind = match route {
                        StructuralEstablishmentRoute::ExactMachine { .. } => 0,
                        StructuralEstablishmentRoute::CheckedRequirement { .. } => 1,
                        StructuralEstablishmentRoute::BoundaryRequirement { .. } => 2,
                    };
                    let declaration = route.declaration().identity();
                    let callable = route.identity();
                    if module.identities.is_invocation_local(declaration)
                        || module.identities.is_invocation_local(callable)
                    {
                        return Err(());
                    }
                    let owner = module
                        .identities
                        .text(declaration)
                        .filter(|name| !name.is_empty())
                        .ok_or(())?;
                    let callable = module
                        .identities
                        .text(callable)
                        .filter(|name| !name.is_empty())
                        .ok_or(())?;
                    Ok((description, (kind, owner.to_owned(), callable.to_owned())))
                },
            )
            .ok()
    }
    match (&source.access_policy, &target.access_policy) {
        (None, None) => true,
        (Some(_), Some(_)) => semantic_plan(left, source)
            .zip(semantic_plan(right, target))
            .is_some_and(|(a, b)| a == b),
        _ => false,
    }
}

// Access rules follow the value and its live storage. Check every possible
// backing and carry each exact argument relationship through its callers.
mod operation_permissions;

fn validate_access_policies(module: &TerminalModule) -> Judged<()> {
    for domain in &module.types.structural_domains {
        validate_access_policy(module, domain)?;
        // A received program can already hold a qualified input. Source
        // formation refusal alone cannot protect its operations. Caller
        // rules are checked below; profile observations and reach still need
        // their actual supply/lifecycle consumers.
        // Description-only plans can still be validated separately.
        if domain
            .access_policy
            .as_ref()
            .is_some_and(|plan| plan.requires_observation_backing() || plan.reach.services().len() != 0)
        {
            return malformed(
                None,
                "memory access checks for these saved rules are not implemented yet",
            );
        }
    }
    Ok(())
}

/// Check the exact membership transfers, domain theories and issuer routes.
/// Byte predicate truth is a separate establishment judgment at each cast edge.
pub fn validate_shape(module: &TerminalModule) -> Judged<()> {
    let bodies = module
        .machines
        .iter()
        .map(|machine| machine.id)
        .collect::<Vec<_>>();
    validate_shape_for_bodies(module, &bodies)
}

/// Check the complete vocabulary and prove returned memberships for the exact
/// checked bodies. Artifact ingress uses every module body through validate_shape.
pub fn validate_shape_for_bodies(module: &TerminalModule, bodies: &[MachineId]) -> Judged<()> {
    validate_access_policies(module)?;
    let facts = crate::membership_facts::analyze_bodies(module, bodies).map_err(|error| {
        StructuralQualificationError {
            denied_establishment: None,
            machine: error.machine,
            detail: error.detail,
        }
    })?;
    for machine in &module.machines {
        validate_machine(module, machine, &facts, None)?;
    }
    operation_permissions::validate(module)?;
    validate_change_owners(module)
}

fn validate_change_owners(module: &TerminalModule) -> Judged<()> {
    if module
        .types.structural_qualification_changes
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

/// Judge primitive byte casts from their actual arguments, reusing the
/// checker's prepared descriptor and storage history. False is reported as
/// unestablished; artifact ingress requires every result to be true.
pub fn byte_qualification_change_judgments_in(
    module: &TerminalModule,
    machine: &TerminalMachine,
    analysis: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
) -> Judged<Vec<(EdgeId, bool)>> {
    let places = Declared::new(machine)?;
    let mut changes = std::collections::BTreeMap::new();
    for change in module
        .types.structural_qualification_changes
        .iter()
        .filter(|change| change.machine == machine.id)
    {
        if changes
            .insert((change.edge, change.argument_ordinal), change)
            .is_some()
        {
            return malformed(
                Some(machine.id),
                "byte qualification change repeats an exact successor argument",
            );
        }
    }
    let mut used = std::collections::BTreeSet::new();
    let mut judgments = Vec::new();
    for block in &machine.blocks {
        let position =
            u32::try_from(block.operations.len()).map_err(|_| StructuralQualificationError {
                denied_establishment: None,
                machine: Some(machine.id),
                detail: "byte qualification cut exceeds descriptor coordinates",
            })?;
        let edges = block.terminator.successor_edges();
        let arguments = block.terminator.successor_block_arguments();
        if edges.len() != arguments.len() {
            return malformed(
                Some(machine.id),
                "byte qualification successor lost its edge",
            );
        }
        for ((edge, target), arguments) in edges.into_iter().zip(arguments) {
            let target = machine.block(&target).ok_or(StructuralQualificationError {
                denied_establishment: None,
                machine: Some(machine.id),
                detail: "byte qualification successor names an absent block",
            })?;
            for (ordinal, source) in arguments.iter().enumerate() {
                let ordinal = u32::try_from(ordinal).map_err(|_| StructuralQualificationError {
                    denied_establishment: None,
                    machine: Some(machine.id),
                    detail: "byte qualification argument ordinal overflow",
                })?;
                let Some(change) = changes.get(&(edge, ordinal)) else {
                    continue;
                };
                let parameter = target
                    .structural_parameters
                    .get(ordinal as usize)
                    .filter(|parameter| parameter.place == change.destination)
                    .ok_or(StructuralQualificationError {
                        denied_establishment: None,
                        machine: Some(machine.id),
                        detail: "byte qualification change names another destination",
                    })?;
                used.insert((edge, ordinal));
                if let Some(established) = byte_change_judgment(
                    module,
                    &places,
                    change,
                    source,
                    &Memberships::of(
                        &parameter.qualifications,
                        &parameter.projected_qualifications,
                    ),
                    Some((&mut *analysis, (block.id, position))),
                )? {
                    judgments.push((edge, established));
                }
            }
        }
    }
    if used.len() != changes.len() {
        return malformed(
            Some(machine.id),
            "byte qualification change names no successor argument",
        );
    }
    Ok(judgments)
}

/// The complete structural membership judgment required at artifact ingress.
pub fn validate(module: &TerminalModule) -> Judged<()> {
    validate_access_policies(module)?;
    let facts =
        crate::membership_facts::analyze(module).map_err(|error| StructuralQualificationError {
            denied_establishment: None,
            machine: error.machine,
            detail: error.detail,
        })?;
    // Prepare one shared closure for actual cast roots. An unsupported body
    // remains absent and cannot add evidence; it must not erase independent
    // supported rows. Modules without casts need no descriptor preparation.
    let graph = if module.types.structural_qualification_changes.is_empty() {
        Vec::new()
    } else {
        crate::descriptor_results::terminal_graph_rows(
            module,
            module
                .types.structural_qualification_changes
                .iter()
                .map(|change| change.machine),
            crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
            &[],
        )
        .into_iter()
        .filter_map(Result::ok)
        .collect()
    };
    let mut analysis = crate::descriptor_results::DescriptorAnalysis::with_borrowed_storage_slots(
        &module.types.structural_types,
        &graph,
    );
    for machine in &module.machines {
        if validate_machine(module, machine, &facts, Some(&mut analysis))?
            .iter()
            .any(|(_, established)| !established)
        {
            return malformed(
                Some(machine.id),
                "a byte transfer's source does not establish its primitive byte-domain qualifications",
            );
        }
    }
    validate_routed_writes(module)?;
    operation_permissions::validate(module)?;
    validate_change_owners(module)
}

/// A stored module gets the judgment the lowered-form checker gives a source
/// program: no machine outside a domain's routes changes part of a value
/// that carries the domain. The borrowed-storage graph is the one source
/// lowering builds for default restoration, so a module this compiler
/// published has it.
fn validate_routed_writes(module: &TerminalModule) -> Judged<()> {
    if !module
        .types
        .structural_domains
        .iter()
        .any(|domain| domain.establishment_routes.is_some())
    {
        return Ok(());
    }
    let defaults = module
        .machines
        .iter()
        .map(|machine| {
            DefaultRequirements::new(
                &module.types.structural_types,
                &module.types.structural_domains,
                machine,
            )
        })
        .collect::<Judged<Vec<_>>>()?;
    let roots = module
        .machines
        .iter()
        .zip(&defaults)
        .filter(|(_, required)| required.needs_borrowed_storage())
        .map(|(machine, _)| machine.id);
    let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(module, roots, &[])
        .map_err(|error| StructuralQualificationError {
            denied_establishment: None,
            machine: Some(error.machine()),
            detail: "a routed write has no complete borrowed-storage graph",
        })?;
    let mut references = crate::descriptor_results::DescriptorAnalysis::with_borrowed_storage_slots(
        &module.types.structural_types,
        &graph,
    );
    for (machine, required) in module.machines.iter().zip(&defaults) {
        for block in &machine.blocks {
            for (position, operation) in block.operations.iter().enumerate() {
                let position =
                    u32::try_from(position).map_err(|_| StructuralQualificationError {
                        denied_establishment: None,
                        machine: Some(machine.id),
                        detail: "a routed write position exceeds the descriptor graph",
                    })?;
                if required
                    .unauthorized_routed_write(
                        module,
                        operation,
                        (block.id, position),
                        &mut references,
                    )?
                    .is_some()
                {
                    return malformed(
                        Some(machine.id),
                        "a write keeps a routed domain on a value its machine may not establish",
                    );
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod default_requirement_tests;
