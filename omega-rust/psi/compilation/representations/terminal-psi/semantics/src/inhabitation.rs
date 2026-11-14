//! Conservative emptiness of declared value domains, shared by source and Terminal types.
//!
//! A false requirement is positive evidence of emptiness. Otherwise an owned field
//! can exclude a record value, and every case must be excluded to exclude a sum.
//! Unknown contracts and cycles alone prove nothing. The reverse dependency graph
//! propagates each exclusion once, including through cycles with a real false seed.

use semantic_vocabulary::{Proposition, StructuralTypeId};
use terminal_psi::{StructuralFieldDeclaration, StructuralTypeDeclaration, StructuralTypeShape};

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

/// Whether actual retained clauses exclude every value by Boolean constants
/// alone. An unproved atom/comparison is unknown, even if its type is unserved.
/// This is also usable for a named domain's already-instantiated predicates.
pub fn predicates_are_false(predicates: &[Proposition]) -> bool {
    predicates
        .iter()
        .any(|predicate| constant_truth(predicate) == Some(false))
}

fn constant_truth(predicate: &Proposition) -> Option<bool> {
    match predicate {
        Proposition::Truth => Some(true),
        Proposition::Falsehood => Some(false),
        Proposition::Conjunction(parts) => {
            let mut all_true = true;
            for part in parts {
                match constant_truth(part) {
                    Some(false) => return Some(false),
                    Some(true) => {}
                    None => all_true = false,
                }
            }
            all_true.then_some(true)
        }
        Proposition::Disjunction(parts) => {
            let mut all_false = true;
            for part in parts {
                match constant_truth(part) {
                    Some(true) => return Some(true),
                    Some(false) => {}
                    None => all_false = false,
                }
            }
            all_false.then_some(false)
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => match (constant_truth(premise), constant_truth(conclusion)) {
            (Some(false), _) | (_, Some(true)) => Some(true),
            (Some(true), Some(false)) => Some(false),
            _ => None,
        },
        _ => None,
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
}
