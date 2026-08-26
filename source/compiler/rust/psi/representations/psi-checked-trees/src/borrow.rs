use psi_arena::{Arena, Handle, HandleSpan};
use psi_symbols::SymbolHandle;

/// One exact structural place captured for compatibility checking.
///
/// Identity is the resolved root symbol plus ordered semantic path segments.
/// Source labels are deliberately absent. Runtime or otherwise unresolved
/// selectors may remain as expression handles, but cannot establish a positive
/// spatial result until a checked tactic understands them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapturedPlace {
    pub root_symbol: SymbolHandle,
    pub segments: Vec<psi_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CapturedPlaceContainment {
    #[default]
    None,
    Same,
    LeftContainsRight,
    RightContainsLeft,
}

/// Transient result of the one checked captured-place compatibility judgment.
///
/// These conclusions are deliberately independent: two places can be both
/// disjoint and non-interfering, while two shared reads can be non-interfering
/// even when one place contains the other. Access polarity is an input to the
/// judgment and is not retained here as proof authority.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapturedPlaceCompatibility {
    pub left: CapturedPlace,
    pub right: CapturedPlace,
    pub disjoint: bool,
    pub containment: CapturedPlaceContainment,
    pub non_interfering: bool,
}

/// The normalized conclusion retained from one automatic loan/loan
/// compatibility judgment.
///
/// The captured places live beside this conclusion on the certificate so the
/// conclusion cannot be transplanted onto a merely shape-compatible pair.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowCompatibilityConclusion {
    pub disjoint: bool,
    pub containment: CapturedPlaceContainment,
    pub non_interfering: bool,
}

/// Checked derivation class for automatic borrow compatibility.
///
/// `Structural` deliberately carries no premise handles: this certificate is
/// emitted only by the ordinary structural loan/loan judgment. It is not a
/// proposition proof and does not claim Terminal replay authority.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BorrowCompatibilityDerivation {
    #[default]
    Structural,
}

/// Exact source coordinate at which the second loan was formed while the
/// first loan remained active.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowCompatibilityFormation {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
}

