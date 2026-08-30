//! Psi-owned checked/typed-tree interpreter used for build-time evaluation and
//! as a differential oracle while the terminal producer grows.
//!
//! Canonical portable execution belongs to the Psi-owned
//! `psi-terminal-interpreter` crate. This crate retains the source-shaped
//! evaluator for constructs not yet represented in terminal Psi.
//!
//! The interpreter evaluates the program at the level of the typed/checked trees
//! (`psi_checked_trees::CheckedTrees`, which derefs to `psi_typed_trees::TypedTrees`)
//! -- above all backend lowering. It is therefore independent of the backend bugs
//! it must catch: if [`interpret_entry`] and the native binary disagree on exit
//! code or stdout for the same checked program, the backend is wrong.
//!
//! ## Value & store model (the crux: aliasing)
//! Every storage place -- local, struct field, machine instance -- is an
//! allocation-owned [`value::Cell`]. A `&mut place` argument evaluates to a
//! [`Value::Ref`] holding a clone of the *same* cell, so a write through the reference
//! mutates the original cell. Multi-level `&mut` aliasing is therefore correct by
//! construction -- this is exactly the property the native backend is known to get
//! wrong (an `&mut`-write through a call chain that does not persist). Once the
//! interpreter's coverage reaches such a program, an `interpret_entry != native`
//! mismatch localizes the bug instantly.
//!
//! ## Execution model
//! [`interpret_entry`] runs the exact machine selected by its caller. The
//! interpreter neither chooses a conventional entry spelling nor searches alternate
//! names. A machine instance is a [`Value::Struct`] with default-initialized
//! fields. A state has parameters + a sequence of statements + guarded transitions; the
//! first transition whose guard holds determines the next state (or the returned value /
//! terminal). Host-boundary calls (`exit_process`, `write`, `write_line`) on a
//! `boundary trait` machine drive exit code / stdout.
//!
//! ## Scope
//! Supported: multiple machines with per-instance contained sub-objects; symbol/group-based
//! machine + sibling-state resolution; self-field assignment; `let` locals; Integer / Bool /
//! Float / Binary (arith + compare + logical) / Unary / Name / Member / Indexed / Cast /
//! ArrayLiteral / StructLiteral expressions; fixed arrays and `.as_slice()`/`.as_mut_slice()`
//! slice views (a slice shares the array's element cells, preserving `&mut` aliasing);
//! width/signedness-aware `as` casts (int<->float, integer narrow/widen); multi-arm
//! value/guard transitions (subject, tuple, and boolean forms); value-calls returning a
//! scalar/struct; method calls on `&mut Data` reference params; `&mut`-aliased argument
//! passing, including MULTI-HOP forwarding (a `&mut` param passed onward as a bare name --
//! to a nested call or a transition-target state -- stays aliased, hop after hop);
//! `dyn Trait` dispatch by the receiver's RUNTIME type (works for any number of
//! impls -- AHEAD of the native backend, which only devirtualizes single-impl traits); the
//! transition guard SUBJECTS evaluate exactly once per transition evaluation (the
//! parser copies the subject call into every arm's guard; the per-frame memo reuses
//! the first arm's result instead of re-running the callee's side effects, matching
//! the native lowering's shared branch prelude); the
//! entry machine's value as the exit code; the Console boundary `exit_process`,
//! `write`/`write_line`, `write_error`/`write_error_line` (collected on a separate
//! stderr stream), and `read_line` (consuming `stdin`), including the imported std
//! `console`. The full `dungeon_crawler_cli` sample interprets end-to-end with
//! depth-correct room rendering. Anything outside this subset returns
//! [`InterpretOutcome::error`] so a differential harness SKIPS (xfail) rather than reporting
//! a false mismatch.
//!
//! CASE PAYLOADS are supported in BOTH engines: construction (`Command::Move
//! { steps: 70 }`, the brace spelling shared with record literals), case-pattern
//! binding in transition arms (`Command::Move { steps } -> done(steps)`, with the
//! bound names rewritten to payload member reads), and tag compares against case
//! references (the lowering of `in` and of payload-less case `==`, matching the
//! native 4-byte tag clamp). Structural `==` on CONFORMING types (`Type
//! satisfies Equatable;`) is expanded by the FRONTEND into ordinary field
//! compares and tag-guarded payload compares before either engine runs, so the
//! interpreter's `Value::Enum` equality stays a tag compare -- by the time a
//! payload matters, the expansion already reads it field by field. Expression
//! `&&`/`||` SHORT-CIRCUIT (the expansion relies on it to keep cross-case
//! payload reads unevaluated; the native backend evaluates eagerly but masks
//! the garbage compare behind the false tag guard). Never-assigned sum fields
//! default to the ZII zero case (first case, zeroed payload), matching native
//! zero-initialized storage. The native backend lowers construction as a
//! tag-prefix write plus payload field writes, so payload coverage runs
//! differentially via the `data/case_*` and `traits/equatable_*` RUN canaries
//! (plus the deeper probes in `tests/coverage.rs`).
//!
//! One formerly-deferred construct is FRONTEND-REJECTED today (probed in
//! `tests/coverage.rs`), so there is nothing to interpret:
//! - General/open range expressions outside the index position (`let r: i32 = 1..5;`,
//!   `f(1..5)`) are parse errors; `ExpressionNode::Range` only ever appears under
//!   `collection[...]`, which the subslice support already covers.
//! - (A paren'd construction against a payload-less case (`E::A(5)`) still parses as a
//!   CALL but resolves to nothing; the interpreter declines it.)

mod build_evaluation_sponsor;
mod build_time;
mod evaluator;
mod filesystem_replay;
mod filesystem_sponsor;
mod value;

pub use build_evaluation_sponsor::{
    BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION, BuildEvaluationSponsor,
    BuildEvaluationSponsorLimits,
};
pub use build_time::BuildTimeValue;
pub use filesystem_replay::{
    FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_MODE, FilesystemInputOutputAbsentRemovesReplayRecord,
    FilesystemInputOutputDirectoryReplayRecord, FilesystemInputOutputTreeReplayRecord,
    FilesystemInputUnknownDescriptorGetOsfHandleReplayRecord,
    FilesystemInputUnknownDescriptorOpenAtReplayRecord,
    FilesystemInputUnknownDescriptorOperationReplayKind,
    FilesystemInputUnknownDescriptorOperationReplayRecord,
    FilesystemInputUnknownDescriptorReadFileMetadataReplayRecord,
    FilesystemInputUnknownDescriptorReadReplayKind,
    FilesystemInputUnknownDescriptorReadReplayRecord,
    FilesystemInputUnknownDescriptorSeekReplayRecord,
    FilesystemInputUnknownDescriptorSetFileTimesReplayRecord,
    FilesystemInputUnknownDescriptorUnlinkAtReplayRecord,
    FilesystemInputUnknownDescriptorWriteOperationReplayKind,
    FilesystemInputUnknownDescriptorWriteOperationReplayRecord,
    FilesystemInputUnknownDescriptorWriteReplayKind,
    FilesystemInputUnknownDescriptorWriteReplayRecord,
    FilesystemInputUnknownNativeHandleCloseHandleReplayRecord,
    FilesystemInputUnknownNativeHandleFinalPathNameByHandleReplayRecord,
    FilesystemInputUnknownNativeHandleMutationReplayKind,
    FilesystemInputUnknownNativeHandleMutationReplayRecord, FilesystemOutputAbsentRemoveKind,
    FilesystemOutputAbsentRemoveReplayRecord, FilesystemOutputDirectoryReplayRecord,
    FilesystemOutputDuplicateReplayRecord, FilesystemOutputHardLinkReplayKind,
    FilesystemOutputHardLinkReplayRecord, FilesystemOutputLockReplayRecord,
    FilesystemOutputSymlinkReplayRecord, FilesystemOutputTreeEntryReplayRecord,
    FilesystemSourceDirectoryReadChainReplayRecord, FilesystemSourceDirectoryReadReplayRecord,
    FilesystemSourceReadLinkReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_ABSENT_REMOVES,
    MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES, MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES,
    MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES,
    MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES, MAX_FILESYSTEM_REPLAY_OUTPUT_LOCK_PAIRS,
    MAX_FILESYSTEM_REPLAY_OUTPUT_SYMLINK_TARGET_BYTES,
};
use filesystem_replay::{
    output_absent_remove_attempt, output_absent_remove_record_from_attempt,
    output_directory_attempt, output_directory_record_from_attempt, output_duplicate_attempts,
    output_duplicate_record_from_attempts, output_hard_link_attempt,
    output_hard_link_record_from_attempt, output_lock_attempts, output_lock_record_from_attempts,
    output_logical_handle_identities, output_symlink_attempt, output_symlink_record_from_attempt,
    source_attempts_use_root, source_directory_chain_attempts, source_directory_chain_is_exact,
    source_read_link_attempt, source_read_link_attempt_is_exact,
    unknown_input_handle_failure_attempt_is_exact, validate_observed_output_tree_records,
    validate_output_duplicate_replay, validate_output_lock_replay,
};
pub use filesystem_sponsor::{
    COMPILER_DEFAULT_STAGING_ENTRY_LIMIT, COMPILER_DEFAULT_STAGING_MAX_OBJECT_EXTENT,
    COMPILER_DEFAULT_STAGING_TOTAL_LOGICAL_BYTES, FilesystemOpenDescriptor, FilesystemSponsor,
    FilesystemSponsorEntry, FilesystemSponsorError, FilesystemSponsorLimits,
    FilesystemSponsorNamespaceEntry, FilesystemSponsorNamespaceEntryKind,
    FilesystemSponsorNamespaceSnapshot, FilesystemSponsorPath, FilesystemSponsorSnapshot,
    PreparedFilesystemMutation, PreparedFilesystemOpen, PreparedFilesystemWrite,
};
pub use value::{Cell, Value};

use psi_checked_trees::CheckedTrees;

/// Identity of the deterministic evaluator-step schedule used before the
/// canonical portable IR exists.
///
/// This is deliberately distinct from canonical-IR `FuelScheduleIdentity`.
/// Its marker names the current TypedTrees interpreter's accounting precursor and
/// must not be used as an IR-derived fixed-work certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvaluationStepScheduleIdentity(u32);

impl EvaluationStepScheduleIdentity {
    pub const fn marker(self) -> u32 {
        self.0
    }
}

/// The current deterministic evaluator-step schedule charges one unit for each
/// entered state, executed statement, and evaluated expression.
pub const CURRENT_EVALUATION_STEP_SCHEDULE: EvaluationStepScheduleIdentity =
    EvaluationStepScheduleIdentity(1);

/// Version of the canonical evaluator usage-record schema. This is distinct
/// from the step schedule: adding an attributed count changes the record shape
/// without changing the meaning or weight of an evaluator step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationUsageSchemaIdentity(u32);

impl EvaluationUsageSchemaIdentity {
    pub const fn schema_version(self) -> u32 {
        self.0
    }
}

pub const CURRENT_EVALUATION_USAGE_SCHEMA: EvaluationUsageSchemaIdentity =
    EvaluationUsageSchemaIdentity(7);

/// Deterministic work measured by the current evaluator-step schedule.
///
/// The fields are private so future attributed telemetry can extend this
/// record without allowing callers to fabricate usage. The evaluated program
/// cannot observe this record or its remaining sponsor allowance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationUsage {
    schema: EvaluationUsageSchemaIdentity,
    schedule: EvaluationStepScheduleIdentity,
    fuel_units: u64,
    fuel_ceiling: u64,
    build_log_bytes: u64,
    filesystem_operation_attempts: u64,
    peak_live_cells: u64,
    peak_live_text_bytes: u64,
    result_cells: u64,
    result_text_bytes: u64,
}

/// Schema for the current incomplete filesystem operation-attempt evidence.
///
/// This records call-start order, exact provider, every successfully authorized
/// scoped path as a grant-root identity plus canonical relative UTF-8 bytes,
/// exact path-like byte operands, each successfully resolved mutable carrier
/// and logical-handle input even when later preparation fails, and a typed
/// returned or evaluator-halted outcome. Exact path results and successful file
/// and directory observation regions plus canonical metadata values are
/// designated, but replay execution is not complete yet.
pub const FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION: u32 = 19;

/// One semantic field in the canonical metadata value returned by the
/// filesystem host seam. This is target-neutral vocabulary; the selected
/// checked layout supplies only its physical offset and stored width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FilesystemMetadataField {
    Device,
    Mode,
    LinkCount,
    Inode,
    User,
    Group,
    ReferencedDevice,
    AccessTime,
    ModificationTime,
    ChangeTime,
    BirthTime,
    Size,
    Blocks512,
    PreferredBlockSize,
}

impl FilesystemMetadataField {
    pub const ALL: [Self; 14] = [
        Self::Device,
        Self::Mode,
        Self::LinkCount,
        Self::Inode,
        Self::User,
        Self::Group,
        Self::ReferencedDevice,
        Self::AccessTime,
        Self::ModificationTime,
        Self::ChangeTime,
        Self::BirthTime,
        Self::Size,
        Self::Blocks512,
        Self::PreferredBlockSize,
    ];

    pub const fn semantic_width_bits(self) -> u16 {
        match self {
            Self::Mode | Self::User | Self::Group => 32,
            Self::Device
            | Self::LinkCount
            | Self::Inode
            | Self::ReferencedDevice
            | Self::AccessTime
            | Self::ModificationTime
            | Self::ChangeTime
            | Self::BirthTime
            | Self::Size
            | Self::Blocks512
            | Self::PreferredBlockSize => 64,
        }
    }

    pub const fn is_signed(self) -> bool {
        matches!(
            self,
            Self::AccessTime
                | Self::ModificationTime
                | Self::ChangeTime
                | Self::BirthTime
                | Self::Size
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemMetadataFieldLayout {
    field: FilesystemMetadataField,
    offset: usize,
    stored_width_bits: u16,
}

impl FilesystemMetadataFieldLayout {
    pub const fn new(
        field: FilesystemMetadataField,
        offset: usize,
        stored_width_bits: u16,
    ) -> Self {
        Self {
            field,
            offset,
            stored_width_bits,
        }
    }

    pub const fn field(self) -> FilesystemMetadataField {
        self.field
    }

    pub const fn offset(self) -> usize {
        self.offset
    }

    pub const fn stored_width_bits(self) -> u16 {
        self.stored_width_bits
    }
}

/// Checked physical carrier geometry for one selected target's `StatRecord`.
///
/// Omega orchestration derives this from the already-evaluated programmable
/// layout and supplies it to Psi. Package strings and raw target IR never enter
/// the interpreter. Construction rejects missing, duplicate, overlapping,
/// over-wide, and out-of-record fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemMetadataLayout {
    record_size: usize,
    fields: Vec<FilesystemMetadataFieldLayout>,
}

impl FilesystemMetadataLayout {
    pub fn new(
        record_size: usize,
        mut fields: Vec<FilesystemMetadataFieldLayout>,
    ) -> Result<Self, String> {
        if record_size == 0 {
            return Err("filesystem metadata layout has an empty record".to_owned());
        }
        fields.sort_unstable_by_key(|field| field.field);
        if fields.len() != FilesystemMetadataField::ALL.len()
            || fields
                .iter()
                .map(|field| field.field)
                .ne(FilesystemMetadataField::ALL)
        {
            return Err(
                "filesystem metadata layout must contain each canonical field exactly once"
                    .to_owned(),
            );
        }
        for field in &fields {
            if !matches!(field.stored_width_bits, 16 | 32 | 64)
                || field.stored_width_bits > field.field.semantic_width_bits()
            {
                return Err(format!(
                    "filesystem metadata field {:?} has invalid stored width {}",
                    field.field, field.stored_width_bits
                ));
            }
            let width = usize::from(field.stored_width_bits / 8);
            let end = field.offset.checked_add(width).ok_or_else(|| {
                format!(
                    "filesystem metadata field {:?} extent overflows",
                    field.field
                )
            })?;
            if end > record_size {
                return Err(format!(
                    "filesystem metadata field {:?} ends at {end}, beyond record size {record_size}",
                    field.field
                ));
            }
        }
        for (index, left) in fields.iter().enumerate() {
            let left_end = left.offset + usize::from(left.stored_width_bits / 8);
            for right in fields.iter().skip(index + 1) {
                let right_end = right.offset + usize::from(right.stored_width_bits / 8);
                if left.offset < right_end && right.offset < left_end {
                    return Err(format!(
                        "filesystem metadata fields {:?} and {:?} overlap",
                        left.field, right.field
                    ));
                }
            }
        }
        Ok(Self {
            record_size,
            fields,
        })
    }

    pub const fn record_size(&self) -> usize {
        self.record_size
    }

    pub fn field_layout(&self, field: FilesystemMetadataField) -> FilesystemMetadataFieldLayout {
        *self
            .fields
            .iter()
            .find(|layout| layout.field == field)
            .expect("validated metadata layout contains every field")
    }

    fn host() -> Self {
        let fields = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            vec![
                (FilesystemMetadataField::Device, 0, 64),
                (FilesystemMetadataField::Mode, 24, 32),
                (FilesystemMetadataField::LinkCount, 16, 64),
                (FilesystemMetadataField::Inode, 8, 64),
                (FilesystemMetadataField::User, 28, 32),
                (FilesystemMetadataField::Group, 32, 32),
                (FilesystemMetadataField::ReferencedDevice, 40, 64),
                (FilesystemMetadataField::AccessTime, 72, 64),
                (FilesystemMetadataField::ModificationTime, 88, 64),
                (FilesystemMetadataField::ChangeTime, 104, 64),
                (FilesystemMetadataField::BirthTime, 120, 64),
                (FilesystemMetadataField::Size, 48, 64),
                (FilesystemMetadataField::Blocks512, 64, 64),
                (FilesystemMetadataField::PreferredBlockSize, 56, 64),
            ]
        } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            vec![
                (FilesystemMetadataField::Device, 0, 64),
                (FilesystemMetadataField::Mode, 16, 32),
                (FilesystemMetadataField::LinkCount, 20, 32),
                (FilesystemMetadataField::Inode, 8, 64),
                (FilesystemMetadataField::User, 24, 32),
                (FilesystemMetadataField::Group, 28, 32),
                (FilesystemMetadataField::ReferencedDevice, 32, 64),
                (FilesystemMetadataField::AccessTime, 72, 64),
                (FilesystemMetadataField::ModificationTime, 88, 64),
                (FilesystemMetadataField::ChangeTime, 104, 64),
                (FilesystemMetadataField::BirthTime, 120, 64),
                (FilesystemMetadataField::Size, 48, 64),
                (FilesystemMetadataField::Blocks512, 64, 64),
                (FilesystemMetadataField::PreferredBlockSize, 56, 32),
            ]
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            vec![
                (FilesystemMetadataField::Device, 0, 32),
                (FilesystemMetadataField::Mode, 4, 16),
                (FilesystemMetadataField::LinkCount, 6, 16),
                (FilesystemMetadataField::Inode, 8, 64),
                (FilesystemMetadataField::User, 16, 32),
                (FilesystemMetadataField::Group, 20, 32),
                (FilesystemMetadataField::ReferencedDevice, 24, 32),
                (FilesystemMetadataField::AccessTime, 32, 64),
                (FilesystemMetadataField::ModificationTime, 48, 64),
                (FilesystemMetadataField::ChangeTime, 64, 64),
                (FilesystemMetadataField::BirthTime, 80, 64),
                (FilesystemMetadataField::Size, 96, 64),
                (FilesystemMetadataField::Blocks512, 104, 64),
                (FilesystemMetadataField::PreferredBlockSize, 112, 32),
            ]
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            vec![
                (FilesystemMetadataField::Device, 0, 32),
                (FilesystemMetadataField::Mode, 6, 16),
                (FilesystemMetadataField::LinkCount, 8, 16),
                (FilesystemMetadataField::Inode, 64, 64),
                (FilesystemMetadataField::User, 72, 32),
                (FilesystemMetadataField::Group, 76, 32),
                (FilesystemMetadataField::ReferencedDevice, 16, 32),
                (FilesystemMetadataField::AccessTime, 32, 64),
                (FilesystemMetadataField::ModificationTime, 40, 64),
                (FilesystemMetadataField::ChangeTime, 80, 64),
                (FilesystemMetadataField::BirthTime, 48, 64),
                (FilesystemMetadataField::Size, 24, 64),
                (FilesystemMetadataField::Blocks512, 88, 64),
                (FilesystemMetadataField::PreferredBlockSize, 96, 32),
            ]
        } else {
            panic!("unsupported host filesystem metadata layout")
        };
        let record_size = if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            128
        } else {
            144
        };
        Self::new(
            record_size,
            fields
                .into_iter()
                .map(|(field, offset, width)| {
                    FilesystemMetadataFieldLayout::new(field, offset, width)
                })
                .collect(),
        )
        .expect("host metadata layout is canonical")
    }
}

impl Default for FilesystemMetadataLayout {
    fn default() -> Self {
        Self::host()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemObservationProvider {
    /// Deterministic in-memory provider; no host filesystem was touched.
    Virtual,
    /// Real process filesystem without path grants. Build admission does not
    /// select this provider.
    RealUnscoped,
    /// Real filesystem constrained by compiler-supplied path grants.
    RealScoped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemGrantAccess {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemGrantRefusalReason {
    Unresolvable,
    OutsideGrantedRoots,
    UnrepresentableRootedPath,
    ObservationEvidenceLimitExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemGrantRefusal {
    operand_ordinal: u8,
    access: FilesystemGrantAccess,
    reason: FilesystemGrantRefusalReason,
}

/// Compiler-issued identity for one scoped filesystem grant root.
///
/// The checked interpreter treats this as an opaque coordinate. The caller
/// owns its meaning (for example, Omega assigns distinct identities to the
/// immutable package source and writable build output roots). Zero is reserved
/// so an omitted/default identity cannot enter evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FilesystemGrantRootIdentity(u32);

/// Current compiler sponsorship ceiling for one canonical path beneath a
/// filesystem grant root. This is an evaluator evidence limit, not a language
/// limit.
pub const FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT: usize = 16 * 1024 * 1024;

/// Whether bytes are the canonical target-neutral spelling of a path beneath
/// one grant root. Authorized targets may denote the root itself; authored
/// rooted operands may not.
pub fn filesystem_root_relative_path_is_canonical(relative: &[u8], allow_empty: bool) -> bool {
    if relative.len() > FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT
        || relative.contains(&0)
        || std::str::from_utf8(relative).is_err()
    {
        return false;
    }
    if relative.is_empty() {
        return allow_empty;
    }
    if relative[0] == b'/'
        || relative.contains(&b'\\')
        || (relative.len() >= 2 && relative[1] == b':')
    {
        return false;
    }
    !relative
        .split(|byte| *byte == b'/')
        .any(|component| component.is_empty() || component == b"." || component == b"..")
}

/// Version of Psi's canonical immutable-source metadata policy.
pub const CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION: u32 = 1;

/// Maximum complete source-tree rows accepted by the canonical metadata
/// carrier. One row is reserved for the source root; the remaining 65,536
/// match the package resolver's source-entry ceiling.
pub const CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT: usize = 65_537;

/// Whether raw bytes are one canonical slash-separated source-tree coordinate.
///
/// Unlike runtime rooted-path evidence, the complete source index deliberately
/// does not require UTF-8. Source custody preserves otherwise valid raw Unix
/// names even when package code cannot express those names through Psi's
/// target-neutral runtime path gate.
pub fn canonical_filesystem_metadata_path_is_canonical(relative: &[u8], allow_empty: bool) -> bool {
    if relative.len() > FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT
        || relative.contains(&0)
        || relative.contains(&b'\\')
    {
        return false;
    }
    if relative.is_empty() {
        return allow_empty;
    }
    if relative[0] == b'/' {
        return false;
    }
    !relative
        .split(|byte| *byte == b'/')
        .any(|component| component.is_empty() || component == b"." || component == b"..")
}

/// Closed source entry shape from which package-visible metadata is derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalFilesystemMetadataRowKind {
    Directory,
    File {
        executable: bool,
        logical_byte_length: u64,
    },
    Symlink {
        target_spelling_logical_byte_length: u64,
    },
}

impl CanonicalFilesystemMetadataRowKind {
    pub const fn logical_byte_length(self) -> u64 {
        match self {
            Self::Directory => 0,
            Self::File {
                logical_byte_length,
                ..
            } => logical_byte_length,
            Self::Symlink {
                target_spelling_logical_byte_length,
            } => target_spelling_logical_byte_length,
        }
    }
}

/// One raw root-relative row in a canonical immutable-source metadata index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalFilesystemMetadataRow {
    relative_path: Vec<u8>,
    kind: CanonicalFilesystemMetadataRowKind,
}

impl CanonicalFilesystemMetadataRow {
    pub fn new(
        relative_path: impl Into<Vec<u8>>,
        kind: CanonicalFilesystemMetadataRowKind,
    ) -> Self {
        Self {
            relative_path: relative_path.into(),
            kind,
        }
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }

    pub const fn kind(&self) -> CanonicalFilesystemMetadataRowKind {
        self.kind
    }
}

/// Why compiler-supplied immutable-source metadata is not canonical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalFilesystemMetadataIndexError {
    UnsupportedPolicyVersion(u32),
    RowLimitExceeded { limit: usize, attempted: usize },
    InvalidRelativePath(Vec<u8>),
    DuplicateRelativePath(Vec<u8>),
    AggregatePathBytesLimitExceeded { limit: usize, attempted: usize },
    LogicalByteLengthExceedsI64(Vec<u8>),
    MissingRootDirectory,
    RootIsNotDirectory,
    MissingParentDirectory(Vec<u8>),
    ParentIsNotDirectory(Vec<u8>),
}

impl std::fmt::Display for CanonicalFilesystemMetadataIndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPolicyVersion(version) => {
                write!(
                    formatter,
                    "unsupported canonical filesystem metadata policy {version}"
                )
            }
            Self::RowLimitExceeded { limit, attempted } => write!(
                formatter,
                "canonical filesystem metadata rows exceed {limit}: attempted {attempted}"
            ),
            Self::InvalidRelativePath(path) => write!(
                formatter,
                "canonical filesystem metadata contains an invalid relative path: {path:?}"
            ),
            Self::DuplicateRelativePath(path) => write!(
                formatter,
                "canonical filesystem metadata duplicates relative path: {path:?}"
            ),
            Self::AggregatePathBytesLimitExceeded { limit, attempted } => write!(
                formatter,
                "canonical filesystem metadata path bytes exceed {limit}: attempted {attempted}"
            ),
            Self::LogicalByteLengthExceedsI64(path) => write!(
                formatter,
                "canonical filesystem metadata length does not fit i64 at path: {path:?}"
            ),
            Self::MissingRootDirectory => {
                write!(
                    formatter,
                    "canonical filesystem metadata omits the root directory"
                )
            }
            Self::RootIsNotDirectory => {
                write!(
                    formatter,
                    "canonical filesystem metadata root is not a directory"
                )
            }
            Self::MissingParentDirectory(path) => write!(
                formatter,
                "canonical filesystem metadata omits a parent directory for path: {path:?}"
            ),
            Self::ParentIsNotDirectory(path) => write!(
                formatter,
                "canonical filesystem metadata parent is not a directory for path: {path:?}"
            ),
        }
    }
}

