//! Mutable declaration selections belong to source resolution and typing.
//! Their occurrence coordinates and copy partitions never cross into a product.

use crate::declaration_selection::AuthoredDeclarationSelectionExposure;
use crate::declaration_selection::AuthoredDeclarationSelectionIntrinsic;
use crate::declaration_selection::AuthoredDeclarationSelectionKind;
use crate::declaration_selection::AuthoredDeclarationSelectionLateBinding;
use crate::declaration_selection::AuthoredDeclarationSelectionTarget;
use crate::declaration_selection::ResolvedAuthoredDeclarationSelection;
use source_text::SourceSpan;
use symbols::SymbolHandle;

/// The realized machine selected for an authored program root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBinding {
    pub slot: String,
    pub implementation: String,
    pub implementation_symbol: SymbolHandle,
    pub source_span: Option<SourceSpan>,
}

/// Compatibility checks requested for one Build-selected wire schema edge.
#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct WireCompatibilityDemand {
    pub edge: String,
    pub lineage: String,
    pub local_schema: String,
    pub peer_schema: String,
    pub require_readable: bool,
    pub require_writable: bool,
    pub require_unknown_preservation: bool,
    pub require_canonical: bool,
    pub require_complete_migration: bool,
}

/// An explicit contract assumption attributed to the selecting Build declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumedBindingContract {
    pub required: SymbolHandle,
    pub supplied: SymbolHandle,
    pub selecting_machine: SymbolHandle,
    pub source_span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceAuditRegistration {
    pub declaration: source_admission::RetainedDeclaration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestGroupRegistration {
    pub configuration: crate::declaration_selection::TestGroupConfiguration,
    pub origin: crate::declaration_selection::BuildSelectionOrigin,
    pub requirement: SymbolHandle,
    pub closed_requirement: Option<SymbolHandle>,
    pub selecting_machine: SymbolHandle,
    pub source_span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub struct AuthoredDeclarationSelectionOccurrenceId(u64);

impl AuthoredDeclarationSelectionOccurrenceId {
    pub fn ordinal(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub struct CompilerDerivedSelectionPartition(u64);

impl CompilerDerivedSelectionPartition {
    pub fn from_compiler_ordinal(ordinal: u64) -> Self {
        Self(ordinal)
    }

    pub fn ordinal(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum AuthoredDeclarationSelectionCoordinateError {
    Missing,
    Ambiguous,
    CompilerCopyRequiresOwner,
}

impl std::fmt::Display for AuthoredDeclarationSelectionCoordinateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Missing => "authored selection source coordinate has no current ledger row",
            Self::Ambiguous => "authored selection source coordinate has multiple current ledger rows",
            Self::CompilerCopyRequiresOwner =>
                "authored selection belongs to a compiler-derived copy whose exact semantic owner is required",
        })
    }
}

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
    StaticArgumentPositionMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionSuffixRebaseError {
    SourceFrontierOutOfRange,
    DestinationPrefixTooShort,
    PrefixIdentityMismatch,
    PrefixTargetMismatch,
    OccurrenceCapacityExceeded,
}

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

    pub fn rebase_appended(
        self,
        occurrence: AuthoredDeclarationSelectionOccurrenceId,
    ) -> Option<AuthoredDeclarationSelectionOccurrenceId> {
        self.retain_base(occurrence)
            .or_else(|| self.rebase_extension(occurrence))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub struct AuthoredDeclarationSelection {
    occurrence_id: AuthoredDeclarationSelectionOccurrenceId,
    source_span: SourceSpan,
    exposure: AuthoredDeclarationSelectionExposure,
    kind: AuthoredDeclarationSelectionKind,
    compiler_partition: Option<CompilerDerivedSelectionPartition>,
    target: AuthoredDeclarationSelectionTarget,
    top_level_static_argument_position: Option<usize>,
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
            top_level_static_argument_position: None,
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
            top_level_static_argument_position: None,
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

    pub fn top_level_static_argument_position(self) -> Option<usize> {
        self.top_level_static_argument_position
    }
}

#[derive(Clone, Default)]
pub struct AuthoredDeclarationSelections {
    rows: SelectionRows,
    by_span: SpanIndex,
}

// The rows determine the complete span index, and decoding verifies its
// coverage. Compare logical rows, not the batches used to look them up.
impl PartialEq for AuthoredDeclarationSelections {
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows
    }
}

impl Eq for AuthoredDeclarationSelections {}

impl record_storage::Record for AuthoredDeclarationSelections {
    fn write(
        &self,
        output: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        output.record(|output| {
            self.rows.write(output)?;
            self.by_span.write(output)
        })
    }

    fn read<Context>(
        input: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
        input.record(|input| {
            let rows = SelectionRows::read(input)?;
            let by_span = SpanIndex::read(input)?;
            if rows
                .iter()
                .enumerate()
                .any(|(position, row)| row.occurrence_id.0 != position as u64)
            {
                return Err(input.error("selection occurrence does not match its row ordinal"));
            }
            let mut indexed = 0usize;
            for map in by_span.maps() {
                for (span, positions) in map {
                    if positions.is_empty()
                        || positions.windows(2).any(|pair| pair[0] >= pair[1])
                        || positions.iter().any(|position| {
                            rows.get(*position as usize)
                                .is_none_or(|row| row.source_span != *span)
                        })
                    {
                        return Err(input.error("selection span index does not match its rows"));
                    }
                    indexed = indexed
                        .checked_add(positions.len())
                        .ok_or_else(|| input.error("selection span index length overflow"))?;
                }
            }
            if indexed != rows.len() {
                return Err(input.error("selection span index omits rows"));
            }
            Ok(Self { rows, by_span })
        })
    }
}

// Occurrence ordinals stay dense and independent of record allocation. An
// empty table has no invented selection sentinel; its first row supplies the
// arena's private dummy, which never enters the logical rows or record codec.
#[derive(Clone, Default)]
struct SelectionRows(Option<arena::RootedArena<AuthoredDeclarationSelection>>);

impl SelectionRows {
    fn view(&self) -> arena::RootRows<'_, AuthoredDeclarationSelection> {
        self.0.as_ref().map_or_else(Default::default, |rows| {
            rows.span_or_empty(arena::HandleSpan::from_parts(
                arena::Handle::from_parts(1, 1),
                rows.len() as u32,
            ))
        })
    }

    fn len(&self) -> usize {
        self.0.as_ref().map_or(0, arena::RootedArena::len)
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn get(&self, position: usize) -> Option<&AuthoredDeclarationSelection> {
        self.view().get(position)
    }

    fn get_mut(&mut self, position: usize) -> Option<&mut AuthoredDeclarationSelection> {
        let slot = u32::try_from(position).ok()?.checked_add(1)?;
        let rows = self.0.as_mut()?;
        let handle = arena::Handle::from_parts(slot, 1);
        rows.is_valid(handle).then(|| rows.get_mut(handle))
    }

    fn iter(&self) -> arena::RootRowsIter<'_, AuthoredDeclarationSelection> {
        self.view().iter()
    }

    fn reserve(&mut self, additional: usize) {
        if let Some(rows) = &mut self.0 {
            rows.reserve(additional);
        }
    }

    fn push(&mut self, row: AuthoredDeclarationSelection) {
        let rows = self.0.get_or_insert_with(|| {
            arena::Arena::from_storage(row, Vec::new(), None, 1, true)
                .expect("empty selection storage has valid geometry")
                .into()
        });
        rows.append(row);
    }
}

impl PartialEq for SelectionRows {
    fn eq(&self, other: &Self) -> bool {
        self.view() == other.view()
    }
}

impl Eq for SelectionRows {}

impl std::ops::Index<usize> for SelectionRows {
    type Output = AuthoredDeclarationSelection;

    fn index(&self, position: usize) -> &Self::Output {
        self.view().at(position)
    }
}

impl record_storage::Record for SelectionRows {
    fn write(
        &self,
        output: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        output.record(|output| {
            output.sequence(self.len())?;
            for row in self.iter() {
                row.write(output)?;
            }
            Ok(())
        })
    }

    fn read<Context>(
        input: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
        let rows = Vec::<AuthoredDeclarationSelection>::read(input)?;
        let Some(&dummy) = rows.first() else {
            return Ok(Self::default());
        };
        let metadata =
            arena::RootedArena::<AuthoredDeclarationSelection>::decoded_record_metadata_bytes(
                rows.len(),
            )
            .ok_or_else(|| input.error("selection row metadata allocation overflow"))?;
        input.allocate(metadata)?;
        let rows = arena::Arena::from_storage(dummy, rows, None, 1, true)
            .map_err(|reason| input.error(reason))?;
        Ok(Self(Some(rows.into())))
    }
}

impl std::fmt::Debug for AuthoredDeclarationSelections {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthoredDeclarationSelections")
            .field("rows", &self.rows.view())
            .finish()
    }
}

