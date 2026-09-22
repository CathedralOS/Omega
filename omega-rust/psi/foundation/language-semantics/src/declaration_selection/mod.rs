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
    /// A declaration named by a domain's issuer-authorization catalog.
    /// Public exposure retains interface identity and dependencies, but does
    /// not make the selected issuer nameable as ordinary consumer API.
    DomainIssuerAuthorization,
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

impl CollectionViewOperation {
    /// Every compiler-owned view operation. Adding a variant without its
    /// spelling stops compiling here rather than silently dropping out of the
    /// spelling map.
    pub const ALL: [Self; 4] = [
        Self::SharedSlice,
        Self::MutableSlice,
        Self::TextView,
        Self::Bytes,
    ];

    /// The authored method spelling which selects this operation.
    ///
    /// The spelling is authored vocabulary, so it belongs to the vocabulary
    /// that owns the operation. Checking selects the operation once and every
    /// later consumer reads the retained identity; only a consumer running
    /// before that selection is recorded -- or one which cannot reach the
    /// selection ledger -- asks about spelling, and it asks here.
    pub const fn authored_spelling(self) -> &'static str {
        match self {
            Self::SharedSlice => "as_slice",
            Self::MutableSlice => "as_mut_slice",
            Self::TextView => "as_view",
            Self::Bytes => "bytes",
        }
    }

    /// The view operation an authored method spelling selects, or `None` when
    /// the spelling names no compiler-owned view.
    ///
    /// A matching spelling is a necessary condition, never a sufficient one:
    /// a declared machine may be spelled `as_slice` too. The caller still owes
    /// the call-shape and receiver-type conditions which separate the
    /// compiler-owned operation from an ordinary call of the same name.
    pub fn from_authored_spelling(spelling: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|operation| operation.authored_spelling() == spelling)
    }
}

/// One compiler-owned standing measure of a collection carrier: the `len`
/// or `capacity` projection on a fixed array or slice.
///
/// A measure is compiler-owned value metadata, never a declaration selected
/// from the package namespace: no package declares `len`, and a record field
/// that happens to be spelled `len` is that record's own field. Checking
/// records the measure once as [`AuthoredDeclarationSelectionIntrinsic`]
/// (see [`Self::intrinsic`]); this map is the only place the two spellings
/// live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionMeasure {
    /// The live element count.
    Length,
    /// The backing extent a constrained carrier can grow into.
    Capacity,
}

impl CollectionMeasure {
    /// Every compiler-owned measure. Adding a variant without its spelling
    /// stops compiling here rather than silently dropping out of the map.
    pub const ALL: [Self; 2] = [Self::Length, Self::Capacity];

    /// The authored member spelling which selects this measure.
    ///
    /// The spelling is authored vocabulary, so it belongs to the vocabulary
    /// that owns the measure. A consumer running before checking has recorded
    /// the selection -- or one which cannot reach the selection ledger --
    /// asks about spelling, and it asks here.
    pub const fn authored_spelling(self) -> &'static str {
        match self {
            Self::Length => "len",
            Self::Capacity => "capacity",
        }
    }

    /// The measure an authored member spelling selects, or `None` when the
    /// spelling names no compiler-owned measure.
    ///
    /// A matching spelling is a necessary condition, never a sufficient one:
    /// a declared field may be spelled `len` too. The caller still owes the
    /// receiver-type condition which separates the compiler-owned measure of
    /// a fixed array or slice from a same-named field of a record.
    pub fn from_authored_spelling(spelling: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|measure| measure.authored_spelling() == spelling)
    }

    /// The intrinsic selection target checking records for this measure.
    pub const fn intrinsic(self) -> AuthoredDeclarationSelectionIntrinsic {
        match self {
            Self::Length => AuthoredDeclarationSelectionIntrinsic::CollectionLength,
            Self::Capacity => AuthoredDeclarationSelectionIntrinsic::CollectionCapacity,
        }
    }
}

