use source::SourceSpan;
use symbols::SymbolHandle;

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
    StaticArgument,
    Operator,
    Conformance,
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
    CheckedDomainMembership,
    CheckedCall,
    CheckedStaticArgument,
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

/// One compiler-owned view operation on a collection or text carrier.
///
/// The operation is retained as closed semantic identity rather than
/// reconstructed from a method spelling after checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionViewOperation {
    SharedSlice,
    MutableSlice,
    TextView,
    Bytes,
}

/// A compiler-owned language meaning selected by authored syntax without a
/// package declaration. Intrinsics finalize explicitly so package admission
/// never invents a declaration symbol or leaves a successful selection
/// unresolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionIntrinsic {
    BuiltinOperator,
    /// One exact compiler-owned carry permission selected by authored domain
    /// syntax. Carry permissions are language meanings, not declarations in
    /// the package namespace.
    CarryPermission(crate::CarryPermission),
    /// The `len` projection on a fixed array or slice. Collection length is
    /// compiler-owned value metadata, not a declaration selected from the
    /// package namespace.
    CollectionLength,
    /// A checked compiler-owned collection/text view operation.
    CollectionView(CollectionViewOperation),
    /// One exact compiler-owned byte-sequence predicate. Retaining the
    /// particular predicate prevents later evidence consumers from having to
    /// reconstruct semantic identity from the call's diagnostic spelling.
    ByteSequencePredicate(crate::byte_predicates::ByteSequencePredicate),
    BuildProviderSelection,
    BuildRepresentationSelection,
    /// Exact toolchain `Optimizations::enable` selection from the root build
    /// vocabulary. This classifies declaration provenance only; optimization
    /// policy and execution remain Omega-owned.
    BuildOptimizationSelection,
    /// Exact toolchain `Optimizations::emit_report` request. Reporting is
    /// deliberately distinct from optimization selection and grants no rule
    /// execution authority.
    BuildOptimizationReportRequest,
    BuildBoundaryAcceptance,
    BuildWireCompatibilityRequest,
    BuildRootBinding,
    BuildIncludedSourceHandoff,
    /// Exact toolchain `BuildLog::write_line` selection. Build logging is a
    /// compiler-owned build observation, not a package or boundary service.
    BuildLogWriteLine,
    /// Compiler-owned wire-schema encoder selected by an exact checked
    /// `Schema::encode(..)` statement. The separately retained schema/type
    /// selections own nominal declaration authority.
    WireEncode,
    /// Compiler-owned wire-schema decoder selected by an exact checked
    /// `Schema::decode(..)` statement. The separately retained schema/type
    /// selections own nominal declaration authority.
    WireDecode,
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

/// Transient compiler partition for one instantiated use of authored syntax.
///
/// Most compiler-derived copies preserve one authored selection occurrence.
/// A trait-default body is different: each conformance application may route
/// the same authored call to a different exact realization. This ordinal
/// separates those applications while source coordinates remain shared. It is
/// compiler-internal join custody and must never enter canonical package
/// evidence, lock identity, or diagnostics as semantic identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompilerDerivedSelectionPartition(u64);

impl CompilerDerivedSelectionPartition {
    pub fn from_compiler_ordinal(ordinal: u64) -> Self {
        Self(ordinal)
    }

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

/// Why a retained authored-selection prefix and one appended suffix could not
/// be joined without changing occurrence identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionSuffixRebaseError {
    SourceFrontierOutOfRange,
    DestinationPrefixTooShort,
    PrefixIdentityMismatch,
    PrefixTargetMismatch,
    OccurrenceCapacityExceeded,
}

/// Checked mapping from the occurrence identities minted while resolving an
/// extension to their positions after a later phase has appended rows to the
/// retained base ledger.
///
/// Construction is private to the ledger join below. Representation owners
/// can therefore distinguish retained and extension sites without recovering
/// either class from spans or names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthoredDeclarationSelectionSuffixRebase {
    source_frontier: u64,
    source_end: u64,
    destination_frontier: u64,
}

