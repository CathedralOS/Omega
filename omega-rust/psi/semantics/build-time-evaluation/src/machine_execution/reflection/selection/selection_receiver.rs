//! The scoped selection receiver.

use crate::machine_execution::reflection::schema_graph::{
    SchemaQueryAuthority, SemanticSchemaGraph,
};
use crate::machine_execution::reflection::selection::requirement_resolution::{
    check_choice_refines, resolve_requirement,
};
use crate::machine_execution::reflection::selection::scoped_members::{
    ScopedMember, graph_members, project, record_matches,
};
use crate::machine_execution::reflection::selection::snapshots::snapshot_revision;
use crate::machine_execution::reflection::selection::{
    MemberSelectionKey, SelectionChoice, SelectionCoverage, SelectionProjection, SelectionRecord,
    SelectionRequirement, SelectionSnapshot,
};
use typed_trees::TypedTrees;

/// The evaluation-local typed-selection receiver. The evaluation root owns
/// it: it is constructed over the authorized schema graph, admits one choice
/// per in-scope member under the projection and coverage contract, and
/// freezes into an owned snapshot. It borrows the typed trees only to
/// validate choices eagerly — the frozen snapshot carries no reference into
/// compiler storage.
#[derive(Debug)]
pub struct ScopedSelectionReceiver<'program> {
    typed: &'program TypedTrees,
    subject_owner_identity: String,
    subject_application_identity: Option<String>,
    authority: SchemaQueryAuthority,
    projection: SelectionProjection,
    coverage: SelectionCoverage,
    requirement: SelectionRequirement,
    members: Vec<ScopedMember>,
    records: Vec<SelectionRecord>,
}

impl<'program> ScopedSelectionReceiver<'program> {
    /// Open a scoped typed selection over `graph`. The selection inherits the
    /// graph's query authority — snapshot manufacture cannot widen it — and
    /// `requirement` must resolve before any policy runs, because a receiver
    /// over an unsatisfiable contract can only produce rejectable snapshots.
    pub fn new(
        typed: &'program TypedTrees,
        graph: &SemanticSchemaGraph,
        projection: SelectionProjection,
        coverage: SelectionCoverage,
        requirement: SelectionRequirement,
    ) -> Result<Self, String> {
        resolve_requirement(typed, &requirement)?;
        let declaration = graph.declaration();
        let members = project(&graph_members(graph)?, &projection)?;
        Ok(Self {
            typed,
            subject_owner_identity: declaration.owner_identity.clone(),
            subject_application_identity: declaration.application_identity.clone(),
            authority: graph.authority().clone(),
            projection,
            coverage,
            requirement,
            members,
            records: Vec::new(),
        })
    }

    /// Resolve an authored member name to its exact key — an ordinary static
    /// name lookup that may find a member or return failure. Names ambiguous
    /// inside the projection (two payloads with the same name in different
    /// cases) fail; use [`Self::member_by_identity`].
    pub fn member(&self, name: &str) -> Result<MemberSelectionKey, String> {
        let mut matches = self.members.iter().filter(|member| member.name == name);
        match (matches.next(), matches.next()) {
            (Some(member), None) => Ok(member.key()),
            (Some(_), Some(_)) => Err(format!(
                "selection member name `{name}` is ambiguous inside the projection"
            )),
            _ => Err(format!(
                "no member named `{name}` is in the receiver's scoped projection"
            )),
        }
    }

    /// Resolve a canonical member identity to its exact key. An identity
    /// shared by more than one in-scope member (possible across distinct
    /// case scopes) is ambiguous; disambiguate through [`Self::members`].
    pub fn member_by_identity(&self, member_identity: &str) -> Result<MemberSelectionKey, String> {
        let mut matches = self
            .members
            .iter()
            .filter(|member| member.member_identity == member_identity);
        match (matches.next(), matches.next()) {
            (Some(member), None) => Ok(member.key()),
            (Some(_), Some(_)) => Err(format!(
                "selection member identity `{member_identity}` is ambiguous inside the projection"
            )),
            _ => Err(format!(
                "selection member `{member_identity}` is outside the receiver's scoped projection"
            )),
        }
    }

