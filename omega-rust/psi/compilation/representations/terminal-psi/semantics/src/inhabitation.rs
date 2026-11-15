//! Conservative emptiness of declared value domains, shared by source and Terminal types.
//!
//! A false requirement is positive evidence of emptiness. Otherwise an owned field
//! can exclude a record value, and every case must be excluded to exclude a sum.
//! Unknown contracts and cycles alone prove nothing. The reverse dependency graph
//! propagates each exclusion once, including through cycles with a real false seed.

use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType, StructuralTypeId, ValueId};
use terminal_psi::{
    StructuralAccess, StructuralFieldDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

/// A borrowed view of one completed type family. Rows have strictly increasing
/// actual IDs; appending rows must not change an already prepared contract.
///
/// Field visits describe required *owned values*, not referenced storage. `None`
/// denotes a common field; `Some(case)` denotes that case's payload field. A
/// reference/view emits no referent edge; a zero-length array emits no element
/// edge. Implementations borrow their existing contracts, never clone shapes.
pub trait TypeContracts {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn type_id(&self, index: usize) -> StructuralTypeId;
    fn requires(&self, index: usize) -> &[Proposition];
    /// `None` for ordinary carriers/records, `Some(n)` for a sum or mixed type.
    /// Zero cases supplies no exclusion evidence; a missing/unfinished shape
    /// must never become a vacuous proof of emptiness.
    fn cases(&self, index: usize) -> Option<usize>;
    fn visit_owned_fields(&self, index: usize, visit: impl FnMut(Option<usize>, StructuralTypeId));
}

/// The actual stored value observed by an empty-domain terminal. A final
/// `Case` selects a payload namespace, so availability and read footprints
/// observe its containing sum; all earlier projections remain exact. The full
/// semantic path still names the selected namespace for emptiness and case
/// evidence. This projection alone grants neither case selection nor emptiness.
pub fn observed_storage_path(path: &[StructuralPathSegment]) -> &[StructuralPathSegment] {
    match path.split_last() {
        Some((StructuralPathSegment::Case(_), carrier)) => carrier,
        _ => path,
    }
}

/// Whether retained clauses exclude every value using closed Boolean and
/// integer computations. An unavailable atom, observation or actual is unknown.
/// This is also usable for a named domain's already-instantiated predicates.
pub fn predicates_are_false(predicates: &[Proposition]) -> bool {
    predicates_are_false_with(predicates, |_| None)
}

/// Bind the visible contract's exact formal IDs to known typed literals. The
/// resolver must expose only actual parameters, including real erased scalar
/// formals; result and foreign IDs remain unknown despite numeric collisions.
/// No body, storage observation or execution authority enters this query.
pub fn predicates_are_false_with(
    predicates: &[Proposition],
    mut scalar_value: impl FnMut(ValueId) -> Option<(ScalarType, semantic_vocabulary::IntegerValue)>,
) -> bool {
    let mut bindings = KnownLiterals(&mut scalar_value);
    let mut integers = proof_admission::ClosedIntegerEvaluator::default();
    predicates.iter().any(|predicate| {
        let truth = proof_admission::closed_guard_with(predicate, |leaf| {
            // The general crash-guard evaluator also decides IEEE relations.
            // This query admits only the Boolean/integer part of that owner.
            let allowed = matches!(
                leaf,
                Proposition::Truth
                    | Proposition::Falsehood
                    | Proposition::Equal(..)
                    | Proposition::LessThan(..)
                    | Proposition::LessOrEqual(..)
                    | Proposition::IntegerMathEqual(..)
                    | Proposition::IntegerMathLessThan(..)
                    | Proposition::IntegerMathLessOrEqual(..)
            );
            let value = if allowed && leaf.validate().is_ok() {
                // Substitute each comparison once. An unresolved sibling must
                // not hide another child's independently false condition.
                crate::substitution::clauses::substitute_clause(leaf, &mut bindings)
                    .ok()
                    .filter(|closed| closed.validate().is_ok())
                    .and_then(|closed| {
                        proof_admission::closed_guard(&closed, &mut integers)
                            .ok()
                            .flatten()
                    })
            } else {
                None
            };
            Ok::<_, std::convert::Infallible>(value)
        });
        matches!(truth, Ok(Some(false)))
    })
}

/// Resolve a caller-owned erased actual before binding it to a genuine formal.
/// Known values keep their declared carrier; unavailable or malformed terms,
/// unresolved widths and guarded arithmetic failures supply no constant.
pub fn known_scalar_value_with(
    term: &ScalarTerm,
    mut scalar_value: impl FnMut(ValueId) -> Option<(ScalarType, semantic_vocabulary::IntegerValue)>,
) -> Option<(ScalarType, semantic_vocabulary::IntegerValue)> {
    use semantic_vocabulary::IntegerValue;
    term.validate().ok()?;
    let closed = crate::substitution::clauses::substitute_clause_term(
        term,
        &mut KnownLiterals(&mut scalar_value),
    )
    .ok()?;
    closed.validate().ok()?;
    match closed.scalar_type() {
        ScalarType::Boolean => Some((
            ScalarType::Boolean,
            IntegerValue::Unsigned(u128::from(u8::from(closed.boolean_value()?))),
        )),
        ScalarType::Integer(integer) if integer.bits().is_some() => {
            let (actual, value) = closed.integer_value()?;
            (actual == integer && integer.admits(value))
                .then_some((ScalarType::Integer(integer), value))
        }
        _ => None,
    }
}

struct KnownLiterals<'a, F>(&'a mut F);

