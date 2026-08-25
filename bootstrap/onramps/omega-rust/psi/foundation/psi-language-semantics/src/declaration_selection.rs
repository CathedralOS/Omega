use psi_source::SourceSpan;
use psi_symbols::SymbolHandle;

/// Whether an authored declaration selection contributes only to an
/// implementation or is exposed through a package's published surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionExposure {
    PrivateImplementation,
    PublicInterface,
}

/// The authored syntax which selected a declaration.
///
/// These kinds describe source authority only. Compiler-planned layout,
/// movement, and automatic cleanup are semantic dependencies and do not belong
/// in this ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionKind {
    TypeReference,
    StaticPathSegment,
    MemberAccess,
    StructLiteralType,
    StructLiteralCase,
    StructLiteralField,
    CaseReference,
    CaseMembership,
    DomainMembership,
    Call,
    Operator,
    Conformance,
    ExplicitCleanupCall,
}

/// The checked fact family which must supply a declaration selected too late
/// for symbol resolution to settle it.
///
/// A late-bound occurrence is explicit ledger state. It must not be encoded as
/// a resolved target containing an invalid symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionLateBinding {
    CheckedStaticPathSegment,
    CheckedMember,
    CheckedStructLiteralType,
    CheckedStructLiteralCase,
    CheckedStructLiteralField,
    CheckedCaseMembership,
    CheckedCall,
    CheckedOperator,
    CheckedConformance,
}

/// A symbol known to be valid when its authored selection row was recorded.
///
/// The private field prevents callers from constructing a resolved target with
/// `SymbolHandle::invalid()` and accidentally bypassing later finalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedAuthoredDeclarationSelection {
    selected_symbol: SymbolHandle,
}

impl ResolvedAuthoredDeclarationSelection {
    pub fn new(selected_symbol: SymbolHandle) -> Option<Self> {
        selected_symbol
            .is_valid()
            .then_some(Self { selected_symbol })
    }

    pub fn selected_symbol(self) -> SymbolHandle {
        self.selected_symbol
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionTarget {
    Resolved(ResolvedAuthoredDeclarationSelection),
    Intrinsic(AuthoredDeclarationSelectionIntrinsic),
    LateBound(AuthoredDeclarationSelectionLateBinding),
}

/// A compiler-owned language meaning selected by authored syntax without a
/// package declaration. Intrinsics finalize explicitly so package admission
/// never invents a declaration symbol or leaves a successful selection
/// unresolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionIntrinsic {
    BuiltinOperator,
    BuildProviderSelection,
    BuildBoundaryAcceptance,
    BuildWireCompatibilityRequest,
    BuildRootBinding,
    InlineAssemblyOperation,
}

/// Deterministic identity of one authored occurrence within a compilation's
/// declaration-selection ledger.
///
/// The ledger mints identities in deterministic resolution traversal order.
/// Later representations carry the value verbatim and attach it to the
/// corresponding typed or checked fact; they never reconstruct it from source
/// text, diagnostic rendering, or a mutable IR handle. Compiler-generated
/// clones may deliberately retain the same identity because they derive from
/// the same authored occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AuthoredDeclarationSelectionOccurrenceId(u64);

impl AuthoredDeclarationSelectionOccurrenceId {
    pub fn ordinal(self) -> u64 {
        self.0
    }
}

/// Why an authored selection could not enter the ledger.
///
/// Recording is transactional: either a complete row receives its occurrence
/// identity or the ledger remains unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionRecordError {
    InvalidSelectedSymbol,
    OccurrenceCapacityExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionFinalizationError {
    UnknownOccurrence,
    AlreadyResolved,
    LateBindingMismatch,
    InvalidSelectedSymbol,
}

/// One authored occurrence retained while its source location and exposure are
/// still exact. Package ownership is intentionally absent and is joined by a
/// later compiler integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthoredDeclarationSelection {
    occurrence_id: AuthoredDeclarationSelectionOccurrenceId,
    source_span: SourceSpan,
    exposure: AuthoredDeclarationSelectionExposure,
    kind: AuthoredDeclarationSelectionKind,
    target: AuthoredDeclarationSelectionTarget,
}

impl AuthoredDeclarationSelection {
    fn resolved(
        occurrence_id: AuthoredDeclarationSelectionOccurrenceId,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        selected_symbol: SymbolHandle,
    ) -> Result<Self, AuthoredDeclarationSelectionRecordError> {
        let selected = ResolvedAuthoredDeclarationSelection::new(selected_symbol)
            .ok_or(AuthoredDeclarationSelectionRecordError::InvalidSelectedSymbol)?;
        Ok(Self {
            occurrence_id,
            source_span,
            exposure,
            kind,
            target: AuthoredDeclarationSelectionTarget::Resolved(selected),
        })
    }

