/*
What a declared domain on a value lets the machine being read do with that
value's contents. Every site of the reader that looks inside a data value or
changes one asks here, so a domain's protection has one owner.

`wiki/spec/language/domains.md`, "Governed subjects and operations": "A
qualified field protects that field; a qualified whole record protects its
contained fields", "Reading stored contents, including discriminants,
comparisons, codecs, and runtime reflection, requires read authority.
Changing contents, including projected writes, whole-value replacement of a
governed place, and atomic stores, requires write authority."

Moving, lending and passing a whole value on is none of these ("Moving an
intact governed value transports its policy and custody"), so those sites
keep the value's complete type and ask nothing.

The record lookups that look inside a value (`member_of`, `case_position`,
`field_type`, `payload_type`, `case_pattern`, `array_element`) take an
`Opened` rather than a bare record, and an `Opened` comes only from here: from
`opened`, which asks, or from `constructing`, which names a fresh carrier
that owes nothing. A new site that projects, tests a case or indexes cannot
skip the question by matching the record out of a qualified type with `_`.
Shape metadata (`data_type`) stays open: "Type/field metadata and already
established proof facts are not new runtime reads".

The Build evaluates the policy's plan before this reader forms an operation
(`wiki/spec/resources/access.md`). An unevaluated plan cannot justify the
operation, so its use is refused. A complete plan stays with the value through
lowering; the canonical judgment checks its rules against the actual memory and
invocation. A domain with no policy restricts neither axis ("Omitting a policy, or omitting the reader or writer axis in a policy,
adds no restriction on that axis").
*/

use semantic_vocabulary::{MachineId, StructuralTypeId};
use terminal_check_tables::{Checked, Construct};

use super::Program;
use crate::body::{Expression, StorageShape, ValueType};
use crate::{Unserved, refuse};

/// What a site does to the contents of a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Access {
    /// Reads inside it: a field, an element, a payload, its case.
    Read,
    /// Selects storage for a shared loan without copying its contents.
    Borrow,
    /// Stores a complete value of the same type over it.
    Replace,
    /// Changes part of it: a store into a field or an element, or an
    /// exclusive loan of one.
    Write,
}

/// A record whose operation can reach canonical access checking, or a fresh
/// carrier being constructed. Only this module makes one; the complete plan
/// remains attached for the storage/caller judgment before execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Opened(StructuralTypeId);

impl Opened {
    /// The record of a carrier being constructed, which no domain governs
    /// yet: "Constructing a fresh valid carrier precedes establishing its
    /// qualification" (domains.md), and "it does not retroactively require
    /// a constructor to read or write an already-protected value".
    pub(super) fn constructing(record: StructuralTypeId) -> Self {
        Self(record)
    }

    pub(super) fn record(self) -> StructuralTypeId {
        self.0
    }
}