/// Checked-only certificate for one automatically admitted loan/loan pair.
///
/// Both resource handles and frozen places are retained. Later checked-tree
/// review can therefore rejoin the row to the exact state-owned loan rows;
/// this row neither creates borrow authority nor changes admission semantics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedBorrowCompatibilityCertificate {
    pub formation: BorrowCompatibilityFormation,
    pub forming_loan: Handle<BorrowLoanFact>,
    pub active_loan: Handle<BorrowLoanFact>,
    pub forming_place: CapturedPlace,
    pub active_place: CapturedPlace,
    pub conclusion: BorrowCompatibilityConclusion,
    pub derivation: BorrowCompatibilityDerivation,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BorrowRootKind {
    #[default]
    OwnedData,
    LocalData,
    MutableParameter,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowWritableRootFact {
    pub symbol: SymbolHandle,
    pub kind: BorrowRootKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub writable_roots: HandleSpan<BorrowWritableRootFact>,
    pub mutable_parameter_count: usize,
    pub calls: HandleSpan<BorrowCallFact>,
    pub loans: HandleSpan<BorrowLoanFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BorrowAccessKind {
    #[default]
    Read,
    Mutable,
    WriteOnly,
}

impl BorrowAccessKind {
    pub fn is_exclusive(&self) -> bool {
        matches!(self, Self::Mutable | Self::WriteOnly)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowArgumentAccessFact {
    pub root_symbol: SymbolHandle,
    pub segments: HandleSpan<psi_facts::PlaceSegment>,
    pub kind: BorrowAccessKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowCallFact {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub has_receiver: bool,
    pub accesses: HandleSpan<BorrowArgumentAccessFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowLoanFact {
    pub statement_index: usize,
    pub last_use_statement_index: usize,
    pub owner_symbol: SymbolHandle,
    /// Projection within the owner that carries this loan. An empty path means
    /// the whole owner; dynamic indexes conservatively overlap every element.
    pub owner_path: HandleSpan<BorrowLoanOwnerSegment>,
    pub source_owner_symbol: SymbolHandle,
    /// Checked formation lineage for this loan occurrence.
    ///
    /// Only an explicit reference-local reborrow with one exact prior source
    /// occurrence names a parent. Aggregate/helper transfers and ambiguous
    /// source aliases remain deliberately unretained.
    pub lineage: BorrowLoanLineage,
    pub root_symbol: SymbolHandle,
    pub segments: HandleSpan<psi_facts::PlaceSegment>,
    pub kind: BorrowAccessKind,
}

/// Checked-only formation lineage for one loan occurrence.
///
/// This classification does not grant authority or change borrow admission.
/// In particular, `UnretainedDerived` is a fence rather than an inferred
/// parent relation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BorrowLoanLineage {
    #[default]
    DirectRoot,
    Reborrow {
        parent_loan: Handle<BorrowLoanFact>,
    },
    UnretainedDerived,
}

/// Exact state-invocation parent lifetime for one direct-root loan.
///
/// This checked-only identity does not create authority. It names the
/// state-owned root whose already-validated authority is temporarily
/// constrained by the loan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedDirectBorrowParentLifetime {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub root_symbol: SymbolHandle,
}

/// Restoration obligation retained for one direct-root loan.
///
/// The row records where checked flow ends the loan and must restore the
/// parent root's availability. It is not evidence that restoration occurred
/// and grants no compatibility authority.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedDirectBorrowRestorationObligation {
    pub parent: CheckedDirectBorrowParentLifetime,
    pub weakening_source: crate::FlowInvalidationSource,
    pub weakening_reason: crate::FlowBorrowWeakeningReason,
}

/// Checked-only resource closure for one state-local loan captured directly
/// from a root authority occurrence.
///
/// `lineage` must be `DirectRoot`. Compatibility certificates remain a
/// separate proof ledger and cannot manufacture one of these rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedDirectBorrowLoanResource {
    pub loan: Handle<BorrowLoanFact>,
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub owner_symbol: SymbolHandle,
    pub owner_path: Vec<BorrowLoanOwnerSegment>,
    pub captured_place: CapturedPlace,
    pub access: BorrowAccessKind,
    pub activation_source: crate::FlowInvalidationSource,
    pub weakening_source: crate::FlowInvalidationSource,
    pub weakening_reason: crate::FlowBorrowWeakeningReason,
    pub parent_lifetime: CheckedDirectBorrowParentLifetime,
    pub restoration: CheckedDirectBorrowRestorationObligation,
}

/// Typed parent-resource identity for one retained direct reborrow.
///
/// The handle points into one of the two checked-only resource arenas. It
/// records the immediate resource occurrence and is not authority to use or
/// reactivate that resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedParentBorrowResource {
    DirectRoot {
        resource: Handle<CheckedDirectBorrowLoanResource>,
    },
    Reborrow {
        resource: Handle<CheckedReborrowLoanResource>,
    },
}

impl Default for CheckedParentBorrowResource {
    fn default() -> Self {
        Self::DirectRoot {
            resource: Handle::invalid(),
        }
    }
}

/// Pending restoration obligation for one retained direct reborrow.
///
/// This row binds the child's weakening to its immediate parent occurrence.
/// It does not prove that the parent remained active, that the two lifetimes
/// are temporally contained, or that the parent was reactivated.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedReborrowRestorationObligation {
    pub child_loan: Handle<BorrowLoanFact>,
    pub parent_loan: Handle<BorrowLoanFact>,
    pub parent_resource: CheckedParentBorrowResource,
    pub child_weakening_source: crate::FlowInvalidationSource,
    pub child_weakening_reason: crate::FlowBorrowWeakeningReason,
}

/// Exact checked-flow boundary at which an explicit direct reborrow suspends
/// use through its immediate parent occurrence.
///
/// The parent constraint is the unique occurrence present immediately before
/// the child activation. This checked-only join says nothing about the parent
/// after formation: it is not interval containment, reactivation, completed
/// restoration, or Terminal authority.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedReborrowParentSuspensionBoundary {
    pub child_loan: Handle<BorrowLoanFact>,
    pub parent_loan: Handle<BorrowLoanFact>,
    pub parent_resource: CheckedParentBorrowResource,
    pub child_activation: Handle<crate::FlowBorrowActivationFact>,
    pub parent_entry_constraint: Handle<crate::FlowConstraintRef>,
    pub source: crate::FlowInvalidationSource,
}

