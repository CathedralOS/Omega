//! Selection snapshots and their replay.

use crate::machine_execution::reflection::schema_graph::{
    SchemaQueryAuthority, check_authority_claim, check_subject_application, resolve_subject,
};
use crate::machine_execution::reflection::selection::requirement_resolution::{
    check_conformance_refines, check_machine_refines, resolve_requirement,
};
use crate::machine_execution::reflection::selection::scoped_members::{
    expected_members, project, record_matches,
};
use crate::machine_execution::reflection::selection::{
    SelectionChoice, SelectionCoverage, SelectionProjection, SelectionRecord, SelectionRequirement,
};
use typed_trees::TypedTrees;

/// An owned, frozen typed-selection snapshot: the scoped member choices one
/// policy evaluation completed, bound to the subject by canonical identity
/// and independent of compiler storage. The revision is a report-only
/// fingerprint; [`replay_selection_snapshot`] is the authority.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionSnapshot {
    pub(crate) subject_owner_identity: String,
    pub(crate) subject_application_identity: Option<String>,
    pub(crate) authority: SchemaQueryAuthority,
    pub(crate) projection: SelectionProjection,
    pub(crate) coverage: SelectionCoverage,
    pub(crate) requirement: SelectionRequirement,
    pub(crate) records: Vec<SelectionRecord>,
    pub(crate) revision: u64,
}

impl SelectionSnapshot {
    /// Canonical identity of the bound schema subject's declaration.
    pub fn subject_owner_identity(&self) -> &str {
        &self.subject_owner_identity
    }

    /// The generated concrete application's identity when the subject is a
    /// generic instance; `None` for the declared form.
    pub fn subject_application_identity(&self) -> Option<&str> {
        self.subject_application_identity.as_deref()
    }

    /// The query authority the selection was produced under — a frozen claim
    /// replay re-checks, never a grant.
    pub fn authority(&self) -> &SchemaQueryAuthority {
        &self.authority
    }

    pub fn projection(&self) -> &SelectionProjection {
        &self.projection
    }

    pub fn coverage(&self) -> SelectionCoverage {
        self.coverage
    }

    pub fn requirement(&self) -> &SelectionRequirement {
        &self.requirement
    }

    /// Frozen member choices in canonical projection order.
    pub fn records(&self) -> &[SelectionRecord] {
        &self.records
    }