    fn late_bound(
        occurrence_id: AuthoredDeclarationSelectionOccurrenceId,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        late_binding: AuthoredDeclarationSelectionLateBinding,
    ) -> Self {
        Self {
            occurrence_id,
            source_span,
            exposure,
            kind,
            target: AuthoredDeclarationSelectionTarget::LateBound(late_binding),
        }
    }

    pub fn occurrence_id(self) -> AuthoredDeclarationSelectionOccurrenceId {
        self.occurrence_id
    }

    pub fn source_span(self) -> SourceSpan {
        self.source_span
    }

    pub fn exposure(self) -> AuthoredDeclarationSelectionExposure {
        self.exposure
    }

    pub fn kind(self) -> AuthoredDeclarationSelectionKind {
        self.kind
    }

    pub fn target(self) -> AuthoredDeclarationSelectionTarget {
        self.target
    }
}

/// Append-only authored-selection custody in deterministic source traversal
/// order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthoredDeclarationSelections {
    rows: Vec<AuthoredDeclarationSelection>,
}

impl AuthoredDeclarationSelections {
    pub fn record_resolved(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        selected_symbol: SymbolHandle,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        let occurrence_id = self.next_occurrence_id()?;
        let selection = AuthoredDeclarationSelection::resolved(
            occurrence_id,
            source_span,
            exposure,
            kind,
            selected_symbol,
        )?;
        self.rows.push(selection);
        Ok(occurrence_id)
    }

    pub fn record_late_bound(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        late_binding: AuthoredDeclarationSelectionLateBinding,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        let occurrence_id = self.next_occurrence_id()?;
        self.rows.push(AuthoredDeclarationSelection::late_bound(
            occurrence_id,
            source_span,
            exposure,
            kind,
            late_binding,
        ));
        Ok(occurrence_id)
    }