impl std::error::Error for CanonicalFilesystemMetadataIndexError {}

/// Immutable, validated metadata for one complete content-authenticated source.
///
/// The source-content commitment is deliberately opaque to Psi. The package
/// resolver owns its construction and the compiler binds it to the source
/// identity; the interpreter only enforces the closed metadata policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalFilesystemMetadataIndex {
    policy_version: u32,
    source_content_commitment: [u8; 32],
    rows: std::collections::BTreeMap<Vec<u8>, CanonicalFilesystemMetadataRowKind>,
}

impl CanonicalFilesystemMetadataIndex {
    pub fn version_1(
        source_content_commitment: [u8; 32],
        rows: impl IntoIterator<Item = CanonicalFilesystemMetadataRow>,
    ) -> Result<Self, CanonicalFilesystemMetadataIndexError> {
        Self::new(
            CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION,
            source_content_commitment,
            rows,
        )
    }

    pub fn new(
        policy_version: u32,
        source_content_commitment: [u8; 32],
        rows: impl IntoIterator<Item = CanonicalFilesystemMetadataRow>,
    ) -> Result<Self, CanonicalFilesystemMetadataIndexError> {
        if policy_version != CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION {
            return Err(
                CanonicalFilesystemMetadataIndexError::UnsupportedPolicyVersion(policy_version),
            );
        }
        let mut total_path_bytes = 0usize;
        let mut canonical_rows = std::collections::BTreeMap::new();
        for (row_index, row) in rows.into_iter().enumerate() {
            if row_index >= CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT {
                return Err(CanonicalFilesystemMetadataIndexError::RowLimitExceeded {
                    limit: CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT,
                    attempted: row_index.saturating_add(1),
                });
            }
            if !canonical_filesystem_metadata_path_is_canonical(&row.relative_path, true) {
                return Err(CanonicalFilesystemMetadataIndexError::InvalidRelativePath(
                    row.relative_path,
                ));
            }
            total_path_bytes = total_path_bytes
                .checked_add(row.relative_path.len())
                .filter(|total| *total <= FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT)
                .ok_or(
                    CanonicalFilesystemMetadataIndexError::AggregatePathBytesLimitExceeded {
                        limit: FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT,
                        attempted: total_path_bytes.saturating_add(row.relative_path.len()),
                    },
                )?;
            if row.kind.logical_byte_length() > i64::MAX as u64 {
                return Err(
                    CanonicalFilesystemMetadataIndexError::LogicalByteLengthExceedsI64(
                        row.relative_path,
                    ),
                );
            }
            if canonical_rows
                .insert(row.relative_path.clone(), row.kind)
                .is_some()
            {
                return Err(
                    CanonicalFilesystemMetadataIndexError::DuplicateRelativePath(row.relative_path),
                );
            }
        }
        match canonical_rows.get(b"".as_slice()) {
            None => return Err(CanonicalFilesystemMetadataIndexError::MissingRootDirectory),
            Some(CanonicalFilesystemMetadataRowKind::Directory) => {}
            Some(_) => return Err(CanonicalFilesystemMetadataIndexError::RootIsNotDirectory),
        }
        for path in canonical_rows.keys().filter(|path| !path.is_empty()) {
            let parent = path
                .iter()
                .rposition(|byte| *byte == b'/')
                .map_or(b"".as_slice(), |separator| &path[..separator]);
            match canonical_rows.get(parent) {
                None => {
                    return Err(
                        CanonicalFilesystemMetadataIndexError::MissingParentDirectory(path.clone()),
                    );
                }
                Some(CanonicalFilesystemMetadataRowKind::Directory) => {}
                Some(_) => {
                    return Err(CanonicalFilesystemMetadataIndexError::ParentIsNotDirectory(
                        path.clone(),
                    ));
                }
            }
        }
        Ok(Self {
            policy_version,
            source_content_commitment,
            rows: canonical_rows,
        })
    }

    pub const fn policy_version(&self) -> u32 {
        self.policy_version
    }

    pub const fn source_content_commitment(&self) -> &[u8; 32] {
        &self.source_content_commitment
    }

    pub fn rows(&self) -> impl ExactSizeIterator<Item = CanonicalFilesystemMetadataRow> + '_ {
        self.rows.iter().map(|(relative_path, kind)| {
            CanonicalFilesystemMetadataRow::new(relative_path.clone(), *kind)
        })
    }

    pub(crate) fn row(&self, relative_path: &[u8]) -> Option<CanonicalFilesystemMetadataRowKind> {
        self.rows.get(relative_path).copied()
    }
}

impl FilesystemGrantRootIdentity {
    pub const fn new(value: u32) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One compiler-supplied physical grant root and its evidence identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemGrantRoot {
    identity: FilesystemGrantRootIdentity,
    path: std::path::PathBuf,
    canonical_metadata: Option<CanonicalFilesystemMetadataIndex>,
}

impl FilesystemGrantRoot {
    pub fn new(identity: FilesystemGrantRootIdentity, path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            identity,
            path: path.into(),
            canonical_metadata: None,
        }
    }

    /// Attach canonical immutable-source metadata to this grant root.
    pub fn with_canonical_metadata(
        mut self,
        canonical_metadata: CanonicalFilesystemMetadataIndex,
    ) -> Self {
        self.canonical_metadata = Some(canonical_metadata);
        self
    }

    pub const fn identity(&self) -> FilesystemGrantRootIdentity {
        self.identity
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub const fn canonical_metadata(&self) -> Option<&CanonicalFilesystemMetadataIndex> {
        self.canonical_metadata.as_ref()
    }
}

/// One scoped path that passed the grant gate before host access.
///
/// `relative_path` uses `/` between UTF-8 components, never carries a leading
/// separator, and is empty for the root itself. It therefore contains no host
/// absolute path or compiler working-directory spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemAuthorizedPath {
    operand_ordinal: u8,
    access: FilesystemGrantAccess,
    root: FilesystemGrantRootIdentity,
    relative_path: Vec<u8>,
}

/// Canonical non-handle scalar value consumed by one filesystem operation.
/// Width and signedness remain explicit so ABI-distinct operands never compare
/// equal merely because the interpreter carries both in an `i64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemScalarOperandValue {
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemScalarOperand {
    operand_ordinal: u8,
    value: FilesystemScalarOperandValue,
}

impl FilesystemScalarOperand {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn value(self) -> FilesystemScalarOperandValue {
        self.value
    }
}

/// Immutable non-path payload bytes consumed by one operation. Rooted paths
/// and path-like byte aliases stay in path evidence so compiler/cache absolute
/// spellings cannot leak through this row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemByteOperand {
    operand_ordinal: u8,
    bytes: Vec<u8>,
}

impl FilesystemByteOperand {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Exact bytes consumed where an operation assigns path-like meaning without
/// consuming a rooted path grant. Keeping this distinct from immutable payload
/// bytes and authorized rooted paths preserves the operation's operand roles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemPathLikeOperand {
    operand_ordinal: u8,
    bytes: Vec<u8>,
}

impl FilesystemPathLikeOperand {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// One compiler-rooted path at the instant its authored operand successfully
/// resolves during call preparation. This preserves the portable input before
/// physical provider-path lowering. It is not an authorization result: a later
/// grant check may resolve symlinks to a different canonical rooted location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemRootedPathOperandResolution {
    operand_ordinal: u8,
    root: FilesystemGrantRootIdentity,
    relative_path: Vec<u8>,
}

/// Closed semantic class for exact meaningful path bytes returned through a
/// mutable output carrier. Terminators and unchanged carrier tails are not part
/// of these bytes; the complete carrier remains available separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemReturnedPathKind {
    ReadLinkPayload,
    CanonicalPath,
    FinalPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemReturnedPathCompleteness {
    Complete,
    LimitReached,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemReturnedPath {
    operand_ordinal: u8,
    kind: FilesystemReturnedPathKind,
    completeness: FilesystemReturnedPathCompleteness,
    bytes: Vec<u8>,
}

/// Semantic designation of one host-derived byte region returned through an
/// already-custodied mutable output carrier. The bytes are referenced from the
/// matching provider post-state rather than copied a fourth time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemObservedByteRegionKind {
    SequentialFileRead,
    PositionedFileRead,
    DirectoryRecords,
    FindEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemObservedByteRegion {
    output_operand_ordinal: u8,
    kind: FilesystemObservedByteRegionKind,
    offset: usize,
    length: usize,
}

/// Semantic source of one successfully returned metadata record. The kind is
/// independent of the target carrier used to return the same fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemMetadataObservationKind {
    FollowedPath,
    OpenDescriptor,
    UnfollowedFinalPath,
}

/// Minimum mutable byte carrier required by the canonical filesystem metadata
/// API on every selected target.
pub const FILESYSTEM_METADATA_API_CARRIER_BYTES: usize = 144;

/// Canonical target-neutral metadata observed by one successful filesystem
/// operation. File-kind predicates are deliberately absent: they are derived
/// from the retained mode bits and must not become disagreeing duplicate facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemMetadataObservation {
    output_operand_ordinal: u8,
    kind: FilesystemMetadataObservationKind,
    device: u64,
    mode: u32,
    link_count: u64,
    inode: u64,
    user: u32,
    group: u32,
    referenced_device: u64,
    access_time: i64,
    modification_time: i64,
    change_time: i64,
    birth_time: i64,
    size: i64,
    blocks_512: u64,
    preferred_block_size: u64,
}

impl FilesystemMetadataObservation {
    pub(crate) const fn new(
        output_operand_ordinal: u8,
        kind: FilesystemMetadataObservationKind,
        mode: u32,
        size: i64,
        modification_time: i64,
    ) -> Self {
        Self {
            output_operand_ordinal,
            kind,
            device: 16_777_220,
            mode,
            link_count: 1,
            inode: 1_000_000,
            user: 501,
            group: 20,
            referenced_device: 0,
            access_time: 1_000_000_100,
            modification_time,
            change_time: 1_000_000_050,
            birth_time: 999_999_900,
            size,
            blocks_512: 8,
            preferred_block_size: 4096,
        }
    }

    /// Reconstruct one canonical metadata row from compiler-owned replay
    /// custody. This does not consult or authorize a host filesystem. The
    /// replay executor still cross-checks every field against the selected
    /// target carrier before admitting the returned operation.
    #[allow(clippy::too_many_arguments)]
    pub const fn from_replay(
        kind: FilesystemMetadataObservationKind,
        device: u64,
        mode: u32,
        link_count: u64,
        inode: u64,
        user: u32,
        group: u32,
        referenced_device: u64,
        access_time: i64,
        modification_time: i64,
        change_time: i64,
        birth_time: i64,
        size: i64,
        blocks_512: u64,
        preferred_block_size: u64,
    ) -> Self {
        Self {
            output_operand_ordinal: 1,
            kind,
            device,
            mode,
            link_count,
            inode,
            user,
            group,
            referenced_device,
            access_time,
            modification_time,
            change_time,
            birth_time,
            size,
            blocks_512,
            preferred_block_size,
        }
    }

    pub const fn output_operand_ordinal(self) -> u8 {
        self.output_operand_ordinal
    }
    pub const fn kind(self) -> FilesystemMetadataObservationKind {
        self.kind
    }
    pub const fn device(self) -> u64 {
        self.device
    }
    pub const fn mode(self) -> u32 {
        self.mode
    }
    pub const fn link_count(self) -> u64 {
        self.link_count
    }
    pub const fn inode(self) -> u64 {
        self.inode
    }
    pub const fn user(self) -> u32 {
        self.user
    }
    pub const fn group(self) -> u32 {
        self.group
    }
    pub const fn referenced_device(self) -> u64 {
        self.referenced_device
    }
    pub const fn access_time(self) -> i64 {
        self.access_time
    }
    pub const fn modification_time(self) -> i64 {
        self.modification_time
    }
    pub const fn change_time(self) -> i64 {
        self.change_time
    }
    pub const fn birth_time(self) -> i64 {
        self.birth_time
    }
    pub const fn size(self) -> i64 {
        self.size
    }
    pub const fn blocks_512(self) -> u64 {
        self.blocks_512
    }
    pub const fn preferred_block_size(self) -> u64 {
        self.preferred_block_size
    }

    pub(crate) const fn unsigned_field(self, field: FilesystemMetadataField) -> Option<u64> {
        match field {
            FilesystemMetadataField::Device => Some(self.device),
            FilesystemMetadataField::Mode => Some(self.mode as u64),
            FilesystemMetadataField::LinkCount => Some(self.link_count),
            FilesystemMetadataField::Inode => Some(self.inode),
            FilesystemMetadataField::User => Some(self.user as u64),
            FilesystemMetadataField::Group => Some(self.group as u64),
            FilesystemMetadataField::ReferencedDevice => Some(self.referenced_device),
            FilesystemMetadataField::Blocks512 => Some(self.blocks_512),
            FilesystemMetadataField::PreferredBlockSize => Some(self.preferred_block_size),
            FilesystemMetadataField::AccessTime
            | FilesystemMetadataField::ModificationTime
            | FilesystemMetadataField::ChangeTime
            | FilesystemMetadataField::BirthTime
            | FilesystemMetadataField::Size => None,
        }
    }

    pub(crate) const fn signed_field(self, field: FilesystemMetadataField) -> Option<i64> {
        match field {
            FilesystemMetadataField::AccessTime => Some(self.access_time),
            FilesystemMetadataField::ModificationTime => Some(self.modification_time),
            FilesystemMetadataField::ChangeTime => Some(self.change_time),
            FilesystemMetadataField::BirthTime => Some(self.birth_time),
            FilesystemMetadataField::Size => Some(self.size),
            _ => None,
        }
    }
}

impl FilesystemObservedByteRegion {
    pub const fn output_operand_ordinal(self) -> u8 {
        self.output_operand_ordinal
    }

    pub const fn kind(self) -> FilesystemObservedByteRegionKind {
        self.kind
    }

    pub const fn offset(self) -> usize {
        self.offset
    }

    pub const fn length(self) -> usize {
        self.length
    }
}

impl FilesystemReturnedPath {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub const fn kind(&self) -> FilesystemReturnedPathKind {
        self.kind
    }

    pub const fn completeness(&self) -> FilesystemReturnedPathCompleteness {
        self.completeness
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl FilesystemRootedPathOperandResolution {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub const fn root(&self) -> FilesystemGrantRootIdentity {
        self.root
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }
}

/// Complete state of one mutable byte carrier at the instant its authored
/// operand successfully resolves. This preparation-prefix row is distinct from
/// the provider-visible pre/post row because evaluating a later argument may
/// alias and mutate the carrier before provider invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemMutableByteOperandResolution {
    operand_ordinal: u8,
    bytes: Vec<u8>,
}

impl FilesystemMutableByteOperandResolution {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Exact value of one mutable i64 carrier at the instant its authored operand
/// successfully resolves. Provider-visible pre/post timing remains separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemMutableI64OperandResolution {
    operand_ordinal: u8,
    value: i64,
}

impl FilesystemMutableI64OperandResolution {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn value(self) -> i64 {
        self.value
    }
}

/// Complete provider-visible state of one mutable byte carrier immediately
/// before and after the operation's provider invocation. Both vectors equal
/// the resolved carrier capacity; unchanged tails remain explicit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemMutableByteOperand {
    operand_ordinal: u8,
    pre_bytes: Vec<u8>,
    post_bytes: Vec<u8>,
}

impl FilesystemMutableByteOperand {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn pre_bytes(&self) -> &[u8] {
        &self.pre_bytes
    }

    pub fn post_bytes(&self) -> &[u8] {
        &self.post_bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemMutableI64Operand {
    operand_ordinal: u8,
    pre_value: i64,
    post_value: i64,
}

impl FilesystemMutableI64Operand {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn pre_value(self) -> i64 {
        self.pre_value
    }

    pub const fn post_value(self) -> i64 {
        self.post_value
    }
}

impl FilesystemAuthorizedPath {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub const fn access(&self) -> FilesystemGrantAccess {
        self.access
    }

    pub const fn root(&self) -> FilesystemGrantRootIdentity {
        self.root
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }
}

/// Evaluator-issued identity for one filesystem descriptor or handle lifetime.
///
/// The identity is allocated independently of provider token values. Reusing a
/// runtime descriptor after close therefore produces a fresh identity, while
/// every operation during one live lifetime refers to the same identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FilesystemLogicalHandleIdentity(u64);

impl FilesystemLogicalHandleIdentity {
    pub(crate) const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Closed resource domain for a logical filesystem handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemLogicalHandleKind {
    /// POSIX/CRT-style integer file descriptor.
    Descriptor,
    /// Native path/file handle, distinct from a CRT descriptor on Windows.
    Native,
    /// Directory-enumeration cursor returned by `find_first`.
    Find,
}

/// Resolution of one authored handle operand against earlier successful
/// operations in the same evaluator run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemLogicalHandleInputResolution {
    Resolved(FilesystemLogicalHandleIdentity),
    /// The canonical ABI explicitly permits a null handle in this position.
    Null,
    /// No live logical lifetime owns the supplied provider token.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemLogicalHandleInput {
    operand_ordinal: u8,
    kind: FilesystemLogicalHandleKind,
    resolution: FilesystemLogicalHandleInputResolution,
}

impl FilesystemLogicalHandleInput {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn kind(self) -> FilesystemLogicalHandleKind {
        self.kind
    }

    pub const fn resolution(self) -> FilesystemLogicalHandleInputResolution {
        self.resolution
    }
}

/// Provenance for a successfully returned logical handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemLogicalHandleOutputSource {
    /// A new independently owned resource was opened.
    Created,
    /// A new descriptor lifetime was duplicated from an existing descriptor.
    Duplicated(FilesystemLogicalHandleIdentity),
    /// A native handle view was borrowed from an existing descriptor.
    Borrowed(FilesystemLogicalHandleIdentity),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemLogicalHandleOutput {
    kind: FilesystemLogicalHandleKind,
    identity: FilesystemLogicalHandleIdentity,
    source: FilesystemLogicalHandleOutputSource,
}

impl FilesystemLogicalHandleOutput {
    pub const fn kind(self) -> FilesystemLogicalHandleKind {
        self.kind
    }

    pub const fn identity(self) -> FilesystemLogicalHandleIdentity {
        self.identity
    }

    pub const fn source(self) -> FilesystemLogicalHandleOutputSource {
        self.source
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemEvaluationHaltKind {
    Exit,
    Unsupported,
    Trap,
    ResourceExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemOperationResult {
    Scalar(i64),
    LogicalHandle(FilesystemLogicalHandleIdentity),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemOperationAttemptOutcome {
    Returned {
        result: FilesystemOperationResult,
        post_error: i32,
    },
    EvaluationHalted(FilesystemEvaluationHaltKind),
}

impl FilesystemGrantRefusal {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn access(self) -> FilesystemGrantAccess {
        self.access
    }

    pub const fn reason(self) -> FilesystemGrantRefusalReason {
        self.reason
    }
}

/// One completed canonical filesystem operation attempted during build-machine
/// evaluation. Failed evaluations retain their completed prefix as
/// non-admission evidence.
///
/// The operation tag is an append-only compiler-owned identity. No package
/// string enters this row. Successful descriptor/handle results and uses are
/// normalized into logical lifetimes; provider token numbers do not survive.
/// Failed handle-result sentinels remain scalar results. Mutable carriers
/// retain both their successfully resolved preparation prefix and complete
/// provider-visible pre/post snapshots. Path results and successful file and
/// directory and metadata observations have semantic rows. Replay execution
/// remains incomplete, so this stays below receipt strength.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemOperationAttempt {
    operation_tag: u16,
    provider: FilesystemObservationProvider,
    outcome: Option<FilesystemOperationAttemptOutcome>,
    scalar_operands: Vec<FilesystemScalarOperand>,
    byte_operands: Vec<FilesystemByteOperand>,
    path_like_operands: Vec<FilesystemPathLikeOperand>,
    rooted_path_operand_resolutions: Vec<FilesystemRootedPathOperandResolution>,
    returned_paths: Vec<FilesystemReturnedPath>,
    observed_byte_regions: Vec<FilesystemObservedByteRegion>,
    metadata_observations: Vec<FilesystemMetadataObservation>,
    mutable_byte_operand_resolutions: Vec<FilesystemMutableByteOperandResolution>,
    mutable_i64_operand_resolutions: Vec<FilesystemMutableI64OperandResolution>,
    mutable_byte_operands: Vec<FilesystemMutableByteOperand>,
    mutable_i64_operands: Vec<FilesystemMutableI64Operand>,
    authorized_paths: Vec<FilesystemAuthorizedPath>,
    logical_handle_inputs: Vec<FilesystemLogicalHandleInput>,
    logical_handle_output: Option<FilesystemLogicalHandleOutput>,
    retired_logical_handles: Vec<FilesystemLogicalHandleIdentity>,
    grant_refusals: Vec<FilesystemGrantRefusal>,
}

impl FilesystemOperationAttempt {
    const fn pending(operation_tag: u16, provider: FilesystemObservationProvider) -> Self {
        Self {
            operation_tag,
            provider,
            outcome: None,
            scalar_operands: Vec::new(),
            byte_operands: Vec::new(),
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: Vec::new(),
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: Vec::new(),
            mutable_i64_operand_resolutions: Vec::new(),
            mutable_byte_operands: Vec::new(),
            mutable_i64_operands: Vec::new(),
            authorized_paths: Vec::new(),
            logical_handle_inputs: Vec::new(),
            logical_handle_output: None,
            retired_logical_handles: Vec::new(),
            grant_refusals: Vec::new(),
        }
    }

    pub const fn operation_tag(&self) -> u16 {
        self.operation_tag
    }

    pub const fn provider(&self) -> FilesystemObservationProvider {
        self.provider
    }

    pub const fn outcome(&self) -> Option<FilesystemOperationAttemptOutcome> {
        self.outcome
    }

    pub const fn result(&self) -> Option<FilesystemOperationResult> {
        match self.outcome {
            Some(FilesystemOperationAttemptOutcome::Returned { result, .. }) => Some(result),
            _ => None,
        }
    }

    pub const fn post_error(&self) -> Option<i32> {
        match self.outcome {
            Some(FilesystemOperationAttemptOutcome::Returned { post_error, .. }) => {
                Some(post_error)
            }
            _ => None,
        }
    }

    pub fn scalar_operands(&self) -> &[FilesystemScalarOperand] {
        &self.scalar_operands
    }

    pub fn byte_operands(&self) -> &[FilesystemByteOperand] {
        &self.byte_operands
    }

    pub fn path_like_operands(&self) -> &[FilesystemPathLikeOperand] {
        &self.path_like_operands
    }

    pub fn rooted_path_operand_resolutions(&self) -> &[FilesystemRootedPathOperandResolution] {
        &self.rooted_path_operand_resolutions
    }

    pub fn returned_paths(&self) -> &[FilesystemReturnedPath] {
        &self.returned_paths
    }

    pub fn observed_byte_regions(&self) -> &[FilesystemObservedByteRegion] {
        &self.observed_byte_regions
    }

    pub fn metadata_observations(&self) -> &[FilesystemMetadataObservation] {
        &self.metadata_observations
    }

    pub fn mutable_byte_operand_resolutions(&self) -> &[FilesystemMutableByteOperandResolution] {
        &self.mutable_byte_operand_resolutions
    }

    pub fn mutable_i64_operand_resolutions(&self) -> &[FilesystemMutableI64OperandResolution] {
        &self.mutable_i64_operand_resolutions
    }

    pub fn mutable_byte_operands(&self) -> &[FilesystemMutableByteOperand] {
        &self.mutable_byte_operands
    }

    pub fn mutable_i64_operands(&self) -> &[FilesystemMutableI64Operand] {
        &self.mutable_i64_operands
    }

    pub fn grant_refusals(&self) -> &[FilesystemGrantRefusal] {
        &self.grant_refusals
    }

    pub fn authorized_paths(&self) -> &[FilesystemAuthorizedPath] {
        &self.authorized_paths
    }

    pub fn logical_handle_inputs(&self) -> &[FilesystemLogicalHandleInput] {
        &self.logical_handle_inputs
    }

    pub const fn logical_handle_output(&self) -> Option<FilesystemLogicalHandleOutput> {
        self.logical_handle_output
    }

    pub fn retired_logical_handles(&self) -> &[FilesystemLogicalHandleIdentity] {
        &self.retired_logical_handles
    }
}

/// Host observations made while evaluating one machine.
///
/// This is deliberately separate from [`EvaluationUsage`]: deterministic
/// evaluator work and build-host observation/replay are different policy
/// axes. Ordinary semantic evaluation must always return an empty row. The
/// granted build-machine entry may report a filesystem observation, which the
/// compiler classifies according to the exact provider it selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationObservations {
    filesystem_operation_schema_version: u32,
    filesystem_operation_attempts: Vec<FilesystemOperationAttempt>,
    build_included_sources: Vec<BuildIncludedSource>,
    build_log: Vec<u8>,
}

/// Opaque, compiler-produced operation record for bounded filesystem replay.
/// Source events may be followed by an ordered parent-before-child Output tree
/// of directories, complete regular-file chains, symbolic links, and hard-link
/// names, by a closed failure-only Output-operation sequence, or by one closed
/// deterministic handle failure. File chains admit only their explicitly
/// validated descriptor operations, and generated-source handoffs retain exact
/// authored order.
/// The record is replay evidence; the compiler separately establishes receipt
/// strength by reproducing the build and matching sponsored staged-tree custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemReplay {
    attempts: std::sync::Arc<[FilesystemOperationAttempt]>,
    expected_included_sources: std::sync::Arc<[BuildIncludedSource]>,
}

/// Ordinary non-executable create mode admitted by the first Output replay
/// rung (`0o666`, represented in Omega source as decimal `438`).
pub const FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE: i32 = 438;
pub const MAX_INCLUDED_BUILD_SOURCES: usize = 256;
pub const MAX_FILESYSTEM_REPLAY_RETAINED_BYTES: usize = 16 * 1024 * 1024;
// Cloning filesystem operation attempts has a deterministic availability
// limit. Fixed row weights are the current canonical row-width upper bounds;
// variable payload bytes contribute one unit each. This is deliberately not a
// second encoder, a Rust-layout measurement, or package evidence.
const MAX_FILESYSTEM_REPLAY_RETENTION_WEIGHT: usize = 16 * 1024 * 1024;
// Attempt = 16 fixed bytes + fifteen 8-byte lane counts + the 19-byte maximum
// logical-output row. The remaining weights are each row's fixed-width maximum
// in encode_attempt; byte-bearing lanes add their payloads below.
const FILESYSTEM_REPLAY_ATTEMPT_RETENTION_WEIGHT: usize = 155;
const FILESYSTEM_REPLAY_SCALAR_OPERAND_RETENTION_WEIGHT: usize = 10;
const FILESYSTEM_REPLAY_BYTE_OPERAND_RETENTION_WEIGHT: usize = 9;
const FILESYSTEM_REPLAY_PATH_LIKE_OPERAND_RETENTION_WEIGHT: usize = 9;
const FILESYSTEM_REPLAY_ROOTED_PATH_RETENTION_WEIGHT: usize = 10;
const FILESYSTEM_REPLAY_RETURNED_PATH_RETENTION_WEIGHT: usize = 11;
const FILESYSTEM_REPLAY_OBSERVED_REGION_RETENTION_WEIGHT: usize = 18;
const FILESYSTEM_REPLAY_METADATA_RETENTION_WEIGHT: usize = 102;
const FILESYSTEM_REPLAY_MUTABLE_BYTE_RESOLUTION_RETENTION_WEIGHT: usize = 9;
const FILESYSTEM_REPLAY_MUTABLE_I64_RESOLUTION_RETENTION_WEIGHT: usize = 9;
const FILESYSTEM_REPLAY_MUTABLE_BYTE_RETENTION_WEIGHT: usize = 17;
const FILESYSTEM_REPLAY_MUTABLE_I64_RETENTION_WEIGHT: usize = 17;
const FILESYSTEM_REPLAY_AUTHORIZED_PATH_RETENTION_WEIGHT: usize = 11;
const FILESYSTEM_REPLAY_LOGICAL_INPUT_RETENTION_WEIGHT: usize = 11;
const FILESYSTEM_REPLAY_RETIRED_HANDLE_RETENTION_WEIGHT: usize = 8;
const FILESYSTEM_REPLAY_GRANT_REFUSAL_RETENTION_WEIGHT: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemReplayReadKind {
    Sequential,
    Positioned { offset: i64 },
}

/// One typed read in the first replay rung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemReplayReadRecord {
    read_kind: FilesystemReplayReadKind,
    requested_count: u64,
    read_result: i64,
    read_post_error: i32,
    mutable_resolution: Vec<u8>,
    mutable_pre_state: Vec<u8>,
    mutable_post_state: Vec<u8>,
}

impl FilesystemReplayReadRecord {
    pub fn new(
        read_kind: FilesystemReplayReadKind,
        requested_count: u64,
        read_result: i64,
        read_post_error: i32,
        mutable_resolution: Vec<u8>,
        mutable_pre_state: Vec<u8>,
        mutable_post_state: Vec<u8>,
    ) -> Result<Self, String> {
        let read_length = usize::try_from(read_result)
            .map_err(|_| "filesystem replay read result must be nonnegative".to_owned())?;
        let requested_capacity = usize::try_from(requested_count)
            .map_err(|_| "filesystem replay request exceeds this host".to_owned())?;
        if matches!(read_kind, FilesystemReplayReadKind::Positioned { offset } if offset < 0)
            || mutable_resolution != mutable_pre_state
            || mutable_pre_state.len() != mutable_post_state.len()
            || requested_capacity > mutable_post_state.len()
            || read_length > requested_capacity
            || mutable_pre_state[read_length..] != mutable_post_state[read_length..]
        {
            return Err("filesystem replay mutable read carrier is inconsistent".to_owned());
        }
        Ok(Self {
            read_kind,
            requested_count,
            read_result,
            read_post_error,
            mutable_resolution,
            mutable_pre_state,
            mutable_post_state,
        })
    }
}

/// One closed source-read chain reconstructed from canonical compiler custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSourceReadChainReplayRecord {
    source_root: FilesystemGrantRootIdentity,
    source_relative_path: Vec<u8>,
    logical_handle_identity: FilesystemLogicalHandleIdentity,
    open_post_error: i32,
    reads: Vec<FilesystemReplayReadRecord>,
    close_post_error: i32,
}

impl FilesystemSourceReadChainReplayRecord {
    pub fn new(
        source_root: FilesystemGrantRootIdentity,
        source_relative_path: Vec<u8>,
        logical_handle_identity: u64,
        open_post_error: i32,
        reads: Vec<FilesystemReplayReadRecord>,
        close_post_error: i32,
    ) -> Result<Self, String> {
        let logical_handle_identity = FilesystemLogicalHandleIdentity::new(logical_handle_identity)
            .ok_or_else(|| "filesystem replay logical identity must be nonzero".to_owned())?;
        if reads.is_empty() {
            return Err("filesystem replay requires at least one read".to_owned());
        }
        Ok(Self {
            source_root,
            source_relative_path,
            logical_handle_identity,
            open_post_error,
            reads,
            close_post_error,
        })
    }
}

/// One successful Source-rooted path metadata read reconstructed from
/// canonical compiler custody. The authored rooted input and the separately
/// authorized target both remain exact because following a symlink may make
/// them differ. Returned bytes are carried by the mutable post-state and
/// checked against `metadata` under the selected target layout during replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSourcePathMetadataReplayRecord {
    kind: FilesystemMetadataObservationKind,
    source_root: FilesystemGrantRootIdentity,
    source_relative_path: Vec<u8>,
    authorized_root: FilesystemGrantRootIdentity,
    authorized_relative_path: Vec<u8>,
    post_error: i32,
    mutable_resolution: Vec<u8>,
    mutable_pre_state: Vec<u8>,
    mutable_post_state: Vec<u8>,
    metadata: FilesystemMetadataObservation,
}

impl FilesystemSourcePathMetadataReplayRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: FilesystemMetadataObservationKind,
        source_root: FilesystemGrantRootIdentity,
        source_relative_path: Vec<u8>,
        authorized_root: FilesystemGrantRootIdentity,
        authorized_relative_path: Vec<u8>,
        post_error: i32,
        mutable_resolution: Vec<u8>,
        mutable_pre_state: Vec<u8>,
        mutable_post_state: Vec<u8>,
        metadata: FilesystemMetadataObservation,
    ) -> Result<Self, String> {
        if !matches!(
            kind,
            FilesystemMetadataObservationKind::FollowedPath
                | FilesystemMetadataObservationKind::UnfollowedFinalPath
        ) || metadata.kind() != kind
            || metadata.output_operand_ordinal() != 1
            || source_root != authorized_root
            || !filesystem_root_relative_path_is_canonical(&source_relative_path, false)
            || !filesystem_root_relative_path_is_canonical(&authorized_relative_path, true)
            || mutable_resolution != mutable_pre_state
            || mutable_pre_state.len() != mutable_post_state.len()
            || mutable_post_state.len() < FILESYSTEM_METADATA_API_CARRIER_BYTES
        {
            return Err("filesystem replay path metadata is inconsistent".to_owned());
        }
        Ok(Self {
            kind,
            source_root,
            source_relative_path,
            authorized_root,
            authorized_relative_path,
            post_error,
            mutable_resolution,
            mutable_pre_state,
            mutable_post_state,
            metadata,
        })
    }
}

/// One successful Source-rooted descriptor metadata event. The descriptor is
/// created by the event's exact flags-zero open, observed once, and retired by
/// its exact close; it cannot be borrowed from or leaked into another event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSourceDescriptorMetadataReplayRecord {
    source_root: FilesystemGrantRootIdentity,
    source_relative_path: Vec<u8>,
    logical_handle_identity: FilesystemLogicalHandleIdentity,
    open_post_error: i32,
    metadata_post_error: i32,
    mutable_resolution: Vec<u8>,
    mutable_pre_state: Vec<u8>,
    mutable_post_state: Vec<u8>,
    metadata: FilesystemMetadataObservation,
    close_post_error: i32,
}

impl FilesystemSourceDescriptorMetadataReplayRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_root: FilesystemGrantRootIdentity,
        source_relative_path: Vec<u8>,
        logical_handle_identity: u64,
        open_post_error: i32,
        metadata_post_error: i32,
        mutable_resolution: Vec<u8>,
        mutable_pre_state: Vec<u8>,
        mutable_post_state: Vec<u8>,
        metadata: FilesystemMetadataObservation,
        close_post_error: i32,
    ) -> Result<Self, String> {
        let logical_handle_identity = FilesystemLogicalHandleIdentity::new(logical_handle_identity)
            .ok_or_else(|| "filesystem replay logical identity must be nonzero".to_owned())?;
        if !filesystem_root_relative_path_is_canonical(&source_relative_path, false)
            || metadata.kind() != FilesystemMetadataObservationKind::OpenDescriptor
            || metadata.output_operand_ordinal() != 1
            || mutable_resolution != mutable_pre_state
            || mutable_pre_state.len() != mutable_post_state.len()
            || mutable_post_state.len() < FILESYSTEM_METADATA_API_CARRIER_BYTES
        {
            return Err("filesystem replay descriptor metadata is inconsistent".to_owned());
        }
        Ok(Self {
            source_root,
            source_relative_path,
            logical_handle_identity,
            open_post_error,
            metadata_post_error,
            mutable_resolution,
            mutable_pre_state,
            mutable_post_state,
            metadata,
            close_post_error,
        })
    }
}

/// One ordered source-input replay event. Descriptor-backed file reads and
/// descriptor metadata remain indivisible closed chains; `read_link` and path
/// metadata reads are independent events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesystemSourceInputReplayEventRecord {
    ReadChain(FilesystemSourceReadChainReplayRecord),
    DirectoryReadChain(FilesystemSourceDirectoryReadChainReplayRecord),
    ReadLink(FilesystemSourceReadLinkReplayRecord),
    DescriptorMetadata(FilesystemSourceDescriptorMetadataReplayRecord),
    PathMetadata(FilesystemSourcePathMetadataReplayRecord),
}

/// Typed source-input replay record reconstructed after canonical bytes cross
/// a process boundary. It grants no ambient host filesystem authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSourceInputReplayRecord {
    events: Vec<FilesystemSourceInputReplayEventRecord>,
}

impl FilesystemSourceInputReplayRecord {
    pub fn new(events: Vec<FilesystemSourceInputReplayEventRecord>) -> Result<Self, String> {
        if events.is_empty() {
            return Err("filesystem replay requires at least one source-input event".to_owned());
        }
        let mut identities = Vec::new();
        for event in &events {
            let identity = match event {
                FilesystemSourceInputReplayEventRecord::ReadChain(chain) => {
                    Some(chain.logical_handle_identity)
                }
                FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain) => {
                    Some(chain.logical_handle_identity())
                }
                FilesystemSourceInputReplayEventRecord::ReadLink(_) => None,
                FilesystemSourceInputReplayEventRecord::DescriptorMetadata(metadata) => {
                    Some(metadata.logical_handle_identity)
                }
                FilesystemSourceInputReplayEventRecord::PathMetadata(_) => None,
            };
            let Some(identity) = identity else { continue };
            if identities.contains(&identity) {
                return Err(
                    "filesystem replay descriptor events must use distinct handles".to_owned(),
                );
            }
            identities.push(identity);
        }
        Ok(Self { events })
    }
}

/// Cursor behavior for one complete write within a freshly created Output file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemOutputWriteReplayKind {
    Sequential,
    Positioned { offset: i64 },
}

/// One complete sequential or positioned write within a freshly created
/// Output file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemOutputWriteReplayRecord {
    kind: FilesystemOutputWriteReplayKind,
    bytes: Vec<u8>,
    result: i64,
    post_error: i32,
}

impl FilesystemOutputWriteReplayRecord {
    pub fn new(bytes: Vec<u8>, result: i64, post_error: i32) -> Result<Self, String> {
        Self::with_kind(
            FilesystemOutputWriteReplayKind::Sequential,
            bytes,
            result,
            post_error,
        )
    }

    pub fn positioned(
        offset: i64,
        bytes: Vec<u8>,
        result: i64,
        post_error: i32,
    ) -> Result<Self, String> {
        if offset < 0 {
            return Err(
                "filesystem replay positioned output offset must be nonnegative".to_owned(),
            );
        }
        Self::with_kind(
            FilesystemOutputWriteReplayKind::Positioned { offset },
            bytes,
            result,
            post_error,
        )
    }

    fn with_kind(
        kind: FilesystemOutputWriteReplayKind,
        bytes: Vec<u8>,
        result: i64,
        post_error: i32,
    ) -> Result<Self, String> {
        let full_result = i64::try_from(bytes.len())
            .map_err(|_| "filesystem replay output exceeds i64 write length".to_owned())?;
        if result != full_result {
            return Err(
                "filesystem replay output write must consume the complete immutable operand"
                    .to_owned(),
            );
        }
        Ok(Self {
            kind,
            bytes,
            result,
            post_error,
        })
    }

    pub const fn kind(&self) -> FilesystemOutputWriteReplayKind {
        self.kind
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn result(&self) -> i64 {
        self.result
    }

    pub const fn post_error(&self) -> i32 {
        self.post_error
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesystemOutputFileOperationReplayRecord {
    Write(FilesystemOutputWriteReplayRecord),
    Seek {
        offset: i64,
        whence: i32,
        result: i64,
    },
    SetLength {
        length: i64,
    },
    SetFilePermissions {
        mode: u32,
    },
    SetFileTimes {
        times: Vec<u8>,
    },
    Sync,
    SyncData,
    DuplicateAndClose(FilesystemOutputDuplicateReplayRecord),
    LockAndUnlock(FilesystemOutputLockReplayRecord),
}

/// One freshly created and closed Output file, optionally containing complete
/// sequential or positioned writes.
///
/// The file grammar is deliberately narrow: canonical `create` (tag 1), zero
/// or more admitted operations through that descriptor, successful
/// `duplicate`/immediate-`close` pairs, then canonical `close` (tag 8) of the
/// original. A compiler may reconstruct the
/// exact attempts from this record, but the record does not claim publication,
/// receipt strength, or custody of a staged tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemOutputFileReplayRecord {
    output_root: FilesystemGrantRootIdentity,
    output_relative_path: Vec<u8>,
    logical_handle_identity: FilesystemLogicalHandleIdentity,
    create_post_error: i32,
    operations: Vec<FilesystemOutputFileOperationReplayRecord>,
    close_post_error: i32,
}

impl FilesystemOutputFileReplayRecord {
    pub fn empty(
        output_root: FilesystemGrantRootIdentity,
        output_relative_path: Vec<u8>,
        logical_handle_identity: u64,
        create_post_error: i32,
        close_post_error: i32,
    ) -> Result<Self, String> {
        Self::with_writes(
            output_root,
            output_relative_path,
            logical_handle_identity,
            create_post_error,
            Vec::new(),
            close_post_error,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        output_root: FilesystemGrantRootIdentity,
        output_relative_path: Vec<u8>,
        logical_handle_identity: u64,
        create_post_error: i32,
        write_bytes: Vec<u8>,
        write_result: i64,
        write_post_error: i32,
        close_post_error: i32,
    ) -> Result<Self, String> {
        if !filesystem_root_relative_path_is_canonical(&output_relative_path, false) {
            return Err("filesystem replay output path must be canonical and non-root".to_owned());
        }
        let logical_handle_identity = FilesystemLogicalHandleIdentity::new(logical_handle_identity)
            .ok_or_else(|| "filesystem replay output identity must be nonzero".to_owned())?;
        let write =
            FilesystemOutputWriteReplayRecord::new(write_bytes, write_result, write_post_error)?;
        Self::with_writes(
            output_root,
            output_relative_path,
            logical_handle_identity.get(),
            create_post_error,
            vec![write],
            close_post_error,
        )
    }

    pub fn with_writes(
        output_root: FilesystemGrantRootIdentity,
        output_relative_path: Vec<u8>,
        logical_handle_identity: u64,
        create_post_error: i32,
        writes: Vec<FilesystemOutputWriteReplayRecord>,
        close_post_error: i32,
    ) -> Result<Self, String> {
        Self::with_operations(
            output_root,
            output_relative_path,
            logical_handle_identity,
            create_post_error,
            writes
                .into_iter()
                .map(FilesystemOutputFileOperationReplayRecord::Write)
                .collect(),
            close_post_error,
        )
    }

    pub fn with_operations(
        output_root: FilesystemGrantRootIdentity,
        output_relative_path: Vec<u8>,
        logical_handle_identity: u64,
        create_post_error: i32,
        operations: Vec<FilesystemOutputFileOperationReplayRecord>,
        close_post_error: i32,
    ) -> Result<Self, String> {
        if !filesystem_root_relative_path_is_canonical(&output_relative_path, false) {
            return Err("filesystem replay output path must be canonical and non-root".to_owned());
        }
        let logical_handle_identity = FilesystemLogicalHandleIdentity::new(logical_handle_identity)
            .ok_or_else(|| "filesystem replay output identity must be nonzero".to_owned())?;
        let record = Self {
            output_root,
            output_relative_path,
            logical_handle_identity,
            create_post_error,
            operations,
            close_post_error,
        };
        record.replayed_extents()?;
        Ok(record)
    }

    pub const fn output_root(&self) -> FilesystemGrantRootIdentity {
        self.output_root
    }

    pub fn output_relative_path(&self) -> &[u8] {
        &self.output_relative_path
    }

    pub const fn logical_handle_identity(&self) -> FilesystemLogicalHandleIdentity {
        self.logical_handle_identity
    }

    pub const fn create_mode(&self) -> i32 {
        FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE
    }

    pub const fn create_post_error(&self) -> i32 {
        self.create_post_error
    }

    pub fn operations(&self) -> &[FilesystemOutputFileOperationReplayRecord] {
        &self.operations
    }

    /// The exact final descriptor-scoped permission operand, when the build
    /// authored one. Absence deliberately remains distinct from an authored
    /// mode equal to the create default.
    pub fn replayed_file_permissions(&self) -> Option<u32> {
        self.operations.iter().rev().find_map(|operation| {
            if let FilesystemOutputFileOperationReplayRecord::SetFilePermissions { mode } =
                operation
            {
                Some(*mode)
            } else {
                None
            }
        })
    }

    /// The final modeled modification time from an authored descriptor-scoped
    /// time operation. The complete carrier remains in the operation record;
    /// this projection exists only to verify the final virtual namespace.
    pub(crate) fn replayed_file_modification_time(&self) -> Option<i64> {
        self.operations.iter().rev().find_map(|operation| {
            let FilesystemOutputFileOperationReplayRecord::SetFileTimes { times } = operation
            else {
                return None;
            };
            Some(i64::from_le_bytes(times[16..24].try_into().expect(
                "validated replay timespec carrier has modification seconds",
            )))
        })
    }

    /// Canonical staged-tree executable class derived from the final authored
    /// permission mode. A newly created file with no permission operation is
    /// ordinary regardless of the capture host's ambient umask.
    pub fn replayed_executable(&self) -> bool {
        self.replayed_file_permissions()
            .is_some_and(|mode| mode & 0o111 != 0)
    }

    pub fn replayed_bytes(&self) -> Result<Vec<u8>, String> {
        let (_, peak_extent) = self.replayed_extents()?;
        if peak_extent > MAX_FILESYSTEM_REPLAY_RETAINED_BYTES {
            return Err(format!(
                "filesystem replay Output exceeds its {MAX_FILESYSTEM_REPLAY_RETAINED_BYTES}-byte extent ceiling"
            ));
        }
        let mut output = Vec::new();
        let mut cursor = 0usize;
        for operation in &self.operations {
            let FilesystemOutputFileOperationReplayRecord::Write(write) = operation else {
                if let FilesystemOutputFileOperationReplayRecord::Seek { result, .. } = operation {
                    cursor = usize::try_from(*result).map_err(|_| {
                        "filesystem replay Output seek result exceeds this host".to_owned()
                    })?;
                }
                if let FilesystemOutputFileOperationReplayRecord::SetLength { length } = operation {
                    let length = usize::try_from(*length).map_err(|_| {
                        "filesystem replay Output length exceeds this host".to_owned()
                    })?;
                    output
                        .try_reserve(length.saturating_sub(output.len()))
                        .map_err(|_| "filesystem replay output allocation failed".to_owned())?;
                    output.resize(length, 0);
                }
                continue;
            };
            let start = match write.kind {
                FilesystemOutputWriteReplayKind::Sequential => cursor,
                FilesystemOutputWriteReplayKind::Positioned { offset } => usize::try_from(offset)
                    .map_err(|_| {
                    "filesystem replay positioned output offset exceeds this host".to_owned()
                })?,
            };
            let end = start
                .checked_add(write.bytes.len())
                .ok_or_else(|| "filesystem replay output extent overflowed".to_owned())?;
            if !write.bytes.is_empty() {
                if output.len() < end {
                    output
                        .try_reserve(end - output.len())
                        .map_err(|_| "filesystem replay output allocation failed".to_owned())?;
                    output.resize(end, 0);
                }
                output[start..end].copy_from_slice(&write.bytes);
            }
            if write.kind == FilesystemOutputWriteReplayKind::Sequential {
                cursor = end;
            }
        }
        Ok(output)
    }

    pub const fn close_post_error(&self) -> i32 {
        self.close_post_error
    }

    fn replayed_extents(&self) -> Result<(usize, usize), String> {
        let mut cursor = 0usize;
        let mut extent = 0usize;
        let mut peak_extent = 0usize;
        let mut duplicate_identities = Vec::new();
        for operation in &self.operations {
            if let FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(duplicate) =
                operation
            {
                let identity = duplicate.logical_handle_identity();
                if identity == self.logical_handle_identity
                    || duplicate_identities.contains(&identity)
                {
                    return Err("filesystem replay Output duplicate identity is reused".to_owned());
                }
                duplicate_identities.push(identity);
                if duplicate_identities.len() > MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES {
                    return Err(format!(
                        "filesystem replay Output duplicates exceed the {MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES}-descriptor ceiling"
                    ));
                }
                continue;
            }
            let FilesystemOutputFileOperationReplayRecord::Write(write) = operation else {
                if let FilesystemOutputFileOperationReplayRecord::SetFileTimes { times } = operation
                    && times.len() < 32
                {
                    return Err(
                        "filesystem replay Output set_file_times carrier is shorter than two timespec records"
                            .to_owned(),
                    );
                }
                if let FilesystemOutputFileOperationReplayRecord::Seek {
                    offset,
                    whence,
                    result,
                } = operation
                {
                    let base = match whence {
                        0 => 0i64,
                        1 => i64::try_from(cursor).map_err(|_| {
                            "filesystem replay Output cursor exceeds i64".to_owned()
                        })?,
                        2 => i64::try_from(extent).map_err(|_| {
                            "filesystem replay Output extent exceeds i64".to_owned()
                        })?,
                        _ => {
                            return Err(
                                "filesystem replay Output seek whence is invalid".to_owned()
                            );
                        }
                    };
                    let expected = base.checked_add(*offset).ok_or_else(|| {
                        "filesystem replay Output seek result overflowed".to_owned()
                    })?;
                    if expected < 0 || expected != *result {
                        return Err(
                            "filesystem replay Output seek result is inconsistent".to_owned()
                        );
                    }
                    cursor = usize::try_from(expected).map_err(|_| {
                        "filesystem replay Output seek result exceeds this host".to_owned()
                    })?;
                }
                if let FilesystemOutputFileOperationReplayRecord::SetLength { length } = operation {
                    extent = usize::try_from(*length).map_err(|_| {
                        "filesystem replay Output length must be nonnegative and fit this host"
                            .to_owned()
                    })?;
                    peak_extent = peak_extent.max(extent);
                }
                continue;
            };
            let start = match write.kind {
                FilesystemOutputWriteReplayKind::Sequential => cursor,
                FilesystemOutputWriteReplayKind::Positioned { offset } => usize::try_from(offset)
                    .map_err(|_| {
                    "filesystem replay positioned output offset exceeds this host".to_owned()
                })?,
            };
            let end = start
                .checked_add(write.bytes.len())
                .ok_or_else(|| "filesystem replay output extent overflowed".to_owned())?;
            if !write.bytes.is_empty() {
                extent = extent.max(end);
                peak_extent = peak_extent.max(extent);
            }
            if write.kind == FilesystemOutputWriteReplayKind::Sequential {
                cursor = end;
            }
        }
        Ok((extent, peak_extent))
    }
}

fn output_file_operation_attempt_count(
    operation: &FilesystemOutputFileOperationReplayRecord,
) -> usize {
    match operation {
        FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(_)
        | FilesystemOutputFileOperationReplayRecord::LockAndUnlock(_) => 2,
        _ => 1,
    }
}

fn output_file_attempt_count(output: &FilesystemOutputFileReplayRecord) -> Option<usize> {
    output
        .operations
        .iter()
        .try_fold(2usize, |count, operation| {
            count.checked_add(output_file_operation_attempt_count(operation))
        })
}

/// Typed record for the bounded Source-input/Output-file replay grammar.
/// Source events are replayed first in their authored order, followed by the
/// output files. Generated-source handoffs retain exact call order and the
/// filesystem-attempt ordinal after which each file was published. Unselected
/// output files remain ordinary artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputOutputReplayRecord {
    source_input: FilesystemSourceInputReplayRecord,
    output_files: Vec<FilesystemOutputFileReplayRecord>,
    expected_included_sources: Vec<BuildIncludedSource>,
}

impl FilesystemInputOutputReplayRecord {
    pub fn new(
        source_input: FilesystemSourceInputReplayRecord,
        output_files: Vec<FilesystemOutputFileReplayRecord>,
        expected_included_sources: Vec<BuildIncludedSource>,
    ) -> Result<Self, String> {
        if output_files.is_empty() {
            return Err("filesystem replay requires at least one Output file".to_owned());
        }
        validate_output_duplicate_replay(&output_files)?;
        validate_output_lock_replay(&output_files)?;
        let source_attempt_count = source_input
            .events
            .iter()
            .try_fold(0usize, |count, event| {
                count.checked_add(match event {
                    FilesystemSourceInputReplayEventRecord::ReadChain(chain) => {
                        chain.reads.len().checked_add(2)?
                    }
                    FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain) => {
                        chain.attempt_count()?
                    }
                    FilesystemSourceInputReplayEventRecord::ReadLink(_) => 1,
                    FilesystemSourceInputReplayEventRecord::DescriptorMetadata(_) => 3,
                    FilesystemSourceInputReplayEventRecord::PathMetadata(_) => 1,
                })
            })
            .ok_or_else(|| "filesystem replay event count overflowed".to_owned())?;
        let mut descriptor_identities = source_input
            .events
            .iter()
            .filter_map(|event| match event {
                FilesystemSourceInputReplayEventRecord::ReadChain(chain) => {
                    Some(chain.logical_handle_identity)
                }
                FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain) => {
                    Some(chain.logical_handle_identity())
                }
                FilesystemSourceInputReplayEventRecord::ReadLink(_) => None,
                FilesystemSourceInputReplayEventRecord::DescriptorMetadata(metadata) => {
                    Some(metadata.logical_handle_identity)
                }
                FilesystemSourceInputReplayEventRecord::PathMetadata(_) => None,
            })
            .collect::<Vec<_>>();
        for (ordinal, output) in output_files.iter().enumerate() {
            if output_files[..ordinal].iter().any(|prior| {
                (prior.output_root == output.output_root
                    && prior.output_relative_path == output.output_relative_path)
                    || prior.logical_handle_identity == output.logical_handle_identity
            }) {
                return Err(
                    "filesystem replay Output paths and descriptors must be distinct".to_owned(),
                );
            }
            if source_input.events.iter().any(|event| match event {
                FilesystemSourceInputReplayEventRecord::ReadChain(chain) => {
                    chain.source_root == output.output_root
                        || chain.logical_handle_identity == output.logical_handle_identity
                }
                FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain) => {
                    chain.source_root() == output.output_root
                        || chain.logical_handle_identity() == output.logical_handle_identity
                }
                FilesystemSourceInputReplayEventRecord::ReadLink(read_link) => {
                    read_link.source_root() == output.output_root
                        || read_link.authorized_root() == output.output_root
                }
                FilesystemSourceInputReplayEventRecord::DescriptorMetadata(metadata) => {
                    metadata.source_root == output.output_root
                        || metadata.logical_handle_identity == output.logical_handle_identity
                }
                FilesystemSourceInputReplayEventRecord::PathMetadata(metadata) => {
                    metadata.source_root == output.output_root
                        || metadata.authorized_root == output.output_root
                }
            }) {
                return Err(
                    "filesystem replay Source and Output roots and descriptors must be distinct"
                        .to_owned(),
                );
            }
            for identity in output_logical_handle_identities(output) {
                if descriptor_identities.contains(&identity) {
                    return Err(
                        "filesystem replay Source and Output descriptors must be globally distinct"
                            .to_owned(),
                    );
                }
                descriptor_identities.push(identity);
            }
        }
        validate_expected_included_sources(
            &output_files,
            &expected_included_sources,
            source_attempt_count,
        )?;
        Ok(Self {
            source_input,
            output_files,
            expected_included_sources,
        })
    }

    pub const fn source_input(&self) -> &FilesystemSourceInputReplayRecord {
        &self.source_input
    }

    pub fn output_files(&self) -> &[FilesystemOutputFileReplayRecord] {
        &self.output_files
    }

    pub fn expected_included_sources(&self) -> &[BuildIncludedSource] {
        &self.expected_included_sources
    }
}

