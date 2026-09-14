//! Scoped typed selections frozen into independently checked snapshots.
//!
//! A [`ScopedSelectionReceiver`] is the evaluation-local surface a reflection
//! policy writes through: it is constructed over a [`SemanticSchemaGraph`]
//! produced under an explicit [`SchemaQueryAuthority`], projects a scoped
//! subset of the graph's members, and pins the contract every recorded choice
//! must refine. Member keys originate only from the receiver — a policy
//! resolves authored names or canonical identities into
//! [`MemberSelectionKey`]s and cannot mint its own — so a recorded selection
//! always names the resolved exact member, never the string that found it.
//!
//! `freeze` seals the completed records into an owned [`SelectionSnapshot`]:
//! the in-scope member set in canonical order, exactly one record per member,
//! and a report-only revision fingerprint over the frozen contents.
//! [`replay_selection_snapshot`] is the independent check. It re-binds the
//! subject declaration from the recorded owner identity, re-derives the
//! member set, the projected scope, and the authority claim from the typed
//! trees, resolves every recorded operation, and re-checks member, type, and
//! requirement correspondence — the records describe choices, not proof by
//! construction, and snapshot manufacture grants no selection authority.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::machine::Machine;
use typed_trees::trait_definition::{Conformance, ConformanceImplementation, TraitDefinition};

use super::schema_graph::{
    FieldDescription, SchemaNode, SchemaNodeHandle, SchemaQueryAuthority, SemanticSchemaGraph,
    check_authority_claim, check_subject_application, describe_field, exact_symbol_identity,
    member_identity_or_name, resolve_subject,
};

/// Which members of the authorized schema a selection is scoped over.
///
/// The projection freezes into the snapshot verbatim so replay can re-derive
/// the exact in-scope member set instead of trusting the producer's
/// enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionProjection {
    /// Every declared member — fields, cases, and case payloads in authored
    /// order, erased members included. This is the declaration-inspection
    /// scope: inspection describes erased members even though runtime
    /// visitation will never borrow them.
    DeclaredMembers,
    /// Members eligible for a runtime borrow: every case plus the non-erased
    /// fields and payloads. Erased members stay described by the schema but
    /// are outside this scope; selecting one rejects rather than silently
    /// hiding it.
    RuntimeMembers,
    /// An explicit subset of canonical member identities under the same
    /// authority. The list is retained verbatim so replay re-checks coverage
    /// of exactly this set — and only this set.
    Members(Vec<String>),
}

/// What a completed selection must record for every in-scope member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionCoverage {
    /// Every in-scope member carries exactly one selection. Explicit
    /// exclusions reject.
    Complete,
    /// An in-scope member may carry an explicit exclusion instead of a
    /// selection. An exclusion is a retained record, not absent coverage:
    /// a member with no record at all still rejects.
    Partial,
}

/// The required contract every recorded selection must refine.
///
/// `trait_identity` names the exact contract family — the canonical identity
/// of the trait declaration. `requirement_identity` optionally pins one
/// requirement signature inside that trait; `None` accepts a realization of
/// any requirement row. Replay resolves both against the bound program and
/// re-checks each choice's refinement, not merely that a name was chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRequirement {
    pub trait_identity: String,
    pub requirement_identity: Option<String>,
}

impl SelectionRequirement {
    /// The contract family each member selection must refine, canonicalized
    /// from the resolved trait declaration. `requirement` optionally pins one
    /// exact requirement signature of the trait.
    pub fn for_trait(
        typed: &TypedTrees,
        trait_symbol: SymbolHandle,
        requirement: Option<SymbolHandle>,
    ) -> Result<Self, String> {
        let definition = typed
            .traits()
            .iter()
            .find(|definition| definition.symbol == trait_symbol)
            .ok_or_else(|| "a selection requirement names an exact trait declaration".to_owned())?;
        let requirement_identity = match requirement {
            Some(symbol) => {
                if !typed
                    .trait_machine_signatures(definition)
                    .iter()
                    .any(|signature| signature.symbol == symbol)
                {
                    return Err(
                        "a selection requirement pins an exact requirement signature of its trait"
                            .to_owned(),
                    );
                }
                Some(exact_symbol_identity(typed, symbol)?.0)
            }
            None => None,
        };
        Ok(Self {
            trait_identity: exact_symbol_identity(typed, trait_symbol)?.0,
            requirement_identity,
        })
    }
}

/// One recorded choice for a member: an exact static declaration or an
/// explicit exclusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionChoice {
    /// An exact conformance application, recorded as the canonical identity
    /// of the conformance declaration. Replay resolves it and re-checks that
    /// it realizes the required contract on the member's carrier type.
    Conformance { conformance_identity: String },
    /// An exact machine declaration, recorded as the canonical identity of
    /// the machine. Replay resolves it and re-checks that it realizes the
    /// required contract on the member's carrier type.
    Machine { machine_identity: String },
    /// An explicit exclusion, admissible only under
    /// [`SelectionCoverage::Partial`]. Recorded through
    /// [`ScopedSelectionReceiver::exclude`].
    Excluded,
}

impl SelectionChoice {
    /// Canonicalize a resolved conformance declaration into a choice.
    pub fn conformance(typed: &TypedTrees, symbol: SymbolHandle) -> Result<Self, String> {
        if !typed.conformances().iter().any(|c| c.symbol == symbol) {
            return Err(
                "a conformance selection names an exact conformance declaration".to_owned(),
            );
        }
        Ok(Self::Conformance {
            conformance_identity: exact_symbol_identity(typed, symbol)?.0,
        })
    }

    /// Canonicalize a resolved machine declaration into a choice.
    pub fn machine(typed: &TypedTrees, symbol: SymbolHandle) -> Result<Self, String> {
        if !typed.machines().iter().any(|m| m.symbol == symbol) {
            return Err("a machine selection names an exact machine declaration".to_owned());
        }
        Ok(Self::Machine {
            machine_identity: exact_symbol_identity(typed, symbol)?.0,
        })
    }
}

/// One frozen selection record. Records describe choices, not proof by
/// construction: replay re-checks every field against the bound program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRecord {
    /// Canonical member identity from the authorized schema — the resolved
    /// key, never the lookup string.
    pub member_identity: String,
    /// The member's complete qualified type for fields and payloads; `None`
    /// for case members, which carry the enclosing sum as their nominal
    /// subject instead.
    pub qualified_type: Option<String>,
    /// Canonical identity of the owning case for payload members.
    pub owner_case_identity: Option<String>,
    /// Whether the member is erased — described but never runtime-borrowed.
    pub erased: bool,
    pub choice: SelectionChoice,
}

/// An exact member key minted by a receiver's projection. A policy cannot
/// construct keys; it resolves names or identities into them, so the recorded
/// selection retains the resolved member. The pinned type and scope travel in
/// the key so a key minted against a stale member pair rejects at selection
/// instead of projecting onto a drifted member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberSelectionKey {
    member_identity: String,
    qualified_type: Option<String>,
    owner_case_identity: Option<String>,
    erased: bool,
}

impl MemberSelectionKey {
    pub fn member_identity(&self) -> &str {
        &self.member_identity
    }

    /// The member's complete qualified type; `None` for case members.
    pub fn qualified_type(&self) -> Option<&str> {
        self.qualified_type.as_deref()
    }

    /// Erased members are described but receive no runtime borrow.
    pub fn is_erased(&self) -> bool {
        self.erased
    }
}