    pub fn get(
        &self,
        occurrence_id: AuthoredDeclarationSelectionOccurrenceId,
    ) -> Option<&AuthoredDeclarationSelection> {
        let index = usize::try_from(occurrence_id.0).ok()?;
        let selection = self.rows.get(index)?;
        (selection.occurrence_id == occurrence_id).then_some(selection)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &AuthoredDeclarationSelection> {
        self.rows.iter()
    }

    pub fn as_slice(&self) -> &[AuthoredDeclarationSelection] {
        &self.rows
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Replace one exact late-binding obligation with the checked declaration
    /// which discharged it. Failure is transactional: no row changes unless
    /// the occurrence exists, its expected fact family matches, and the
    /// selected symbol is valid.
    pub fn finalize_late_bound(
        &mut self,
        occurrence_id: AuthoredDeclarationSelectionOccurrenceId,
        expected_binding: AuthoredDeclarationSelectionLateBinding,
        selected_symbol: SymbolHandle,
    ) -> Result<(), AuthoredDeclarationSelectionFinalizationError> {
        let selected = ResolvedAuthoredDeclarationSelection::new(selected_symbol)
            .ok_or(AuthoredDeclarationSelectionFinalizationError::InvalidSelectedSymbol)?;
        let index = usize::try_from(occurrence_id.0)
            .map_err(|_| AuthoredDeclarationSelectionFinalizationError::UnknownOccurrence)?;
        let row = self
            .rows
            .get_mut(index)
            .filter(|row| row.occurrence_id == occurrence_id)
            .ok_or(AuthoredDeclarationSelectionFinalizationError::UnknownOccurrence)?;
        match row.target {
            AuthoredDeclarationSelectionTarget::Resolved(_)
            | AuthoredDeclarationSelectionTarget::Intrinsic(_) => {
                Err(AuthoredDeclarationSelectionFinalizationError::AlreadyResolved)
            }
            AuthoredDeclarationSelectionTarget::LateBound(actual) if actual != expected_binding => {
                Err(AuthoredDeclarationSelectionFinalizationError::LateBindingMismatch)
            }
            AuthoredDeclarationSelectionTarget::LateBound(_) => {
                row.target = AuthoredDeclarationSelectionTarget::Resolved(selected);
                Ok(())
            }
        }
    }

    pub fn finalize_intrinsic(
        &mut self,
        occurrence_id: AuthoredDeclarationSelectionOccurrenceId,
        expected_binding: AuthoredDeclarationSelectionLateBinding,
        intrinsic: AuthoredDeclarationSelectionIntrinsic,
    ) -> Result<(), AuthoredDeclarationSelectionFinalizationError> {
        let index = usize::try_from(occurrence_id.0)
            .map_err(|_| AuthoredDeclarationSelectionFinalizationError::UnknownOccurrence)?;
        let row = self
            .rows
            .get_mut(index)
            .filter(|row| row.occurrence_id == occurrence_id)
            .ok_or(AuthoredDeclarationSelectionFinalizationError::UnknownOccurrence)?;
        match row.target {
            AuthoredDeclarationSelectionTarget::Resolved(_)
            | AuthoredDeclarationSelectionTarget::Intrinsic(_) => {
                Err(AuthoredDeclarationSelectionFinalizationError::AlreadyResolved)
            }
            AuthoredDeclarationSelectionTarget::LateBound(actual) if actual != expected_binding => {
                Err(AuthoredDeclarationSelectionFinalizationError::LateBindingMismatch)
            }
            AuthoredDeclarationSelectionTarget::LateBound(_) => {
                row.target = AuthoredDeclarationSelectionTarget::Intrinsic(intrinsic);
                Ok(())
            }
        }
    }

    pub fn all_finalized(&self) -> bool {
        self.rows.iter().all(|row| {
            matches!(
                row.target,
                AuthoredDeclarationSelectionTarget::Resolved(_)
                    | AuthoredDeclarationSelectionTarget::Intrinsic(_)
            )
        })
    }

    fn next_occurrence_id(
        &self,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        let ordinal = u64::try_from(self.rows.len())
            .map_err(|_| AuthoredDeclarationSelectionRecordError::OccurrenceCapacityExceeded)?;
        Ok(AuthoredDeclarationSelectionOccurrenceId(ordinal))
    }
}

impl<'ledger> IntoIterator for &'ledger AuthoredDeclarationSelections {
    type Item = &'ledger AuthoredDeclarationSelection;
    type IntoIter = std::slice::Iter<'ledger, AuthoredDeclarationSelection>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_source::{SourceId, Span};

    fn source_span(start: usize, end: usize) -> SourceSpan {
        SourceSpan::new(SourceId(7), Span::new(start, end))
    }

    fn record_fixture(selections: &mut AuthoredDeclarationSelections) {
        selections
            .record_resolved(
                source_span(2, 6),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess,
                SymbolHandle::from_arena_index(3),
            )
            .expect("valid resolved selection");
        selections
            .record_late_bound(
                source_span(2, 6),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess,
                AuthoredDeclarationSelectionLateBinding::CheckedCall,
            )
            .expect("ledger capacity");
    }

    #[test]
    fn occurrence_identities_are_unique_and_deterministic() {
        let mut first_run = AuthoredDeclarationSelections::default();
        let mut second_run = AuthoredDeclarationSelections::default();
        record_fixture(&mut first_run);
        record_fixture(&mut second_run);

        let first_ids = first_run
            .iter()
            .map(|selection| selection.occurrence_id())
            .collect::<Vec<_>>();
        let second_ids = second_run
            .iter()
            .map(|selection| selection.occurrence_id())
            .collect::<Vec<_>>();

        assert_eq!(first_ids, second_ids);
        assert_eq!(first_ids.len(), 2);
        assert_ne!(first_ids[0], first_ids[1]);
        assert_eq!(first_ids[0].ordinal(), 0);
        assert_eq!(first_ids[1].ordinal(), 1);
        assert_eq!(first_run.get(first_ids[0]), first_run.as_slice().first());
        assert_eq!(first_run.get(first_ids[1]), first_run.as_slice().get(1));
    }

    #[test]
    fn invalid_resolved_target_fails_closed_without_consuming_an_identity() {
        let mut selections = AuthoredDeclarationSelections::default();

        let error = selections
            .record_resolved(
                source_span(4, 9),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::TypeReference,
                SymbolHandle::invalid(),
            )
            .expect_err("invalid target must reject");

        assert_eq!(
            error,
            AuthoredDeclarationSelectionRecordError::InvalidSelectedSymbol
        );
        assert!(selections.is_empty());

        let first_valid = selections
            .record_late_bound(
                source_span(4, 9),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                AuthoredDeclarationSelectionLateBinding::CheckedCall,
            )
            .expect("ledger capacity");
        assert_eq!(first_valid.ordinal(), 0);
    }

    #[test]
    fn late_finalization_is_exact_and_transactional() {
        let mut selections = AuthoredDeclarationSelections::default();
        let occurrence = selections
            .record_late_bound(
                source_span(9, 12),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Operator,
                AuthoredDeclarationSelectionLateBinding::CheckedOperator,
            )
            .expect("record late operator");
        let before = selections.clone();

        assert_eq!(
            selections.finalize_late_bound(
                occurrence,
                AuthoredDeclarationSelectionLateBinding::CheckedCall,
                SymbolHandle::from_arena_index(7),
            ),
            Err(AuthoredDeclarationSelectionFinalizationError::LateBindingMismatch)
        );
        assert_eq!(selections, before);
        assert!(!selections.all_finalized());

        selections
            .finalize_late_bound(
                occurrence,
                AuthoredDeclarationSelectionLateBinding::CheckedOperator,
                SymbolHandle::from_arena_index(7),
            )
            .expect("finalize exact operator occurrence");
        assert!(selections.all_finalized());
        assert_eq!(
            selections.finalize_late_bound(
                occurrence,
                AuthoredDeclarationSelectionLateBinding::CheckedOperator,
                SymbolHandle::from_arena_index(8),
            ),
            Err(AuthoredDeclarationSelectionFinalizationError::AlreadyResolved)
        );
    }
}