impl FilesystemReplay {
    pub(crate) fn executes_replay_attempt(&self, attempt_index: usize) -> bool {
        if self
            .attempts
            .get(attempt_index)
            .is_some_and(unknown_input_handle_failure_attempt_is_exact)
        {
            return true;
        }
        self.attempts
            .iter()
            .position(|attempt| filesystem_output_attempt_tag(attempt.operation_tag()))
            .is_some_and(|output_start| attempt_index >= output_start)
    }

    /// Whether this replay contains any Output-rooted operation, including a
    /// failure-only sequence that leaves no final staged-tree entry.
    pub fn has_output_attempts(&self) -> bool {
        self.attempts
            .iter()
            .any(|attempt| filesystem_output_attempt_tag(attempt.operation_tag()))
    }

    pub fn from_source_input_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        if observations.filesystem_operation_schema_version()
            != FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
        {
            return Err("filesystem replay observation schema is not current".to_owned());
        }
        let attempts = observations.filesystem_operation_attempts();
        validate_filesystem_replay_size(attempts)?;
        validate_source_input_attempts(attempts)?;
        Ok(Self {
            attempts: attempts.to_vec().into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    pub fn attempts(&self) -> &[FilesystemOperationAttempt] {
        &self.attempts
    }

    /// Reconstruct the typed Output files retained by this replay. Source-only
    /// records return an empty vector. Public constructors ensure every present
    /// file is exact, distinct, and ordered as authored.
    pub fn output_files(&self) -> Vec<FilesystemOutputFileReplayRecord> {
        self.output_entries()
            .into_iter()
            .filter_map(|entry| match entry {
                FilesystemOutputTreeEntryReplayRecord::Directory(_)
                | FilesystemOutputTreeEntryReplayRecord::HardLink(_)
                | FilesystemOutputTreeEntryReplayRecord::Symlink(_) => None,
                FilesystemOutputTreeEntryReplayRecord::File(file) => Some(file),
            })
            .collect()
    }

    /// Reconstruct all exact Output entries in authored operation order.
    pub fn output_entries(&self) -> Vec<FilesystemOutputTreeEntryReplayRecord> {
        let Some(output_start) = self
            .attempts
            .iter()
            .position(|attempt| filesystem_output_attempt_tag(attempt.operation_tag()))
        else {
            return Vec::new();
        };
        if self.attempts[output_start..]
            .iter()
            .all(output_absent_remove_attempt_is_exact)
        {
            return Vec::new();
        }
        output_tree_entries_from_attempts(&self.attempts[output_start..])
            .expect("validated filesystem replay retains exact Output entries")
    }

    /// Reconstruct the exact ordered Output directories retained by this
    /// replay. File-only and source-only records return an empty vector.
    pub fn output_directories(&self) -> Vec<FilesystemOutputDirectoryReplayRecord> {
        self.output_entries()
            .into_iter()
            .filter_map(|entry| match entry {
                FilesystemOutputTreeEntryReplayRecord::Directory(directory) => Some(directory),
                FilesystemOutputTreeEntryReplayRecord::File(_)
                | FilesystemOutputTreeEntryReplayRecord::HardLink(_)
                | FilesystemOutputTreeEntryReplayRecord::Symlink(_) => None,
            })
            .collect()
    }

    /// Reconstruct exact Output symlinks in authored operation order.
    pub fn output_symlinks(&self) -> Vec<FilesystemOutputSymlinkReplayRecord> {
        self.output_entries()
            .into_iter()
            .filter_map(|entry| match entry {
                FilesystemOutputTreeEntryReplayRecord::Directory(_)
                | FilesystemOutputTreeEntryReplayRecord::File(_)
                | FilesystemOutputTreeEntryReplayRecord::HardLink(_) => None,
                FilesystemOutputTreeEntryReplayRecord::Symlink(symlink) => Some(symlink),
            })
            .collect()
    }

    /// Reconstruct exact Output hard links in authored operation order.
    pub fn output_hard_links(&self) -> Vec<FilesystemOutputHardLinkReplayRecord> {
        self.output_entries()
            .into_iter()
            .filter_map(|entry| match entry {
                FilesystemOutputTreeEntryReplayRecord::Directory(_)
                | FilesystemOutputTreeEntryReplayRecord::File(_)
                | FilesystemOutputTreeEntryReplayRecord::Symlink(_) => None,
                FilesystemOutputTreeEntryReplayRecord::HardLink(hard_link) => Some(hard_link),
            })
            .collect()
    }

    /// Generated-source coordinates expected during Output replay, in exact
    /// authored handoff order and with their filesystem-attempt ordinals.
    pub fn expected_included_sources(&self) -> &[BuildIncludedSource] {
        &self.expected_included_sources
    }

    pub fn from_source_input_record(
        record: FilesystemSourceInputReplayRecord,
    ) -> Result<Self, String> {
        let attempts = source_input_record_attempts(record);
        validate_filesystem_replay_size(&attempts)?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    /// Validate an optional observed Source-input prefix followed by one or
    /// more exact Output entries, plus an exact ordered subset of explicit
    /// generated-source handoffs. A present Source prefix retains the same
    /// closed validation grammar as a source-bearing replay.
    pub fn from_input_output_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        if observations.filesystem_operation_schema_version()
            != FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
        {
            return Err("filesystem replay observation schema is not current".to_owned());
        }
        let attempts = observations.filesystem_operation_attempts();
        validate_filesystem_replay_size(attempts)?;
        let output_start = attempts
            .iter()
            .position(|attempt| filesystem_output_attempt_tag(attempt.operation_tag()))
            .ok_or_else(|| {
                "bounded filesystem replay requires one or more exact Output operations".to_owned()
            })?;
        if output_start > 0 {
            validate_source_input_attempts(&attempts[..output_start])?;
        }
        if attempts[output_start..]
            .iter()
            .all(output_absent_remove_attempt_is_exact)
        {
            validate_output_absent_remove_attempts(
                &attempts[..output_start],
                &attempts[output_start..],
                observations.build_included_sources(),
            )?;
            return Ok(Self {
                attempts: attempts.to_vec().into(),
                expected_included_sources: std::sync::Arc::from([]),
            });
        }
        let output_entries = output_tree_entries_from_attempts(&attempts[output_start..])?;
        validate_observed_output_tree_records(
            &attempts[..output_start],
            &output_entries,
            observations.build_included_sources(),
        )?;
        Ok(Self {
            attempts: attempts.to_vec().into(),
            expected_included_sources: observations.build_included_sources().to_vec().into(),
        })
    }

    /// Construct the closed optional-Source plus failure-only Output rung from
    /// typed compiler-owned coordinates.
    pub fn from_input_output_absent_removes_record(
        record: FilesystemInputOutputAbsentRemovesReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, absent_removes) = record.into_parts();
        let mut attempts = source_input.map_or_else(Vec::new, source_input_record_attempts);
        let output_start = attempts.len();
        attempts.extend(absent_removes.into_iter().map(output_absent_remove_attempt));
        validate_filesystem_replay_size(&attempts)?;
        validate_output_absent_remove_attempts(
            &attempts[..output_start],
            &attempts[output_start..],
            &[],
        )?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    /// Construct the same bounded grammar from already typed records.
    pub fn from_input_output_record(
        record: FilesystemInputOutputReplayRecord,
    ) -> Result<Self, String> {
        validate_output_duplicate_replay(&record.output_files)?;
        validate_output_time_replay_retention(&record.output_files)?;
        validate_output_replay_extents(&record.output_files)?;
        let mut attempts = source_input_record_attempts(record.source_input);
        for output in record.output_files {
            attempts.extend(output_file_attempts(output));
        }
        validate_filesystem_replay_size(&attempts)?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: record.expected_included_sources.into(),
        })
    }

    /// Construct the bounded optional-Source-input plus ordered Output-tree
    /// grammar from typed compiler-owned records. Directory and complete file
    /// entries retain their authored order.
    pub fn from_input_output_tree_record(
        record: FilesystemInputOutputTreeReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, output_entries, expected_included_sources) = record.into_parts();
        let mut attempts = source_input.map_or_else(Vec::new, source_input_record_attempts);
        for entry in output_entries {
            match entry {
                FilesystemOutputTreeEntryReplayRecord::Directory(directory) => {
                    attempts.push(output_directory_attempt(directory));
                }
                FilesystemOutputTreeEntryReplayRecord::File(file) => {
                    attempts.extend(output_file_attempts(file));
                }
                FilesystemOutputTreeEntryReplayRecord::HardLink(hard_link) => {
                    attempts.push(output_hard_link_attempt(hard_link));
                }
                FilesystemOutputTreeEntryReplayRecord::Symlink(symlink) => {
                    attempts.push(output_symlink_attempt(symlink));
                }
            }
        }
        validate_filesystem_replay_size(&attempts)?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: expected_included_sources.into(),
        })
    }

    /// Construct the bounded Source-input plus ordered empty Output-directory
    /// tree grammar from typed compiler-owned records.
    pub fn from_input_output_directory_record(
        record: FilesystemInputOutputDirectoryReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, output_directories) = record.into_parts();
        let mut attempts = source_input_record_attempts(source_input);
        attempts.extend(output_directories.into_iter().map(output_directory_attempt));
        validate_filesystem_replay_size(&attempts)?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }
}

fn validate_output_time_replay_retention(
    outputs: &[FilesystemOutputFileReplayRecord],
) -> Result<(), String> {
    outputs
        .iter()
        .flat_map(|output| output.operations.iter())
        .try_fold(0usize, |retained, operation| {
            let FilesystemOutputFileOperationReplayRecord::SetFileTimes { times } = operation
            else {
                return Some(retained);
            };
            retained.checked_add(times.len().checked_mul(3)?)
        })
        .filter(|retained| *retained <= MAX_FILESYSTEM_REPLAY_RETAINED_BYTES)
        .map(|_| ())
        .ok_or_else(|| {
            format!(
                "filesystem replay Output time carriers exceed the {MAX_FILESYSTEM_REPLAY_RETAINED_BYTES}-byte retained-evidence ceiling"
            )
        })
}

fn validate_output_replay_extents(
    outputs: &[FilesystemOutputFileReplayRecord],
) -> Result<(), String> {
    outputs
        .iter()
        .try_fold(0usize, |total, output| {
            total
                .checked_add(output.replayed_extents()?.1)
                .filter(|extent| *extent <= MAX_FILESYSTEM_REPLAY_RETAINED_BYTES)
                .ok_or_else(|| {
                    format!(
                        "filesystem replay Output exceeds its {MAX_FILESYSTEM_REPLAY_RETAINED_BYTES}-byte aggregate extent ceiling"
                    )
                })
        })
        .map(|_| ())
}