impl Program<'_> {
    /// Whether the machine being read may do `access` to the contents of a
    /// value of type `value_type`: every declared domain the type carries
    /// must permit it.
    pub(super) fn authorized(
        &self,
        id: MachineId,
        value_type: ValueType,
        access: Access,
    ) -> Checked<()> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        for domain in value_type.qualification().domains.members() {
            let definition = self.domain_theory(id, domain)?;
            // Only a complete selected plan can reach operation checking.
            // Its canonical field/caller rules remain attached to the value
            // and are checked against actual storage before execution.
            if definition.access_unjudged() {
                return unserved(match access {
                    Access::Read | Access::Borrow => {
                        "read inside a value governed by an access policy"
                    }
                    Access::Replace => "replacement of a value governed by an access policy",
                    Access::Write => "write inside a value governed by an access policy",
                });
            }
            // Pure predicates restrict validity, independently of access.
            // Canonical default requirements keep their restoration windows
            // on the actual storage, including standalone qualified roots.
        }
        Ok(())
    }

    /// The record a data value is, once `access` to its contents is
    /// authorized: `None` for a value that is not data. This is how a site
    /// takes a data value apart.
    pub(super) fn opened(
        &self,
        id: MachineId,
        value_type: Option<ValueType>,
        access: Access,
    ) -> Checked<Option<Opened>> {
        let Some(value_type @ ValueType::Record(record, _, _)) = value_type else {
            return Ok(None);
        };
        self.authorized(id, value_type, access)?;
        Ok(Some(Opened(record)))
    }

    /// Authorize the actual value a place expression projects through.
    pub(super) fn opened_place(
        &self,
        id: MachineId,
        base: &Expression,
        access: Access,
    ) -> Checked<Option<Opened>> {
        self.opened(id, base.result, access)
    }

    /// Authorizes a partial write through `place`, a slot or a path of
    /// fields, elements and case payloads beneath one: each value the path
    /// goes through is changed in part. A subject a match saved is the
    /// place the match captured, so the path continues through it.
    pub(super) fn written_through(&self, id: MachineId, place: &Expression) -> Checked<()> {
        let mut captures = super::storage_path::Captures::new(id, &self.captures);
        super::storage_path::walk(
            place,
            |capture| captures.resolve(capture),
            |_, base| {
                if let Some(value_type) = base.result {
                    self.authorized(id, value_type, Access::Write)?;
                }
                Ok(())
            },
        )?;
        Ok(())
    }

    pub(super) fn representation_read_authorized(
        &self,
        id: MachineId,
        place: &Expression,
    ) -> Checked<()> {
        let mut captures = super::storage_path::Captures::new(id, &self.captures);
        super::storage_path::walk(
            place,
            |capture| captures.resolve(capture),
            |_, base| {
                if let Some(value_type) = base.result {
                    self.authorized(id, value_type, Access::Read)?;
                }
                Ok(())
            },
        )?;
        let mut pending: Vec<_> = place.result.into_iter().collect();
        let mut visited = Vec::new();
        while let Some(value_type) = pending.pop() {
            self.authorized(id, value_type, Access::Read)?;
            let ValueType::Record(record, _, _) = value_type else {
                continue;
            };
            if visited.contains(&record) {
                continue;
            }
            visited.push(record);
            match &self.data_type(id, record)?.shape {
                StorageShape::Record(fields) => {
                    pending.extend(fields.iter().map(|field| field.value_type))
                }
                StorageShape::FixedArray {
                    element,
                    qualification,
                    ..
                }
                | StorageShape::ElementView {
                    element,
                    qualification,
                    ..
                } => pending.push(self.storage_value_type(id, *element, *qualification)?),
                StorageShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
                    ..
                })
                | StorageShape::PrimitiveScalar { .. } => {}
                _ => {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Expression("representation read without ordinary closed storage"),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Refuses a zero-established value of `value_type` when the zero
    /// representation would hold a value in a domain nobody established.
    ///
    /// dependent_values.md, "Default domains and zero initialization": "A
    /// zeroed storage representation can be accessed as an established
    /// value only after its default-domain obligations hold", and "Gating
    /// propagates through containment; a zero-valid first sum case can
    /// represent emptiness without constructing a gated payload." A domain
    /// declared on a constructed field owes its predicates through the owning
    /// data type. Issuance is separate: zero construction must be authorized by
    /// the same exact route as an explicit domain addition.
    pub(super) fn zero_established(
        &mut self,
        id: MachineId,
        value_type: ValueType,
        source: crate::SourceOrigin,
    ) -> Checked<()> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Statement(what)));
        // Each type with whether it is a field of a record in the value.
        let mut pending = vec![(value_type, false)];
        while let Some((value_type, contained)) = pending.pop() {
            for domain in value_type.qualification().domains.members() {
                // A field's predicates are clauses of the data type that
                // declares it (`predicates`), so the checker judges whether
                // zero satisfies them and keeps the window when it does not.
                // A zeroed local that is itself the qualified scalar has no
                // such owner.
                if !contained && has_predicates(self.domain_theory(id, domain)?) {
                    return unserved("zero-established value in a domain that must be established");
                }
                self.issue(id, domain, source)?;
            }
            let ValueType::Record(record, _, _) = value_type else {
                continue;
            };
            let data = self.data_type(id, record)?;
            match &data.shape {
                StorageShape::Opaque => {
                    return unserved("opaque initialization requires boundary-supplied contents");
                }
                StorageShape::Record(fields) => {
                    pending.extend(fields.iter().map(|field| (field.value_type, true)));
                }
                // "The ordinary zero representation of a sum selects the
                // first declared case and a recursively zeroed payload."
                StorageShape::Sum(cases) => pending.extend(
                    cases
                        .first()
                        .into_iter()
                        .flat_map(|case| case.fields.iter().map(|field| (field.value_type, true))),
                ),
                StorageShape::FixedArray {
                    element,
                    qualification,
                    ..
                } => {
                    pending.push((
                        self.storage_value_type(id, *element, *qualification)?,
                        false,
                    ));
                }
                // The canonical zero producer establishes an actual empty
                // immutable sequence; no element is constructed or borrowed.
                StorageShape::ElementView {
                    access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
                    ..
                } => {}
                StorageShape::ElementView { .. } => {
                    return unserved("zero-established mutable sequence");
                }
                StorageShape::Reference { .. } => {
                    return unserved("zero-established reference without backing");
                }
                StorageShape::AtomicCell(_)
                | StorageShape::ByteSequence(_)
                | StorageShape::PrimitiveScalar { .. } => {}
            }
        }
        Ok(())
    }
}

/// Whether membership in the domain is more than its name: it states
/// predicates of its own.
pub(crate) fn has_predicates(definition: &typed_trees::domain::DomainDefinition) -> bool {
    definition.predicate_presence != language_semantics::DomainPredicatePresence::Absent
        || !definition.facts.is_empty()
}
