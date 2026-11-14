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

Who may read or write a governed value is stated by its policy's plan
(`wiki/spec/resources/access.md`), and no pass evaluates a plan. A domain
with a policy therefore admits no reader or writer this checker can name,
and the use is refused by name. A domain with no policy restricts neither
axis ("Omitting a policy, or omitting the reader or writer axis in a policy,
adds no restriction on that axis").
*/

use semantic_vocabulary::{MachineId, StructuralTypeId};
use terminal_check_tables::{Checked, Construct};

use super::Program;
use crate::body::{Expression, ExpressionKind, StorageShape, ValueType};
use crate::{Unserved, refuse};

/// What a site does to the contents of a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Access {
    /// Reads inside it: a field, an element, a payload, its case.
    Read,
    /// Stores a complete value of the same type over it.
    Replace,
    /// Changes part of it: a store into a field or an element, or an
    /// exclusive loan of one.
    Write,
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
            let Some(definition) = self.typed.domain_definition(domain) else {
                return unserved("use of a value in a domain with no declaration");
            };
            if definition.access_policy.is_some() {
                return unserved(match access {
                    Access::Read => "read inside a value governed by an access policy",
                    Access::Replace => "replacement of a value governed by an access policy",
                    Access::Write => "write inside a value governed by an access policy",
                });
            }
            // dependent_values.md, "Invariant windows": a write that is not
            // known to preserve the domain "opens a window on the place",
            // closed by proving the predicates again. A window is kept for
            // the clauses of a data type, which a field's domain joins
            // (`predicates`); a data value that carries the domain itself
            // has no such owner, so a partial write under it would leave a
            // value its type no longer describes. A complete value of the
            // same type brings its own membership.
            if access == Access::Write && has_predicates(definition) {
                return unserved("write inside a value whose domain has predicates");
            }
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
    ) -> Checked<Option<StructuralTypeId>> {
        let Some(value_type @ ValueType::Record(record, _, _)) = value_type else {
            return Ok(None);
        };
        self.authorized(id, value_type, access)?;
        Ok(Some(record))
    }

    /// Authorizes a partial write through `place`, a slot or a path of
    /// fields, elements and case payloads beneath one: each value the path
    /// goes through is changed in part. A subject a match saved is the
    /// place the match captured, so the path continues through it.
    pub(super) fn written_through(&self, id: MachineId, place: &Expression) -> Checked<()> {
        let mut at = place;
        let mut followed = 0;
        loop {
            at = match &at.kind {
                ExpressionKind::Field { base, .. }
                | ExpressionKind::Index { base, .. }
                | ExpressionKind::Payload { base, .. } => {
                    if let Some(value_type) = base.result {
                        self.authorized(id, value_type, Access::Write)?;
                    }
                    base
                }
                ExpressionKind::SavedSubject(capture) => {
                    followed += 1;
                    let subject = self
                        .captures
                        .iter()
                        .rev()
                        .find(|(known, _)| known == capture)
                        .filter(|_| followed <= self.captures.len());
                    let Some((_, subject)) = subject else {
                        return Err(refuse(
                            id,
                            Construct::Machine,
                            Unserved::Call("saved subject outside its match capture"),
                        ));
                    };
                    subject
                }
                _ => return Ok(()),
            };
        }
    }

    /// Refuses a zero-established value of `value_type` when the zero
    /// representation would hold a value in a domain nobody established.
    ///
    /// dependent_values.md, "Default domains and zero initialization": "A
    /// zeroed storage representation can be accessed as an established
    /// value only after its default-domain obligations hold", and "Gating
    /// propagates through containment; a zero-valid first sum case can
    /// represent emptiness without constructing a gated payload." A domain
    /// with neither predicates nor an issuer restriction owes nothing
    /// (domains.md: "adds no predicate or issuer obligation"), so zero is a
    /// member; any other domain makes the type gated, and a gated value is
    /// constructed with every field supplied.
    pub(super) fn zero_established(&self, id: MachineId, value_type: ValueType) -> Checked<()> {
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
                let free = self
                    .typed
                    .domain_definition(domain)
                    .is_some_and(|definition| {
                        (contained || !has_predicates(definition))
                            && definition.issuance()
                                == typed_trees::domain::DomainIssuance::Unrestricted
                    });
                if !free {
                    return unserved("zero-established value in a domain that must be established");
                }
            }
            let ValueType::Record(record, _, _) = value_type else {
                continue;
            };
            let data = self.data_type(id, record)?;
            match &data.shape {
                StorageShape::Record(fields) => {
                    pending.extend(fields.iter().map(|field| (field.value_type, true)));
                }
                // "The ordinary zero representation of a sum selects the
                // first declared case and a recursively zeroed payload."
                StorageShape::Sum(cases) => pending.extend(
                    cases
                        .first()
                        .into_iter()
                        .flat_map(|case| case.fields.iter().map(|field| (field.value_type, false))),
                ),
                StorageShape::FixedArray { element, .. } => {
                    pending.push((ValueType::record(*element), false));
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
                StorageShape::ByteSequence(_) | StorageShape::PrimitiveScalar { .. } => {}
            }
        }
        Ok(())
    }
}

/// Whether membership in the domain is more than its name: it states
/// predicates of its own.
pub(super) fn has_predicates(definition: &typed_trees::domain::DomainDefinition) -> bool {
    definition.predicate_presence != language_semantics::DomainPredicatePresence::Absent
        || !definition.facts.is_empty()
}