type SpanPositions = std::collections::HashMap<SourceSpan, Vec<u32>>;

// A continuation freezes the existing span index and appends to its own map.
// Queries visit these immutable batches in insertion order, so repeated source
// coordinates keep both their original ambiguity and their first-match order.
#[derive(Clone, Default)]
struct SpanIndex {
    retained: Vec<std::sync::Arc<SpanPositions>>,
    output: Option<std::sync::Arc<SpanPositions>>,
}

impl SpanIndex {
    fn maps(&self) -> impl Iterator<Item = &SpanPositions> {
        self.retained
            .iter()
            .chain(self.output.iter())
            .map(std::sync::Arc::as_ref)
    }

    fn positions(&self, source_span: SourceSpan) -> impl Iterator<Item = u32> + '_ {
        self.maps()
            .filter_map(move |map| map.get(&source_span))
            .flat_map(|positions| positions.iter().copied())
    }

    fn add(&mut self, source_span: SourceSpan, position: usize) {
        let position = u32::try_from(position).expect("selection ledger position overflow");
        if self
            .output
            .as_ref()
            .is_some_and(|map| std::sync::Arc::strong_count(map) != 1)
        {
            let shared = self.output.take().expect("shared selection index output");
            if !shared.is_empty() {
                self.retained.push(shared);
            }
        }
        let output = self
            .output
            .get_or_insert_with(|| std::sync::Arc::new(SpanPositions::new()));
        std::sync::Arc::get_mut(output)
            .expect("selection index output is private")
            .entry(source_span)
            .or_default()
            .push(position);
    }
}