/// One toolchain-owned build operation authored syntax selects on the root
/// build vocabulary (`Build`, `BuildOutput`, `BuildLog`, `Optimizations`).
///
/// No package declares these methods: the toolchain's build prelude owns
/// them, and Psi checking classifies each authored use as the matching
/// [`AuthoredDeclarationSelectionIntrinsic`] (see [`Self::intrinsic`]). The
/// spelling is the call target the trees retain, and this map is the only
/// place those spellings live. Provider selection, optimization policy, wire
/// compatibility and boundary grants are then harvested and enforced by
/// Omega's build evaluation; the map classifies declaration provenance only.
///
/// Omega's `build.omg` dependency-row grammar (`depend`, `depend_as`, ...)
/// is a different vocabulary and stays with Omega's build declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildOperation {
    /// `b.select_provider<Subject, Product>()` on the mutable root `Build`.
    ProviderSelection,
    /// `select_representation<Type, Representation>()`.
    RepresentationSelection,
    /// `b.exclude_service<BoundaryTrait>()`.
    ServiceExclusion,
    /// `Optimizations::enable`.
    OptimizationSelection,
    /// `Optimizations::emit_report`.
    OptimizationReportRequest,
    /// `b.accept_boundary<pkg::symbol>()`, retained as the
    /// `accept_boundary#<path>` marker.
    BoundaryAcceptance,
    /// `b.require_wire_compatibility<Edge, Lineage, Local, Peer, Fact..>()`,
    /// retained as the `wire_compatibility#<operands>` marker.
    WireCompatibilityRequest,
    /// `BuildOutput::include_source`.
    IncludedSourceHandoff,
    /// `BuildLog::write_line`.
    LogWriteLine,
}

impl BuildOperation {
    /// The separator after which a marker-carrying operation's angle-bracket
    /// operands follow in its retained call target. `#` cannot appear in an
    /// identifier, so a marker never collides with a declared method.
    pub const MARKER_OPERAND_SEPARATOR: char = '#';

    /// Every toolchain-owned build operation. Adding a variant without its
    /// spelling stops compiling here rather than silently dropping out of
    /// the map.
    pub const ALL: [Self; 9] = [
        Self::ProviderSelection,
        Self::RepresentationSelection,
        Self::ServiceExclusion,
        Self::OptimizationSelection,
        Self::OptimizationReportRequest,
        Self::BoundaryAcceptance,
        Self::WireCompatibilityRequest,
        Self::IncludedSourceHandoff,
        Self::LogWriteLine,
    ];