impl AuthoredDeclarationSelectionSuffixRebase {
    pub fn retain_base(
        self,
        occurrence: AuthoredDeclarationSelectionOccurrenceId,
    ) -> Option<AuthoredDeclarationSelectionOccurrenceId> {
        (occurrence.0 < self.source_frontier).then_some(occurrence)
    }

    pub fn rebase_extension(
        self,
        occurrence: AuthoredDeclarationSelectionOccurrenceId,
    ) -> Option<AuthoredDeclarationSelectionOccurrenceId> {
        if occurrence.0 < self.source_frontier || occurrence.0 >= self.source_end {
            return None;
        }
        let suffix_offset = occurrence.0.checked_sub(self.source_frontier)?;
        Some(AuthoredDeclarationSelectionOccurrenceId(
            self.destination_frontier.checked_add(suffix_offset)?,
        ))
    }

    /// Map an occurrence stored after an append frontier. Authored extension
    /// occurrences shift, while compiler-generated clones may deliberately
    /// retain the base occurrence from which they were derived.
    pub fn rebase_appended(
        self,
        occurrence: AuthoredDeclarationSelectionOccurrenceId,
    ) -> Option<AuthoredDeclarationSelectionOccurrenceId> {
        self.retain_base(occurrence)
            .or_else(|| self.rebase_extension(occurrence))
    }
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
    compiler_partition: Option<CompilerDerivedSelectionPartition>,
    target: AuthoredDeclarationSelectionTarget,
}

impl AuthoredDeclarationSelection {
    fn resolved(
        occurrence_id: AuthoredDeclarationSelectionOccurrenceId,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        compiler_partition: Option<CompilerDerivedSelectionPartition>,
        selected_symbol: SymbolHandle,
    ) -> Result<Self, AuthoredDeclarationSelectionRecordError> {
        let selected = ResolvedAuthoredDeclarationSelection::new(selected_symbol)
            .ok_or(AuthoredDeclarationSelectionRecordError::InvalidSelectedSymbol)?;
        Ok(Self {
            occurrence_id,
            source_span,
            exposure,
            kind,
            compiler_partition,
            target: AuthoredDeclarationSelectionTarget::Resolved(selected),
        })
    }