impl record_storage::Record for SpanIndex {
    fn write(
        &self,
        output: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        output.record(|output| {
            let keys = self
                .maps()
                .flat_map(|map| map.keys().copied())
                .collect::<std::collections::HashSet<_>>();
            let mut encoded = Vec::with_capacity(keys.len());
            for key in keys {
                let mut scratch = output.scratch();
                key.write(&mut scratch)?;
                encoded.push((scratch.finish(), key));
            }
            encoded.sort_by(|left, right| left.0.cmp(&right.0));
            output.sequence(encoded.len())?;
            for (_, key) in encoded {
                key.write(output)?;
                output.record(|output| {
                    output.sequence(self.positions(key).count())?;
                    for position in self.positions(key) {
                        position.write(output)?;
                    }
                    Ok(())
                })?;
            }
            Ok(())
        })
    }

    fn read<Context>(
        input: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
        input.record(|input| {
            let map = SpanPositions::read(input)?;
            if map.is_empty() {
                return Ok(Self::default());
            }
            let bytes = record_storage::shared_allocation_bytes(std::alloc::Layout::new::<
                SpanPositions,
            >())
            .ok_or_else(|| input.error("selection index allocation overflow"))?;
            input.allocate(bytes)?;
            Ok(Self {
                retained: Vec::new(),
                output: Some(std::sync::Arc::new(map)),
            })
        })
    }
}

impl AuthoredDeclarationSelections {
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
        if destination_base
            .rows
            .len()
            .checked_add(self.rows.len() - source_frontier)
            .is_none_or(|length| length > u32::MAX as usize)
        {
            return Err(AuthoredDeclarationSelectionSuffixRebaseError::OccurrenceCapacityExceeded);
        }

