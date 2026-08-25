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
/// and a typed returned or evaluator-halted outcome. It is not the canonical
/// replay transcript: path-like byte operands beyond rooted grant evidence,
/// complete returned content, and complete content custody are not present yet.
pub const FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION: u32 = 9;

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
/// Failed handle-result sentinels remain scalar results. Mutable carrier
/// regions retain complete pre/post snapshots, but
/// complete path/content custody is still absent, so this remains below
/// receipt strength.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemOperationAttempt {
    operation_tag: u16,
    provider: FilesystemObservationProvider,
    outcome: Option<FilesystemOperationAttemptOutcome>,
    scalar_operands: Vec<FilesystemScalarOperand>,
    byte_operands: Vec<FilesystemByteOperand>,
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
/// virtual filesystem.
#[derive(Clone, Debug, Default)]
pub struct InterpretOptions {
    pub filesystem: FilesystemAccess,
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
