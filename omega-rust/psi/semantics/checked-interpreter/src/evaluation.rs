//! Evaluator work, observed effects, and returned-result custody.

use crate::{
    FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION, FilesystemGrantRootIdentity,
    FilesystemOperationAttempt, filesystem_root_relative_path_is_canonical,
};

/// Versioned identity of the checked interpreter semantics used for
/// build-time result evaluation. This is independent of work accounting: a
/// semantic change must advance this marker even when the step schedule and
/// usage-record shape stay unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvaluationSemanticsIdentity(u32);

impl EvaluationSemanticsIdentity {
    pub const fn marker(self) -> u32 {
        self.0
    }
}

pub const CURRENT_EVALUATION_SEMANTICS: EvaluationSemanticsIdentity =
    EvaluationSemanticsIdentity(1);

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
    pub(crate) schema: EvaluationUsageSchemaIdentity,
    pub(crate) schedule: EvaluationStepScheduleIdentity,
    pub(crate) fuel_units: u64,
    pub(crate) fuel_ceiling: u64,
    pub(crate) build_log_bytes: u64,
    pub(crate) filesystem_operation_attempts: u64,
    pub(crate) peak_live_cells: u64,
    pub(crate) peak_live_text_bytes: u64,
    pub(crate) result_cells: u64,
    pub(crate) result_text_bytes: u64,
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
    pub(crate) filesystem_operation_schema_version: u32,
    pub(crate) filesystem_operation_attempts: Vec<FilesystemOperationAttempt>,
    pub(crate) build_included_sources: Vec<BuildIncludedSource>,
    pub(crate) build_log: Vec<u8>,
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
    pub(crate) fn from_filesystem_operation_attempts(
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

    pub(crate) fn from_build_run(
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
    pub(crate) root: FilesystemGrantRootIdentity,
    pub(crate) relative_path: Vec<u8>,
    pub(crate) filesystem_attempt_ordinal: usize,
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

/// One product-entry description issued by the compiler-owned
/// `Build.product.entry` query during build evaluation. The evaluator keeps
/// this semantic payload in a private side table; the `ProductEntryRef` value
/// handed to evaluated code carries only an opaque index into it, so source
/// cannot read, convert, or fabricate the selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribedProductEntry {
    /// Exact product machine the description selected in its query's lexical
    /// package scope. Final admission rejoins this symbol; it never reselects
    /// by spelling.
    pub machine_symbol: symbols::SymbolHandle,
    /// The selected machine's authored name, retained for diagnostics only.
    pub machine_name: String,
    /// The root slot spelling the description was selected for. A later
    /// `roots.bind` must spell the same slot path.
    pub slot: String,
}

/// One executed `roots.bind` declaration plus, when its implementation
/// operand was a delegated description, the exact product entry the compiler
/// issued for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedRootBinding {
    /// The exact `roots.bind` statement that executed.
    pub statement: typed_trees::statement::StatementHandle,
    /// `Some` when the implementation operand evaluated to a compiler-issued
    /// `ProductEntryRef` description instead of a product machine path.
    pub described: Option<DescribedProductEntry>,
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
    pub(crate) kind: BuildMachineEvaluationFailureKind,
    pub(crate) diagnostic: String,
    pub(crate) usage: Option<EvaluationUsage>,
    pub(crate) observations: Option<EvaluationObservations>,
}