fn validate_expected_included_sources(
    outputs: &[FilesystemOutputFileReplayRecord],
    included_sources: &[BuildIncludedSource],
    source_attempt_count: usize,
) -> Result<(), String> {
    if included_sources.len() > MAX_INCLUDED_BUILD_SOURCES {
        return Err(format!(
            "filesystem replay exceeds its {MAX_INCLUDED_BUILD_SOURCES}-source handoff ceiling"
        ));
    }
    let total_attempt_count = outputs
        .iter()
        .try_fold(source_attempt_count, |count, output| {
            count.checked_add(output_file_attempt_count(output)?)
        })
        .ok_or_else(|| "filesystem replay event count overflowed".to_owned())?;
    let mut previous_ordinal = source_attempt_count;
    for (handoff_index, included) in included_sources.iter().enumerate() {
        if included.filesystem_attempt_ordinal() < previous_ordinal {
            return Err(
                "filesystem replay included-source handoff ordinals must be nondecreasing"
                    .to_owned(),
            );
        }
        previous_ordinal = included.filesystem_attempt_ordinal();
        if included_sources[..handoff_index].iter().any(|prior| {
            prior.root() == included.root() && prior.relative_path() == included.relative_path()
        }) {
            return Err(
                "filesystem replay included-source handoff names one output more than once"
                    .to_owned(),
            );
        }
        let Some(output_index) = outputs.iter().position(|output| {
            output.output_root() == included.root()
                && output.output_relative_path() == included.relative_path()
        }) else {
            return Err(
                "filesystem replay included-source handoff has no matching output file".to_owned(),
            );
        };
        let earliest_ordinal = outputs[..=output_index]
            .iter()
            .try_fold(source_attempt_count, |count, output| {
                count.checked_add(output_file_attempt_count(output)?)
            })
            .ok_or_else(|| "filesystem replay event count overflowed".to_owned())?;
        if included.filesystem_attempt_ordinal() < earliest_ordinal
            || included.filesystem_attempt_ordinal() > total_attempt_count
        {
            return Err(
                "filesystem replay included-source handoff must follow its exact Output close"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

fn validate_source_input_attempts(attempts: &[FilesystemOperationAttempt]) -> Result<(), String> {
    let mut cursor = 0;
    let mut event_count = 0;
    while cursor < attempts.len() {
        if filesystem_output_attempt_tag(attempts[cursor].operation_tag()) {
            break;
        }
        if attempts[cursor].operation_tag() == 21 {
            if !source_read_link_attempt_is_exact(&attempts[cursor]) {
                return Err(
                    "bounded filesystem replay source read-link event is inconsistent".to_owned(),
                );
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        if matches!(attempts[cursor].operation_tag(), 38 | 40) {
            if !source_path_metadata_attempt_is_exact(&attempts[cursor]) {
                return Err("bounded filesystem replay source metadata is inconsistent".to_owned());
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        if attempts[cursor].operation_tag() != 2 {
            return Err(
                "bounded filesystem replay requires ordered source-input events".to_owned(),
            );
        }
        let event_start = cursor;
        cursor += 1;
        if cursor < attempts.len() && attempts[cursor].operation_tag() == 39 {
            cursor += 1;
            if cursor == attempts.len() || attempts[cursor].operation_tag() != 8 {
                return Err(
                    "bounded filesystem replay requires ordered source-input events".to_owned(),
                );
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        if cursor < attempts.len() && attempts[cursor].operation_tag() == 23 {
            while cursor < attempts.len() && attempts[cursor].operation_tag() == 23 {
                cursor += 1;
            }
            if cursor == attempts.len()
                || attempts[cursor].operation_tag() != 8
                || !source_directory_chain_is_exact(&attempts[event_start..=cursor])
            {
                return Err(
                    "bounded filesystem replay Source directory chain is inconsistent".to_owned(),
                );
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        let reads_start = cursor;
        while cursor < attempts.len() && matches!(attempts[cursor].operation_tag(), 4 | 6) {
            cursor += 1;
        }
        if cursor == reads_start
            || cursor == attempts.len()
            || attempts[cursor].operation_tag() != 8
        {
            return Err(
                "bounded filesystem replay requires ordered source-input events".to_owned(),
            );
        }
        cursor += 1;
        event_count += 1;
    }
    if event_count == 0 {
        return Err("bounded filesystem replay requires source-input events".to_owned());
    }
    for (index, attempt) in attempts.iter().enumerate() {
        if !matches!(
            attempt.outcome,
            Some(FilesystemOperationAttemptOutcome::Returned { .. })
        ) {
            return Err(format!(
                "filesystem replay event {index} did not return normally"
            ));
        }
    }
    Ok(())
}

fn validate_filesystem_replay_size(attempts: &[FilesystemOperationAttempt]) -> Result<(), String> {
    let mut retained = attempts
        .len()
        .checked_mul(FILESYSTEM_REPLAY_ATTEMPT_RETENTION_WEIGHT);
    let mut add = |weight: usize| {
        retained = retained
            .and_then(|total| total.checked_add(weight))
            .filter(|total| *total <= MAX_FILESYSTEM_REPLAY_RETENTION_WEIGHT);
    };
    let lane_weight =
        |length: usize, weight: usize| length.checked_mul(weight).unwrap_or(usize::MAX);
    for attempt in attempts {
        add(lane_weight(
            attempt.scalar_operands.len(),
            FILESYSTEM_REPLAY_SCALAR_OPERAND_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.byte_operands.len(),
            FILESYSTEM_REPLAY_BYTE_OPERAND_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.path_like_operands.len(),
            FILESYSTEM_REPLAY_PATH_LIKE_OPERAND_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.rooted_path_operand_resolutions.len(),
            FILESYSTEM_REPLAY_ROOTED_PATH_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.returned_paths.len(),
            FILESYSTEM_REPLAY_RETURNED_PATH_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.observed_byte_regions.len(),
            FILESYSTEM_REPLAY_OBSERVED_REGION_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.metadata_observations.len(),
            FILESYSTEM_REPLAY_METADATA_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.mutable_byte_operand_resolutions.len(),
            FILESYSTEM_REPLAY_MUTABLE_BYTE_RESOLUTION_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.mutable_i64_operand_resolutions.len(),
            FILESYSTEM_REPLAY_MUTABLE_I64_RESOLUTION_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.mutable_byte_operands.len(),
            FILESYSTEM_REPLAY_MUTABLE_BYTE_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.mutable_i64_operands.len(),
            FILESYSTEM_REPLAY_MUTABLE_I64_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.authorized_paths.len(),
            FILESYSTEM_REPLAY_AUTHORIZED_PATH_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.logical_handle_inputs.len(),
            FILESYSTEM_REPLAY_LOGICAL_INPUT_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.retired_logical_handles.len(),
            FILESYSTEM_REPLAY_RETIRED_HANDLE_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.grant_refusals.len(),
            FILESYSTEM_REPLAY_GRANT_REFUSAL_RETENTION_WEIGHT,
        ));
        for operand in &attempt.byte_operands {
            add(operand.bytes.len());
        }
        for operand in &attempt.path_like_operands {
            add(operand.bytes.len());
        }
        for operand in &attempt.rooted_path_operand_resolutions {
            add(operand.relative_path.len());
        }
        for returned in &attempt.returned_paths {
            add(returned.bytes.len());
        }
        for operand in &attempt.mutable_byte_operand_resolutions {
            add(operand.bytes.len());
        }
        for operand in &attempt.mutable_byte_operands {
            add(operand.pre_bytes.len());
            add(operand.post_bytes.len());
        }
        for path in &attempt.authorized_paths {
            add(path.relative_path.len());
        }
    }
    retained
        .map(|_| ())
        .ok_or_else(|| format!(
            "filesystem replay attempts exceed their {MAX_FILESYSTEM_REPLAY_RETENTION_WEIGHT}-unit deterministic retention-weight ceiling"
        ))
}

fn source_input_record_attempts(
    record: FilesystemSourceInputReplayRecord,
) -> Vec<FilesystemOperationAttempt> {
    let attempt_count = record.events.iter().fold(0usize, |count, event| {
        count
            + match event {
                FilesystemSourceInputReplayEventRecord::ReadChain(chain) => chain.reads.len() + 2,
                FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain) => chain
                    .attempt_count()
                    .expect("typed directory replay count fits"),
                FilesystemSourceInputReplayEventRecord::ReadLink(_) => 1,
                FilesystemSourceInputReplayEventRecord::DescriptorMetadata(_) => 3,
                FilesystemSourceInputReplayEventRecord::PathMetadata(_) => 1,
            }
    });
    let mut attempts = Vec::with_capacity(attempt_count);
    for event in record.events {
        match event {
            FilesystemSourceInputReplayEventRecord::ReadChain(chain) => {
                attempts.extend(source_read_chain_attempts(chain));
            }
            FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain) => {
                attempts.extend(source_directory_chain_attempts(chain));
            }
            FilesystemSourceInputReplayEventRecord::ReadLink(read_link) => {
                attempts.push(source_read_link_attempt(read_link));
            }
            FilesystemSourceInputReplayEventRecord::DescriptorMetadata(metadata) => {
                attempts.extend(source_descriptor_metadata_attempts(metadata));
            }
            FilesystemSourceInputReplayEventRecord::PathMetadata(metadata) => {
                attempts.push(source_path_metadata_attempt(metadata));
            }
        }
    }
    attempts
}

fn source_attempts_overlap_output(
    attempts: &[FilesystemOperationAttempt],
    output_root: FilesystemGrantRootIdentity,
    output_identity: FilesystemLogicalHandleIdentity,
) -> bool {
    attempts.iter().any(|attempt| {
        attempt
            .rooted_path_operand_resolutions
            .iter()
            .any(|path| path.root == output_root)
            || attempt
                .authorized_paths
                .iter()
                .any(|path| path.root == output_root)
            || attempt.logical_handle_output.is_some_and(|output| {
                output.identity == output_identity
                    || matches!(
                        output.source,
                        FilesystemLogicalHandleOutputSource::Duplicated(source)
                            | FilesystemLogicalHandleOutputSource::Borrowed(source)
                            if source == output_identity
                    )
            })
            || attempt.logical_handle_inputs.iter().any(|input| {
                input.resolution
                    == FilesystemLogicalHandleInputResolution::Resolved(output_identity)
            })
            || attempt.retired_logical_handles.contains(&output_identity)
            || attempt.result() == Some(FilesystemOperationResult::LogicalHandle(output_identity))
    })
}

fn output_file_record_from_attempts(
    attempts: &[FilesystemOperationAttempt],
) -> Result<FilesystemOutputFileReplayRecord, String> {
    let Some((create, remainder)) = attempts.split_first() else {
        return Err("bounded filesystem replay requires a complete Output file".to_owned());
    };
    let Some((close, operations)) = remainder.split_last() else {
        return Err("bounded filesystem replay requires a complete Output file".to_owned());
    };

    let [create_mode] = create.scalar_operands.as_slice() else {
        return Err("filesystem replay Output create lanes are inconsistent".to_owned());
    };
    let [rooted] = create.rooted_path_operand_resolutions.as_slice() else {
        return Err("filesystem replay Output create lanes are inconsistent".to_owned());
    };
    let [authorized] = create.authorized_paths.as_slice() else {
        return Err("filesystem replay Output create lanes are inconsistent".to_owned());
    };
    let Some(logical_output) = create.logical_handle_output else {
        return Err("filesystem replay Output create lanes are inconsistent".to_owned());
    };
    let Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::LogicalHandle(create_result),
        post_error: create_post_error,
    }) = create.outcome
    else {
        return Err("filesystem replay Output create must succeed".to_owned());
    };
    if create.operation_tag != 1
        || create.provider != FilesystemObservationProvider::RealScoped
        || create_mode.operand_ordinal != 1
        || create_mode.value
            != FilesystemScalarOperandValue::I32(FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE)
        || rooted.operand_ordinal != 0
        || !filesystem_root_relative_path_is_canonical(&rooted.relative_path, false)
        || authorized.operand_ordinal != 0
        || authorized.access != FilesystemGrantAccess::Write
        || authorized.root != rooted.root
        || authorized.relative_path != rooted.relative_path
        || logical_output.kind != FilesystemLogicalHandleKind::Descriptor
        || logical_output.identity != create_result
        || logical_output.source != FilesystemLogicalHandleOutputSource::Created
        || !create.byte_operands.is_empty()
        || !create.path_like_operands.is_empty()
        || !create.returned_paths.is_empty()
        || !create.observed_byte_regions.is_empty()
        || !create.metadata_observations.is_empty()
        || !create.mutable_byte_operand_resolutions.is_empty()
        || !create.mutable_i64_operand_resolutions.is_empty()
        || !create.mutable_byte_operands.is_empty()
        || !create.mutable_i64_operands.is_empty()
        || !create.logical_handle_inputs.is_empty()
        || !create.retired_logical_handles.is_empty()
        || !create.grant_refusals.is_empty()
    {
        return Err("filesystem replay Output create lanes are inconsistent".to_owned());
    }

    let mut operation_records = Vec::new();
    operation_records
        .try_reserve_exact(operations.len())
        .map_err(|_| "filesystem replay Output operation allocation failed".to_owned())?;
    let mut operation_cursor = 0;
    while operation_cursor < operations.len() {
        let operation = &operations[operation_cursor];
        if operation.operation_tag == 45 {
            let close_duplicate = operations.get(operation_cursor + 1).ok_or_else(|| {
                "filesystem replay Output duplicate is not immediately retired".to_owned()
            })?;
            operation_records.push(
                FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(
                    output_duplicate_record_from_attempts(
                        operation,
                        close_duplicate,
                        create_result,
                    )?,
                ),
            );
            operation_cursor += 2;
            continue;
        }
        if operation.operation_tag == 46 {
            let release = operations.get(operation_cursor + 1).ok_or_else(|| {
                "filesystem replay Output lock is not immediately released".to_owned()
            })?;
            operation_records.push(FilesystemOutputFileOperationReplayRecord::LockAndUnlock(
                output_lock_record_from_attempts(operation, release, create_result)?,
            ));
            operation_cursor += 2;
            continue;
        }
        operation_records.push(match operation.operation_tag {
            5 | 7 => FilesystemOutputFileOperationReplayRecord::Write(
                output_write_record_from_attempt(operation, create_result)?,
            ),
            10 => output_seek_record_from_attempt(operation, create_result)?,
            17 => output_set_file_permissions_record_from_attempt(operation, create_result)?,
            41 => output_set_length_record_from_attempt(operation, create_result)?,
            42 => output_set_file_times_record_from_attempt(operation, create_result)?,
            43 | 44 => output_sync_record_from_attempt(operation, create_result)?,
            _ => return Err("filesystem replay Output operation is unsupported".to_owned()),
        });
        operation_cursor += 1;
    }

    let [close_input] = close.logical_handle_inputs.as_slice() else {
        return Err("filesystem replay Output close lanes are inconsistent".to_owned());
    };
    let [retired] = close.retired_logical_handles.as_slice() else {
        return Err("filesystem replay Output close lanes are inconsistent".to_owned());
    };
    let Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(0),
        post_error: close_post_error,
    }) = close.outcome
    else {
        return Err("filesystem replay Output close must succeed".to_owned());
    };
    if close.operation_tag != 8
        || close.provider != FilesystemObservationProvider::RealScoped
        || close_input.operand_ordinal != 0
        || close_input.kind != FilesystemLogicalHandleKind::Descriptor
        || close_input.resolution != FilesystemLogicalHandleInputResolution::Resolved(create_result)
        || *retired != create_result
        || !close.scalar_operands.is_empty()
        || !close.byte_operands.is_empty()
        || !close.path_like_operands.is_empty()
        || !close.rooted_path_operand_resolutions.is_empty()
        || !close.returned_paths.is_empty()
        || !close.observed_byte_regions.is_empty()
        || !close.metadata_observations.is_empty()
        || !close.mutable_byte_operand_resolutions.is_empty()
        || !close.mutable_i64_operand_resolutions.is_empty()
        || !close.mutable_byte_operands.is_empty()
        || !close.mutable_i64_operands.is_empty()
        || !close.authorized_paths.is_empty()
        || close.logical_handle_output.is_some()
        || !close.grant_refusals.is_empty()
    {
        return Err("filesystem replay Output close lanes are inconsistent".to_owned());
    }

    FilesystemOutputFileReplayRecord::with_operations(
        rooted.root,
        rooted.relative_path.clone(),
        create_result.get(),
        create_post_error,
        operation_records,
        close_post_error,
    )
}

fn output_seek_record_from_attempt(
    operation: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> Result<FilesystemOutputFileOperationReplayRecord, String> {
    let [
        FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I64(offset),
        },
        FilesystemScalarOperand {
            operand_ordinal: 2,
            value: FilesystemScalarOperandValue::I32(whence),
        },
    ] = operation.scalar_operands.as_slice()
    else {
        return Err("filesystem replay Output seek has no exact offset and whence".to_owned());
    };
    let [input] = operation.logical_handle_inputs.as_slice() else {
        return Err("filesystem replay Output seek lanes are inconsistent".to_owned());
    };
    let Some(FilesystemOperationResult::Scalar(result)) = operation.result() else {
        return Err("filesystem replay Output seek did not return a scalar".to_owned());
    };
    if !matches!(*whence, 0 | 1 | 2)
        || result < 0
        || operation.provider != FilesystemObservationProvider::RealScoped
        || operation.post_error() != Some(0)
        || input.operand_ordinal != 0
        || input.kind != FilesystemLogicalHandleKind::Descriptor
        || input.resolution != FilesystemLogicalHandleInputResolution::Resolved(identity)
        || !operation.byte_operands.is_empty()
        || !operation.path_like_operands.is_empty()
        || !operation.rooted_path_operand_resolutions.is_empty()
        || !operation.returned_paths.is_empty()
        || !operation.observed_byte_regions.is_empty()
        || !operation.metadata_observations.is_empty()
        || !operation.mutable_byte_operand_resolutions.is_empty()
        || !operation.mutable_i64_operand_resolutions.is_empty()
        || !operation.mutable_byte_operands.is_empty()
        || !operation.mutable_i64_operands.is_empty()
        || !operation.authorized_paths.is_empty()
        || operation.logical_handle_output.is_some()
        || !operation.retired_logical_handles.is_empty()
        || !operation.grant_refusals.is_empty()
    {
        return Err("filesystem replay Output seek lanes are inconsistent".to_owned());
    }
    Ok(FilesystemOutputFileOperationReplayRecord::Seek {
        offset: *offset,
        whence: *whence,
        result,
    })
}

fn output_set_length_record_from_attempt(
    operation: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> Result<FilesystemOutputFileOperationReplayRecord, String> {
    let [
        FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I64(length),
        },
    ] = operation.scalar_operands.as_slice()
    else {
        return Err("filesystem replay Output set_len has no exact length".to_owned());
    };
    let [input] = operation.logical_handle_inputs.as_slice() else {
        return Err("filesystem replay Output set_len lanes are inconsistent".to_owned());
    };
    if *length < 0
        || usize::try_from(*length).is_err()
        || operation.provider != FilesystemObservationProvider::RealScoped
        || operation.result() != Some(FilesystemOperationResult::Scalar(0))
        || operation.post_error() != Some(0)
        || input.operand_ordinal != 0
        || input.kind != FilesystemLogicalHandleKind::Descriptor
        || input.resolution != FilesystemLogicalHandleInputResolution::Resolved(identity)
        || !operation.byte_operands.is_empty()
        || !operation.path_like_operands.is_empty()
        || !operation.rooted_path_operand_resolutions.is_empty()
        || !operation.returned_paths.is_empty()
        || !operation.observed_byte_regions.is_empty()
        || !operation.metadata_observations.is_empty()
        || !operation.mutable_byte_operand_resolutions.is_empty()
        || !operation.mutable_i64_operand_resolutions.is_empty()
        || !operation.mutable_byte_operands.is_empty()
        || !operation.mutable_i64_operands.is_empty()
        || !operation.authorized_paths.is_empty()
        || operation.logical_handle_output.is_some()
        || !operation.retired_logical_handles.is_empty()
        || !operation.grant_refusals.is_empty()
    {
        return Err("filesystem replay Output set_len lanes are inconsistent".to_owned());
    }
    Ok(FilesystemOutputFileOperationReplayRecord::SetLength { length: *length })
}

fn output_set_file_permissions_record_from_attempt(
    operation: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> Result<FilesystemOutputFileOperationReplayRecord, String> {
    let [
        FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::U32(mode),
        },
    ] = operation.scalar_operands.as_slice()
    else {
        return Err("filesystem replay Output set_file_permissions has no exact mode".to_owned());
    };
    let [input] = operation.logical_handle_inputs.as_slice() else {
        return Err(
            "filesystem replay Output set_file_permissions lanes are inconsistent".to_owned(),
        );
    };
    if operation.provider != FilesystemObservationProvider::RealScoped
        || operation.result() != Some(FilesystemOperationResult::Scalar(0))
        || operation.post_error() != Some(0)
        || input.operand_ordinal != 0
        || input.kind != FilesystemLogicalHandleKind::Descriptor
        || input.resolution != FilesystemLogicalHandleInputResolution::Resolved(identity)
        || !operation.byte_operands.is_empty()
        || !operation.path_like_operands.is_empty()
        || !operation.rooted_path_operand_resolutions.is_empty()
        || !operation.returned_paths.is_empty()
        || !operation.observed_byte_regions.is_empty()
        || !operation.metadata_observations.is_empty()
        || !operation.mutable_byte_operand_resolutions.is_empty()
        || !operation.mutable_i64_operand_resolutions.is_empty()
        || !operation.mutable_byte_operands.is_empty()
        || !operation.mutable_i64_operands.is_empty()
        || !operation.authorized_paths.is_empty()
        || operation.logical_handle_output.is_some()
        || !operation.retired_logical_handles.is_empty()
        || !operation.grant_refusals.is_empty()
    {
        return Err(
            "filesystem replay Output set_file_permissions lanes are inconsistent".to_owned(),
        );
    }
    Ok(FilesystemOutputFileOperationReplayRecord::SetFilePermissions { mode: *mode })
}

fn output_set_file_times_record_from_attempt(
    operation: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> Result<FilesystemOutputFileOperationReplayRecord, String> {
    let [input] = operation.logical_handle_inputs.as_slice() else {
        return Err("filesystem replay Output set_file_times lanes are inconsistent".to_owned());
    };
    let [resolution] = operation.mutable_byte_operand_resolutions.as_slice() else {
        return Err(
            "filesystem replay Output set_file_times has no exact input carrier".to_owned(),
        );
    };
    let [carrier] = operation.mutable_byte_operands.as_slice() else {
        return Err(
            "filesystem replay Output set_file_times has no exact provider carrier".to_owned(),
        );
    };
    if operation.provider != FilesystemObservationProvider::RealScoped
        || operation.result() != Some(FilesystemOperationResult::Scalar(0))
        || operation.post_error() != Some(0)
        || input.operand_ordinal != 0
        || input.kind != FilesystemLogicalHandleKind::Descriptor
        || input.resolution != FilesystemLogicalHandleInputResolution::Resolved(identity)
        || resolution.operand_ordinal != 1
        || carrier.operand_ordinal != 1
        || resolution.bytes.len() < 32
        || resolution.bytes != carrier.pre_bytes
        || carrier.pre_bytes != carrier.post_bytes
        || !operation.scalar_operands.is_empty()
        || !operation.byte_operands.is_empty()
        || !operation.path_like_operands.is_empty()
        || !operation.rooted_path_operand_resolutions.is_empty()
        || !operation.returned_paths.is_empty()
        || !operation.observed_byte_regions.is_empty()
        || !operation.metadata_observations.is_empty()
        || !operation.mutable_i64_operand_resolutions.is_empty()
        || !operation.mutable_i64_operands.is_empty()
        || !operation.authorized_paths.is_empty()
        || operation.logical_handle_output.is_some()
        || !operation.retired_logical_handles.is_empty()
        || !operation.grant_refusals.is_empty()
    {
        return Err("filesystem replay Output set_file_times lanes are inconsistent".to_owned());
    }
    Ok(FilesystemOutputFileOperationReplayRecord::SetFileTimes {
        times: resolution.bytes.clone(),
    })
}

fn output_sync_record_from_attempt(
    operation: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> Result<FilesystemOutputFileOperationReplayRecord, String> {
    let [input] = operation.logical_handle_inputs.as_slice() else {
        return Err("filesystem replay Output sync lanes are inconsistent".to_owned());
    };
    if !matches!(operation.operation_tag, 43 | 44)
        || operation.provider != FilesystemObservationProvider::RealScoped
        || operation.result() != Some(FilesystemOperationResult::Scalar(0))
        || operation.post_error() != Some(0)
        || input.operand_ordinal != 0
        || input.kind != FilesystemLogicalHandleKind::Descriptor
        || input.resolution != FilesystemLogicalHandleInputResolution::Resolved(identity)
        || !operation.scalar_operands.is_empty()
        || !operation.byte_operands.is_empty()
        || !operation.path_like_operands.is_empty()
        || !operation.rooted_path_operand_resolutions.is_empty()
        || !operation.returned_paths.is_empty()
        || !operation.observed_byte_regions.is_empty()
        || !operation.metadata_observations.is_empty()
        || !operation.mutable_byte_operand_resolutions.is_empty()
        || !operation.mutable_i64_operand_resolutions.is_empty()
        || !operation.mutable_byte_operands.is_empty()
        || !operation.mutable_i64_operands.is_empty()
        || !operation.authorized_paths.is_empty()
        || operation.logical_handle_output.is_some()
        || !operation.retired_logical_handles.is_empty()
        || !operation.grant_refusals.is_empty()
    {
        return Err("filesystem replay Output sync lanes are inconsistent".to_owned());
    }
    Ok(if operation.operation_tag == 43 {
        FilesystemOutputFileOperationReplayRecord::Sync
    } else {
        FilesystemOutputFileOperationReplayRecord::SyncData
    })
}

fn output_write_record_from_attempt(
    write: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> Result<FilesystemOutputWriteReplayRecord, String> {
    let [write_bytes] = write.byte_operands.as_slice() else {
        return Err("filesystem replay Output write lanes are inconsistent".to_owned());
    };
    let [write_input] = write.logical_handle_inputs.as_slice() else {
        return Err("filesystem replay Output write lanes are inconsistent".to_owned());
    };
    let Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(write_result),
        post_error: write_post_error,
    }) = write.outcome
    else {
        return Err("filesystem replay Output write must succeed".to_owned());
    };
    let kind_is_exact = match write.operation_tag {
        5 => write.scalar_operands.is_empty(),
        7 => matches!(
            write.scalar_operands.as_slice(),
            [FilesystemScalarOperand {
                operand_ordinal: 2,
                value: FilesystemScalarOperandValue::I64(offset),
            }] if *offset >= 0
        ),
        _ => false,
    };
    if !kind_is_exact
        || write.provider != FilesystemObservationProvider::RealScoped
        || write_bytes.operand_ordinal != 1
        || write_input.operand_ordinal != 0
        || write_input.kind != FilesystemLogicalHandleKind::Descriptor
        || write_input.resolution != FilesystemLogicalHandleInputResolution::Resolved(identity)
        || !write.path_like_operands.is_empty()
        || !write.rooted_path_operand_resolutions.is_empty()
        || !write.returned_paths.is_empty()
        || !write.observed_byte_regions.is_empty()
        || !write.metadata_observations.is_empty()
        || !write.mutable_byte_operand_resolutions.is_empty()
        || !write.mutable_i64_operand_resolutions.is_empty()
        || !write.mutable_byte_operands.is_empty()
        || !write.mutable_i64_operands.is_empty()
        || !write.authorized_paths.is_empty()
        || write.logical_handle_output.is_some()
        || !write.retired_logical_handles.is_empty()
        || !write.grant_refusals.is_empty()
    {
        return Err("filesystem replay Output write lanes are inconsistent".to_owned());
    }
    match write.operation_tag {
        5 => FilesystemOutputWriteReplayRecord::new(
            write_bytes.bytes.clone(),
            write_result,
            write_post_error,
        ),
        7 => {
            let [
                FilesystemScalarOperand {
                    value: FilesystemScalarOperandValue::I64(offset),
                    ..
                },
            ] = write.scalar_operands.as_slice()
            else {
                unreachable!("validated positioned write has one i64 offset")
            };
            FilesystemOutputWriteReplayRecord::positioned(
                *offset,
                write_bytes.bytes.clone(),
                write_result,
                write_post_error,
            )
        }
        _ => unreachable!("validated output write has a supported operation"),
    }
}

fn output_file_attempt_end(
    attempts: &[FilesystemOperationAttempt],
    start: usize,
) -> Result<usize, String> {
    if attempts
        .get(start)
        .is_none_or(|attempt| attempt.operation_tag() != 1)
    {
        return Err("filesystem replay Output file must begin with create".to_owned());
    }
    let Some(root_identity) = attempts[start]
        .logical_handle_output
        .map(|output| output.identity)
    else {
        return Err("filesystem replay Output create has no descriptor identity".to_owned());
    };
    let mut cursor = start + 1;
    loop {
        if cursor == attempts.len() {
            return Err(
                "bounded filesystem replay requires complete create-operation*-close Output files"
                    .to_owned(),
            );
        }
        if matches!(
            attempts[cursor].operation_tag(),
            5 | 7 | 10 | 17 | 41 | 42 | 43 | 44
        ) {
            cursor += 1;
            continue;
        }
        if attempts[cursor].operation_tag() == 45 {
            if cursor + 1 >= attempts.len() || attempts[cursor + 1].operation_tag() != 8 {
                return Err(
                    "filesystem replay Output duplicate must be immediately retired".to_owned(),
                );
            }
            cursor += 2;
            continue;
        }
        if attempts[cursor].operation_tag() == 46 {
            if cursor + 1 >= attempts.len() || attempts[cursor + 1].operation_tag() != 46 {
                return Err("filesystem replay Output lock must be immediately released".to_owned());
            }
            cursor += 2;
            continue;
        }
        let closes_root = attempts[cursor].operation_tag() == 8
            && matches!(
                attempts[cursor].logical_handle_inputs.as_slice(),
                [FilesystemLogicalHandleInput {
                    resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
                    ..
                }] if *identity == root_identity
            );
        if closes_root {
            return Ok(cursor + 1);
        }
        return Err(
            "bounded filesystem replay requires complete create-operation*-close Output files"
                .to_owned(),
        );
    }
}

fn filesystem_output_attempt_tag(operation_tag: u16) -> bool {
    matches!(operation_tag, 1 | 9 | 11 | 12 | 19 | 20 | 27)
}

fn output_absent_remove_attempt_is_exact(attempt: &FilesystemOperationAttempt) -> bool {
    output_absent_remove_record_from_attempt(attempt).is_ok()
}

fn validate_output_absent_remove_attempts(
    source_attempts: &[FilesystemOperationAttempt],
    attempts: &[FilesystemOperationAttempt],
    included_sources: &[BuildIncludedSource],
) -> Result<(), String> {
    if attempts.is_empty() {
        return Err("filesystem replay requires at least one absent Output remove".to_owned());
    }
    if !included_sources.is_empty() {
        return Err(
            "filesystem replay failure-only Output operations cannot hand off generated sources"
                .to_owned(),
        );
    }
    let mut records = Vec::new();
    records
        .try_reserve_exact(attempts.len())
        .map_err(|_| "filesystem replay absent Output remove allocation failed".to_owned())?;
    for attempt in attempts {
        records.push(output_absent_remove_record_from_attempt(attempt)?);
    }
    let record = FilesystemInputOutputAbsentRemovesReplayRecord::new(None, records)?;
    let (_, records) = record.into_parts();
    let output_root = records[0].output_root();
    if source_attempts_use_root(source_attempts, output_root) {
        return Err("filesystem replay Source and Output roots must be distinct".to_owned());
    }
    Ok(())
}

fn output_tree_entries_from_attempts(
    attempts: &[FilesystemOperationAttempt],
) -> Result<Vec<FilesystemOutputTreeEntryReplayRecord>, String> {
    if attempts.is_empty() {
        return Err("bounded filesystem replay requires Output entries".to_owned());
    }
    let mut entries = Vec::new();
    let mut cursor = 0;
    while cursor < attempts.len() {
        match attempts[cursor].operation_tag() {
            11 => {
                entries.push(FilesystemOutputTreeEntryReplayRecord::Directory(
                    output_directory_record_from_attempt(&attempts[cursor])?,
                ));
                cursor += 1;
            }
            1 => {
                let end = output_file_attempt_end(attempts, cursor)?;
                entries.push(FilesystemOutputTreeEntryReplayRecord::File(
                    output_file_record_from_attempts(&attempts[cursor..end])?,
                ));
                cursor = end;
            }
            20 => {
                entries.push(FilesystemOutputTreeEntryReplayRecord::Symlink(
                    output_symlink_record_from_attempt(&attempts[cursor])?,
                ));
                cursor += 1;
            }
            19 | 27 => {
                entries.push(FilesystemOutputTreeEntryReplayRecord::HardLink(
                    output_hard_link_record_from_attempt(&attempts[cursor])?,
                ));
                cursor += 1;
            }
            _ => {
                return Err("bounded filesystem replay requires ordered Output entries".to_owned());
            }
        }
    }
    Ok(entries)
}

fn output_file_attempts(
    record: FilesystemOutputFileReplayRecord,
) -> Vec<FilesystemOperationAttempt> {
    let identity = record.logical_handle_identity;
    let create = FilesystemOperationAttempt {
        operation_tag: 1,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::LogicalHandle(identity),
            post_error: record.create_post_error,
        }),
        scalar_operands: vec![FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I32(FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE),
        }],
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: vec![FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: record.output_root,
            relative_path: record.output_relative_path.clone(),
        }],
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: Vec::new(),
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: Vec::new(),
        mutable_i64_operands: Vec::new(),
        authorized_paths: vec![FilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: FilesystemGrantAccess::Write,
            root: record.output_root,
            relative_path: record.output_relative_path,
        }],
        logical_handle_inputs: Vec::new(),
        logical_handle_output: Some(FilesystemLogicalHandleOutput {
            kind: FilesystemLogicalHandleKind::Descriptor,
            identity,
            source: FilesystemLogicalHandleOutputSource::Created,
        }),
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    };
    let operation_capacity = record
        .operations
        .iter()
        .map(output_file_operation_attempt_count)
        .sum();
    let mut operations = Vec::with_capacity(operation_capacity);
    for operation in record.operations {
        match operation {
            FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(duplicate) => {
                operations.extend(output_duplicate_attempts(identity, duplicate));
            }
            FilesystemOutputFileOperationReplayRecord::LockAndUnlock(lock) => {
                operations.extend(output_lock_attempts(identity, lock));
            }
            operation => operations.push(output_file_operation_attempt(operation, identity)),
        }
    }
    let close = FilesystemOperationAttempt {
        operation_tag: 8,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: record.close_post_error,
        }),
        scalar_operands: Vec::new(),
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: Vec::new(),
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: Vec::new(),
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: Vec::new(),
        mutable_i64_operands: Vec::new(),
        authorized_paths: Vec::new(),
        logical_handle_inputs: vec![FilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Descriptor,
            resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
        }],
        logical_handle_output: None,
        retired_logical_handles: vec![identity],
        grant_refusals: Vec::new(),
    };
    let mut attempts = Vec::with_capacity(operations.len() + 2);
    attempts.push(create);
    attempts.extend(operations);
    attempts.push(close);
    attempts
}