        for (source, destination) in self
            .rows
            .view()
            .slice(0..source_frontier)
            .iter()
            .zip(destination_base.rows.view().slice(0..source_frontier))
        {
            if source.occurrence_id != destination.occurrence_id
                || source.source_span != destination.source_span
                || source.exposure != destination.exposure
                || source.kind != destination.kind
                || source.compiler_partition != destination.compiler_partition
                || source.top_level_static_argument_position
                    != destination.top_level_static_argument_position
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
        let mut destination = destination_base.clone();
        destination
            .rows
            .reserve(self.rows.len() - source_frontier_index);
        for source in self
            .rows
            .view()
            .slice(source_frontier_index..self.rows.len())
        {
            let occurrence_id = rebase
                .rebase_extension(source.occurrence_id)
                .ok_or(AuthoredDeclarationSelectionSuffixRebaseError::OccurrenceCapacityExceeded)?;
            let mut rebased = *source;
            rebased.occurrence_id = occurrence_id;
            destination
                .by_span
                .add(rebased.source_span, destination.rows.len());
            destination.rows.push(rebased);
        }
        Ok((destination, rebase))
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
        self.by_span.add(source_span, self.rows.len());
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
        self.by_span.add(source_span, self.rows.len());
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

    pub fn source_coordinate_occurrence(
        &self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionCoordinateError>
    {
        use AuthoredDeclarationSelectionCoordinateError as Error;
        let positions = self.by_span.positions(source_span);
        let mut selected = None;
        let mut ambiguous = false;
        let mut partitioned = false;
        for position in positions {
            let row = &self.rows[position as usize];
            if row.exposure != exposure || row.kind != kind {
                continue;
            }
            partitioned |= row.compiler_partition.is_some();
            if selected.replace(row.occurrence_id).is_some() {
                ambiguous = true;
            }
        }
        if partitioned {
            Err(Error::CompilerCopyRequiresOwner)
        } else if ambiguous {
            Err(Error::Ambiguous)
        } else {
            selected.ok_or(Error::Missing)
        }
    }

    pub fn resolved_occurrence(
        &self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        selected_symbol: SymbolHandle,
    ) -> Option<AuthoredDeclarationSelectionOccurrenceId> {
        self.by_span
            .positions(source_span)
            .map(|position| &self.rows[position as usize])
            .find(|selection| {
                selection.exposure == exposure
                    && selection.kind == kind
                    && selection.compiler_partition.is_none()
                    && matches!(
                        selection.target,
                        AuthoredDeclarationSelectionTarget::Resolved(target)
                            if target.selected_symbol() == selected_symbol
                    )
            })
            .map(|selection| selection.occurrence_id)
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

    pub fn rows(&self) -> arena::RootRows<'_, AuthoredDeclarationSelection> {
        self.rows.view()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn retain_top_level_static_argument_position(
        &mut self,
        occurrence: AuthoredDeclarationSelectionOccurrenceId,
        position: usize,
    ) -> Result<(), AuthoredDeclarationSelectionFinalizationError> {
        let row = usize::try_from(occurrence.0)
            .ok()
            .and_then(|index| self.rows.get(index))
            .filter(|row| row.occurrence_id == occurrence)
            .ok_or(AuthoredDeclarationSelectionFinalizationError::UnknownOccurrence)?;
        if !matches!(
            row.kind,
            AuthoredDeclarationSelectionKind::StaticArgument
                | AuthoredDeclarationSelectionKind::Conformance
        ) || row
            .top_level_static_argument_position
            .is_some_and(|retained| retained != position)
        {
            return Err(
                AuthoredDeclarationSelectionFinalizationError::StaticArgumentPositionMismatch,
            );
        }
        if row.top_level_static_argument_position != Some(position) {
            self.rows
                .get_mut(occurrence.0 as usize)
                .expect("validated selection occurrence")
                .top_level_static_argument_position = Some(position);
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

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
            .get(index)
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
                self.rows
                    .get_mut(index)
                    .expect("validated selection occurrence")
                    .target = AuthoredDeclarationSelectionTarget::Resolved(selected);
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
            .get(index)
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
                self.rows
                    .get_mut(index)
                    .expect("validated selection occurrence")
                    .target = AuthoredDeclarationSelectionTarget::Intrinsic(intrinsic);
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
        if self.rows.len() >= u32::MAX as usize {
            return Err(AuthoredDeclarationSelectionRecordError::OccurrenceCapacityExceeded);
        }
        let ordinal = u64::try_from(self.rows.len())
            .map_err(|_| AuthoredDeclarationSelectionRecordError::OccurrenceCapacityExceeded)?;
        Ok(AuthoredDeclarationSelectionOccurrenceId(ordinal))
    }
}

impl<'ledger> IntoIterator for &'ledger AuthoredDeclarationSelections {
    type Item = &'ledger AuthoredDeclarationSelection;
    type IntoIter = arena::RootRowsIter<'ledger, AuthoredDeclarationSelection>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_text::{SourceId, Span};

    fn source_span(start: usize, end: usize) -> SourceSpan {
        SourceSpan::new(SourceId(7), Span::new(start, end))
    }

    #[test]
    fn static_operand_positions_survive_resolution_and_reject_drift() {
        let mut selections = AuthoredDeclarationSelections::default();
        let conformance = selections
            .record_resolved(
                source_span(10, 17),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Conformance,
                SymbolHandle::from_arena_index(1),
            )
            .expect("conformance occurrence");
        let argument = selections
            .record_late_bound(
                source_span(30, 37),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::StaticArgument,
                AuthoredDeclarationSelectionLateBinding::CheckedStaticArgument,
            )
            .expect("static occurrence");
        selections
            .retain_top_level_static_argument_position(conformance, 1)
            .expect("conformance slot");
        selections
            .retain_top_level_static_argument_position(argument, 4)
            .expect("argument slot");
        let original = selections.clone();
        selections
            .finalize_late_bound(
                argument,
                AuthoredDeclarationSelectionLateBinding::CheckedStaticArgument,
                SymbolHandle::from_arena_index(2),
            )
            .expect("resolved argument");
        assert_eq!(
            selections
                .get(argument)
                .expect("retained occurrence")
                .top_level_static_argument_position(),
            Some(4)
        );
        assert_eq!(
            selections.retain_top_level_static_argument_position(argument, 3),
            Err(AuthoredDeclarationSelectionFinalizationError::StaticArgumentPositionMismatch)
        );
        let (rebased, _) = original
            .replace_prefix_and_rebase_suffix(original.len(), &selections)
            .expect("resolution preserves operand custody");
        assert_eq!(
            rebased
                .get(conformance)
                .expect("retained conformance")
                .top_level_static_argument_position(),
            Some(1)
        );
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
    fn retained_selection_rows_and_span_indexes_keep_fork_writes_private() {
        use super::{
            AuthoredDeclarationSelectionCoordinateError as CoordinateError,
            AuthoredDeclarationSelectionTarget as Target,
        };
        let mut original = AuthoredDeclarationSelections::default();
        let first = original
            .record_resolved(
                source_span(2, 6),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess,
                SymbolHandle::from_arena_index(3),
            )
            .unwrap();
        let late = original
            .record_late_bound(
                source_span(8, 9),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                AuthoredDeclarationSelectionLateBinding::CheckedCall,
            )
            .unwrap();
        let mut fork = original.clone();
        assert!(std::ptr::eq(
            original.get(first).unwrap(),
            fork.get(first).unwrap()
        ));
        assert!(std::sync::Arc::ptr_eq(
            original.by_span.output.as_ref().unwrap(),
            fork.by_span.output.as_ref().unwrap()
        ));
        fork.finalize_late_bound(
            late,
            AuthoredDeclarationSelectionLateBinding::CheckedCall,
            SymbolHandle::from_arena_index(7),
        )
        .unwrap();
        assert!(matches!(
            original.get(late).unwrap().target(),
            Target::LateBound(_)
        ));
        assert!(matches!(
            fork.get(late).unwrap().target(),
            Target::Resolved(_)
        ));
        assert!(std::ptr::eq(
            original.get(first).unwrap(),
            fork.get(first).unwrap()
        ));
        assert!(!std::ptr::eq(
            original.get(late).unwrap(),
            fork.get(late).unwrap()
        ));

        let appended = fork
            .record_resolved(
                source_span(2, 6),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess,
                SymbolHandle::from_arena_index(9),
            )
            .unwrap();
        assert_eq!(appended.ordinal(), 2);
        assert!(std::sync::Arc::ptr_eq(
            original.by_span.output.as_ref().unwrap(),
            &fork.by_span.retained[0]
        ));
        assert_eq!(
            original.source_coordinate_occurrence(
                source_span(2, 6),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess
            ),
            Ok(first)
        );
        assert_eq!(
            fork.source_coordinate_occurrence(
                source_span(2, 6),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess
            ),
            Err(CoordinateError::Ambiguous)
        );
        assert_eq!(
            fork.resolved_occurrence(
                source_span(2, 6),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess,
                SymbolHandle::from_arena_index(9)
            ),
            Some(appended)
        );
        assert_eq!(
            original.resolved_occurrence(
                source_span(2, 6),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess,
                SymbolHandle::from_arena_index(9)
            ),
            None
        );
        let original_tail = original
            .record_resolved(
                source_span(10, 12),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess,
                SymbolHandle::from_arena_index(11),
            )
            .unwrap();
        assert_eq!(
            original_tail, appended,
            "forks reuse the same independent tail ordinal"
        );
        assert_ne!(original.get(original_tail), fork.get(appended));
        fork.record_resolved_in_partition(
            source_span(2, 6),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::MemberAccess,
            Some(CompilerDerivedSelectionPartition::from_compiler_ordinal(1)),
            SymbolHandle::from_arena_index(12),
        )
        .unwrap();
        assert_eq!(
            fork.source_coordinate_occurrence(
                source_span(2, 6),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess
            ),
            Err(CoordinateError::CompilerCopyRequiresOwner)
        );
    }

    #[test]
    fn shared_selection_batches_keep_legacy_record_bytes_and_refuse_bad_indexes() {
        #[derive(record_storage::Record)]
        struct LegacyIndex(super::SpanPositions);
        #[derive(record_storage::Record)]
        struct LegacySelections {
            rows: Vec<super::AuthoredDeclarationSelection>,
            by_span: LegacyIndex,
        }
        let limits = record_storage::RecordLimits::default();
        let mut original = AuthoredDeclarationSelections::default();
        record_fixture(&mut original);
        let mut fork = original.clone();
        fork.record_resolved(
            source_span(2, 6),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::MemberAccess,
            SymbolHandle::from_arena_index(5),
        )
        .unwrap();
        fork.finalize_intrinsic(
            AuthoredDeclarationSelectionOccurrenceId(1),
            AuthoredDeclarationSelectionLateBinding::CheckedCall,
            AuthoredDeclarationSelectionIntrinsic::CollectionLength,
        )
        .unwrap();
        let mut map = super::SpanPositions::new();
        for row in fork.iter() {
            map.entry(row.source_span())
                .or_default()
                .push(row.occurrence_id().ordinal() as u32);
        }
        let mut legacy = LegacySelections {
            rows: fork.iter().copied().collect(),
            by_span: LegacyIndex(map),
        };
        let bytes = record_storage::encode(&legacy, limits).unwrap();
        assert_eq!(record_storage::encode(&fork, limits).unwrap(), bytes);
        let decoded: AuthoredDeclarationSelections =
            record_storage::decode(&bytes, limits).unwrap();
        assert_eq!(decoded, fork);
        assert_eq!(record_storage::encode(&decoded, limits).unwrap(), bytes);
        let legacy_decoded: LegacySelections = record_storage::decode(&bytes, limits).unwrap();
        assert_eq!(legacy_decoded.rows, legacy.rows);

        let row_bytes =
            legacy.rows.len() * std::mem::size_of::<super::AuthoredDeclarationSelection>();
        let metadata = arena::RootedArena::<super::AuthoredDeclarationSelection>::decoded_record_metadata_bytes(legacy.rows.len()).unwrap();
        let error = record_storage::decode::<AuthoredDeclarationSelections>(
            &bytes,
            record_storage::RecordLimits {
                allocation: row_bytes + metadata - 1,
                ..limits
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("allocation limit"));
        assert!(
            record_storage::decode::<AuthoredDeclarationSelections>(
                &bytes,
                record_storage::RecordLimits { depth: 1, ..limits }
            )
            .is_err()
        );
        legacy
            .by_span
            .0
            .get_mut(&source_span(2, 6))
            .unwrap()
            .push(0);
        let bad_index = record_storage::encode(&legacy, limits).unwrap();
        assert!(
            record_storage::decode::<AuthoredDeclarationSelections>(&bad_index, limits)
                .unwrap_err()
                .to_string()
                .contains("span index")
        );
        legacy.by_span.0.clear();
        let missing_index = record_storage::encode(&legacy, limits).unwrap();
        assert!(
            record_storage::decode::<AuthoredDeclarationSelections>(&missing_index, limits)
                .unwrap_err()
                .to_string()
                .contains("omits rows")
        );
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
        assert_eq!(first_run.get(first_ids[0]), first_run.rows().first());
        assert_eq!(first_run.get(first_ids[1]), first_run.rows().get(1));
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
            joined.rows().slice(0..destination.len()),
            destination.rows()
        );
        assert_eq!(joined.len(), 3);
        assert_eq!(joined.rows()[2].occurrence_id().ordinal(), 2);
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