/// Lexical disposition of the immediate parent carrier at the child's end.
///
/// This classification describes only source/flow liveness. In particular,
/// `LivePastChild` is not proof of authority reactivation, and a retired parent
/// does not imply that authority was returned, cascaded, or discarded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ParentLexicalStatusAtChildEnd {
    RetiredBeforeChild,
    #[default]
    RetiredWithChild,
    LivePastChild,
}

/// Exact checked weakening-order join for one retained direct reborrow.
///
/// The two handles identify the authoritative flow events; `status` is derived
/// from their semantic statement phases rather than arena insertion order.
/// This checked-only row grants no suspension-containment, restoration,
/// reactivation, cascade, or Terminal authority.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedReborrowParentEndStatus {
    pub child_loan: Handle<BorrowLoanFact>,
    pub parent_loan: Handle<BorrowLoanFact>,
    pub parent_resource: CheckedParentBorrowResource,
    pub child_weakening: Handle<crate::FlowBorrowWeakeningFact>,
    pub parent_weakening: Handle<crate::FlowBorrowWeakeningFact>,
    pub status: ParentLexicalStatusAtChildEnd,
}

/// Semantic phase used by checked-only borrow-resource lifecycle replay.
///
/// The ordering is source-defined rather than arena-defined: last-use expiry
/// precedes statement entry, reassignment retires the displaced carrier after
/// its right-hand side, replacement loans activate afterward, and state exit
/// is the final boundary. These phases do not grant authority by themselves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CheckedBorrowResourceLifecyclePhase {
    #[default]
    LastUseExpired,
    LocalReassigned,
    Activation,
    StateExit,
}

/// Final checked-only target of one reborrow disposition event.
///
/// A retained resource target names an exact loan occurrence. A direct-root
/// lifetime target names the state-local root beyond the last retired loan
/// carrier. Neither variant proves that authority was returned to the target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedBorrowResourceDispositionTarget {
    ParentResource(CheckedParentBorrowResource),
    DirectRootLifetime(CheckedDirectBorrowParentLifetime),
}

impl Default for CheckedBorrowResourceDispositionTarget {
    fn default() -> Self {
        Self::ParentResource(CheckedParentBorrowResource::default())
    }
}

/// Checked-only disposition selected when an available reborrow carrier ends.
///
/// These names classify the resource-ledger route required at the boundary;
/// they are not evidence of completed return, post-return use legality,
/// cleanup, or Terminal authority.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CheckedReborrowResourceDisposition {
    #[default]
    Reactivate,
    CascadeThroughRetiredParent,
    RetireOrDiscard,
}

/// One retired carrier traversed by a cascading disposition.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedRetiredParentResourceDispositionStep {
    pub resource: CheckedParentBorrowResource,
    pub weakening: Handle<crate::FlowBorrowWeakeningFact>,
}

/// One independently replayable checked-only reborrow disposition event.
///
/// `retired_parent_path` is ordered from the immediate retired parent toward
/// the final target. An empty path is required for direct reactivation. The
/// row preserves exact resource and flow handles but authorizes no use.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedReborrowResourceDispositionEvent {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub child_loan: Handle<BorrowLoanFact>,
    pub child_resource: Handle<CheckedReborrowLoanResource>,
    pub child_activation: Handle<crate::FlowBorrowActivationFact>,
    pub child_weakening: Handle<crate::FlowBorrowWeakeningFact>,
    pub parent_loan: Handle<BorrowLoanFact>,
    pub parent_resource: CheckedParentBorrowResource,
    pub boundary_source: crate::FlowInvalidationSource,
    pub boundary_phase: CheckedBorrowResourceLifecyclePhase,
    pub retired_parent_path: Vec<CheckedRetiredParentResourceDispositionStep>,
    pub final_target: CheckedBorrowResourceDispositionTarget,
    pub disposition: CheckedReborrowResourceDisposition,
}

