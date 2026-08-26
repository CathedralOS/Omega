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
//! `Rc<RefCell<Value>>` ([`value::Cell`]). A `&mut place` argument evaluates to a
//! [`Value::Ref`] holding a CLONE of the *same* `Rc`, so a write through the reference
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

mod build_time;
mod evaluator;
mod filesystem_sponsor;
mod value;

pub use build_time::BuildTimeValue;
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
    EvaluationUsageSchemaIdentity(1);

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
    result_cells: u64,
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
pub const FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION: u32 = 18;

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
}

impl FilesystemGrantRoot {
    pub fn new(identity: FilesystemGrantRootIdentity, path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            identity,
            path: path.into(),
        }
    }

    pub const fn identity(&self) -> FilesystemGrantRootIdentity {
        self.identity
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
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
}

/// Opaque, compiler-produced operation record for the first filesystem replay
/// rung. The bounded rung accepts only `open`, one or more sequential or
/// positioned reads, and `close`; broadening that set requires explicit replay
/// semantics for the added operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemReplay {
    attempts: Vec<FilesystemOperationAttempt>,
}

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

/// Typed input used to reconstruct the first replay rung after its canonical
/// compiler record has crossed a process boundary. Construction validates a
/// nonempty set of closed, descriptor-disjoint source-read chains; it grants no
/// host filesystem authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSourceReadChainsReplayRecord {
    chains: Vec<FilesystemSourceReadChainReplayRecord>,
}

impl FilesystemSourceReadChainsReplayRecord {
    pub fn new(chains: Vec<FilesystemSourceReadChainReplayRecord>) -> Result<Self, String> {
        if chains.is_empty() {
            return Err("filesystem replay requires at least one source-read chain".to_owned());
        }
        for (index, chain) in chains.iter().enumerate() {
            if chains[..index]
                .iter()
                .any(|prior| prior.logical_handle_identity == chain.logical_handle_identity)
            {
                return Err(
                    "filesystem replay source-read chains must use distinct handles".to_owned(),
                );
            }
        }
        Ok(Self { chains })
    }
}

impl FilesystemReplay {
    pub fn from_source_read_chains_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        if observations.filesystem_operation_schema_version()
            != FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
        {
            return Err("filesystem replay observation schema is not current".to_owned());
        }
        let attempts = observations.filesystem_operation_attempts();
        let mut cursor = 0;
        let mut chain_count = 0;
        while cursor < attempts.len() {
            if attempts[cursor].operation_tag() != 2 {
                return Err(
                    "bounded filesystem replay requires one or more closed source-read chains"
                        .to_owned(),
                );
            }
            cursor += 1;
            let reads_start = cursor;
            while cursor < attempts.len() && matches!(attempts[cursor].operation_tag(), 4 | 6) {
                cursor += 1;
            }
            if cursor == reads_start
                || cursor == attempts.len()
                || attempts[cursor].operation_tag() != 8
            {
                return Err(
                    "bounded filesystem replay requires one or more closed source-read chains"
                        .to_owned(),
                );
            }
            cursor += 1;
            chain_count += 1;
        }
        if chain_count == 0 {
            return Err(
                "bounded filesystem replay requires one or more closed source-read chains"
                    .to_owned(),
            );
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
        Ok(Self {
            attempts: attempts.to_vec(),
        })
    }

    pub fn attempts(&self) -> &[FilesystemOperationAttempt] {
        &self.attempts
    }

    pub fn from_source_read_chains_record(record: FilesystemSourceReadChainsReplayRecord) -> Self {
        let attempt_count = record
            .chains
            .iter()
            .map(|chain| chain.reads.len() + 2)
            .sum();
        let mut attempts = Vec::with_capacity(attempt_count);
        for chain in record.chains {
            attempts.extend(source_read_chain_attempts(chain));
        }
        Self { attempts }
    }
}

fn source_read_chain_attempts(
    record: FilesystemSourceReadChainReplayRecord,
) -> Vec<FilesystemOperationAttempt> {
    let identity = record.logical_handle_identity;
    let open = FilesystemOperationAttempt {
        operation_tag: 2,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::LogicalHandle(identity),
            post_error: record.open_post_error,
        }),
        scalar_operands: vec![FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I32(0),
        }],
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: vec![FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: record.source_root,
            relative_path: record.source_relative_path.clone(),
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
            root: record.source_root,
            relative_path: record.source_relative_path,
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
    let mut attempts = Vec::with_capacity(read_count + 2);
    attempts.push(open);
    attempts.extend(reads);
    attempts.push(close);
    attempts
}

impl Default for EvaluationObservations {
    fn default() -> Self {
        Self {
            filesystem_operation_schema_version: FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
            filesystem_operation_attempts: Vec::new(),
            build_included_sources: Vec::new(),
        }
    }
}

impl EvaluationObservations {
    fn from_filesystem_operation_attempts(
        filesystem_operation_attempts: Vec<FilesystemOperationAttempt>,
        build_included_sources: Vec<BuildIncludedSource>,
    ) -> Self {
        Self {
            filesystem_operation_schema_version: FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
            filesystem_operation_attempts,
            build_included_sources,
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
}

/// One explicit generated-source handoff emitted by the exact toolchain
/// `BuildOutput::include_source` machine during a successful granted build.
/// The compiler still has to match this coordinate to its captured staged
/// tree before the bytes may enter compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildIncludedSource {
    root: FilesystemGrantRootIdentity,
    relative_path: Vec<u8>,
}

impl BuildIncludedSource {
    pub(crate) fn new(root: FilesystemGrantRootIdentity, relative_path: Vec<u8>) -> Self {
        Self {
            root,
            relative_path,
        }
    }

    pub const fn root(&self) -> FilesystemGrantRootIdentity {
        self.root
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
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
    const fn empty() -> Self {
        Self {
            schema: CURRENT_EVALUATION_USAGE_SCHEMA,
            schedule: CURRENT_EVALUATION_STEP_SCHEDULE,
            fuel_units: 0,
            result_cells: 0,
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

    /// Number of value cells retained by the successful evaluation result.
    /// Scalar and unit roots count as one cell; each structured value counts
    /// its root plus every recursively retained field, payload, or element.
    pub const fn result_cells(self) -> u64 {
        self.result_cells
    }

    fn charge_step(&mut self) -> Option<()> {
        self.fuel_units = self.fuel_units.checked_add(1)?;
        Some(())
    }

    fn record_result_cells(&mut self, result_cells: u64) {
        self.result_cells = result_cells;
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
    /// Consume compiler-produced source-read chains without
    /// installing virtual or real filesystem authority. Every event and lane
    /// must match exactly and the record must be exhausted.
    ReplaySourceReadChains(FilesystemReplay),
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