/// The member's role in the declaring type's shape — the distinction a
/// visitation callback reads on its `FieldInfo` argument to tell an
/// active-case report from a field borrow. Cases carry the enclosing sum as
/// their nominal subject; payloads carry their owning case's identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberKind {
    /// A record or common field.
    Field,
    /// A sum case, visited through the active-case callback.
    Case,
    /// A case payload member.
    PayloadField,
}

/// One in-scope member of a projection, derived from the authorized schema
/// graph by the receiver and re-derived from the typed trees by replay. The
/// two derivations must agree; a snapshot replaying cleanly is the witness.
///
/// `pub(super)` so the visitation slice reuses this exact member record —
/// its `FieldInfo` contexts and carrier checks must not drift from what a
/// selection selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ScopedMember {
    pub(super) member_identity: String,
    pub(super) name: String,
    pub(super) kind: MemberKind,
    /// Authored declaration order within the member's owning scope (the
    /// top-level sequence for record fields and cases, the case's payload
    /// sequence for payload members).
    pub(super) declaration_position: u32,
    /// Optional authored stable number.
    pub(super) stable_number: Option<u64>,
    pub(super) qualified_type: Option<String>,
    pub(super) owner_case_identity: Option<String>,
    pub(super) erased: bool,
    /// First nominal owner identity in the member type's discovery order —
    /// the carrier a selected operation must cover. A self-edge resolves to
    /// the subject's own declaration; a case member's carrier is the
    /// enclosing sum itself. `None` for member types with no nominal
    /// subject, which admit only exclusions.
    pub(super) head_nominal_identity: Option<String>,
}

impl ScopedMember {
    fn key(&self) -> MemberSelectionKey {
        MemberSelectionKey {
            member_identity: self.member_identity.clone(),
            qualified_type: self.qualified_type.clone(),
            owner_case_identity: self.owner_case_identity.clone(),
            erased: self.erased,
        }
    }

    /// Whether a key names exactly this member. The whole key is the match:
    /// member identities can repeat across distinct case scopes (two cases
    /// may each declare a `value` payload), so identity alone is not a
    /// unique handle.
    fn matches_key(&self, key: &MemberSelectionKey) -> bool {
        self.member_identity == key.member_identity
            && self.qualified_type == key.qualified_type
            && self.owner_case_identity == key.owner_case_identity
            && self.erased == key.erased
    }

    fn record(&self, choice: SelectionChoice) -> SelectionRecord {
        SelectionRecord {
            member_identity: self.member_identity.clone(),
            qualified_type: self.qualified_type.clone(),
            owner_case_identity: self.owner_case_identity.clone(),
            erased: self.erased,
            choice,
        }
    }
}

/// Whether a frozen record names exactly this member — the full-key match
/// replay uses when re-binding records to re-derived members.
pub(super) fn record_matches(record: &SelectionRecord, member: &ScopedMember) -> bool {
    record.member_identity == member.member_identity
        && record.qualified_type == member.qualified_type
        && record.owner_case_identity == member.owner_case_identity
        && record.erased == member.erased
}

/// Apply the frozen projection to a candidate member set. Shared by the
/// receiver (graph-derived members) and replay (tree-derived members) so the
/// two sides cannot drift on what "in scope" means.
pub(super) fn project(
    members: &[ScopedMember],
    projection: &SelectionProjection,
) -> Result<Vec<ScopedMember>, String> {
    match projection {
        SelectionProjection::DeclaredMembers => Ok(members.to_vec()),
        SelectionProjection::RuntimeMembers => Ok(members
            .iter()
            .filter(|member| !member.erased)
            .cloned()
            .collect()),
        SelectionProjection::Members(identities) => {
            let mut scoped = Vec::with_capacity(identities.len());
            let mut seen: Vec<&str> = Vec::with_capacity(identities.len());
            for identity in identities {
                if seen.contains(&identity.as_str()) {
                    return Err(format!(
                        "selection projection lists member `{identity}` twice"
                    ));
                }
                seen.push(identity.as_str());
                let mut matches = members
                    .iter()
                    .filter(|member| member.member_identity == *identity);
                match (matches.next(), matches.next()) {
                    (Some(member), None) => scoped.push(member.clone()),
                    (Some(_), Some(_)) => {
                        return Err(format!(
                            "selection projection member `{identity}` is ambiguous in the bound schema"
                        ));
                    }
                    _ => {
                        return Err(format!(
                            "selection projection member `{identity}` is not a member of the bound schema"
                        ));
                    }
                }
            }
            Ok(scoped)
        }
    }
}

/// Derive the in-scope-checkable member set from the authorized schema graph,
/// in authored order: each field, each case, then each case's payload
/// members. This is the producer walk; replay's `expected_members` is its
/// independent tree-derived counterpart.
pub(super) fn graph_members(graph: &SemanticSchemaGraph) -> Result<Vec<ScopedMember>, String> {
    let declaration = graph.declaration();
    let mut members = Vec::new();
    for handle in &declaration.members {
        match graph.node(*handle) {
            SchemaNode::Field(field) => {
                members.push(graph_field_member(graph, field, MemberKind::Field, None)?)
            }
            SchemaNode::Case(case) => {
                members.push(ScopedMember {
                    member_identity: case.member_identity.clone(),
                    name: case.name.clone(),
                    kind: MemberKind::Case,
                    declaration_position: case.declaration_position,
                    stable_number: case.stable_number,
                    qualified_type: None,
                    owner_case_identity: None,
                    erased: false,
                    head_nominal_identity: Some(declaration.owner_identity.clone()),
                });
                for payload in &case.payload_fields {
                    let SchemaNode::Field(field) = graph.node(*payload) else {
                        return Err(format!(
                            "schema `{}` case `{}` payload is not a field node",
                            declaration.name, case.name
                        ));
                    };
                    members.push(graph_field_member(
                        graph,
                        field,
                        MemberKind::PayloadField,
                        Some(case.member_identity.clone()),
                    )?);
                }
            }
            _ => {}
        }
    }
    Ok(members)
}

fn graph_field_member(
    graph: &SemanticSchemaGraph,
    field: &FieldDescription,
    kind: MemberKind,
    owner_case_identity: Option<String>,
) -> Result<ScopedMember, String> {
    // The head nominal in discovery order is the carrier a selected
    // operation must cover: the declaration for a named/generic head, the
    // element for arrays and slices, the referee for references, the base
    // for constrained types — and the subject itself for a self-edge.
    let head_nominal_identity = match field.nominal_references.first() {
        None => None,
        Some(handle) if *handle == graph.root() => Some(graph.declaration().owner_identity.clone()),
        Some(handle) => match graph.node(*handle) {
            SchemaNode::NominalReference(reference) => Some(reference.owner_identity.clone()),
            _ => {
                return Err(format!(
                    "schema member `{}` nominal edge is not a reference node",
                    field.name
                ));
            }
        },
    };
    Ok(ScopedMember {
        member_identity: field.member_identity.clone(),
        name: field.name.clone(),
        kind,
        declaration_position: field.declaration_position,
        stable_number: field.stable_number,
        qualified_type: Some(field.qualified_type.clone()),
        owner_case_identity,
        erased: field.erased,
        head_nominal_identity,
    })
}