impl<F> crate::substitution::clauses::ClauseBindings for KnownLiterals<'_, F>
where
    F: FnMut(ValueId) -> Option<(ScalarType, semantic_vocabulary::IntegerValue)>,
{
    type Error = &'static str;

    fn value(&mut self, id: ValueId, expected: ScalarType) -> Result<ScalarTerm, Self::Error> {
        use semantic_vocabulary::IntegerValue;
        let (actual, value) = (self.0)(id).ok_or("scalar value is unavailable")?;
        if actual != expected {
            return Err("scalar actual has another carrier");
        }
        match (actual, value) {
            (ScalarType::Boolean, IntegerValue::Unsigned(value @ 0..=1)) => {
                Ok(ScalarTerm::Boolean(value != 0))
            }
            (ScalarType::Integer(integer), value)
                if integer.bits().is_some() && integer.admits(value) =>
            {
                Ok(ScalarTerm::Integer {
                    scalar_type: integer,
                    value,
                })
            }
            _ => Err("scalar actual has no valid closed literal"),
        }
    }

    fn field(
        &mut self,
        _version: semantic_vocabulary::ContentPlaceVersion,
        _root: semantic_vocabulary::PlaceId,
        _path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        _scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        Err("scalar predicate has no storage observation")
    }

    fn case(
        &mut self,
        _root: semantic_vocabulary::PlaceId,
        _path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        _case: semantic_vocabulary::StructuralCaseId,
    ) -> Result<bool, Self::Error> {
        Err("scalar predicate has no case observation")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    ProvedEmpty,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    ChangedFamily,
    UnorderedIdentity(StructuralTypeId),
    MissingType(StructuralTypeId),
    InvalidCase {
        owner: StructuralTypeId,
        case: usize,
    },
}

#[derive(Clone, Copy, Debug)]
enum Dependent {
    Type(usize),
    Case(usize),
}

#[derive(Debug)]
struct Link {
    dependent: Dependent,
    next: Option<usize>,
}

#[derive(Debug)]
struct Case {
    owner: usize,
    empty: bool,
}

#[derive(Debug)]
struct Type {
    empty: bool,
    cases_left: Option<usize>,
    first_case: usize,
    first_link: Option<usize>,
}

/// Prepared stage-local scratch, not a proof receipt or an inter-stage product.
/// Only positive exclusions are cached; an unknown cyclic node can become empty
/// when another node supplies a false requirement. Queries do no catalog scans.
#[derive(Debug, Default)]
pub struct Inhabitation {
    ids: Vec<StructuralTypeId>,
    types: Vec<Type>,
    cases: Vec<Case>,
    links: Vec<Link>,
}

impl Inhabitation {
    pub fn prepare(contracts: &impl TypeContracts) -> Result<Self, Error> {
        let mut result = Self::default();
        result.extend(contracts)?;
        Ok(result)
    }

    /// Append completed contracts. The stage owner guarantees the old prefix is
    /// immutable. Only new rows and their fields are visited, so constructing a
    /// source family incrementally does not repeatedly traverse the whole arena.
    /// A failed extension leaves this cache unchanged.
    pub fn extend(&mut self, contracts: &impl TypeContracts) -> Result<(), Error> {
        let start = self.ids.len();
        if contracts.len() < start {
            return Err(Error::ChangedFamily);
        }
        let mut new_ids = Vec::with_capacity(contracts.len() - start);
        let mut previous = self.ids.last().copied();
        for index in start..contracts.len() {
            let id = contracts.type_id(index);
            if previous.is_some_and(|previous| previous >= id) {
                return Err(Error::UnorderedIdentity(id));
            }
            previous = Some(id);
            new_ids.push(id);
        }
        let index_of = |id| {
            self.ids
                .binary_search(&id)
                .ok()
                .or_else(|| new_ids.binary_search(&id).ok().map(|index| start + index))
        };
        // Validate links before changing scratch, including identities that do
        // not belong to this family. Never size storage from a serialized ID.
        let mut fields = Vec::new();
        for index in start..contracts.len() {
            let owner = contracts.type_id(index);
            let cases = contracts.cases(index);
            let mut error = None;
            contracts.visit_owned_fields(index, |case, child| {
                if let Some(case) = case
                    && !cases.is_some_and(|count| case < count)
                {
                    error = Some(Error::InvalidCase { owner, case });
                    return;
                }
                match index_of(child) {
                    Some(child) => fields.push((index, case, child)),
                    None => error = Some(Error::MissingType(child)),
                }
            });
            if let Some(error) = error {
                return Err(error);
            }
        }
        self.ids.extend(new_ids);
        let mut queue = Vec::new();
        for index in start..contracts.len() {
            let count = contracts.cases(index);
            let first_case = self.cases.len();
            if let Some(count) = count {
                self.cases.extend((0..count).map(|_| Case {
                    owner: index,
                    empty: false,
                }));
            }
            self.types.push(Type {
                empty: false,
                cases_left: count,
                first_case,
                first_link: None,
            });
            if predicates_are_false(contracts.requires(index)) {
                self.exclude(index, &mut queue);
            }
        }
        for (owner, case, child) in fields {
            let dependent = match case {
                Some(case) => Dependent::Case(self.types[owner].first_case + case),
                None => Dependent::Type(owner),
            };
            if self.types[child].empty {
                self.notify(dependent, &mut queue);
            } else {
                let next = self.types[child].first_link;
                self.types[child].first_link = Some(self.links.len());
                self.links.push(Link { dependent, next });
            }
        }
        let mut cursor = 0;
        while cursor < queue.len() {
            let mut link = self.types[queue[cursor]].first_link;
            cursor += 1;
            while let Some(index) = link {
                let dependent = self.links[index].dependent;
                link = self.links[index].next;
                self.notify(dependent, &mut queue);
            }
        }
        Ok(())
    }

    /// A foreign/missing ID has no emptiness proof in this family.
    pub fn status(&self, id: StructuralTypeId) -> Status {
        if self
            .ids
            .binary_search(&id)
            .ok()
            .is_some_and(|index| self.types[index].empty)
        {
            Status::ProvedEmpty
        } else {
            Status::Unknown
        }
    }

    /// A case position belongs to one prepared type, not to a global case
    /// namespace. Validate it before using even an empty owner's contract.
    pub fn case_status(&self, owner: StructuralTypeId, case: usize) -> Status {
        let Some(index) = self.ids.binary_search(&owner).ok() else {
            return Status::Unknown;
        };
        let row = &self.types[index];
        let end = self
            .types
            .get(index + 1)
            .map_or(self.cases.len(), |next| next.first_case);
        if row.cases_left.is_none() || case >= end - row.first_case {
            return Status::Unknown;
        }
        if row.empty || self.cases[row.first_case + case].empty {
            Status::ProvedEmpty
        } else {
            Status::Unknown
        }
    }

    /// Positive declared-domain emptiness at one exact observation path.
    /// Source availability, active-case evidence and loans are separate
    /// judgments. A final case denotes its payload namespace, not a value
    /// fabricated in a new home. No unknown or malformed path proves empty.
    ///
    /// The borrowed catalog is prepared once with this query. Each step uses
    /// its binary declaration lookup; no per-observation arena scan occurs.
    /// Static fields, cases, fixed indices and readable referents are served;
    /// ranges, representation views and dynamic selectors remain unknown.
    pub fn declared_path_status(
        &self,
        contracts: &TerminalContracts<'_>,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Status {
        let Some(mut declaration) = contracts.declaration(root) else {
            return Status::Unknown;
        };
        let mut payload = None;
        for segment in path {
            match segment {
                StructuralPathSegment::Case(case_id) if payload.is_none() => {
                    let (StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. }) = &declaration.shape
                    else {
                        return Status::Unknown;
                    };
                    let mut matching = cases
                        .iter()
                        .enumerate()
                        .filter(|(_, case)| case.id == *case_id);
                    let Some((position, case)) = matching.next() else {
                        return Status::Unknown;
                    };
                    if matching.next().is_some() {
                        return Status::Unknown;
                    }
                    payload = Some((position, case));
                }
                StructuralPathSegment::Field(identity) => {
                    if identity.is_absent() {
                        return Status::Unknown;
                    }
                    let fields = match payload.take() {
                        Some((_, case)) => &case.fields,
                        None => match &declaration.shape {
                            StructuralTypeShape::Record { fields, .. }
                            | StructuralTypeShape::Mixed { fields, .. } => fields,
                            _ => return Status::Unknown,
                        },
                    };
                    let mut matching = fields.iter().filter(|field| field.identity == *identity);
                    let Some(field) = matching.next() else {
                        return Status::Unknown;
                    };
                    if matching.next().is_some() || field.relevance.is_erased() {
                        return Status::Unknown;
                    }
                    let Some(child) = field
                        .field_type
                        .as_structural()
                        .and_then(|id| contracts.declaration(id))
                    else {
                        return Status::Unknown;
                    };
                    declaration = child;
                }
                StructuralPathSegment::FixedIndex(index) if payload.is_none() => {
                    let StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } = declaration.shape
                    else {
                        return Status::Unknown;
                    };
                    if *index >= length {
                        return Status::Unknown;
                    }
                    let Some(child) = contracts.declaration(element) else {
                        return Status::Unknown;
                    };
                    declaration = child;
                }
                StructuralPathSegment::Referent if payload.is_none() => {
                    let StructuralTypeShape::Reference {
                        referent, access, ..
                    } = declaration.shape
                    else {
                        return Status::Unknown;
                    };
                    if access == StructuralAccess::WriteOnlyBorrow {
                        return Status::Unknown;
                    }
                    let Some(child) = contracts.declaration(referent) else {
                        return Status::Unknown;
                    };
                    declaration = child;
                }
                _ => return Status::Unknown,
            }
        }
        match payload {
            Some((position, _)) => self.case_status(declaration.id, position),
            None => self.status(declaration.id),
        }
    }

    fn exclude(&mut self, index: usize, queue: &mut Vec<usize>) {
        if !self.types[index].empty {
            self.types[index].empty = true;
            queue.push(index);
        }
    }

    fn notify(&mut self, dependent: Dependent, queue: &mut Vec<usize>) {
        match dependent {
            Dependent::Type(owner) => self.exclude(owner, queue),
            Dependent::Case(index) if !self.cases[index].empty => {
                self.cases[index].empty = true;
                let owner = self.cases[index].owner;
                if let Some(left) = &mut self.types[owner].cases_left {
                    *left -= 1;
                    if *left == 0 {
                        self.exclude(owner, queue);
                    }
                }
            }
            Dependent::Case(_) => {}
        }
    }
}

