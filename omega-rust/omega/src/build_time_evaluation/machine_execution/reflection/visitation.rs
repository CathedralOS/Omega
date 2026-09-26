//! Checked per-member call plans for typed runtime visitation.
//!
//! `reflect::visit_runtime_fields<T, Visitor>` specializes the selected
//! callback for each visited member of the bound subject
//! (wiki/spec/language/reflection.md). This module owns the checked-call
//! surface of that elaboration — the plan of resolved invocations — not the
//! runtime adapter stack that later issues subloans, tests the discriminator,
//! or sequences the calls.
//!
//! A [`RuntimeVisitationPlan`] is composed over a frozen
//! [`SelectionSnapshot`]: the snapshot is the coverage and operation
//! authority, and the plan expands its in-scope records into one
//! [`VisitationCall`] per selected member in canonical (authored
//! declaration) order. Each call pairs the resolved operation with a
//! [`FieldInfo`] — the per-member context record the named callback receives
//! as its description argument, carrying the member's exact identity, kind,
//! declaration position, stable number, complete qualified type, owning case,
//! and the carrier application the operation covers. Records describe
//! choices, not proof by construction: [`replay_runtime_visitation_plan`]
//! re-binds the subject from the typed trees, replays the snapshot itself,
//! and recomputes the entire expected call sequence so a forged or stale plan
//! cannot stand in for the checked expansion.
//!
//! Two scope rules fall out of the snapshot's own contract rather than new
//! machinery: an excluded member (partial coverage) produces no call — the
//! snapshot retains the exclusion — and an erased member never receives a
//! runtime borrow, so composing a plan over a snapshot that selects an
//! operation for an erased member rejects with the member's identity instead
//! of silently dropping the recorded choice.

use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;

use super::schema_graph::{SchemaQueryAuthority, resolve_subject};
use super::selection::{
    MemberKind, ScopedMember, SelectionChoice, SelectionRecord, SelectionRequirement,
    SelectionSnapshot, expected_members, project, replay_selection_snapshot,
};

/// The per-member context record a visitation callback receives — the
/// `FieldInfo` argument of the selected operation. Its contents come from the
/// same member derivation the selection receiver and replay use, so the
/// context a callback observes cannot drift from what the policy selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldInfo {
    /// Canonical member identity — the resolved key, never a lookup string.
    member_identity: String,
    /// Presentation name; member identity is the durable key.
    name: String,
    /// The member's role: record/common field, sum case (visited through the
    /// active-case callback), or case payload member.
    kind: MemberKind,
    /// Authored declaration order within the member's owning scope.
    declaration_position: u32,
    /// Optional authored stable number; wire numbering remains the encoding
    /// library's choice.
    stable_number: Option<u64>,
    /// The member's complete qualified type for fields and payloads; `None`
    /// for case members, which carry the enclosing sum as their subject.
    qualified_type: Option<String>,
    /// Canonical identity of the owning case for payload members.
    owner_case_identity: Option<String>,
    /// The member type's head nominal identity — the exact owning
    /// application the selected operation covers. `None` when the member
    /// type has no nominal subject.
    carrier_identity: Option<String>,
}

impl FieldInfo {
    pub fn member_identity(&self) -> &str {
        &self.member_identity
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether this context describes an active-case report
    /// ([`MemberKind::Case`]) or a field/payload borrow.
    pub fn kind(&self) -> MemberKind {
        self.kind
    }

    pub fn declaration_position(&self) -> u32 {
        self.declaration_position
    }

    pub fn stable_number(&self) -> Option<u64> {
        self.stable_number
    }

    pub fn qualified_type(&self) -> Option<&str> {
        self.qualified_type.as_deref()
    }

    pub fn owner_case_identity(&self) -> Option<&str> {
        self.owner_case_identity.as_deref()
    }

    pub fn carrier_identity(&self) -> Option<&str> {
        self.carrier_identity.as_deref()
    }
}

/// The exact operation one visitation call invokes — a selected conformance
/// application or machine identity taken verbatim from the member's frozen
/// selection record. An exclusion is not an operation and never reaches a
/// call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisitationOperation {
    /// Invoke through the named conformance application.
    Conformance { conformance_identity: String },
    /// Invoke the named machine directly.
    Machine { machine_identity: String },
}

impl VisitationOperation {
    /// Canonical identity of the invoked declaration, whichever family it is.
    pub fn identity(&self) -> &str {
        match self {
            Self::Conformance {
                conformance_identity,
            } => conformance_identity,
            Self::Machine { machine_identity } => machine_identity,
        }
    }
}

/// One checked call in the visitation sequence: the selected operation plus
/// the per-member context it receives. The borrowed field operand is the
/// runtime adapter's projection of this member — this surface resolves
/// everything the call needs except the loan itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisitationCall {
    field: FieldInfo,
    operation: VisitationOperation,
}

impl VisitationCall {
    /// The per-member context record passed to the callback.
    pub fn field(&self) -> &FieldInfo {
        &self.field
    }