/// Checked-only resource closure for one explicit direct reborrow.
///
/// The row retains the child's exact activation/weakening lifecycle and a
/// typed link to the immediate checked parent resource. It is a pending
/// restoration obligation only: aggregate transfers, temporal containment,
/// parent activity/reactivation, and Terminal authority remain outside this
/// representation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedReborrowLoanResource {
    pub loan: Handle<BorrowLoanFact>,
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub owner_symbol: SymbolHandle,
    pub owner_path: Vec<BorrowLoanOwnerSegment>,
    pub captured_place: CapturedPlace,
    pub access: BorrowAccessKind,
    pub activation_source: crate::FlowInvalidationSource,
    pub weakening_source: crate::FlowInvalidationSource,
    pub weakening_reason: crate::FlowBorrowWeakeningReason,
    pub parent_loan: Handle<BorrowLoanFact>,
    pub parent_resource: CheckedParentBorrowResource,
    pub parent_suspension: CheckedReborrowParentSuspensionBoundary,
    pub parent_end_status: CheckedReborrowParentEndStatus,
    pub restoration: CheckedReborrowRestorationObligation,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BorrowLoanOwnerSegment {
    Field(SymbolHandle),
    Case(SymbolHandle),
    FixedIndex(usize),
    #[default]
    DynamicIndex,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowFacts {
    pub writable_roots: Arena<BorrowWritableRootFact>,
    pub access_segments: Arena<psi_facts::PlaceSegment>,
    pub owner_segments: Arena<BorrowLoanOwnerSegment>,
    pub argument_accesses: Arena<BorrowArgumentAccessFact>,
    pub calls: Arena<BorrowCallFact>,
    pub loans: Arena<BorrowLoanFact>,
    pub states: Arena<StateBorrowFact>,
    /// Zero-premise structural certificates retained by checked borrow
    /// admission. This is a separate proof ledger from the loan resource rows.
    pub compatibility_certificates: Arena<CheckedBorrowCompatibilityCertificate>,
    /// Non-authorizing direct-root resource closures reconstructed from the
    /// exact loan activation/weakening ledger. Reborrow parent identity is
    /// retained separately.
    pub direct_loan_resources: Arena<CheckedDirectBorrowLoanResource>,
    /// Non-authorizing resource closures for the narrow explicit direct-
    /// reborrow lineage. Rows form a topological arena through typed parent
    /// resource handles. Aggregate and otherwise unretained transfers have no
    /// row.
    pub reborrow_loan_resources: Arena<CheckedReborrowLoanResource>,
    /// Checked-only resource-lifecycle dispositions for available direct
    /// reborrows at their weakening boundary. Suspended carriers remain
    /// pending and therefore have no premature disposition row.
    pub reborrow_disposition_events: Arena<CheckedReborrowResourceDispositionEvent>,
}

impl BorrowFacts {
    pub fn with_roots(
        writable_roots: Arena<BorrowWritableRootFact>,
        access_segments: Arena<psi_facts::PlaceSegment>,
        owner_segments: Arena<BorrowLoanOwnerSegment>,
        argument_accesses: Arena<BorrowArgumentAccessFact>,
        calls: Arena<BorrowCallFact>,
        loans: Arena<BorrowLoanFact>,
        states: Arena<StateBorrowFact>,
    ) -> Self {
        Self {
            writable_roots,
            access_segments,
            owner_segments,
            argument_accesses,
            calls,
            loans,
            states,
            compatibility_certificates: Arena::new(),
            direct_loan_resources: Arena::new(),
            reborrow_loan_resources: Arena::new(),
            reborrow_disposition_events: Arena::new(),
        }
    }

    /// Rejoins a retained compatibility row to the exact state-owned loan
    /// resources and their frozen places.
    pub fn compatibility_certificate_matches_resources(
        &self,
        certificate: &CheckedBorrowCompatibilityCertificate,
    ) -> bool {
        if certificate.forming_loan == certificate.active_loan
            || !self.loans.is_valid(certificate.forming_loan)
            || !self.loans.is_valid(certificate.active_loan)
            || !certificate.formation.machine_symbol.is_valid()
            || !certificate.formation.state_symbol.is_valid()
        {
            return false;
        }

        let Some(state) = self.states.iter().find_map(|(_, state)| {
            (state.machine_symbol == certificate.formation.machine_symbol
                && state.state_symbol == certificate.formation.state_symbol)
                .then_some(state)
        }) else {
            return false;
        };
        if !handle_span_contains(state.loans, certificate.forming_loan)
            || !handle_span_contains(state.loans, certificate.active_loan)
        {
            return false;
        }

        let forming_loan = self.loans.get(certificate.forming_loan);
        let active_loan = self.loans.get(certificate.active_loan);
        forming_loan.statement_index == certificate.formation.statement_index
            && self.certificate_place_matches_resource(
                state,
                certificate.forming_loan,
                forming_loan,
                &certificate.forming_place,
            )
            && self.certificate_place_matches_resource(
                state,
                certificate.active_loan,
                active_loan,
                &certificate.active_place,
            )
    }

    /// Returns the exact access polarities that an independently replayed
    /// compatibility certificate must consume.
    ///
    /// Direct-root and direct-reborrow loans use their joined checked resource
    /// rows. Deliberately unretained transfer loans remain on their established
    /// raw-loan route.
    pub fn compatibility_certificate_resource_accesses(
        &self,
        certificate: &CheckedBorrowCompatibilityCertificate,
    ) -> Option<(&BorrowAccessKind, &BorrowAccessKind)> {
        if !self.compatibility_certificate_matches_resources(certificate) {
            return None;
        }
        let forming_loan = self.loans.get(certificate.forming_loan);
        let active_loan = self.loans.get(certificate.active_loan);
        Some((
            self.certificate_loan_access(certificate.forming_loan, forming_loan)?,
            self.certificate_loan_access(certificate.active_loan, active_loan)?,
        ))
    }

    fn certificate_place_matches_resource(
        &self,
        state: &StateBorrowFact,
        handle: Handle<BorrowLoanFact>,
        loan: &BorrowLoanFact,
        place: &CapturedPlace,
    ) -> bool {
        match loan.lineage {
            BorrowLoanLineage::DirectRoot => {
                let mut matches = self
                    .direct_loan_resources
                    .iter()
                    .filter(|(_, resource)| resource.loan == handle);
                let Some((_, resource)) = matches.next() else {
                    return false;
                };
                matches.next().is_none()
                    && resource.machine_symbol == state.machine_symbol
                    && resource.state_symbol == state.state_symbol
                    && &resource.captured_place == place
            }
            BorrowLoanLineage::Reborrow { .. } => {
                let mut matches = self
                    .reborrow_loan_resources
                    .iter()
                    .filter(|(_, resource)| resource.loan == handle);
                let Some((_, resource)) = matches.next() else {
                    return false;
                };
                matches.next().is_none()
                    && resource.machine_symbol == state.machine_symbol
                    && resource.state_symbol == state.state_symbol
                    && &resource.captured_place == place
            }
            BorrowLoanLineage::UnretainedDerived => {
                place.root_symbol == loan.root_symbol && place.segments == self.loan_segments(loan)
            }
        }
    }

    fn certificate_loan_access<'a>(
        &'a self,
        handle: Handle<BorrowLoanFact>,
        loan: &'a BorrowLoanFact,
    ) -> Option<&'a BorrowAccessKind> {
        match loan.lineage {
            BorrowLoanLineage::DirectRoot => {
                let mut matches = self
                    .direct_loan_resources
                    .iter()
                    .filter(|(_, resource)| resource.loan == handle);
                let access = &matches.next()?.1.access;
                matches.next().is_none().then_some(access)
            }
            BorrowLoanLineage::Reborrow { .. } => {
                let mut matches = self
                    .reborrow_loan_resources
                    .iter()
                    .filter(|(_, resource)| resource.loan == handle);
                let access = &matches.next()?.1.access;
                matches.next().is_none().then_some(access)
            }
            BorrowLoanLineage::UnretainedDerived => Some(&loan.kind),
        }
    }

    pub fn access_segments(&self, access: &BorrowArgumentAccessFact) -> &[psi_facts::PlaceSegment] {
        self.access_segments.span_or_empty(access.segments)
    }

    pub fn loan_segments(&self, loan: &BorrowLoanFact) -> &[psi_facts::PlaceSegment] {
        self.access_segments.span_or_empty(loan.segments)
    }

    pub fn loan_owner_path(&self, loan: &BorrowLoanFact) -> &[BorrowLoanOwnerSegment] {
        self.owner_segments.span_or_empty(loan.owner_path)
    }

    pub fn state_owns_loan(&self, state: &StateBorrowFact, loan: Handle<BorrowLoanFact>) -> bool {
        self.loans.is_valid(loan) && handle_span_contains(state.loans, loan)
    }
}

