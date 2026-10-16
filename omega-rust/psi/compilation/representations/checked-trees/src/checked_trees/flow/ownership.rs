use arena::HandleSpan;
use symbols::SymbolHandle;

mod computation_custody;

/// Source occurrence of a storage movement, before permission claims expand it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowStorageTransferSource {
    Statement {
        statement_index: usize,
    },
    Call {
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
    },
}

impl Default for FlowStorageTransferSource {
    fn default() -> Self {
        Self::Statement { statement_index: 0 }
    }
}

/// One formal-directed movement candidate in the source evaluation schedule.
/// Permission production and borrow checking consume the same occurrence.
/// Claim expansion can split a carrier into several debts or omit an inactive
/// payload; neither changes the storage whose movement excludes a live loan.
/// Selected-arm candidates retain their route and are never unconditional
/// permission events. Reference-carrier transfers retain their selected type
/// so consumers do not mistake them for movement of the referent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowStorageTransferFact {
    pub source: FlowStorageTransferSource,
    pub evaluation: FlowPermissionEvaluation,
    pub expression: typed_trees::expression::ExpressionHandle,
    pub value_type: typed_trees::types::TypeReferenceHandle,
    pub source_arm: arena::Handle<typed_trees::expression::TableMatchArm>,
    pub root: facts::PlaceRoot,
    pub segments: HandleSpan<facts::PlaceSegment>,
}

/// The claim minted for one exact loan activation. Equal backing paths can
/// occur more than once in a statement; they cannot recover this association.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowBorrowClaimFact {
    pub loan: arena::Handle<crate::BorrowLoanFact>,
    pub claim_identity: language_semantics::PermissionClaimIdentity,
}

/// Caller-owned storage returned by one normal exclusive invocation. Claims
/// are the Restore events at this call beneath this exact region; the region
/// is retained separately because an empty or inactive case owns no payload.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowLoanCompletionFact {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub source: language_semantics::PermissionEventSource,
    pub root: facts::PlaceRoot,
    pub segments: HandleSpan<facts::PlaceSegment>,
    pub multiplicity: language_semantics::Multiplicity,
}

/// Exact evaluation lane within a source statement. Guards precede the
/// custody fork; target operands execute only on their selected edge.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum FlowPermissionEvaluation {
    #[default]
    Statement,
    TransitionGuard,
    TransitionTarget(typed_trees::statement::TransitionTargetHandle),
    /// A malformed source occurrence cannot acquire unconditional custody.
    Unknown,
}

/// The permission/resource algebra established by the multiplicity checker.
/// This is the authoritative claim ledger for later checked-IR consumers.
/// Storage-transfer candidates are retained separately: claim expansion and
/// inactive-payload filtering cannot describe every movement of borrowed storage.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct FlowPermissionEventFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub source: language_semantics::PermissionEventSource,
    pub evaluation: FlowPermissionEvaluation,
    /// Exact consuming operand retained by move discovery. Equal source places
    /// at different occurrences can carry different claims after restoration.
    /// Zero means this event has no operand (ingress, establishment or disposal),
    /// never a wildcard for a later consumer.
    pub expression: typed_trees::expression::ExpressionHandle,
    pub kind: language_semantics::PermissionEventKind,
    pub multiplicity: language_semantics::Multiplicity,
    pub access: language_semantics::PermissionAccess,
    pub claim_identity: language_semantics::PermissionClaimIdentity,
    pub provenance: language_semantics::PermissionProvenance,
    pub root: facts::PlaceRoot,
    pub segments: HandleSpan<facts::PlaceSegment>,
    /// Inactive sum alternatives establish/transfer a carrier while carrying
    /// no payload debt. Keep the event and record whether an obligation existed.
    pub obligation_live: bool,
}

/// One owned expression destination whose incoming transfers follow its retained
/// structural-value selection topology. These transfers are conditional and
/// must never be replayed as the unconditional permission stream.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowOwnedSelectionReceipt {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
    pub expression: typed_trees::expression::ExpressionHandle,
    /// Zero denotes the enclosing state's anonymous return, identified by the
    /// exact expression/statement and return type. Otherwise this names the
    /// authored local receiving the selected owner.
    pub destination: SymbolHandle,
    pub type_reference: typed_trees::types::TypeReferenceHandle,
    pub transfers: HandleSpan<FlowOwnedSelectionTransfer>,
    /// Distinct incoming owners, in reverse declaration order. This is a
    /// candidate roster, not a committed transfer or unconditional drop list.
    /// At the death edge, dispose precisely its complement under the selected
    /// transfer, after disposing the newer destination when it remains owned.
    pub sources: HandleSpan<FlowOwnedSelectionSource>,
    pub death: language_semantics::PermissionEventSource,
}