fn output_file_operation_attempt(
    operation: FilesystemOutputFileOperationReplayRecord,
    identity: FilesystemLogicalHandleIdentity,
) -> FilesystemOperationAttempt {
    match operation {
        FilesystemOutputFileOperationReplayRecord::Write(write) => FilesystemOperationAttempt {
            operation_tag: match write.kind {
                FilesystemOutputWriteReplayKind::Sequential => 5,
                FilesystemOutputWriteReplayKind::Positioned { .. } => 7,
            },
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(write.result),
                post_error: write.post_error,
            }),
            scalar_operands: match write.kind {
                FilesystemOutputWriteReplayKind::Sequential => Vec::new(),
                FilesystemOutputWriteReplayKind::Positioned { offset } => {
                    vec![FilesystemScalarOperand {
                        operand_ordinal: 2,
                        value: FilesystemScalarOperandValue::I64(offset),
                    }]
                }
            },
            byte_operands: vec![FilesystemByteOperand {
                operand_ordinal: 1,
                bytes: write.bytes,
            }],
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: Vec::new(),
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: Vec::new(),
            mutable_i64_operand_resolutions: Vec::new(),
            mutable_byte_operands: Vec::new(),
            mutable_i64_operands: Vec::new(),
            authorized_paths: Vec::new(),
            logical_handle_inputs: vec![FilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: FilesystemLogicalHandleKind::Descriptor,
                resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
            }],
            logical_handle_output: None,
            retired_logical_handles: Vec::new(),
            grant_refusals: Vec::new(),
        },
        FilesystemOutputFileOperationReplayRecord::Seek {
            offset,
            whence,
            result,
        } => FilesystemOperationAttempt {
            operation_tag: 10,
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(result),
                post_error: 0,
            }),
            scalar_operands: vec![
                FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::I64(offset),
                },
                FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::I32(whence),
                },
            ],
            byte_operands: Vec::new(),
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: Vec::new(),
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: Vec::new(),
            mutable_i64_operand_resolutions: Vec::new(),
            mutable_byte_operands: Vec::new(),
            mutable_i64_operands: Vec::new(),
            authorized_paths: Vec::new(),
            logical_handle_inputs: vec![FilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: FilesystemLogicalHandleKind::Descriptor,
                resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
            }],
            logical_handle_output: None,
            retired_logical_handles: Vec::new(),
            grant_refusals: Vec::new(),
        },
        FilesystemOutputFileOperationReplayRecord::SetLength { length } => {
            FilesystemOperationAttempt {
                operation_tag: 41,
                provider: FilesystemObservationProvider::RealScoped,
                outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                    result: FilesystemOperationResult::Scalar(0),
                    post_error: 0,
                }),
                scalar_operands: vec![FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::I64(length),
                }],
                byte_operands: Vec::new(),
                path_like_operands: Vec::new(),
                rooted_path_operand_resolutions: Vec::new(),
                returned_paths: Vec::new(),
                observed_byte_regions: Vec::new(),
                metadata_observations: Vec::new(),
                mutable_byte_operand_resolutions: Vec::new(),
                mutable_i64_operand_resolutions: Vec::new(),
                mutable_byte_operands: Vec::new(),
                mutable_i64_operands: Vec::new(),
                authorized_paths: Vec::new(),
                logical_handle_inputs: vec![FilesystemLogicalHandleInput {
                    operand_ordinal: 0,
                    kind: FilesystemLogicalHandleKind::Descriptor,
                    resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
                }],
                logical_handle_output: None,
                retired_logical_handles: Vec::new(),
                grant_refusals: Vec::new(),
            }
        }
        FilesystemOutputFileOperationReplayRecord::SetFilePermissions { mode } => {
            FilesystemOperationAttempt {
                operation_tag: 17,
                provider: FilesystemObservationProvider::RealScoped,
                outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                    result: FilesystemOperationResult::Scalar(0),
                    post_error: 0,
                }),
                scalar_operands: vec![FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::U32(mode),
                }],
                byte_operands: Vec::new(),
                path_like_operands: Vec::new(),
                rooted_path_operand_resolutions: Vec::new(),
                returned_paths: Vec::new(),
                observed_byte_regions: Vec::new(),
                metadata_observations: Vec::new(),
                mutable_byte_operand_resolutions: Vec::new(),
                mutable_i64_operand_resolutions: Vec::new(),
                mutable_byte_operands: Vec::new(),
                mutable_i64_operands: Vec::new(),
                authorized_paths: Vec::new(),
                logical_handle_inputs: vec![FilesystemLogicalHandleInput {
                    operand_ordinal: 0,
                    kind: FilesystemLogicalHandleKind::Descriptor,
                    resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
                }],
                logical_handle_output: None,
                retired_logical_handles: Vec::new(),
                grant_refusals: Vec::new(),
            }
        }
        FilesystemOutputFileOperationReplayRecord::SetFileTimes { times } => {
            let resolution_times = times.clone();
            let pre_times = times.clone();
            FilesystemOperationAttempt {
                operation_tag: 42,
                provider: FilesystemObservationProvider::RealScoped,
                outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                    result: FilesystemOperationResult::Scalar(0),
                    post_error: 0,
                }),
                scalar_operands: Vec::new(),
                byte_operands: Vec::new(),
                path_like_operands: Vec::new(),
                rooted_path_operand_resolutions: Vec::new(),
                returned_paths: Vec::new(),
                observed_byte_regions: Vec::new(),
                metadata_observations: Vec::new(),
                mutable_byte_operand_resolutions: vec![FilesystemMutableByteOperandResolution {
                    operand_ordinal: 1,
                    bytes: resolution_times,
                }],
                mutable_i64_operand_resolutions: Vec::new(),
                mutable_byte_operands: vec![FilesystemMutableByteOperand {
                    operand_ordinal: 1,
                    pre_bytes: pre_times,
                    post_bytes: times,
                }],
                mutable_i64_operands: Vec::new(),
                authorized_paths: Vec::new(),
                logical_handle_inputs: vec![FilesystemLogicalHandleInput {
                    operand_ordinal: 0,
                    kind: FilesystemLogicalHandleKind::Descriptor,
                    resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
                }],
                logical_handle_output: None,
                retired_logical_handles: Vec::new(),
                grant_refusals: Vec::new(),
            }
        }
        FilesystemOutputFileOperationReplayRecord::Sync
        | FilesystemOutputFileOperationReplayRecord::SyncData => FilesystemOperationAttempt {
            operation_tag: match operation {
                FilesystemOutputFileOperationReplayRecord::Sync => 43,
                FilesystemOutputFileOperationReplayRecord::SyncData => 44,
                FilesystemOutputFileOperationReplayRecord::Write(_)
                | FilesystemOutputFileOperationReplayRecord::Seek { .. }
                | FilesystemOutputFileOperationReplayRecord::SetLength { .. }
                | FilesystemOutputFileOperationReplayRecord::SetFilePermissions { .. }
                | FilesystemOutputFileOperationReplayRecord::SetFileTimes { .. }
                | FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(_)
                | FilesystemOutputFileOperationReplayRecord::LockAndUnlock(_) => {
                    unreachable!()
                }
            },
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(0),
                post_error: 0,
            }),
            scalar_operands: Vec::new(),
            byte_operands: Vec::new(),
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: Vec::new(),
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: Vec::new(),
            mutable_i64_operand_resolutions: Vec::new(),
            mutable_byte_operands: Vec::new(),
            mutable_i64_operands: Vec::new(),
            authorized_paths: Vec::new(),
            logical_handle_inputs: vec![FilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: FilesystemLogicalHandleKind::Descriptor,
                resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
            }],
            logical_handle_output: None,
            retired_logical_handles: Vec::new(),
            grant_refusals: Vec::new(),
        },
        FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(_) => {
            unreachable!("duplicate pairs are expanded by output_file_attempts")
        }
        FilesystemOutputFileOperationReplayRecord::LockAndUnlock(_) => {
            unreachable!("lock pairs are expanded by output_file_attempts")
        }
    }
}

#[cfg(test)]
mod filesystem_replay_record_tests {
    use super::*;

    fn root(value: u32) -> FilesystemGrantRootIdentity {
        FilesystemGrantRootIdentity::new(value).expect("test root is nonzero")
    }

    fn source_input(identity: u64) -> FilesystemSourceInputReplayRecord {
        let read = FilesystemReplayReadRecord::new(
            FilesystemReplayReadKind::Sequential,
            0,
            0,
            0,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("empty successful read is canonical");
        let chain = FilesystemSourceReadChainReplayRecord::new(
            root(1),
            b"inputs/table.txt".to_vec(),
            identity,
            0,
            vec![read],
            0,
        )
        .expect("source chain is canonical");
        FilesystemSourceInputReplayRecord::new(vec![
            FilesystemSourceInputReplayEventRecord::ReadChain(chain),
        ])
        .expect("source input is nonempty")
    }

    fn output_file(identity: u64) -> FilesystemOutputFileReplayRecord {
        let bytes = b"pub data Generated {}\n".to_vec();
        FilesystemOutputFileReplayRecord::new(
            root(2),
            b"table.generated.omg".to_vec(),
            identity,
            0,
            bytes.clone(),
            i64::try_from(bytes.len()).unwrap(),
            0,
            0,
        )
        .expect("full output write is canonical")
    }

    fn empty_output_file(identity: u64) -> FilesystemOutputFileReplayRecord {
        FilesystemOutputFileReplayRecord::empty(root(2), b"empty.bin".to_vec(), identity, 0, 0)
            .expect("freshly created and closed empty Output file is canonical")
    }

    #[test]
    fn typed_absent_output_removes_form_a_closed_failure_only_replay() {
        let first = FilesystemOutputAbsentRemoveReplayRecord::new(
            FilesystemOutputAbsentRemoveKind::File,
            root(2),
            b"missing-first.bin".to_vec(),
        )
        .unwrap();
        let second = FilesystemOutputAbsentRemoveReplayRecord::new(
            FilesystemOutputAbsentRemoveKind::Directory,
            root(2),
            b"nested/missing-second".to_vec(),
        )
        .unwrap();
        let record = FilesystemInputOutputAbsentRemovesReplayRecord::new(
            Some(source_input(7)),
            vec![first, second],
        )
        .unwrap();
        let replay = FilesystemReplay::from_input_output_absent_removes_record(record).unwrap();

        assert!(replay.has_output_attempts());
        assert!(replay.output_entries().is_empty());
        assert!((0..3).all(|index| !replay.executes_replay_attempt(index)));
        assert!((3..5).all(|index| replay.executes_replay_attempt(index)));
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![2, 4, 8, 9, 12]
        );
        for attempt in &replay.attempts()[3..] {
            assert_eq!(
                attempt.result(),
                Some(FilesystemOperationResult::Scalar(-1))
            );
            assert_eq!(attempt.post_error(), Some(2));
            assert!(output_absent_remove_attempt_is_exact(attempt));
        }
    }

    fn multiwrite_output_file(identity: u64) -> FilesystemOutputFileReplayRecord {
        FilesystemOutputFileReplayRecord::with_writes(
            root(2),
            b"multi.generated.omg".to_vec(),
            identity,
            0,
            vec![
                FilesystemOutputWriteReplayRecord::new(b"first".to_vec(), 5, 0).unwrap(),
                FilesystemOutputWriteReplayRecord::new(Vec::new(), 0, 0).unwrap(),
                FilesystemOutputWriteReplayRecord::new(b"second".to_vec(), 6, 0).unwrap(),
            ],
            0,
        )
        .expect("multiple complete sequential writes are canonical")
    }

    fn positioned_output_file(identity: u64) -> FilesystemOutputFileReplayRecord {
        FilesystemOutputFileReplayRecord::with_writes(
            root(2),
            b"positioned.generated.omg".to_vec(),
            identity,
            0,
            vec![
                FilesystemOutputWriteReplayRecord::new(b"head".to_vec(), 4, 0).unwrap(),
                FilesystemOutputWriteReplayRecord::positioned(8, b"tail".to_vec(), 4, 0).unwrap(),
                FilesystemOutputWriteReplayRecord::new(b"-cur".to_vec(), 4, 0).unwrap(),
                FilesystemOutputWriteReplayRecord::positioned(0, Vec::new(), 0, 0).unwrap(),
            ],
            0,
        )
        .expect("mixed complete sequential and positioned writes are canonical")
    }

    fn descriptor_metadata_input(identity: u64) -> FilesystemSourceInputReplayRecord {
        let carrier = vec![0; FILESYSTEM_METADATA_API_CARRIER_BYTES];
        let metadata = FilesystemMetadataObservation::new(
            1,
            FilesystemMetadataObservationKind::OpenDescriptor,
            0o100444,
            23,
            1_000_000_000,
        );
        let event = FilesystemSourceDescriptorMetadataReplayRecord::new(
            root(1),
            b"main.omg".to_vec(),
            identity,
            0,
            0,
            carrier.clone(),
            carrier.clone(),
            carrier,
            metadata,
            0,
        )
        .expect("descriptor metadata event is canonical");
        FilesystemSourceInputReplayRecord::new(vec![
            FilesystemSourceInputReplayEventRecord::DescriptorMetadata(event),
        ])
        .expect("descriptor metadata is a source event")
    }

    fn typed_replay() -> FilesystemReplay {
        let output = output_file(2);
        let included = BuildIncludedSource::from_coordinate(
            output.output_root(),
            output.output_relative_path().to_vec(),
            6,
        )
        .expect("handoff path is canonical");
        let record =
            FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], vec![included])
                .expect("Source and Output coordinates are distinct");
        FilesystemReplay::from_input_output_record(record).expect("typed replay fits policy")
    }

    #[test]
    fn typed_input_output_record_emits_one_exact_chain_and_handoff() {
        let source_only = FilesystemReplay::from_source_input_record(source_input(1)).unwrap();
        assert!(source_only.output_files().is_empty());
        assert!(source_only.expected_included_sources().is_empty());

        let replay = typed_replay();
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![2, 4, 8, 1, 5, 8]
        );
        let outputs = replay.output_files();
        let [output] = outputs.as_slice() else {
            panic!("one Output file is typed")
        };
        assert_eq!(output.output_root(), root(2));
        assert_eq!(output.output_relative_path(), b"table.generated.omg");
        assert_eq!(output.logical_handle_identity().get(), 2);
        assert_eq!(output.create_mode(), 438);
        let [FilesystemOutputFileOperationReplayRecord::Write(write)] = output.operations() else {
            panic!("singleton Output file retains one write")
        };
        assert_eq!(write.result(), write.bytes().len() as i64);
        let [included] = replay.expected_included_sources() else {
            panic!("one handoff coordinate is retained")
        };
        assert_eq!(included.root(), output.output_root());
        assert_eq!(included.relative_path(), output.output_relative_path());
    }

    #[test]
    fn typed_input_output_record_retains_an_ordinary_file_without_handoff() {
        let record = FilesystemInputOutputReplayRecord::new(
            source_input(1),
            vec![output_file(2)],
            Vec::new(),
        )
        .expect("ordinary output needs no generated-source handoff");
        let replay = FilesystemReplay::from_input_output_record(record)
            .expect("ordinary output replay fits policy");
        assert_eq!(replay.output_files().len(), 1);
        assert!(replay.expected_included_sources().is_empty());

        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        );
        let decoded = FilesystemReplay::from_input_output_observations(&observations)
            .expect("observed ordinary output is accepted");
        assert!(decoded.expected_included_sources().is_empty());
    }

    #[test]
    fn typed_input_output_record_retains_multiple_ordinary_files_without_handoff() {
        let mut second = output_file(3);
        second.output_relative_path = b"metadata.bin".to_vec();
        let record = FilesystemInputOutputReplayRecord::new(
            source_input(1),
            vec![output_file(2), second],
            Vec::new(),
        )
        .expect("distinct ordinary outputs need no generated-source handoff");
        let replay = FilesystemReplay::from_input_output_record(record)
            .expect("multiple ordinary output replay fits policy");
        assert_eq!(replay.output_files().len(), 2);
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![2, 4, 8, 1, 5, 8, 1, 5, 8]
        );

        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        );
        let decoded = FilesystemReplay::from_input_output_observations(&observations)
            .expect("observed ordinary outputs are accepted");
        assert_eq!(decoded.output_files().len(), 2);
        assert!(decoded.expected_included_sources().is_empty());
    }

    #[test]
    fn typed_output_file_retains_one_or_more_full_sequential_writes() {
        let output = multiwrite_output_file(2);
        let handoff = BuildIncludedSource::from_coordinate(
            output.output_root(),
            output.output_relative_path().to_vec(),
            8,
        )
        .unwrap();
        let record = FilesystemInputOutputReplayRecord::new(
            source_input(1),
            vec![output],
            vec![handoff.clone()],
        )
        .expect("handoff follows the variable-length output close");
        let replay = FilesystemReplay::from_input_output_record(record).unwrap();
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![2, 4, 8, 1, 5, 5, 5, 8]
        );
        let decoded_chains = replay.output_files();
        let [decoded] = decoded_chains.as_slice() else {
            panic!("one Output file is retained")
        };
        assert_eq!(
            decoded
                .operations()
                .iter()
                .filter_map(|operation| match operation {
                    FilesystemOutputFileOperationReplayRecord::Write(write) => Some(write),
                    _ => None,
                })
                .flat_map(FilesystemOutputWriteReplayRecord::bytes)
                .copied()
                .collect::<Vec<_>>(),
            b"firstsecond"
        );
        assert_eq!(replay.expected_included_sources(), &[handoff.clone()]);

        assert!(FilesystemOutputWriteReplayRecord::new(vec![1, 2], 1, 0).is_err());
        let mut no_writes = replay.attempts().to_vec();
        no_writes.drain(4..7);
        let observations =
            EvaluationObservations::from_filesystem_operation_attempts(no_writes, Vec::new());
        let empty = FilesystemReplay::from_input_output_observations(&observations)
            .expect("create-close is an exact empty ordinary Output file");
        assert!(empty.output_files()[0].replayed_bytes().unwrap().is_empty());

        let mut partial_write = replay.attempts().to_vec();
        partial_write[4].outcome = Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(4),
            post_error: 0,
        });
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            partial_write,
            vec![handoff.clone()],
        );
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

        let mut wrong_descriptor = replay.attempts().to_vec();
        wrong_descriptor[6].logical_handle_inputs[0].resolution =
            FilesystemLogicalHandleInputResolution::Resolved(
                FilesystemLogicalHandleIdentity::new(99).unwrap(),
            );
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            wrong_descriptor,
            vec![handoff],
        );
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
    }

    #[test]
    fn typed_output_file_retains_full_positioned_writes_and_cursor_semantics() {
        let output = positioned_output_file(2);
        assert_eq!(output.replayed_bytes().unwrap(), b"head-curtail");
        let record =
            FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new())
                .unwrap();
        let replay = FilesystemReplay::from_input_output_record(record).unwrap();
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![2, 4, 8, 1, 5, 7, 5, 7, 8]
        );
        let decoded = FilesystemReplay::from_input_output_observations(
            &EvaluationObservations::from_filesystem_operation_attempts(
                replay.attempts().to_vec(),
                Vec::new(),
            ),
        )
        .expect("positioned output observations retain exact operation shape");
        let output_files = decoded.output_files();
        let [decoded] = output_files.as_slice() else {
            panic!("one positioned Output file is retained")
        };
        assert_eq!(decoded.replayed_bytes().unwrap(), b"head-curtail");
        let FilesystemOutputFileOperationReplayRecord::Write(positioned) = &decoded.operations()[1]
        else {
            panic!("second Output operation is a positioned write")
        };
        assert_eq!(
            positioned.kind(),
            FilesystemOutputWriteReplayKind::Positioned { offset: 8 }
        );

        assert!(FilesystemOutputWriteReplayRecord::positioned(-1, vec![1], 1, 0).is_err());
        let zero_length_beyond_extent = FilesystemOutputFileReplayRecord::with_writes(
            root(2),
            b"empty.bin".to_vec(),
            3,
            0,
            vec![FilesystemOutputWriteReplayRecord::positioned(1, Vec::new(), 0, 0).unwrap()],
            0,
        )
        .expect("zero-length positioned writes do not extend Output");
        assert!(
            zero_length_beyond_extent
                .replayed_bytes()
                .unwrap()
                .is_empty()
        );

        let sparse_over_ceiling = FilesystemOutputFileReplayRecord::with_writes(
            root(2),
            b"sparse.bin".to_vec(),
            3,
            0,
            vec![
                FilesystemOutputWriteReplayRecord::positioned(
                    i64::try_from(MAX_FILESYSTEM_REPLAY_RETAINED_BYTES).unwrap(),
                    vec![1],
                    1,
                    0,
                )
                .unwrap(),
            ],
            0,
        )
        .expect("typed positioned chain is valid before replay allocation policy");
        let sparse_record = FilesystemInputOutputReplayRecord::new(
            source_input(1),
            vec![sparse_over_ceiling],
            Vec::new(),
        )
        .unwrap();
        assert!(FilesystemReplay::from_input_output_record(sparse_record).is_err());

        let mut wrong_offset_lane = replay.attempts().to_vec();
        wrong_offset_lane[5].scalar_operands[0].operand_ordinal = 1;
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            wrong_offset_lane,
            Vec::new(),
        );
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
    }

    #[test]
    fn typed_output_file_retains_create_close_without_synthetic_write() {
        let output = empty_output_file(2);
        assert!(output.operations().is_empty());
        assert!(output.replayed_bytes().unwrap().is_empty());
        let record =
            FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new())
                .unwrap();
        let replay = FilesystemReplay::from_input_output_record(record).unwrap();
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![2, 4, 8, 1, 8]
        );
        let decoded = FilesystemReplay::from_input_output_observations(
            &EvaluationObservations::from_filesystem_operation_attempts(
                replay.attempts().to_vec(),
                Vec::new(),
            ),
        )
        .expect("create-close Output file observations are exact");
        let output_files = decoded.output_files();
        let [decoded] = output_files.as_slice() else {
            panic!("one empty Output file is retained")
        };
        assert!(decoded.operations().is_empty());
        assert!(decoded.replayed_bytes().unwrap().is_empty());

        let mut incomplete = replay.attempts().to_vec();
        incomplete.pop();
        let observations =
            EvaluationObservations::from_filesystem_operation_attempts(incomplete, Vec::new());
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
    }

    #[test]
    fn typed_output_file_retains_sync_operations_in_authored_order() {
        let output = FilesystemOutputFileReplayRecord::with_operations(
            root(2),
            b"synced.bin".to_vec(),
            2,
            0,
            vec![
                FilesystemOutputFileOperationReplayRecord::Sync,
                FilesystemOutputFileOperationReplayRecord::Write(
                    FilesystemOutputWriteReplayRecord::new(b"first".to_vec(), 5, 0).unwrap(),
                ),
                FilesystemOutputFileOperationReplayRecord::SyncData,
                FilesystemOutputFileOperationReplayRecord::Write(
                    FilesystemOutputWriteReplayRecord::new(b"second".to_vec(), 6, 0).unwrap(),
                ),
                FilesystemOutputFileOperationReplayRecord::Sync,
            ],
            0,
        )
        .unwrap();
        assert_eq!(output.replayed_bytes().unwrap(), b"firstsecond");
        let record =
            FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new())
                .unwrap();
        let replay = FilesystemReplay::from_input_output_record(record).unwrap();
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![2, 4, 8, 1, 43, 5, 44, 5, 43, 8]
        );
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        );
        let decoded = FilesystemReplay::from_input_output_observations(&observations)
            .expect("successful sync operations are exact Output operations");
        assert_eq!(decoded.output_files()[0].operations().len(), 5);
        assert_eq!(
            decoded.output_files()[0].replayed_bytes().unwrap(),
            b"firstsecond"
        );

        let mut malformed = replay.attempts().to_vec();
        malformed[4].scalar_operands.push(FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I32(0),
        });
        let observations =
            EvaluationObservations::from_filesystem_operation_attempts(malformed, Vec::new());
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
    }

    #[test]
    fn typed_output_file_replays_exact_duplicate_and_immediate_retirement() {
        let output = FilesystemOutputFileReplayRecord::with_operations(
            root(2),
            b"duplicated.bin".to_vec(),
            2,
            0,
            vec![
                FilesystemOutputFileOperationReplayRecord::Write(
                    FilesystemOutputWriteReplayRecord::new(b"before".to_vec(), 6, 0).unwrap(),
                ),
                FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(
                    FilesystemOutputDuplicateReplayRecord::new(3).unwrap(),
                ),
                FilesystemOutputFileOperationReplayRecord::Write(
                    FilesystemOutputWriteReplayRecord::new(b"after".to_vec(), 5, 0).unwrap(),
                ),
            ],
            0,
        )
        .unwrap();
        assert_eq!(output.replayed_bytes().unwrap(), b"beforeafter");
        let record =
            FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new())
                .unwrap();
        let replay = FilesystemReplay::from_input_output_record(record).unwrap();
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![2, 4, 8, 1, 5, 45, 8, 5, 8]
        );
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        );
        let decoded = FilesystemReplay::from_input_output_observations(&observations)
            .expect("successful duplicate and immediate close are exact Output operations");
        assert!(matches!(
            decoded.output_files()[0].operations()[1],
            FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(duplicate)
                if duplicate.logical_handle_identity().get() == 3
        ));

        let mut wrong_lineage = replay.attempts().to_vec();
        wrong_lineage[5]
            .logical_handle_output
            .as_mut()
            .unwrap()
            .source = FilesystemLogicalHandleOutputSource::Created;
        let observations =
            EvaluationObservations::from_filesystem_operation_attempts(wrong_lineage, Vec::new());
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

        let mut failed_duplicate = replay.attempts().to_vec();
        failed_duplicate[5].outcome = Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(-1),
            post_error: 9,
        });
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            failed_duplicate,
            Vec::new(),
        );
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

        let mut wrong_close = replay.attempts().to_vec();
        wrong_close[6].logical_handle_inputs[0].resolution =
            FilesystemLogicalHandleInputResolution::Resolved(
                FilesystemLogicalHandleIdentity::new(2).unwrap(),
            );
        let observations =
            EvaluationObservations::from_filesystem_operation_attempts(wrong_close, Vec::new());
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

        let source_colliding_output = FilesystemOutputFileReplayRecord::with_operations(
            root(2),
            b"source-collision.bin".to_vec(),
            2,
            0,
            vec![
                FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(
                    FilesystemOutputDuplicateReplayRecord::new(1).unwrap(),
                ),
            ],
            0,
        )
        .unwrap();
        assert!(
            FilesystemInputOutputReplayRecord::new(
                source_input(1),
                vec![source_colliding_output],
                Vec::new(),
            )
            .is_err()
        );

        let over_quota = (0..=MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES)
            .map(|index| {
                FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(
                    FilesystemOutputDuplicateReplayRecord::new(u64::try_from(index).unwrap() + 3)
                        .unwrap(),
                )
            })
            .collect();
        assert!(
            FilesystemOutputFileReplayRecord::with_operations(
                root(2),
                b"too-many-duplicates.bin".to_vec(),
                2,
                0,
                over_quota,
                0,
            )
            .is_err()
        );
    }

    #[test]
    fn typed_output_file_retains_exact_descriptor_permission_changes() {
        let output = FilesystemOutputFileReplayRecord::with_operations(
            root(2),
            b"tool.bin".to_vec(),
            2,
            0,
            vec![
                FilesystemOutputFileOperationReplayRecord::Write(
                    FilesystemOutputWriteReplayRecord::new(b"tool".to_vec(), 4, 0).unwrap(),
                ),
                FilesystemOutputFileOperationReplayRecord::SetFilePermissions { mode: 0o755 },
            ],
            0,
        )
        .unwrap();
        assert_eq!(output.replayed_file_permissions(), Some(0o755));
        assert!(output.replayed_executable());
        assert_eq!(output.replayed_bytes().unwrap(), b"tool");

        let record =
            FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new())
                .unwrap();
        let replay = FilesystemReplay::from_input_output_record(record).unwrap();
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![2, 4, 8, 1, 5, 17, 8]
        );
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        );
        let decoded = FilesystemReplay::from_input_output_observations(&observations)
            .expect("successful descriptor permission changes are exact Output operations");
        assert_eq!(
            decoded.output_files()[0].replayed_file_permissions(),
            Some(0o755)
        );

        let mut failed = replay.attempts().to_vec();
        failed[5].outcome = Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(-1),
            post_error: 1,
        });
        let observations =
            EvaluationObservations::from_filesystem_operation_attempts(failed, Vec::new());
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

        let mut wrong_descriptor = replay.attempts().to_vec();
        wrong_descriptor[5].logical_handle_inputs[0].resolution =
            FilesystemLogicalHandleInputResolution::Unknown;
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            wrong_descriptor,
            Vec::new(),
        );
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
    }

    #[test]
    fn typed_output_file_retains_exact_descriptor_time_carrier() {
        let mut times = vec![0; 32];
        times[0..8].copy_from_slice(&11i64.to_le_bytes());
        times[16..24].copy_from_slice(&29i64.to_le_bytes());
        let output = FilesystemOutputFileReplayRecord::with_operations(
            root(2),
            b"dated.bin".to_vec(),
            2,
            0,
            vec![
                FilesystemOutputFileOperationReplayRecord::Write(
                    FilesystemOutputWriteReplayRecord::new(b"dated".to_vec(), 5, 0).unwrap(),
                ),
                FilesystemOutputFileOperationReplayRecord::SetFileTimes {
                    times: times.clone(),
                },
            ],
            0,
        )
        .unwrap();
        assert_eq!(output.replayed_bytes().unwrap(), b"dated");

        let record =
            FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new())
                .unwrap();
        let replay = FilesystemReplay::from_input_output_record(record).unwrap();
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![2, 4, 8, 1, 5, 42, 8]
        );
        let time_attempt = &replay.attempts()[5];
        assert_eq!(
            time_attempt.mutable_byte_operand_resolutions[0].bytes,
            times
        );
        assert_eq!(time_attempt.mutable_byte_operands[0].pre_bytes, times);
        assert_eq!(time_attempt.mutable_byte_operands[0].post_bytes, times);

        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        );
        let decoded = FilesystemReplay::from_input_output_observations(&observations)
            .expect("successful descriptor time changes are exact Output operations");
        assert!(matches!(
            &decoded.output_files()[0].operations()[1],
            FilesystemOutputFileOperationReplayRecord::SetFileTimes { times: retained }
                if retained == &times
        ));

        let mut changed_post_state = replay.attempts().to_vec();
        changed_post_state[5].mutable_byte_operands[0].post_bytes[0] ^= 1;
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            changed_post_state,
            Vec::new(),
        );
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

        assert!(
            FilesystemOutputFileReplayRecord::with_operations(
                root(2),
                b"short-times.bin".to_vec(),
                2,
                0,
                vec![FilesystemOutputFileOperationReplayRecord::SetFileTimes {
                    times: vec![0; 31],
                }],
                0,
            )
            .is_err()
        );

        let over_retention = FilesystemOutputFileReplayRecord::with_operations(
            root(2),
            b"large-times.bin".to_vec(),
            2,
            0,
            vec![FilesystemOutputFileOperationReplayRecord::SetFileTimes {
                times: vec![0; MAX_FILESYSTEM_REPLAY_RETAINED_BYTES / 3 + 1],
            }],
            0,
        )
        .unwrap();
        let record = FilesystemInputOutputReplayRecord::new(
            source_input(1),
            vec![over_retention],
            Vec::new(),
        )
        .unwrap();
        assert!(FilesystemReplay::from_input_output_record(record).is_err());
    }

    #[test]
    fn typed_output_file_replays_set_length_without_moving_cursor() {
        let output = FilesystemOutputFileReplayRecord::with_operations(
            root(2),
            b"resized.bin".to_vec(),
            2,
            0,
            vec![
                FilesystemOutputFileOperationReplayRecord::Write(
                    FilesystemOutputWriteReplayRecord::new(b"abcdef".to_vec(), 6, 0).unwrap(),
                ),
                FilesystemOutputFileOperationReplayRecord::SetLength { length: 3 },
                FilesystemOutputFileOperationReplayRecord::Write(
                    FilesystemOutputWriteReplayRecord::new(b"XY".to_vec(), 2, 0).unwrap(),
                ),
                FilesystemOutputFileOperationReplayRecord::SetLength { length: 5 },
            ],
            0,
        )
        .unwrap();
        assert_eq!(output.replayed_bytes().unwrap(), b"abc\0\0");
        let record =
            FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new())
                .unwrap();
        let replay = FilesystemReplay::from_input_output_record(record).unwrap();
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![2, 4, 8, 1, 5, 41, 5, 41, 8]
        );
        let decoded = FilesystemReplay::from_input_output_observations(
            &EvaluationObservations::from_filesystem_operation_attempts(
                replay.attempts().to_vec(),
                Vec::new(),
            ),
        )
        .expect("successful set_len operations are exact Output operations");
        assert_eq!(
            decoded.output_files()[0].replayed_bytes().unwrap(),
            b"abc\0\0"
        );

        assert!(
            FilesystemOutputFileReplayRecord::with_operations(
                root(2),
                b"negative.bin".to_vec(),
                3,
                0,
                vec![FilesystemOutputFileOperationReplayRecord::SetLength { length: -1 }],
                0,
            )
            .is_err()
        );
        let mut malformed = replay.attempts().to_vec();
        malformed[5].scalar_operands[0].operand_ordinal = 0;
        let observations =
            EvaluationObservations::from_filesystem_operation_attempts(malformed, Vec::new());
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
    }

    #[test]
    fn typed_output_file_replays_exact_seek_cursor_transitions() {
        let output = FilesystemOutputFileReplayRecord::with_operations(
            root(2),
            b"seeked.bin".to_vec(),
            2,
            0,
            vec![
                FilesystemOutputFileOperationReplayRecord::Write(
                    FilesystemOutputWriteReplayRecord::new(b"abcdef".to_vec(), 6, 0).unwrap(),
                ),
                FilesystemOutputFileOperationReplayRecord::Seek {
                    offset: 2,
                    whence: 0,
                    result: 2,
                },
                FilesystemOutputFileOperationReplayRecord::Write(
                    FilesystemOutputWriteReplayRecord::new(b"XY".to_vec(), 2, 0).unwrap(),
                ),
                FilesystemOutputFileOperationReplayRecord::Seek {
                    offset: -1,
                    whence: 2,
                    result: 5,
                },
                FilesystemOutputFileOperationReplayRecord::Write(
                    FilesystemOutputWriteReplayRecord::new(b"Z".to_vec(), 1, 0).unwrap(),
                ),
                FilesystemOutputFileOperationReplayRecord::Seek {
                    offset: -3,
                    whence: 1,
                    result: 3,
                },
                FilesystemOutputFileOperationReplayRecord::Write(
                    FilesystemOutputWriteReplayRecord::new(b"Q".to_vec(), 1, 0).unwrap(),
                ),
            ],
            0,
        )
        .unwrap();
        assert_eq!(output.replayed_bytes().unwrap(), b"abXQeZ");
        let record =
            FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new())
                .unwrap();
        let replay = FilesystemReplay::from_input_output_record(record).unwrap();
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![2, 4, 8, 1, 5, 10, 5, 10, 5, 10, 5, 8]
        );
        let decoded = FilesystemReplay::from_input_output_observations(
            &EvaluationObservations::from_filesystem_operation_attempts(
                replay.attempts().to_vec(),
                Vec::new(),
            ),
        )
        .expect("successful canonical seeks are exact Output operations");
        assert_eq!(
            decoded.output_files()[0].replayed_bytes().unwrap(),
            b"abXQeZ"
        );

        assert!(
            FilesystemOutputFileReplayRecord::with_operations(
                root(2),
                b"bad-seek.bin".to_vec(),
                3,
                0,
                vec![FilesystemOutputFileOperationReplayRecord::Seek {
                    offset: 1,
                    whence: 0,
                    result: 2,
                }],
                0,
            )
            .is_err()
        );
        let mut malformed = replay.attempts().to_vec();
        malformed[5].scalar_operands[1].value = FilesystemScalarOperandValue::I32(9);
        let observations =
            EvaluationObservations::from_filesystem_operation_attempts(malformed, Vec::new());
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
    }

    #[test]
    fn typed_input_output_record_retains_ordered_multiple_source_handoffs() {
        let first = output_file(2);
        let mut second = output_file(3);
        second.output_relative_path = b"other.generated.omg".to_vec();
        let handoffs = vec![
            BuildIncludedSource::from_coordinate(
                second.output_root(),
                second.output_relative_path().to_vec(),
                9,
            )
            .unwrap(),
            BuildIncludedSource::from_coordinate(
                first.output_root(),
                first.output_relative_path().to_vec(),
                9,
            )
            .unwrap(),
        ];
        let record = FilesystemInputOutputReplayRecord::new(
            source_input(1),
            vec![first, second],
            handoffs.clone(),
        )
        .expect("handoff order may differ from output-chain order after both closes");
        let replay = FilesystemReplay::from_input_output_record(record).unwrap();
        assert_eq!(replay.expected_included_sources(), handoffs);

        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            handoffs.clone(),
        );
        let decoded = FilesystemReplay::from_input_output_observations(&observations).unwrap();
        assert_eq!(decoded.expected_included_sources(), handoffs);
    }

    #[test]
    fn typed_descriptor_metadata_record_emits_one_closed_exact_event() {
        let replay =
            FilesystemReplay::from_source_input_record(descriptor_metadata_input(7)).unwrap();
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![2, 39, 8]
        );
        let metadata = &replay.attempts()[1];
        assert_eq!(
            metadata.logical_handle_inputs[0].resolution,
            FilesystemLogicalHandleInputResolution::Resolved(
                FilesystemLogicalHandleIdentity::new(7).unwrap()
            )
        );
        assert_eq!(
            metadata.metadata_observations[0].kind(),
            FilesystemMetadataObservationKind::OpenDescriptor
        );

        let mut events = source_input(7).events;
        events.extend(descriptor_metadata_input(7).events);
        assert!(FilesystemSourceInputReplayRecord::new(events).is_err());
    }

    #[test]
    fn typed_output_rejects_partial_noncanonical_and_overlapping_records() {
        assert!(
            FilesystemInputOutputReplayRecord::new(source_input(1), Vec::new(), Vec::new())
                .is_err()
        );
        assert!(
            FilesystemOutputFileReplayRecord::new(
                root(2),
                b"../escape.omg".to_vec(),
                2,
                0,
                vec![1],
                1,
                0,
                0,
            )
            .is_err()
        );
        assert!(
            FilesystemOutputFileReplayRecord::new(
                root(2),
                b"generated.omg".to_vec(),
                2,
                0,
                vec![1, 2],
                1,
                0,
                0,
            )
            .is_err()
        );
        let output = output_file(1);
        let included = BuildIncludedSource::from_coordinate(
            output.output_root(),
            output.output_relative_path().to_vec(),
            6,
        )
        .unwrap();
        assert!(
            FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], vec![included])
                .is_err()
        );

        let output = output_file(2);
        let duplicate_handoff = BuildIncludedSource::from_coordinate(
            output.output_root(),
            output.output_relative_path().to_vec(),
            6,
        )
        .unwrap();
        assert!(
            FilesystemInputOutputReplayRecord::new(
                source_input(1),
                vec![output],
                vec![duplicate_handoff.clone(), duplicate_handoff],
            )
            .is_err()
        );

        let first = output_file(2);
        let duplicate_path = output_file(3);
        assert!(
            FilesystemInputOutputReplayRecord::new(
                source_input(1),
                vec![first, duplicate_path],
                Vec::new(),
            )
            .is_err()
        );

        let first = output_file(2);
        let mut duplicate_descriptor = output_file(2);
        duplicate_descriptor.output_relative_path = b"other.bin".to_vec();
        assert!(
            FilesystemInputOutputReplayRecord::new(
                source_input(1),
                vec![first, duplicate_descriptor],
                Vec::new(),
            )
            .is_err()
        );

        let output = output_file(2);
        let wrong_handoff =
            BuildIncludedSource::from_coordinate(root(2), b"other.omg".to_vec(), 6).unwrap();
        assert!(
            FilesystemInputOutputReplayRecord::new(
                source_input(1),
                vec![output],
                vec![wrong_handoff],
            )
            .is_err()
        );

        let output = output_file(2);
        let early_handoff = BuildIncludedSource::from_coordinate(
            output.output_root(),
            output.output_relative_path().to_vec(),
            5,
        )
        .unwrap();
        assert!(
            FilesystemInputOutputReplayRecord::new(
                source_input(1),
                vec![output],
                vec![early_handoff],
            )
            .is_err()
        );

        let first = output_file(2);
        let mut second = output_file(3);
        second.output_relative_path = b"other.generated.omg".to_vec();
        let early_second = BuildIncludedSource::from_coordinate(
            second.output_root(),
            second.output_relative_path().to_vec(),
            6,
        )
        .unwrap();
        assert!(
            FilesystemInputOutputReplayRecord::new(
                source_input(1),
                vec![first, second],
                vec![early_second],
            )
            .is_err()
        );
    }

    #[test]
    fn observed_input_output_replay_rejects_nonexact_output_lanes() {
        let replay = typed_replay();
        let included = replay.expected_included_sources()[0].clone();
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            vec![included.clone()],
        );
        let decoded = FilesystemReplay::from_input_output_observations(&observations)
            .expect("exact observed chain is accepted");
        assert_eq!(decoded.expected_included_sources(), &[included]);

        let mut attempts = replay.attempts().to_vec();
        let output_start = attempts.len() - 3;
        attempts[output_start].scalar_operands[0].value = FilesystemScalarOperandValue::I32(511);
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            attempts,
            replay.expected_included_sources().to_vec(),
        );
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

        let early_handoff = BuildIncludedSource::from_coordinate(
            root(2),
            b"table.generated.omg".to_vec(),
            output_start + 2,
        )
        .unwrap();
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            vec![early_handoff],
        );
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

        let mut attempts = replay.attempts().to_vec();
        attempts[output_start + 1].grant_refusals = vec![FilesystemGrantRefusal {
            operand_ordinal: 0,
            access: FilesystemGrantAccess::Write,
            reason: FilesystemGrantRefusalReason::OutsideGrantedRoots,
        }];
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            attempts,
            replay.expected_included_sources().to_vec(),
        );
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
    }

    #[test]
    fn replay_retention_has_a_lower_aggregate_clone_ceiling() {
        let bytes = vec![0; MAX_FILESYSTEM_REPLAY_RETAINED_BYTES + 1];
        let output = FilesystemOutputFileReplayRecord::new(
            root(2),
            b"large.generated.omg".to_vec(),
            2,
            0,
            bytes,
            i64::try_from(MAX_FILESYSTEM_REPLAY_RETAINED_BYTES + 1).unwrap(),
            0,
            0,
        )
        .unwrap();
        let included = BuildIncludedSource::from_coordinate(
            output.output_root(),
            output.output_relative_path().to_vec(),
            6,
        )
        .unwrap();
        let record =
            FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], vec![included])
                .expect("large typed record is valid before replay-retention policy");
        assert!(FilesystemReplay::from_input_output_record(record).is_err());
    }

    #[test]
    fn replay_retention_weight_accepts_the_exact_limit_and_rejects_one_more_unit() {
        let payload_length = MAX_FILESYSTEM_REPLAY_RETENTION_WEIGHT
            - FILESYSTEM_REPLAY_ATTEMPT_RETENTION_WEIGHT
            - FILESYSTEM_REPLAY_BYTE_OPERAND_RETENTION_WEIGHT;
        let attempt = FilesystemOperationAttempt {
            operation_tag: 0,
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(0),
                post_error: 0,
            }),
            scalar_operands: Vec::new(),
            byte_operands: vec![FilesystemByteOperand {
                operand_ordinal: 0,
                bytes: vec![0; payload_length],
            }],
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: Vec::new(),
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: Vec::new(),
            mutable_i64_operand_resolutions: Vec::new(),
            mutable_byte_operands: Vec::new(),
            mutable_i64_operands: Vec::new(),
            authorized_paths: Vec::new(),
            logical_handle_inputs: Vec::new(),
            logical_handle_output: None,
            retired_logical_handles: Vec::new(),
            grant_refusals: Vec::new(),
        };
        let mut attempts = vec![attempt];
        assert!(validate_filesystem_replay_size(&attempts).is_ok());
        attempts[0].byte_operands[0].bytes.push(0);
        assert!(validate_filesystem_replay_size(&attempts).is_err());
    }
}

