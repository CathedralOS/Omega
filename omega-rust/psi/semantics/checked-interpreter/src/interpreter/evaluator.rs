//! The checked interpreter's evaluator.
//!
//! `Evaluator` (this file) owns one run: the program, its frames (`Frame` is
//! one lexical scope), the step and depth budgets, and the virtual
//! filesystem state. `Halt` is the non-local control-flow signal every
//! method returns through. The evaluator's behaviour lives in its child
//! modules, grouped by concern:
//!
//! - program and values: `program_lookup`, `record_views`, `type_metadata`,
//!   `names_recasts_and_places`, `value_projections`, `array_windows`;
//! - execution: `execution` (states and transitions),
//!   `statements_and_calls`, `expressions_and_value_calls`,
//!   `scalar_operations`, `casts_and_recasts`, `numeric_landing`,
//!   `host_dispatch`, `boundary_adapter_dispatch`, `boundary_console`,
//!   `wire_codec`;
//! - build-machine facets: `build_log`, `build_paths`, `root_bindings`,
//!   `output_obligations`, `product_entries`, `product_providers`,
//!   `product_schemas`;
//! - the filesystem: `filesystem` (the virtual provider),
//!   `filesystem_host_operation`, `filesystem_logical_handles`,
//!   `filesystem_preparation`, `real_filesystem` and `host_open_flags`;
//! - shared helpers: `halts`, `directory_entries` and `scalar_numerics`.

