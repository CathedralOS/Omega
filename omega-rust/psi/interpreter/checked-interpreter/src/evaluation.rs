//! Evaluator work, observed effects, and returned-result custody.

use crate::FilesystemGrantRootIdentity;

/// Versioned identity of the checked interpreter semantics used for
/// build-time result evaluation. This is independent of work accounting: a
/// semantic change must advance this marker even when the step schedule and
/// usage-record shape stay unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct EvaluationSemanticsIdentity(u32);

impl EvaluationSemanticsIdentity {
    pub const fn marker(self) -> u32 {
        self.0
    }
}

pub const CURRENT_EVALUATION_SEMANTICS: EvaluationSemanticsIdentity =
    EvaluationSemanticsIdentity(5);

/// Identity of the deterministic evaluator-step schedule used before the
/// canonical portable IR exists.
///
/// This is deliberately distinct from canonical-IR `FuelScheduleIdentity`.
/// Its marker names the current TypedTrees interpreter's accounting precursor and
/// must not be used as an IR-derived fixed-work certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
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

pub use filesystem_authority::{
    BuildOutputObligation, BuildOutputObligationState, BuildOutputReceipt,
    MAX_BUILD_OUTPUT_OBLIGATIONS,
};

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

/// A provider choice executed against the activation's original Build cell.
/// Its operands are declaration references, never evaluated product code.
/// Omega rejoins this coordinate to validate product scope and provider coverage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedBuildSelection {
    /// The typed Build operation that executed, independently of its operands.
    pub operation: language_semantics::declaration_selection::BuildOperation,
    pub machine: symbols::SymbolHandle,
    /// The authored call: stage 02 retains every provider-selection
    /// spelling, statement or discard included, as this one expression.
    pub expression: typed_trees::expression::ExpressionHandle,
    /// Zero means the omitted default; otherwise the evaluated toolchain case.
    pub composition_case: symbols::SymbolHandle,
}

/// One crash, physical-authority, or service exclusion selection that
/// actually evaluated against the activation's original Build value
/// (wiki/spec/build/behavior_exclusions.md): exclusions are evaluated Build
/// selections, so a call present in the static call scope but never executed
/// selects nothing. The coordinate rejoins the authored call in the exact
/// evaluated program; an enum case is the EVALUATED value's exact variant
/// symbol, not the spelled argument shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedBehaviorExclusion {
    /// The machine whose state body evaluated the exclusion call.
    pub machine: symbols::SymbolHandle,
    /// The executed call's coordinate in the evaluated program.
    pub site: ExecutedBehaviorExclusionSite,
    /// The evaluated selection.
    pub kind: ExecutedBehaviorExclusionKind,
}

/// Where an executed exclusion call sat in the evaluated program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutedBehaviorExclusionSite {
    /// A `StatementNode::Call` statement.
    Statement(typed_trees::statement::StatementHandle),
    /// An `ExpressionNode::Call` at any expression position.
    Expression(typed_trees::expression::ExpressionHandle),
}

/// The exclusion one evaluated call selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutedBehaviorExclusionKind {
    /// `Build::exclude_crash(builder, cause)`: the exact toolchain `CrashCause`
    /// variant symbol the evaluated argument carried. The argument is a
    /// value here — a bound local or a computed case selects as much as a
    /// literal spelling.
    CrashCause { case_symbol: symbols::SymbolHandle },
    /// The exact compiler-owned physical-authority case selected at run time.
    /// Omega admission maps this source identity to its physical vocabulary;
    /// the target-neutral evaluator does not classify mechanisms.
    PhysicalAuthorityClass { case_symbol: symbols::SymbolHandle },
    /// `Build::exclude_service<Trait>(builder)`: the call executed; the retained
    /// call node still carries the resolved trait identity admission
    /// validates.
    Service,
}

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

    /// The value and its usage, dropping the placement receipts.
    pub fn into_measured(self) -> MeasuredEvaluation<T> {
        self.measured
    }

    pub fn into_value(self) -> T {
        self.measured.into_value()
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
    /// A reason ordinary completion is unavailable. Crash-aware consumers use
    /// the typed `crash_cause`; other failures remain evaluator declines.
    pub error: Option<String>,
    /// The cause of an executed authored crash, never inferred from failure
    /// text or an internal evaluator trap. `error` remains populated so
    /// consumers expecting only ordinary completion stay fail-closed.
    pub crash_cause: Option<typed_trees::signature::CrashCause>,
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
            crash_cause: None,
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
            crash_cause: None,
            usage,
        }
    }

    pub(crate) fn crashed(
        cause: typed_trees::signature::CrashCause,
        site: String,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        usage: EvaluationUsage,
    ) -> Self {
        let mut outcome = Self::error(
            format!("explicit crash {cause:?} at {site}"),
            stdout,
            stderr,
            usage,
        );
        outcome.crash_cause = Some(cause);
        outcome
    }
}

/// Maximum generated sources included by one build evaluation.
pub const MAX_INCLUDED_BUILD_SOURCES: usize = 256;