/// An exact established source, or an earlier selected destination whose
/// incoming receipt preserves its alternative origins without copying them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowOwnedSelectionSource {
    pub symbol: SymbolHandle,
    pub statement_ordinal: u32,
    pub provenance: language_semantics::PermissionProvenance,
    pub claim_identity: language_semantics::PermissionClaimIdentity,
    /// Zero means the source has the direct provenance above. A nonzero handle
    /// is authoritative for the source's alternative origins; Unknown alone
    /// never authorizes a transfer.
    pub origin_selection: arena::Handle<FlowOwnedSelectionReceipt>,
}

/// One selected place occurrence. Nested arm scopes are reconstructed from the
/// existing authored/structural selection graph, not a product of path states.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowOwnedSelectionTransfer {
    pub expression: typed_trees::expression::ExpressionHandle,
    pub source_arm: arena::Handle<typed_trees::expression::TableMatchArm>,
    pub source: arena::Handle<FlowOwnedSelectionSource>,
    /// Exact moved path below the source root. Empty selects the whole local;
    /// a nonempty path (fields and literal fixed indexes only) selects one
    /// affine child while the root's residual custody dies on the same edge.
    pub path: HandleSpan<facts::PlaceSegment>,
    /// The exact linear claims this leaf consumes, each named by its full path
    /// below the source root in canonical frontier order. A plain-owned leaf
    /// carries none; a linear leaf names its one claim; a whole affine carrier
    /// names its complete claim frontier, so the join discharges every linear
    /// child instead of inventing an aggregate root claim.
    pub claims: HandleSpan<FlowOwnedSelectionClaim>,
}

/// One consumed linear claim inside a selection transfer. `path` is the
/// claim's exact source-root-relative place (the transfer's `path` plus the
/// claim's position below the leaf); `claim_identity`/`provenance` are the
/// consumed claim's own evidence, so the destination's claims and the outcome
/// map can name the claim's original authority without re-reading the source
/// place ledger.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowOwnedSelectionClaim {
    pub path: HandleSpan<facts::PlaceSegment>,
    pub claim_identity: language_semantics::PermissionClaimIdentity,
    pub provenance: language_semantics::PermissionProvenance,
}

impl super::FlowOwnershipFacts {
    /// Candidate cleanup owners in first-occurrence order, not an executable
    /// drop stream. Permission production records each selected exit separately;
    /// planning and lowering must agree that equal rows on distinct exits name
    /// one owner, while two rows on the same exit remain a duplicate disposal.
    /// Selected-edge plans still decide which owners die and in what order.
    /// The original permission stream retains every outgoing coordinate.
    pub fn state_exit_cleanup_roots(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Option<Vec<&FlowPermissionEventFact>> {
        let mut roots = Vec::<&FlowPermissionEventFact>::new();
        let mut occurrences = Vec::new();
        for (_, event) in self.permissions.iter().filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.source == language_semantics::PermissionEventSource::StateExit
                && event.kind == language_semantics::PermissionEventKind::AffineDrop
        }) {
            let occurrence = (event.root, event.segments, event.evaluation);
            if occurrences.contains(&occurrence) {
                return None;
            }
            occurrences.push(occurrence);
            if let Some(prior) = roots
                .iter()
                .find(|prior| prior.root == event.root && prior.segments == event.segments)
            {
                let mut same_exit = (**prior).clone();
                same_exit.evaluation = event.evaluation;
                if same_exit != *event {
                    return None;
                }
            } else {
                roots.push(event);
            }
        }
        Some(roots)
    }

    pub fn owned_selection_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
    ) -> Option<(
        arena::Handle<FlowOwnedSelectionReceipt>,
        &FlowOwnedSelectionReceipt,
    )> {
        let mut matches = self.owned_selections.iter().filter(|(_, receipt)| {
            receipt.state == state && receipt.statement_ordinal == statement_ordinal
        });
        let receipt = matches.next()?;
        matches.next().is_none().then_some(receipt)
    }
}