use crate::build_evaluation_sponsor::BuildEvaluationLiveFilesystemHandleLease;
use crate::value::{Cell, CellMeter, TextByteMeter, Value};
use crate::{
    BuildEvaluationSponsor, EvaluationUsage, FilesystemEvaluationHaltKind, FilesystemGrantAccess,
    FilesystemGrantRefusal, FilesystemGrantRefusalReason, FilesystemGrantRootIdentity,
    FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInput,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemLogicalHandleOutput, FilesystemLogicalHandleOutputSource, FilesystemMetadataLayout,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, PrivateLayoutPlacementReceipt,
};
use checked_trees::{CheckedOperatorFacts, CheckedTrees};
use numerics::arithmetic::ArithmeticDomain;
use numerics::bignum::BigInt;
use numerics::float_semantics::{
    FloatClass as SemanticFloatClass, FloatFormat as SemanticFloatFormat, FloatMeaning,
    FloatPolicyTrap, FloatSemantics,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableNamePath, UnaryOperator,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{
    StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetNode,
};
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

// Program and values.
mod array_windows;
mod names_recasts_and_places;
mod program_lookup;
mod record_views;
mod type_metadata;
mod value_projections;

// Execution.
mod boundary_adapter_dispatch;
mod boundary_console;
mod casts_and_recasts;
mod execution;
mod expressions_and_value_calls;
mod host_dispatch;
mod numeric_landing;
mod scalar_operations;
mod statements_and_calls;
mod wire_codec;

// Build-machine facets.
mod behavior_exclusions;
mod build_log;
mod build_paths;
mod output_obligations;
mod product_entries;
mod product_providers;
mod product_schemas;
mod root_bindings;

// The filesystem.
mod filesystem;
mod filesystem_host_operation;
mod filesystem_logical_handles;
mod filesystem_preparation;
mod host_open_flags;
/// The REAL-filesystem provider (opt-in `FilesystemAccess::RealUnscoped`; the
/// build.omg rung). A CHILD module so it can serve ops against the private
/// `Evaluator` internals (the fs argument/buffer helpers) without widening
/// their visibility outside the interpreter owner.
pub(super) mod real_filesystem;

// Shared helpers.
mod directory_entries;
mod halts;
mod scalar_numerics;

use build_paths::{rooted_build_path_parts, validate_build_relative_path};
use directory_entries::{
    checked_directory_name_snapshot_total, checked_directory_record_snapshot_total,
    dirent_record_chunk, pack_dirent_records, portable_directory_entry_name,
};
use filesystem_host_operation::{FilesystemHostOperation, FilesystemHostResultKind};
use filesystem_logical_handles::FilesystemLogicalHandles;
use filesystem_preparation::{
    FIND_DATA_OUTPUT_BYTES, PreparedByteOutput, PreparedFilesystemCall,
    PreparedFilesystemLogicalHandleOutput, PreparedFilesystemLogicalHandlePlan,
    PreparedFilesystemMutableObservationPlan, PreparedFilesystemPreparation, STAT_OUTPUT_BYTES,
    synthetic_handle_fd,
};
pub(crate) use halts::Halt;
use halts::{EvalResult, filesystem_sponsor_halt, trap, unsupported};
use scalar_numerics::{
    apply_arithmetic_domain, big_integer_runtime_value, float_to_integer_trap_message,
    integer_bounds, integer_primitive_byte_width, interpreter_f32_from_bits,
    interpreter_f32_to_bits, is_unsigned_integer_primitive, primitive_bit_width,
    primitive_is_unsigned64, project_landed_float, semantic_integer_format, wrap_to_width,
};

pub(super) const STEP_BUDGET: u64 = 10_000_000;

pub(super) fn ambient_step_budget() -> u64 {
    std::env::var("OMEGA_INTERP_STEP_BUDGET")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(STEP_BUDGET)
}

/// Fuel cap for CONST EVALUATION (comptime stage 1). The language's
/// termination discipline (no general recursion, loops carry decreases) is the
/// real guarantee; this cap is defense-in-depth against checker gaps. Exceeding
/// it is a compile error at the const site.
pub(super) const CONST_EVAL_STEP_BUDGET: u64 = 100_000;
/// Max native recursion depth (call / cross-machine transition nesting) before we decline
/// rather than overflow the host stack. Deep recursive programs are skipped (reported as
/// unsupported), never crash the differential harness.
const CALL_DEPTH_BUDGET: u32 = 512;
/// Aggregate byte custody for immutable, path-like, rooted-resolution,
/// returned-path, and mutable filesystem evidence retained during one
/// evaluator run. A successful mutable byte call retains resolution, provider
/// pre-state, and provider post-state under this same sponsor. Individual
/// prepared carriers remain bounded by their separate 16 MiB evaluator limit.
const MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES: usize = 256 * 1024 * 1024;
/// Exact logical ceiling for one complete directory-enumeration payload lane:
/// packed records for `read_dir`, retained names for a find cursor. Packed
/// extent strictly dominates its source names. Allocator capacity and process
/// memory are deliberately outside the claim.
const MAX_DIRECTORY_SNAPSHOT_BYTES: usize = 16 * 1024 * 1024;
/// The portable std directory-entry contract retains at most 255 name bytes,
/// and its 512-byte read buffer must always admit at least one complete record.
const MAX_DIRECTORY_ENTRY_NAME_BYTES: usize = 255;

/// The modeled `st_mtime` (seconds since the Unix epoch) the hermetic virtual
/// filesystem reports for every entry â€” it has no real clock. A recognizable
/// round value (2001-09-09T01:46:40Z). Native `stat` returns the real time.
/// The canonical metadata observation constructor supplies distinct modeled
/// values for the remaining timestamps and identity/allocation fields.
const VIRTUAL_MTIME_SECS: i64 = 1_000_000_000;
/// The hermetic ownership mutation model uses the same fixed identities as the
/// canonical metadata observation.
const VIRTUAL_UID: u32 = 501;
const VIRTUAL_GID: u32 = 20;

#[derive(Clone)]
enum MutableScalarRecast {
    Direct {
        source: PrimitiveType,
        target: PrimitiveType,
    },
    ByteRegion {
        cells: Vec<Cell>,
        offset: usize,
        target: PrimitiveType,
    },
    AggregateByteRegion {
        cells: Vec<Cell>,
        offset: usize,
        target_type: TypeReferenceHandle,
    },
    AggregateTyped {
        source: Cell,
        source_type: TypeReferenceHandle,
        target_type: TypeReferenceHandle,
    },
}

impl MutableScalarRecast {
    fn target(&self) -> Option<PrimitiveType> {
        match self {
            Self::Direct { target, .. } | Self::ByteRegion { target, .. } => Some(*target),
            Self::AggregateByteRegion { .. } | Self::AggregateTyped { .. } => None,
        }
    }
}

#[derive(Clone)]
enum MutableRecordProjectionStep {
    Field(String),
    Index(ExpressionHandle),
}

#[derive(Clone, Copy)]
struct MutableRecordProjection {
    offset: usize,
    type_reference: TypeReferenceHandle,
    stored_integer: Option<typed_trees::PlanLaidIntegerField>,
}

/// A lexical scope: parameter / local bindings by name, plus the receiver (`self`) cell.
/// `locals` is behind a `RefCell` so `let` bindings can be added while the frame is
/// shared by `&` during statement execution.
pub(crate) struct Frame {
    return_type: TypeReferenceHandle,
    locals: RefCell<BTreeMap<String, Cell>>,
    type_locals: RefCell<BTreeMap<String, TypeReferenceHandle>>,
    /// DECLARED scalar (primitive, arithmetic-domain) of locals/params, recorded
    /// at binding -- the static type witness `Value::Int` alone cannot carry.
    /// Read for two classifications native derives from the same declared types:
    /// u64-classed names (`u64`/`usize`/`addr`) make comparisons UNSIGNED at
    /// width 8 (`Value::Int` cannot distinguish u64::MAX from -1), and
    /// Saturating/Trapping names make arithmetic NODES clamp/trap at the
    /// operation itself (native emits the saturating ADD; a landing-seam
    /// coercion alone cannot represent an expression whose own domain differs
    /// from its landing slot's).
    scalar_locals: RefCell<BTreeMap<String, (PrimitiveType, ArithmeticDomain)>>,
    /// Mutable recast locals retain either one equal-width scalar cell or an
    /// indexed byte region. The local remains a normal `Ref` for place
    /// resolution, while these descriptors preserve the stated scalar/record
    /// geometry at the observable read/write seams.
    mutable_scalar_recasts: RefCell<BTreeMap<String, MutableScalarRecast>>,
    self_cell: Cell,
    /// The machine whose state is currently executing. Lets a call/transition that names a
    /// SIBLING state resolve it within this machine (rather than re-entering the machine's
    /// entry state, which would recurse forever).
    machine_symbol: SymbolHandle,
    /// Exact state selected at entry; refreshed with the frame on every transition.
    state_symbol: SymbolHandle,
    /// Value-call results computed while evaluating THIS state pass's transition guards,
    /// keyed by call-expression handle. A transition subject is evaluated ONCE per
    /// transition evaluation: the parser lowers `transition self.f(x) { true -> a
    /// false -> b }` into one guard per arm, each holding a COPY of the subject call, so
    /// a later arm must reuse the first arm's result (matching the native lowering)
    /// instead of re-running the callee's side effects. Copies have distinct handles, so
    /// lookups compare structurally. The frame is rebuilt for every state (re)entry, so
    /// loops re-evaluate naturally. Destructure-marked case payload projections
    /// reuse this subject too; separately authored successor calls do not.
    guard_call_results: RefCell<Vec<(ExpressionHandle, Value)>>,
}

/// One open descriptor in the interpreter's virtual filesystem: which path it
/// refers to, the read/write cursor, and whether it was opened writable.
struct VirtualFd {
    path: Vec<u8>,
    /// Open-file-description cursor shared by every descriptor produced by
    /// `duplicate`, matching POSIX `dup`/Rust `File::try_clone` semantics.
    cursor: std::rc::Rc<std::cell::Cell<usize>>,
    writable: bool,
    /// A descriptor over a DIRECTORY (opened read-only for `read_dir`); a normal
    /// `read`/`write` on it is EISDIR.
    is_dir: bool,
}

pub(crate) struct Evaluator<'program> {
    program: &'program TypedTrees,
    /// Full-program interpretation retains checked named-operator evidence so
    /// a root-preserving intrinsic rewrite can still report the source
    /// operation. Const/build-time evaluation runs before that evidence exists.
    operator_facts: Option<&'program CheckedOperatorFacts>,
    boundary_adapter_dispatch: &'program [checked_trees::CheckedBoundaryAdapterDispatch],
    pub(super) selected_build_time_operators: &'program [crate::SelectedBuildTimeBinaryOperator],
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    /// Exact output emitted through the compiler-owned `Build.log` facet.
    /// This stays distinct from runtime Console output so build evidence
    /// cannot accidentally attribute an ordinary boundary call to BuildLog.
    pub(super) build_log: Vec<u8>,
    /// The activation's original Build cell, never an authored copy.
    root_build: Option<Cell>,
    pub(super) executed_root_bindings: Vec<crate::ExecutedRootBinding>,
    /// Executed `exclude_crash`/`exclude_service` selections on the root
    /// Build, in evaluation order.
    pub(super) executed_behavior_exclusions: Vec<crate::ExecutedBehaviorExclusion>,
    /// Compiler-issued product-entry descriptions handed to evaluated code as
    /// opaque `ProductEntryRef` markers. The marker value indexes this table;
    /// evaluated code can copy the marker but cannot read or fabricate the
    /// selection payload.
    product_entry_descriptions: Vec<crate::DescribedProductEntry>,
    /// Compiler-issued product-type-schema descriptions handed to evaluated
    /// code as opaque `ProductTypeSchema` markers. Same marker discipline as
    /// the entry table: the marker indexes this table and evaluated code
    /// cannot read or fabricate the payload; `schema.path()` is the single
    /// sanctioned inspection.
    product_schema_descriptions: Vec<crate::DescribedProductSchema>,
    /// Compiler-issued product-provider descriptions handed to evaluated code
    /// as opaque `ProductProviderRef` markers. Same marker discipline as the
    /// schema table: the marker indexes this table and evaluated code cannot
    /// read or fabricate the payload; `provider.path()` is the single
    /// sanctioned inspection.
    product_provider_descriptions: Vec<crate::DescribedProductProvider>,
    stdin: &'program [u8],
    stdin_cursor: usize,
    /// Virtual monotonic tick counter for `Clock.tick_count` (advances on every
    /// read and every `sleep`); deterministic, so tick-based programs must
    /// assert monotonicity rather than concrete values.
    virtual_ticks: i64,
    /// The virtual window system: `window_create` mints opaque non-zero handle
    /// tokens; `is_window` reports membership; `window_destroy` removes.
    /// Deterministic, so programs must branch on liveness (handle != 0,
    /// is_window > 0), never on concrete handle values.
    virtual_live_windows: std::collections::HashSet<i64>,
    virtual_window_next: i64,
    /// A deterministic in-memory filesystem for `std::fs` programs: no real
    /// disk, so the differential oracle stays reproducible (mirrors the other
    /// `virtual_*` subsystems). `virtual_files` maps a path's bytes to its
    /// content bytes; `virtual_fds` maps an open descriptor to its cursor +
    /// writability. Descriptors start at 3 â€” 0/1/2 are the standard streams and
    /// are never minted as `File` handles.
    virtual_files: BTreeMap<Vec<u8>, Vec<u8>>,
    virtual_fds: BTreeMap<i32, VirtualFd>,
    virtual_next_fd: i32,
    /// Directories in the virtual filesystem (create_dir/remove_dir).
    virtual_dirs: std::collections::BTreeSet<Vec<u8>>,
    /// Exact checked physical carrier selected for canonical metadata results.
    /// Omega supplies this closed descriptor; Psi never receives target names
    /// or programmable-layout source.
    pub(super) filesystem_metadata_layout: FilesystemMetadataLayout,
    /// Exact package-owned boundary selected by Omega consumer policy. `None`
    /// retains standalone bundled-std compatibility only.
    pub(super) filesystem_service_symbol: Option<SymbolHandle>,
    /// Open find-enumeration cursors (`find_first`/`find_next`/`find_close`,
    /// the windows dir-walk seam ops, fs rung 3a): handle -> the REMAINING
    /// entries (name bytes, is_dir), snapshotted at `find_first` exactly like
    /// a Win32 find handle. Handles start at 1 (-1 is INVALID_HANDLE_VALUE).
    virtual_finds: BTreeMap<i64, std::collections::VecDeque<(Vec<u8>, bool)>>,
    virtual_next_find: i64,
    /// Explicitly-set permission bits per path (`set_permissions`/chmod). A path
    /// absent from this map is treated as writable (the default); only a path
    /// chmod'd to drop the owner-write bit (mode & 0o200 == 0) makes a write-open
    /// fail with EACCES â€” enough to model `set_permissions` without tracking a
    /// mode for every created file.
    virtual_perms: BTreeMap<Vec<u8>, u32>,
    /// Symbolic links: link path -> target bytes (`symlink`/`read_link`). The
    /// hermetic model stores and returns targets but does NOT resolve them on
    /// open/stat (see TASKS_FS.md); native symlinks resolve for real.
    virtual_symlinks: BTreeMap<Vec<u8>, Vec<u8>>,
    /// Explicitly-set modification times: path -> mtime seconds (`set_file_times`
    /// / `File::set_times`). `stat`/`fstat` report this when present, else the fixed
    /// modeled epoch. The hermetic model round-trips MODIFIED time (whole seconds);
    /// access time is set natively but the model reports the fixed modeled atime.
    virtual_times: BTreeMap<Vec<u8>, i64>,
    /// Advisory whole-file locks (`flock` / Rust `File::lock`/`unlock`): path ->
    /// the fd that holds an EXCLUSIVE lock. A non-blocking acquire on a path
    /// another fd already holds returns EWOULDBLOCK; a lock is released by
    /// LOCK_UN or by closing the owning fd. Shared-lock coexistence and real
    /// blocking are documented approximations (a single-threaded run can't
    /// exercise them); exclusive contention is what the model tracks.
    virtual_flocks: BTreeMap<Vec<u8>, i32>,
    /// Character-special device files (`/dev/null` etc.): paths that `stat` reports
    /// with an `S_IFCHR` mode instead of a regular file, so `FileType`/
    /// `FileTypeExt::is_char_device()` resolves the same on both engines. The
    /// hermetic FS has no real device nodes; this seeds the common ones so a
    /// differential test can `metadata("/dev/null")` without special-casing.
    virtual_char_devices: std::collections::BTreeSet<Vec<u8>>,
    /// The thread-local `errno` model: set to a POSIX code when a virtual fs op
    /// fails (ENOENT=2, EACCES=13, EEXIST=17, EBADF=9), read back by
    /// `read_errno` (darwin `___error()`). Mirrors the native seam so the typed
    /// error model (`io::ErrorKind`) resolves identically on both engines.
    virtual_errno: i32,
    /// `Some` iff the run was started with `FilesystemAccess::RealUnscoped`
    /// (build.omg rung 1): every filesystem op is served against the REAL host
    /// filesystem instead of the virtual model above. The default (`None`)
    /// keeps the interpreter hermetic -- the differential oracle never touches
    /// real disk.
    pub(super) real_fs: Option<real_filesystem::RealFs>,
    /// Expected compiler-produced events for bounded no-host filesystem replay.
    pub(super) filesystem_replay: Option<crate::FilesystemReplay>,
    /// The canonical Build activation carried Source/Output facets. In this
    /// mode path-taking host operations require interpreter-retained rooted
    /// provenance; bare byte spellings cannot select a grant root.
    rooted_build_paths_required: bool,
    /// Explicit Output-rooted source coordinates recorded by the exact
    /// toolchain handoff machine. Orchestration validates these against its
    /// captured sponsored tree before using any bytes.
    pub(super) build_included_sources: Vec<crate::BuildIncludedSource>,
    /// Compiler-issued required-output obligations in `BuildOutput::require`
    /// issue order. Evaluated code holds only an opaque marker carrying the
    /// row index; the declared name and settlement state stay here.
    pub(super) output_obligations: Vec<crate::BuildOutputObligation>,
    /// Compiler-issued completion receipts in `BuildOutput::complete` issue
    /// order. The `OutputReceipt` marker value carries only this row's index.
    pub(super) output_receipts: Vec<crate::BuildOutputReceipt>,
    /// `(root, relative)` -> obligation index for every issued obligation.
    /// Registration is permanent for the activation: a failed or completed
    /// name still collides with a later `require`.
    pub(super) output_obligation_paths: BTreeMap<(FilesystemGrantRootIdentity, Vec<u8>), usize>,
    /// Per-path output-file custody observed from completed filesystem
    /// attempts: a rooted path is `Open` while any writer descriptor remains
    /// live and `Sealed` once its last writer retires. `complete` binds only
    /// a `Sealed` name.
    pub(super) output_seal_states:
        BTreeMap<(FilesystemGrantRootIdentity, Vec<u8>), output_obligations::OutputSealState>,
    /// Live descriptor identities bound to a rooted path by a successful
    /// create/open output in this run. Retirement seals the path once its
    /// last writer is gone.
    pub(super) output_open_writers:
        BTreeMap<FilesystemLogicalHandleIdentity, (FilesystemGrantRootIdentity, Vec<u8>)>,
    /// Set whenever a host-boundary call is driven (statement position or the
    /// value-call fallback). The build-time evaluation entry rejects runs that
    /// touched the host: a dynamic backstop behind decision 12's static gate.
    host_boundary_touched: bool,
    /// Like `host_boundary_touched` but EXCLUDING the filesystem family: the
    /// GRANTED build entry (`evaluate_granted_build_machine_arguments`) allows
    /// fs ops (the grant is the audit surface, open-work #3's settled design)
    /// while still rejecting every OTHER host boundary (console, clock, gui)
    /// as its dynamic backstop.
    non_fs_host_boundary_touched: bool,
    /// Ordered operation-attempt evidence for exact canonical filesystem host
    /// calls. Direct scoped path authorizations retain compiler-rooted paths;
    /// typed operands, mutable carriers, and logical handles retain their
    /// completed preparation prefix. Exact path results and file/directory
    /// observation regions and canonical metadata values are designated, but
    /// replay execution remains incomplete.
    pub(super) filesystem_operation_attempts: Vec<FilesystemOperationAttempt>,
    /// Compiler-only normalization state for provider descriptor/handle tokens.
    /// This state is not observable by evaluated Omega code.
    filesystem_logical_handles: FilesystemLogicalHandles,
    /// Active compiler-owned package-build reservations keyed by the logical
    /// resource identity that owns them. Borrowed native views have no entry.
    filesystem_live_handle_leases:
        BTreeMap<crate::FilesystemLogicalHandleIdentity, BuildEvaluationLiveFilesystemHandleLease>,
    /// Aggregate retained authorized rooted-path bytes.
    filesystem_observation_path_bytes: usize,
    /// Aggregate retained immutable, path-like, rooted-resolution,
    /// returned-path, and mutable evidence bytes, including resolution and
    /// provider pre/post copies. Observed-byte regions reference post-state and
    /// add no byte copy. This compiler-side account is not observable by Omega.
    filesystem_observation_evidence_bytes: usize,
    /// Pending non-catchable halt set when retaining a successfully authorized
    /// rooted path would exceed the compiler's evidence-custody bound.
    filesystem_observation_resource_halt: Option<String>,
    /// Stack of call-start indices used to attach nested provider-side facts to
    /// the exact active operation attempt.
    filesystem_operation_attempt_stack: Vec<usize>,
    /// Compiler-only receipts from executed `Plan::place_private` calls. The
    /// values are returned beside build-time evaluation and never enter the
    /// interpreted store.
    pub(super) private_layout_placements: Vec<PrivateLayoutPlacementReceipt>,
    pub(super) usage: EvaluationUsage,
    /// Exact allocation-lifetime meter for semantic interpreter cells.
    cell_meter: CellMeter,
    /// Exact logical-byte lifetime meter for interpreter Text backing buffers.
    text_byte_meter: TextByteMeter,
    /// Optional compiler-owned account shared across a complete build-review
    /// session. This measures deterministic compiler-owned build resources.
    pub(super) build_evaluation_sponsor: Option<BuildEvaluationSponsor>,
    /// Total step allowance for this run. Full-program interpretation uses
    /// `STEP_BUDGET`; const evaluation uses the much smaller
    /// `CONST_EVAL_STEP_BUDGET` as a defense-in-depth step ceiling.
    step_budget: u64,
    call_depth: u32,
    /// Non-zero while evaluating a transition GUARD expression. Value-calls evaluated
    /// under a guard memoize into the frame's `guard_call_results` so the per-arm
    /// copies of one transition subject evaluate the callee once (see `Frame`).
    guard_depth: u32,
}

#[derive(Clone)]
struct EvaluatedArgument {
    cell: Cell,
    mutable_recast: Option<MutableScalarRecast>,
}

impl EvaluatedArgument {
    fn plain(cell: Cell) -> Self {
        Self {
            cell,
            mutable_recast: None,
        }
    }
}

enum TransitionDecision<'program> {
    Terminal,
    SelfTarget,
    Value(Value),
    Named {
        state: &'program State,
        machine: &'program Machine,
        instance: Cell,
        args: Vec<EvaluatedArgument>,
    },
}

// `Frame::locals` needs interior mutability so `let` bindings can be added while the
// frame is shared by `&`. Wrap the map in a RefCell.
impl Frame {
    fn get(&self, name: &str) -> Option<Cell> {
        self.locals_ref().borrow().get(name).cloned()
    }

    fn bind(&self, name: &str, cell: Cell) {
        self.locals_ref().borrow_mut().insert(name.to_owned(), cell);
    }

    fn bind_type(&self, name: &str, type_reference: TypeReferenceHandle) {
        self.type_locals
            .borrow_mut()
            .insert(name.to_owned(), type_reference);
    }

    fn locals_ref(&self) -> &RefCell<BTreeMap<String, Cell>> {
        &self.locals
    }
}