    /// The resolved operation this call invokes.
    pub fn operation(&self) -> &VisitationOperation {
        &self.operation
    }
}

/// The frozen checked-call plan for one visitation: the call sequence a
/// `visit_runtime_fields` elaboration generates, bound to the subject and
/// requirement of the snapshot it was composed over. Calls are in canonical
/// member order; for a sum subject every admitted case and payload has its
/// application compiled here, and which case report actually executes is the
/// runtime adapter's discriminator dispatch — not this plan's concern.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeVisitationPlan {
    subject_owner_identity: String,
    subject_application_identity: Option<String>,
    authority: SchemaQueryAuthority,
    requirement: SelectionRequirement,
    calls: Vec<VisitationCall>,
    /// Report-only content fingerprint; replay is the authority, never this
    /// value.
    revision: u64,
}

impl RuntimeVisitationPlan {
    /// Compose the checked-call plan over `snapshot`. The snapshot is
    /// replayed first — validation rechecks member, type, requirement, and
    /// authority correspondence before any call is generated — then the
    /// member set is re-derived from the typed trees and expanded into calls.
    /// A snapshot that selects an operation for an erased member rejects:
    /// erased bindings receive no runtime borrow, so a call cannot be emitted
    /// for one and silently dropping the choice would hide it.
    pub fn compose(typed: &TypedTrees, snapshot: &SelectionSnapshot) -> Result<Self, String> {
        replay_selection_snapshot(typed, snapshot)?;
        let data = resolve_subject(typed, snapshot.subject_owner_identity())?;
        let members = expected_members(
            typed,
            data,
            snapshot.subject_owner_identity(),
            snapshot.subject_application_identity(),
        )?;
        let in_scope = project(&members, snapshot.projection())?;
        let calls = expand_calls(&in_scope, snapshot.records())?;
        let mut plan = Self {
            subject_owner_identity: snapshot.subject_owner_identity().to_owned(),
            subject_application_identity: snapshot
                .subject_application_identity()
                .map(str::to_owned),
            authority: snapshot.authority().clone(),
            requirement: snapshot.requirement().clone(),
            calls,
            revision: 0,
        };
        plan.revision = plan_revision(&plan);
        Ok(plan)
    }

    /// Canonical identity of the visited subject's declaration.
    pub fn subject_owner_identity(&self) -> &str {
        &self.subject_owner_identity
    }

    /// The generated concrete application's identity when the subject is a
    /// generic instance; `None` for the declared form.
    pub fn subject_application_identity(&self) -> Option<&str> {
        self.subject_application_identity.as_deref()
    }

    /// The query authority the source snapshot was produced under — a frozen
    /// claim, never a grant.
    pub fn authority(&self) -> &SchemaQueryAuthority {
        &self.authority
    }

    /// The contract every call's operation was selected to refine.
    pub fn requirement(&self) -> &SelectionRequirement {
        &self.requirement
    }

    /// The checked calls in canonical member order. An empty subject
    /// produces an empty sequence — no field calls are made.
    pub fn calls(&self) -> &[VisitationCall] {
        &self.calls
    }