impl BuildMachineEvaluationFailure {
    pub(crate) fn with_evidence(
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

    pub(crate) fn without_evidence(
        kind: BuildMachineEvaluationFailureKind,
        diagnostic: String,
    ) -> Self {
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
    pub(crate) const fn empty(fuel_ceiling: u64) -> Self {
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

    pub(crate) fn charge_step(&mut self) -> Option<()> {
        self.fuel_units = self.fuel_units.checked_add(1)?;
        Some(())
    }

    pub(crate) fn set_fuel_ceiling(&mut self, fuel_ceiling: u64) {
        self.fuel_ceiling = fuel_ceiling;
    }

    pub(crate) fn charge_build_log_bytes(&mut self, bytes: u64) -> Option<()> {
        self.build_log_bytes = self.build_log_bytes.checked_add(bytes)?;
        Some(())
    }

    pub(crate) fn charge_filesystem_operation_attempt(&mut self) -> Option<()> {
        self.filesystem_operation_attempts = self.filesystem_operation_attempts.checked_add(1)?;
        Some(())
    }

    pub(crate) fn record_peak_live_cells(&mut self, peak_live_cells: u64) {
        self.peak_live_cells = peak_live_cells;
    }

    pub(crate) fn record_peak_live_text_bytes(&mut self, peak_live_text_bytes: u64) {
        self.peak_live_text_bytes = peak_live_text_bytes;
    }

    pub(crate) fn record_result_custody(&mut self, result_cells: u64, result_text_bytes: u64) {
        self.result_cells = result_cells;
        self.result_text_bytes = result_text_bytes;
    }
}

/// A successful semantic evaluation paired with deterministic usage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredEvaluation<T> {
    pub(crate) value: T,
    pub(crate) usage: EvaluationUsage,
}

impl<T> MeasuredEvaluation<T> {
    pub(crate) fn new(value: T, usage: EvaluationUsage) -> Self {
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
    pub operation_expression: typed_trees::expression::ExpressionHandle,
    pub selected_slot: typed_trees::expression::StaticMachineArgument,
    pub offset: u64,
}

/// Structured evaluation plus compiler-only operation receipts. Receipts are
/// not Omega values and cannot be observed or fabricated by evaluated code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildTimeOperationEvaluation<T> {
    pub(crate) measured: MeasuredEvaluation<T>,
    pub(crate) private_layout_placements: Vec<PrivateLayoutPlacementReceipt>,
}

impl<T> BuildTimeOperationEvaluation<T> {
    pub(crate) fn new(
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
    pub(crate) measured: MeasuredEvaluation<T>,
    pub(crate) observations: EvaluationObservations,
    pub(crate) executed_root_bindings: Vec<ExecutedRootBinding>,
}

impl<T> MeasuredBuildMachineEvaluation<T> {
    pub(crate) fn new(
        value: T,
        usage: EvaluationUsage,
        observations: EvaluationObservations,
        executed_root_bindings: Vec<ExecutedRootBinding>,
    ) -> Self {
        Self {
            measured: MeasuredEvaluation::new(value, usage),
            observations,
            executed_root_bindings,
        }
    }

    /// Lift a statically pure build-machine evaluation into the common build
    /// result without inventing a host observation.
    pub fn hermetic(measured: MeasuredEvaluation<T>) -> Self {
        Self {
            measured,
            observations: EvaluationObservations::default(),
            executed_root_bindings: Vec::new(),
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

    /// Executed declaration coordinates in the exact evaluated program.
    /// These are selection requests, not proof of target admission. Callers
    /// must rejoin them to that program and check lexical/product authority.
    /// A described binding additionally carries the exact compiler-issued
    /// `ProductEntryRef` payload it bound.
    pub fn executed_root_bindings(&self) -> &[ExecutedRootBinding] {
        &self.executed_root_bindings
    }

    pub fn into_value(self) -> T {
        self.measured.into_value()
    }

    pub fn into_parts(
        self,
    ) -> (
        T,
        EvaluationUsage,
        EvaluationObservations,
        Vec<ExecutedRootBinding>,
    ) {
        let (value, usage) = self.measured.into_parts();
        (value, usage, self.observations, self.executed_root_bindings)
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
    pub(crate) fn exited(
        exit_code: i32,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        usage: EvaluationUsage,
    ) -> Self {
        Self {
            exit_code,
            stdout,
            stderr,
            error: None,
            usage,
        }
    }

    pub(crate) fn error(
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