fn handle_span_contains<T>(span: HandleSpan<T>, handle: Handle<T>) -> bool {
    handle.is_valid()
        && !span.is_empty()
        && handle.generation() == span.start().generation()
        && handle.arena_index() >= span.start().arena_index()
        && handle.arena_index()
            < span
                .start()
                .arena_index()
                .checked_add(span.count())
                .unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use crate::{
        BorrowArgumentAccessFact, BorrowCallFact, BorrowFacts, BorrowLoanFact,
        BorrowLoanOwnerSegment, BorrowWritableRootFact, CapturedPlace, StateBorrowFact,
    };
    use psi_arena::Arena;

    #[test]
    fn borrow_facts_constructor_keeps_borrow_roots_explicit() {
        let writable_roots = Arena::<BorrowWritableRootFact>::with_capacity(1);
        let access_segments = Arena::<psi_facts::PlaceSegment>::with_capacity(2);
        let owner_segments = Arena::<BorrowLoanOwnerSegment>::with_capacity(3);
        let argument_accesses = Arena::<BorrowArgumentAccessFact>::with_capacity(4);
        let calls = Arena::<BorrowCallFact>::with_capacity(5);
        let loans = Arena::<BorrowLoanFact>::with_capacity(6);
        let states = Arena::<StateBorrowFact>::with_capacity(7);

        let facts = BorrowFacts::with_roots(
            writable_roots.clone(),
            access_segments.clone(),
            owner_segments.clone(),
            argument_accesses.clone(),
            calls.clone(),
            loans.clone(),
            states.clone(),
        );

        assert_eq!(facts.writable_roots, writable_roots);
        assert_eq!(facts.access_segments, access_segments);
        assert_eq!(facts.owner_segments, owner_segments);
        assert_eq!(facts.argument_accesses, argument_accesses);
        assert_eq!(facts.calls, calls);
        assert_eq!(facts.loans, loans);
        assert_eq!(facts.states, states);
        assert!(facts.compatibility_certificates.is_empty());
        assert!(facts.direct_loan_resources.is_empty());
        assert!(facts.reborrow_loan_resources.is_empty());
    }

    #[test]
    fn captured_place_identity_is_structural_and_order_sensitive() {
        let root = psi_symbols::SymbolHandle::from_arena_index(1);
        let first = psi_symbols::SymbolHandle::from_arena_index(2);
        let second = psi_symbols::SymbolHandle::from_arena_index(3);
        let first_expression = crate::expression::ExpressionHandle::from_arena_index(4);
        let place = CapturedPlace {
            root_symbol: root,
            segments: vec![
                psi_facts::PlaceSegment::Field { symbol: first },
                psi_facts::PlaceSegment::Case { variant: second },
                psi_facts::PlaceSegment::FixedIndex { index: 5 },
                psi_facts::PlaceSegment::FixedRange { start: 6, end: 8 },
                psi_facts::PlaceSegment::Index {
                    expression: first_expression,
                },
            ],
        };
        let mut reordered = place.clone();
        reordered.segments.swap(0, 1);
        let mut changed_root = place.clone();
        changed_root.root_symbol = second;
        let mut changed_selector = place.clone();
        let Some(psi_facts::PlaceSegment::Index { expression }) =
            changed_selector.segments.last_mut()
        else {
            unreachable!()
        };
        *expression = crate::expression::ExpressionHandle::from_arena_index(5);

        assert_ne!(place, reordered);
        assert_ne!(place, changed_root);
        assert_ne!(place, changed_selector);
        assert_eq!(place, place.clone());
    }
}