    /// Report-only content fingerprint; replay is the authority, never this
    /// value.
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

/// The `FieldInfo` for one projected member — the shared derivation both
/// sides of the check use so the callback's context is exactly what the
/// selection described.
fn field_info(member: &ScopedMember) -> FieldInfo {
    FieldInfo {
        member_identity: member.member_identity.clone(),
        name: member.name.clone(),
        kind: member.kind,
        declaration_position: member.declaration_position,
        stable_number: member.stable_number,
        qualified_type: member.qualified_type.clone(),
        owner_case_identity: member.owner_case_identity.clone(),
        carrier_identity: member.head_nominal_identity.clone(),
    }
}

/// Expand the projected member/record pairs into the call sequence. Shared
/// by `compose` and replay so the two sides cannot drift on what the
/// canonical visitation is; replay's independence comes from re-deriving the
/// member set from the typed trees, not from a second expansion rule.
fn expand_calls(
    members: &[ScopedMember],
    records: &[SelectionRecord],
) -> Result<Vec<VisitationCall>, String> {
    let mut calls = Vec::new();
    for (member, record) in members.iter().zip(records.iter()) {
        let operation = match &record.choice {
            SelectionChoice::Conformance {
                conformance_identity,
            } => VisitationOperation::Conformance {
                conformance_identity: conformance_identity.clone(),
            },
            SelectionChoice::Machine { machine_identity } => VisitationOperation::Machine {
                machine_identity: machine_identity.clone(),
            },
            SelectionChoice::Excluded => continue,
        };
        if member.erased {
            return Err(format!(
                "member `{}` is erased and receives no runtime borrow; its selected operation cannot be visited",
                member.member_identity
            ));
        }
        calls.push(VisitationCall {
            field: field_info(member),
            operation,
        });
    }
    Ok(calls)
}

/// Independently check a frozen visitation plan against the snapshot it
/// claims to expand and the typed trees it describes.
///
/// Replay first verifies the plan's header claims exactly the snapshot's
/// subject, application, authority, and requirement, then replays the
/// snapshot itself — member correspondence, coverage, authority, and
/// per-choice refinement all re-check there — and finally recomputes the
/// entire expected call sequence from the typed trees. Header mismatches,
/// dropped, reordered, forged, or extra calls, stale `FieldInfo` contents,
/// and tampered revisions all reject with the exact disagreement.
pub fn replay_runtime_visitation_plan(
    typed: &TypedTrees,
    snapshot: &SelectionSnapshot,
    plan: &RuntimeVisitationPlan,
) -> Result<(), String> {
    if plan.subject_owner_identity != snapshot.subject_owner_identity()
        || plan.subject_application_identity.as_deref() != snapshot.subject_application_identity()
        || plan.authority != *snapshot.authority()
        || plan.requirement != *snapshot.requirement()
    {
        return Err(
            "visitation plan header does not match the snapshot it claims to expand".to_owned(),
        );
    }
    replay_selection_snapshot(typed, snapshot)?;
    let data = resolve_subject(typed, snapshot.subject_owner_identity())?;
    let members = expected_members(
        typed,
        data,
        snapshot.subject_owner_identity(),
        snapshot.subject_application_identity(),
    )?;
    let in_scope = project(&members, snapshot.projection()).map_err(|error| {
        format!(
            "visitation plan projection does not resolve against the bound declaration: {error}"
        )
    })?;
    let expected = expand_calls(&in_scope, snapshot.records())?;
    if plan.calls != expected {
        return Err(format!(
            "visitation plan for `{}` does not match the snapshot's checked call expansion",
            data.name.as_str()
        ));
    }
    if plan.revision != plan_revision(plan) {
        return Err(format!(
            "visitation plan for `{}` report fingerprint does not match its frozen contents",
            data.name.as_str()
        ));
    }
    Ok(())
}

/// Report-only FNV-1a fingerprint over the frozen plan contents. The exact
/// replay above is the authority; this value is a cheap staleness gate and
/// diagnostic coordinate, never identity (matching the graph and snapshot
/// fingerprints).
fn plan_revision(plan: &RuntimeVisitationPlan) -> u64 {
    fn byte(hash: &mut u64, value: u8) {
        *hash ^= u64::from(value);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
    fn bytes(hash: &mut u64, value: &[u8]) {
        for value in value {
            byte(hash, *value);
        }
    }
    fn uint(hash: &mut u64, value: u64) {
        bytes(hash, &value.to_le_bytes());
    }
    fn text(hash: &mut u64, value: &str) {
        uint(hash, value.len() as u64);
        bytes(hash, value.as_bytes());
    }
    fn optional_text(hash: &mut u64, value: &Option<String>) {
        match value {
            Some(value) => {
                byte(hash, 1);
                text(hash, value);
            }
            None => byte(hash, 0),
        }
    }

    let mut hash = 0xcbf29ce484222325u64;
    bytes(&mut hash, b"omega.reflect.visitation.v1");
    text(&mut hash, &plan.subject_owner_identity);
    optional_text(&mut hash, &plan.subject_application_identity);
    match &plan.authority {
        SchemaQueryAuthority::OwningScope { requester_identity } => {
            byte(&mut hash, 0);
            text(&mut hash, requester_identity);
        }
        SchemaQueryAuthority::ForeignScope { requester_identity } => {
            byte(&mut hash, 1);
            text(&mut hash, requester_identity);
        }
    }
    text(&mut hash, &plan.requirement.trait_identity);
    optional_text(&mut hash, &plan.requirement.requirement_identity);
    uint(&mut hash, plan.calls.len() as u64);
    for call in &plan.calls {
        let field = &call.field;
        text(&mut hash, &field.member_identity);
        text(&mut hash, &field.name);
        byte(
            &mut hash,
            match field.kind {
                MemberKind::Field => 0,
                MemberKind::Case => 1,
                MemberKind::PayloadField => 2,
            },
        );
        uint(&mut hash, u64::from(field.declaration_position));
        match field.stable_number {
            Some(number) => {
                byte(&mut hash, 1);
                uint(&mut hash, number);
            }
            None => byte(&mut hash, 0),
        }
        optional_text(&mut hash, &field.qualified_type);
        optional_text(&mut hash, &field.owner_case_identity);
        optional_text(&mut hash, &field.carrier_identity);
        match &call.operation {
            VisitationOperation::Conformance {
                conformance_identity,
            } => {
                byte(&mut hash, 0);
                text(&mut hash, conformance_identity);
            }
            VisitationOperation::Machine { machine_identity } => {
                byte(&mut hash, 1);
                text(&mut hash, machine_identity);
            }
        }
    }
    if hash == 0 { 1 } else { hash }
}

#[cfg(test)]
mod tests;