    /// Every in-scope member key, in projection order.
    pub fn members(&self) -> impl Iterator<Item = MemberSelectionKey> + '_ {
        self.members.iter().map(ScopedMember::key)
    }

    /// Record an exact operation choice for one member. Selecting twice is
    /// rejected — the receiver is not last-write-wins — and the choice must
    /// refine the receiver's declared requirement on the member's carrier
    /// type, so an insufficient-contract selection fails at selection.
    pub fn select(
        &mut self,
        key: &MemberSelectionKey,
        choice: SelectionChoice,
    ) -> Result<(), String> {
        if matches!(choice, SelectionChoice::Excluded) {
            return Err(
                "an explicit exclusion is recorded with `exclude`, not `select`".to_owned(),
            );
        }
        let member = self.scoped_member_for_key(key)?;
        self.check_unrecorded(&member)?;
        check_choice_refines(self.typed, &self.requirement, &member, &choice)?;
        self.records.push(member.record(choice));
        Ok(())
    }

    /// Record an explicit exclusion for one member. Exclusions are retained
    /// records, not absent coverage; a complete-coverage contract rejects
    /// them outright.
    pub fn exclude(&mut self, key: &MemberSelectionKey) -> Result<(), String> {
        if self.coverage == SelectionCoverage::Complete {
            return Err(format!(
                "member `{}` cannot be excluded under a complete-coverage contract; every in-scope member needs exactly one selection",
                key.member_identity
            ));
        }
        let member = self.scoped_member_for_key(key)?;
        self.check_unrecorded(&member)?;
        self.records.push(member.record(SelectionChoice::Excluded));
        Ok(())
    }

    /// Resolve a key to its member by full-key match. A key whose pinned
    /// member facts no longer match any projected member — minted against a
    /// member whose type or scope drifted — is a stale key/type pair and
    /// rejects distinctly from a member outside the projection.
    fn scoped_member_for_key(&self, key: &MemberSelectionKey) -> Result<ScopedMember, String> {
        if let Some(member) = self.members.iter().find(|member| member.matches_key(key)) {
            return Ok(member.clone());
        }
        if self
            .members
            .iter()
            .any(|member| member.member_identity == key.member_identity)
        {
            Err(format!(
                "selection key for `{}` is stale: it does not match the projected member's type and scope",
                key.member_identity
            ))
        } else {
            Err(format!(
                "selection member `{}` is outside the receiver's scoped projection",
                key.member_identity
            ))
        }
    }

    fn check_unrecorded(&self, member: &ScopedMember) -> Result<(), String> {
        if self
            .records
            .iter()
            .any(|record| record_matches(record, member))
        {
            return Err(format!(
                "member `{}` already has a recorded choice; selecting twice is not last-write-wins",
                member.member_identity
            ));
        }
        Ok(())
    }

    /// Seal the completed selections into an owned snapshot. Every in-scope
    /// member must carry exactly one record — a missing member rejects rather
    /// than fabricating coverage — and the records are emitted in canonical
    /// projection order so equal selections freeze to equal snapshots.
    pub fn freeze(self) -> Result<SelectionSnapshot, String> {
        let mut records = self.records;
        let mut ordered = Vec::with_capacity(self.members.len());
        for member in &self.members {
            let Some(position) = records
                .iter()
                .position(|record| record_matches(record, member))
            else {
                return Err(format!(
                    "member `{}` has no recorded selection or exclusion; a scoped selection covers every in-scope member exactly once",
                    member.member_identity
                ));
            };
            ordered.push(records.swap_remove(position));
        }
        if !records.is_empty() {
            return Err("selection records name members outside the projection".to_owned());
        }
        let mut snapshot = SelectionSnapshot {
            subject_owner_identity: self.subject_owner_identity,
            subject_application_identity: self.subject_application_identity,
            authority: self.authority,
            projection: self.projection,
            coverage: self.coverage,
            requirement: self.requirement,
            records: ordered,
            revision: 0,
        };
        snapshot.revision = snapshot_revision(&snapshot);
        Ok(snapshot)
    }
}