    /// Report-only content fingerprint; replay is the authority, never this
    /// value.
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

/// Independently check a frozen selection snapshot against the typed trees it
/// claims to describe.
///
/// Replay re-binds the subject declaration from the recorded owner identity,
/// re-derives the member set and projected scope from `typed`, re-checks the
/// frozen authority claim, resolves every recorded operation, and re-verifies
/// member/type/requirement correspondence and coverage. Forged member
/// identities, stale key/type pairs, missing or duplicate records, exclusions
/// under complete coverage, choices that do not refine the required contract,
/// unresolvable identities, and tampered revisions all reject with the exact
/// disagreement.
pub fn replay_selection_snapshot(
    typed: &TypedTrees,
    snapshot: &SelectionSnapshot,
) -> Result<(), String> {
    let data = resolve_subject(typed, &snapshot.subject_owner_identity)?;
    if data.quotient.is_some() {
        return Err(format!(
            "selection snapshot `{}` binds a quotient declaration, which exposes no representative to reflection",
            data.name.as_str()
        ));
    }
    check_authority_claim(
        typed,
        data,
        &snapshot.authority,
        "selection snapshot",
        data.name.as_str(),
    )?;
    check_subject_application(
        typed,
        data,
        &snapshot.subject_application_identity,
        "selection snapshot",
        data.name.as_str(),
    )?;
    let requirement = resolve_requirement(typed, &snapshot.requirement)?;
    let expected = expected_members(
        typed,
        data,
        &snapshot.subject_owner_identity,
        snapshot.subject_application_identity.as_deref(),
    )?;
    let in_scope = project(&expected, &snapshot.projection).map_err(|error| {
        format!(
            "selection snapshot `{}` projection does not resolve against the bound declaration: {error}",
            data.name.as_str()
        )
    })?;
    if snapshot.records.len() != in_scope.len() {
        return Err(format!(
            "selection snapshot `{}` records {} members but its projection covers {}",
            data.name.as_str(),
            snapshot.records.len(),
            in_scope.len()
        ));
    }
    for (record, member) in snapshot.records.iter().zip(in_scope.iter()) {
        if !record_matches(record, member) {
            return Err(format!(
                "selection snapshot `{}` record for `{}` does not match the bound member",
                data.name.as_str(),
                record.member_identity
            ));
        }
        match &record.choice {
            SelectionChoice::Excluded => {
                if snapshot.coverage == SelectionCoverage::Complete {
                    return Err(format!(
                        "selection snapshot `{}` excludes member `{}` under a complete-coverage contract",
                        data.name.as_str(),
                        record.member_identity
                    ));
                }
            }
            SelectionChoice::Conformance {
                conformance_identity,
            } => check_conformance_refines(typed, conformance_identity, &requirement, member)?,
            SelectionChoice::Machine { machine_identity } => {
                check_machine_refines(typed, machine_identity, &requirement, member)?
            }
        }
    }
    if snapshot.revision != snapshot_revision(snapshot) {
        return Err(format!(
            "selection snapshot `{}` report fingerprint does not match its frozen contents",
            data.name.as_str()
        ));
    }
    Ok(())
}

/// Report-only FNV-1a fingerprint over the frozen snapshot contents. The
/// exact replay above is the authority; this value is a cheap staleness gate
/// and diagnostic coordinate, never identity (the repository's
/// report-only-fingerprint rule, matching `schema_graph`).
pub(crate) fn snapshot_revision(snapshot: &SelectionSnapshot) -> u64 {
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
    bytes(&mut hash, b"omega.reflect.selection.v1");
    text(&mut hash, &snapshot.subject_owner_identity);
    optional_text(&mut hash, &snapshot.subject_application_identity);
    match &snapshot.authority {
        SchemaQueryAuthority::OwningScope { requester_identity } => {
            byte(&mut hash, 0);
            text(&mut hash, requester_identity);
        }
        SchemaQueryAuthority::ForeignScope { requester_identity } => {
            byte(&mut hash, 1);
            text(&mut hash, requester_identity);
        }
    }
    match &snapshot.projection {
        SelectionProjection::DeclaredMembers => byte(&mut hash, 0),
        SelectionProjection::RuntimeMembers => byte(&mut hash, 1),
        SelectionProjection::Members(identities) => {
            byte(&mut hash, 2);
            uint(&mut hash, identities.len() as u64);
            for identity in identities {
                text(&mut hash, identity);
            }
        }
    }
    byte(
        &mut hash,
        match snapshot.coverage {
            SelectionCoverage::Complete => 0,
            SelectionCoverage::Partial => 1,
        },
    );
    text(&mut hash, &snapshot.requirement.trait_identity);
    optional_text(&mut hash, &snapshot.requirement.requirement_identity);
    match &snapshot.requirement.trait_application {
        Some(application) => {
            byte(&mut hash, 1);
            uint(&mut hash, application.type_argument_identities.len() as u64);
            for identity in &application.type_argument_identities {
                text(&mut hash, identity);
            }
            uint(&mut hash, application.lifetime_arguments.len() as u64);
            for ordinal in &application.lifetime_arguments {
                uint(&mut hash, u64::from(*ordinal));
            }
        }
        None => byte(&mut hash, 0),
    }
    uint(&mut hash, snapshot.records.len() as u64);
    for record in &snapshot.records {
        text(&mut hash, &record.member_identity);
        optional_text(&mut hash, &record.qualified_type);
        optional_text(&mut hash, &record.owner_case_identity);
        byte(&mut hash, record.erased as u8);
        match &record.choice {
            SelectionChoice::Conformance {
                conformance_identity,
            } => {
                byte(&mut hash, 0);
                text(&mut hash, conformance_identity);
            }
            SelectionChoice::Machine { machine_identity } => {
                byte(&mut hash, 1);
                text(&mut hash, machine_identity);
            }
            SelectionChoice::Excluded => byte(&mut hash, 2),
        }
    }
    if hash == 0 { 1 } else { hash }
}