/// An external completion produces no return value. Erasing its physical result
/// still requires proof that the exact declared result has no possible value;
/// an empty storage layout alone does not establish that fact.
pub fn external_result_is_erased(
    completion: semantic_vocabulary::BoundaryCompletion,
    result: Option<StructuralTypeId>,
    declarations: &[StructuralTypeDeclaration],
) -> Result<bool, String> {
    if !matches!(
        completion,
        semantic_vocabulary::BoundaryCompletion::External(_)
    ) {
        return Ok(false);
    }
    let result = result.ok_or("external completion lost its logical result type")?;
    let query = Inhabitation::prepare(&TerminalContracts::new(declarations.iter()))
        .map_err(|error| format!("external result type contracts: {error:?}"))?;
    if query.status(result) != Status::ProvedEmpty {
        return Err("external completion result is not proved uninhabited".into());
    }
    Ok(true)
}

/// A sorted borrowed catalog; declaration order and shapes stay untouched.
pub struct TerminalContracts<'a> {
    declarations: Vec<&'a StructuralTypeDeclaration>,
}

impl<'a> TerminalContracts<'a> {
    pub fn new(declarations: impl IntoIterator<Item = &'a StructuralTypeDeclaration>) -> Self {
        let mut declarations: Vec<_> = declarations.into_iter().collect();
        declarations.sort_unstable_by_key(|declaration| declaration.id);
        Self { declarations }
    }