/// Re-derive the member set of `data` from the typed trees, in the same
/// authored order `graph_members` produces — the independent counterpart
/// replay checks frozen records against.
pub(super) fn expected_members(
    typed: &TypedTrees,
    data: &DataDefinition,
    owner_identity: &str,
    application_identity: Option<&str>,
) -> Result<Vec<ScopedMember>, String> {
    let mut members = Vec::new();
    for (position, member) in typed.data_members(data).iter().enumerate() {
        match member {
            DataMember::Field(field) => {
                let (description, expectations) = describe_field(
                    typed,
                    field,
                    owner_identity,
                    position as u32,
                    SchemaNodeHandle::INVALID,
                    application_identity,
                )?;
                members.push(ScopedMember {
                    member_identity: description.member_identity,
                    name: description.name,
                    kind: MemberKind::Field,
                    declaration_position: description.declaration_position,
                    stable_number: description.stable_number,
                    qualified_type: Some(description.qualified_type),
                    owner_case_identity: None,
                    erased: description.erased,
                    head_nominal_identity: expectations
                        .first()
                        .map(|expectation| expectation.owner_identity.clone()),
                });
            }
            DataMember::Variant(variant) => {
                let case_identity = member_identity_or_name(
                    typed,
                    variant.symbol,
                    owner_identity,
                    variant.name.as_str(),
                );
                members.push(ScopedMember {
                    member_identity: case_identity.clone(),
                    name: variant.name.as_str().to_owned(),
                    kind: MemberKind::Case,
                    declaration_position: position as u32,
                    stable_number: variant.identity,
                    qualified_type: None,
                    owner_case_identity: None,
                    erased: false,
                    head_nominal_identity: Some(owner_identity.to_owned()),
                });
                for (payload_position, payload_field) in
                    typed.data_payload_fields(variant).iter().enumerate()
                {
                    let (description, expectations) = describe_field(
                        typed,
                        payload_field,
                        owner_identity,
                        payload_position as u32,
                        SchemaNodeHandle::INVALID,
                        application_identity,
                    )?;
                    members.push(ScopedMember {
                        member_identity: description.member_identity,
                        name: description.name,
                        kind: MemberKind::PayloadField,
                        declaration_position: description.declaration_position,
                        stable_number: description.stable_number,
                        qualified_type: Some(description.qualified_type),
                        owner_case_identity: Some(case_identity.clone()),
                        erased: description.erased,
                        head_nominal_identity: expectations
                            .first()
                            .map(|expectation| expectation.owner_identity.clone()),
                    });
                }
            }
        }
    }
    Ok(members)
}

/// The requirement claim resolved against the checking program: the exact
/// trait definition and the pinned requirement signature when present.
/// Checks below compare handles directly once resolution has happened.
pub(super) struct ResolvedRequirement<'program> {
    definition: &'program TraitDefinition,
    requirement_symbol: Option<SymbolHandle>,
}

pub(super) fn resolve_requirement<'program>(
    typed: &'program TypedTrees,
    requirement: &SelectionRequirement,
) -> Result<ResolvedRequirement<'program>, String> {
    let mut matches = typed.traits().iter().filter(|definition| {
        exact_symbol_identity(typed, definition.symbol)
            .is_ok_and(|(identity, _)| identity == requirement.trait_identity)
    });
    let Some(definition) = matches.next() else {
        return Err(format!(
            "selection requirement `{}` does not resolve to a trait declaration in this program",
            requirement.trait_identity
        ));
    };
    if matches.next().is_some() {
        return Err(format!(
            "selection requirement `{}` resolves to more than one trait declaration",
            requirement.trait_identity
        ));
    }
    let requirement_symbol = match &requirement.requirement_identity {
        Some(identity) => Some(
            typed
                .trait_machine_signatures(definition)
                .iter()
                .find(|signature| {
                    exact_symbol_identity(typed, signature.symbol)
                        .is_ok_and(|(candidate, _)| candidate == *identity)
                })
                .map(|signature| signature.symbol)
                .ok_or_else(|| {
                    format!(
                        "selection requirement `{identity}` is not a requirement of trait `{}`",
                        requirement.trait_identity
                    )
                })?,
        ),
        None => None,
    };
    Ok(ResolvedRequirement {
        definition,
        requirement_symbol,
    })
}

fn resolve_conformance<'program>(
    typed: &'program TypedTrees,
    identity: &str,
) -> Result<&'program Conformance, String> {
    let mut matches = typed.conformances().iter().filter(|conformance| {
        exact_symbol_identity(typed, conformance.symbol)
            .is_ok_and(|(candidate, _)| candidate == identity)
    });
    match (matches.next(), matches.next()) {
        (Some(conformance), None) => Ok(conformance),
        (Some(_), Some(_)) => Err(format!(
            "selection conformance `{identity}` resolves to more than one declaration"
        )),
        _ => Err(format!(
            "selection conformance `{identity}` does not resolve to a conformance in this program"
        )),
    }
}

fn resolve_machine<'program>(
    typed: &'program TypedTrees,
    identity: &str,
) -> Result<&'program Machine, String> {
    let mut matches = typed.machines().iter().filter(|machine| {
        exact_symbol_identity(typed, machine.symbol)
            .is_ok_and(|(candidate, _)| candidate == identity)
    });
    match (matches.next(), matches.next()) {
        (Some(machine), None) => Ok(machine),
        (Some(_), Some(_)) => Err(format!(
            "selection machine `{identity}` resolves to more than one declaration"
        )),
        _ => Err(format!(
            "selection machine `{identity}` does not resolve to a machine in this program"
        )),
    }
}

/// The carrier a selected operation covers must be the member type's head
/// nominal — the declaration for a named member, the element type for an
/// array member, the referee for a reference member, the enclosing sum for a
/// case member. A member type with no nominal subject admits no operation.
fn check_carrier_covers_member(
    typed: &TypedTrees,
    carrier: SymbolHandle,
    member: &ScopedMember,
) -> Result<(), String> {
    let Some(head) = &member.head_nominal_identity else {
        return Err(format!(
            "selection for `{}` requires an operation on the member's declared type, which has no nominal subject",
            member.member_identity
        ));
    };
    if !carrier.is_valid() {
        return Err(format!(
            "selection for `{}` names an operation with no carrier",
            member.member_identity
        ));
    }
    let carrier_identity = exact_symbol_identity(typed, carrier)?.0;
    if carrier_identity != *head {
        return Err(format!(
            "selection for `{}` names an operation on `{carrier_identity}` which does not cover the member's declared type",
            member.member_identity
        ));
    }
    Ok(())
}

/// Whether `machine` binds a requirement of the resolved trait through the
/// attached-form name rule: `Carrier::requirement_name` attached to the
/// conformance's carrier. This is the language's own binding for
/// `AttachedRequirementMachines` conformances; an authored `satisfies` edge
/// on the machine is checked separately by the machine path.
fn machine_binds_attached_requirement(
    typed: &TypedTrees,
    machine: &Machine,
    conformance: &Conformance,
    requirement: &ResolvedRequirement,
) -> bool {
    if machine.attached_data_symbol != conformance.carrier_symbol {
        return false;
    }
    let carrier_name = typed.symbols.name(conformance.carrier_symbol);
    typed
        .trait_machine_signatures(requirement.definition)
        .iter()
        .any(|signature| {
            requirement
                .requirement_symbol
                .is_none_or(|required| signature.symbol == required)
                && machine.name.as_str() == format!("{carrier_name}::{}", signature.name.as_str())
        })
}