/// One normalized source for a claim transferred through a checked state's
/// result. Inputs are relative to the callee's declared parameter frontier;
/// claims established by the checked body retain their exact semantic
/// identity and root-lineage provenance.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowClaimOutcomeSource {
    #[default]
    Unknown,
    Input {
        parameter_symbol: SymbolHandle,
        segments: HandleSpan<facts::PlaceSegment>,
    },
    Established {
        claim_identity: language_semantics::PermissionClaimIdentity,
        provenance: language_semantics::PermissionProvenance,
    },
}

/// One output-path entry in a checked state's normalized claim outcome map.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowClaimOutcomeEntryFact {
    pub output_segments: HandleSpan<facts::PlaceSegment>,
    pub source: FlowClaimOutcomeSource,
}

/// Path-indexed live-claim mapping published by one checked state result.
/// Statically inactive case alternatives are intentionally absent. Absence of
/// the map means the state has no live linear result frontier or could not
/// prove one unique mapping for every possibly-live output claim.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowClaimOutcomeMapFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub entries: HandleSpan<FlowClaimOutcomeEntryFact>,
}

/// Exact source control edge; the two targets of one statement are distinct.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowClaimJoinExitKind {
    #[default]
    FinalExpression,
    TransitionTarget,
    TransitionContinuation,
    /// Completion of a nested call whose result supplies the enclosing exit.
    CallResult {
        call_ordinal: usize,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowClaimJoinExit {
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub kind: FlowClaimJoinExitKind,
}

/// One complete normal-exit alternative and its exact argument substitution.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowClaimJoinAlternative {
    pub exits: HandleSpan<FlowClaimJoinExit>,
    pub source: FlowClaimJoinAlternativeSource,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FlowClaimJoinAlternativeSource {
    /// The exact returned case excludes this path, not an unknown origin.
    #[default]
    Inactive,
    /// This exit requires an input payload proved absent at this invocation.
    /// Preserve the callee origin and the exact caller establishment; absence
    /// is not a claim and grants no fresh resource authority.
    ExcludedInput {
        source: FlowClaimOutcomeSource,
        root: facts::PlaceRoot,
        segments: HandleSpan<facts::PlaceSegment>,
        established_at: usize,
    },
    Claim {
        source: FlowClaimOutcomeSource,
        bound_source: FlowClaimOutcomeSource,
    },
}

/// Conditional returned custody for one call result path. The local claim id
/// names the returned occurrence, not new authority. No alternative is an
/// unconditional identity/content theorem.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowClaimJoinReceipt {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target_symbol: SymbolHandle,
    pub expression: typed_trees::expression::ExpressionHandle,
    pub result_root: facts::PlaceRoot,
    pub result_segments: HandleSpan<facts::PlaceSegment>,
    pub claim_identity: language_semantics::PermissionClaimIdentity,
    pub alternatives: HandleSpan<FlowClaimJoinAlternative>,
}

/// A checked instruction event settles this exact owned argument. The normal
/// permission event retains claim identity and provenance; this receipt binds
/// its authority and source proposition to the event that actually occurred.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowInstructionDispositionFact {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub source: language_semantics::PermissionEventSource,
    pub evaluation: FlowPermissionEvaluation,
    pub requirement: SymbolHandle,
    pub fact: arena::Handle<typed_trees::domain::ProofFact>,
    pub argument: typed_trees::expression::ExpressionHandle,
    pub event: facts::FactHandle,
    pub evidence: arena::Handle<FlowInstructionEffectReceiptFact>,
    pub root: facts::PlaceRoot,
    pub segments: HandleSpan<facts::PlaceSegment>,
}

/// Fresh instruction evidence retained through an exact helper invocation.
/// Children cover normal callee exits; leaves name actual catalog events.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowInstructionEffectReceiptFact {
    pub event: facts::FactHandle,
    /// A helper's exact instantiated postcondition; zero for catalog leaves.
    /// Exact authored normal-return clause, joined through the event call's ensures rows.
    pub contract: arena::Handle<crate::ContractProofFact>,
    pub observation: facts::ProgramPoint,
    pub children: HandleSpan<FlowInstructionEffectReceiptFact>,
}