    fn declaration(&self, id: StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> {
        let index = self
            .declarations
            .binary_search_by_key(&id, |row| row.id)
            .ok()?;
        // A malformed duplicate catalog cannot choose one contract as authority.
        if index > 0 && self.declarations[index - 1].id == id
            || self
                .declarations
                .get(index + 1)
                .is_some_and(|next| next.id == id)
        {
            return None;
        }
        Some(self.declarations[index])
    }
}

impl TypeContracts for TerminalContracts<'_> {
    fn len(&self) -> usize {
        self.declarations.len()
    }
    fn type_id(&self, index: usize) -> StructuralTypeId {
        self.declarations[index].id
    }
    fn requires(&self, index: usize) -> &[Proposition] {
        &self.declarations[index].requires
    }
    fn cases(&self, index: usize) -> Option<usize> {
        match &self.declarations[index].shape {
            StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                Some(cases.len())
            }
            _ => None,
        }
    }
    fn visit_owned_fields(
        &self,
        index: usize,
        mut visit: impl FnMut(Option<usize>, StructuralTypeId),
    ) {
        let mut fields = |case, fields: &[StructuralFieldDeclaration]| {
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                if let Some(child) = field.field_type.as_structural() {
                    visit(case, child);
                }
            }
        };
        match &self.declarations[index].shape {
            StructuralTypeShape::Record { fields: common, .. } => fields(None, common),
            StructuralTypeShape::Sum { cases } => {
                for (index, case) in cases.iter().enumerate() {
                    fields(Some(index), &case.fields);
                }
            }
            StructuralTypeShape::Mixed {
                fields: common,
                cases,
            } => {
                fields(None, common);
                for (index, case) in cases.iter().enumerate() {
                    fields(Some(index), &case.fields);
                }
            }
            StructuralTypeShape::FixedArray { element, length }
                if length.literal().is_some_and(|length| length > 0) =>
            {
                visit(None, *element)
            }
            StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::ElementView { .. }
            | StructuralTypeShape::FixedArray { .. }
            | StructuralTypeShape::Opaque
            | StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::AtomicCell(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        PropositionId, ScalarApplicationId, StructuralCaseId, StructuralFieldId,
    };
    use terminal_psi::{
        ArrayExtent, StructuralAccess, StructuralCaseDeclaration, StructuralFieldType,
        StructuralRecordStorage, TerminalIdentityIndex,
    };

    fn id(raw: u64) -> StructuralTypeId {
        StructuralTypeId::new(raw).unwrap()
    }

    fn field(raw: u64) -> StructuralFieldDeclaration {
        StructuralFieldDeclaration {
            id: StructuralFieldId::new(raw).unwrap(),
            identity: TerminalIdentityIndex::ABSENT,
            relevance: Default::default(),
            field_type: StructuralFieldType::Structural(id(raw)),
            inline_carrier: None,
            storage: None,
        }
    }

    fn record(children: &[u64]) -> StructuralTypeShape {
        StructuralTypeShape::Record {
            fields: children.iter().map(|child| field(*child)).collect(),
            storage: StructuralRecordStorage::Packed,
        }
    }

    fn sum(cases: &[&[u64]]) -> StructuralTypeShape {
        StructuralTypeShape::Sum {
            cases: cases
                .iter()
                .enumerate()
                .map(|(index, children)| StructuralCaseDeclaration {
                    id: StructuralCaseId::new(index as u64 + 1).unwrap(),
                    identity: TerminalIdentityIndex::ABSENT,
                    fields: children.iter().map(|child| field(*child)).collect(),
                })
                .collect(),
        }
    }

    fn declaration(raw: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            id: id(raw),
            identity: TerminalIdentityIndex::ABSENT,
            quotient_carrier: None,
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            shape,
            scalar_restriction: None,
            requires: Vec::new(),
        }
    }

    fn empty(raw: u64) -> StructuralTypeDeclaration {
        let mut declaration = declaration(raw, record(&[]));
        declaration.requires.push(Proposition::Falsehood);
        declaration
    }

    fn prepare(declarations: &[StructuralTypeDeclaration]) -> Inhabitation {
        Inhabitation::prepare(&TerminalContracts::new(declarations)).unwrap()
    }

    #[test]
    fn external_results_erase_only_the_exact_proved_empty_type() {
        let completion = semantic_vocabulary::BoundaryCompletion::External(
            semantic_vocabulary::ExternalCompletionKind::ProcessExit,
        );
        let declarations = [empty(1), declaration(2, record(&[]))];
        assert_eq!(
            external_result_is_erased(completion, Some(id(1)), &declarations),
            Ok(true)
        );
        assert!(
            external_result_is_erased(completion, Some(id(2)), &declarations)
                .unwrap_err()
                .contains("not proved uninhabited")
        );
        assert!(
            external_result_is_erased(completion, None, &declarations)
                .unwrap_err()
                .contains("logical result")
        );
        assert_eq!(
            external_result_is_erased(
                semantic_vocabulary::BoundaryCompletion::NormalReturn,
                Some(id(1)),
                &declarations
            ),
            Ok(false)
        );
    }

    #[test]
    fn case_emptiness_is_local_to_the_exact_declared_roster() {
        let mut sum_type = declaration(2, sum(&[&[], &[1]]));
        let declarations = [empty(1), sum_type.clone()];
        let query = prepare(&declarations);
        let contracts = TerminalContracts::new(&declarations);
        assert_eq!(query.case_status(id(2), 0), Status::Unknown);
        assert_eq!(query.case_status(id(2), 1), Status::ProvedEmpty);
        assert_eq!(query.status(id(2)), Status::Unknown);
        assert_eq!(
            query.declared_path_status(
                &contracts,
                id(2),
                &[StructuralPathSegment::Case(
                    StructuralCaseId::new(2).unwrap()
                )]
            ),
            Status::ProvedEmpty
        );
        assert_eq!(
            query.declared_path_status(
                &contracts,
                id(2),
                &[StructuralPathSegment::Case(
                    StructuralCaseId::new(3).unwrap()
                )]
            ),
            Status::Unknown
        );
        sum_type.requires.push(Proposition::Falsehood);
        let declarations = [empty(1), sum_type];
        let query = prepare(&declarations);
        assert_eq!(query.case_status(id(2), 2), Status::Unknown);
        assert_eq!(query.case_status(id(1), 0), Status::Unknown);
        assert_eq!(query.case_status(id(99), 0), Status::Unknown);
    }

    #[test]
    fn observing_an_empty_referent_needs_an_explicit_readable_crossing() {
        let declarations = [
            empty(1),
            declaration(
                2,
                StructuralTypeShape::Reference {
                    referent: id(1),
                    access: StructuralAccess::SharedBorrow,
                },
            ),
            declaration(
                3,
                StructuralTypeShape::Reference {
                    referent: id(1),
                    access: StructuralAccess::WriteOnlyBorrow,
                },
            ),
        ];
        let query = prepare(&declarations);
        let contracts = TerminalContracts::new(&declarations);
        assert_eq!(
            query.declared_path_status(&contracts, id(2), &[]),
            Status::Unknown
        );
        assert_eq!(
            query.declared_path_status(&contracts, id(2), &[StructuralPathSegment::Referent]),
            Status::ProvedEmpty
        );
        assert_eq!(
            query.declared_path_status(&contracts, id(3), &[StructuralPathSegment::Referent]),
            Status::Unknown
        );
        assert_eq!(
            query.declared_path_status(&contracts, id(1), &[StructuralPathSegment::Referent]),
            Status::Unknown
        );
    }

    #[test]
    fn empty_owner_never_authorizes_a_malformed_or_vacant_path() {
        let declarations = [
            empty(1),
            declaration(
                2,
                StructuralTypeShape::FixedArray {
                    element: id(1),
                    length: ArrayExtent::Literal(0),
                },
            ),
        ];
        let query = prepare(&declarations);
        let contracts = TerminalContracts::new(&declarations);
        assert_eq!(
            query.declared_path_status(
                &contracts,
                id(1),
                &[StructuralPathSegment::Field(TerminalIdentityIndex(9))]
            ),
            Status::Unknown
        );
        assert_eq!(
            query.declared_path_status(&contracts, id(2), &[StructuralPathSegment::FixedIndex(0)]),
            Status::Unknown
        );
        assert_eq!(
            query.declared_path_status(&contracts, id(99), &[]),
            Status::Unknown
        );
    }

    #[test]
    fn declared_false_is_evidence_but_empty_storage_or_missing_shape_is_not() {
        let declarations = [
            declaration(9, sum(&[])),
            empty(4),
            declaration(1, record(&[])),
        ];
        let query = prepare(&declarations);
        assert_eq!(query.status(id(4)), Status::ProvedEmpty);
        assert_eq!(query.status(id(1)), Status::Unknown);
        assert_eq!(query.status(id(9)), Status::Unknown);
        assert_eq!(query.status(id(u64::MAX)), Status::Unknown);
    }

    #[test]
    fn every_sum_case_must_have_an_empty_owned_field() {
        let mut mixed = declaration(7, sum(&[&[], &[]]));
        let StructuralTypeShape::Sum { cases } = mixed.shape else {
            unreachable!()
        };
        mixed.shape = StructuralTypeShape::Mixed {
            fields: vec![field(1)],
            cases,
        };
        let declarations = [
            empty(1),
            declaration(2, record(&[])),
            declaration(3, record(&[2, 1])),
            declaration(4, sum(&[&[3], &[1, 2]])),
            declaration(5, sum(&[&[1], &[2]])),
            declaration(6, sum(&[&[1], &[]])),
            mixed,
        ];
        let query = prepare(&declarations);
        for raw in [1, 3, 4, 7] {
            assert_eq!(query.status(id(raw)), Status::ProvedEmpty);
        }
        for raw in [2, 5, 6] {
            assert_eq!(query.status(id(raw)), Status::Unknown);
        }
    }

    #[test]
    fn cycles_need_a_real_seed_and_propagate_it_in_either_order() {
        let declarations = [
            declaration(1, record(&[2])),
            declaration(2, record(&[1])),
            declaration(3, record(&[4])),
            declaration(4, record(&[3, 5])),
            empty(5),
            declaration(6, sum(&[&[7], &[5]])),
            declaration(7, record(&[6])),
        ];
        let query = prepare(&declarations);
        for raw in [1, 2, 6, 7] {
            assert_eq!(query.status(id(raw)), Status::Unknown);
        }
        for raw in [3, 4, 5] {
            assert_eq!(query.status(id(raw)), Status::ProvedEmpty);
        }
    }

    #[test]
    fn descriptors_and_zero_or_unknown_extents_do_not_own_an_element() {
        let declarations = [
            empty(1),
            declaration(
                2,
                StructuralTypeShape::Reference {
                    referent: id(1),
                    access: StructuralAccess::SharedBorrow,
                },
            ),
            declaration(
                3,
                StructuralTypeShape::ElementView {
                    element: id(1),
                    access: None,
                },
            ),
            declaration(
                4,
                StructuralTypeShape::FixedArray {
                    element: id(1),
                    length: ArrayExtent::Literal(0),
                },
            ),
            declaration(
                5,
                StructuralTypeShape::FixedArray {
                    element: id(1),
                    length: ArrayExtent::Application(ScalarApplicationId::new(1).unwrap()),
                },
            ),
            declaration(
                6,
                StructuralTypeShape::FixedArray {
                    element: id(1),
                    length: ArrayExtent::Literal(2),
                },
            ),
            declaration(7, StructuralTypeShape::Opaque),
            declaration(
                8,
                StructuralTypeShape::Reference {
                    referent: id(1),
                    access: StructuralAccess::WriteOnlyBorrow,
                },
            ),
        ];
        let query = prepare(&declarations);
        assert_eq!(query.status(id(6)), Status::ProvedEmpty);
        for raw in [2, 3, 4, 5, 7, 8] {
            assert_eq!(query.status(id(raw)), Status::Unknown);
        }
    }

    #[test]
    fn missing_or_duplicate_identity_is_an_error_not_an_emptiness_proof() {
        let mut broken = empty(1);
        broken.shape = record(&[u64::MAX]);
        assert!(matches!(
            Inhabitation::prepare(&TerminalContracts::new([&broken])),
            Err(Error::MissingType(missing)) if missing == id(u64::MAX)
        ));
        assert!(matches!(
            Inhabitation::prepare(&TerminalContracts::new([&broken, &broken])),
            Err(Error::UnorderedIdentity(duplicate)) if duplicate == id(1)
        ));
    }

    #[test]
    fn appending_completed_contracts_only_visits_the_new_family_rows() {
        struct Counted<'a> {
            inner: TerminalContracts<'a>,
            visits: std::cell::Cell<usize>,
        }
        impl TypeContracts for Counted<'_> {
            fn len(&self) -> usize {
                self.inner.len()
            }
            fn type_id(&self, index: usize) -> StructuralTypeId {
                self.inner.type_id(index)
            }
            fn requires(&self, index: usize) -> &[Proposition] {
                self.inner.requires(index)
            }
            fn cases(&self, index: usize) -> Option<usize> {
                self.inner.cases(index)
            }
            fn visit_owned_fields(
                &self,
                index: usize,
                visit: impl FnMut(Option<usize>, StructuralTypeId),
            ) {
                self.visits.set(self.visits.get() + 1);
                self.inner.visit_owned_fields(index, visit);
            }
        }
        let declarations = [
            empty(1),
            declaration(2, record(&[1])),
            declaration(3, sum(&[&[2]])),
        ];
        let first = TerminalContracts::new(&declarations[..1]);
        let mut query = Inhabitation::prepare(&first).unwrap();
        let rest = Counted {
            inner: TerminalContracts::new(&declarations),
            visits: Default::default(),
        };
        query.extend(&rest).unwrap();
        query.extend(&rest).unwrap();
        assert_eq!(rest.visits.get(), 2);
        assert_eq!(query.status(id(3)), Status::ProvedEmpty);
        assert_eq!(query.extend(&first), Err(Error::ChangedFamily));
        assert_eq!(query.status(id(3)), Status::ProvedEmpty);
    }

    #[test]
    fn only_retained_boolean_consequences_seed_a_named_domain() {
        let unknown = Proposition::Atom(PropositionId::new(1).unwrap());
        assert!(!predicates_are_false(std::slice::from_ref(&unknown)));
        assert!(!predicates_are_false(&[Proposition::Implication {
            premise: Box::new(unknown.clone()),
            conclusion: Box::new(Proposition::Falsehood),
        }]));
        assert!(predicates_are_false(&[Proposition::Conjunction(vec![
            unknown.clone(),
            Proposition::Falsehood
        ])]));
        assert!(!predicates_are_false(&[Proposition::Disjunction(vec![
            unknown,
            Proposition::Falsehood
        ])]));
        assert!(predicates_are_false(&[Proposition::Implication {
            premise: Box::new(Proposition::Truth),
            conclusion: Box::new(Proposition::Falsehood),
        }]));
    }

    fn known_boolean(value: bool) -> (ScalarType, semantic_vocabulary::IntegerValue) {
        (
            ScalarType::Boolean,
            semantic_vocabulary::IntegerValue::Unsigned(u128::from(u8::from(value))),
        )
    }

    fn boolean_value(raw: u64) -> ScalarTerm {
        ScalarTerm::Value {
            id: ValueId::new(raw).unwrap(),
            scalar_type: ScalarType::Boolean,
        }
    }

    #[test]
    fn a_visible_boolean_postcondition_uses_only_scoped_known_actuals() {
        let formal = ValueId::new(1).unwrap();
        let clause = Proposition::Equal(
            ScalarTerm::BooleanNot {
                operand: Box::new(boolean_value(1)),
            },
            ScalarTerm::Boolean(true),
        );
        assert!(predicates_are_false_with(
            std::slice::from_ref(&clause),
            |id| (id == formal).then_some(known_boolean(true))
        ));
        assert!(!predicates_are_false_with(
            std::slice::from_ref(&clause),
            |id| (id == formal).then_some(known_boolean(false))
        ));
        assert!(!predicates_are_false_with(
            std::slice::from_ref(&clause),
            |_| None
        ));
        assert!(!predicates_are_false(std::slice::from_ref(&clause)));
        assert!(predicates_are_false(&[Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::Boolean(false)
        )]));
    }

    #[test]
    fn result_or_foreign_ids_do_not_become_actuals_by_numeric_collision() {
        let formal = ValueId::new(1).unwrap();
        let caller_constant = ValueId::new(2).unwrap();
        // Caller value2 is true, but contract value2 is its RESULT, not a
        // formal. Only the actual parameter mapping may consult that constant.
        let formal_actual = [(formal, caller_constant)];
        let resolve = |id| {
            let (_, actual) = formal_actual.iter().find(|(formal, _)| *formal == id)?;
            (*actual == caller_constant).then_some(known_boolean(true))
        };
        for raw in [2, 99] {
            assert!(!predicates_are_false_with(
                &[Proposition::Equal(
                    boolean_value(raw),
                    ScalarTerm::Boolean(false)
                )],
                resolve
            ));
        }
        assert!(predicates_are_false_with(
            &[Proposition::Equal(
                boolean_value(1),
                ScalarTerm::Boolean(false)
            )],
            resolve
        ));
    }

    #[test]
    fn boolean_equal_and_connectives_preserve_unknown_alternatives() {
        let equal = ScalarTerm::BooleanEqual {
            left: Box::new(boolean_value(1)),
            right: Box::new(ScalarTerm::Boolean(true)),
        };
        let condition = Proposition::Equal(equal, ScalarTerm::Boolean(false));
        assert!(predicates_are_false_with(
            std::slice::from_ref(&condition),
            |_| Some(known_boolean(true))
        ));
        assert!(!predicates_are_false_with(
            std::slice::from_ref(&condition),
            |_| Some(known_boolean(false))
        ));
        let unknown = Proposition::Atom(PropositionId::new(1).unwrap());
        assert!(!predicates_are_false_with(
            &[Proposition::Disjunction(vec![
                condition.clone(),
                unknown.clone()
            ])],
            |_| Some(known_boolean(true))
        ));
        assert!(predicates_are_false_with(
            &[Proposition::Conjunction(vec![condition, unknown])],
            |_| Some(known_boolean(true))
        ));
    }

    #[test]
    fn ill_typed_integer_relationships_do_not_resolve_actuals() {
        use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, WidthParameter};
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let unresolved = IntegerType::parameterized(
            IntegerSign::Unsigned,
            WidthParameter {
                owner: MachineId::new(1).unwrap(),
                value: ValueId::new(2).unwrap(),
            },
        );
        for integer in [integer, unresolved] {
            let clause = Proposition::Equal(
                ScalarTerm::Value {
                    id: ValueId::new(1).unwrap(),
                    scalar_type: ScalarType::Integer(integer),
                },
                ScalarTerm::Boolean(false),
            );
            assert!(!predicates_are_false_with(&[clause], |_| panic!(
                "ill-typed comparison operands cannot acquire actual values"
            )));
        }
    }

    #[test]
    fn typed_integer_actuals_decide_closed_scalar_and_mathematical_comparisons() {
        use semantic_vocabulary::{IntegerMathTerm, IntegerSign, IntegerType, IntegerValue};
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let formal = ValueId::new(1).unwrap();
        let value = ScalarTerm::Value {
            id: formal,
            scalar_type: ScalarType::Integer(integer),
        };
        let zero = ScalarTerm::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(0),
        };
        let clauses = [
            Proposition::Equal(value.clone(), zero.clone()),
            Proposition::LessOrEqual(value.clone(), zero.clone()),
            Proposition::IntegerMathEqual(
                IntegerMathTerm::literal(IntegerValue::Unsigned(1)),
                IntegerMathTerm::Add(
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer,
                        value: formal,
                    }),
                    Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
                ),
            ),
            Proposition::Equal(
                ScalarTerm::IntegerEqual {
                    scalar_type: integer,
                    left: Box::new(value),
                    right: Box::new(zero),
                },
                ScalarTerm::Boolean(true),
            ),
        ];
        for clause in clauses {
            clause.validate().unwrap();
            for actual in [0, 1] {
                assert_eq!(
                    predicates_are_false_with(std::slice::from_ref(&clause), |id| {
                        (id == formal).then_some((
                            ScalarType::Integer(integer),
                            IntegerValue::Unsigned(actual),
                        ))
                    }),
                    actual != 0,
                    "{clause:?}, actual={actual}"
                );
            }
        }
        // Binding zero reverses the original symbolic equality's operand
        // order. The substituted relation must remain canonical and false.
        let reverses = Proposition::IntegerMathEqual(
            IntegerMathTerm::literal(IntegerValue::Unsigned(1)),
            IntegerMathTerm::MathValue {
                source_type: integer,
                value: formal,
            },
        );
        reverses.validate().unwrap();
        assert!(predicates_are_false_with(&[reverses], |_| {
            Some((ScalarType::Integer(integer), IntegerValue::Unsigned(0)))
        }));
    }

    #[test]
    fn scalar_actuals_cannot_reinterpret_carriers_or_capture_result_ids() {
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, MachineId, WidthParameter,
        };
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let signed = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        let formal = ValueId::new(1).unwrap();
        let clause = |id| {
            Proposition::Equal(
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(integer),
                },
                ScalarTerm::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Unsigned(0),
                },
            )
        };
        for actual in [
            (ScalarType::Integer(signed), IntegerValue::Signed(1)),
            (ScalarType::Integer(integer), IntegerValue::Signed(1)),
            (ScalarType::Integer(integer), IntegerValue::Unsigned(256)),
            known_boolean(true),
        ] {
            assert!(!predicates_are_false_with(&[clause(formal)], |_| Some(
                actual
            )));
        }
        for foreign in [2, 99] {
            assert!(!predicates_are_false_with(
                &[clause(ValueId::new(foreign).unwrap())],
                |id| {
                    (id == formal)
                        .then_some((ScalarType::Integer(integer), IntegerValue::Unsigned(1)))
                }
            ));
        }
        let unresolved = IntegerType::parameterized(
            IntegerSign::Unsigned,
            WidthParameter {
                owner: MachineId::new(1).unwrap(),
                value: ValueId::new(2).unwrap(),
            },
        );
        let condition = Proposition::Equal(
            ScalarTerm::Value {
                id: formal,
                scalar_type: ScalarType::Integer(unresolved),
            },
            ScalarTerm::Value {
                id: ValueId::new(2).unwrap(),
                scalar_type: ScalarType::Integer(unresolved),
            },
        );
        assert!(!predicates_are_false_with(&[condition], |id| {
            Some((
                ScalarType::Integer(unresolved),
                IntegerValue::Unsigned(u128::from(id == formal)),
            ))
        }));
    }

    #[test]
    fn closed_scalar_evaluation_keeps_arithmetic_policies_and_guard_failures() {
        use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let literal = |value| ScalarTerm::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(value),
        };
        let exact = ScalarTerm::ExactIntegerAdd {
            scalar_type: integer,
            left: Box::new(literal(255)),
            right: Box::new(literal(1)),
        };
        let wrapping = ScalarTerm::WrappingIntegerAdd {
            scalar_type: integer,
            left: Box::new(literal(255)),
            right: Box::new(literal(1)),
        };
        let saturating = ScalarTerm::SaturatingIntegerAdd {
            scalar_type: integer,
            left: Box::new(literal(255)),
            right: Box::new(literal(1)),
        };
        assert!(!predicates_are_false(&[Proposition::Equal(
            exact.clone(),
            literal(0)
        )]));
        assert!(!predicates_are_false(&[Proposition::Equal(
            wrapping.clone(),
            literal(0)
        )]));
        assert!(predicates_are_false(&[Proposition::Equal(
            saturating,
            literal(0)
        )]));
        assert_eq!(known_scalar_value_with(&exact, |_| None), None);
        assert_eq!(
            known_scalar_value_with(&wrapping, |_| None),
            Some((ScalarType::Integer(integer), IntegerValue::Unsigned(0)))
        );
    }

    #[test]
    fn scalar_predicates_keep_unknown_alternatives_and_opaque_meanings() {
        use semantic_vocabulary::{
            IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType,
            IntegerValue,
        };
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let falsehood = Proposition::Equal(
            ScalarTerm::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(1),
            },
            ScalarTerm::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(0),
            },
        );
        let unknown = Proposition::Atom(PropositionId::new(1).unwrap());
        assert!(predicates_are_false(&[Proposition::Conjunction(vec![
            unknown.clone(),
            falsehood.clone()
        ])]));
        assert!(!predicates_are_false(&[Proposition::Disjunction(vec![
            unknown, falsehood
        ])]));
        assert!(!predicates_are_false(&[
            Proposition::ScalarIeeeFloatComparison {
                kind: IeeeFloatComparisonKind::Equal,
                format: IeeeFloatFormat::Binary32,
                left: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(0x3f80_0000)),
                right: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(0x4000_0000)),
            }
        ]));
    }

    #[test]
    fn a_closed_integer_resource_limit_does_not_prove_a_predicate_false() {
        use semantic_vocabulary::{IntegerMathTerm, IntegerValue};
        let condition = Proposition::IntegerMathEqual(
            IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
            IntegerMathTerm::ShiftLeft {
                value: Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
                count: Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1_000_000))),
            },
        );
        condition.validate().unwrap();
        assert!(!predicates_are_false(&[condition]));
    }
}