fn source_path_metadata_attempt_is_exact(attempt: &FilesystemOperationAttempt) -> bool {
    let expected_kind = match attempt.operation_tag {
        38 => FilesystemMetadataObservationKind::FollowedPath,
        40 => FilesystemMetadataObservationKind::UnfollowedFinalPath,
        _ => return false,
    };
    let [rooted] = attempt.rooted_path_operand_resolutions.as_slice() else {
        return false;
    };
    let [authorized] = attempt.authorized_paths.as_slice() else {
        return false;
    };
    let [metadata] = attempt.metadata_observations.as_slice() else {
        return false;
    };
    let [mutable_resolution] = attempt.mutable_byte_operand_resolutions.as_slice() else {
        return false;
    };
    let [mutable] = attempt.mutable_byte_operands.as_slice() else {
        return false;
    };
    attempt.provider == FilesystemObservationProvider::RealScoped
        && attempt.result() == Some(FilesystemOperationResult::Scalar(0))
        && attempt.scalar_operands.is_empty()
        && attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && rooted.operand_ordinal == 0
        && filesystem_root_relative_path_is_canonical(&rooted.relative_path, false)
        && attempt.returned_paths.is_empty()
        && attempt.observed_byte_regions.is_empty()
        && metadata.output_operand_ordinal == 1
        && metadata.kind == expected_kind
        && mutable_resolution.operand_ordinal == 1
        && mutable.operand_ordinal == 1
        && mutable_resolution.bytes == mutable.pre_bytes
        && mutable.pre_bytes.len() == mutable.post_bytes.len()
        && mutable.post_bytes.len() >= FILESYSTEM_METADATA_API_CARRIER_BYTES
        && authorized.operand_ordinal == 0
        && authorized.access == FilesystemGrantAccess::Read
        && authorized.root == rooted.root
        && filesystem_root_relative_path_is_canonical(&authorized.relative_path, true)
        && attempt.mutable_i64_operand_resolutions.is_empty()
        && attempt.mutable_i64_operands.is_empty()
        && attempt.logical_handle_inputs.is_empty()
        && attempt.logical_handle_output.is_none()
        && attempt.retired_logical_handles.is_empty()
        && attempt.grant_refusals.is_empty()
}

fn source_path_metadata_attempt(
    record: FilesystemSourcePathMetadataReplayRecord,
) -> FilesystemOperationAttempt {
    let operation_tag = match record.kind {
        FilesystemMetadataObservationKind::FollowedPath => 38,
        FilesystemMetadataObservationKind::UnfollowedFinalPath => 40,
        FilesystemMetadataObservationKind::OpenDescriptor => {
            unreachable!("validated source path metadata cannot target a descriptor")
        }
    };
    FilesystemOperationAttempt {
        operation_tag,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: record.post_error,
        }),
        scalar_operands: Vec::new(),
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: vec![FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: record.source_root,
            relative_path: record.source_relative_path,
        }],
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: vec![record.metadata],
        mutable_byte_operand_resolutions: vec![FilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: record.mutable_resolution,
        }],
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: vec![FilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: record.mutable_pre_state,
            post_bytes: record.mutable_post_state,
        }],
        mutable_i64_operands: Vec::new(),
        authorized_paths: vec![FilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: FilesystemGrantAccess::Read,
            root: record.authorized_root,
            relative_path: record.authorized_relative_path,
        }],
        logical_handle_inputs: Vec::new(),
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

fn source_read_chain_attempts(
    record: FilesystemSourceReadChainReplayRecord,
) -> Vec<FilesystemOperationAttempt> {
    let identity = record.logical_handle_identity;
    let open = source_descriptor_open_attempt(
        record.source_root,
        record.source_relative_path,
        identity,
        record.open_post_error,
    );
    let read_count = record.reads.len();
    let reads = record.reads.into_iter().map(|read| {
        let read_length =
            usize::try_from(read.read_result).expect("validated replay read result fits usize");
        let (operation_tag, scalar_operands, region_kind) = match read.read_kind {
            FilesystemReplayReadKind::Positioned { offset } => (
                6,
                vec![
                    FilesystemScalarOperand {
                        operand_ordinal: 2,
                        value: FilesystemScalarOperandValue::U64(read.requested_count),
                    },
                    FilesystemScalarOperand {
                        operand_ordinal: 3,
                        value: FilesystemScalarOperandValue::I64(offset),
                    },
                ],
                FilesystemObservedByteRegionKind::PositionedFileRead,
            ),
            FilesystemReplayReadKind::Sequential => (
                4,
                vec![FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::U64(read.requested_count),
                }],
                FilesystemObservedByteRegionKind::SequentialFileRead,
            ),
        };
        FilesystemOperationAttempt {
            operation_tag,
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(read.read_result),
                post_error: read.read_post_error,
            }),
            scalar_operands,
            byte_operands: Vec::new(),
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: vec![FilesystemObservedByteRegion {
                output_operand_ordinal: 1,
                kind: region_kind,
                offset: 0,
                length: read_length,
            }],
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: vec![FilesystemMutableByteOperandResolution {
                operand_ordinal: 1,
                bytes: read.mutable_resolution,
            }],
            mutable_i64_operand_resolutions: Vec::new(),
            mutable_byte_operands: vec![FilesystemMutableByteOperand {
                operand_ordinal: 1,
                pre_bytes: read.mutable_pre_state,
                post_bytes: read.mutable_post_state,
            }],
            mutable_i64_operands: Vec::new(),
            authorized_paths: Vec::new(),
            logical_handle_inputs: vec![FilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: FilesystemLogicalHandleKind::Descriptor,
                resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
            }],
            logical_handle_output: None,
            retired_logical_handles: Vec::new(),
            grant_refusals: Vec::new(),
        }
    });
    let close = source_descriptor_close_attempt(identity, record.close_post_error);
    let mut attempts = Vec::with_capacity(read_count + 2);
    attempts.push(open);
    attempts.extend(reads);
    attempts.push(close);
    attempts
}

fn source_descriptor_metadata_attempts(
    record: FilesystemSourceDescriptorMetadataReplayRecord,
) -> [FilesystemOperationAttempt; 3] {
    let identity = record.logical_handle_identity;
    let open = source_descriptor_open_attempt(
        record.source_root,
        record.source_relative_path,
        identity,
        record.open_post_error,
    );
    let metadata = FilesystemOperationAttempt {
        operation_tag: 39,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: record.metadata_post_error,
        }),
        scalar_operands: Vec::new(),
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: Vec::new(),
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: vec![record.metadata],
        mutable_byte_operand_resolutions: vec![FilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: record.mutable_resolution,
        }],
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: vec![FilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: record.mutable_pre_state,
            post_bytes: record.mutable_post_state,
        }],
        mutable_i64_operands: Vec::new(),
        authorized_paths: Vec::new(),
        logical_handle_inputs: vec![FilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Descriptor,
            resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
        }],
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    };
    let close = source_descriptor_close_attempt(identity, record.close_post_error);
    [open, metadata, close]
}

fn source_descriptor_open_attempt(
    source_root: FilesystemGrantRootIdentity,
    source_relative_path: Vec<u8>,
    identity: FilesystemLogicalHandleIdentity,
    post_error: i32,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: 2,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::LogicalHandle(identity),
            post_error,
        }),
        scalar_operands: vec![FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I32(0),
        }],
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: vec![FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: source_root,
            relative_path: source_relative_path.clone(),
        }],
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: Vec::new(),
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: Vec::new(),
        mutable_i64_operands: Vec::new(),
        authorized_paths: vec![FilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: FilesystemGrantAccess::Read,
            root: source_root,
            relative_path: source_relative_path,
        }],
        logical_handle_inputs: Vec::new(),
        logical_handle_output: Some(FilesystemLogicalHandleOutput {
            kind: FilesystemLogicalHandleKind::Descriptor,
            identity,
            source: FilesystemLogicalHandleOutputSource::Created,
        }),
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

fn source_descriptor_close_attempt(
    identity: FilesystemLogicalHandleIdentity,
    post_error: i32,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: 8,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error,
        }),
        scalar_operands: Vec::new(),
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: Vec::new(),
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: Vec::new(),
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: Vec::new(),
        mutable_i64_operands: Vec::new(),
        authorized_paths: Vec::new(),
        logical_handle_inputs: vec![FilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Descriptor,
            resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
        }],
        logical_handle_output: None,
        retired_logical_handles: vec![identity],
        grant_refusals: Vec::new(),
    }
}

impl Default for EvaluationObservations {
    fn default() -> Self {
        Self {
            filesystem_operation_schema_version: FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
            filesystem_operation_attempts: Vec::new(),
            build_included_sources: Vec::new(),
            build_log: Vec::new(),
        }
    }
}

impl EvaluationObservations {
    #[cfg(test)]
    fn from_filesystem_operation_attempts(
        filesystem_operation_attempts: Vec<FilesystemOperationAttempt>,
        build_included_sources: Vec<BuildIncludedSource>,
    ) -> Self {
        Self {
            filesystem_operation_schema_version: FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
            filesystem_operation_attempts,
            build_included_sources,
            build_log: Vec::new(),
        }
    }

    fn from_build_run(
        filesystem_operation_attempts: Vec<FilesystemOperationAttempt>,
        build_included_sources: Vec<BuildIncludedSource>,
        build_log: Vec<u8>,
    ) -> Self {
        Self {
            filesystem_operation_schema_version: FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
            filesystem_operation_attempts,
            build_included_sources,
            build_log,
        }
    }

    pub fn filesystem_host_observed(&self) -> bool {
        !self.filesystem_operation_attempts.is_empty()
    }

    pub const fn filesystem_operation_schema_version(&self) -> u32 {
        self.filesystem_operation_schema_version
    }

    pub fn filesystem_operation_attempts(&self) -> &[FilesystemOperationAttempt] {
        &self.filesystem_operation_attempts
    }

    pub fn build_included_sources(&self) -> &[BuildIncludedSource] {
        &self.build_included_sources
    }

    /// Exact bytes emitted by the compiler-owned `Build.log` facet.
    pub fn build_log(&self) -> &[u8] {
        &self.build_log
    }
}

/// One explicit generated-source handoff emitted by the exact toolchain
/// `BuildOutput::include_source` machine during a successful granted build.
/// The compiler still has to match this coordinate to its captured staged tree
/// before the bytes may enter compilation. The filesystem-attempt ordinal binds
/// the handoff after the mutation it publishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildIncludedSource {
    root: FilesystemGrantRootIdentity,
    relative_path: Vec<u8>,
    filesystem_attempt_ordinal: usize,
}

impl BuildIncludedSource {
    pub(crate) fn new(
        root: FilesystemGrantRootIdentity,
        relative_path: Vec<u8>,
        filesystem_attempt_ordinal: usize,
    ) -> Self {
        Self {
            root,
            relative_path,
            filesystem_attempt_ordinal,
        }
    }

    /// Reconstruct one compiler-supplied handoff coordinate from canonical
    /// replay/codec data. This names a path and its ordering point only; it does
    /// not assert that the file exists or belongs to a reconstructed tree.
    pub fn from_coordinate(
        root: FilesystemGrantRootIdentity,
        relative_path: Vec<u8>,
        filesystem_attempt_ordinal: usize,
    ) -> Result<Self, String> {
        if !filesystem_root_relative_path_is_canonical(&relative_path, false) {
            return Err(
                "included build source must use a canonical non-root relative path".to_owned(),
            );
        }
        Ok(Self::new(root, relative_path, filesystem_attempt_ordinal))
    }

    pub const fn root(&self) -> FilesystemGrantRootIdentity {
        self.root
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }

    pub const fn filesystem_attempt_ordinal(&self) -> usize {
        self.filesystem_attempt_ordinal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildMachineEvaluationFailureKind {
    InvalidFilesystemGrant,
    Exit,
    Unsupported,
    Trap,
    ResourceExhausted,
    ResultAccountingOverflow,
    WorkerUnavailable,
    WorkerPanicked,
}

/// A failed granted build evaluation keeps partial work and host observations
/// when the evaluator returned normally. Worker creation/panic failures mark
/// both as unavailable rather than fabricating empty evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildMachineEvaluationFailure {
    kind: BuildMachineEvaluationFailureKind,
    diagnostic: String,
    usage: Option<EvaluationUsage>,
    observations: Option<EvaluationObservations>,
}

impl BuildMachineEvaluationFailure {
    fn with_evidence(
        kind: BuildMachineEvaluationFailureKind,
        diagnostic: String,
        usage: EvaluationUsage,
        observations: EvaluationObservations,
    ) -> Self {
        Self {
            kind,
            diagnostic,
            usage: Some(usage),
            observations: Some(observations),
        }
    }

    fn without_evidence(kind: BuildMachineEvaluationFailureKind, diagnostic: String) -> Self {
        Self {
            kind,
            diagnostic,
            usage: None,
            observations: None,
        }
    }

    pub const fn kind(&self) -> BuildMachineEvaluationFailureKind {
        self.kind
    }

    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub const fn usage(&self) -> Option<EvaluationUsage> {
        self.usage
    }

    pub const fn observations(&self) -> Option<&EvaluationObservations> {
        self.observations.as_ref()
    }

    pub fn into_diagnostic(self) -> String {
        self.diagnostic
    }
}

impl std::fmt::Display for BuildMachineEvaluationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.diagnostic)
    }
}

impl std::error::Error for BuildMachineEvaluationFailure {}

impl EvaluationUsage {
    const fn empty(fuel_ceiling: u64) -> Self {
        Self {
            schema: CURRENT_EVALUATION_USAGE_SCHEMA,
            schedule: CURRENT_EVALUATION_STEP_SCHEDULE,
            fuel_units: 0,
            fuel_ceiling,
            build_log_bytes: 0,
            filesystem_operation_attempts: 0,
            peak_live_cells: 0,
            peak_live_text_bytes: 0,
            result_cells: 0,
            result_text_bytes: 0,
        }
    }

    pub const fn schedule(self) -> EvaluationStepScheduleIdentity {
        self.schedule
    }

    pub const fn schema(self) -> EvaluationUsageSchemaIdentity {
        self.schema
    }

    pub const fn fuel_units(self) -> u64 {
        self.fuel_units
    }

    /// Exact per-invocation evaluator fuel ceiling installed for this run.
    pub const fn fuel_ceiling(self) -> u64 {
        self.fuel_ceiling
    }

    /// Bytes emitted through the compiler-owned BuildLog facet.
    pub const fn build_log_bytes(self) -> u64 {
        self.build_log_bytes
    }

    /// Canonical filesystem calls that entered operation-attempt capture.
    pub const fn filesystem_operation_attempts(self) -> u64 {
        self.filesystem_operation_attempts
    }

    /// Maximum semantic interpreter-cell allocations live concurrently during
    /// this invocation. This is an allocation count, not a byte estimate.
    pub const fn peak_live_cells(self) -> u64 {
        self.peak_live_cells
    }

    /// Maximum logical bytes held by interpreter Text backing buffers during
    /// this invocation. This is not Vec capacity or process-memory usage.
    pub const fn peak_live_text_bytes(self) -> u64 {
        self.peak_live_text_bytes
    }

    /// Number of value cells retained by the successful evaluation result.
    /// Scalar and unit roots count as one cell; each structured value counts
    /// its root plus every recursively retained field, payload, or element.
    pub const fn result_cells(self) -> u64 {
        self.result_cells
    }

    /// Exact Text payload bytes retained by the successful result.
    pub const fn result_text_bytes(self) -> u64 {
        self.result_text_bytes
    }

    fn charge_step(&mut self) -> Option<()> {
        self.fuel_units = self.fuel_units.checked_add(1)?;
        Some(())
    }

    fn set_fuel_ceiling(&mut self, fuel_ceiling: u64) {
        self.fuel_ceiling = fuel_ceiling;
    }

    fn charge_build_log_bytes(&mut self, bytes: u64) -> Option<()> {
        self.build_log_bytes = self.build_log_bytes.checked_add(bytes)?;
        Some(())
    }

    fn charge_filesystem_operation_attempt(&mut self) -> Option<()> {
        self.filesystem_operation_attempts = self.filesystem_operation_attempts.checked_add(1)?;
        Some(())
    }

    fn record_peak_live_cells(&mut self, peak_live_cells: u64) {
        self.peak_live_cells = peak_live_cells;
    }

    fn record_peak_live_text_bytes(&mut self, peak_live_text_bytes: u64) {
        self.peak_live_text_bytes = peak_live_text_bytes;
    }

    fn record_result_custody(&mut self, result_cells: u64, result_text_bytes: u64) {
        self.result_cells = result_cells;
        self.result_text_bytes = result_text_bytes;
    }
}

/// A successful semantic evaluation paired with deterministic usage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredEvaluation<T> {
    value: T,
    usage: EvaluationUsage,
}

impl<T> MeasuredEvaluation<T> {
    fn new(value: T, usage: EvaluationUsage) -> Self {
        Self { value, usage }
    }

    pub const fn value(&self) -> &T {
        &self.value
    }

    pub const fn usage(&self) -> EvaluationUsage {
        self.usage
    }

    pub fn into_value(self) -> T {
        self.value
    }

    pub fn into_parts(self) -> (T, EvaluationUsage) {
        (self.value, self.usage)
    }
}

/// One executed compiler-known private layout placement. The static
/// conformance application is retained exactly; semantic validation, not the
/// evaluator, decides whether it is the declared slot for the active layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateLayoutPlacementReceipt {
    pub operation_expression: psi_typed_trees::expression::ExpressionHandle,
    pub selected_slot: psi_typed_trees::expression::StaticMachineArgument,
    pub offset: u64,
}

/// Structured evaluation plus compiler-only operation receipts. Receipts are
/// not Omega values and cannot be observed or fabricated by evaluated code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildTimeOperationEvaluation<T> {
    measured: MeasuredEvaluation<T>,
    private_layout_placements: Vec<PrivateLayoutPlacementReceipt>,
}

impl<T> BuildTimeOperationEvaluation<T> {
    fn new(
        value: T,
        usage: EvaluationUsage,
        private_layout_placements: Vec<PrivateLayoutPlacementReceipt>,
    ) -> Self {
        Self {
            measured: MeasuredEvaluation::new(value, usage),
            private_layout_placements,
        }
    }

    pub const fn value(&self) -> &T {
        self.measured.value()
    }

    pub const fn usage(&self) -> EvaluationUsage {
        self.measured.usage()
    }

    pub fn private_layout_placements(&self) -> &[PrivateLayoutPlacementReceipt] {
        &self.private_layout_placements
    }

    pub fn into_parts(self) -> (T, EvaluationUsage, Vec<PrivateLayoutPlacementReceipt>) {
        let (value, usage) = self.measured.into_parts();
        (value, usage, self.private_layout_placements)
    }
}

/// A granted build-machine result keeps host observations beside, but
/// distinct from, deterministic evaluator work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredBuildMachineEvaluation<T> {
    measured: MeasuredEvaluation<T>,
    observations: EvaluationObservations,
}

impl<T> MeasuredBuildMachineEvaluation<T> {
    fn new(value: T, usage: EvaluationUsage, observations: EvaluationObservations) -> Self {
        Self {
            measured: MeasuredEvaluation::new(value, usage),
            observations,
        }
    }

    /// Lift a statically pure build-machine evaluation into the common build
    /// result without inventing a host observation.
    pub fn hermetic(measured: MeasuredEvaluation<T>) -> Self {
        Self {
            measured,
            observations: EvaluationObservations::default(),
        }
    }

    pub const fn value(&self) -> &T {
        self.measured.value()
    }

    pub const fn usage(&self) -> EvaluationUsage {
        self.measured.usage()
    }

    pub const fn observations(&self) -> &EvaluationObservations {
        &self.observations
    }

    pub fn into_value(self) -> T {
        self.measured.into_value()
    }

    pub fn into_parts(self) -> (T, EvaluationUsage, EvaluationObservations) {
        let (value, usage) = self.measured.into_parts();
        (value, usage, self.observations)
    }
}

/// The result of interpreting a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpretOutcome {
    /// The process exit code (from `exit_process`, or 0 if the program ran to a terminal
    /// transition without exiting).
    pub exit_code: i32,
    /// Bytes written to stdout via `write` / `write_line`.
    pub stdout: Vec<u8>,
    /// Bytes written to stderr via `write_error` / `write_error_line`.
    pub stderr: Vec<u8>,
    /// `Some` when the interpreter hit an UNSUPPORTED construct (so a harness can skip),
    /// or a genuine trap. `None` on a clean run.
    pub error: Option<String>,
    /// Deterministic work under the current evaluator-step schedule. This is a
    /// precursor usage record, not canonical-IR fuel.
    pub usage: EvaluationUsage,
}

impl InterpretOutcome {
    fn exited(exit_code: i32, stdout: Vec<u8>, stderr: Vec<u8>, usage: EvaluationUsage) -> Self {
        Self {
            exit_code,
            stdout,
            stderr,
            error: None,
            usage,
        }
    }

    fn error(
        message: impl Into<String>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        usage: EvaluationUsage,
    ) -> Self {
        Self {
            exit_code: 0,
            stdout,
            stderr,
            error: Some(message.into()),
            usage,
        }
    }

    /// Whether the interpreter declined to evaluate the program (unsupported construct or
    /// trap). Differential harnesses skip these rather than treat them as a mismatch.
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

/// Interpret a checked program from one exact machine identity.
///
/// Build/target selection owns this identity. The interpreter neither discovers
/// an entry from source spelling nor retries alternate names. `stdin` provides
/// the bytes a `read_line` host call would consume.
pub fn interpret_entry(
    checked: &CheckedTrees,
    entry_machine_name: &str,
    stdin: &[u8],
) -> InterpretOutcome {
    evaluator::run(checked, entry_machine_name, stdin)
}

/// How the interpreter serves a program's `Filesystem` capability.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FilesystemAccess {
    /// The deterministic in-memory filesystem (the default; the differential
    /// oracle). Hermetic: no real disk is ever touched.
    #[default]
    Virtual,
    /// The REAL host filesystem, UNSCOPED: fs ops act on real paths with the
    /// invoking process's full authority. The trust level of `build.rs`; for
    /// build.omg proper, prefer [`FilesystemAccess::RealScoped`] -- the grants
    /// ARE the audit surface (open-work #3's settled design).
    RealUnscoped,
    /// The REAL host filesystem behind PATH GRANTS (build.omg rung 2): reads
    /// must land under a read or write root, writes/creates/removes under a
    /// write root; anything else is refused with EACCES before the OS is
    /// touched. build.omg's shape: read = source tree, write = build dir.
    RealScoped(FsGrants),
    /// The same path authority, plus a compiler-owned resource sponsor shared
    /// across every build-machine evaluation in one package-review session.
    /// The program cannot inspect or enlarge this account.
    RealScopedSponsored {
        grants: FsGrants,
        sponsor: FilesystemSponsor,
    },
    /// Consume compiler-produced bounded events without installing host
    /// filesystem authority. Source observations are record-served; an
    /// admitted Output suffix executes in a fresh virtual namespace. Every
    /// event and lane must match exactly and the record must be exhausted.
    ReplayFilesystem(FilesystemReplay),
}

/// Path grants for [`FilesystemAccess::RealScoped`]. Roots are canonicalized
/// when the run starts (so symlinked spellings of a root work), and every
/// op's path is canonicalized before the prefix check (so `..` traversal and
/// symlinks INSIDE a granted tree that point OUTSIDE it are resolved and
/// refused, not string-matched).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FsGrants {
    /// Trees the program may READ from (open read-only, stat). A write root
    /// implicitly grants read-back -- staging then verifying is the normal
    /// build shape -- so these are the read-ONLY trees.
    pub read_roots: Vec<FilesystemGrantRoot>,
    /// Trees the program may WRITE under: create/truncate/append opens,
    /// remove, create_dir/remove_dir, and BOTH ends of a rename.
    pub write_roots: Vec<FilesystemGrantRoot>,
}

/// Options for [`interpret_entry_with_options`]. `Default` selects the hermetic
/// virtual filesystem and the compiler host's checked standard metadata
/// carrier. Cross-target and package-build callers supply the selected checked
/// metadata layout explicitly.
#[derive(Clone, Debug, Default)]
pub struct InterpretOptions {
    pub filesystem: FilesystemAccess,
    pub filesystem_metadata_layout: FilesystemMetadataLayout,
}

/// [`interpret_entry`] with explicit [`InterpretOptions`].
pub fn interpret_entry_with_options(
    checked: &CheckedTrees,
    entry_machine_name: &str,
    stdin: &[u8],
    options: InterpretOptions,
) -> InterpretOutcome {
    evaluator::run_with_options(checked, entry_machine_name, stdin, options)
}

/// CONST EVALUATION (comptime stage 1): evaluate the zero-argument machine
/// `machine_name` at compile time, returning its terminal value width-adjusted
/// to the machine's declared integer return type (TARGET widths -- the same
/// wrap-on-write the differential interpreter applies -- never host widths).
///
/// The CALLER owns the legality gate (the machine's inferred transitive effect
/// surface must be empty and it must take no parameters -- frozen decision 12's
/// purity predicate); this entry owns evaluation only. Termination rides the
/// language's existing discipline (no general recursion, loops carry
/// decreases); a small evaluator-step ceiling (~100k units) backstops checker
/// gaps. Errors are human-readable reasons for the compile diagnostic at the
/// const site.
///
/// Works over `TypedTrees` (pre-checking) so the compiler pipeline can
/// substitute results BEFORE range checking and layout consume the lengths.
pub fn evaluate_const_machine(
    program: &psi_typed_trees::TypedTrees,
    machine_name: &str,
) -> Result<i64, String> {
    evaluate_const_machine_measured(program, machine_name).map(MeasuredEvaluation::into_value)
}

/// [`evaluate_const_machine`] with its deterministic evaluator usage.
pub fn evaluate_const_machine_measured(
    program: &psi_typed_trees::TypedTrees,
    machine_name: &str,
) -> Result<MeasuredEvaluation<i64>, String> {
    evaluator::run_const_machine(program, machine_name)
}

/// STRUCTURED build-time evaluation (design_briefs/build_time_evaluation.md;
/// the R2 layouts enabler): invoke the effect-free machine `machine_name`
/// with compiler-built arguments and read back its terminal value as a
/// structured tree. As with [`evaluate_const_machine`], the CALLER owns the
/// legality gate (decision 12's transitive effect surface must be empty and
/// parameters must be by-value); this entry owns evaluation, positional
/// argument binding (count-checked), and the evaluator-step ceiling. No keyword
/// marks build-time machines -- the position makes the evaluation build-time,
/// and the effect system makes it legal.
pub fn evaluate_build_time_machine(
    program: &psi_typed_trees::TypedTrees,
    machine_name: &str,
    arguments: Vec<BuildTimeValue>,
) -> Result<BuildTimeValue, String> {
    evaluate_build_time_machine_measured(program, machine_name, arguments)
        .map(MeasuredEvaluation::into_value)
}

/// [`evaluate_build_time_machine`] with its deterministic evaluator usage.
pub fn evaluate_build_time_machine_measured(
    program: &psi_typed_trees::TypedTrees,
    machine_name: &str,
    arguments: Vec<BuildTimeValue>,
) -> Result<MeasuredEvaluation<BuildTimeValue>, String> {
    evaluator::run_build_time_machine(program, machine_name, arguments)
}

/// Evaluate one build-time machine while retaining compiler-known operation
/// receipts beside the ordinary result. This is the authoritative entry for
/// native layout policies containing `Plan::place_private`; callers that do
/// not consume such receipts may continue using [`evaluate_build_time_machine`].
pub fn evaluate_build_time_machine_with_operation_receipts(
    program: &psi_typed_trees::TypedTrees,
    machine_name: &str,
    arguments: Vec<BuildTimeValue>,
) -> Result<BuildTimeOperationEvaluation<BuildTimeValue>, String> {
    evaluator::run_build_time_machine_with_operation_receipts(program, machine_name, arguments)
}

/// The AUGMENTING-MACHINE build-time entry (build_and_package_model.md): run
/// the effect-free machine and read back the FINAL argument values -- the
/// `machine build(b: &mut Build)` shape, where the machine's output IS its
/// augmented arguments. A unit terminal is accepted. The caller owns the
/// legality gate, exactly as for [`evaluate_build_time_machine`].
pub fn evaluate_build_time_machine_arguments(
    program: &psi_typed_trees::TypedTrees,
    machine_name: &str,
    arguments: Vec<BuildTimeValue>,
) -> Result<Vec<BuildTimeValue>, String> {
    evaluate_build_time_machine_arguments_measured(program, machine_name, arguments)
        .map(MeasuredEvaluation::into_value)
}

/// [`evaluate_build_time_machine_arguments`] with deterministic evaluator
/// usage.
pub fn evaluate_build_time_machine_arguments_measured(
    program: &psi_typed_trees::TypedTrees,
    machine_name: &str,
    arguments: Vec<BuildTimeValue>,
) -> Result<MeasuredEvaluation<Vec<BuildTimeValue>>, String> {
    evaluator::run_build_time_machine_arguments(program, machine_name, arguments)
}

/// Sponsored form of [`evaluate_build_time_machine_arguments_measured`]. The
/// sponsor is compiler-only and shared across every evaluation using one of
/// its clones.
pub fn evaluate_build_time_machine_arguments_measured_with_sponsor(
    program: &psi_typed_trees::TypedTrees,
    machine_name: &str,
    arguments: Vec<BuildTimeValue>,
    sponsor: &BuildEvaluationSponsor,
) -> Result<MeasuredEvaluation<Vec<BuildTimeValue>>, String> {
    evaluator::run_build_time_machine_arguments_with_sponsor(
        program,
        machine_name,
        arguments,
        sponsor,
    )
}

/// Evaluate an effect-free augmenting build machine while retaining output
/// from the compiler-owned `Build.log` facet as a distinct observation.
pub fn evaluate_observed_build_time_machine_arguments_measured(
    program: &psi_typed_trees::TypedTrees,
    machine_name: &str,
    arguments: Vec<BuildTimeValue>,
) -> Result<MeasuredBuildMachineEvaluation<Vec<BuildTimeValue>>, String> {
    evaluator::run_observed_build_time_machine_arguments(program, machine_name, arguments)
}

/// Sponsored form of
/// [`evaluate_observed_build_time_machine_arguments_measured`].
pub fn evaluate_observed_build_time_machine_arguments_measured_with_sponsor(
    program: &psi_typed_trees::TypedTrees,
    machine_name: &str,
    arguments: Vec<BuildTimeValue>,
    sponsor: &BuildEvaluationSponsor,
) -> Result<MeasuredBuildMachineEvaluation<Vec<BuildTimeValue>>, String> {
    evaluator::run_observed_build_time_machine_arguments_with_sponsor(
        program,
        machine_name,
        arguments,
        sponsor,
    )
}

/// The GRANTED build entry (open-work #3's settled design, rung 4): run the
/// augmenting `build(b: &mut Build)` machine WITH a `Filesystem` capability
/// and read back the augmented arguments. The capability grant IS the audit
/// surface -- filesystem ops are allowed and served per `options` (hermetic
/// virtual by default; real scoped/unscoped for actual builds), while every
/// OTHER host boundary (console, clock, gui) still rejects dynamically.
/// Unlike [`evaluate_build_time_machine_arguments`], the caller does NOT
/// require an empty transitive effect surface for the filesystem effect --
/// but should still gate the rest.
pub fn evaluate_build_machine_with_filesystem(
    program: &psi_typed_trees::TypedTrees,
    machine_name: &str,
    arguments: Vec<BuildTimeValue>,
    options: InterpretOptions,
) -> Result<Vec<BuildTimeValue>, String> {
    evaluate_build_machine_with_filesystem_measured(program, machine_name, arguments, options)
        .map(MeasuredBuildMachineEvaluation::into_value)
        .map_err(BuildMachineEvaluationFailure::into_diagnostic)
}

/// [`evaluate_build_machine_with_filesystem`] with deterministic evaluator
/// usage and distinct host observations.
pub fn evaluate_build_machine_with_filesystem_measured(
    program: &psi_typed_trees::TypedTrees,
    machine_name: &str,
    arguments: Vec<BuildTimeValue>,
    options: InterpretOptions,
) -> Result<MeasuredBuildMachineEvaluation<Vec<BuildTimeValue>>, BuildMachineEvaluationFailure> {
    evaluator::run_granted_build_machine_arguments(program, machine_name, arguments, options)
}

/// Sponsored form of [`evaluate_build_machine_with_filesystem_measured`].
pub fn evaluate_build_machine_with_filesystem_measured_with_sponsor(
    program: &psi_typed_trees::TypedTrees,
    machine_name: &str,
    arguments: Vec<BuildTimeValue>,
    options: InterpretOptions,
    sponsor: &BuildEvaluationSponsor,
) -> Result<MeasuredBuildMachineEvaluation<Vec<BuildTimeValue>>, BuildMachineEvaluationFailure> {
    evaluator::run_granted_build_machine_arguments_with_sponsor(
        program,
        machine_name,
        arguments,
        options,
        sponsor,
    )
}

#[cfg(test)]
mod canonical_filesystem_metadata_tests {
    use super::*;

    fn row(
        path: &[u8],
        kind: CanonicalFilesystemMetadataRowKind,
    ) -> CanonicalFilesystemMetadataRow {
        CanonicalFilesystemMetadataRow::new(path.to_vec(), kind)
    }

    #[test]
    fn canonical_metadata_accepts_raw_non_utf8_paths_and_preserves_ordered_rows() {
        let index = CanonicalFilesystemMetadataIndex::version_1(
            [3; 32],
            [
                row(b"raw-\xff", CanonicalFilesystemMetadataRowKind::Directory),
                row(b"a:b", CanonicalFilesystemMetadataRowKind::Directory),
                row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                row(
                    b"raw-\xff/file",
                    CanonicalFilesystemMetadataRowKind::File {
                        executable: false,
                        logical_byte_length: 9,
                    },
                ),
            ],
        )
        .unwrap();

        assert_eq!(index.policy_version(), 1);
        assert_eq!(index.source_content_commitment(), &[3; 32]);
        assert_eq!(
            index
                .rows()
                .map(|row| row.relative_path().to_vec())
                .collect::<Vec<_>>(),
            vec![
                b"".to_vec(),
                b"a:b".to_vec(),
                b"raw-\xff".to_vec(),
                b"raw-\xff/file".to_vec()
            ]
        );
    }

    #[test]
    fn canonical_metadata_rejects_invalid_and_duplicate_paths() {
        for invalid in [
            b"/absolute".as_slice(),
            b"a//b".as_slice(),
            b"a/./b".as_slice(),
            b"a/../b".as_slice(),
            b"a\\b".as_slice(),
            b"a\0b".as_slice(),
        ] {
            assert!(matches!(
                CanonicalFilesystemMetadataIndex::version_1(
                    [0; 32],
                    [
                        row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                        row(invalid, CanonicalFilesystemMetadataRowKind::Directory),
                    ],
                ),
                Err(CanonicalFilesystemMetadataIndexError::InvalidRelativePath(path))
                    if path == invalid
            ));
        }
        assert!(matches!(
            CanonicalFilesystemMetadataIndex::version_1(
                [0; 32],
                [
                    row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                    row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                ],
            ),
            Err(CanonicalFilesystemMetadataIndexError::DuplicateRelativePath(path))
                if path.is_empty()
        ));
    }

    #[test]
    fn canonical_metadata_requires_one_directory_root_and_directory_parent_closure() {
        assert!(matches!(
            CanonicalFilesystemMetadataIndex::version_1([0; 32], []),
            Err(CanonicalFilesystemMetadataIndexError::MissingRootDirectory)
        ));
        assert!(matches!(
            CanonicalFilesystemMetadataIndex::version_1(
                [0; 32],
                [row(
                    b"",
                    CanonicalFilesystemMetadataRowKind::File {
                        executable: false,
                        logical_byte_length: 0,
                    },
                )],
            ),
            Err(CanonicalFilesystemMetadataIndexError::RootIsNotDirectory)
        ));
        assert!(matches!(
            CanonicalFilesystemMetadataIndex::version_1(
                [0; 32],
                [
                    row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                    row(
                        b"missing/leaf",
                        CanonicalFilesystemMetadataRowKind::File {
                            executable: false,
                            logical_byte_length: 0,
                        },
                    ),
                ],
            ),
            Err(CanonicalFilesystemMetadataIndexError::MissingParentDirectory(path))
                if path == b"missing/leaf"
        ));
        assert!(matches!(
            CanonicalFilesystemMetadataIndex::version_1(
                [0; 32],
                [
                    row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                    row(
                        b"file",
                        CanonicalFilesystemMetadataRowKind::File {
                            executable: false,
                            logical_byte_length: 0,
                        },
                    ),
                    row(b"file/child", CanonicalFilesystemMetadataRowKind::Directory),
                ],
            ),
            Err(CanonicalFilesystemMetadataIndexError::ParentIsNotDirectory(path))
                if path == b"file/child"
        ));
    }

    #[test]
    fn canonical_metadata_rejects_lengths_outside_the_stat_domain() {
        assert!(matches!(
            CanonicalFilesystemMetadataIndex::version_1(
                [0; 32],
                [
                    row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                    row(
                        b"huge",
                        CanonicalFilesystemMetadataRowKind::File {
                            executable: false,
                            logical_byte_length: i64::MAX as u64 + 1,
                        },
                    ),
                ],
            ),
            Err(CanonicalFilesystemMetadataIndexError::LogicalByteLengthExceedsI64(path))
                if path == b"huge"
        ));
    }

    #[test]
    fn canonical_metadata_rejects_more_rows_than_the_resolver_can_issue() {
        let rows = std::iter::once(row(b"", CanonicalFilesystemMetadataRowKind::Directory)).chain(
            (0..CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT).map(|index| {
                CanonicalFilesystemMetadataRow::new(
                    format!("entry-{index}").into_bytes(),
                    CanonicalFilesystemMetadataRowKind::File {
                        executable: false,
                        logical_byte_length: 0,
                    },
                )
            }),
        );

        assert_eq!(
            CanonicalFilesystemMetadataIndex::version_1([0; 32], rows),
            Err(CanonicalFilesystemMetadataIndexError::RowLimitExceeded {
                limit: CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT,
                attempted: CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT + 1,
            })
        );
    }
}
