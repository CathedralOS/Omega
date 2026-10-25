//! Reference-slot relations keep binding identity, backing, and authority separate.
//! Unknown calls keep their outstanding relation obligations even after later stores.

use super::*;

/// One exact destination leaf after an ordinary call. All sources must remain
/// live while this leaf is used. Incoming means the call may leave its prior
/// contents intact; an empty new-source set is not permission to drop them.
#[derive(Clone)]
pub struct StoredReferenceEffect {
    /// Static alternatives carry no local loan but remain value alternatives.
    pub has_static_values: bool,
    pub destination: LocalWriteOrigin,
    pub sources: Vec<StoredReferenceSource>,
    pub retains_incoming: bool,
    /// A runtime-selected destination may leave other physical slots intact.
    /// Unlike a conditional write to one exact slot, it cannot replace the
    /// destination's complete current descriptor occurrence.
    pub may_select: bool,
    pub access: language_semantics::ReferenceAccess,
}

/// The actual descriptor home and its backing are distinct coordinates.
/// Both are required: backing alone cannot authorize an exclusive transfer.
#[derive(Clone)]
pub struct StoredReferenceSource {
    /// Keeps this source alive without identifying the output's referent.
    pub lifetime_access: Option<language_semantics::ReferenceAccess>,
    pub carrier_symbol: SymbolHandle,
    pub carrier_segments: Vec<PlaceSegment>,
    pub backing: LocalWriteSource,
}

#[derive(Clone)]
pub(in crate::machine_calls::calls::write_frames) struct SlotEffect {
    pub(in crate::machine_calls::calls::write_frames) root: SymbolHandle,
    pub(in crate::machine_calls::calls::write_frames) segments: Vec<PlaceSegment>,
    /// None retains unknown contents; Some([]) is not an unknown write.
    pub(in crate::machine_calls::calls::write_frames) sources: Option<Vec<SlotSource>>,
    pub(in crate::machine_calls::calls::write_frames) incoming: bool,
    pub(in crate::machine_calls::calls::write_frames) may_select: bool,
    pub(in crate::machine_calls::calls::write_frames) kind: stored_origins::StoredOriginKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::machine_calls::calls::write_frames) struct SlotSource {
    pub(in crate::machine_calls::calls::write_frames) origin: FramePlaceOrigin,
    pub(in crate::machine_calls::calls::write_frames) carrier: FrameSourcePlace,
    pub(in crate::machine_calls::calls::write_frames) lifetime_access:
        Option<language_semantics::ReferenceAccess>,
}

#[derive(Clone)]
pub(in crate::machine_calls::calls::write_frames) struct SlotInventory {
    pub(in crate::machine_calls::calls::write_frames) origin: StoredWriteOrigin,
    pub(in crate::machine_calls::calls::write_frames) access: language_semantics::ReferenceAccess,
    /// Each crossed pointer still denotes its incoming descriptor occurrence.
    /// A final spelling cannot recover a destination captured before rebinding.
    pub(in crate::machine_calls::calls::write_frames) ancestors: Vec<Vec<PlaceSegment>>,
}

/// A failed invocation remains an obligation even if a later call overwrites
/// every affected slot. Declaration dependence is recorded separately.
#[derive(Clone)]
pub struct StoredReferenceCallFailure {
    pub target: SymbolHandle,
    pub requires_relation: bool,
}

#[derive(Clone, Default)]
pub(in crate::machine_calls::calls::write_frames) struct SlotRelationTransfer {
    pub effects: Vec<SlotEffect>,
    pub template_dependent: bool,
    pub incomplete_calls: Vec<StoredReferenceCallFailure>,
}

impl SlotRelationTransfer {
    pub fn complete(effects: Vec<SlotEffect>) -> Self {
        Self {
            effects,
            ..Self::default()
        }
    }

    pub fn is_complete(&self) -> bool {
        !self.template_dependent && self.incomplete_calls.is_empty()
    }

    /// Unknown reach cannot preserve any possibly aliased incoming origin.
    /// Preserve typed opaque regions as opaque, never invent reference access.
    pub fn unknown(
        context: &CollectedStatementPrefix,
        target: SymbolHandle,
        template_dependent: bool,
        requires_relation: bool,
    ) -> Self {
        let mut effects = Vec::new();
        for record in &context.stored {
            for leaf in &record.references {
                if effects.iter().any(|prior: &SlotEffect| {
                    prior.root == record.local_symbol
                        && prior.segments == leaf.local_segments
                        && prior.kind == leaf.kind
                        && prior.may_select == leaf.may_select
                }) {
                    continue;
                }
                effects.push(SlotEffect {
                    root: record.local_symbol,
                    segments: leaf.local_segments.clone(),
                    sources: None,
                    incoming: false,
                    may_select: leaf.may_select,
                    kind: leaf.kind,
                });
            }
        }
        Self {
            effects,
            template_dependent,
            incomplete_calls: if template_dependent {
                Vec::new()
            } else {
                vec![StoredReferenceCallFailure {
                    target,
                    requires_relation,
                }]
            },
        }
    }
}

pub(in crate::machine_calls::calls::write_frames) struct DestinationInventory {
    pub(in crate::machine_calls::calls::write_frames) slots: Vec<SlotInventory>,
    pub(in crate::machine_calls::calls::write_frames) template_dependent: bool,
}
