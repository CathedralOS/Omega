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
//!
//! This file owns the selection vocabulary. `scoped_members.rs` projects
//! scoped members and their keys, `requirement_resolution.rs` resolves
//! requirements against conformances and machines,
//! `selection_receiver.rs` receives scoped selections, `snapshots.rs`
//! replays selection snapshots and `tests.rs` holds the selection tests.

mod requirement_resolution;
mod scoped_members;
mod selection_receiver;
mod snapshots;
#[cfg(test)]
mod tests;

pub use scoped_members::{MemberKind, MemberSelectionKey};
pub(crate) use scoped_members::{ScopedMember, expected_members, project};
pub use selection_receiver::ScopedSelectionReceiver;
pub use snapshots::{SelectionSnapshot, replay_selection_snapshot};

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::types::TypeReferenceHandle;

use super::schema_graph::exact_symbol_identity;

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

/// The exact trait application a selection requirement demands: canonical
/// type-argument identities plus the lifetime ordinals under
/// first-occurrence normalization. A requirement may demand only the trait
/// declaration (`None`), or pin the application itself — the distinction
/// between "any encoder for this member" and "the `Encode<Json>` encoder".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemandedTraitApplication {
    pub type_argument_identities: Vec<String>,
    pub lifetime_arguments: Vec<u32>,
}

/// The required contract every recorded selection must refine.
///
/// `trait_identity` names the exact contract family — the canonical identity
/// of the trait declaration. `requirement_identity` optionally pins one
/// requirement signature inside that trait; `None` accepts a realization of
/// any requirement row. `trait_application` optionally pins the exact trait
/// application — `Encode<Json>` rather than `Encode<Cbor>` — and replay
/// re-checks that the selected conformance or satisfies edge carries it.
/// Replay resolves all three against the bound program and re-checks each
/// choice's refinement, not merely that a name was chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRequirement {
    pub trait_identity: String,
    pub requirement_identity: Option<String>,
    pub trait_application: Option<DemandedTraitApplication>,
}

/// Renumber a lifetime ordinal list by first occurrence so two telescopes
/// compare under binder renaming — the convention `TraitConformance` and
/// `Conformance` edge identity already applies.
pub(crate) fn first_occurrence_normalized(ordinals: &[u32]) -> Vec<u32> {
    let mut seen: Vec<u32> = Vec::with_capacity(ordinals.len());
    ordinals
        .iter()
        .map(
            |ordinal| match seen.iter().position(|known| known == ordinal) {
                Some(position) => position as u32,
                None => {
                    seen.push(*ordinal);
                    (seen.len() - 1) as u32
                }
            },
        )
        .collect()
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
            trait_application: None,
        })
    }

    /// Pin one exact trait application inside the requirement: the type
    /// arguments canonicalize through the package-qualified identity and the
    /// lifetime arguments normalize by first occurrence, so the demanded
    /// application survives binder renaming on both sides of a package
    /// boundary. Arity is checked against the trait's declared parameter
    /// lists — a mismatched application is a construction error here, not a
    /// later selection rejection.
    pub fn for_trait_application(
        typed: &TypedTrees,
        trait_symbol: SymbolHandle,
        requirement: Option<SymbolHandle>,
        type_arguments: &[TypeReferenceHandle],
        lifetime_arguments: &[u32],
    ) -> Result<Self, String> {
        let mut requirement = Self::for_trait(typed, trait_symbol, requirement)?;
        let definition = typed
            .traits()
            .iter()
            .find(|definition| definition.symbol == trait_symbol)
            .expect("for_trait resolved this declaration");
        if type_arguments.len() != typed.trait_type_parameters(definition).len() {
            return Err(
                "a demanded trait application must supply every declared type parameter".to_owned(),
            );
        }
        if lifetime_arguments.len() != definition.lifetime_parameters.len() {
            return Err(
                "a demanded trait application must supply every declared lifetime parameter"
                    .to_owned(),
            );
        }
        let type_argument_identities = type_arguments
            .iter()
            .map(|argument| {
                typed
                    .package_qualified_type_identity(*argument)
                    .into_string()
            })
            .collect();
        requirement.trait_application = Some(DemandedTraitApplication {
            type_argument_identities,
            lifetime_arguments: first_occurrence_normalized(lifetime_arguments),
        });
        Ok(requirement)
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