pub(super) fn check_conformance_refines(
    typed: &TypedTrees,
    conformance_identity: &str,
    requirement: &ResolvedRequirement,
    member: &ScopedMember,
) -> Result<(), String> {
    let conformance = resolve_conformance(typed, conformance_identity)?;
    if conformance.trait_symbol != requirement.definition.symbol {
        return Err(format!(
            "selection for `{}` names conformance `{conformance_identity}` of a different trait; insufficient contract",
            member.member_identity
        ));
    }
    check_carrier_covers_member(typed, conformance.carrier_symbol, member)?;
    if let Some(requirement_symbol) = requirement.requirement_symbol {
        let realized = match &conformance.implementation {
            ConformanceImplementation::Closed { rows } => rows.iter().any(|row| {
                row.requirement == requirement_symbol && row.realization_machine.is_valid()
            }),
            ConformanceImplementation::AttachedRequirementMachines => {
                let requirement_name = typed
                    .trait_machine_signatures(requirement.definition)
                    .iter()
                    .find(|signature| signature.symbol == requirement_symbol)
                    .map(|signature| signature.name.as_str().to_owned());
                match requirement_name {
                    Some(requirement_name) => {
                        let carrier_name = typed.symbols.name(conformance.carrier_symbol);
                        let attached = format!("{carrier_name}::{requirement_name}");
                        typed.machines().iter().any(|machine| {
                            machine.attached_data_symbol == conformance.carrier_symbol
                                && (machine.name.as_str() == attached
                                    || typed.machine_trait_conformances(machine).iter().any(
                                        |edge| {
                                            edge.symbol == conformance.trait_symbol
                                                && edge.requirement_symbol == requirement_symbol
                                        },
                                    ))
                        })
                    }
                    None => false,
                }
            }
        };
        if !realized {
            return Err(format!(
                "selection for `{}` names conformance `{conformance_identity}` which does not realize the pinned requirement",
                member.member_identity
            ));
        }
    }
    Ok(())
}

pub(super) fn check_machine_refines(
    typed: &TypedTrees,
    machine_identity: &str,
    requirement: &ResolvedRequirement,
    member: &ScopedMember,
) -> Result<(), String> {
    let machine = resolve_machine(typed, machine_identity)?;
    // Path A: an authored `satisfies` edge on the machine itself names the
    // trait and optionally the exact requirement row.
    if typed
        .machine_trait_conformances(machine)
        .iter()
        .any(|edge| {
            edge.symbol == requirement.definition.symbol
                && requirement
                    .requirement_symbol
                    .is_none_or(|required| edge.requirement_symbol == required)
        })
    {
        return check_carrier_covers_member(typed, machine.attached_data_symbol, member);
    }
    // Path B: the machine is a realization of the required trait on the
    // member's carrier — a closed conformance row, or the attached-form
    // `Carrier::requirement` name binding.
    for conformance in typed
        .conformances()
        .iter()
        .filter(|conformance| conformance.trait_symbol == requirement.definition.symbol)
    {
        let realizes = match &conformance.implementation {
            ConformanceImplementation::Closed { rows } => rows.iter().any(|row| {
                row.realization_machine == machine.symbol
                    && requirement
                        .requirement_symbol
                        .is_none_or(|required| row.requirement == required)
            }),
            ConformanceImplementation::AttachedRequirementMachines => {
                machine_binds_attached_requirement(typed, machine, conformance, requirement)
            }
        };
        if realizes {
            return check_carrier_covers_member(typed, conformance.carrier_symbol, member);
        }
    }
    Err(format!(
        "selection for `{}` names machine `{machine_identity}` which does not realize the required contract",
        member.member_identity
    ))
}

/// Eager producer-side check that a choice refines the required contract for
/// `member`. Replay runs the same checks independently; a rejection here is
/// the policy's error at the point of selection.
fn check_choice_refines(
    typed: &TypedTrees,
    requirement: &SelectionRequirement,
    member: &ScopedMember,
    choice: &SelectionChoice,
) -> Result<(), String> {
    let requirement = resolve_requirement(typed, requirement)?;
    match choice {
        SelectionChoice::Conformance {
            conformance_identity,
        } => check_conformance_refines(typed, conformance_identity, &requirement, member),
        SelectionChoice::Machine { machine_identity } => {
            check_machine_refines(typed, machine_identity, &requirement, member)
        }
        SelectionChoice::Excluded => Ok(()),
    }
}

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