    fn late_bound(
        occurrence_id: AuthoredDeclarationSelectionOccurrenceId,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        compiler_partition: Option<CompilerDerivedSelectionPartition>,
        late_binding: AuthoredDeclarationSelectionLateBinding,
    ) -> Self {
        Self {
            occurrence_id,
            source_span,
            exposure,
            kind,
            compiler_partition,
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

    pub fn compiler_partition(self) -> Option<CompilerDerivedSelectionPartition> {
        self.compiler_partition
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
    /// Replace the exact retained prefix of a combined resolved ledger with a
    /// destination ledger owned by a later phase, then append and re-identify
    /// only the resolved extension suffix.
    ///
    /// A destination prefix may finalize a source `LateBound` row, but may not
    /// change its source identity, exposure, kind, or an already-settled
    /// target. Extra destination rows are retained verbatim ahead of the
    /// shifted suffix.
    pub fn replace_prefix_and_rebase_suffix(
        &self,
        source_frontier: usize,
        destination_base: &Self,
    ) -> Result<
        (Self, AuthoredDeclarationSelectionSuffixRebase),
        AuthoredDeclarationSelectionSuffixRebaseError,
    > {
        if source_frontier > self.rows.len() {
            return Err(AuthoredDeclarationSelectionSuffixRebaseError::SourceFrontierOutOfRange);
        }
        if destination_base.rows.len() < source_frontier {
            return Err(AuthoredDeclarationSelectionSuffixRebaseError::DestinationPrefixTooShort);
        }

        for (source, destination) in self.rows[..source_frontier]
            .iter()
            .zip(&destination_base.rows[..source_frontier])
        {
            if source.occurrence_id != destination.occurrence_id
                || source.source_span != destination.source_span
                || source.exposure != destination.exposure
                || source.kind != destination.kind
                || source.compiler_partition != destination.compiler_partition
            {
                return Err(AuthoredDeclarationSelectionSuffixRebaseError::PrefixIdentityMismatch);
            }
            let target_is_compatible = source.target == destination.target
                || matches!(
                    (source.target, destination.target),
                    (
                        AuthoredDeclarationSelectionTarget::LateBound(_),
                        AuthoredDeclarationSelectionTarget::Resolved(_)
                            | AuthoredDeclarationSelectionTarget::Intrinsic(_)
                    )
                );
            if !target_is_compatible {
                return Err(AuthoredDeclarationSelectionSuffixRebaseError::PrefixTargetMismatch);
            }
        }

        let source_frontier_index = source_frontier;
        let source_frontier = u64::try_from(source_frontier_index).map_err(|_| {
            AuthoredDeclarationSelectionSuffixRebaseError::OccurrenceCapacityExceeded
        })?;
        let source_end = u64::try_from(self.rows.len()).map_err(|_| {
            AuthoredDeclarationSelectionSuffixRebaseError::OccurrenceCapacityExceeded
        })?;
        let destination_frontier = u64::try_from(destination_base.rows.len()).map_err(|_| {
            AuthoredDeclarationSelectionSuffixRebaseError::OccurrenceCapacityExceeded
        })?;
        let rebase = AuthoredDeclarationSelectionSuffixRebase {
            source_frontier,
            source_end,
            destination_frontier,
        };
        let mut rows = destination_base.rows.clone();
        rows.reserve(self.rows.len() - source_frontier_index);
        for source in &self.rows[source_frontier_index..] {
            let occurrence_id = rebase
                .rebase_extension(source.occurrence_id)
                .ok_or(AuthoredDeclarationSelectionSuffixRebaseError::OccurrenceCapacityExceeded)?;
            let mut rebased = *source;
            rebased.occurrence_id = occurrence_id;
            rows.push(rebased);
        }
        Ok((Self { rows }, rebase))
    }

    pub fn record_resolved(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        selected_symbol: SymbolHandle,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        self.record_resolved_in_partition(source_span, exposure, kind, None, selected_symbol)
    }

    pub fn record_resolved_in_partition(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        compiler_partition: Option<CompilerDerivedSelectionPartition>,
        selected_symbol: SymbolHandle,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        let occurrence_id = self.next_occurrence_id()?;
        let selection = AuthoredDeclarationSelection::resolved(
            occurrence_id,
            source_span,
            exposure,
            kind,
            compiler_partition,
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
        self.record_late_bound_in_partition(source_span, exposure, kind, None, late_binding)
    }

    pub fn record_late_bound_in_partition(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        compiler_partition: Option<CompilerDerivedSelectionPartition>,
        late_binding: AuthoredDeclarationSelectionLateBinding,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        let occurrence_id = self.next_occurrence_id()?;
        self.rows.push(AuthoredDeclarationSelection::late_bound(
            occurrence_id,
            source_span,
            exposure,
            kind,
            compiler_partition,
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
    use source::{SourceId, Span};

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
    fn compiler_partitions_separate_applications_without_replacing_source_custody() {
        let mut selections = AuthoredDeclarationSelections::default();
        let source = source_span(12, 16);
        let first_partition = CompilerDerivedSelectionPartition::from_compiler_ordinal(3);
        let second_partition = CompilerDerivedSelectionPartition::from_compiler_ordinal(7);
        let first = selections
            .record_resolved_in_partition(
                source,
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                Some(first_partition),
                SymbolHandle::from_arena_index(4),
            )
            .expect("first application");
        let second = selections
            .record_resolved_in_partition(
                source,
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                Some(second_partition),
                SymbolHandle::from_arena_index(8),
            )
            .expect("second application");

        assert_ne!(first, second);
        assert_eq!(selections.get(first).unwrap().source_span(), source);
        assert_eq!(selections.get(second).unwrap().source_span(), source);
        assert_eq!(
            selections.get(first).unwrap().compiler_partition(),
            Some(first_partition)
        );
        assert_eq!(
            selections.get(second).unwrap().compiler_partition(),
            Some(second_partition)
        );
    }

    #[test]
    fn suffix_rebase_preserves_destination_prefix_and_shifts_only_extension_rows() {
        let mut combined = AuthoredDeclarationSelections::default();
        combined
            .record_late_bound(
                source_span(1, 2),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                AuthoredDeclarationSelectionLateBinding::CheckedCall,
            )
            .expect("base row");
        let mut destination = combined.clone();
        destination
            .finalize_late_bound(
                AuthoredDeclarationSelectionOccurrenceId(0),
                AuthoredDeclarationSelectionLateBinding::CheckedCall,
                SymbolHandle::from_arena_index(4),
            )
            .expect("typed base finalization");
        destination
            .record_resolved(
                source_span(3, 4),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess,
                SymbolHandle::from_arena_index(5),
            )
            .expect("typed-only base row");
        let extension = combined
            .record_resolved(
                source_span(5, 6),
                AuthoredDeclarationSelectionExposure::PublicInterface,
                AuthoredDeclarationSelectionKind::TypeReference,
                SymbolHandle::from_arena_index(6),
            )
            .expect("extension row");

        let (joined, rebase) = combined
            .replace_prefix_and_rebase_suffix(1, &destination)
            .expect("compatible retained prefix");

        assert_eq!(
            &joined.as_slice()[..destination.len()],
            destination.as_slice()
        );
        assert_eq!(joined.len(), 3);
        assert_eq!(joined.as_slice()[2].occurrence_id().ordinal(), 2);
        assert_eq!(
            rebase.rebase_extension(extension).map(|id| id.ordinal()),
            Some(2)
        );
        assert_eq!(
            rebase.retain_base(AuthoredDeclarationSelectionOccurrenceId(0)),
            Some(AuthoredDeclarationSelectionOccurrenceId(0))
        );
        assert_eq!(
            rebase.retain_base(extension),
            None,
            "extension identity cannot be laundered through a base site"
        );
    }

    #[test]
    fn suffix_rebase_rejects_prefix_identity_and_target_tampering() {
        let mut combined = AuthoredDeclarationSelections::default();
        combined
            .record_resolved(
                source_span(1, 2),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                SymbolHandle::from_arena_index(4),
            )
            .expect("base row");

        let mut identity_tamper = AuthoredDeclarationSelections::default();
        identity_tamper
            .record_resolved(
                source_span(9, 10),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                SymbolHandle::from_arena_index(4),
            )
            .expect("tampered row");
        assert_eq!(
            combined.replace_prefix_and_rebase_suffix(1, &identity_tamper),
            Err(AuthoredDeclarationSelectionSuffixRebaseError::PrefixIdentityMismatch)
        );

        let mut target_tamper = AuthoredDeclarationSelections::default();
        target_tamper
            .record_resolved(
                source_span(1, 2),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                SymbolHandle::from_arena_index(9),
            )
            .expect("retargeted row");
        assert_eq!(
            combined.replace_prefix_and_rebase_suffix(1, &target_tamper),
            Err(AuthoredDeclarationSelectionSuffixRebaseError::PrefixTargetMismatch)
        );

        let mut partitioned = AuthoredDeclarationSelections::default();
        partitioned
            .record_resolved_in_partition(
                source_span(1, 2),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                Some(CompilerDerivedSelectionPartition::from_compiler_ordinal(3)),
                SymbolHandle::from_arena_index(4),
            )
            .expect("partitioned row");
        assert_eq!(
            combined.replace_prefix_and_rebase_suffix(1, &partitioned),
            Err(AuthoredDeclarationSelectionSuffixRebaseError::PrefixIdentityMismatch),
            "compiler application custody cannot be changed while rebasing a retained row"
        );
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
