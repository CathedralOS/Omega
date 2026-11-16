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

use language_semantics::type_identity::{
    TypeIdentityPackageOwnerVisitor, TypeIdentityVisitError, visit_static_argument_package_owners,
    visit_type_identity_package_owners, vocabulary::OwnerForm,
};

use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, ContentPlaceVersion, IntegerType, MachineId, PlaceId,
    Proposition, ScalarTerm, ScalarType, StructuralCaseId, StructuralDomainId, StructuralFieldId,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    Operation, OperationKind, StructuralArgument, StructuralDomainDeclaration,
    StructuralEstablishmentRoute, StructuralPathQualification, StructuralPathSegment,
    StructuralQualificationChange, StructuralTypeDeclaration, StructuralTypeShape,
    TargetMachineCallable, TerminalMachine, TerminalModule, Terminator,
};

use crate::scalar_qualifications::{IssuerRoute, issuer_route_authorizes};
use crate::substitution::clauses::ClauseBindings;

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

/// The default clauses of actual declared storage. Additional entry facts and
/// the result placeholder are not storage declarations. Named memberships only
/// select what must be proved; they never supply predicate truth.
pub struct DefaultRequirements<'a> {
    types: &'a [StructuralTypeDeclaration],
    domains: &'a [StructuralDomainDeclaration],
    machine: &'a TerminalMachine,
    declared: Declared<'a>,
    type_defaults: Vec<bool>,
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
            type_defaults: memo
                .into_iter()
                .map(|value| value.expect("prepared type"))
                .collect(),
        })
    }

    fn error(&self, detail: &'static str) -> StructuralQualificationError {
        StructuralQualificationError {
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
        let memberships = self.declared.at(root, path);
        let mut named = Vec::new();
        for id in memberships.whole {
            let row = self
                .domains
                .binary_search_by_key(&id, |row| row.id)
                .ok()
                .map(|position| &self.domains[position])
                .ok_or_else(|| self.error("a default requirement names an absent domain"))?;
            if row.carrier != carrier {
                return Err(self.error("a named default requirement has a different carrier"));
            }
            let closure = row.required_domain_closure(self.domains).ok_or_else(|| {
                self.error("a named default has incomplete membership requirements")
            })?;
            for domain in closure {
                if !domain.complete_predicate_theory || domain.content_projection.is_some() {
                    return Err(
                        self.error("a named default has no complete storage predicate theory")
                    );
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
            row.2
                .projected
                .iter()
                .any(|qualified| qualified.path.starts_with(path))
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
                if self.type_has_defaults(*element)? {
                    // Nested type defaults genuinely apply to each element.
                    for index in 0..*length {
                        path.push(StructuralPathSegment::FixedIndex(index));
                        self.walk(root, path, *element, zero, active, result)?;
                        path.pop();
                    }
                } else if let Some(row) = self.declared.row(root) {
                    // No default can mention a plain element. Visit only the
                    // actual indexed qualification occurrences, never all bytes.
                    let mut selected = Vec::new();
                    for qualified in &row.2.projected {
                        if !qualified.path.starts_with(path) {
                            continue;
                        }
                        match qualified.path.get(path.len()) {
                            Some(StructuralPathSegment::FixedIndex(index)) if *index < *length => {
                                selected.push(*index)
                            }
                            None => {}
                            _ => {
                                return Err(self.error(
                                    "a default inventory has no concrete array qualification path",
                                ));
                            }
                        }
                    }
                    selected.sort_unstable();
                    selected.dedup();
                    for index in selected {
                        path.push(StructuralPathSegment::FixedIndex(index));
                        self.walk(root, path, *element, zero, active, result)?;
                        path.pop();
                    }
                }
            }
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

    /// The exact ordinal roster reconstructed independently by producers and
    /// receivers: innermost owners first on stores, type clauses before named
    /// domains at each owner, named domains sorted by canonical identity.
    pub fn operation(&self, operation: &Operation) -> Judged<Vec<DefaultRequirement>> {
        let mut result = Vec::new();
        let (root, mut path, strict) = match &operation.kind {
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
            _ => return Ok(result),
        };
        if strict && path.pop().is_none() {
            return Ok(result);
        }
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
        let self_place = PlaceId::new(1).expect("self has a nonzero coordinate");
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
                let mut clauses = Vec::new();
                if let Some(predicate) = &row.predicate {
                    let mut bindings = DefaultBindings(Rerooted {
                        subject: predicate.subject,
                        root: self_place,
                        referent: false,
                    });
                    clauses.push(
                        crate::substitution::clauses::substitute_clause(
                            &predicate.proposition,
                            &mut bindings,
                        )
                        .map_err(|detail| self.error(detail))?,
                    );
                }
                if let Some(predicate) = row.byte_predicate {
                    if !crate::byte_predicate_carrier(self.types.iter(), required.carrier) {
                        return Err(self.error("a named byte default has a nonbyte carrier"));
                    }
                    clauses.push(Proposition::Equal(
                        ScalarTerm::byte_sequence_predicate(predicate, self_place, Vec::new()),
                        ScalarTerm::Boolean(true),
                    ));
                }
                Ok(match clauses.len() {
                    0 => return Err(self.error("a named default has no predicate clause")),
                    1 => clauses.pop().expect("one predicate"),
                    _ => Proposition::Conjunction(clauses),
                })
            }
        }
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
        if declared.byte_predicate.is_some() || declared.content_projection.is_some() {
            return malformed(
                machine,
                "a structural cast cannot prove a byte predicate or content projection",
            );
        }
        // `value as +D` proves the predicates of `D` of the operand on the
        // edge into the block the cast continues in: that block's invariant
        // states them of its parameter, as a scalar coercion's does
        // (`scalar_qualifications`).
        if let Some(predicate) = &declared.predicate {
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
    Ok(())
}

fn validate_machine(module: &TerminalModule, machine: &TerminalMachine) -> Judged<()> {
    let places = Declared::new(machine)?;
    let id = Some(machine.id);
    validate_declared(module, &places)?;
    let changes: Vec<_> = module
        .structural_qualification_changes
        .iter()
        .filter(|change| change.machine == machine.id)
        .collect();
    let mut used = vec![false; changes.len()];
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
                        validate_change(
                            module,
                            &places,
                            changes[index],
                            argument,
                            &expected,
                            target.id,
                        )?;
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
                    if !produced.whole.is_empty()
                        || produced
                            .projected
                            .iter()
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
    Ok(())
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
    description: &language_semantics::reflection::CallableDescription,
    declaration: &terminal_psi::NominalDeclarationIdentity,
    callable: terminal_psi::TerminalIdentityIndex,
) -> Judged<MachineCallerDescription> {
    let error = |detail| StructuralQualificationError {
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
    let revision = source::source_identity::toolchain_source_revision_digest(&description.1[2]);
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
    description: &language_semantics::reflection::CallableDescription,
    described: &MachineCallerDescription,
) -> Judged<bool> {
    let error = |detail| StructuralQualificationError {
        machine: Some(current.id),
        detail,
    };
    let revision = source::source_identity::toolchain_source_revision_digest(&description.1[2]);
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
                        if expected.scalar_index_identity().as_deref() != Some(argument.as_str()) {
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
        language_semantics::reflection::CallableDescription,
        StructuralEstablishmentRoute,
    ),
) -> Judged<bool> {
    let current = module
        .machine(current)
        .ok_or(StructuralQualificationError {
            machine: Some(current),
            detail: "policy caller is absent from the actual module",
        })?;
    let declaration = current
        .declaration
        .as_ref()
        .ok_or(StructuralQualificationError {
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
        machine: None,
        detail,
    };
    let projected = plan.clone().try_bind(
        |(node, _)| Ok::<_, StructuralQualificationError>(node),
        |(description, _)| Ok(description),
    )?;
    let normalized = access_plans::normalize_access_plan(projected.clone()).map_err(|_| {
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
            language_semantics::reflection::SchemaNode::Case(case) => Some(case),
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
            language_semantics::reflection::SchemaNode::Case(case)
                if description.owner_case.is_valid() =>
            {
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
        language_semantics::reflection::SchemaNodeHandle,
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
        access_plans::AccessPlan<
            language_semantics::reflection::SchemaNodeHandle,
            (
                language_semantics::reflection::CallableDescription,
                (u8, String, String),
            ),
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

/// The whole judgment of a module's structural memberships.
pub fn validate(module: &TerminalModule) -> Judged<()> {
    for domain in &module.structural_domains {
        validate_access_policy(module, domain)?;
        // A received program can already hold a qualified input. Source
        // formation refusal alone cannot protect its operations until memory
        // access checks implement read, write, observation and reach rules.
        // Description-only plans can still be validated separately.
        if domain.access_policy.as_ref().is_some_and(|plan| {
            !plan.readers.is_empty()
                || !plan.writers.is_empty()
                || !plan.observations.is_empty()
                || plan.reach.services().len() != 0
        }) {
            return malformed(
                None,
                "memory access checks for these saved rules are not implemented yet",
            );
        }
    }
    for machine in &module.machines {
        validate_machine(module, machine)?;
    }
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

#[cfg(test)]
mod default_requirement_tests {
    use super::*;
    use semantic_vocabulary::{
        ByteSequencePredicate, ContractId, DomainSemanticId, IntegerSign, OperationId,
        StructuralPlaceKind,
    };
    use terminal_psi::{
        ArrayExtent, MachineContract, OperationResult, RegisterTargetApplicability,
        StructuralAccess, StructuralDomainPredicate, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralPathQualification, StructuralPlaceDeclaration,
        TerminalIdentityIndex,
    };

    fn ty(id: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            declaration_identity: TerminalIdentityIndex::ABSENT,
            quotient_carrier: None,
            id: StructuralTypeId::new(id).unwrap(),
            identity: TerminalIdentityIndex::ABSENT,
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            shape,
            scalar_restriction: None,
            requires: Vec::new(),
        }
    }

    fn types(length: u64) -> Vec<StructuralTypeDeclaration> {
        vec![
            ty(
                1,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                )),
            ),
            ty(
                2,
                StructuralTypeShape::FixedArray {
                    element: StructuralTypeId::new(1).unwrap(),
                    length: ArrayExtent::Literal(length),
                },
            ),
        ]
    }

    fn machine() -> TerminalMachine {
        let root = PlaceId::new(2).unwrap();
        let mut contract = MachineContract::empty(ContractId::new(1).unwrap());
        contract
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place: root,
                position: 0,
                structural_type: StructuralTypeId::new(2).unwrap(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
            });
        TerminalMachine {
            assembly_target_requirement: RegisterTargetApplicability::Any,
            id: MachineId::new(1).unwrap(),
            attachment: None,
            ranked_scc: None,
            structural_places: vec![StructuralPlaceDeclaration {
                id: root,
                kind: StructuralPlaceKind::Parameter { position: 0 },
            }],
            call_formations: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks: Vec::new(),
            contract,
        }
    }

    fn domain(carrier: StructuralTypeId) -> StructuralDomainDeclaration {
        let subject = PlaceId::new(9).unwrap();
        StructuralDomainDeclaration {
            required_memberships: Some(Vec::new()),
            complete_predicate_theory: true,
            predicate: Some(StructuralDomainPredicate {
                subject,
                proposition: Proposition::Equal(
                    ScalarTerm::byte_sequence_predicate(
                        ByteSequencePredicate::ValidUtf8,
                        subject,
                        Vec::new(),
                    ),
                    ScalarTerm::Boolean(true),
                ),
            }),
            id: StructuralDomainId::new(1).unwrap(),
            semantic_domain: DomainSemanticId::new(1).unwrap(),
            identity: TerminalIdentityIndex(1),
            carrier,
            subject_identity: TerminalIdentityIndex::ABSENT,
            content_projection: None,
            byte_predicate: None,
            establishment_routes: None,
            access_policy: None,
        }
    }

    fn store(path: Vec<StructuralPathSegment>) -> Operation {
        Operation::unsuspended(
            OperationId::new(1).unwrap(),
            OperationResult::Unit,
            OperationKind::WriteOnlyPrimitiveStore {
                destination: PlaceId::new(2).unwrap(),
                path,
                value: ValueId::new(1).unwrap(),
                range_obligation: None,
            },
        )
    }

    #[test]
    fn named_defaults_are_rerooted_goals_not_membership_truth() {
        let mut types = types(2);
        types[1].requires.push(Proposition::Truth);
        let mut machine = machine();
        machine.structural_parameters[0]
            .qualifications
            .push(StructuralDomainId::new(1).unwrap());
        let domains = vec![domain(types[1].id)];
        let defaults = DefaultRequirements::new(&types, &domains, &machine).unwrap();
        let rows = defaults
            .operation(&store(vec![StructuralPathSegment::FixedIndex(0)]))
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(defaults.clause(&rows[0]).unwrap(), Proposition::Truth);
        assert_eq!(
            defaults.clause(&rows[1]).unwrap(),
            Proposition::Equal(
                ScalarTerm::byte_sequence_predicate(
                    ByteSequencePredicate::ValidUtf8,
                    PlaceId::new(1).unwrap(),
                    Vec::new()
                ),
                ScalarTerm::Boolean(true),
            )
        );
        assert_eq!(defaults.declared().unwrap(), rows);
    }

    #[test]
    fn additional_input_membership_is_not_a_declared_default() {
        let types = types(2);
        let mut machine = machine();
        machine.structural_parameters[0]
            .additional_input_qualifications
            .push(StructuralPathQualification {
                path: Vec::new(),
                domain: StructuralDomainId::new(1).unwrap(),
            });
        let result_place = PlaceId::new(3).unwrap();
        machine.result = terminal_psi::TerminalMachineResult::Structural(
            terminal_psi::StructuralResultDeclaration {
                place: result_place,
                structural_type: types[1].id,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: vec![StructuralDomainId::new(1).unwrap()],
                projected_qualifications: Vec::new(),
                reference_sources: Vec::new(),
            },
        );
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: result_place,
            kind: StructuralPlaceKind::Result,
        });
        let domains = vec![domain(types[1].id)];
        let defaults = DefaultRequirements::new(&types, &domains, &machine).unwrap();
        assert!(defaults.declared().unwrap().is_empty());
        assert!(
            defaults
                .operation(&store(vec![StructuralPathSegment::FixedIndex(0)]))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn plain_arrays_do_not_enumerate_elements_for_default_inventory() {
        let types = types(u64::MAX);
        let machine = machine();
        let defaults = DefaultRequirements::new(&types, &[], &machine).unwrap();
        assert!(defaults.declared().unwrap().is_empty());
        assert!(
            defaults
                .operation(&store(vec![StructuralPathSegment::FixedIndex(0)]))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn reference_display_does_not_become_a_lender_default() {
        let mut types = types(2);
        types.push(ty(
            3,
            StructuralTypeShape::Reference {
                referent: types[1].id,
                access: StructuralAccess::MutableBorrow,
            },
        ));
        let mut machine = machine();
        machine.structural_parameters[0].structural_type = types[2].id;
        machine.structural_parameters[0]
            .projected_qualifications
            .push(StructuralPathQualification {
                path: vec![StructuralPathSegment::Referent],
                domain: StructuralDomainId::new(1).unwrap(),
            });
        let domains = vec![domain(types[1].id)];
        let defaults = DefaultRequirements::new(&types, &domains, &machine).unwrap();
        let refused = defaults
            .operation(&store(vec![
                StructuralPathSegment::Referent,
                StructuralPathSegment::FixedIndex(0),
            ]))
            .unwrap_err();
        assert_eq!(
            refused.detail,
            "a reference display is not a lender's declared default"
        );
    }
}