    /// The call-target spelling which selects this operation.
    ///
    /// The spelling is authored vocabulary, so it belongs to the vocabulary
    /// that owns the operation. For a marker-carrying operation
    /// ([`Self::carries_marker_operands`]) it is the marker prefix the parser
    /// retains ahead of [`Self::MARKER_OPERAND_SEPARATOR`], which for the
    /// wire-compatibility request differs from the authored method name
    /// `require_wire_compatibility`; everywhere else it is the authored
    /// method name itself.
    pub const fn authored_spelling(self) -> &'static str {
        match self {
            Self::ProviderSelection => "select_provider",
            Self::RepresentationSelection => "select_representation",
            Self::ServiceExclusion => "exclude_service",
            Self::OptimizationSelection => "enable",
            Self::OptimizationReportRequest => "emit_report",
            Self::BoundaryAcceptance => "accept_boundary",
            Self::WireCompatibilityRequest => "wire_compatibility",
            Self::IncludedSourceHandoff => "include_source",
            Self::LogWriteLine => "write_line",
        }
    }

    /// Whether the parser retains this operation's angle-bracket operands in
    /// the call target after [`Self::MARKER_OPERAND_SEPARATOR`]
    /// (`accept_boundary#pkg::symbol`), so a call target selects the
    /// operation by prefix rather than by exact spelling.
    pub const fn carries_marker_operands(self) -> bool {
        matches!(
            self,
            Self::BoundaryAcceptance | Self::WireCompatibilityRequest
        )
    }

    /// The build operation a retained call target selects, or `None` when
    /// the target names no toolchain-owned build operation.
    ///
    /// An exact spelling selects an ordinary operation; a marker-carrying
    /// operation is selected by its spelling followed by the separator and
    /// its operands. A matching spelling is a necessary condition, never a
    /// sufficient one: a declared machine may be spelled `enable` too. The
    /// caller still owes the receiver and call-shape conditions which
    /// separate the toolchain-owned operation from an ordinary call of the
    /// same name.
    pub fn from_call_target(target: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|operation| {
            let spelling = operation.authored_spelling();
            if operation.carries_marker_operands() {
                target
                    .strip_prefix(spelling)
                    .is_some_and(|rest| rest.starts_with(Self::MARKER_OPERAND_SEPARATOR))
            } else {
                target == spelling
            }
        })
    }

    /// The operands a marker-carrying operation retained after its spelling
    /// and [`Self::MARKER_OPERAND_SEPARATOR`] in `target`, or `None` when the
    /// target does not select this operation or the operation carries no
    /// marker operands. Consumers that read the operands (a root grant path,
    /// a wire-compatibility demand) ask here instead of stripping the prefix
    /// themselves, so the spelling stays in one place.
    pub fn marker_operands(self, target: &str) -> Option<&str> {
        if !self.carries_marker_operands() {
            return None;
        }
        target
            .strip_prefix(self.authored_spelling())?
            .strip_prefix(Self::MARKER_OPERAND_SEPARATOR)
    }

    /// The intrinsic selection target checking records for this operation.
    pub const fn intrinsic(self) -> AuthoredDeclarationSelectionIntrinsic {
        match self {
            Self::ProviderSelection => {
                AuthoredDeclarationSelectionIntrinsic::BuildProviderSelection
            }
            Self::RepresentationSelection => {
                AuthoredDeclarationSelectionIntrinsic::BuildRepresentationSelection
            }
            Self::ServiceExclusion => AuthoredDeclarationSelectionIntrinsic::BuildServiceExclusion,
            Self::OptimizationSelection => {
                AuthoredDeclarationSelectionIntrinsic::BuildOptimizationSelection
            }
            Self::OptimizationReportRequest => {
                AuthoredDeclarationSelectionIntrinsic::BuildOptimizationReportRequest
            }
            Self::BoundaryAcceptance => {
                AuthoredDeclarationSelectionIntrinsic::BuildBoundaryAcceptance
            }
            Self::WireCompatibilityRequest => {
                AuthoredDeclarationSelectionIntrinsic::BuildWireCompatibilityRequest
            }
            Self::IncludedSourceHandoff => {
                AuthoredDeclarationSelectionIntrinsic::BuildIncludedSourceHandoff
            }
            Self::LogWriteLine => AuthoredDeclarationSelectionIntrinsic::BuildLogWriteLine,
        }
    }
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
    /// The `capacity` projection on a fixed array or slice. Collection
    /// capacity is compiler-owned value metadata alongside length, not a
    /// declaration selected from the package namespace.
    CollectionCapacity,
    /// A checked compiler-owned collection/text view operation.
    CollectionView(CollectionViewOperation),
    /// One exact compiler-owned byte-sequence predicate. Retaining the
    /// particular predicate prevents later evidence consumers from having to
    /// reconstruct semantic identity from the call's diagnostic spelling.
    ByteSequencePredicate(crate::byte_predicates::ByteSequencePredicate),
    BuildProviderSelection,
    BuildRepresentationSelection,
    /// `b.exclude_service<BoundaryTrait>()`: a build behavior exclusion of
    /// one abstract service (wiki/spec/build/behavior_exclusions.md). The
    /// marker classifies declaration provenance only; the admission
    /// requirement it selects is harvested and enforced by Omega's build
    /// evaluation and product admission.
    BuildServiceExclusion,
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
    /// A contract-fact call whose spelled name selects no package
    /// declaration. Undeclared proof views (`Seq`/`Bag`/`Range`-style
    /// schematic atoms) are admitted as opaque proof terms: they carry no
    /// machine realization, numeric interpretation, or equality semantics.
    /// The ledger retains their provenance as a compiler-owned admission so
    /// a successful selection never stays unresolved and admission never
    /// invents a declaration symbol.
    ProofView,
    /// The sealed `Quotient::define<F, Congruence>(..)` request. The
    /// namespace is compiler vocabulary, not a package declaration; the
    /// representative and theorem selections it names are retained as
    /// ordinary static-argument selections. Resolution is proof-only: the
    /// request binds no executable call.
    QuotientDefine,
    /// The sealed `Quotient::lift<F, Congruence[, Transport]>(..)` request;
    /// see [`Self::QuotientDefine`].
    QuotientLift,
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
mod tests;