/// An owned, frozen typed-selection snapshot: the scoped member choices one
/// policy evaluation completed, bound to the subject by canonical identity
/// and independent of compiler storage. The revision is a report-only
/// fingerprint; [`replay_selection_snapshot`] is the authority.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionSnapshot {
    subject_owner_identity: String,
    subject_application_identity: Option<String>,
    authority: SchemaQueryAuthority,
    projection: SelectionProjection,
    coverage: SelectionCoverage,
    requirement: SelectionRequirement,
    records: Vec<SelectionRecord>,
    revision: u64,
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
fn snapshot_revision(snapshot: &SelectionSnapshot) -> u64 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reflection::construct_semantic_schema_graph;
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use tokens_to_syntax_trees::parse_syntax_trees;

    /// A program exercising both choice families: authored `satisfies`
    /// machines, closed conformance applications naming realizations, and a
    /// second trait so an insufficient-contract choice has something to be.
    const ENCODE_PROGRAM: &str = "
        trait Encode { machine encode(&self) -> u64; }
        trait Hash { machine hash(&self) -> u64; }
        data Health { v: u32; }
        data Speed { v: f32; }
        machine Health::encode(&self) -> u64 satisfies Encode::encode { 0 }
        machine Speed::encode(&self) -> u64 satisfies Encode::encode { 0 }
        machine Health::hash(&self) -> u64 satisfies Hash::hash { 0 }
        HealthEnc: Health satisfies Encode { Encode::encode = Health::encode; }
        SpeedEnc: Speed satisfies Encode { Encode::encode = Speed::encode; }
        HealthHash: Health satisfies Hash { Hash::hash = Health::hash; }
        data Player { health: Health; speed: Speed; }
    ";

    /// A pinned-requirement program: `Codec` carries two requirements so one
    /// exact row can be demanded while a sibling satisfies a different one.
    const CODEC_PROGRAM: &str = "
        trait Codec { machine encode(&self) -> u64; machine decode(&self) -> u64; }
        data Blob { bytes: [u8; 4]; }
        machine Blob::encode(&self) -> u64 satisfies Codec::encode { 0 }
        machine Blob::decode(&self) -> u64 satisfies Codec::decode { 0 }
        data Wrap { blob: Blob; }
    ";

    /// A closed conformance whose realization machine carries no authored
    /// `satisfies` edge: the closed row alone must admit the machine choice.
    const CLOSED_PROGRAM: &str = "
        trait Shape { machine code(&self) -> i32; }
        data Circle { r: u32; }
        machine Circle::code(&self) -> i32 { 9 }
        CircleShape: Circle satisfies Shape { Shape::code = Circle::code; }
        data Board { c: Circle; }
    ";

    /// A bodyless conformance: the attached `Carrier::requirement` machine
    /// name binding alone must admit both the conformance and the machine.
    const ATTACHED_PROGRAM: &str = "
        trait Shape { machine code(&self) -> i32; }
        data Circle { r: u32; }
        machine Circle::code(&self) -> i32 { 9 }
        CircleShape: Circle satisfies Shape;
        data Board { c: Circle; }
    ";

    /// A sum subject whose members include cases and their payloads.
    const SUM_PROGRAM: &str = "
        trait Encode { machine encode(&self) -> u64; }
        data Health { v: u32; }
        machine Health::encode(&self) -> u64 satisfies Encode::encode { 0 }
        HealthEnc: Health satisfies Encode { Encode::encode = Health::encode; }
        data Outcome { case Ready(value: Health); case Empty; }
        machine Outcome::encode(&self) -> u64 satisfies Encode::encode { 0 }
        OutcomeEnc: Outcome satisfies Encode { Encode::encode = Outcome::encode; }
    ";

    fn typed(source: &str) -> TypedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        lower_symbol_resolved_trees(&resolved).expect("type")
    }

    fn subject(typed: &TypedTrees, name: &str) -> SymbolHandle {
        typed
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == name)
            .unwrap_or_else(|| panic!("data `{name}` exists"))
            .symbol
    }

    fn owning() -> SchemaQueryAuthority {
        SchemaQueryAuthority::OwningScope {
            requester_identity: "test::requester".to_owned(),
        }
    }

    fn foreign() -> SchemaQueryAuthority {
        SchemaQueryAuthority::ForeignScope {
            requester_identity: "pkg::consumer".to_owned(),
        }
    }

    fn graph(typed: &TypedTrees, name: &str) -> SemanticSchemaGraph {
        construct_semantic_schema_graph(typed, subject(typed, name), owning())
            .unwrap_or_else(|error| panic!("schema for `{name}` constructs: {error}"))
    }

    fn requirement(
        typed: &TypedTrees,
        trait_name: &str,
        requirement_name: Option<&str>,
    ) -> SelectionRequirement {
        let definition = typed
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == trait_name)
            .unwrap_or_else(|| panic!("trait `{trait_name}` exists"));
        let requirement = requirement_name.map(|name| {
            typed
                .trait_machine_signatures(definition)
                .iter()
                .find(|signature| signature.name.as_str() == name)
                .unwrap_or_else(|| panic!("requirement `{trait_name}::{name}` exists"))
                .symbol
        });
        SelectionRequirement::for_trait(typed, definition.symbol, requirement)
            .expect("requirement resolves")
    }

    fn machine_choice(typed: &TypedTrees, name: &str) -> SelectionChoice {
        let symbol = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine `{name}` exists"))
            .symbol;
        SelectionChoice::machine(typed, symbol).expect("machine choice canonicalizes")
    }

    fn conformance_choice(typed: &TypedTrees, name: &str) -> SelectionChoice {
        let symbol = typed
            .conformances()
            .iter()
            .find(|conformance| typed.symbols.name(conformance.symbol) == name)
            .unwrap_or_else(|| panic!("conformance `{name}` exists"))
            .symbol;
        SelectionChoice::conformance(typed, symbol).expect("conformance choice canonicalizes")
    }

    fn receiver<'program>(
        typed: &'program TypedTrees,
        graph: &SemanticSchemaGraph,
        trait_name: &str,
    ) -> ScopedSelectionReceiver<'program> {
        ScopedSelectionReceiver::new(
            typed,
            graph,
            SelectionProjection::DeclaredMembers,
            SelectionCoverage::Complete,
            requirement(typed, trait_name, None),
        )
        .expect("receiver opens")
    }

    /// A valid snapshot is mutated, resealed so only the mutation is under
    /// test, then replayed — the witness that replay checks content, not the
    /// fingerprint.
    fn reseal(snapshot: &mut SelectionSnapshot) {
        snapshot.revision = snapshot_revision(snapshot);
    }

    #[test]
    fn complete_machine_selection_freezes_and_replays() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let mut receiver = receiver(&typed, &graph, "Encode");
        receiver
            .select(
                &receiver.member("health").expect("health key"),
                machine_choice(&typed, "Health::encode"),
            )
            .expect("health selects");
        receiver
            .select(
                &receiver.member("speed").expect("speed key"),
                machine_choice(&typed, "Speed::encode"),
            )
            .expect("speed selects");
        let snapshot = receiver.freeze().expect("complete coverage freezes");
        assert_eq!(snapshot.records().len(), 2);
        assert_eq!(snapshot.records()[0].member_identity, "Player::health");
        assert_eq!(snapshot.records()[1].member_identity, "Player::speed");
        replay_selection_snapshot(&typed, &snapshot).expect("a valid snapshot replays");
    }

    #[test]
    fn conformance_choices_refine_and_replay() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let mut receiver = receiver(&typed, &graph, "Encode");
        receiver
            .select(
                &receiver.member("health").expect("health key"),
                conformance_choice(&typed, "HealthEnc"),
            )
            .expect("health selects");
        receiver
            .select(
                &receiver.member("speed").expect("speed key"),
                conformance_choice(&typed, "SpeedEnc"),
            )
            .expect("speed selects");
        let snapshot = receiver.freeze().expect("freezes");
        replay_selection_snapshot(&typed, &snapshot).expect("replays");
    }

    #[test]
    fn equal_selections_freeze_to_equal_snapshots() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let snapshot = {
            let mut receiver = receiver(&typed, &graph, "Encode");
            receiver
                .select(
                    &receiver.member("health").expect("key"),
                    machine_choice(&typed, "Health::encode"),
                )
                .expect("selects");
            receiver
                .select(
                    &receiver.member("speed").expect("key"),
                    machine_choice(&typed, "Speed::encode"),
                )
                .expect("selects");
            receiver.freeze().expect("freezes")
        };
        // The same policy in a different authored order seals identically:
        // canonical order is a property of the snapshot, not of the calls.
        let reordered = {
            let mut receiver = receiver(&typed, &graph, "Encode");
            receiver
                .select(
                    &receiver.member("speed").expect("key"),
                    machine_choice(&typed, "Speed::encode"),
                )
                .expect("selects");
            receiver
                .select(
                    &receiver.member("health").expect("key"),
                    machine_choice(&typed, "Health::encode"),
                )
                .expect("selects");
            receiver.freeze().expect("freezes")
        };
        assert_eq!(snapshot, reordered);
        replay_selection_snapshot(&typed, &reordered).expect("replays");
    }

    #[test]
    fn closed_conformance_row_admits_machine_choice() {
        let typed = typed(CLOSED_PROGRAM);
        let graph = graph(&typed, "Board");
        let mut receiver = receiver(&typed, &graph, "Shape");
        // `Circle::code` carries no authored `satisfies` edge; only the
        // closed conformance row realizes `Shape::code` on `Circle`.
        receiver
            .select(
                &receiver.member("c").expect("key"),
                machine_choice(&typed, "Circle::code"),
            )
            .expect("the closed row realizes the requirement");
        let snapshot = receiver.freeze().expect("freezes");
        replay_selection_snapshot(&typed, &snapshot).expect("replays");
    }

    #[test]
    fn attached_conformance_admits_its_machines() {
        let typed = typed(ATTACHED_PROGRAM);
        let graph = graph(&typed, "Board");
        let mut receiver = ScopedSelectionReceiver::new(
            &typed,
            &graph,
            SelectionProjection::DeclaredMembers,
            SelectionCoverage::Complete,
            requirement(&typed, "Shape", Some("code")),
        )
        .expect("receiver opens");
        // The bodyless conformance's `Circle::code` name binding realizes the
        // pinned requirement; both the conformance and the machine itself are
        // admissible choices.
        receiver
            .select(
                &receiver.member("c").expect("key"),
                machine_choice(&typed, "Circle::code"),
            )
            .expect("the attached machine binds the pinned requirement");
        let snapshot = receiver.freeze().expect("freezes");
        replay_selection_snapshot(&typed, &snapshot).expect("replays");

        let mut receiver = ScopedSelectionReceiver::new(
            &typed,
            &graph,
            SelectionProjection::DeclaredMembers,
            SelectionCoverage::Complete,
            requirement(&typed, "Shape", Some("code")),
        )
        .expect("receiver opens");
        receiver
            .select(
                &receiver.member("c").expect("key"),
                conformance_choice(&typed, "CircleShape"),
            )
            .expect("the conformance realizes the pinned requirement");
        let snapshot = receiver.freeze().expect("freezes");
        replay_selection_snapshot(&typed, &snapshot).expect("replays");
    }

    #[test]
    fn pinned_requirement_accepts_the_exact_row_and_rejects_siblings() {
        let typed = typed(CODEC_PROGRAM);
        let graph = graph(&typed, "Wrap");
        let mut receiver = ScopedSelectionReceiver::new(
            &typed,
            &graph,
            SelectionProjection::DeclaredMembers,
            SelectionCoverage::Complete,
            requirement(&typed, "Codec", Some("encode")),
        )
        .expect("receiver opens");
        let blob = receiver.member("blob").expect("key");
        let error = receiver
            .select(&blob, machine_choice(&typed, "Blob::decode"))
            .expect_err("a machine satisfying only `Codec::decode` cannot fill `Codec::encode`");
        assert!(error.contains("does not realize"), "{error}");
        receiver
            .select(&blob, machine_choice(&typed, "Blob::encode"))
            .expect("the pinned row selects");
        let snapshot = receiver.freeze().expect("freezes");
        replay_selection_snapshot(&typed, &snapshot).expect("replays");
    }

    #[test]
    fn missing_selection_rejects_at_freeze() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let mut receiver = receiver(&typed, &graph, "Encode");
        receiver
            .select(
                &receiver.member("health").expect("key"),
                machine_choice(&typed, "Health::encode"),
            )
            .expect("selects");
        let error = receiver
            .freeze()
            .expect_err("an unrecorded member rejects rather than fabricating coverage");
        assert!(error.contains("no recorded selection"), "{error}");
    }

    #[test]
    fn selecting_twice_rejects() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let mut receiver = receiver(&typed, &graph, "Encode");
        let health = receiver.member("health").expect("key");
        receiver
            .select(&health, machine_choice(&typed, "Health::encode"))
            .expect("selects");
        let error = receiver
            .select(&health, machine_choice(&typed, "Health::encode"))
            .expect_err("a second selection is not last-write-wins");
        assert!(error.contains("not last-write-wins"), "{error}");

        let mut receiver = ScopedSelectionReceiver::new(
            &typed,
            &graph,
            SelectionProjection::DeclaredMembers,
            SelectionCoverage::Partial,
            requirement(&typed, "Encode", None),
        )
        .expect("partial receiver opens");
        let health = receiver.member("health").expect("key");
        receiver
            .select(&health, machine_choice(&typed, "Health::encode"))
            .expect("selects");
        let error = receiver
            .exclude(&health)
            .expect_err("an exclusion over a recorded selection also rejects");
        assert!(error.contains("not last-write-wins"), "{error}");
    }

    #[test]
    fn stale_key_rejects() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let mut receiver = receiver(&typed, &graph, "Encode");
        let mut health = receiver.member("health").expect("key");
        health.qualified_type = Some("stale type".to_owned());
        let error = receiver
            .select(&health, machine_choice(&typed, "Health::encode"))
            .expect_err("a key minted against a drifted member rejects");
        assert!(error.contains("stale"), "{error}");
    }

    #[test]
    fn erased_member_is_outside_the_runtime_scope() {
        let typed = typed(
            "trait Encode { machine encode(&self) -> u64; } \
             machine Rec::encode(&self) -> u64 satisfies Encode::encode { 0 } \
             RecEnc: Rec satisfies Encode { Encode::encode = Rec::encode; } \
             data Rec { secret [erased]: i32; tag: u8; }",
        );
        let graph = graph(&typed, "Rec");
        let declared = ScopedSelectionReceiver::new(
            &typed,
            &graph,
            SelectionProjection::DeclaredMembers,
            SelectionCoverage::Partial,
            requirement(&typed, "Encode", None),
        )
        .expect("declared scope opens");
        let secret = declared
            .member("secret")
            .expect("the erased member is described");
        assert!(secret.is_erased());
        let mut runtime = ScopedSelectionReceiver::new(
            &typed,
            &graph,
            SelectionProjection::RuntimeMembers,
            SelectionCoverage::Partial,
            requirement(&typed, "Encode", None),
        )
        .expect("runtime scope opens");
        let error = runtime
            .member("secret")
            .expect_err("the erased member is outside the runtime scope");
        assert!(error.contains("scoped projection"), "{error}");
        let error = runtime
            .select(&secret, conformance_choice(&typed, "RecEnc"))
            .expect_err("a key minted in the declared scope cannot select in the runtime scope");
        assert!(error.contains("scoped projection"), "{error}");
        let tag = runtime.member("tag").expect("tag key");
        runtime
            .exclude(&tag)
            .expect("the only runtime member may exclude");
        let snapshot = runtime.freeze().expect("freezes");
        replay_selection_snapshot(&typed, &snapshot).expect("replays");
    }

    #[test]
    fn insufficient_contract_rejects_at_selection() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let mut receiver = receiver(&typed, &graph, "Encode");
        let health = receiver.member("health").expect("key");
        let error = receiver
            .select(&health, machine_choice(&typed, "Health::hash"))
            .expect_err("a `Hash` machine cannot satisfy an `Encode` requirement");
        assert!(error.contains("does not realize"), "{error}");
        let error = receiver
            .select(&health, conformance_choice(&typed, "HealthHash"))
            .expect_err("a `Hash` conformance cannot satisfy an `Encode` requirement");
        assert!(error.contains("different trait"), "{error}");
        let error = receiver
            .select(&health, conformance_choice(&typed, "SpeedEnc"))
            .expect_err("an `Encode` conformance on the wrong carrier cannot cover `health`");
        assert!(error.contains("does not cover"), "{error}");
    }

    #[test]
    fn unresolvable_requirement_rejects_at_open() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let error = ScopedSelectionReceiver::new(
            &typed,
            &graph,
            SelectionProjection::DeclaredMembers,
            SelectionCoverage::Complete,
            SelectionRequirement {
                trait_identity: "No::SuchTrait".to_owned(),
                requirement_identity: None,
            },
        )
        .expect_err("a requirement naming no trait cannot open a receiver");
        assert!(error.contains("does not resolve"), "{error}");
    }

    #[test]
    fn partial_coverage_retains_exclusions() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let mut receiver = ScopedSelectionReceiver::new(
            &typed,
            &graph,
            SelectionProjection::DeclaredMembers,
            SelectionCoverage::Partial,
            requirement(&typed, "Encode", None),
        )
        .expect("receiver opens");
        receiver
            .select(
                &receiver.member("health").expect("key"),
                machine_choice(&typed, "Health::encode"),
            )
            .expect("selects");
        receiver
            .exclude(&receiver.member("speed").expect("key"))
            .expect("exclusion records");
        let snapshot = receiver.freeze().expect("freezes");
        assert_eq!(
            snapshot.records()[1].choice,
            SelectionChoice::Excluded,
            "the exclusion is a retained record"
        );
        replay_selection_snapshot(&typed, &snapshot).expect("replays");
    }

    #[test]
    fn complete_coverage_rejects_exclusions() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let mut receiver = receiver(&typed, &graph, "Encode");
        let error = receiver
            .exclude(&receiver.member("speed").expect("key"))
            .expect_err("complete coverage admits no exclusions");
        assert!(error.contains("complete-coverage"), "{error}");

        // Forge the same shape past the receiver: a partial snapshot with an
        // exclusion relabeled as complete must still reject on replay.
        let mut receiver = ScopedSelectionReceiver::new(
            &typed,
            &graph,
            SelectionProjection::DeclaredMembers,
            SelectionCoverage::Partial,
            requirement(&typed, "Encode", None),
        )
        .expect("receiver opens");
        receiver
            .select(
                &receiver.member("health").expect("key"),
                machine_choice(&typed, "Health::encode"),
            )
            .expect("selects");
        receiver
            .exclude(&receiver.member("speed").expect("key"))
            .expect("exclusion records");
        let mut snapshot = receiver.freeze().expect("freezes");
        snapshot.coverage = SelectionCoverage::Complete;
        reseal(&mut snapshot);
        let error = replay_selection_snapshot(&typed, &snapshot)
            .expect_err("an exclusion under forged complete coverage must reject");
        assert!(error.contains("complete-coverage"), "{error}");
    }

    #[test]
    fn explicit_member_projection_scopes_the_selection() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let mut receiver = ScopedSelectionReceiver::new(
            &typed,
            &graph,
            SelectionProjection::Members(vec!["Player::speed".to_owned()]),
            SelectionCoverage::Complete,
            requirement(&typed, "Encode", None),
        )
        .expect("projection opens");
        let error = receiver
            .member("health")
            .expect_err("`health` is outside the projected member list");
        assert!(error.contains("scoped projection"), "{error}");
        receiver
            .select(
                &receiver.member("speed").expect("speed key"),
                machine_choice(&typed, "Speed::encode"),
            )
            .expect("selects");
        let snapshot = receiver.freeze().expect("one-member coverage freezes");
        assert_eq!(snapshot.records().len(), 1);
        replay_selection_snapshot(&typed, &snapshot).expect("replays");

        let error = ScopedSelectionReceiver::new(
            &typed,
            &graph,
            SelectionProjection::Members(vec!["Player::missing".to_owned()]),
            SelectionCoverage::Complete,
            requirement(&typed, "Encode", None),
        )
        .expect_err("a member outside the schema cannot be projected");
        assert!(error.contains("not a member"), "{error}");
        let error = ScopedSelectionReceiver::new(
            &typed,
            &graph,
            SelectionProjection::Members(vec![
                "Player::health".to_owned(),
                "Player::health".to_owned(),
            ]),
            SelectionCoverage::Complete,
            requirement(&typed, "Encode", None),
        )
        .expect_err("a duplicated projected member rejects");
        assert!(error.contains("twice"), "{error}");
    }

    #[test]
    fn sum_cases_and_payloads_select_operations() {
        let typed = typed(SUM_PROGRAM);
        let graph = graph(&typed, "Outcome");
        let mut receiver = receiver(&typed, &graph, "Encode");
        let keys: Vec<_> = receiver.members().collect();
        assert_eq!(
            keys.iter()
                .map(|key| key.member_identity())
                .collect::<Vec<_>>(),
            vec!["Outcome::Ready", "Outcome::Ready::value", "Outcome::Empty"],
            "cases and their payloads are the declared members"
        );
        receiver
            .select(&keys[0], conformance_choice(&typed, "OutcomeEnc"))
            .expect("a case's carrier is the enclosing sum");
        receiver
            .select(&keys[1], conformance_choice(&typed, "HealthEnc"))
            .expect("the payload member's carrier is its own type");
        receiver
            .select(&keys[2], conformance_choice(&typed, "OutcomeEnc"))
            .expect("a nullary case selects like any case");
        let snapshot = receiver.freeze().expect("freezes");
        replay_selection_snapshot(&typed, &snapshot).expect("replays");
    }

    #[test]
    fn receiver_is_storage_independent_of_the_graph() {
        let typed = typed(ENCODE_PROGRAM);
        // The receiver and its frozen snapshot carry owned data only: the
        // graph is dropped before any selection is recorded.
        let mut receiver = {
            let graph = graph(&typed, "Player");
            receiver(&typed, &graph, "Encode")
        };
        receiver
            .select(
                &receiver.member("health").expect("key"),
                machine_choice(&typed, "Health::encode"),
            )
            .expect("selects");
        receiver
            .select(
                &receiver.member("speed").expect("key"),
                machine_choice(&typed, "Speed::encode"),
            )
            .expect("selects");
        let snapshot = receiver.freeze().expect("freezes");
        replay_selection_snapshot(&typed, &snapshot).expect("replays without the graph");
    }

    #[test]
    fn empty_subject_records_nothing() {
        let typed = typed("trait Encode { machine encode(&self) -> u64; } data Empty {}");
        let graph = graph(&typed, "Empty");
        let receiver = receiver(&typed, &graph, "Encode");
        assert_eq!(receiver.members().count(), 0);
        let snapshot = receiver.freeze().expect("an empty scope freezes");
        replay_selection_snapshot(&typed, &snapshot).expect("replays");
    }

    #[test]
    fn foreign_scope_snapshot_replays_under_its_claim() {
        let typed = typed(
            "trait Encode { machine encode(&self) -> u64; } \
             data Health { v: u32; } \
             machine Health::encode(&self) -> u64 satisfies Encode::encode { 0 } \
             HealthEnc: Health satisfies Encode { Encode::encode = Health::encode; } \
             pub data Open { health: Health; } \
             data Private { health: Health; }",
        );
        let open_graph =
            construct_semantic_schema_graph(&typed, subject(&typed, "Open"), foreign())
                .expect("a public subject admits a foreign scope");
        let mut receiver = ScopedSelectionReceiver::new(
            &typed,
            &open_graph,
            SelectionProjection::DeclaredMembers,
            SelectionCoverage::Complete,
            requirement(&typed, "Encode", None),
        )
        .expect("receiver opens");
        receiver
            .select(
                &receiver.member("health").expect("key"),
                conformance_choice(&typed, "HealthEnc"),
            )
            .expect("selects");
        let snapshot = receiver.freeze().expect("freezes");
        replay_selection_snapshot(&typed, &snapshot).expect("replays");

        // The frozen authority is a claim, not a grant: replaying the same
        // snapshot under an owning claim over the same public subject
        // re-checks cleanly, while a foreign claim over a private subject
        // cannot be manufactured past replay.
        let mut forged = snapshot.clone();
        forged.authority = owning();
        reseal(&mut forged);
        replay_selection_snapshot(&typed, &forged)
            .expect("the same public subject satisfies an owning claim");

        let private_graph = graph(&typed, "Private");
        let mut receiver = ScopedSelectionReceiver::new(
            &typed,
            &private_graph,
            SelectionProjection::DeclaredMembers,
            SelectionCoverage::Complete,
            requirement(&typed, "Encode", None),
        )
        .expect("receiver opens");
        receiver
            .select(
                &receiver.member("health").expect("key"),
                conformance_choice(&typed, "HealthEnc"),
            )
            .expect("selects");
        let mut snapshot = receiver.freeze().expect("freezes");
        snapshot.authority = foreign();
        reseal(&mut snapshot);
        let error = replay_selection_snapshot(&typed, &snapshot)
            .expect_err("a foreign claim over a private subject must reject");
        assert!(error.contains("non-public"), "{error}");
    }

    #[test]
    fn replay_rejects_forged_member_identity() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let mut receiver = receiver(&typed, &graph, "Encode");
        receiver
            .select(
                &receiver.member("health").expect("key"),
                machine_choice(&typed, "Health::encode"),
            )
            .expect("selects");
        receiver
            .select(
                &receiver.member("speed").expect("key"),
                machine_choice(&typed, "Speed::encode"),
            )
            .expect("selects");
        let mut snapshot = receiver.freeze().expect("freezes");
        snapshot.records[0].member_identity = "Player::stamina".to_owned();
        reseal(&mut snapshot);
        let error = replay_selection_snapshot(&typed, &snapshot)
            .expect_err("a forged member identity must reject");
        assert!(error.contains("does not match the bound member"), "{error}");
    }

    #[test]
    fn replay_rejects_stale_qualified_type() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let mut receiver = receiver(&typed, &graph, "Encode");
        receiver
            .select(
                &receiver.member("health").expect("key"),
                machine_choice(&typed, "Health::encode"),
            )
            .expect("selects");
        receiver
            .select(
                &receiver.member("speed").expect("key"),
                machine_choice(&typed, "Speed::encode"),
            )
            .expect("selects");
        let mut snapshot = receiver.freeze().expect("freezes");
        snapshot.records[0].qualified_type = Some("stale".to_owned());
        reseal(&mut snapshot);
        let error = replay_selection_snapshot(&typed, &snapshot)
            .expect_err("a stale key/type pair must reject");
        assert!(error.contains("does not match the bound member"), "{error}");
    }

    #[test]
    fn replay_rejects_dropped_and_duplicated_records() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let mut receiver = receiver(&typed, &graph, "Encode");
        receiver
            .select(
                &receiver.member("health").expect("key"),
                machine_choice(&typed, "Health::encode"),
            )
            .expect("selects");
        receiver
            .select(
                &receiver.member("speed").expect("key"),
                machine_choice(&typed, "Speed::encode"),
            )
            .expect("selects");
        let mut dropped = receiver_dummy(&typed, &graph);
        dropped.records.pop();
        reseal(&mut dropped);
        let error =
            replay_selection_snapshot(&typed, &dropped).expect_err("a dropped record must reject");
        assert!(error.contains("records 1 members"), "{error}");

        let mut duplicated = receiver_dummy(&typed, &graph);
        let extra = duplicated.records[0].clone();
        duplicated.records.push(extra);
        reseal(&mut duplicated);
        let error = replay_selection_snapshot(&typed, &duplicated)
            .expect_err("a duplicated record must reject");
        assert!(error.contains("records 3 members"), "{error}");
    }

    fn receiver_dummy(typed: &TypedTrees, graph: &SemanticSchemaGraph) -> SelectionSnapshot {
        let mut receiver = receiver(typed, graph, "Encode");
        receiver
            .select(
                &receiver.member("health").expect("key"),
                machine_choice(typed, "Health::encode"),
            )
            .expect("selects");
        receiver
            .select(
                &receiver.member("speed").expect("key"),
                machine_choice(typed, "Speed::encode"),
            )
            .expect("selects");
        receiver.freeze().expect("freezes")
    }

    #[test]
    fn replay_rejects_forged_choice_identities() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");

        // An identity that resolves nowhere.
        let mut snapshot = receiver_dummy(&typed, &graph);
        snapshot.records[0].choice = SelectionChoice::Machine {
            machine_identity: "No::such_machine".to_owned(),
        };
        reseal(&mut snapshot);
        let error = replay_selection_snapshot(&typed, &snapshot)
            .expect_err("an unresolvable choice must reject");
        assert!(error.contains("does not resolve"), "{error}");

        // An `Encode` conformance on `Speed` cannot cover `health: Health`.
        let mut snapshot = receiver_dummy(&typed, &graph);
        snapshot.records[0].choice = conformance_choice(&typed, "SpeedEnc");
        reseal(&mut snapshot);
        let error = replay_selection_snapshot(&typed, &snapshot)
            .expect_err("a conformance on the wrong carrier must reject");
        assert!(error.contains("does not cover"), "{error}");

        // A `Hash` machine cannot satisfy the frozen `Encode` requirement.
        let mut snapshot = receiver_dummy(&typed, &graph);
        snapshot.records[0].choice = machine_choice(&typed, "Health::hash");
        reseal(&mut snapshot);
        let error = replay_selection_snapshot(&typed, &snapshot)
            .expect_err("a machine outside the required contract must reject");
        assert!(error.contains("does not realize"), "{error}");
    }

    #[test]
    fn replay_rejects_forged_requirement() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let mut snapshot = receiver_dummy(&typed, &graph);
        snapshot.requirement = requirement(&typed, "Hash", None);
        reseal(&mut snapshot);
        let error = replay_selection_snapshot(&typed, &snapshot)
            .expect_err("choices that realize only `Encode` cannot satisfy a forged `Hash` claim");
        assert!(error.contains("does not realize"), "{error}");
    }

    #[test]
    fn replay_rejects_projection_drift() {
        let typed = typed(
            "trait Encode { machine encode(&self) -> u64; } \
             machine Rec::encode(&self) -> u64 satisfies Encode::encode { 0 } \
             RecEnc: Rec satisfies Encode { Encode::encode = Rec::encode; } \
             data Rec { secret [erased]: i32; tag: u8; }",
        );
        let graph = graph(&typed, "Rec");
        let mut receiver = ScopedSelectionReceiver::new(
            &typed,
            &graph,
            SelectionProjection::RuntimeMembers,
            SelectionCoverage::Partial,
            requirement(&typed, "Encode", None),
        )
        .expect("receiver opens");
        receiver
            .exclude(&receiver.member("tag").expect("key"))
            .expect("exclusion records");
        let mut snapshot = receiver.freeze().expect("freezes");
        // Widening the frozen projection reintroduces the erased member the
        // runtime scope deliberately excludes — replay re-derives the scope
        // and rejects the now-uncovered member.
        snapshot.projection = SelectionProjection::DeclaredMembers;
        reseal(&mut snapshot);
        let error = replay_selection_snapshot(&typed, &snapshot)
            .expect_err("a widened projection must reject");
        assert!(
            error.contains("records 1 members but its projection covers 2"),
            "{error}"
        );
    }

    #[test]
    fn replay_rejects_unresolvable_subject() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let mut snapshot = receiver_dummy(&typed, &graph);
        snapshot.subject_owner_identity = "Forged::Owner".to_owned();
        reseal(&mut snapshot);
        let error = replay_selection_snapshot(&typed, &snapshot)
            .expect_err("an unbound subject must reject");
        assert!(error.contains("does not resolve"), "{error}");
    }

    #[test]
    fn replay_rejects_tampered_revision() {
        let typed = typed(ENCODE_PROGRAM);
        let graph = graph(&typed, "Player");
        let mut snapshot = receiver_dummy(&typed, &graph);
        snapshot.revision = snapshot.revision.wrapping_add(1);
        let error = replay_selection_snapshot(&typed, &snapshot)
            .expect_err("a tampered report fingerprint must reject");
        assert!(error.contains("fingerprint"), "{error}");
    }
}
