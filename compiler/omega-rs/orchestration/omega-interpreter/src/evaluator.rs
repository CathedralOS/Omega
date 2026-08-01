use crate::{
    EvaluationUsage, FilesystemAccess, InterpretOptions, InterpretOutcome, MeasuredEvaluation,
};

/// The REAL-filesystem provider (opt-in `FilesystemAccess::RealUnscoped`; the
/// build.omg rung). A CHILD module so it can serve ops against the private
/// `Evaluator` internals (the fs argument/buffer helpers) without widening
/// their visibility; `#[path]` keeps the flat one-file-per-module layout.
#[path = "evaluator_real_fs.rs"]
mod real_fs;

/// Per-target open-flag BIT POSITIONS, mirroring the checked target encoders in
/// `std/targets/<target>/filesystem_impl.omg`. The differential oracle compiles
/// for `host()` and runs ON the host, so selecting by `cfg!(target_os)` needs no
/// target threading. The create/open differential canaries guard this mirror.
/// Access mode (O_WRONLY 1 / O_RDWR 2, mask 0x3) is universal.
mod host_open_flags {
    #[cfg(target_os = "windows")]
    pub const O_CREAT_BIT: i32 = 8;
    #[cfg(target_os = "windows")]
    pub const O_EXCL_BIT: i32 = 10;
    #[cfg(target_os = "windows")]
    pub const O_TRUNC_BIT: i32 = 9;
    #[cfg(target_os = "windows")]
    pub const O_APPEND_BIT: i32 = 3;

    #[cfg(target_os = "macos")]
    pub const O_CREAT_BIT: i32 = 9;
    #[cfg(target_os = "macos")]
    pub const O_EXCL_BIT: i32 = 11;
    #[cfg(target_os = "macos")]
    pub const O_TRUNC_BIT: i32 = 10;
    #[cfg(target_os = "macos")]
    pub const O_APPEND_BIT: i32 = 3;

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    pub const O_CREAT_BIT: i32 = 6;
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    pub const O_EXCL_BIT: i32 = 7;
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    pub const O_TRUNC_BIT: i32 = 9;
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    pub const O_APPEND_BIT: i32 = 10;

    pub const fn o_creat(flags: i32) -> bool {
        (flags >> O_CREAT_BIT) & 1 != 0
    }
    pub const fn o_excl(flags: i32) -> bool {
        (flags >> O_EXCL_BIT) & 1 != 0
    }
    pub const fn o_trunc(flags: i32) -> bool {
        (flags >> O_TRUNC_BIT) & 1 != 0
    }
    pub const fn o_append(flags: i32) -> bool {
        (flags >> O_APPEND_BIT) & 1 != 0
    }
}
use crate::value::{Cell, Value};
use omega_checked_trees::{CheckedOperatorFacts, CheckedTrees};
use omega_core::arithmetic::ArithmeticDomain;
use omega_core::bignum::BigInt;
use omega_core::float_semantics::{
    FloatClass as SemanticFloatClass, FloatFormat as SemanticFloatFormat, FloatMeaning,
    FloatPolicyTrap, FloatSemantics, FloatToIntegerError, IntegerFormat as SemanticIntegerFormat,
};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::{DataDefinition, DataMember};
use omega_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableNamePath, UnaryOperator,
};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetNode,
};
use omega_typed_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

const STEP_BUDGET: u64 = 10_000_000;
/// Fuel cap for CONST EVALUATION (comptime stage 1). The language's
/// termination discipline (no general recursion, loops carry decreases) is the
/// real guarantee; this cap is defense-in-depth against checker gaps. Exceeding
/// it is a compile error at the const site.
const CONST_EVAL_STEP_BUDGET: u64 = 100_000;
/// Max native recursion depth (call / cross-machine transition nesting) before we decline
/// rather than overflow the host stack. Deep recursive programs are skipped (reported as
/// unsupported), never crash the differential harness.
const CALL_DEPTH_BUDGET: u32 = 512;

/// The modeled `st_mtime` (seconds since the Unix epoch) the hermetic virtual
/// filesystem reports for every entry — it has no real clock. A recognizable
/// round value (2001-09-09T01:46:40Z). Native `stat` returns the real time.
/// The accessed/created times are offset to DISTINCT modeled values so a test
/// can confirm each `st_*time` field is decoded from its own stat offset:
/// created (birthtime) <= modified <= accessed, as is realistic.
const VIRTUAL_MTIME_SECS: i64 = 1_000_000_000;
const VIRTUAL_ATIME_SECS: i64 = 1_000_000_100;
const VIRTUAL_BIRTHTIME_SECS: i64 = 999_999_900;
/// Change time (`st_ctime`): metadata-change time, distinct from the others so a
/// decode-offset bug is caught. Realistically birthtime <= ctime ~ mtime.
const VIRTUAL_CTIME_SECS: i64 = 1_000_000_050;
/// Device id (`st_dev`): fixed non-zero modeled value. Native returns the real
/// device; tests assert this constant in the interpreter and only that two files
/// on the same FS share a device natively.
const VIRTUAL_DEV: u64 = 16_777_220;
/// Allocation fields (`st_blocks` = 512-byte block count, `st_blksize` = preferred
/// I/O block size): the hermetic FS reports fixed modeled values. Native `stat`
/// returns the real allocation; tests assert these constants in the interpreter and
/// only `blksize > 0` natively (it is filesystem-dependent).
const VIRTUAL_BLOCKS: u64 = 8;
const VIRTUAL_BLKSIZE: u64 = 4096;
/// The hermetic FS reports FIXED identity/ownership fields (`st_ino`/`st_uid`/
/// `st_gid`): it has no real inodes or process identity. Native `stat` returns the
/// real values; tests assert these exact constants in the interpreter and only the
/// deterministic relationships (two hard links share an inode; two files share an
/// owner) natively.
const VIRTUAL_INO: u64 = 1_000_000;
const VIRTUAL_UID: u32 = 501;
const VIRTUAL_GID: u32 = 20;

/// Byte offsets at which the hermetic FS lays out a `struct stat` for the HOST
/// target, mirroring the selected target's checked `StatLayout` policy. A
/// program compiled for `host()` runs here, so the differential canary guards
/// this Rust carrier against the `.omg` layout policy.
///
/// Non-windows hosts use the darwin/POSIX layout (every field has a real home).
/// Windows uses the msvcrt `_stat64` layout (56 bytes); the fields absent or
/// width-mismatched in `_stat64` (ino/uid/gid, the status-CHANGE time, blocks,
/// blksize) live in a SYNTHETIC TAIL (>=64) that the interpreter fills but a real
/// native `_stat64` leaves zero -- so native windows reports 0 for those.
mod host_stat_offsets {
    #[cfg(not(target_os = "windows"))]
    pub const DEV: usize = 0;
    #[cfg(not(target_os = "windows"))]
    pub const MODE: usize = 4;
    #[cfg(not(target_os = "windows"))]
    pub const NLINK: usize = 6;
    #[cfg(not(target_os = "windows"))]
    pub const INO: usize = 8;
    #[cfg(not(target_os = "windows"))]
    pub const UID: usize = 16;
    #[cfg(not(target_os = "windows"))]
    pub const GID: usize = 20;
    #[cfg(not(target_os = "windows"))]
    pub const ATIME: usize = 32;
    #[cfg(not(target_os = "windows"))]
    pub const MTIME: usize = 48;
    #[cfg(not(target_os = "windows"))]
    pub const CTIME: usize = 64;
    #[cfg(not(target_os = "windows"))]
    pub const BTIME: usize = 80;
    #[cfg(not(target_os = "windows"))]
    pub const SIZE: usize = 96;
    #[cfg(not(target_os = "windows"))]
    pub const BLOCKS: usize = 104;
    #[cfg(not(target_os = "windows"))]
    pub const BLKSIZE: usize = 112;

    // msvcrt `_stat64` real fields (0..55)
    #[cfg(target_os = "windows")]
    pub const DEV: usize = 0;
    #[cfg(target_os = "windows")]
    pub const MODE: usize = 6;
    #[cfg(target_os = "windows")]
    pub const NLINK: usize = 8;
    #[cfg(target_os = "windows")]
    pub const ATIME: usize = 32;
    #[cfg(target_os = "windows")]
    pub const MTIME: usize = 40;
    #[cfg(target_os = "windows")]
    pub const BTIME: usize = 48; // windows st_ctime == creation time
    #[cfg(target_os = "windows")]
    pub const SIZE: usize = 24;
    // synthetic tail (native `_stat64` leaves these zero)
    #[cfg(target_os = "windows")]
    pub const INO: usize = 64;
    #[cfg(target_os = "windows")]
    pub const UID: usize = 72;
    #[cfg(target_os = "windows")]
    pub const GID: usize = 76;
    #[cfg(target_os = "windows")]
    pub const CTIME: usize = 80; // no change time on windows -> synthetic
    #[cfg(target_os = "windows")]
    pub const BLOCKS: usize = 88;
    #[cfg(target_os = "windows")]
    pub const BLKSIZE: usize = 96;
}

pub(crate) fn run(checked: &CheckedTrees, stdin: &[u8]) -> InterpretOutcome {
    run_with_options(checked, stdin, InterpretOptions::default())
}

pub(crate) fn run_with_options(
    checked: &CheckedTrees,
    stdin: &[u8],
    options: InterpretOptions,
) -> InterpretOutcome {
    // Run on a worker thread with a generous stack: the tree-walker recurses with the
    // program's call/expression nesting, which can exceed the default test-thread stack on
    // deep programs even with the call-depth budget. A scoped thread lets us keep the
    // borrow of `checked`/`stdin`.
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || run_on_current_thread(checked, stdin, options))
            .expect("spawn interpreter worker thread")
            .join()
            .unwrap_or_else(|_| {
                InterpretOutcome::error(
                    "interpreter thread panicked",
                    Vec::new(),
                    Vec::new(),
                    EvaluationUsage::empty(),
                )
            })
    })
}

/// BUILD-TIME EVALUATION (stage 1): run a zero-argument, effect-free
/// machine to its terminal value and return that value as an `i64`, width-
/// adjusted to the machine's declared integer return type (the same
/// `wrap_to_width` the interpreter applies on writes, so the result is
/// TARGET-width-correct, not host-width). The caller (the compiler's
/// const-eval pass) owns the purity gate; this entry owns evaluation and a
/// small evaluator-step ceiling. Errors carry a human-readable reason for the
/// compile diagnostic at the const site.
pub(crate) fn run_const_machine(
    program: &TypedTrees,
    machine_name: &str,
) -> Result<MeasuredEvaluation<i64>, String> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || {
                run_const_machine_on_current_thread(program, machine_name)
            })
            .expect("spawn const-eval worker thread")
            .join()
            .unwrap_or_else(|_| Err("const evaluator thread panicked".to_owned()))
    })
}

fn run_const_machine_on_current_thread(
    program: &TypedTrees,
    machine_name: &str,
) -> Result<MeasuredEvaluation<i64>, String> {
    let mut evaluator = Evaluator::new(program, &[]);
    evaluator.step_budget = CONST_EVAL_STEP_BUDGET;
    let result = evaluator.run_const_machine(machine_name);
    let usage = evaluator.usage;
    match result {
        Ok(value) => Ok(MeasuredEvaluation::new(value, usage)),
        Err(Halt::Exit(code)) => Err(format!(
            "the machine attempted to exit the process (code {code}) instead of returning a value"
        )),
        Err(Halt::Unsupported(message)) | Err(Halt::Trap(message)) => Err(message),
    }
}

/// STRUCTURED build-time evaluation (the R2 layouts enabler): run an
/// effect-free machine with compiler-built ARGUMENTS and read back its
/// terminal value as a structured tree. Same ownership split as
/// `run_const_machine`: the caller owns the purity gate (decision 12's
/// transitive effect surface), this entry owns evaluation + the evaluator-step
/// ceiling.
pub(crate) fn run_build_time_machine(
    program: &TypedTrees,
    machine_name: &str,
    arguments: Vec<crate::build_time::BuildTimeValue>,
) -> Result<MeasuredEvaluation<crate::build_time::BuildTimeValue>, String> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || {
                let mut evaluator = Evaluator::new(program, &[]);
                evaluator.step_budget = CONST_EVAL_STEP_BUDGET;
                let result = evaluator.run_build_time_machine(machine_name, arguments);
                let usage = evaluator.usage;
                match result {
                    Ok(value) => Ok(MeasuredEvaluation::new(value, usage)),
                    Err(Halt::Exit(code)) => Err(format!(
                        "the machine attempted to exit the process (code {code}) instead of returning a value"
                    )),
                    Err(Halt::Unsupported(message)) | Err(Halt::Trap(message)) => Err(message),
                }
            })
            .expect("spawn build-time evaluation worker thread")
            .join()
            .unwrap_or_else(|_| Err("build-time evaluator thread panicked".to_owned()))
    })
}

/// The AUGMENTING-MACHINE build-time entry (build_and_package_model.md):
/// evaluate `machine_name` with the given arguments and read back the FINAL
/// argument values -- the `machine build(b: &mut Build)` shape, where the
/// machine augments a passed-in value and returns nothing. The terminal value
/// (if any) is discarded; a unit machine is fine.
pub(crate) fn run_build_time_machine_arguments(
    program: &TypedTrees,
    machine_name: &str,
    arguments: Vec<crate::build_time::BuildTimeValue>,
) -> Result<MeasuredEvaluation<Vec<crate::build_time::BuildTimeValue>>, String> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || {
                let mut evaluator = Evaluator::new(program, &[]);
                evaluator.step_budget = CONST_EVAL_STEP_BUDGET;
                let result = evaluator.run_build_time_machine_arguments(machine_name, arguments);
                let usage = evaluator.usage;
                match result {
                    Ok(values) => Ok(MeasuredEvaluation::new(values, usage)),
                    Err(Halt::Exit(code)) => Err(format!(
                        "the machine attempted to exit the process (code {code}) instead of returning"
                    )),
                    Err(Halt::Unsupported(message)) | Err(Halt::Trap(message)) => Err(message),
                }
            })
            .expect("spawn build-time evaluation worker thread")
            .join()
            .unwrap_or_else(|_| Err("build-time evaluator thread panicked".to_owned()))
    })
}

/// The GRANTED build entry (open-work #3 rung 4, interpreter side): run the
/// augmenting `build(b: &mut Build)` machine WITH a filesystem capability --
/// virtual (hermetic tests) or real scoped/unscoped per `options` -- and read
/// back the augmented arguments. Filesystem ops are allowed (the grant is the
/// audit surface); any OTHER host boundary (console, clock, gui) rejects.
/// Runs under the FULL step budget: staging assets is real work, unlike the
/// const-eval step ceiling the pure entry rides.
pub(crate) fn run_granted_build_machine_arguments(
    program: &TypedTrees,
    machine_name: &str,
    arguments: Vec<crate::build_time::BuildTimeValue>,
    options: InterpretOptions,
) -> Result<MeasuredEvaluation<Vec<crate::build_time::BuildTimeValue>>, String> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, move || {
                let mut evaluator = Evaluator::new(program, &[]);
                match options.filesystem {
                    FilesystemAccess::Virtual => {}
                    FilesystemAccess::RealUnscoped => {
                        evaluator.real_fs = Some(real_fs::RealFs::new(None));
                    }
                    FilesystemAccess::RealScoped(grants) => {
                        evaluator.real_fs = Some(real_fs::RealFs::new(Some(grants)));
                    }
                }
                let result = evaluator.run_build_machine_arguments_with_policy(
                    machine_name,
                    arguments,
                    true,
                );
                // Build logging reaches the REAL streams (owner answer #5:
                // "the interpreter should never just catch it") -- including
                // on failure, where the partial log is the diagnostic.
                use std::io::Write as _;
                if !evaluator.stdout.is_empty() {
                    let _ = std::io::stdout().write_all(&evaluator.stdout);
                    let _ = std::io::stdout().flush();
                }
                if !evaluator.stderr.is_empty() {
                    let _ = std::io::stderr().write_all(&evaluator.stderr);
                    let _ = std::io::stderr().flush();
                }
                let usage = evaluator.usage;
                match result {
                    Ok(values) => Ok(MeasuredEvaluation::new(values, usage)),
                    Err(Halt::Exit(code)) => Err(format!(
                        "the machine attempted to exit the process (code {code}) instead of returning"
                    )),
                    Err(Halt::Unsupported(message)) | Err(Halt::Trap(message)) => Err(message),
                }
            })
            .expect("spawn granted build evaluation worker thread")
            .join()
            .unwrap_or_else(|_| Err("granted build evaluator thread panicked".to_owned()))
    })
}

fn run_on_current_thread(
    checked: &CheckedTrees,
    stdin: &[u8],
    options: InterpretOptions,
) -> InterpretOutcome {
    let mut evaluator = Evaluator::new_checked(checked, stdin);
    match options.filesystem {
        FilesystemAccess::Virtual => {}
        FilesystemAccess::RealUnscoped => {
            evaluator.real_fs = Some(real_fs::RealFs::new(None));
        }
        FilesystemAccess::RealScoped(grants) => {
            evaluator.real_fs = Some(real_fs::RealFs::new(Some(grants)));
        }
    }
    let result = evaluator.run_entry();
    let usage = evaluator.usage;
    match result {
        Ok(()) => {
            // Reached a terminal transition without an explicit exit_process.
            InterpretOutcome::exited(0, evaluator.stdout, evaluator.stderr, usage)
        }
        Err(Halt::Exit(code)) => {
            InterpretOutcome::exited(code, evaluator.stdout, evaluator.stderr, usage)
        }
        Err(Halt::Unsupported(message)) | Err(Halt::Trap(message)) => {
            InterpretOutcome::error(message, evaluator.stdout, evaluator.stderr, usage)
        }
    }
}

/// A non-local control-flow signal. `Exit` halts cleanly with a code; the others abort
/// the run and surface as `InterpretOutcome.error` (so a harness skips rather than
/// reports a false mismatch).
enum Halt {
    Exit(i32),
    Unsupported(String),
    Trap(String),
}

type EvalResult<T> = Result<T, Halt>;

/// Pack `(name, d_type)` entries as darwin `dirent` records, the layout native
/// `___getdirentries64` returns (reclen u16 @16, namlen u16 @18, d_type u8
/// @20, name @21, records 8-byte aligned) -- so a parser is identical on both
/// engines. Shared by the virtual fs (`build_dirent_records`) and the real-fs
/// provider (`try_real_filesystem_call`'s `read_dir`), which differ only in
/// where the names come from.
fn pack_dirent_records(entries: &[(Vec<u8>, u8)]) -> Vec<u8> {
    let mut buffer = Vec::new();
    for (name, d_type) in entries {
        let namlen = name.len();
        let reclen = (25 + namlen).div_ceil(8) * 8;
        let start = buffer.len();
        buffer.resize(start + reclen, 0);
        buffer[start + 16..start + 18].copy_from_slice(&(reclen as u16).to_le_bytes());
        buffer[start + 18..start + 20].copy_from_slice(&(namlen as u16).to_le_bytes());
        buffer[start + 20] = *d_type;
        buffer[start + 21..start + 21 + namlen].copy_from_slice(name);
    }
    buffer
}

fn unsupported<T>(message: impl Into<String>) -> EvalResult<T> {
    Err(Halt::Unsupported(message.into()))
}

fn trap<T>(message: impl Into<String>) -> EvalResult<T> {
    Err(Halt::Trap(message.into()))
}

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

/// A lexical scope: parameter / local bindings by name, plus the receiver (`self`) cell.
/// `locals` is behind a `RefCell` so `let` bindings can be added while the frame is
/// shared by `&` during statement execution.
struct Frame {
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
    /// Value-call results computed while evaluating THIS state pass's transition guards,
    /// keyed by call-expression handle. A transition subject is evaluated ONCE per
    /// transition evaluation: the parser lowers `transition self.f(x) { true -> a
    /// false -> b }` into one guard per arm, each holding a COPY of the subject call, so
    /// a later arm must reuse the first arm's result (matching the native lowering)
    /// instead of re-running the callee's side effects. Copies have distinct handles, so
    /// lookups compare structurally. The frame is rebuilt for every state (re)entry, so
    /// loops re-evaluate naturally.
    guard_call_results: RefCell<Vec<(ExpressionHandle, Value)>>,
}

/// One open descriptor in the interpreter's virtual filesystem: which path it
/// refers to, the read/write cursor, and whether it was opened writable.
struct VirtualFd {
    path: Vec<u8>,
    cursor: usize,
    writable: bool,
    /// A descriptor over a DIRECTORY (opened read-only for `read_dir`); a normal
    /// `read`/`write` on it is EISDIR.
    is_dir: bool,
}

struct Evaluator<'program> {
    program: &'program TypedTrees,
    /// Full-program interpretation retains checked named-operator evidence so
    /// a root-preserving intrinsic rewrite can still report the source
    /// operation. Const/build-time evaluation runs before that evidence exists.
    operator_facts: Option<&'program CheckedOperatorFacts>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
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
    /// writability. Descriptors start at 3 — 0/1/2 are the standard streams and
    /// are never minted as `File` handles.
    virtual_files: BTreeMap<Vec<u8>, Vec<u8>>,
    virtual_fds: BTreeMap<i32, VirtualFd>,
    virtual_next_fd: i32,
    /// Directories in the virtual filesystem (create_dir/remove_dir).
    virtual_dirs: std::collections::BTreeSet<Vec<u8>>,
    /// Open find-enumeration cursors (`find_first`/`find_next`/`find_close`,
    /// the windows dir-walk seam ops, fs rung 3a): handle -> the REMAINING
    /// entries (name bytes, is_dir), snapshotted at `find_first` exactly like
    /// a Win32 find handle. Handles start at 1 (-1 is INVALID_HANDLE_VALUE).
    virtual_finds: BTreeMap<i64, std::collections::VecDeque<(Vec<u8>, bool)>>,
    virtual_next_find: i64,
    /// Explicitly-set permission bits per path (`set_permissions`/chmod). A path
    /// absent from this map is treated as writable (the default); only a path
    /// chmod'd to drop the owner-write bit (mode & 0o200 == 0) makes a write-open
    /// fail with EACCES — enough to model `set_permissions` without tracking a
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
    real_fs: Option<real_fs::RealFs>,
    /// Set whenever a host-boundary call is driven (statement position or the
    /// value-call fallback). The build-time evaluation entry rejects runs that
    /// touched the host: a dynamic backstop behind decision 12's static gate.
    host_boundary_touched: bool,
    /// Like `host_boundary_touched` but EXCLUDING the filesystem family: the
    /// GRANTED build entry (`evaluate_build_machine_with_filesystem`) allows
    /// fs ops (the grant is the audit surface, open-work #3's settled design)
    /// while still rejecting every OTHER host boundary (console, clock, gui)
    /// as its dynamic backstop.
    non_fs_host_boundary_touched: bool,
    usage: EvaluationUsage,
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

impl<'program> Evaluator<'program> {
    fn new(program: &'program TypedTrees, stdin: &'program [u8]) -> Self {
        Self {
            program,
            operator_facts: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            stdin,
            stdin_cursor: 0,
            virtual_ticks: 0,
            virtual_live_windows: std::collections::HashSet::new(),
            virtual_window_next: 0,
            virtual_files: BTreeMap::new(),
            virtual_fds: BTreeMap::new(),
            virtual_next_fd: 3,
            virtual_dirs: std::collections::BTreeSet::new(),
            virtual_finds: BTreeMap::new(),
            virtual_next_find: 1,
            virtual_perms: BTreeMap::new(),
            virtual_symlinks: BTreeMap::new(),
            virtual_times: BTreeMap::new(),
            virtual_flocks: BTreeMap::new(),
            virtual_char_devices: [b"/dev/null".to_vec(), b"/dev/zero".to_vec()]
                .into_iter()
                .collect(),
            virtual_errno: 0,
            real_fs: None,
            host_boundary_touched: false,
            non_fs_host_boundary_touched: false,
            usage: EvaluationUsage::empty(),
            // OMEGA_INTERP_STEP_BUDGET overrides the default for
            // measurement / long-running sample runs (dev knob, same
            // convention as the OMEGA_DEBUG_* flags); unset = the default.
            step_budget: std::env::var("OMEGA_INTERP_STEP_BUDGET")
                .ok()
                .and_then(|raw| raw.parse().ok())
                .unwrap_or(STEP_BUDGET),
            call_depth: 0,
            guard_depth: 0,
        }
    }

    fn new_checked(checked: &'program CheckedTrees, stdin: &'program [u8]) -> Self {
        let mut evaluator = Self::new(&checked.typed, stdin);
        evaluator.operator_facts = Some(&checked.facts.operators);
        evaluator
    }

    fn tick(&mut self) -> EvalResult<()> {
        self.usage
            .charge_step()
            .ok_or_else(|| Halt::Trap("evaluator usage overflowed".to_owned()))?;
        if self.usage.fuel_units() > self.step_budget {
            return trap("step budget exceeded");
        }
        Ok(())
    }

    // ---- entry --------------------------------------------------------------

    fn run_entry(&mut self) -> EvalResult<()> {
        let entry_machine = self
            .find_machine_by_name("Main::main")
            .or_else(|| self.find_machine_by_name("main"))
            .ok_or_else(|| Halt::Unsupported("no entry machine `Main::main`".to_owned()))?;
        let entry_state_name = if self.find_state(entry_machine, "main").is_some() {
            "main"
        } else {
            "entry"
        };

        let instance = self.instantiate_machine(entry_machine)?;
        // The entry machine's value (its terminal `Value` transition / final expression)
        // becomes the process exit code when it has no explicit `exit_process`. Mirrors the
        // backend: `machine Main::main(...) -> i32` returns the exit status.
        let returned =
            self.run_state_collect(entry_machine, entry_state_name, instance, Vec::new())?;
        if let Some(value) = returned {
            if let Some(code) = value.as_int() {
                return Err(Halt::Exit(code as i32));
            }
        }
        Ok(())
    }

    /// CONST EVALUATION: run `machine_name` (zero arguments, fresh default
    /// instance) to its terminal value. The machine's declared integer return
    /// type fixes the result's width semantics via `wrap_to_width` (target
    /// widths, never host widths). Non-integer terminal values are errors.
    fn run_const_machine(&mut self, machine_name: &str) -> EvalResult<i64> {
        let machine = self
            .find_machine_by_name(machine_name)
            .ok_or_else(|| Halt::Trap(format!("no machine named `{machine_name}` exists")))?
            .clone();
        let entry_state_name = self.machine_entry_state_name(&machine).ok_or_else(|| {
            Halt::Trap(format!(
                "machine `{machine_name}` has no states to evaluate"
            ))
        })?;
        // The declared INTEGER return type fixes the result's width semantics
        // (checked before running so the diagnostic names the type, not
        // whatever value the body happened to produce).
        let return_primitive = match self
            .find_state(&machine, &entry_state_name)
            .and_then(|state| self.program.primitive_type_reference(state.return_type))
        {
            Some(primitive)
                if primitive != PrimitiveType::Bool
                    && primitive != PrimitiveType::F32
                    && primitive != PrimitiveType::F64 =>
            {
                primitive
            }
            Some(primitive) => {
                return Err(Halt::Trap(format!(
                    "machine `{machine_name}` returns `{}`, not an integer type",
                    primitive.name()
                )));
            }
            None => {
                return Err(Halt::Trap(format!(
                    "machine `{machine_name}` does not declare an integer return type"
                )));
            }
        };

        let instance = self.instantiate_machine(&machine)?;
        let returned = self.run_state_collect(&machine, &entry_state_name, instance, Vec::new())?;
        let value = returned.ok_or_else(|| {
            Halt::Trap(format!(
                "machine `{machine_name}` terminated without producing a value"
            ))
        })?;
        let raw = value.as_int().ok_or_else(|| {
            Halt::Trap(format!(
                "machine `{machine_name}` produced a non-integer value"
            ))
        })?;

        Ok(wrap_to_width(raw, return_primitive))
    }

    /// STRUCTURED build-time evaluation: bind compiler-built arguments to the
    /// machine's entry-state parameters positionally, run to the terminal
    /// value, and deep-read it back out. Argument-count mismatch is a clear
    /// error here (the position's diagnostic names the machine); the caller
    /// owns the purity gate.
    fn run_build_time_machine(
        &mut self,
        machine_name: &str,
        arguments: Vec<crate::build_time::BuildTimeValue>,
    ) -> EvalResult<crate::build_time::BuildTimeValue> {
        let machine = self
            .find_machine_by_name(machine_name)
            .ok_or_else(|| Halt::Trap(format!("no machine named `{machine_name}` exists")))?
            .clone();
        let entry_state_name = self.machine_entry_state_name(&machine).ok_or_else(|| {
            Halt::Trap(format!(
                "machine `{machine_name}` has no states to evaluate"
            ))
        })?;
        // The `&self` receiver is bound from the machine instance, not the
        // argument list -- exclude it from the positional count.
        let parameter_count = self
            .find_state(&machine, &entry_state_name)
            .map(|state| {
                self.program
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| parameter.name.as_str() != "self")
                    .count()
            })
            .unwrap_or(0);
        if parameter_count != arguments.len() {
            return Err(Halt::Trap(format!(
                "machine `{machine_name}` takes {parameter_count} argument(s); the build-time \
                 position supplied {}",
                arguments.len()
            )));
        }

        let instance = self.instantiate_machine(&machine)?;
        let argument_cells = arguments
            .into_iter()
            .map(|argument| EvaluatedArgument::plain(argument.into_value().cell()))
            .collect();
        let returned =
            self.run_state_collect(&machine, &entry_state_name, instance, argument_cells)?;
        let value = returned.ok_or_else(|| {
            Halt::Trap(format!(
                "machine `{machine_name}` terminated without producing a value"
            ))
        })?;
        // The dynamic purity backstop: the static effect surface does not yet
        // fold host-authority audit facts (boundary-trait calls), so any run
        // that actually touched the host is rejected here.
        if self.host_boundary_touched {
            return Err(Halt::Trap(format!(
                "machine `{machine_name}` is not effect-free: it drove a host-boundary call \
                 during build-time evaluation"
            )));
        }
        Ok(crate::build_time::BuildTimeValue::from_value(&value))
    }

    /// The augmenting-machine variant: run and read back the FINAL argument
    /// values (a `&mut` parameter aliases its argument cell, so mutations land
    /// there). A unit terminal is accepted -- the machine's OUTPUT is its
    /// arguments.
    fn run_build_time_machine_arguments(
        &mut self,
        machine_name: &str,
        arguments: Vec<crate::build_time::BuildTimeValue>,
    ) -> EvalResult<Vec<crate::build_time::BuildTimeValue>> {
        self.run_build_machine_arguments_with_policy(machine_name, arguments, false)
    }

    /// The shared augmenting-machine runner. `allow_filesystem` selects the
    /// dynamic backstop: `false` = the PURE build-time entry (any host touch
    /// rejects -- decision 12's discipline); `true` = the GRANTED build entry
    /// (filesystem ops are the point; every OTHER host boundary rejects).
    fn run_build_machine_arguments_with_policy(
        &mut self,
        machine_name: &str,
        arguments: Vec<crate::build_time::BuildTimeValue>,
        allow_filesystem: bool,
    ) -> EvalResult<Vec<crate::build_time::BuildTimeValue>> {
        let machine = self
            .find_machine_by_name(machine_name)
            .ok_or_else(|| Halt::Trap(format!("no machine named `{machine_name}` exists")))?
            .clone();
        let entry_state_name = self.machine_entry_state_name(&machine).ok_or_else(|| {
            Halt::Trap(format!(
                "machine `{machine_name}` has no states to evaluate"
            ))
        })?;
        let parameter_count = self
            .find_state(&machine, &entry_state_name)
            .map(|state| {
                self.program
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| parameter.name.as_str() != "self")
                    .count()
            })
            .unwrap_or(0);
        if parameter_count != arguments.len() {
            return Err(Halt::Trap(format!(
                "machine `{machine_name}` takes {parameter_count} argument(s); the build-time position supplied {}",
                arguments.len()
            )));
        }

        let instance = self.instantiate_machine(&machine)?;
        let argument_cells: Vec<Cell> = arguments
            .into_iter()
            .map(|argument| argument.into_value().cell())
            .collect();
        // Keep the cells: a `&mut` parameter aliases its cell, so the run's
        // mutations are visible here afterward.
        let kept: Vec<Cell> = argument_cells.clone();
        let evaluated_arguments = argument_cells
            .into_iter()
            .map(EvaluatedArgument::plain)
            .collect();
        let _terminal =
            self.run_state_collect(&machine, &entry_state_name, instance, evaluated_arguments)?;
        let impure = if allow_filesystem {
            self.non_fs_host_boundary_touched
        } else {
            self.host_boundary_touched
        };
        if impure {
            return Err(Halt::Trap(if allow_filesystem {
                format!(
                    "machine `{machine_name}` drove a NON-filesystem host-boundary call during \
                     granted build evaluation -- only the Filesystem capability is granted"
                )
            } else {
                format!(
                    "machine `{machine_name}` is not effect-free: it drove a host-boundary call during build-time evaluation"
                )
            }));
        }
        Ok(kept
            .iter()
            .map(|cell| crate::build_time::BuildTimeValue::from_value(&cell.borrow()))
            .collect())
    }

    // ---- machine / data instantiation --------------------------------------

    /// Build a machine instance as a `Struct` whose fields are the attached data's
    /// fields (with their defaults) plus the machine's contained sub-objects.
    fn instantiate_machine(&mut self, machine: &Machine) -> EvalResult<Cell> {
        let mut fields: BTreeMap<String, Cell> = BTreeMap::new();

        if let Some(data_name) = machine.attached_data.as_ref() {
            if let Some(data) = self.find_data_by_name(data_name.as_str()) {
                self.populate_data_fields(data, &mut fields)?;
            }
        }

        // Machine-owned data (the `owned_data` span) are additional named cells.
        for owned in self.program.machine_owned_data(machine) {
            let value = if owned.initial_value.is_valid() {
                let frame = Frame {
                    locals: RefCell::new(BTreeMap::new()),
                    type_locals: RefCell::new(BTreeMap::new()),
                    self_cell: Value::Unit.cell(),
                    machine_symbol: SymbolHandle::invalid(),
                    scalar_locals: RefCell::new(BTreeMap::new()),
                    mutable_scalar_recasts: RefCell::new(BTreeMap::new()),
                    guard_call_results: RefCell::new(Vec::new()),
                };
                self.eval_expression(owned.initial_value, &frame)?
            } else {
                self.default_value_for_type(owned.type_reference)?
            };
            fields.insert(owned.name.as_str().to_owned(), value.cell());
        }

        Ok(Value::Struct {
            type_symbol: machine.symbol,
            type_name: machine.name.as_str().to_owned(),
            fields,
        }
        .cell())
    }

    /// Insert a `data` definition's fields (with defaults) into `fields`. Nested `data`
    /// members recurse so their own defaults are populated.
    fn populate_data_fields(
        &mut self,
        data: &DataDefinition,
        fields: &mut BTreeMap<String, Cell>,
    ) -> EvalResult<()> {
        self.populate_data_fields_with_bindings(data, fields, &[])
    }

    fn populate_data_fields_with_bindings(
        &mut self,
        data: &DataDefinition,
        fields: &mut BTreeMap<String, Cell>,
        bindings: &[(
            SymbolHandle,
            String,
            omega_typed_trees::types::TypeReferenceHandle,
        )],
    ) -> EvalResult<()> {
        let members = self.program.data_members(data).to_vec();
        for member in &members {
            let DataMember::Field(field) = member else {
                continue;
            };
            let name = field.name.as_str().to_owned();
            // Field defaults are retired: every field ZII zero-initializes.
            let value =
                self.default_value_for_type_with_bindings(field.type_reference, bindings)?;
            fields.insert(name, value.cell());
        }
        Ok(())
    }

    /// Build a default-initialized value for a declared type, recursing into nested `data`
    /// records (a sub-Struct with its own defaults) and fixed arrays (an `Array` of
    /// per-element default cells). Falls back to the primitive/unit default.
    fn default_value_for_type(
        &mut self,
        type_reference: omega_typed_trees::types::TypeReferenceHandle,
    ) -> EvalResult<Value> {
        self.default_value_for_type_with_bindings(type_reference, &[])
    }

    fn default_value_for_type_with_bindings(
        &mut self,
        type_reference: omega_typed_trees::types::TypeReferenceHandle,
        bindings: &[(
            SymbolHandle,
            String,
            omega_typed_trees::types::TypeReferenceHandle,
        )],
    ) -> EvalResult<Value> {
        if type_reference.is_valid() {
            if let omega_typed_trees::types::TypeReferenceNode::Named { symbol, name } = self
                .program
                .type_reference_table
                .type_reference(type_reference)
                && let Some(argument) =
                    self.generic_binding_argument(*symbol, name.as_str(), bindings)
            {
                return self.default_value_for_type_with_bindings(argument, bindings);
            }

            // An owned `[u8; N] in <text-domain>` is not an always-full fixed
            // array. Its runtime representation is the bounded carrier
            // `{ len, bytes[N] }`, whose honest ZII value has `len == 0`.
            // Recognize that shape BEFORE stripping the domain constraint;
            // otherwise the interpreter constructs N zero-valued array
            // elements and compares it against text literals as a non-text
            // value, diverging from both the checked semantics and native
            // `BoundedByteBuffer` layout.
            if self.declared_type_is_bounded_byte_buffer(type_reference) {
                return Ok(Value::bytes(Vec::new()));
            }

            // See THROUGH a domain constraint (`[i32; N] in Wrapping`, `i32 in Saturating`):
            // the default of a constrained type is the default of its base type (zero in every
            // arithmetic domain). Without this, a domain-constrained ARRAY field falls past the
            // FixedArray case below and defaults to `Unit`, so a later `self.arr[i]` raised
            // "cannot index Unit" and the whole canary was SKIPPED by the differential oracle.
            if let omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } = self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                let base_type = *base_type;
                return self.default_value_for_type_with_bindings(base_type, bindings);
            }

            // Fixed array `[T; N]` -> N default-initialized element cells.
            if let omega_typed_trees::types::TypeReferenceNode::FixedArray {
                element_type,
                length,
            } = self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                let element_type = *element_type;
                let count = match length {
                    omega_typed_trees::types::FixedArrayLength::Literal(count) => Some(*count),
                    omega_typed_trees::types::FixedArrayLength::ConstParameter { symbol, name } => {
                        self.generic_binding_argument(*symbol, name.as_str(), bindings)
                            .and_then(|argument| {
                                self.const_argument_value_with_bindings(argument, bindings, 0)
                            })
                    }
                    omega_typed_trees::types::FixedArrayLength::ConstCall { .. } => None,
                };
                if let Some(count) = count {
                    let mut elements = Vec::with_capacity(count);
                    for _ in 0..count {
                        elements.push(
                            self.default_value_for_type_with_bindings(element_type, bindings)?
                                .cell(),
                        );
                    }
                    return Ok(Value::Array(elements));
                }
            }

            if let omega_typed_trees::types::TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments,
                ..
            } = self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                let definition = self
                    .program
                    .data_definitions()
                    .iter()
                    .find(|data| {
                        (base_symbol.is_valid() && data.symbol == *base_symbol)
                            || data.name.as_str() == base_name.as_str()
                    })
                    .cloned();
                if let Some(definition) = definition
                    && matches!(
                        DataDefinition::shape_kind_from_members(
                            self.program.data_members(&definition)
                        ),
                        omega_typed_trees::data::DataShapeKind::Record
                    )
                {
                    let parameters = self.program.data_type_parameters(&definition).to_vec();
                    let arguments = self
                        .program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .to_vec();
                    let mut nested_bindings = bindings.to_vec();
                    nested_bindings.extend(parameters.iter().zip(arguments).map(
                        |(parameter, argument)| {
                            (
                                parameter.symbol,
                                parameter.name.as_str().to_owned(),
                                argument,
                            )
                        },
                    ));
                    let mut nested_fields = BTreeMap::new();
                    self.populate_data_fields_with_bindings(
                        &definition,
                        &mut nested_fields,
                        &nested_bindings,
                    )?;
                    return Ok(Value::Struct {
                        type_symbol: definition.symbol,
                        type_name: definition.name.as_str().to_owned(),
                        fields: nested_fields,
                    });
                }
            }

            // Nested `data` record -> a sub-Struct of its own defaults.
            if let Some(nested) = self.field_nested_data(type_reference) {
                let mut nested_fields = BTreeMap::new();
                self.populate_data_fields(nested, &mut nested_fields)?;
                return Ok(Value::Struct {
                    type_symbol: nested.symbol,
                    type_name: nested.name.as_str().to_owned(),
                    fields: nested_fields,
                });
            }

            // An enum-shaped field defaults to the ZERO CASE (ZII: tag 0 is
            // the first case) with the case's payload fields zeroed --
            // matching native zero-initialized storage, so tag compares and
            // synthesized structural equality agree on never-assigned sum
            // fields instead of seeing a Unit placeholder.
            if let Some((type_symbol, variant_name, payload_fields)) =
                self.enum_zero_case(type_reference)
            {
                let mut payload = Vec::with_capacity(payload_fields.len());
                for field in payload_fields {
                    let value =
                        self.default_value_for_type_with_bindings(field.type_reference, bindings)?;
                    payload.push((field.name.as_str().to_owned(), value.cell()));
                }
                return Ok(Value::Enum {
                    type_symbol,
                    variant_name,
                    payload,
                });
            }
        }
        Ok(self.default_for_type(type_reference))
    }

    fn generic_binding_argument(
        &self,
        symbol: SymbolHandle,
        name: &str,
        bindings: &[(
            SymbolHandle,
            String,
            omega_typed_trees::types::TypeReferenceHandle,
        )],
    ) -> Option<omega_typed_trees::types::TypeReferenceHandle> {
        bindings
            .iter()
            .find(|(parameter, spelling, _)| {
                if symbol.is_valid() {
                    *parameter == symbol
                } else {
                    spelling == name
                }
            })
            .map(|(_, _, argument)| *argument)
    }

    fn const_argument_value_with_bindings(
        &self,
        argument: omega_typed_trees::types::TypeReferenceHandle,
        bindings: &[(
            SymbolHandle,
            String,
            omega_typed_trees::types::TypeReferenceHandle,
        )],
        depth: usize,
    ) -> Option<usize> {
        if depth >= 16 {
            return None;
        }
        let omega_typed_trees::types::TypeReferenceNode::Named { symbol, name } =
            self.program.type_reference_table.type_reference(argument)
        else {
            return None;
        };
        if !symbol.is_valid() {
            if let Ok(value) = name.as_str().parse::<usize>() {
                return Some(value);
            }
        }
        let argument = self.generic_binding_argument(*symbol, name.as_str(), bindings)?;
        self.const_argument_value_with_bindings(argument, bindings, depth + 1)
    }

    /// The first case of a case-bearing declared type (the ZII zero case),
    /// with the field declarations a zeroed value carries: the COMMON fields
    /// (mixed shapes -- present in every case) followed by the zero case's
    /// payload fields.
    fn enum_zero_case(
        &self,
        type_reference: omega_typed_trees::types::TypeReferenceHandle,
    ) -> Option<(
        SymbolHandle,
        String,
        Vec<omega_typed_trees::data::DataField>,
    )> {
        if self
            .program
            .primitive_type_reference(type_reference)
            .is_some()
        {
            return None;
        }
        let symbol = self.program.type_reference_symbol(type_reference);
        if !symbol.is_valid() {
            return None;
        }
        let data = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == symbol)?;
        let members = self.program.data_members(data);
        let first_variant = members.iter().find_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            _ => None,
        })?;
        let mut fields: Vec<omega_typed_trees::data::DataField> = members
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) => Some(field.clone()),
                _ => None,
            })
            .collect();
        fields.extend(self.program.data_payload_fields(first_variant).to_vec());
        Some((data.symbol, first_variant.name.as_str().to_owned(), fields))
    }

    /// If a field's declared type is a (non-primitive) `data` record, return it.
    fn field_nested_data(
        &self,
        type_reference: omega_typed_trees::types::TypeReferenceHandle,
    ) -> Option<&'program DataDefinition> {
        if !type_reference.is_valid() {
            return None;
        }
        if self
            .program
            .primitive_type_reference(type_reference)
            .is_some()
        {
            return None;
        }
        let symbol = self.program.type_reference_symbol(type_reference);
        if !symbol.is_valid() {
            return None;
        }
        self.program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == symbol)
            .filter(|data| {
                // Records instantiate as nested structs; case-bearing data (sums
                // AND mixed shapes) doesn't -- it zero-initializes to the first
                // case via `enum_zero_case`. EMPTY records (e.g. `data Circle {}`)
                // must still instantiate as a typed struct -- the type identity
                // is what a `dyn Trait` receiver dispatches on at runtime.
                !matches!(
                    DataDefinition::shape_kind_from_members(self.program.data_members(data)),
                    omega_typed_trees::data::DataShapeKind::Enum
                        | omega_typed_trees::data::DataShapeKind::Mixed
                )
            })
    }

    fn default_for_type(
        &self,
        type_reference: omega_typed_trees::types::TypeReferenceHandle,
    ) -> Value {
        match self.program.primitive_type_reference(type_reference) {
            Some(PrimitiveType::Bool) => Value::Bool(false),
            Some(PrimitiveType::F32) | Some(PrimitiveType::F64) => Value::Float(0.0),
            Some(_) => Value::Int(0),
            // A `&[u8] in Utf8` text view (the encoding-domain model that retires
            // the retired owned primitive, #66) uses a fat `{ptr,len}` descriptor
            // and the content-compare/literal-store path natively. The
            // zero-initialized field is a zeroed descriptor (empty bytes), so the
            // interpreter must default it to an EMPTY `Str` -- not `Unit`. (A `Unit`
            // default makes `self.name == "literal"` fall through `values_equal`'s
            // int-compare arm where `None == None` is spuriously TRUE, diverging from
            // the native empty-vs-nonempty content compare.)
            None if self.program.is_borrowed_byte_slice(type_reference) => {
                Value::str(String::new())
            }
            None => Value::Unit,
        }
    }

    // ---- state execution ----------------------------------------------------

    /// Run a state; returns the value produced by a `Value` transition target, if any.
    /// Guards native recursion depth so a deeply recursive program is declined (skipped)
    /// instead of overflowing the host stack.
    fn run_state_collect(
        &mut self,
        machine: &Machine,
        state_name: &str,
        instance: Cell,
        args: Vec<EvaluatedArgument>,
    ) -> EvalResult<Option<Value>> {
        self.call_depth += 1;
        if self.call_depth > CALL_DEPTH_BUDGET {
            self.call_depth -= 1;
            return unsupported("recursion depth budget exceeded");
        }
        let result = self.run_state_collect_inner(machine, state_name, instance, args);
        self.call_depth -= 1;
        result
    }

    fn run_state_collect_inner(
        &mut self,
        machine: &Machine,
        state_name: &str,
        instance: Cell,
        args: Vec<EvaluatedArgument>,
    ) -> EvalResult<Option<Value>> {
        // MR4 admission: the cross-machine tail transition REBINDS these and
        // continues the loop (a jump, mirroring the native dispatch-loop
        // lowering) instead of recursing -- an admitted measured mutual
        // cycle must not consume interpreter call depth.
        let mut machine = machine.clone();
        let mut instance = instance;
        let mut current_state = state_name.to_owned();
        let mut current_args = args;
        // Locals accumulated across SAME-machine sibling transitions: the backend models a
        // machine as one frame whose slots persist, so an inlined sub-state still sees the
        // enclosing state's params/`let`s (e.g. `mark_current_room` reading `enter_room`'s
        // `room_index`). New args bind on top; carried-over names stay visible.
        let mut carried: BTreeMap<String, Cell> = BTreeMap::new();
        let mut carried_types: BTreeMap<String, TypeReferenceHandle> = BTreeMap::new();
        let mut carried_recasts: BTreeMap<String, MutableScalarRecast> = BTreeMap::new();

        loop {
            self.tick()?;
            let state = self
                .find_state(&machine, &current_state)
                .ok_or_else(|| Halt::Unsupported(format!("unknown state `{current_state}`")))?
                .clone();

            let frame = self.bind_frame(
                &state,
                Rc::clone(&instance),
                &current_args,
                machine.symbol,
                &carried,
                &carried_types,
                &carried_recasts,
            )?;

            // Execute statements, watching for the first satisfied transition. A state
            // whose body ends in a bare expression (`{ 22 }`) returns that expression's
            // value as its result (the backend's value-state form).
            let mut next: Option<TransitionDecision> = None;
            let mut tail_value: Option<Value> = None;
            for statement in self
                .program
                .statement_table
                .statements(state.statement_nodes)
            {
                let statement = statement.clone();
                match &statement {
                    StatementNode::Transition(transition) => {
                        if let Some(decision) = self.eval_transition(transition, &frame)? {
                            next = Some(decision);
                            break;
                        }
                    }
                    StatementNode::Expression(expression) => {
                        tail_value = Some(self.eval_expression(*expression, &frame)?);
                    }
                    other => {
                        self.exec_statement(other, &frame)?;
                    }
                }
            }

            match next {
                None => return Ok(tail_value),
                Some(TransitionDecision::Value(value)) => return Ok(Some(value)),
                Some(TransitionDecision::Terminal) => return Ok(None),
                Some(TransitionDecision::SelfTarget) => {
                    // Re-run the same state (rare; guard against infinite loops via budget),
                    // carrying its bindings forward.
                    carried = frame.locals.into_inner();
                    carried_types = frame.type_locals.into_inner();
                    carried_recasts = frame.mutable_scalar_recasts.into_inner();
                    continue;
                }
                Some(TransitionDecision::Named {
                    state_name,
                    machine: target_machine,
                    instance: target_instance,
                    args,
                }) => {
                    if target_machine.symbol == machine.symbol
                        && Rc::ptr_eq(&target_instance, &instance)
                    {
                        // Carry this state's bindings forward to the sibling state.
                        carried = frame.locals.into_inner();
                        carried_types = frame.type_locals.into_inner();
                        carried_recasts = frame.mutable_scalar_recasts.into_inner();
                        current_state = state_name;
                        current_args = args;
                        continue;
                    }
                    // Cross-machine named transition: a TAIL JUMP into the
                    // target machine (the arm target is the arm's last
                    // action; whichever machine terminates delivers the
                    // value). Rebind the loop -- constant depth, matching
                    // the native SetDispatchState lowering. The carried
                    // locals clear: the callee binds a fresh frame.
                    machine = target_machine;
                    instance = target_instance;
                    current_state = state_name;
                    current_args = args;
                    carried = BTreeMap::new();
                    carried_types = BTreeMap::new();
                    carried_recasts = BTreeMap::new();
                    continue;
                }
            }
        }
    }

    /// Bind a state's parameters (skipping `self`) to the positional argument cells. Seeds
    /// from `carried` (the enclosing same-machine state's bindings) so an inlined sub-state
    /// still sees outer params/locals; the state's own params override on top.
    fn bind_frame(
        &self,
        state: &State,
        self_cell: Cell,
        args: &[EvaluatedArgument],
        machine_symbol: SymbolHandle,
        carried: &BTreeMap<String, Cell>,
        carried_types: &BTreeMap<String, TypeReferenceHandle>,
        carried_recasts: &BTreeMap<String, MutableScalarRecast>,
    ) -> EvalResult<Frame> {
        let mut scalar_locals = BTreeMap::new();
        let mut locals = carried.clone();
        let mut type_locals = carried_types.clone();
        let mut mutable_scalar_recasts = carried_recasts.clone();
        let mut arg_index = 0;
        for parameter in self.program.state_parameters(state) {
            if parameter.is_self {
                continue;
            }
            let argument = args
                .get(arg_index)
                .cloned()
                .unwrap_or_else(|| EvaluatedArgument::plain(Value::Unit.cell()));
            let cell = argument.cell;
            // Coerce a by-value ARGUMENT to the param's declared width/domain at
            // the binding, matching the native truncating/clamping/trapping store
            // at the call boundary. Mirrors the Assignment/LocalData store wraps:
            //   * f32 param: round a `Float` to f32 (an inline `+1.0` arg is f64;
            //     native passes it in an f32 register).
            //   * integer param: wrap/clamp/trap an `Int` to the param's width +
            //     arithmetic domain (a u8 param given `a+b`=300 must read 44).
            // A `&mut` arg carries a `Ref`/place (not a `Float`/`Int`), so it is
            // left untouched and its aliasing preserved (keep the original cell);
            // a by-value scalar is a copy anyway, so a fresh coerced cell is
            // correct. Funnels through `coerce_scalar_with` like every other seam.
            // The resolved (primitive, domain) is also RECORDED so arithmetic on
            // the param applies its declared domain at the operation node.
            let cell = match self
                .program
                .primitive_type_reference(parameter.type_reference)
            {
                Some(primitive) => {
                    let domain = self
                        .program
                        .arithmetic_domain_for_type_reference(parameter.type_reference);
                    scalar_locals.insert(parameter.name.as_str().to_owned(), (primitive, domain));
                    let scalar = match &*cell.borrow() {
                        v @ (Value::Int(_) | Value::Float(_)) => Some(v.clone()),
                        _ => None,
                    };
                    match scalar {
                        Some(value) => self.coerce_scalar_with(value, primitive, domain)?.cell(),
                        None => cell,
                    }
                }
                None => cell,
            };
            locals.insert(parameter.name.as_str().to_owned(), cell);
            type_locals.insert(parameter.name.as_str().to_owned(), parameter.type_reference);
            if let Some(recast) = argument.mutable_recast {
                mutable_scalar_recasts.insert(parameter.name.as_str().to_owned(), recast);
            }
            arg_index += 1;
        }
        Ok(Frame {
            locals: RefCell::new(locals),
            type_locals: RefCell::new(type_locals),
            self_cell,
            machine_symbol,
            scalar_locals: RefCell::new(scalar_locals),
            mutable_scalar_recasts: RefCell::new(mutable_scalar_recasts),
            guard_call_results: RefCell::new(Vec::new()),
        })
    }

    // ---- statements ---------------------------------------------------------

    fn exec_statement(&mut self, statement: &StatementNode, frame: &Frame) -> EvalResult<()> {
        self.tick()?;
        match statement {
            // Assembly facts are compile-time assertions and have no runtime
            // evaluation in either interpreter or native execution.
            StatementNode::AssemblyFact(_) => Ok(()),
            StatementNode::Assignment(assignment) => {
                // Atomic RMW source syntax is carried as an opaque expression so
                // native instruction selection can replace the whole assignment
                // with one instruction. The interpreter executes serially, but it
                // must preserve the same observable contract: the result local is
                // the value observed by that RMW, not a separate earlier read.
                // Seed the compiler-authored result place from the target before
                // evaluating the arithmetic-shaped single-threaded model.
                if let ExpressionNode::Atomic(atomic) = self
                    .program
                    .expression_table
                    .expression(assignment.value)
                    .clone()
                    && matches!(
                        atomic.ordering,
                        omega_core::atomic::AtomicOrderingPlan::ReadModifyWrite(_)
                            | omega_core::atomic::AtomicOrderingPlan::Swap(_)
                            | omega_core::atomic::AtomicOrderingPlan::CompareExchange { .. }
                    )
                {
                    if !atomic.result.is_valid() {
                        return Err(Halt::Trap(
                            "atomic RMW carrier lost its result place".to_owned(),
                        ));
                    }
                    let target = self.resolve_place(assignment.target, frame)?;
                    let target = self.deref_cell(target);
                    let prior = target.borrow().clone();
                    let result = self.resolve_place(atomic.result, frame)?;
                    let result = self.deref_cell(result);
                    *result.borrow_mut() = prior;
                }
                // A STRUCT, or a whole owned ARRAY, assignment is a VALUE copy: deep-clone so
                // mutating the destination later does not alias the source (`self.f =
                // self.arr[1]; self.f.x = 50` must not touch arr[1]; `self.b = self.a;
                // self.b[0] = 9` must not touch a). A `Value::Array` is deep-cloned ONLY when the
                // TARGET's declared type is an owned `[T; N]` (FixedArray) -- a slice `&[T]`
                // target is a shared view whose writes MUST alias the backing array, so it stays
                // shared. `Ref` is likewise left shared for `&mut` write-through.
                let value = self.eval_expression(assignment.value, frame)?;
                let copy_array = matches!(value, Value::Array(_))
                    && self
                        .assignment_target_type_reference(assignment.target, frame)
                        .map(|target| self.declared_type_is_fixed_array(target))
                        .unwrap_or(false);
                let value = if matches!(value, Value::Struct { .. }) || copy_array {
                    value.deep_clone()
                } else {
                    value
                };
                // Apply the target field's declared width AND arithmetic domain
                // (decision 17), matching the native store: Exact/Wrapping truncate
                // to the field's low bytes (a u16 field assigned 70000 reads back
                // 4464), Saturating clamps to the type range (a u8 Saturating field
                // assigned a folded 10000 reads back 255, not the wrapped 16), and
                // Trapping halts on overflow. Mirrors the LocalData store below.
                // Coerce the stored SCALAR to the target's declared width +
                // arithmetic domain, matching the native store -- for a FIELD from
                // its type, for an ARRAY ELEMENT `arr[i]` from the element width +
                // the array's domain (`[u8;N]` given `a+b`=300 reads 44,
                // `[u8;N] in Saturating` clamps to 255). Integers truncate/clamp/
                // trap (decision 17); an f32 target rounds to f32 (native keeps f32
                // in the slot). Mirrors the LocalData store below.
                if self.write_mutable_record_recast_target(
                    assignment.target,
                    frame,
                    value.clone(),
                )? {
                    return Ok(());
                }
                let value = if let Some(recast) =
                    self.mutable_scalar_recast_target(assignment.target, frame)
                {
                    // The write is stated in the VIEW type, then lands in the
                    // backing scalar cell or byte region as the identical bit
                    // pattern. Validation proves the complete footprint.
                    let target = recast.target().ok_or_else(|| {
                        Halt::Trap("record recast reached the scalar write seam".to_owned())
                    })?;
                    let value = self.coerce_scalar_with(value, target, ArithmeticDomain::Exact)?;
                    match recast {
                        MutableScalarRecast::Direct { source, .. } => {
                            self.eval_recast(value, Some(source))?
                        }
                        MutableScalarRecast::ByteRegion {
                            cells,
                            offset,
                            target,
                        } => {
                            self.write_scalar_byte_region(&cells, offset, target, value)?;
                            return Ok(());
                        }
                        MutableScalarRecast::AggregateByteRegion { .. }
                        | MutableScalarRecast::AggregateTyped { .. } => {
                            return trap("aggregate recast reached the scalar write seam");
                        }
                    }
                } else {
                    match self.assignment_target_coercion(assignment.target, frame) {
                        Some((primitive, domain)) => {
                            self.coerce_scalar_with(value, primitive, domain)?
                        }
                        None => value,
                    }
                };
                // Carrier byte WRITE: `out[i] = ch` where `out` is text (`Value::Str`, packed
                // BYTES). The byte has no per-element cell, so write it straight into the vec
                // rather than resolving an element place (element_cell only handles Array). The
                // value is the byte (an Int); a range index is not a scalar write.
                if let ExpressionNode::Indexed(indexed) = self
                    .program
                    .expression_table
                    .expression(assignment.target)
                    .clone()
                    && !matches!(
                        self.program.expression_table.expression(indexed.index),
                        ExpressionNode::Range(_)
                    )
                    && let Ok(collection_cell) = self.resolve_place(indexed.collection, frame)
                {
                    let collection_cell = self.deref_cell(collection_cell);
                    if matches!(&*collection_cell.borrow(), Value::Str(_)) {
                        let index = self.eval_index(indexed.index, frame)?;
                        let byte = value.as_int().ok_or_else(|| {
                            Halt::Trap("carrier byte write value is not an integer".to_owned())
                        })? as u8;
                        if let Value::Str(text) = &*collection_cell.borrow() {
                            let mut bytes = text.borrow_mut();
                            match bytes.get_mut(index) {
                                Some(slot) => *slot = byte,
                                None => {
                                    return Err(Halt::Trap(format!(
                                        "carrier byte write index {index} out of bounds (len {})",
                                        bytes.len()
                                    )));
                                }
                            }
                        }
                        return Ok(());
                    }
                }
                let target = self.resolve_place(assignment.target, frame)?;
                // Assigning to a `&mut` place writes THROUGH the reference into the aliased
                // cell (so assigning through a mutable text-carrier parameter mutates the
                // caller's carrier), rather than rebinding the local to a non-reference value.
                let target = self.deref_cell(target);
                *target.borrow_mut() = value;
                Ok(())
            }
            StatementNode::LocalData(local) => {
                // A mutable scalar recast is an ALIAS, not a snapshot. Preserve
                // the source cell and remember the two scalar interpretations;
                // eval_name performs source -> view reads, while assignment
                // performs view -> source writes before touching the cell.
                if local.initial_value.is_valid() {
                    if let Some((source, recast)) =
                        self.mutable_scalar_recast_initializer(local.initial_value, frame)?
                    {
                        frame.bind(local.name.as_str(), Value::Ref(source).cell());
                        frame.bind_type(local.name.as_str(), local.type_reference);
                        frame
                            .mutable_scalar_recasts
                            .borrow_mut()
                            .insert(local.name.as_str().to_owned(), recast);
                        return Ok(());
                    }
                }
                // A `let v = <struct>` or `let v = <owned array>` is a VALUE copy: deep-clone so
                // a later mutation of `v` does not alias the initializer's source. A
                // `Value::Array` is deep-cloned ONLY when the local's declared type is an owned
                // `[T; N]` (FixedArray); a slice `let s = arr[1..3]` (a `&[T]` local) is a shared
                // view and must keep sharing the array's cells. A `Ref` keeps aliasing the
                // referent.
                let value = if local.initial_value.is_valid() {
                    let value = self.eval_expression(local.initial_value, frame)?;
                    let copy_array = matches!(value, Value::Array(_))
                        && self.declared_type_is_fixed_array(local.type_reference);
                    if matches!(value, Value::Struct { .. }) || copy_array {
                        value.deep_clone()
                    } else {
                        value
                    }
                } else {
                    self.default_value_for_type(local.type_reference)?
                };
                // Coerce to the local's declared width + arithmetic domain
                // (decision 17): Wrapping/Exact truncate like the native store,
                // Saturating clamps, Trapping traps, an f32 local rounds to f32.
                let value = self.coerce_scalar_value(value, local.type_reference)?;
                // A `let` introduces a fresh local cell, bound through the frame's
                // interior-mutable locals map. A scalar local also RECORDS its
                // declared (primitive, domain) so later arithmetic on the name
                // applies the domain at the operation node.
                if let Some(primitive) = self.program.primitive_type_reference(local.type_reference)
                {
                    let domain = self
                        .program
                        .arithmetic_domain_for_type_reference(local.type_reference);
                    frame
                        .scalar_locals
                        .borrow_mut()
                        .insert(local.name.as_str().to_owned(), (primitive, domain));
                }
                frame.bind(local.name.as_str(), value.cell());
                frame.bind_type(local.name.as_str(), local.type_reference);
                Ok(())
            }
            StatementNode::Call(call) => {
                self.eval_call_statement(call, frame)?;
                Ok(())
            }
            StatementNode::Expression(expression) => {
                let _ = self.eval_expression(*expression, frame)?;
                Ok(())
            }
            StatementNode::Transition(_) => {
                // Handled in run_state_collect.
                Ok(())
            }
        }
    }

    // ---- transitions --------------------------------------------------------

    fn eval_transition(
        &mut self,
        transition: &TableTransition,
        frame: &Frame,
    ) -> EvalResult<Option<TransitionDecision>> {
        let holds = match transition.guard {
            TransitionGuardNode::Always => true,
            TransitionGuardNode::When(expression) => {
                self.guard_depth += 1;
                let value = self.eval_expression(expression, frame);
                self.guard_depth -= 1;
                let value = value?;
                value
                    .as_bool()
                    .ok_or_else(|| Halt::Trap("transition guard is not boolean".to_owned()))?
            }
        };
        if !holds {
            return Ok(None);
        }

        let target = self
            .program
            .statement_table
            .transition_target(transition.target)
            .clone();
        let decision = self.resolve_transition_target(&target, frame)?;
        Ok(Some(decision))
    }

    fn resolve_transition_target(
        &mut self,
        target: &TransitionTargetNode,
        frame: &Frame,
    ) -> EvalResult<TransitionDecision> {
        match target {
            TransitionTargetNode::Terminal => Ok(TransitionDecision::Terminal),
            TransitionTargetNode::SelfTarget => Ok(TransitionDecision::SelfTarget),
            TransitionTargetNode::Value(expression) => {
                let value = self.eval_expression(*expression, frame)?;
                Ok(TransitionDecision::Value(value))
            }
            TransitionTargetNode::Named { path, arguments } => {
                let members = self.program.statement_table.name_path_members(path.members);
                let state_name = members
                    .last()
                    .map(|name| name.as_str().to_owned())
                    .ok_or_else(|| Halt::Unsupported("empty named transition".to_owned()))?;

                // Same-machine sibling state on the current `self`, or a FREE
                // machine's self-recursion (`-> count(...)` inside top-level
                // `machine count` names the MACHINE, whose body state is the
                // generated `entry`).
                let (machine, state_name) = match self.machine_of_state_named(&state_name, frame) {
                    Some(machine) => (machine, state_name),
                    None => self
                        .free_machine_self_recursion_target(&state_name, frame)
                        .ok_or_else(|| {
                            Halt::Unsupported(format!(
                                "transition target `{state_name}` not found in current machine"
                            ))
                        })?,
                };

                let mut args = Vec::new();
                for argument in self.program.statement_table.expression_handles(*arguments) {
                    args.push(self.eval_state_argument(*argument, frame)?);
                }

                Ok(TransitionDecision::Named {
                    state_name,
                    machine,
                    instance: Rc::clone(&frame.self_cell),
                    args,
                })
            }
        }
    }

    /// A FREE machine's self-recursive transition target: the named target is the
    /// CURRENT machine's own (leaf) name and the machine has no attached data, so
    /// the recursion re-enters the machine's entry state (the generated `entry`)
    /// with the transition's arguments.
    fn free_machine_self_recursion_target(
        &self,
        state_name: &str,
        frame: &Frame,
    ) -> Option<(Machine, String)> {
        let machine = self.current_machine(frame)?;
        let leaf = machine.name.as_str().rsplit("::").next().unwrap_or("");
        if machine.attached_data.is_some() || leaf != state_name {
            return None;
        }
        let entry = self.machine_entry_state_name(machine)?;
        Some((machine.clone(), entry))
    }

    /// Find the machine that owns a sibling state of `self` by state name. The entry and
    /// its sub-states all live in the same machine group; a named transition stays within
    /// the current machine.
    fn machine_of_state_named(&self, state_name: &str, frame: &Frame) -> Option<Machine> {
        // A named transition target is a SIBLING state of the machine currently executing, so
        // resolve within the CURRENT machine FIRST. Otherwise a state name shared across machines
        // -- e.g. `Picker::pick` and `Main::read_at` BOTH having a `try1` sub-state -- collides on
        // the type/global fallbacks below and runs the WRONG machine's body (the read_at `try1`
        // transition would run pick's `try1`, returning pick's value).
        if let Some(machine) = self.current_machine(frame) {
            if self.find_state(machine, state_name).is_some() {
                return Some(machine.clone());
            }
        }
        let type_symbol = match &*frame.self_cell.borrow() {
            Value::Struct { type_symbol, .. } => *type_symbol,
            _ => SymbolHandle::invalid(),
        };
        // First, the machine whose symbol matches the instance and has the state.
        for machine in self.program.machines() {
            if machine.symbol == type_symbol && self.find_state(machine, state_name).is_some() {
                return Some(machine.clone());
            }
        }
        // Fall back: any machine that defines a state of that name (single-machine
        // programs share one instance shape).
        self.program
            .machines()
            .iter()
            .find(|machine| self.find_state(machine, state_name).is_some())
            .cloned()
    }

    // ---- calls --------------------------------------------------------------

    fn eval_call_statement(&mut self, call: &TableCall, frame: &Frame) -> EvalResult<Value> {
        // Asm intrinsic statement (`asm { hlt }`): the tree-walker cannot model
        // halting the CPU, but `hlt` in an idle loop is observably a no-op step
        // (the loop simply proceeds), so evaluate it as unit. Memory fences
        // are also no-ops in the single-threaded tree walker: its evaluation
        // order is already total. CLI/STI cannot change an interrupt source
        // the interpreter does not model, so they are unit steps as well.
        // Port I/O (`asm#port_out`) has real device effects the interpreter
        // cannot reproduce and stays unsupported.
        if call.target.as_str() == "asm#hlt"
            || call.target.as_str() == "asm#popfq"
            || omega_core::inline_assembly::AsmFenceKind::from_intrinsic_name(call.target.as_str())
                .is_some()
            || omega_core::inline_assembly::AsmInterruptControlKind::from_intrinsic_name(
                call.target.as_str(),
            )
            .is_some()
        {
            return Ok(Value::Unit);
        }
        // CH10 root grant (GR3): `b.accept_boundary<path>();` desugars to
        // the `accept_boundary#<path>` marker call. Grants are DECLARATIONS
        // harvested statically by the build-config pass; evaluation serves
        // the marker as a no-op so the build machine runs through it.
        if call.target.as_str().starts_with("accept_boundary#")
            || call.target.as_str().starts_with("select_provider#")
            || call.target.as_str().starts_with("wire_compatibility#")
        {
            return Ok(Value::Unit);
        }

        // Host boundary call? (e.g. self.console.exit_process(70))
        if let Some(value) = self.try_host_call(call, frame)? {
            return Ok(value);
        }

        // The synthesized wire encoder (chapter 20, wire stage 2a)?
        if let Some(value) = self.try_wire_encode_call(call, frame)? {
            return Ok(value);
        }

        // The synthesized wire decoder (chapter 20, wire stage 2b)?
        if let Some(value) = self.try_wire_decode_call(call, frame)? {
            return Ok(value);
        }

        let target = call.target.as_str();
        let (machine, state_name, instance) = if call.receiver.is_empty() {
            self.resolve_entry_state_symbol(call.target_symbol, frame)
                .map_or_else(|| self.resolve_state_call(call.receiver, target, frame), Ok)?
        } else {
            self.resolve_state_call(call.receiver, target, frame)?
        };

        let mut args = Vec::new();
        for argument in self
            .program
            .statement_table
            .expression_handles(call.arguments)
        {
            args.push(self.eval_state_argument(*argument, frame)?);
        }

        self.run_state_collect(&machine, &state_name, instance, args)
            .map(|value| value.unwrap_or(Value::Unit))
    }

    /// Resolve a receiverless direct call through its exact entry-state
    /// symbol. Selected checked adapters and static-machine specialization
    /// retain this identity specifically so attached/plural realizations do
    /// not fall back to ambiguous display-name lookup.
    fn resolve_entry_state_symbol(
        &self,
        target_symbol: SymbolHandle,
        frame: &Frame,
    ) -> Option<(Machine, String, Cell)> {
        if !target_symbol.is_valid() {
            return None;
        }
        self.program.machines().iter().find_map(|machine| {
            self.program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == target_symbol)
                .map(|state| {
                    (
                        machine.clone(),
                        state.name.as_str().to_owned(),
                        Rc::clone(&frame.self_cell),
                    )
                })
        })
    }

    /// Resolve a call target -- a state name with an optional receiver path -- to the
    /// (machine, state, instance) it runs against. Priority:
    /// 1. An explicit receiver path naming a CONTAINED sub-machine instance field whose
    ///    type defines the target state (`self.dungeon.foo()`): run on that sub-instance.
    /// 2. A SIBLING state of the current machine (`self.foo()` where `foo` is a state of
    ///    the machine currently executing): run that state on the same `self`.
    /// 3. A free helper machine named `<group>::<target>` or any machine with that state:
    ///    run its entry state on the current `self`.
    fn resolve_state_call(
        &self,
        receiver: omega_core::arena::HandleSpan<omega_typed_trees::name::Identifier>,
        target: &str,
        frame: &Frame,
    ) -> EvalResult<(Machine, String, Cell)> {
        // (1) Explicit receiver path to a contained sub-machine instance.
        if let Some(resolved) = self.resolve_receiver_state_call(receiver, target, frame)? {
            return Ok(resolved);
        }

        // (2) Sibling state of the current machine.
        if let Some(machine) = self.current_machine(frame) {
            if self.find_state(machine, target).is_some() {
                return Ok((
                    machine.clone(),
                    target.to_owned(),
                    Rc::clone(&frame.self_cell),
                ));
            }
        }

        // (3) A free helper machine.
        let machine = self
            .find_machine_for_call(target, frame)
            .ok_or_else(|| Halt::Unsupported(format!("unknown call target `{target}`")))?;
        let entry_state = self
            .machine_entry_state_name(&machine)
            .ok_or_else(|| Halt::Unsupported(format!("call target `{target}` has no state")))?;
        Ok((machine, entry_state, Rc::clone(&frame.self_cell)))
    }

    /// If the call has a receiver path that resolves (relative to `self`) to a CONTAINED
    /// sub-machine instance whose machine defines the target state, return that instance and
    /// machine. The receiver path's leaf is the field; the head may be `self`.
    fn resolve_receiver_state_call(
        &self,
        receiver: omega_core::arena::HandleSpan<omega_typed_trees::name::Identifier>,
        target: &str,
        frame: &Frame,
    ) -> EvalResult<Option<(Machine, String, Cell)>> {
        let members: Vec<String> = self
            .program
            .statement_table
            .name_path_members(receiver)
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect();
        if members.is_empty() {
            return Ok(None);
        }

        // Walk the receiver path to a cell, starting at `self` (an implicit-self leaf like
        // `console` is a single-member path; `self.dungeon` is `[self, dungeon]`).
        let mut cell = Rc::clone(&frame.self_cell);
        let mut start = 0;
        if members[0] == "self" {
            start = 1;
        } else if let Some(local) = frame.get(&members[0]) {
            cell = local;
            start = 1;
        }
        for member in &members[start..] {
            cell = self.deref_cell(cell);
            match self.field_cell(&cell, member) {
                Ok(next) => cell = next,
                Err(_) => return Ok(None),
            }
        }
        cell = self.deref_cell(cell);

        // Only treat this as a sub-machine call if the receiver is NOT just `self` (a bare
        // self receiver is handled by the sibling-state path).
        let bare_self = members.len() == start && start == 1;
        if bare_self {
            return Ok(None);
        }
        Ok(self
            .machine_for_instance_state(&cell, target)
            .map(|machine| (machine, target.to_owned(), cell)))
    }

    /// Find the machine that operates on `instance` and defines `target` as a state. The
    /// instance is a `Struct` whose `type_name` is the data/machine type (e.g. `Circle`); a
    /// free machine `Circle::code` lives in that type's group. Matches by machine symbol, by
    /// attached-data name, or by the `<type>::<target>` group-qualified machine name.
    fn machine_for_instance_state(&self, instance: &Cell, target: &str) -> Option<Machine> {
        let (type_symbol, type_name) = match &*instance.borrow() {
            Value::Struct {
                type_symbol,
                type_name,
                ..
            } => (*type_symbol, type_name.clone()),
            // An ENUM receiver (`self.s.go_value()` where `s: Signal`): the
            // enum-attached machine group is the declaring data type, whose
            // NAME resolves from the value's type_symbol. Without this arm the
            // method call silently failed to find its machine and returned ZII.
            Value::Enum { type_symbol, .. } => {
                let name = self
                    .program
                    .data_definitions()
                    .iter()
                    .find(|data| type_symbol.is_valid() && data.symbol == *type_symbol)
                    .map(|data| data.name.as_str().to_owned())?;
                (*type_symbol, name)
            }
            _ => return None,
        };
        // The group is the leading segment of the type name (e.g. `Circle` from `Circle`).
        let group = type_name
            .split("::")
            .next()
            .unwrap_or(&type_name)
            .to_owned();
        for machine in self.program.machines() {
            if self.find_state(machine, target).is_none() {
                continue;
            }
            let machine_group = machine
                .name
                .as_str()
                .split("::")
                .next()
                .unwrap_or("")
                .to_owned();
            let by_symbol = type_symbol.is_valid() && machine.symbol == type_symbol;
            let by_attached = machine
                .attached_data
                .as_ref()
                .is_some_and(|data| data.as_str() == group);
            let by_group = machine_group == group;
            if by_symbol || by_attached || by_group {
                return Some(machine.clone());
            }
        }
        None
    }

    fn current_machine(&self, frame: &Frame) -> Option<&'program Machine> {
        if !frame.machine_symbol.is_valid() {
            return None;
        }
        self.program
            .machines()
            .iter()
            .find(|machine| machine.symbol == frame.machine_symbol)
    }

    /// Find the machine invoked by a call whose `target` is a state name. A free helper
    /// machine is named `<group>::<target>` (e.g. `Main::bump`); resolve by that name, or
    /// by any machine that contains a state of that name and shares the receiver group.
    fn find_machine_for_call(&self, target: &str, frame: &Frame) -> Option<Machine> {
        // The receiver's machine-group prefix (e.g. "Main" from "Main::main").
        let group = {
            let self_name = match &*frame.self_cell.borrow() {
                Value::Struct { type_name, .. } => type_name.clone(),
                _ => String::new(),
            };
            self_name
                .split("::")
                .next()
                .map(|prefix| prefix.to_owned())
                .unwrap_or_default()
        };

        let qualified = format!("{group}::{target}");
        if let Some(machine) = self.find_machine_by_name(&qualified) {
            return Some(machine.clone());
        }
        // A FREE top-level machine named exactly `target` (`machine pick(x: i32)
        // -> i32`): its body state is the generated `entry`, so the state-name
        // scan below would miss it.
        if let Some(machine) = self.find_machine_by_name(target) {
            if machine.attached_data.is_none() {
                return Some(machine.clone());
            }
        }
        // Otherwise a machine that simply has a state named `target` -- but only when that
        // is UNAMBIGUOUS. With several candidates (e.g. two impls of the same trait
        // machine), guessing the first would silently dispatch to the wrong type; decline
        // instead so the caller reports unsupported (dispatch by the RECEIVER's runtime
        // type is handled earlier, in `machine_for_instance_state`).
        let mut candidates = self
            .program
            .machines()
            .iter()
            .filter(|machine| self.find_state(machine, target).is_some());
        let first = candidates.next().cloned();
        if candidates.next().is_some() {
            return None;
        }
        first
    }

    fn machine_entry_state_name(&self, machine: &Machine) -> Option<String> {
        // A free helper machine `Main::bump` exposes its body as a state. Prefer a state
        // whose name matches the machine's leaf (`bump`); else the first state.
        let leaf = machine.name.as_str().rsplit("::").next().unwrap_or("");
        if self.find_state(machine, leaf).is_some() {
            return Some(leaf.to_owned());
        }
        self.program
            .machine_states(machine)
            .first()
            .map(|state| state.name.as_str().to_owned())
    }

    /// Evaluate an argument. A `Mutable(place)` or a direct place under a `&mut` param
    /// yields a `Ref` that ALIASES the original cell; a value argument yields a fresh
    /// cell holding a copy.
    fn eval_argument(&mut self, argument: ExpressionHandle, frame: &Frame) -> EvalResult<Cell> {
        match self.program.expression_table.expression(argument) {
            ExpressionNode::Mutable(inner) => {
                // &mut place -> a Ref to the SAME cell (the whole point of the oracle). The
                // param binding holds a `Ref`, so a later forward of that param (as a bare
                // name) can detect it is a reference and keep aliasing -- otherwise a
                // mutable text carrier passed down a call chain detaches after the first hop.
                let cell = self.resolve_place(*inner, frame)?;
                // A RE-BORROW (`&mut t` where `t` is itself a `&mut` param)
                // aliases the SAME target: forward the inner Ref instead of
                // nesting Ref-to-Ref, which downstream single-level derefs
                // (receiver method resolution) cannot see through -- the
                // param-forwarding chain declined with "unknown value-call
                // target" while the native build served it (2026-07-11l).
                let target = match &*cell.borrow() {
                    Value::Ref(target) => Rc::clone(target),
                    _ => Rc::clone(&cell),
                };
                Ok(Value::Ref(target).cell())
            }
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                // A bare place argument that is ALREADY a reference (a forwarded `&mut`
                // parameter, e.g. `out_room` of type `&mut Room`) must keep aliasing the
                // same underlying cell -- otherwise a chain of forwarding calls silently
                // detaches the write. If the place resolves and holds a `Ref`, forward that
                // cell; otherwise evaluate the expression normally (handles enum-value name
                // paths, plain values, etc.).
                if let Ok(place) = self.resolve_place(argument, frame) {
                    let forwarded = match &*place.borrow() {
                        Value::Ref(target) => Some(Rc::clone(target)),
                        _ => None,
                    };
                    if let Some(target) = forwarded {
                        // Keep the Ref WRAPPER (not the bare target cell) so reference-ness
                        // survives the NEXT hop too: the callee's param must itself look like
                        // a `&mut` binding when it forwards the bare name onward (e.g. a
                        // transition arm `gate_title(out_line)` two machines deep).
                        return Ok(Value::Ref(target).cell());
                    }
                }
                let value = self.eval_expression(argument, frame)?;
                Ok(value.cell())
            }
            _ => {
                let value = self.eval_expression(argument, frame)?;
                Ok(value.cell())
            }
        }
    }

    /// `Schema::encode(&value, &mut out, &mut written)` -- the
    /// compact_binary v0 encoder the compiler synthesizes for a wire schema
    /// (chapter 20, wire stage 2a). The interpreter implements the IDENTICAL
    /// framing the native backends emit: the CURRENT era discriminator
    /// varint, then per field in field-number order a field-number varint and
    /// a value varint (unsigned LEB128; signed values zigzag
    /// `(n << 1) ^ (n >> 63)`; bool = 0/1). A `String` field (at most one,
    /// encoding LAST) rides as its byte-count varint followed by the raw
    /// UTF-8 bytes, and -- matching the native bounds-checked byte-copy --
    /// content past the out buffer's capacity is DROPPED rather than written.
    /// A NESTED MESSAGE field rides as its tag varint, a byte-LENGTH varint,
    /// then the child schema's fields (tags + scalar varints) WITHOUT an era
    /// discriminator (decision 10: the era rides only the top-level
    /// envelope). The shared `omega_typed_trees::wire` vocabulary (field
    /// encodings + varint bytes) keeps interpreter and backends byte-for-byte
    /// in lockstep.
    fn try_wire_encode_call(
        &mut self,
        call: &TableCall,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        use omega_typed_trees::wire::{WireFieldEncoding, WireMember, wire_varint_bytes};

        let Some(schema) = self.program.wire_encode_call_schema(call) else {
            return Ok(None);
        };
        let schema_name = schema.name.as_str().to_owned();
        let era = self.program.wire_schema_current_era(schema);

        // (field name, number, content) of the CURRENT era, in field-number
        // order -- validation has already enforced the stage 2 field set
        // (scalars, at most one trailing String, scalar-only nested
        // messages).
        let mut fields = Vec::new();
        for member in self.program.wire_members(schema.members) {
            let WireMember::Field(field) = member else {
                continue;
            };
            if let Some(repeated) = self.program.wire_field_repeated_encoding(field) {
                fields.push((
                    field.name.as_str().to_owned(),
                    field.number,
                    WireInterpField::Repeated(repeated),
                ));
                continue;
            }
            if let Some(child) = self.program.wire_field_nested_schema(field) {
                let children = wire_nested_scalar_fields(&self.program, child)?;
                fields.push((
                    field.name.as_str().to_owned(),
                    field.number,
                    WireInterpField::Nested(children),
                ));
                continue;
            }
            // A borrowed `&[u8]` field encodes as RAW bytes (length + the bytes),
            // read from the field's element array.
            if self.program.is_borrowed_byte_slice(field.type_reference) {
                fields.push((
                    field.name.as_str().to_owned(),
                    field.number,
                    WireInterpField::ByteSlice,
                ));
                continue;
            }
            if let Some(slice) = self
                .program
                .wire_field_borrowed_scalar_slice_encoding(field)
            {
                fields.push((
                    field.name.as_str().to_owned(),
                    field.number,
                    WireInterpField::ScalarSlice(slice),
                ));
                continue;
            }
            let encoding = self
                .program
                .primitive_type_reference(field.type_reference)
                .and_then(WireFieldEncoding::for_primitive)
                .ok_or_else(|| {
                    Halt::Unsupported(format!(
                        "data `{schema_name}` field `{}` is not a stage 2a scalar or String",
                        field.name
                    ))
                })?;
            fields.push((
                field.name.as_str().to_owned(),
                field.number,
                WireInterpField::Direct(encoding),
            ));
        }
        fields.sort_by_key(|(_, number, _)| *number);
        let has_text_field = fields.iter().any(|(_, _, content)| {
            matches!(
                content,
                WireInterpField::Direct(WireFieldEncoding::Text)
                    | WireInterpField::ByteSlice
                    | WireInterpField::ScalarSlice(_)
            )
        });

        let arguments = self
            .program
            .statement_table
            .expression_handles(call.arguments);
        let [value_argument, out_argument, written_argument] = arguments else {
            return Err(Halt::Trap(format!(
                "`{schema_name}::encode` expects 3 arguments, got {}",
                arguments.len()
            )));
        };
        let (value_argument, out_argument, written_argument) =
            (*value_argument, *out_argument, *written_argument);

        let value_cell = self.eval_argument(value_argument, frame)?;
        let value_cell = self.deref_cell(value_cell);
        let out_cell = self.eval_argument(out_argument, frame)?;
        let out_cell = self.deref_cell(out_cell);
        let written_cell = self.eval_argument(written_argument, frame)?;
        let written_cell = self.deref_cell(written_cell);

        let mut bytes = wire_varint_bytes(era);
        for (field_name, number, content) in &fields {
            bytes.extend(wire_varint_bytes(*number));

            let raw = match &*value_cell.borrow() {
                Value::Struct { fields, .. } => fields
                    .get(field_name)
                    .map(|cell| self.deref_cell(Rc::clone(cell)))
                    .ok_or_else(|| {
                        Halt::Trap(format!(
                            "`{schema_name}::encode` value has no field `{field_name}`"
                        ))
                    })?,
                _ => {
                    return Err(Halt::Trap(format!(
                        "`{schema_name}::encode` value argument is not a data value"
                    )));
                }
            };
            match content {
                WireInterpField::Direct(WireFieldEncoding::Scalar(scalar)) => {
                    let raw = raw.borrow().as_int().ok_or_else(|| {
                        Halt::Trap(format!(
                            "`{schema_name}::encode` field `{field_name}` is not a scalar value"
                        ))
                    })?;
                    bytes.extend(wire_varint_bytes(wire_scalar_varint_value(raw, *scalar)?));
                }
                WireInterpField::Nested(children) => {
                    // The sub-message's fields into a staging body first --
                    // mirroring the native scratch staging -- then the LENGTH
                    // varint and the body. NO era discriminator: the era
                    // rides only the top-level envelope (decision 10).
                    let mut body = Vec::new();
                    for (child_name, child_number, scalar) in children {
                        body.extend(wire_varint_bytes(*child_number));
                        let child_raw = match &*raw.borrow() {
                            Value::Struct { fields, .. } => fields
                                .get(child_name)
                                .map(|cell| self.deref_cell(Rc::clone(cell)))
                                .ok_or_else(|| {
                                    Halt::Trap(format!(
                                        "`{schema_name}::encode` nested field `{field_name}` has no member `{child_name}`"
                                    ))
                                })?,
                            _ => {
                                return Err(Halt::Trap(format!(
                                    "`{schema_name}::encode` nested field `{field_name}` is not a data value"
                                )));
                            }
                        };
                        let child_raw = child_raw.borrow().as_int().ok_or_else(|| {
                            Halt::Trap(format!(
                                "`{schema_name}::encode` nested field `{field_name}.{child_name}` is not a scalar value"
                            ))
                        })?;
                        body.extend(wire_varint_bytes(wire_scalar_varint_value(
                            child_raw, *scalar,
                        )?));
                    }
                    bytes.extend(wire_varint_bytes(body.len() as u64));
                    bytes.extend(body);
                }
                WireInterpField::Repeated(repeated) => {
                    // Fixed arrays are exactly full. FixedVec carries its
                    // runtime length beside the inline items array.
                    let (items, live) = match repeated.carrier {
                        omega_typed_trees::wire::WireRepeatedCarrier::FixedArray => {
                            (Rc::clone(&raw), repeated.max_count)
                        }
                        omega_typed_trees::wire::WireRepeatedCarrier::FixedVec => {
                            let (items, length) = match &*raw.borrow() {
                                Value::Struct { fields, .. } => {
                                    let items = fields.get("items").map(Rc::clone).ok_or_else(|| {
                                        Halt::Trap(format!(
                                            "`{schema_name}::encode` FixedVec field `{field_name}` has no `items`"
                                        ))
                                    })?;
                                    let length =
                                        fields.get("length").map(Rc::clone).ok_or_else(|| {
                                            Halt::Trap(format!(
                                                "`{schema_name}::encode` FixedVec field `{field_name}` has no `length`"
                                            ))
                                        })?;
                                    (self.deref_cell(items), self.deref_cell(length))
                                }
                                _ => {
                                    return Err(Halt::Trap(format!(
                                        "`{schema_name}::encode` repeated field `{field_name}` is not a FixedVec value"
                                    )));
                                }
                            };
                            let length = length.borrow().as_int().ok_or_else(|| {
                                Halt::Trap(format!(
                                    "`{schema_name}::encode` FixedVec field `{field_name}.length` is not a scalar value"
                                ))
                            })? as u64;
                            (items, length.min(repeated.max_count as u64) as usize)
                        }
                    };
                    let mut body = Vec::new();
                    match &*items.borrow() {
                        Value::Array(elements) => {
                            for element in elements.iter().take(live) {
                                let element_raw =
                                    self.deref_cell(Rc::clone(element)).borrow().as_int().ok_or_else(
                                        || {
                                            Halt::Trap(format!(
                                                "`{schema_name}::encode` repeated field `{field_name}` element is not a scalar value"
                                            ))
                                        },
                                    )?;
                                body.extend(wire_varint_bytes(wire_scalar_varint_value(
                                    element_raw,
                                    repeated.element,
                                )?));
                            }
                        }
                        _ => {
                            return Err(Halt::Trap(format!(
                                "`{schema_name}::encode` repeated field `{field_name}` has no inline array storage"
                            )));
                        }
                    }
                    bytes.extend(wire_varint_bytes(body.len() as u64));
                    bytes.extend(body);
                }
                WireInterpField::ScalarSlice(slice) => {
                    let elements = match &*raw.borrow() {
                        Value::Array(elements) => elements.clone(),
                        _ => {
                            return Err(Halt::Trap(format!(
                                "`{schema_name}::encode` field `{field_name}` is not a borrowed scalar-slice value"
                            )));
                        }
                    };
                    let mut body = Vec::new();
                    for element in &elements {
                        let element_raw =
                            self.deref_cell(Rc::clone(element)).borrow().as_int().ok_or_else(
                                || {
                                    Halt::Trap(format!(
                                        "`{schema_name}::encode` borrowed scalar-slice field `{field_name}` element is not a scalar value"
                                    ))
                                },
                            )?;
                        body.extend(wire_varint_bytes(wire_scalar_varint_value(
                            element_raw,
                            slice.element,
                        )?));
                    }
                    bytes.extend(wire_varint_bytes(body.len() as u64));
                    bytes.extend(body);
                }
                WireInterpField::Direct(WireFieldEncoding::Text) => {
                    // Length varint (byte count) then the raw UTF-8 bytes --
                    // the same framing the native text-bytes append emits.
                    let text = match &*raw.borrow() {
                        Value::Str(text) => text.borrow().clone(),
                        _ => {
                            return Err(Halt::Trap(format!(
                                "`{schema_name}::encode` field `{field_name}` is not a String value"
                            )));
                        }
                    };
                    bytes.extend(wire_varint_bytes(text.len() as u64));
                    bytes.extend_from_slice(&text);
                }
                WireInterpField::ByteSlice => {
                    // Length varint (byte count) then the raw bytes, framed like Text. A `&[u8]`
                    // field is text BYTES (`Value::Str`, after the text=bytes model) OR a fixed
                    // array of byte cells; both yield the raw content.
                    let str_bytes = if let Value::Str(text) = &*raw.borrow() {
                        Some(text.borrow().clone())
                    } else {
                        None
                    };
                    let content: Vec<u8> = if let Some(content) = str_bytes {
                        content
                    } else {
                        let elements = match &*raw.borrow() {
                            Value::Array(elements) => elements.clone(),
                            _ => {
                                return Err(Halt::Trap(format!(
                                    "`{schema_name}::encode` field `{field_name}` is not a byte-slice value"
                                )));
                            }
                        };
                        let mut content = Vec::with_capacity(elements.len());
                        for element in &elements {
                            let byte = self
                                .deref_cell(Rc::clone(element))
                                .borrow()
                                .as_int()
                                .ok_or_else(|| {
                                    Halt::Trap(format!(
                                        "`{schema_name}::encode` byte-slice field `{field_name}` element is not a byte"
                                    ))
                                })?;
                            content.push(byte as u8);
                        }
                        content
                    };
                    bytes.extend(wire_varint_bytes(content.len() as u64));
                    bytes.extend(content);
                }
            }
        }

        match &*out_cell.borrow() {
            Value::Array(elements) => {
                if bytes.len() > elements.len() && !has_text_field {
                    // Without a runtime-sized text field validation's worst-case budget
                    // covers every byte, so an overflow here is a compiler
                    // bug, not a program state -- trap loudly.
                    return Err(Halt::Trap(format!(
                        "`{schema_name}::encode` produced {} bytes into a {}-byte buffer",
                        bytes.len(),
                        elements.len()
                    )));
                }
                // With a runtime-sized text field the native byte-copy bounds every store
                // against the buffer's capacity and DROPS overflowing content
                // (the text field encodes last); `zip` clamps identically.
                for (element, byte) in elements.iter().zip(&bytes) {
                    *element.borrow_mut() = Value::Int(i64::from(*byte));
                }
            }
            _ => {
                return Err(Halt::Trap(format!(
                    "`{schema_name}::encode` out argument is not a fixed byte array"
                )));
            }
        }
        let buffer_capacity = match &*out_cell.borrow() {
            Value::Array(elements) => elements.len(),
            _ => unreachable!("out argument validated as an array above"),
        };
        *written_cell.borrow_mut() = Value::Int(bytes.len().min(buffer_capacity) as i64);

        Ok(Some(Value::Unit))
    }

    /// `Schema::decode(&mut value, &buffer, &mut read, &mut verdict)` -- the
    /// compact_binary v0 decoder the compiler synthesizes for a wire schema
    /// (chapter 20, wire stage 2b). The interpreter simulates the IDENTICAL
    /// operation sequence the native backends emit -- expected framing bytes
    /// for the CURRENT era discriminator and each field-number tag, then a
    /// bounds-checked LEB128 value read per field -- including the sticky
    /// failure semantics: the first violation (wrong era, unexpected tag,
    /// truncated input, overlong varint) clears `ok`, but the remaining
    /// operations still run so cursor and field side effects match the native
    /// sequences byte for byte even on the failure path.
    fn try_wire_decode_call(
        &mut self,
        call: &TableCall,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        use omega_typed_trees::wire::{WireMember, WireScalarEncoding, wire_varint_bytes};

        let Some(schema) = self.program.wire_decode_call_schema(call) else {
            return Ok(None);
        };
        let schema_name = schema.name.as_str().to_owned();
        let era = self.program.wire_schema_current_era(schema);
        let arguments = self
            .program
            .statement_table
            .expression_handles(call.arguments);
        let [value_argument, buffer_argument, read_argument, ok_argument] = arguments else {
            return Err(Halt::Trap(format!(
                "`{schema_name}::decode` expects 4 arguments, got {}",
                arguments.len()
            )));
        };
        let (value_argument, buffer_argument, read_argument, ok_argument) = (
            *value_argument,
            *buffer_argument,
            *read_argument,
            *ok_argument,
        );
        let value_type = wire_argument_declared_type(self.program, frame, value_argument)
            .ok_or_else(|| {
                Halt::Unsupported(format!(
                    "`{schema_name}::decode` cannot resolve the declared value type"
                ))
            })?;

        // (field name, number, content) of the CURRENT era, in field-number
        // order -- validation has already enforced the stage 2 field set
        // (scalars plus scalar-only nested messages).
        let mut fields = Vec::new();
        for member in self.program.wire_members(schema.members) {
            let WireMember::Field(field) = member else {
                continue;
            };
            let target_type = omega_typed_trees::wire::data_field_type(
                self.program,
                value_type,
                field.name.as_str(),
            )
            .ok_or_else(|| {
                Halt::Unsupported(format!(
                    "`{schema_name}::decode` cannot resolve destination field `{}`",
                    field.name
                ))
            })?;
            if let Some(repeated) = self.program.wire_field_repeated_encoding(field) {
                let range = omega_typed_trees::wire::repeated_element_type(
                    self.program,
                    target_type,
                    repeated.carrier,
                )
                .and_then(|element| {
                    omega_typed_trees::wire::scalar_decode_range(self.program, element)
                });
                fields.push((
                    field.name.as_str().to_owned(),
                    field.number,
                    WireInterpScalarField::Repeated {
                        encoding: repeated,
                        range,
                    },
                ));
                continue;
            }
            if let Some(child) = self.program.wire_field_nested_schema(field) {
                let children = wire_nested_decode_scalar_fields(&self.program, child, target_type)?;
                fields.push((
                    field.name.as_str().to_owned(),
                    field.number,
                    WireInterpScalarField::Nested(children),
                ));
                continue;
            }
            // A borrowed `&[u8]` field decodes zero-copy: length-prefixed bytes
            // viewed in the buffer (validation requires the value field `&[u8]`).
            // A DOMAIN on the slice (`&[u8] in Utf8`) is a decode-boundary
            // obligation: the wire carries UNTRUSTED bytes no compile-time
            // proof covers, so the decoder evaluates the domain's recognized
            // byte predicate and fails the verdict when it does not hold. A
            // declared domain not reducible to one recognized byte-predicate
            // fact refuses LOUDLY -- silently skipping validation would
            // deliver a domain-tagged slice with unchecked bytes (the pinned
            // utf8_decode_accepts_invalid_bytes soundness hole).
            if self.program.is_borrowed_byte_slice(field.type_reference) {
                let mut predicates = Vec::new();
                for (domain_name, predicate) in
                    omega_typed_trees::byte_predicates::type_reference_domain_predicates(
                        &self.program,
                        field.type_reference,
                    )
                {
                    let Some(predicate) = predicate else {
                        return Err(Halt::Unsupported(format!(
                            "`{schema_name}::decode` field `{}` carries domain `{domain_name}`, which is not exactly one recognized byte-predicate fact -- the decode boundary cannot validate it yet",
                            field.name
                        )));
                    };
                    predicates.push(predicate);
                }
                fields.push((
                    field.name.as_str().to_owned(),
                    field.number,
                    WireInterpScalarField::ByteSlice { predicates },
                ));
                continue;
            }
            let encoding = self
                .program
                .primitive_type_reference(field.type_reference)
                .and_then(WireScalarEncoding::for_primitive)
                .ok_or_else(|| {
                    Halt::Unsupported(format!(
                        "data `{schema_name}` field `{}` is not a stage 2 scalar",
                        field.name
                    ))
                })?;
            fields.push((
                field.name.as_str().to_owned(),
                field.number,
                WireInterpScalarField::Scalar {
                    encoding,
                    range: omega_typed_trees::wire::scalar_decode_range(self.program, target_type),
                },
            ));
        }
        fields.sort_by_key(|(_, number, _)| *number);

        let value_cell = self.eval_argument(value_argument, frame)?;
        let value_cell = self.deref_cell(value_cell);
        let buffer_cell = self.eval_argument(buffer_argument, frame)?;
        let buffer_cell = self.deref_cell(buffer_cell);
        let read_cell = self.eval_argument(read_argument, frame)?;
        let read_cell = self.deref_cell(read_cell);
        let ok_cell = self.eval_argument(ok_argument, frame)?;
        let ok_cell = self.deref_cell(ok_cell);

        // The decode buffer's bytes and compile-time length.
        let buffer: Vec<u8> = match &*buffer_cell.borrow() {
            Value::Array(elements) => elements
                .iter()
                .map(|element| {
                    element
                        .borrow()
                        .as_int()
                        .map(|byte| byte as u8)
                        .ok_or_else(|| {
                            Halt::Trap(format!(
                                "`{schema_name}::decode` buffer element is not a byte"
                            ))
                        })
                })
                .collect::<Result<_, _>>()?,
            _ => {
                return Err(Halt::Trap(format!(
                    "`{schema_name}::decode` buffer argument is not a fixed byte array"
                )));
            }
        };

        // read = 0, ok = true -- then the sticky flag only ever clears.
        let mut cursor = 0usize;
        let mut ok = true;

        // One expected framing byte: out of bounds clears ok without
        // consuming; a mismatch consumes the byte and clears ok.
        let expect_byte = |cursor: &mut usize, ok: &mut bool, expected: u8| {
            let Some(byte) = buffer.get(*cursor).copied() else {
                *ok = false;
                return;
            };
            *cursor += 1;
            if byte != expected {
                *ok = false;
            }
        };

        // One canonical LEB128 value read, mirroring the native loop exactly:
        // truncation, more than ten groups, a zero terminal payload after the
        // first group, or a tenth payload above one clear ok. The accumulated
        // value is returned regardless (failure permits partial output).
        let read_varint = |cursor: &mut usize, ok: &mut bool| -> u64 {
            let mut value = 0u64;
            let mut shift = 0u32;
            loop {
                if shift > 63 {
                    *ok = false;
                    return value;
                }
                let Some(byte) = buffer.get(*cursor).copied() else {
                    *ok = false;
                    return value;
                };
                *cursor += 1;
                let payload = u64::from(byte & 0x7f);
                if shift == 63 && payload > 1 {
                    *ok = false;
                }
                value |= payload << shift;
                let terminal = byte & 0x80 == 0;
                if terminal && shift > 0 && payload == 0 {
                    *ok = false;
                }
                shift += 7;
                if terminal {
                    return value;
                }
            }
        };

        for byte in wire_varint_bytes(era) {
            expect_byte(&mut cursor, &mut ok, byte);
        }

        for (field_name, number, content) in &fields {
            for byte in wire_varint_bytes(*number) {
                expect_byte(&mut cursor, &mut ok, byte);
            }

            let field_cell = match &*value_cell.borrow() {
                Value::Struct { fields, .. } => {
                    fields.get(field_name).map(Rc::clone).ok_or_else(|| {
                        Halt::Trap(format!(
                            "`{schema_name}::decode` value has no field `{field_name}`"
                        ))
                    })?
                }
                _ => {
                    return Err(Halt::Trap(format!(
                        "`{schema_name}::decode` value argument is not a data value"
                    )));
                }
            };
            let field_cell = self.deref_cell(field_cell);

            match content {
                WireInterpScalarField::Scalar { encoding, range } => {
                    let raw = read_varint(&mut cursor, &mut ok);
                    let decoded = wire_decoded_scalar_value(raw, *encoding)?;
                    if range
                        .is_none_or(|range| wire_scalar_in_range(raw, *encoding, &decoded, range))
                    {
                        *field_cell.borrow_mut() = decoded;
                    } else {
                        ok = false;
                    }
                }
                WireInterpScalarField::ByteSlice { predicates } => {
                    // A borrowed `&[u8]`: a byte-LENGTH varint then that many
                    // bytes, stored as an owned Array of byte values
                    // (observationally identical to a buffer view for any read).
                    // A length past the buffer clears ok and the cursor stops at
                    // the buffer end -- the native byte-copy bounds-checks the
                    // same way.
                    let length = read_varint(&mut cursor, &mut ok) as usize;
                    let available = buffer.len().saturating_sub(cursor);
                    if length > available {
                        ok = false;
                    }
                    let take = length.min(available);
                    let bytes = &buffer[cursor..cursor + take];
                    // Decode-boundary domain validation: untrusted wire bytes
                    // must satisfy the slice's declared byte predicates or the
                    // verdict is Invalid (a truncated read is already !ok
                    // above; validating the truncated view is harmless).
                    for predicate in predicates {
                        if !predicate.holds_for(bytes) {
                            ok = false;
                        }
                    }
                    let elements: Vec<Cell> = bytes
                        .iter()
                        .map(|byte| Value::Int(i64::from(*byte)).cell())
                        .collect();
                    cursor += take;
                    *field_cell.borrow_mut() = Value::Array(elements);
                }
                WireInterpScalarField::Nested(children) => {
                    // LENGTH varint, then the absolute end bound -- the same
                    // two checks the native nested OPEN applies: the raw
                    // length must fit the buffer (so the 64-bit sum cannot
                    // wrap back inside it) and so must the bound. The child's
                    // fields decode WITHOUT an era discriminator, and the
                    // CLOSE check fails ok unless the cursor landed exactly
                    // on the bound.
                    let length = read_varint(&mut cursor, &mut ok);
                    if length > buffer.len() as u64 {
                        ok = false;
                    }
                    let end = cursor.wrapping_add(length as usize);
                    if end > buffer.len() {
                        ok = false;
                    }
                    for (child_name, child_number, encoding, range) in children {
                        for byte in wire_varint_bytes(*child_number) {
                            expect_byte(&mut cursor, &mut ok, byte);
                        }
                        let raw = read_varint(&mut cursor, &mut ok);
                        let decoded = wire_decoded_scalar_value(raw, *encoding)?;
                        let child_cell = match &*field_cell.borrow() {
                            Value::Struct { fields, .. } => {
                                fields.get(child_name).map(Rc::clone).ok_or_else(|| {
                                    Halt::Trap(format!(
                                        "`{schema_name}::decode` nested field `{field_name}` has no member `{child_name}`"
                                    ))
                                })?
                            }
                            _ => {
                                return Err(Halt::Trap(format!(
                                    "`{schema_name}::decode` nested field `{field_name}` is not a data value"
                                )));
                            }
                        };
                        let child_cell = self.deref_cell(child_cell);
                        if range.is_none_or(|range| {
                            wire_scalar_in_range(raw, *encoding, &decoded, range)
                        }) {
                            *child_cell.borrow_mut() = decoded;
                        } else {
                            ok = false;
                        }
                    }
                    if cursor != end {
                        ok = false;
                    }
                }
                WireInterpScalarField::Repeated { encoding, range } => {
                    // Byte-LENGTH varint, the same OPEN bound checks as a
                    // nested message, then the carrier-specific bounded
                    // reads. Fixed arrays require exactly N elements;
                    // FixedVec guards on the region end and updates its own
                    // length. The CLOSE check rejects framing that disagrees
                    // with either carrier.
                    let length = read_varint(&mut cursor, &mut ok);
                    if length > buffer.len() as u64 {
                        ok = false;
                    }
                    let end = cursor.wrapping_add(length as usize);
                    if end > buffer.len() {
                        ok = false;
                    }
                    let (items_cell, count_cell) = match encoding.carrier {
                        omega_typed_trees::wire::WireRepeatedCarrier::FixedArray => {
                            (Rc::clone(&field_cell), None)
                        }
                        omega_typed_trees::wire::WireRepeatedCarrier::FixedVec => {
                            match &*field_cell.borrow() {
                                Value::Struct { fields, .. } => {
                                    let items =
                                        fields.get("items").map(Rc::clone).ok_or_else(|| {
                                            Halt::Trap(format!(
                                                "`{schema_name}::decode` FixedVec field `{field_name}` has no `items`"
                                            ))
                                        })?;
                                    let length =
                                        fields.get("length").map(Rc::clone).ok_or_else(|| {
                                            Halt::Trap(format!(
                                                "`{schema_name}::decode` FixedVec field `{field_name}` has no `length`"
                                            ))
                                        })?;
                                    (self.deref_cell(items), Some(self.deref_cell(length)))
                                }
                                _ => {
                                    return Err(Halt::Trap(format!(
                                        "`{schema_name}::decode` repeated field `{field_name}` is not a FixedVec value"
                                    )));
                                }
                            }
                        }
                    };
                    let mut decoded = 0i64;
                    if let Some(count_cell) = &count_cell {
                        *count_cell.borrow_mut() = Value::Int(0);
                    }
                    for index in 0..encoding.max_count {
                        if matches!(
                            encoding.carrier,
                            omega_typed_trees::wire::WireRepeatedCarrier::FixedVec
                        ) && cursor >= end
                        {
                            continue;
                        }
                        let raw_value = read_varint(&mut cursor, &mut ok);
                        let decoded_value = wire_decoded_scalar_value(raw_value, encoding.element)?;
                        let element_cell = match &*items_cell.borrow() {
                            Value::Array(elements) => {
                                elements.get(index).map(Rc::clone).ok_or_else(|| {
                                    Halt::Trap(format!(
                                        "`{schema_name}::decode` repeated field `{field_name}` has no element {index}"
                                    ))
                                })?
                            }
                            _ => {
                                return Err(Halt::Trap(format!(
                                    "`{schema_name}::decode` repeated field `{field_name}` is not a fixed array value"
                                )));
                            }
                        };
                        let element_cell = self.deref_cell(element_cell);
                        if range.is_none_or(|range| {
                            wire_scalar_in_range(raw_value, encoding.element, &decoded_value, range)
                        }) {
                            *element_cell.borrow_mut() = decoded_value;
                        } else {
                            ok = false;
                        }
                        decoded += 1;
                        if let Some(count_cell) = &count_cell {
                            *count_cell.borrow_mut() = Value::Int(decoded);
                        }
                    }
                    if cursor != end {
                        ok = false;
                    }
                }
            }
        }

        *read_cell.borrow_mut() = Value::Int(cursor as i64);
        // The verdict enum (`WireVerdict`): Sound on a clean decode, Invalid
        // on the first violation -- mirrors the native tag write (Invalid = 0
        // = the ZII zero case, Sound = 1). The declaring type resolves by
        // name (invalid when the program declares no WireVerdict, and the
        // name-global fallback covers it).
        *ok_cell.borrow_mut() = Value::Enum {
            type_symbol: self
                .find_data_by_name("WireVerdict")
                .map(|data| data.symbol)
                .unwrap_or_else(SymbolHandle::invalid),
            variant_name: if ok { "Sound" } else { "Invalid" }.to_owned(),
            payload: Vec::new(),
        };

        Ok(Some(Value::Unit))
    }

    fn try_host_call(&mut self, call: &TableCall, frame: &Frame) -> EvalResult<Option<Value>> {
        if !self.is_boundary_call(call, frame) {
            return Ok(None);
        }
        // Any driven host-boundary call marks the run: the build-time
        // evaluation entry uses this as a DYNAMIC purity backstop (the static
        // effect surface does not fold host-authority audit facts in yet).
        self.host_boundary_touched = true;
        let target = call.target.as_str();

        // Host dispatch is keyed on the boundary TRAIT, not the bare method
        // name: a `Filesystem` call routes to the fs handler so `File::write`
        // is not mistaken for `Console::write` (they share the leaf name).
        let receiver_trait = self.receiver_boundary_type_name(call, frame);
        if receiver_trait
            .as_deref()
            .is_some_and(|name| name.contains("Filesystem"))
        {
            let args = self
                .program
                .statement_table
                .expression_handles(call.arguments)
                .to_vec();
            if let Some(value) = self.try_filesystem_call(target, &args, frame)? {
                return Ok(Some(value));
            }
            return unsupported(format!("filesystem host call `{target}` not yet supported"));
        }

        // Everything past the Filesystem branch is a NON-fs host boundary
        // (console, exit, clock, gui) -- the granted-build backstop's line.
        // EXCEPTION (owner answer #5, 2026-07-11k): the CONSOLE WRITE family
        // is served during granted builds. The effect gate already verified
        // statically that the build machine reaches console only through
        // DECLARED stdout_io/stderr_io rows (a row-less boundary surfaces as
        // opaque `host_boundary` and refuses before evaluation starts), and
        // the granted entry flushes the buffered bytes to the compiler's
        // real streams -- "the interpreter should never just catch it".
        // Everything else keeps tripping the backstop (defense in depth
        // beneath the gate). The name family IS the interpreter's console
        // dispatch (the serve below matches the same names).
        let served_console_write = matches!(
            target,
            "write" | "write_line" | "write_error" | "write_error_line"
        );
        if !served_console_write {
            self.non_fs_host_boundary_touched = true;
        }

        let arguments = self
            .program
            .statement_table
            .expression_handles(call.arguments)
            .to_vec();

        match target {
            "exit_process" => {
                let code = if let Some(first) = arguments.first() {
                    self.eval_expression(*first, frame)?
                        .as_int()
                        .ok_or_else(|| Halt::Trap("exit_process arg not integer".to_owned()))?
                } else {
                    0
                };
                Err(Halt::Exit(code as i32))
            }
            "write" | "write_line" | "write_error" | "write_error_line" => {
                let bytes = if let Some(first) = arguments.first() {
                    let value = self.eval_expression(*first, frame)?;
                    match value {
                        Value::Str(text) => text.borrow().clone(),
                        other => {
                            return unsupported(format!(
                                "host write of non-string value {other:?}"
                            ));
                        }
                    }
                } else {
                    Vec::new()
                };
                let stream = if target.starts_with("write_error") {
                    &mut self.stderr
                } else {
                    &mut self.stdout
                };
                stream.extend_from_slice(&bytes);
                if target.ends_with("_line") {
                    stream.push(b'\n');
                }
                Ok(Some(Value::Unit))
            }
            "read_byte" => {
                // The next raw stdin byte as `ByteRead::Byte { value }`, or
                // `ByteRead::Eof` at end-of-input (Eof = ordinal 0 = the ZII
                // zero case; sentinel spellings vetoed).
                // No CRLF normalization: byte-level readers see the stream
                // as-is.
                Ok(Some(self.read_stdin_byte_value()))
            }
            "write_byte" => {
                // Append one byte (the argument's low 8 bits) to stdout.
                let byte = arguments
                    .first()
                    .and_then(|argument| self.eval_expression(*argument, frame).ok())
                    .and_then(|value| match value {
                        Value::Int(byte) => Some(byte as u8),
                        _ => None,
                    });
                match byte {
                    Some(byte) => {
                        self.stdout.push(byte);
                        Ok(Some(Value::Unit))
                    }
                    None => unsupported("write_byte expects one integer argument".to_string()),
                }
            }
            "read_line" => {
                // Read up to (and including) the next newline from the remaining stdin into
                // the mutable text-carrier out-parameter. CRLF is normalized (a trailing `\r` is
                // dropped). Returns whether a line was available (some programs ignore it).
                let line = self.read_stdin_line();
                if let Some(first) = arguments.first() {
                    if let Ok(cell) = self.resolve_place(*first, frame) {
                        let cell = self.deref_cell(cell);
                        if let Value::Str(text) = &*cell.borrow() {
                            *text.borrow_mut() = line.clone().into_bytes();
                        } else {
                            *cell.borrow_mut() = Value::str(line.clone());
                        }
                    }
                }
                Ok(Some(Value::Bool(!line.is_empty())))
            }
            // TimeHost read ops (std::time rung 4): one shared helper for both
            // statement- and value-position dispatch.
            "monotonic_ticks"
            | "monotonic_ticks_per_second"
            | "wall_clock_raw"
            | "wall_clock_units_per_second"
            | "wall_clock_epoch_offset_seconds" => Ok(self.virtual_time_host_value(target)),
            "sleep" => {
                // Frame pacing: no REAL delay in the interpreter (real time has no
                // effect on the deterministic state the differential oracle
                // compares), but the VIRTUAL clock advances by the slept
                // milliseconds -- so tick-paced programs observe the same elapsed
                // arithmetic natively (where GetTickCount64 advances across a real
                // Sleep) and virtually.
                let slept = arguments
                    .first()
                    .and_then(|argument| self.eval_expression(*argument, frame).ok())
                    .and_then(|value| match value {
                        Value::Int(ms) => Some(ms.max(1)),
                        _ => None,
                    })
                    .unwrap_or(1);
                self.virtual_ticks += slept;
                Ok(Some(Value::Unit))
            }
            "tick_count" => {
                // A VIRTUAL monotonic millisecond counter: deterministic (the
                // differential oracle compares exit codes, and tick-based
                // programs must assert MONOTONICITY, not values), advancing on
                // every read and every sleep.
                self.virtual_ticks += 1;
                Ok(Some(Value::Int(self.virtual_ticks)))
            }
            other => unsupported(format!("host boundary call `{other}` not yet supported")),
        }
    }

    /// Drive one `std::fs` boundary call against the interpreter's virtual
    /// filesystem (create/open_read/read/write/close/remove). Every op writes a
    /// ZII outcome enum into its `&mut out` argument; `File` handles carry their
    /// fd directly as the `Opened` payload. Deterministic and hermetic — no real
    /// disk touches, so the differential oracle stays reproducible.
    /// Drive a value-returning `FilesystemHost` op against the in-memory FS.
    /// `arguments` are pre-resolved by the caller (from the statement or the
    /// expression table, whichever the call site lives in). Returns `Ok(None)`
    /// if `method` is not a filesystem op, so a caller can fall through.
    fn try_filesystem_call(
        &mut self,
        method: &str,
        arguments: &[ExpressionHandle],
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        // REAL-filesystem mode (build.omg rung; opt-in via
        // `FilesystemAccess::RealUnscoped`): the whole op family routes to the
        // real provider, same `Ok(None)`-if-not-an-fs-op contract.
        if self.real_fs.is_some() {
            return self.try_real_filesystem_call(method, arguments, frame);
        }
        // Value-returning raw `FilesystemHost` ops, matching the native seam:
        // each returns its "syscall" result (fd / byte count / rc; negative on
        // error) against the deterministic in-memory filesystem.
        let result: i64 = match method {
            "create" => {
                // O_WRONLY|O_CREAT|O_TRUNC: create/truncate, writable.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                self.virtual_open(path, true, true) as i64
            }
            "open" => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let flags = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
                self.virtual_open_flags(path, flags) as i64
            }
            "open_path_handle" => {
                // Hermetic CreateFileA model for metadata/query handles. The
                // wrapper supplies access=0 + OPEN_EXISTING; the virtual fd
                // table already models both files and read-only directories.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let fd = self.virtual_open_flags(path, 0);
                if fd < 0 {
                    // `GetLastError`, not CRT errno, is the native error source.
                    self.virtual_errno = match self.virtual_errno {
                        13 => 5,   // EACCES -> ERROR_ACCESS_DENIED
                        9 => 6,    // EBADF -> ERROR_INVALID_HANDLE
                        17 => 183, // EEXIST -> ERROR_ALREADY_EXISTS
                        _ => 2,    // ERROR_FILE_NOT_FOUND
                    };
                }
                fd as i64
            }
            "open_create" => {
                // `open(path, flags, mode)` with O_CREAT (Rust `File::create_new`,
                // `OpenOptions.create`/`.create_new`). Flag bits are the HOST's
                // (host_open_flags, mirroring the checked target encoder). This
                // adds the O_EXCL/EEXIST atomic
                // create-new guard + create-mode recording; every other flag bit
                // (O_TRUNC/O_APPEND/access/EACCES/ENOENT) is handled by the shared
                // `virtual_open_flags`, so `open_create` cleanly SUBSUMES `open`.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let flags = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
                let mode = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as u32;
                let exists = self.virtual_files.contains_key(&path)
                    || self.virtual_dirs.contains(&path)
                    || self.virtual_char_devices.contains(&path);
                if host_open_flags::o_creat(flags) && host_open_flags::o_excl(flags) && exists {
                    self.virtual_errno = 17; // EEXIST (O_CREAT|O_EXCL, path present)
                    -1
                } else {
                    // Whether this call actually creates the file (records the mode
                    // AFTER the open so the create's own access is not gated by it).
                    let created = host_open_flags::o_creat(flags) && !exists;
                    let fd = self.virtual_open_flags(path.clone(), flags);
                    if fd >= 0 && created {
                        self.virtual_perms.insert(path, mode & 0o777);
                    }
                    fd as i64
                }
            }
            "read" => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let count = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as usize;
                match self.virtual_read_n(fd, count) {
                    Some(bytes) => {
                        let n = bytes.len() as i64;
                        self.write_fs_buffer(arguments.get(1).copied(), frame, &bytes);
                        n
                    }
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            "write" => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let bytes = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                match self.virtual_write(fd, &bytes) {
                    Some(count) => count as i64,
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            "read_at" => {
                // `pread(fd, buf, count, offset)`: read at an absolute offset
                // WITHOUT moving the cursor (Rust `FileExt::read_at`).
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let count = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as usize;
                let offset = self.eval_fs_scalar(arguments.get(3).copied(), frame)?;
                match self.virtual_read_at(fd, offset, count) {
                    Some(bytes) => {
                        let n = bytes.len() as i64;
                        self.write_fs_buffer(arguments.get(1).copied(), frame, &bytes);
                        n
                    }
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            "write_at" => {
                // `pwrite(fd, buf, count, offset)`: write at an absolute offset
                // WITHOUT moving the cursor (Rust `FileExt::write_at`).
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let bytes = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                let offset = self.eval_fs_scalar(arguments.get(2).copied(), frame)?;
                match self.virtual_write_at(fd, offset, &bytes) {
                    Some(count) => count as i64,
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            "close" => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                if self.virtual_fds.remove(&fd).is_some() {
                    // Closing the owning fd releases any advisory lock it held.
                    self.virtual_flocks.retain(|_, owner| *owner != fd);
                    0
                } else {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
            }
            "close_handle" => {
                let handle = self.eval_fs_fd(arguments.first().copied(), frame)?;
                if self.virtual_fds.remove(&handle).is_some() {
                    self.virtual_flocks.retain(|_, owner| *owner != handle);
                    1 // Win32 BOOL success
                } else {
                    self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                    0
                }
            }
            "duplicate" => {
                // `dup(fd)`: mint a fresh descriptor over the same open file (Rust
                // `File::try_clone`). Native dup SHARES the underlying file offset;
                // the hermetic model gives the clone its OWN cursor snapshotted from
                // the source (independent thereafter) -- faithful for the common
                // clone-then-use pattern, where the clone's offset starts where the
                // source's was. EBADF for an unknown fd.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let clone = self.virtual_fds.get(&fd).map(|descriptor| VirtualFd {
                    path: descriptor.path.clone(),
                    cursor: descriptor.cursor,
                    writable: descriptor.writable,
                    is_dir: descriptor.is_dir,
                });
                match clone {
                    Some(clone) => {
                        let new_fd = self.virtual_next_fd;
                        self.virtual_next_fd += 1;
                        self.virtual_fds.insert(new_fd, clone);
                        new_fd as i64
                    }
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            "lock_file" => {
                // `flock(fd, operation)`: advisory whole-file lock (Rust
                // `File::lock`/`lock_shared`/`try_lock`/`unlock`). operation
                // bitmask: LOCK_SH=1, LOCK_EX=2, LOCK_NB=4, LOCK_UN=8. The
                // hermetic model tracks EXCLUSIVE ownership per path; a
                // non-blocking acquire on a path another fd holds is EWOULDBLOCK.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let operation = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
                let path = self
                    .virtual_fds
                    .get(&fd)
                    .map(|descriptor| descriptor.path.clone());
                match path {
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                    Some(path) if operation & 8 != 0 => {
                        // LOCK_UN: release this fd's lock (a no-op if it held none).
                        if self.virtual_flocks.get(&path) == Some(&fd) {
                            self.virtual_flocks.remove(&path);
                        }
                        0
                    }
                    Some(path) => {
                        let held_by_other = matches!(
                            self.virtual_flocks.get(&path),
                            Some(owner) if *owner != fd
                        );
                        if held_by_other && operation & 4 != 0 {
                            self.virtual_errno = 35; // EWOULDBLOCK (== EAGAIN)
                            -1
                        } else {
                            self.virtual_flocks.insert(path, fd);
                            0
                        }
                    }
                }
            }
            "lock_file_ex" => {
                // Win32 LockFileEx over the synthetic fd/HANDLE. flags:
                // EXCLUSIVE=2, FAIL_IMMEDIATELY=1. The range/OVERLAPPED
                // arguments are ABI-shape inputs; the std wrapper always asks
                // for offset zero and the whole file.
                let fd = self.eval_fs_scalar(arguments.first().copied(), frame)? as i32;
                let flags = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
                let path = self
                    .virtual_fds
                    .get(&fd)
                    .map(|descriptor| descriptor.path.clone());
                match path {
                    None => {
                        self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                        0
                    }
                    Some(path) => {
                        let held_by_other = matches!(
                            self.virtual_flocks.get(&path),
                            Some(owner) if *owner != fd
                        );
                        if held_by_other && flags & 1 != 0 {
                            self.virtual_errno = 33; // ERROR_LOCK_VIOLATION
                            0
                        } else {
                            self.virtual_flocks.insert(path, fd);
                            1
                        }
                    }
                }
            }
            "unlock_file" => {
                let fd = self.eval_fs_scalar(arguments.first().copied(), frame)? as i32;
                let path = self
                    .virtual_fds
                    .get(&fd)
                    .map(|descriptor| descriptor.path.clone());
                match path {
                    None => {
                        self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                        0
                    }
                    Some(path) if self.virtual_flocks.get(&path) == Some(&fd) => {
                        self.virtual_flocks.remove(&path);
                        1
                    }
                    Some(_) => {
                        self.virtual_errno = 158; // ERROR_NOT_LOCKED
                        0
                    }
                }
            }
            "get_last_error" => i64::from(self.virtual_errno),
            // `remove_name` is the TRUSTED plain-path twin (D-at trust class,
            // the create_dir_name precedent): the arg bytes ARE the path, so
            // both spellings share one model.
            "remove" | "remove_name" => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                if self.virtual_files.remove(&path).is_some() {
                    0
                } else {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
            "seek" => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let offset = self.eval_fs_scalar(arguments.get(1).copied(), frame)?;
                let whence = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as i32;
                match self.virtual_seek(fd, offset, whence) {
                    Some(position) => position,
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            "set_len" => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let length = self.eval_fs_scalar(arguments.get(1).copied(), frame)?;
                let rc = self.virtual_set_len(fd, length);
                if rc < 0 {
                    self.virtual_errno = 9; // EBADF
                }
                rc
            }
            "set_file_permissions" => {
                // `fchmod(fd, mode)`: record the mode against the fd's path so a
                // subsequent write-open sees it (mirrors path-based chmod). EBADF
                // if the descriptor is unknown.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let mode = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as u32;
                match self.virtual_fds.get(&fd) {
                    Some(descriptor) => {
                        let path = descriptor.path.clone();
                        self.virtual_perms.insert(path, mode);
                        0
                    }
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            "set_file_times" => {
                // `futimens(fd, times)`: `times` is two packed `struct timespec`
                // (atime then mtime, {tv_sec i64, tv_nsec i64} each). Read the
                // modification seconds -- times[1].tv_sec at byte offset 16 -- and
                // record it against the fd's path so stat/fstat report it. EBADF if
                // the descriptor is unknown.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let times = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                match self.virtual_fds.get(&fd) {
                    Some(descriptor) => {
                        let path = descriptor.path.clone();
                        let mtime = times
                            .get(16..24)
                            .and_then(|s| <[u8; 8]>::try_from(s).ok())
                            .map(i64::from_le_bytes)
                            .unwrap_or(0);
                        self.virtual_times.insert(path, mtime);
                        0
                    }
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            "sync" | "sync_data" => {
                // `fsync(fd)`: flush to durable storage (`sync_data` aliases it --
                // macOS has no `fdatasync`). In the hermetic in-memory FS the bytes
                // are already "durable", so this is a no-op that only validates the
                // descriptor: 0 for a live fd, -1 (EBADF) otherwise -- matching the
                // native seam's contract.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                i64::from(self.virtual_fds.contains_key(&fd)) - 1
            }
            "errno" => {
                // `read_errno()` (darwin `___error()` deref): the thread-local
                // errno set by the most recent failing op. Not cleared on
                // success (POSIX), so it is only meaningful right after a -1.
                i64::from(self.virtual_errno)
            }
            // The trusted plain-name variant shares create_dir's semantics
            // (the arg bytes ARE the path -- the scratch subslice excludes
            // the native NUL, so both engines see identical bytes).
            "create_dir" | "create_dir_name" => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                // -1 (EEXIST) if the dir already exists.
                if self.virtual_dirs.insert(path) {
                    0
                } else {
                    self.virtual_errno = 17; // EEXIST
                    -1
                }
            }
            "remove_dir" | "remove_dir_name" => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                if self.virtual_dirs.remove(&path) {
                    0
                } else {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
            "open_at" => {
                // `openat(dirfd, name, flags)`: open `name` relative to the open
                // directory `dirfd`. The full path (dirfd's path + "/" + name) is
                // joined HERE (the OS does it natively), so no Omega path build.
                let dirfd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let name = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                let flags = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as i32;
                match self.virtual_at_path(dirfd, &name) {
                    Some(full) => self.virtual_open_flags(full, flags) as i64,
                    None => {
                        self.virtual_errno = 9; // EBADF (dirfd not an open directory)
                        -1
                    }
                }
            }
            "unlink_at" => {
                // `unlinkat(dirfd, name, flags)`: remove `name` relative to `dirfd`.
                // flags & AT_REMOVEDIR(0x80) removes an empty directory, else a file.
                let dirfd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let name = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                let flags = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as i32;
                match self.virtual_at_path(dirfd, &name) {
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                    Some(full) => {
                        let removed = if (flags & 128) != 0 {
                            self.virtual_dirs.remove(&full)
                        } else {
                            self.virtual_files.remove(&full).is_some()
                        };
                        if removed {
                            0
                        } else {
                            self.virtual_errno = 2; // ENOENT
                            -1
                        }
                    }
                }
            }
            "set_permissions" => {
                // `chmod(path, mode)`: record the mode. ENOENT if the path names
                // neither a file nor a directory. `mode` is the second arg.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let mode = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as u32;
                if self.virtual_files.contains_key(&path) || self.virtual_dirs.contains(&path) {
                    self.virtual_perms.insert(path, mode);
                    0
                } else {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
            "change_owner" | "change_owner_no_follow" => {
                // `chown`/`lchown(path, uid, gid)`: change owner/group. ENOENT if
                // the path is absent. The hermetic model's process identity is
                // VIRTUAL_UID/GID (a normal, non-root user), so only a NO-OP change
                // is permitted: a uid/gid of -1 leaves that component alone, and
                // setting the CURRENT owner succeeds; any OTHER owner is EPERM --
                // exactly what native `chown` does when run as a normal user.
                // (`lchown` differs from `chown` only on symlinks, which the
                // hermetic FS never follows on ownership ops, so they behave
                // identically here.)
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let uid = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
                let gid = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as i32;
                let exists = self.virtual_files.contains_key(&path)
                    || self.virtual_dirs.contains(&path)
                    || self.virtual_symlinks.contains_key(&path);
                if !exists {
                    self.virtual_errno = 2; // ENOENT
                    -1
                } else {
                    self.virtual_chown_result(uid, gid)
                }
            }
            "change_file_owner" => {
                // `fchown(fd, uid, gid)`: like `chown` by descriptor. EBADF for an
                // unknown fd; otherwise the same non-root ownership rule.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let uid = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
                let gid = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as i32;
                if self.virtual_fds.contains_key(&fd) {
                    self.virtual_chown_result(uid, gid)
                } else {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
            }
            "rename" => {
                let from = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let to = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                match self.virtual_files.remove(&from) {
                    Some(content) => {
                        self.virtual_files.insert(to, content);
                        0
                    }
                    None => {
                        self.virtual_errno = 2; // ENOENT
                        -1
                    }
                }
            }
            "hard_link" => {
                // `link(original, link)`: a second name for the same inode.
                // ENOENT if the original is absent; EEXIST if the link name is
                // taken. The hermetic FS has no inodes, so this COPIES the bytes
                // (approximate: a later write to one name won't show in the
                // other — see TASKS_FS.md). Enough to model create+readback.
                let original = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let link = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                if self.virtual_files.contains_key(&link) || self.virtual_dirs.contains(&link) {
                    self.virtual_errno = 17; // EEXIST
                    -1
                } else if let Some(content) = self.virtual_files.get(&original).cloned() {
                    self.virtual_files.insert(link, content);
                    0
                } else {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
            "create_hard_link" => {
                // `CreateHardLinkA(link, existing, security)` -- the WINDOWS
                // hard-link primitive (session slice 3): the ARG ORDER is
                // (new link, existing), REVERSED from `hard_link`, and the
                // result is BOOL (1 success / 0 failure). Same hermetic
                // copy-the-bytes model as `hard_link` above. virtual_errno is
                // also the provider's Win32 last-error slot for GetLastError.
                let link = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let existing = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                if self.virtual_files.contains_key(&link) || self.virtual_dirs.contains(&link) {
                    self.virtual_errno = 183; // ERROR_ALREADY_EXISTS
                    0
                } else if let Some(content) = self.virtual_files.get(&existing).cloned() {
                    self.virtual_files.insert(link, content);
                    1
                } else {
                    self.virtual_errno = 2; // ERROR_FILE_NOT_FOUND
                    0
                }
            }
            "get_osfhandle" => {
                // `_get_osfhandle(fd)` -- the fd -> HANDLE bridge (session
                // slice 4a). The hermetic model's handles ARE its fds
                // (identity), so consumers key the same descriptor table;
                // -2 (msvcrt's bad-fd spelling) for an unknown fd.
                let fd = self.eval_fs_scalar(arguments.first().copied(), frame)? as i32;
                if self.virtual_fds.contains_key(&fd) {
                    i64::from(fd)
                } else {
                    -2
                }
            }
            "final_path_name_by_handle" => {
                // `GetFinalPathNameByHandleA(handle, buffer, capacity, flags)`:
                // resolve an OPEN handle to its final path. The hermetic
                // model's canonical path IS the descriptor's stored key
                // (already absolute for its namespace; no drive letters or
                // \\?\ prefixes to synthesize), NUL-terminated into the
                // buffer. Win32 return contract: the length WITHOUT the NUL
                // when it fits, the REQUIRED size INCLUDING the NUL when the
                // capacity is too small, 0 for a bad handle (GetLastError
                // semantics -- no errno touched).
                let handle = self.eval_fs_scalar(arguments.first().copied(), frame)?;
                let capacity = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as usize;
                let path = self
                    .virtual_fds
                    .get(&(handle as i32))
                    .map(|descriptor| descriptor.path.clone());
                match path {
                    Some(path) => {
                        if path.len() + 1 <= capacity {
                            let mut bytes = path.clone();
                            bytes.push(0);
                            self.write_fs_buffer(arguments.get(1).copied(), frame, &bytes);
                            path.len() as i64
                        } else {
                            (path.len() + 1) as i64
                        }
                    }
                    None => {
                        self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                        0
                    }
                }
            }
            "set_file_time" => {
                // `SetFileTime(handle, creation, access_ft, write_ft)` (session
                // slice 4b): stamp the handle's path with the WRITE time from
                // its 8-byte FILETIME buffer (100ns units since 1601 -> unix
                // seconds via the calibration constants), the same
                // virtual_times store `set_file_times` uses. BOOL result;
                // 0 for a bad handle (GetLastError semantics -- no errno).
                let handle = self.eval_fs_scalar(arguments.first().copied(), frame)?;
                let write_ft = self.eval_fs_bytes(arguments.get(3).copied(), frame)?;
                match self.virtual_fds.get(&(handle as i32)) {
                    Some(descriptor) => {
                        let path = descriptor.path.clone();
                        let filetime = write_ft
                            .get(0..8)
                            .and_then(|s| <[u8; 8]>::try_from(s).ok())
                            .map(i64::from_le_bytes)
                            .unwrap_or(0);
                        let secs = filetime / 10_000_000 - 11_644_473_600;
                        self.virtual_times.insert(path, secs);
                        1
                    }
                    None => {
                        self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                        0
                    }
                }
            }
            "symlink" => {
                // `symlink(target, linkpath)`: record the link -> target mapping.
                // EEXIST if the link name already names a file/dir/symlink.
                let target = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let link = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                if self.virtual_files.contains_key(&link)
                    || self.virtual_dirs.contains(&link)
                    || self.virtual_symlinks.contains_key(&link)
                {
                    self.virtual_errno = 17; // EEXIST
                    -1
                } else {
                    self.virtual_symlinks.insert(link, target);
                    0
                }
            }
            "read_link" => {
                // `readlink(path, buf, count)`: write the target bytes into the
                // buffer (up to `count`), returning the number written. ENOENT if
                // `path` is not a symlink in the hermetic model.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let count = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as usize;
                match self.virtual_symlinks.get(&path).cloned() {
                    Some(target) => {
                        let n = target.len().min(count);
                        self.write_fs_buffer(arguments.get(1).copied(), frame, &target[..n]);
                        n as i64
                    }
                    None => {
                        self.virtual_errno = 2; // ENOENT
                        -1
                    }
                }
            }
            "canonicalize" => {
                // `realpath(path, buf)`: resolve `path` to its canonical absolute
                // form and write it NUL-terminated into the buffer. The hermetic FS
                // is already absolute and does not resolve `.`/`..`; it follows one
                // symlink level (matching `read_link`). Returns a non-zero success
                // flag (native returns the resolved-buffer pointer) or 0 (NULL) +
                // ENOENT when the target does not exist.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let resolved = self.virtual_symlinks.get(&path).cloned().unwrap_or(path);
                let exists = self.virtual_files.contains_key(&resolved)
                    || self.virtual_dirs.contains(&resolved);
                if exists {
                    let mut bytes = resolved;
                    bytes.push(0); // NUL-terminate like realpath's C string
                    self.write_fs_buffer(arguments.get(1).copied(), frame, &bytes);
                    1
                } else {
                    self.virtual_errno = 2; // ENOENT
                    0
                }
            }
            "read_dir" => {
                // `read_dir(fd, buf, count, &position)`: on the first call
                // (position == 0) pack the directory's entries as darwin `dirent`
                // records (`.`, `..`, then each immediate child) into the buffer
                // and set `position`; a later call returns 0 (end). The record
                // layout matches native `___getdirentries64` so a parser is
                // identical on both engines.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let dir_path = self
                    .virtual_fds
                    .get(&fd)
                    .filter(|descriptor| descriptor.is_dir)
                    .map(|descriptor| descriptor.path.clone());
                match dir_path {
                    None => {
                        // Unknown fd -> EBADF; a live non-dir fd -> ENOTDIR.
                        self.virtual_errno = if self.virtual_fds.contains_key(&fd) {
                            20 // ENOTDIR
                        } else {
                            9 // EBADF
                        };
                        -1
                    }
                    Some(path) => {
                        let count = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as usize;
                        let position = self.read_fs_position(arguments.get(3).copied(), frame);
                        if position != 0 {
                            0
                        } else {
                            let records = self.build_dirent_records(&path);
                            let n = records.len().min(count);
                            self.write_fs_buffer(arguments.get(1).copied(), frame, &records[..n]);
                            // Any non-zero marker so the next call reports end.
                            self.write_fs_position(
                                arguments.get(3).copied(),
                                frame,
                                n.max(1) as i64,
                            );
                            n as i64
                        }
                    }
                }
            }
            "find_first" => {
                // `find_first(pattern, &data)` -- the windows dir-walk seam (fs
                // rung 3a). `pattern` is `dir/*`: the impl joins with `/`, which
                // Win32 accepts natively and which matches the hermetic FS keys
                // byte-exactly. Snapshot the directory's entries (".", "..",
                // then the immediate children -- the same set read_dir packs)
                // into a cursor keyed by a fresh handle, fill the FIRST entry's
                // find-data record, and return the handle; -1
                // (INVALID_HANDLE_VALUE, ENOENT) when the directory does not
                // exist. A real directory always yields "." first, so an open
                // enumeration always has a first entry -- exactly Win32.
                let pattern = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let entries = pattern
                    .strip_suffix(b"/*")
                    .filter(|dir_path| self.virtual_dirs.contains(*dir_path))
                    .map(|dir_path| self.build_find_entries(dir_path));
                match entries {
                    Some(mut entries) => {
                        let (name, is_dir) =
                            entries.pop_front().expect("dot entries are always present");
                        self.write_find_data(arguments.get(1).copied(), frame, &name, is_dir);
                        let handle = self.virtual_next_find;
                        self.virtual_next_find += 1;
                        self.virtual_finds.insert(handle, entries);
                        handle
                    }
                    None => {
                        self.virtual_errno = 2; // ENOENT
                        -1
                    }
                }
            }
            "find_next" => {
                // `find_next(handle, &data)`: fill the next snapshotted entry
                // (1 = filled, 0 = end-of-enumeration or unknown handle).
                let handle = self.eval_fs_scalar(arguments.first().copied(), frame)?;
                match self
                    .virtual_finds
                    .get_mut(&handle)
                    .and_then(std::collections::VecDeque::pop_front)
                {
                    Some((name, is_dir)) => {
                        self.write_find_data(arguments.get(1).copied(), frame, &name, is_dir);
                        1
                    }
                    None => 0,
                }
            }
            "find_close" => {
                // `find_close(handle)`: release the cursor (BOOL, like Win32).
                let handle = self.eval_fs_scalar(arguments.first().copied(), frame)?;
                if self.virtual_finds.remove(&handle).is_some() {
                    1
                } else {
                    0
                }
            }
            "read_metadata" => {
                // `stat(path, buf)`: fill the buffer's st_mode (off 4, u16) and
                // st_size (off 96, i64) as the darwin kernel would. A regular
                // file is S_IFREG(0o100000)|0o644 with size = content length; a
                // directory is S_IFDIR(0o040000)|0o755 size 0. ENOENT otherwise.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                // st_mode = format bits (S_IFREG/S_IFDIR) | permission bits, so
                // a prior `set_permissions` (chmod) shows through `readonly()`.
                let chmod_perm = self
                    .virtual_perms
                    .get(&path)
                    .map(|mode| (*mode as u16) & 0o7777);
                let meta = if self.virtual_char_devices.contains(&path) {
                    // A character-special device (`/dev/null`): S_IFCHR|0o666, size 0.
                    Some((0o020_000u16 | chmod_perm.unwrap_or(0o666), 0i64))
                } else if let Some(content) = self.virtual_files.get(&path) {
                    let size = content.len() as i64;
                    Some((0o100_000u16 | chmod_perm.unwrap_or(0o644), size))
                } else if self.virtual_dirs.contains(&path) {
                    Some((0o040_000u16 | chmod_perm.unwrap_or(0o755), 0i64))
                } else {
                    None
                };
                match meta {
                    Some((mode, size)) => {
                        // A `set_file_times` mtime shows through; otherwise the
                        // hermetic FS has no clock, so it reports a fixed modeled
                        // mtime (native `stat` returns the real time -- tests assert
                        // exact == in the interpreter and a lower bound natively).
                        let mtime = self
                            .virtual_times
                            .get(&path)
                            .copied()
                            .unwrap_or(VIRTUAL_MTIME_SECS);
                        self.write_fs_stat(arguments.get(1).copied(), frame, mode, size, mtime);
                        0
                    }
                    None => {
                        self.virtual_errno = 2; // ENOENT
                        -1
                    }
                }
            }
            "read_file_metadata" => {
                // `fstat(fd, buf)`: like `stat` but keyed by an OPEN descriptor. Map
                // the fd to its path, then fill the same stat record (a held `File`
                // is always a regular file here). EBADF for an unknown fd. Never
                // touches the cursor.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let path = self
                    .virtual_fds
                    .get(&fd)
                    .map(|descriptor| descriptor.path.clone());
                let meta = path.and_then(|path| {
                    // A `set_file_times` mtime shows through; else the modeled epoch.
                    let mtime = self
                        .virtual_times
                        .get(&path)
                        .copied()
                        .unwrap_or(VIRTUAL_MTIME_SECS);
                    let chmod_perm = self
                        .virtual_perms
                        .get(&path)
                        .map(|mode| (*mode as u16) & 0o7777);
                    if let Some(content) = self.virtual_files.get(&path) {
                        Some((
                            0o100_000u16 | chmod_perm.unwrap_or(0o644),
                            content.len() as i64,
                            mtime,
                        ))
                    } else if self.virtual_dirs.contains(&path) {
                        Some((0o040_000u16 | chmod_perm.unwrap_or(0o755), 0i64, mtime))
                    } else {
                        None
                    }
                });
                match meta {
                    Some((mode, size, mtime)) => {
                        self.write_fs_stat(arguments.get(1).copied(), frame, mode, size, mtime);
                        0
                    }
                    None => {
                        self.virtual_errno = 9; // EBADF (unknown descriptor)
                        -1
                    }
                }
            }
            "read_symlink_metadata" => {
                // `lstat(path, buf)`: like `stat`, but does NOT follow a final
                // symlink. A symlink reports S_IFLNK(0o120000)|0o777 with size =
                // the target path length (POSIX: a symlink's size is its target's
                // byte length); everything else is identical to `stat`.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let meta = if let Some(target) = self.virtual_symlinks.get(&path) {
                    Some((0o120_000u16 | 0o777, target.len() as i64))
                } else {
                    let chmod_perm = self
                        .virtual_perms
                        .get(&path)
                        .map(|mode| (*mode as u16) & 0o7777);
                    if let Some(content) = self.virtual_files.get(&path) {
                        Some((
                            0o100_000u16 | chmod_perm.unwrap_or(0o644),
                            content.len() as i64,
                        ))
                    } else if self.virtual_dirs.contains(&path) {
                        Some((0o040_000u16 | chmod_perm.unwrap_or(0o755), 0i64))
                    } else {
                        None
                    }
                };
                match meta {
                    Some((mode, size)) => {
                        self.write_fs_stat(
                            arguments.get(1).copied(),
                            frame,
                            mode,
                            size,
                            VIRTUAL_MTIME_SECS,
                        );
                        0
                    }
                    None => {
                        self.virtual_errno = 2; // ENOENT
                        -1
                    }
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(Value::Int(result)))
    }

    /// Evaluate an argument to an integer scalar (fd / flags / offset / count).
    fn eval_fs_scalar(
        &mut self,
        argument: Option<ExpressionHandle>,
        frame: &Frame,
    ) -> EvalResult<i64> {
        let Some(argument) = argument else {
            return Ok(0);
        };
        Ok(self.eval_expression(argument, frame)?.as_int().unwrap_or(0))
    }

    /// Evaluate an argument expected to be byte data (a path or a write payload).
    fn eval_fs_bytes(
        &mut self,
        argument: Option<ExpressionHandle>,
        frame: &Frame,
    ) -> EvalResult<Vec<u8>> {
        let Some(argument) = argument else {
            return Ok(Vec::new());
        };
        match self.eval_expression(argument, frame)? {
            Value::Str(text) => Ok(text.borrow().clone()),
            // A byte array or a subslice view of one (`buffer` / `buffer[0..n]`):
            // each element cell holds a byte as an `Int`. This is the write-side
            // mirror of `write_fs_buffer`'s `Array` arm, and lets a caller write
            // a bounded prefix of a buffer (Rust `fs::copy`, `write` of a slice).
            Value::Array(cells) => {
                let mut bytes = Vec::with_capacity(cells.len());
                for cell in &cells {
                    bytes.push(cell.borrow().as_int().unwrap_or(0) as u8);
                }
                Ok(bytes)
            }
            // `&mut buffer` / `&buffer`: a reference to a caller field/local (e.g. a
            // `set_file_times` timespec buffer built in place). Deref to the array.
            Value::Ref(target) => {
                if let Value::Array(cells) = &*target.borrow() {
                    let mut bytes = Vec::with_capacity(cells.len());
                    for cell in cells {
                        bytes.push(cell.borrow().as_int().unwrap_or(0) as u8);
                    }
                    Ok(bytes)
                } else {
                    unsupported("filesystem call expected byte data behind a reference".to_owned())
                }
            }
            other => unsupported(format!("filesystem call expected byte data, got {other:?}")),
        }
    }

    /// Evaluate a `File` handle argument to its raw descriptor. The interpreter
    /// carries the fd directly (see the `Opened` construction), but a wrapping
    /// single-field struct is accepted defensively.
    fn eval_fs_fd(&mut self, argument: Option<ExpressionHandle>, frame: &Frame) -> EvalResult<i32> {
        let Some(argument) = argument else {
            return trap("filesystem call missing file handle");
        };
        let value = self.eval_expression(argument, frame)?;
        let fd = match &value {
            Value::Struct { fields, .. } => {
                fields.get("fd").and_then(|cell| cell.borrow().as_int())
            }
            other => other.as_int(),
        };
        fd.map(|fd| fd as i32)
            .ok_or_else(|| Halt::Trap("filesystem call file handle is not an fd".to_owned()))
    }

    /// Copy read bytes into a caller `&mut [u8]` buffer (a text carrier or a byte
    /// array), truncated to the buffer's length. Best-effort: the outcome's
    /// `count` is authoritative; an unrecognized buffer shape is left untouched.
    fn write_fs_buffer(&mut self, argument: Option<ExpressionHandle>, frame: &Frame, bytes: &[u8]) {
        let Some(argument) = argument else {
            return;
        };
        let Ok(cell) = self.resolve_place(argument, frame) else {
            return;
        };
        let cell = self.deref_cell(cell);
        let shape = cell.borrow().clone();
        match shape {
            Value::Str(text) => {
                *text.borrow_mut() = bytes.to_vec();
            }
            Value::Array(cells) => {
                let count = bytes.len().min(cells.len());
                for (slot, byte) in cells.iter().zip(bytes.iter()).take(count) {
                    *slot.borrow_mut() = Value::Int(*byte as i64);
                }
            }
            _ => {}
        }
    }

    /// Read the current value of a `&mut i64` argument (the in/out `position`
    /// cursor of `read_dir`), 0 if unresolvable.
    fn read_fs_position(&mut self, argument: Option<ExpressionHandle>, frame: &Frame) -> i64 {
        let Some(argument) = argument else {
            return 0;
        };
        let Ok(cell) = self.resolve_place(argument, frame) else {
            return 0;
        };
        let value = self.deref_cell(cell).borrow().as_int().unwrap_or(0);
        value
    }

    /// Write back a `&mut i64` argument (the in/out `position` cursor).
    fn write_fs_position(&mut self, argument: Option<ExpressionHandle>, frame: &Frame, value: i64) {
        let Some(argument) = argument else {
            return;
        };
        let Ok(cell) = self.resolve_place(argument, frame) else {
            return;
        };
        *self.deref_cell(cell).borrow_mut() = Value::Int(value);
    }

    /// Build the packed darwin `dirent` records for a directory: `.` and `..`
    /// then each IMMEDIATE child (files in `virtual_files`, subdirs in
    /// `virtual_dirs` directly under `dir_path/`). Each record is
    /// `[d_ino(8) d_seekoff(8) d_reclen@16(u16) d_namlen@18(u16) d_type@20(u8)
    /// d_name@21(namlen) NUL pad]`, `d_reclen = round_up_8(25 + namlen)` — the
    /// exact layout `___getdirentries64` produces, so byte counts and a parser
    /// agree with native.
    /// Resolve `name` RELATIVE to the open directory `dirfd` to a full virtual
    /// path (`dirfd`'s path + "/" + name). Returns None if `dirfd` is not an open
    /// directory descriptor. The `*at` ops do their path-joining here -- in Rust,
    /// the way the OS does natively -- so the Omega layer never builds a path.
    fn virtual_at_path(&self, dirfd: i32, name: &[u8]) -> Option<Vec<u8>> {
        let dir = self
            .virtual_fds
            .get(&dirfd)
            .filter(|descriptor| descriptor.is_dir)
            .map(|descriptor| descriptor.path.clone())?;
        let mut full = dir;
        full.push(b'/');
        full.extend_from_slice(name);
        Some(full)
    }

    /// The find-enumeration twin of `build_dirent_records` (fs rung 3a): the
    /// same entry set (".", "..", then the immediate children of `dir_path`)
    /// as (name, is_dir) pairs for a `find_first` cursor snapshot.
    fn build_find_entries(&self, dir_path: &[u8]) -> std::collections::VecDeque<(Vec<u8>, bool)> {
        let mut entries: std::collections::VecDeque<(Vec<u8>, bool)> =
            std::collections::VecDeque::from([(b".".to_vec(), true), (b"..".to_vec(), true)]);
        let mut prefix = dir_path.to_vec();
        prefix.push(b'/');
        let immediate_child = |path: &[u8]| -> Option<Vec<u8>> {
            let rest = path.strip_prefix(prefix.as_slice())?;
            if rest.is_empty() || rest.contains(&b'/') {
                None
            } else {
                Some(rest.to_vec())
            }
        };
        for path in self.virtual_files.keys() {
            if let Some(name) = immediate_child(path) {
                entries.push_back((name, false));
            }
        }
        for path in &self.virtual_dirs {
            if let Some(name) = immediate_child(path) {
                entries.push_back((name, true));
            }
        }
        entries
    }

    /// Fill a caller find-data buffer (`&mut [u8]`, >= 320 bytes) the way
    /// `FindFirstFileA`/`FindNextFileA` write WIN32_FIND_DATAA: file
    /// attributes u32 little-endian at byte 0 (FILE_ATTRIBUTE_DIRECTORY 0x10 /
    /// FILE_ATTRIBUTE_NORMAL 0x80) and the NUL-terminated entry name at byte
    /// 44. Other fields are left zero.
    fn write_find_data(
        &mut self,
        argument: Option<ExpressionHandle>,
        frame: &Frame,
        name: &[u8],
        is_dir: bool,
    ) {
        let mut record = vec![0u8; 320];
        let attributes: u32 = if is_dir { 0x10 } else { 0x80 };
        record[0..4].copy_from_slice(&attributes.to_le_bytes());
        let name_len = name.len().min(259);
        record[44..44 + name_len].copy_from_slice(&name[..name_len]);
        self.write_fs_buffer(argument, frame, &record);
    }

    fn build_dirent_records(&self, dir_path: &[u8]) -> Vec<u8> {
        let mut entries: Vec<(Vec<u8>, u8)> = vec![(b".".to_vec(), 4), (b"..".to_vec(), 4)];
        let mut prefix = dir_path.to_vec();
        prefix.push(b'/');
        let immediate_child = |path: &[u8]| -> Option<Vec<u8>> {
            let rest = path.strip_prefix(prefix.as_slice())?;
            if rest.is_empty() || rest.contains(&b'/') {
                None
            } else {
                Some(rest.to_vec())
            }
        };
        for path in self.virtual_files.keys() {
            if let Some(name) = immediate_child(path) {
                entries.push((name, 8)); // DT_REG
            }
        }
        for path in &self.virtual_dirs {
            if let Some(name) = immediate_child(path) {
                entries.push((name, 4)); // DT_DIR
            }
        }
        pack_dirent_records(&entries)
    }

    /// Fill a caller stat buffer (`&mut [u8]` of at least 144 bytes) the way the
    /// darwin kernel writes `struct stat`: `st_mode` (u16) at byte offset 4 and
    /// `st_size` (i64) at byte offset 96, both little-endian. The Omega layer
    /// reads those fields back with byte-assembly. Other fields are left zero.
    fn write_fs_stat(
        &mut self,
        argument: Option<ExpressionHandle>,
        frame: &Frame,
        mode: u16,
        size: i64,
        mtime_secs: i64,
    ) {
        let Some(argument) = argument else {
            return;
        };
        let Ok(cell) = self.resolve_place(argument, frame) else {
            return;
        };
        let cell = self.deref_cell(cell);
        if let Value::Array(cells) = &*cell.borrow() {
            let put = |offset: usize, byte: u8| {
                if let Some(slot) = cells.get(offset) {
                    *slot.borrow_mut() = Value::Int(i64::from(byte));
                }
            };
            // Lay the fields out at the HOST target's stat offsets (mirrors the
            // selected StatLayout policy the wrapper projects). On
            // windows the width-mismatched/absent fields go to a synthetic tail; a
            // real native `_stat64` would leave that tail zero.
            use host_stat_offsets as off;
            put(off::MODE, (mode & 0xff) as u8);
            put(off::MODE + 1, (mode >> 8) as u8);
            // st_nlink: the hermetic FS models a fixed link count of 1 -- it does not
            // track hard-link groups (its `hard_link` copies bytes), so every path
            // reports 1. Native `stat` returns the real count (2 after a `hard_link`);
            // that case is asserted only in the native canary.
            put(off::NLINK, 1);
            put(off::NLINK + 1, 0);
            for i in 0..8 {
                put(off::INO + i, (VIRTUAL_INO >> (8 * i)) as u8);
                put(off::ATIME + i, (VIRTUAL_ATIME_SECS >> (8 * i)) as u8);
                put(off::MTIME + i, (mtime_secs >> (8 * i)) as u8);
                put(off::CTIME + i, (VIRTUAL_CTIME_SECS >> (8 * i)) as u8);
                put(off::BTIME + i, (VIRTUAL_BIRTHTIME_SECS >> (8 * i)) as u8);
                put(off::SIZE + i, (size >> (8 * i)) as u8);
                put(off::BLOCKS + i, (VIRTUAL_BLOCKS >> (8 * i)) as u8);
            }
            for i in 0..4 {
                put(off::DEV + i, (VIRTUAL_DEV >> (8 * i)) as u8);
                put(off::UID + i, (VIRTUAL_UID >> (8 * i)) as u8);
                put(off::GID + i, (VIRTUAL_GID >> (8 * i)) as u8);
                put(off::BLKSIZE + i, (VIRTUAL_BLKSIZE >> (8 * i)) as u8);
            }
        }
    }

    /// Mint a fresh descriptor over `path`; `create` truncates (or creates) the
    /// file first.
    fn virtual_open(&mut self, path: Vec<u8>, writable: bool, create: bool) -> i32 {
        if create {
            self.virtual_files.insert(path.clone(), Vec::new());
        }
        let fd = self.virtual_next_fd;
        self.virtual_next_fd += 1;
        self.virtual_fds.insert(
            fd,
            VirtualFd {
                path,
                cursor: 0,
                writable,
                is_dir: false,
            },
        );
        fd
    }

    /// Write `bytes` at the descriptor's cursor (extending the file as needed),
    /// advancing the cursor. `None` if the fd is unknown or not writable.
    fn virtual_write(&mut self, fd: i32, bytes: &[u8]) -> Option<usize> {
        let descriptor = self.virtual_fds.get(&fd)?;
        if !descriptor.writable {
            return None;
        }
        let path = descriptor.path.clone();
        let cursor = descriptor.cursor;
        let content = self.virtual_files.get_mut(&path)?;
        let end = cursor + bytes.len();
        if content.len() < end {
            content.resize(end, 0);
        }
        content[cursor..end].copy_from_slice(bytes);
        if let Some(descriptor) = self.virtual_fds.get_mut(&fd) {
            descriptor.cursor = end;
        }
        Some(bytes.len())
    }

    /// Read up to `count` bytes from the descriptor's cursor, advancing it.
    /// `None` if the fd is unknown.
    fn virtual_read_n(&mut self, fd: i32, count: usize) -> Option<Vec<u8>> {
        let descriptor = self.virtual_fds.get(&fd)?;
        let path = descriptor.path.clone();
        let cursor = descriptor.cursor;
        let content = self.virtual_files.get(&path)?;
        let available = content.get(cursor..).unwrap_or(&[]);
        let take = available.len().min(count);
        let bytes = available[..take].to_vec();
        if let Some(descriptor) = self.virtual_fds.get_mut(&fd) {
            descriptor.cursor = cursor + take;
        }
        Some(bytes)
    }

    /// Read up to `count` bytes starting at absolute `offset` WITHOUT moving the
    /// cursor (Rust `FileExt::read_at` / `pread`). `None` if the fd is unknown or
    /// the offset is negative. A read past end-of-file yields fewer (or zero) bytes.
    fn virtual_read_at(&mut self, fd: i32, offset: i64, count: usize) -> Option<Vec<u8>> {
        if offset < 0 {
            return None;
        }
        let descriptor = self.virtual_fds.get(&fd)?;
        let path = descriptor.path.clone();
        let content = self.virtual_files.get(&path)?;
        let available = content.get(offset as usize..).unwrap_or(&[]);
        let take = available.len().min(count);
        Some(available[..take].to_vec())
    }

    /// Write `bytes` at absolute `offset` (extending + zero-filling any gap) WITHOUT
    /// moving the cursor (Rust `FileExt::write_at` / `pwrite`). `None` if the fd is
    /// unknown, not writable, or the offset is negative.
    fn virtual_write_at(&mut self, fd: i32, offset: i64, bytes: &[u8]) -> Option<usize> {
        if offset < 0 {
            return None;
        }
        let descriptor = self.virtual_fds.get(&fd)?;
        if !descriptor.writable {
            return None;
        }
        let path = descriptor.path.clone();
        let start = offset as usize;
        let content = self.virtual_files.get_mut(&path)?;
        let end = start + bytes.len();
        if content.len() < end {
            content.resize(end, 0);
        }
        content[start..end].copy_from_slice(bytes);
        Some(bytes.len())
    }

    /// `open(path, flags)`: model the O_CREAT/O_TRUNC/O_APPEND/access bits.
    /// Returns a fresh fd, or -1 if the path is absent and O_CREAT is not set.
    fn virtual_open_flags(&mut self, path: Vec<u8>, flags: i32) -> i32 {
        // Follow one symlink level (the canonicalize/read_link model): native
        // open on BOTH families resolves symlinks, and the hermetic open never
        // did -- surfaced when the windows canonicalize composition made open
        // its entry point. The descriptor stores the RESOLVED path, so
        // handle-keyed consumers (final_path_name_by_handle) report the final
        // target exactly like Win32.
        let path = self.virtual_symlinks.get(&path).cloned().unwrap_or(path);
        let exists = self.virtual_files.contains_key(&path);
        let o_creat = host_open_flags::o_creat(flags);
        let o_trunc = host_open_flags::o_trunc(flags);
        let o_append = host_open_flags::o_append(flags);
        let writable = flags & 0x3 != 0; // O_WRONLY | O_RDWR (universal)
        // Opening a directory for writing is EISDIR (Rust `ErrorKind::IsADirectory`).
        // Checked before the ENOENT test so a dir path (never in `virtual_files`)
        // reports the more specific kind.
        if self.virtual_dirs.contains(&path) && writable {
            self.virtual_errno = 21; // EISDIR
            return -1;
        }
        // Permission enforcement: opening a chmod'd path fails with EACCES when
        // the needed bit is clear — the owner-write bit (0o200) for a write-open,
        // or the owner-read bit (0o400) for a read-open (Rust
        // `ErrorKind::PermissionDenied`).
        let needed_bit = if writable { 0o200 } else { 0o400 };
        if self
            .virtual_perms
            .get(&path)
            .is_some_and(|mode| mode & needed_bit == 0)
        {
            self.virtual_errno = 13; // EACCES
            return -1;
        }
        // Read-open of a DIRECTORY: POSIX allows opening a dir read-only (the
        // basis for `read_dir`). Mint a dir descriptor. Checked before the ENOENT
        // test since a dir path is never in `virtual_files`. (This also aligns
        // `exists`/`try_exists` on a dir with native, where opening a dir works.)
        if !writable && self.virtual_dirs.contains(&path) {
            let fd = self.virtual_next_fd;
            self.virtual_next_fd += 1;
            self.virtual_fds.insert(
                fd,
                VirtualFd {
                    path,
                    cursor: 0,
                    writable: false,
                    is_dir: true,
                },
            );
            return fd;
        }
        if !exists && !o_creat {
            self.virtual_errno = 2; // ENOENT
            return -1;
        }
        if !exists || o_trunc {
            self.virtual_files.insert(path.clone(), Vec::new());
        }
        let cursor = if o_append {
            self.virtual_files.get(&path).map_or(0, Vec::len)
        } else {
            0
        };
        let fd = self.virtual_next_fd;
        self.virtual_next_fd += 1;
        self.virtual_fds.insert(
            fd,
            VirtualFd {
                path,
                cursor,
                writable,
                is_dir: false,
            },
        );
        fd
    }

    /// `lseek(fd, offset, whence)`: reposition the cursor, returning the new
    /// absolute offset. `None` on unknown fd, bad whence, or a negative result.
    fn virtual_seek(&mut self, fd: i32, offset: i64, whence: i32) -> Option<i64> {
        let descriptor = self.virtual_fds.get(&fd)?;
        let path = descriptor.path.clone();
        let cursor = descriptor.cursor as i64;
        let len = self.virtual_files.get(&path).map_or(0, Vec::len) as i64;
        let new_pos = match whence {
            0 => offset,          // SEEK_SET
            1 => cursor + offset, // SEEK_CUR
            2 => len + offset,    // SEEK_END
            _ => return None,
        };
        if new_pos < 0 {
            return None;
        }
        if let Some(descriptor) = self.virtual_fds.get_mut(&fd) {
            descriptor.cursor = new_pos as usize;
        }
        Some(new_pos)
    }

    /// `ftruncate(fd, length)`: resize the file backing `fd` (truncate or
    /// zero-extend). Returns 0 on success, -1 on an unknown fd/path.
    fn virtual_set_len(&mut self, fd: i32, length: i64) -> i64 {
        let Some(descriptor) = self.virtual_fds.get(&fd) else {
            return -1;
        };
        let path = descriptor.path.clone();
        let Some(content) = self.virtual_files.get_mut(&path) else {
            return -1;
        };
        content.resize(length.max(0) as usize, 0);
        0
    }

    /// The non-root `chown`/`fchown`/`lchown` rule shared by the ownership
    /// handlers: a change to the CURRENT owner -- or a uid/gid of -1, meaning
    /// "leave that component unchanged" -- is a permitted no-op (returns 0); any
    /// OTHER owner is EPERM (sets errno 1, returns -1). Mirrors what the native
    /// syscalls do for a normal (non-root) user, keeping the two engines'
    /// differential consistent.
    fn virtual_chown_result(&mut self, uid: i32, gid: i32) -> i64 {
        let effective_uid = if uid == -1 { VIRTUAL_UID as i32 } else { uid };
        let effective_gid = if gid == -1 { VIRTUAL_GID as i32 } else { gid };
        if effective_uid == VIRTUAL_UID as i32 && effective_gid == VIRTUAL_GID as i32 {
            0
        } else {
            self.virtual_errno = 1; // EPERM
            -1
        }
    }

    /// The boundary-trait type name of a call's receiver field (e.g. `console`
    /// -> `Console`, `fs` -> `Filesystem`), used to key host dispatch on the
    /// trait rather than the bare method name. `None` when the receiver is not a
    /// `self` field of a boundary-trait type.
    fn receiver_boundary_type_name(&self, call: &TableCall, frame: &Frame) -> Option<String> {
        let leaf = self
            .program
            .statement_table
            .name_path_members(call.receiver)
            .last()
            .map(|name| name.as_str().to_owned())?;
        let self_type = match &*frame.self_cell.borrow() {
            Value::Struct { type_name, .. } => type_name.clone(),
            _ => return None,
        };
        let machine = self.find_machine_by_name(&self_type)?;
        let data_name = machine.attached_data.as_ref()?;
        let data = self.find_data_by_name(data_name.as_str())?;
        for member in self.program.data_members(data) {
            if let DataMember::Field(field) = member
                && field.name.as_str() == leaf
            {
                return Some(self.program.display_type_reference(field.type_reference));
            }
        }
        None
    }

    /// Consume the next line from the remaining stdin (without the line terminator). CRLF
    /// and LF are both handled; returns an empty string at end of input.
    /// One raw stdin byte as a std `ByteRead` value: `Byte { value }` while
    /// input remains, `Eof` after (ordinal 0 -- the ZII zero case; sentinel
    /// spellings vetoed). The declaring type resolves by
    /// name from std/console.omg (invalid + name-global fallback when a
    /// program shadows or lacks it, the WireVerdict precedent).
    fn read_stdin_byte_value(&mut self) -> Value {
        let type_symbol = self
            .find_data_by_name("ByteRead")
            .map(|data| data.symbol)
            .unwrap_or_else(SymbolHandle::invalid);
        if self.stdin_cursor < self.stdin.len() {
            let byte = self.stdin[self.stdin_cursor];
            self.stdin_cursor += 1;
            Value::Enum {
                type_symbol,
                variant_name: "Byte".to_owned(),
                payload: vec![("value".to_owned(), Value::Int(i64::from(byte)).cell())],
            }
        } else {
            Value::Enum {
                type_symbol,
                variant_name: "Eof".to_owned(),
                payload: Vec::new(),
            }
        }
    }

    fn read_stdin_line(&mut self) -> String {
        let mut line = String::new();
        while self.stdin_cursor < self.stdin.len() {
            let byte = self.stdin[self.stdin_cursor];
            self.stdin_cursor += 1;
            if byte == b'\n' {
                break;
            }
            if byte == b'\r' {
                // Drop a CRLF terminator; a lone CR also ends the line.
                if self.stdin_cursor < self.stdin.len() && self.stdin[self.stdin_cursor] == b'\n' {
                    self.stdin_cursor += 1;
                }
                break;
            }
            line.push(byte as char);
        }
        line
    }

    /// A call is a host-boundary call when its target state is declared on a
    /// `boundary trait` (matched by `target_symbol`, or by the receiver leaf naming a
    /// field whose type is a boundary trait).
    fn is_boundary_call(&self, call: &TableCall, frame: &Frame) -> bool {
        // By target symbol: any boundary trait machine signature with this symbol.
        if call.target_symbol.is_valid() {
            for trait_definition in self.program.traits() {
                if !trait_definition.is_boundary {
                    continue;
                }
                for signature in self.program.trait_machine_signatures(trait_definition) {
                    if signature.symbol == call.target_symbol {
                        return true;
                    }
                }
            }
        }

        // By the receiver field's declared type being a boundary trait. The receiver leaf
        // (e.g. "console") names a field whose type symbol is a boundary trait.
        let receiver_leaf = self
            .program
            .statement_table
            .name_path_members(call.receiver)
            .last()
            .map(|name| name.as_str().to_owned());
        if let Some(leaf) = receiver_leaf {
            // The receiver field exists on `self`; look up its declared type via the
            // attached data definition.
            let self_type = match &*frame.self_cell.borrow() {
                Value::Struct { type_name, .. } => type_name.clone(),
                _ => String::new(),
            };
            if let Some(machine) = self.find_machine_by_name(&self_type) {
                if let Some(data_name) = machine.attached_data.as_ref() {
                    if let Some(data) = self.find_data_by_name(data_name.as_str()) {
                        for member in self.program.data_members(data) {
                            if let DataMember::Field(field) = member {
                                if field.name.as_str() == leaf {
                                    let type_symbol =
                                        self.program.type_reference_symbol(field.type_reference);
                                    if self.is_boundary_trait_symbol(type_symbol) {
                                        return true;
                                    }
                                    // Fallback for an imported boundary trait whose
                                    // `is_boundary` flag did not survive resolution (the std
                                    // `console`): a canonical host method on a `Console`-typed
                                    // field is a host call.
                                    let type_name =
                                        self.program.display_type_reference(field.type_reference);
                                    return type_name.contains("Console")
                                        && is_canonical_host_method(call.target.as_str());
                                }
                            }
                        }
                    }
                }
            }
        }

        false
    }

    fn is_boundary_trait_symbol(&self, symbol: SymbolHandle) -> bool {
        symbol.is_valid()
            && self.program.traits().iter().any(|trait_definition| {
                trait_definition.is_boundary && trait_definition.symbol == symbol
            })
    }

    // ---- expressions --------------------------------------------------------

    fn eval_expression(&mut self, handle: ExpressionHandle, frame: &Frame) -> EvalResult<Value> {
        self.tick()?;
        let node = self.program.expression_table.expression(handle).clone();
        if matches!(
            &node,
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
        ) && let Some(value) = self.read_mutable_record_recast_target(handle, frame)?
        {
            return Ok(value);
        }
        match node {
            ExpressionNode::Atomic(atomic) => self.eval_expression(atomic.value, frame),
            ExpressionNode::Integer(value) => match value.bits_u64() {
                // Value::Int carries the 8-byte two's-complement pattern; u64
                // semantics ride the bits. The literal-width gate guarantees an
                // oversize literal only reaches u64-classed positions, so the
                // bit-cast is the value there -- refuse anything wider.
                Some(bits) => Ok(Value::Int(bits as i64)),
                None => unsupported(format!(
                    "integer literal `{value}` exceeds the interpreter's 8-byte value width"
                )),
            },
            ExpressionNode::Boolean(value) => Ok(Value::Bool(value)),
            // The LANDED read (F2a): an f32-suffixed literal means its
            // correctly-rounded f32 value everywhere -- widened exactly to the
            // carrier f64. Keyed on the landing, identically to the native
            // literal reads (f32_bits), so the engines stay bit-for-bit.
            ExpressionNode::Float(value) => Ok(Value::Float(value.landed_f64())),
            ExpressionNode::String(value) => Ok(Value::str(value.to_string())),
            ExpressionNode::Name(path) => self.eval_name(&path, frame),
            ExpressionNode::Member(member) => {
                // A member on a PLACE receiver reads through its storage cell,
                // preserving aliasing. An inline NON-place receiver -- e.g. `.len`
                // on a subslice literal `(arr[a..b]).len`, whose receiver is a VIEW,
                // not a storage location -- has no place; evaluate the receiver to a
                // value and read the field off it. (A subslice BOUND to a local is a
                // place and takes the fast path.) Without this fallback the
                // receiver's range index reached the `Range` arm below and tripped
                // "range expression outside index position", diverging from the
                // native fold of `(arr[a..b]).len`.
                match self.resolve_place(handle, frame) {
                    Ok(cell) => Ok(cell.borrow().clone()),
                    Err(_) => {
                        let receiver = self.eval_expression(member.receiver, frame)?;
                        let field = self.field_cell(&receiver.cell(), member.member.as_str())?;
                        Ok(self.deref_cell(field).borrow().clone())
                    }
                }
            }
            ExpressionNode::Mutable(inner) => {
                // `&mut place as &mut View` can survive typed-tree rewriting as
                // `Mutable(Cast(RecastMutable))` in a guard or forwarded value.
                // In value position evaluate the recast view itself; argument
                // and assignment seams separately preserve its mutable alias.
                if matches!(
                    self.program.expression_table.expression(inner),
                    ExpressionNode::Cast(cast) if cast.form.is_recast()
                ) {
                    return self.eval_expression(inner, frame);
                }
                let cell = self.resolve_place(inner, frame)?;
                // Re-borrow collapse: see eval_argument's Mutable arm.
                let target = match &*cell.borrow() {
                    Value::Ref(target) => Rc::clone(target),
                    _ => Rc::clone(&cell),
                };
                Ok(Value::Ref(target))
            }
            ExpressionNode::Unary(unary) => {
                let operand = self.eval_expression(unary.operand, frame)?;
                self.eval_unary(unary.operator, operand)
            }
            ExpressionNode::Binary(binary) => {
                let left = self.eval_expression(binary.left, frame)?;
                // `&&`/`||` SHORT-CIRCUIT: synthesized structural equality
                // (Equatable) guards each sum arm's payload reads behind tag
                // compares, so the right operand must not evaluate when the
                // left already decides -- a cross-case payload read would
                // trap here while the native backend's eager read of
                // in-allocation payload bytes is masked by the false tag.
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                    let decided = left
                        .as_bool()
                        .ok_or_else(|| Halt::Trap("logical operand not boolean".to_owned()))?;
                    if (binary.operator == BinaryOperator::And) != decided {
                        // false && _  /  true || _
                        return Ok(Value::Bool(decided));
                    }
                    let right = self.eval_expression(binary.right, frame)?;
                    return right
                        .as_bool()
                        .map(Value::Bool)
                        .ok_or_else(|| Halt::Trap("logical operand not boolean".to_owned()));
                }
                let right = self.eval_expression(binary.right, frame)?;
                let unsigned_operands = matches!(
                    binary.operator,
                    BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual
                        | BinaryOperator::Divide
                        | BinaryOperator::Modulo
                        | BinaryOperator::ShiftRight
                ) && (self.expression_is_unsigned64(binary.left, frame)
                    || self.expression_is_unsigned64(binary.right, frame));
                // Non-Exact ADD/SUB/MUL apply their domain at the OPERATION
                // node (native emits the clamping/trapping/wrapping-width
                // sequence itself), signed DIV/MOD resolve the MIN/-1
                // corner there, and Wrapping SHIFTS need the type WIDTH for
                // their at/above-width count semantics (modular zero /
                // sign-fill), so resolve the expression's declared scalar
                // type for the operators the domains cover.
                let scalar_type = if matches!(
                    binary.operator,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                        | BinaryOperator::Modulo
                        | BinaryOperator::ShiftLeft
                        | BinaryOperator::ShiftRight
                ) {
                    self.expression_scalar_type(handle, frame)
                } else {
                    None
                };
                let named_float_operation = self.named_float_operation_name(handle);
                self.eval_binary(
                    binary.operator,
                    left,
                    right,
                    unsigned_operands,
                    scalar_type,
                    named_float_operation,
                )
            }
            ExpressionNode::Call(call) => self.eval_call_expression(handle, &call, frame),
            ExpressionNode::Cast(cast) => {
                let target = self.cast_target_primitive(cast.target_type);
                // RUNG B interior recast (`&self.buf[4] as &u32`): assemble the
                // target's bytes LITTLE-ENDIAN from the byte region starting at
                // the indexed element (the judged class guarantees a literal
                // in-bounds offset over a `[u8; N]` place).
                if cast.form.is_recast()
                    && let Some(assembled) = self.eval_interior_recast(&cast, target, frame)?
                {
                    return Ok(assembled);
                }
                let value = self.eval_expression(cast.value, frame)?;
                if cast.form.is_recast() {
                    if target.is_none()
                        && let Some(source_type) = self.expression_type_reference(cast.value, frame)
                        && self
                            .record_view_type_layout(source_type, &mut HashSet::new())
                            .is_some()
                    {
                        let source = value.clone().cell();
                        let cells = self.snapshot_typed_value_bytes(&source, source_type)?;
                        if let Some(value) =
                            self.assemble_record_view_type(cast.target_type, &cells, 0)?
                        {
                            return Ok(value);
                        }
                    }
                    return self.eval_recast_to_type(value, cast.target_type);
                }
                let source = self
                    .expression_type_reference(cast.value, frame)
                    .and_then(|source| self.program.primitive_type_reference(source));
                self.eval_cast(value, source, target, cast.domain)
            }
            ExpressionNode::Indexed(indexed) => {
                // A range index `arr[start..end]` produces a SUBSLICE view sharing the
                // collection's element cells; a scalar index reads one element.
                if let ExpressionNode::Range(range) = self
                    .program
                    .expression_table
                    .expression(indexed.index)
                    .clone()
                {
                    return self.eval_subslice(indexed.collection, &range, frame);
                }
                // A scalar index into a string VIEW (`Value::Str`) reads the i-th BYTE as an Int
                // -- this is how the oracle cross-checks byte-string canaries (hashing,
                // comparison, byte walks) instead of skipping them as "cannot index Str". A
                // carrier `[u8; N]` is a `Value::Array` and takes the element path below. READ
                // ONLY: a write `s[i] = x` still traps via element_cell (string views are
                // immutable), so there is no silent no-op.
                if let Ok(collection_cell) = self.resolve_place(indexed.collection, frame) {
                    let collection_cell = self.deref_cell(collection_cell);
                    let indexes_str = matches!(&*collection_cell.borrow(), Value::Str(_));
                    if indexes_str {
                        let index = self.eval_index(indexed.index, frame)?;
                        if let Value::Str(text) = &*collection_cell.borrow() {
                            return text
                                .borrow()
                                .get(index)
                                .map(|byte| Value::Int(i64::from(*byte)))
                                .ok_or_else(|| {
                                    Halt::Trap(format!("string index {index} out of bounds"))
                                });
                        }
                    }
                }
                let cell = self.resolve_place(handle, frame)?;
                let value = self.deref_cell(cell).borrow().clone();
                Ok(value)
            }
            ExpressionNode::ArrayLiteral(values) => {
                let mut elements = Vec::new();
                for value in self.program.expression_table.expression_handles(values) {
                    elements.push(self.eval_expression(*value, frame)?.cell());
                }
                Ok(Value::Array(elements))
            }
            // The frontend only produces a Range under an index expression (handled in the
            // Indexed arm above as a subslice); general/open ranges in value or argument
            // position are parse errors (probed in tests/coverage.rs). Decline defensively
            // in case a future frontend starts emitting them elsewhere.
            ExpressionNode::Range(_) => unsupported("range expression outside index position"),
            ExpressionNode::StructLiteral(literal) => self.eval_struct_literal(&literal, frame),
            ExpressionNode::ZeroValue(_) => {
                unsupported("proof-only zero_value<T>() reached runtime evaluation")
            }
        }
    }

    fn eval_call_expression(
        &mut self,
        handle: ExpressionHandle,
        call: &omega_typed_trees::expression::TableCallExpression,
        frame: &Frame,
    ) -> EvalResult<Value> {
        // Builtins: max / min over two integer/float operands.
        let target = call.target.as_str();
        // CH10 root grant marker (see the statement-call twin): a no-op.
        if target.starts_with("accept_boundary#")
            || target.starts_with("select_provider#")
            || target.starts_with("wire_compatibility#")
        {
            return Ok(Value::Unit);
        }
        // The tree walker has no architectural flags register. Preserve the
        // value-flow shape with the architecturally fixed RFLAGS bit 1 set;
        // the matching restore statement is a no-op above.
        if target == "asm#pushfq" && !call.receiver.is_valid() {
            return Ok(Value::Int(2));
        }
        if matches!(target, "max" | "min") {
            let args = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if args.len() == 2 {
                let left = self.eval_expression(args[0], frame)?;
                let right = self.eval_expression(args[1], frame)?;
                // A u64-classed operand selects the UNSIGNED min/max witness (the
                // same test the binary div/mod/shr path uses): `max(u64::MAX, 5)`
                // must pick u64::MAX, not the signed -1. Native lowers these to
                // MaxUnsigned/MinUnsigned for unsigned targets.
                let unsigned = self.expression_is_unsigned64(args[0], frame)
                    || self.expression_is_unsigned64(args[1], frame);
                return self.eval_min_max(target, left, right, unsigned);
            }
        }
        if matches!(
            target,
            "float#multiply_then_add_f32"
                | "float#multiply_then_add_f64"
                | "float#fused_multiply_add_f32"
                | "float#fused_multiply_add_f64"
        ) && call.receiver == ExpressionHandle::invalid()
        {
            let args = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if args.len() == 3 {
                let format = if target.ends_with("_f32") {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                let fused = target.contains("#fused_");
                return self.eval_rewritten_ternary_float(&args, format, fused, frame);
            }
        }
        // Builtin: sqrt over a single float operand. The interpreter consumes
        // the same exact semantic function used as the native
        // sqrtsd/sqrtss contract oracle.
        if target == "sqrt" && call.receiver == ExpressionHandle::invalid() {
            let args = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if args.len() == 1 {
                let format = if matches!(
                    self.expression_scalar_type(args[0], frame),
                    Some((PrimitiveType::F32, _))
                ) {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                return match self.eval_expression(args[0], frame)? {
                    Value::Float(value) => {
                        let meaning = if format == SemanticFloatFormat::BINARY32 {
                            FloatMeaning::from_f32(value as f32)
                        } else {
                            FloatMeaning::from_f64(value)
                        };
                        Ok(Value::Float(
                            FloatSemantics::square_root(format, &meaning)
                                .to_interpreter_value(format),
                        ))
                    }
                    other => Err(Halt::Trap(format!(
                        "sqrt expects a float argument, got {other:?}"
                    ))),
                };
            }
        }
        // Internal enum-valued classifiers selected only by exact named-float
        // plans. The format is encoded in the unnameable builtin identity so
        // state-local expression copying cannot erase it.
        if matches!(target, "float#classify_f32" | "float#classify_f64")
            && call.receiver == ExpressionHandle::invalid()
        {
            let args = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if args.len() == 1 {
                let format = if target == "float#classify_f32" {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                return match self.eval_expression(args[0], frame)? {
                    Value::Float(value) => {
                        let meaning = if format == SemanticFloatFormat::BINARY32 {
                            FloatMeaning::from_f32(value as f32)
                        } else {
                            FloatMeaning::from_f64(value)
                        };
                        let (variant_name, negative) =
                            match FloatSemantics::classify(format, &meaning) {
                                SemanticFloatClass::NaN => ("NaN", None),
                                SemanticFloatClass::Infinity { negative } => {
                                    ("Infinity", Some(negative))
                                }
                                SemanticFloatClass::Normal { negative } => {
                                    ("Normal", Some(negative))
                                }
                                SemanticFloatClass::Subnormal { negative } => {
                                    ("Subnormal", Some(negative))
                                }
                                SemanticFloatClass::Zero { negative } => ("Zero", Some(negative)),
                            };
                        Ok(Value::Enum {
                            type_symbol: self
                                .find_data_by_name("FloatClass")
                                .map(|data| data.symbol)
                                .unwrap_or_else(SymbolHandle::invalid),
                            variant_name: variant_name.to_owned(),
                            payload: negative
                                .map(|negative| {
                                    vec![("negative".to_owned(), Value::Bool(negative).cell())]
                                })
                                .unwrap_or_default(),
                        })
                    }
                    _ => unsupported("float classification argument is not a float"),
                };
            }
        }

        // Internal unary predicates selected only by exact named-float plans.
        // Evaluate the argument once; lowering a predicate to a duplicated
        // expression would repeat nontrivial argument evaluation.
        if matches!(
            target,
            "float#is_nan"
                | "float#is_finite"
                | "float#is_infinite"
                | "float#is_normal"
                | "float#is_subnormal"
        ) && call.receiver == ExpressionHandle::invalid()
        {
            let args = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if args.len() == 1 {
                let format = if matches!(
                    self.expression_scalar_type(args[0], frame),
                    Some((PrimitiveType::F32, _))
                ) {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                return match self.eval_expression(args[0], frame)? {
                    Value::Float(value) => {
                        let meaning = if format == SemanticFloatFormat::BINARY32 {
                            FloatMeaning::from_f32(value as f32)
                        } else {
                            FloatMeaning::from_f64(value)
                        };
                        let result = match target {
                            "float#is_nan" => FloatSemantics::is_nan(&meaning),
                            "float#is_finite" => FloatSemantics::is_finite(&meaning),
                            "float#is_infinite" => FloatSemantics::is_infinite(&meaning),
                            "float#is_normal" => FloatSemantics::is_normal(format, &meaning),
                            "float#is_subnormal" => FloatSemantics::is_subnormal(format, &meaning),
                            _ => unreachable!(),
                        };
                        Ok(Value::Bool(result))
                    }
                    _ => unsupported("float classification argument is not a float"),
                };
            }
        }

        // Slice/array view builtins on an array-valued receiver. `.as_slice()` /
        // `.as_mut_slice()` produce a slice that SHARES the array's element cells (so a
        // write through the slice aliases the array); `.len()` returns the element count.
        if matches!(target, "as_slice" | "as_mut_slice" | "len") && call.receiver.is_valid() {
            if let Ok(cell) = self.resolve_place(call.receiver, frame) {
                let cell = self.deref_cell(cell);
                let elements = match &*cell.borrow() {
                    Value::Array(elements) => Some(elements.clone()),
                    _ => None,
                };
                if let Some(elements) = elements {
                    return Ok(match target {
                        "len" => Value::Int(elements.len() as i64),
                        // A slice view shares the same element `Rc`s.
                        _ => Value::Array(elements),
                    });
                }
            }
        }

        // Borrowed text-view builtins are descriptor-preserving views. The
        // interpreter represents literal-backed and borrowed byte views with
        // the same shared `Value::Str` cell, mirroring the native `{ptr, len}`
        // descriptor copy. Returning a clone shares the bytes.
        if matches!(target, "as_view" | "bytes") && call.receiver.is_valid() {
            if let Ok(cell) = self.resolve_place(call.receiver, frame) {
                let cell = self.deref_cell(cell);
                let value = cell.borrow().clone();
                if matches!(value, Value::Str(_)) {
                    return Ok(value);
                }
            }
        }

        // A transition's guard subject evaluates ONCE per transition evaluation: the
        // parser lowers `transition self.f(x) { true -> a false -> b }` into one guard
        // per arm, each holding a COPY of the subject call (distinct handles, identical
        // structure). A later arm reuses the earlier arm's result instead of re-running
        // the callee's side effects -- matching the native lowering's shared prelude.
        if self.guard_depth > 0 {
            let memo = frame.guard_call_results.borrow();
            for (seen, value) in memo.iter() {
                if self
                    .program
                    .expression_table
                    .expressions_structurally_equal(*seen, handle)
                {
                    return Ok(value.clone());
                }
            }
        }

        // Resolve the value-call. A bare-self receiver naming a SIBLING state of the
        // current machine runs that state; a receiver expression resolving to a contained
        // sub-machine instance runs on that instance; otherwise a free helper machine.
        let (machine, entry_state, instance) = match self
            .resolve_value_call_target(call, target, frame)
        {
            Ok(resolution) => resolution,
            Err(halt) => {
                // A host-boundary VALUE call (`self.clock.tick_count()`,
                // `self.fs.create(..)`): driven directly, like the
                // statement-position host calls in try_host_call. User machines
                // take precedence -- the host fallback only fires when nothing
                // else resolves, mirroring the native collection (which keys on
                // boundary-trait signature symbols).

                // Compatibility Math boundary calls are still imported through
                // the host tables natively. The interpreter must nevertheless
                // consume the same executable semantic definition as build-time
                // folding and ordinary landed arithmetic; otherwise FMA acquires
                // a third, host-language meaning (or remains unsupported).
                if let Some(value) = self.try_float_boundary_value_call(target, call, frame)? {
                    return Ok(value);
                }

                // Value-returning FilesystemHost ops (assignment-position calls
                // like `self.fd = self.fs.create(path, mode)`).
                let fs_args = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec();
                if let Some(value) = self.try_filesystem_call(target, &fs_args, frame)? {
                    self.host_boundary_touched = true;
                    return Ok(value);
                }

                if matches!(
                    target,
                    "tick_count"
                        | "key_state"
                        | "dc_create"
                        | "get_dc"
                        | "window_create"
                        | "is_window"
                        | "window_destroy"
                        | "foreground_window"
                        | "msg_peek"
                        | "msg_translate"
                        | "msg_dispatch"
                ) {
                    // Value-position host fallbacks are host-boundary calls too
                    // (the build-time purity backstop must see them); none of
                    // these are filesystem ops, so the granted-build backstop
                    // sees them as well.
                    self.host_boundary_touched = true;
                    self.non_fs_host_boundary_touched = true;
                }
                if target == "read_byte" {
                    // The next raw stdin byte as `ByteRead::Byte { value }`,
                    // or `ByteRead::Eof` at end-of-input (the ZII zero case;
                    // sentinel spellings are vetoed); the byte path does no CRLF
                    // normalization. Mirrors the statement-position arm in
                    // try_host_call, but read_byte is value-position by
                    // nature (`let r = self.console.read_byte()`).
                    self.host_boundary_touched = true;
                    self.non_fs_host_boundary_touched = true;
                    return Ok(self.read_stdin_byte_value());
                }
                if let Some(value) = self.virtual_time_host_value(target) {
                    self.host_boundary_touched = true;
                    self.non_fs_host_boundary_touched = true;
                    return Ok(value);
                }
                if target == "tick_count" {
                    self.virtual_ticks += 1;
                    return Ok(Value::Int(self.virtual_ticks));
                }
                if target == "key_state" {
                    // The virtual host has no keyboard: no key is ever down.
                    return Ok(Value::Int(0));
                }
                if target == "dc_create" || target == "get_dc" {
                    // Virtual device contexts are the opaque non-zero token 1
                    // (programs must branch on handle != 0, never on a concrete
                    // handle value -- native handles are real pointers).
                    return Ok(Value::Int(1));
                }
                if target == "window_create" {
                    // Mint a live virtual window handle token.
                    self.virtual_window_next += 1;
                    self.virtual_live_windows.insert(self.virtual_window_next);
                    return Ok(Value::Int(self.virtual_window_next));
                }
                if target == "foreground_window" {
                    // The virtual desktop has one app: the most recently
                    // created window is foreground while it lives, 0 after.
                    let foreground = if self
                        .virtual_live_windows
                        .contains(&self.virtual_window_next)
                    {
                        self.virtual_window_next
                    } else {
                        0
                    };
                    return Ok(Value::Int(foreground));
                }
                if target == "is_window" || target == "window_destroy" {
                    // Liveness mirrors native IsWindow/DestroyWindow: 1 for a
                    // live handle, 0 otherwise; destroy removes it.
                    let Some(handle_argument) = self
                        .program
                        .expression_table
                        .expression_handles(call.arguments)
                        .first()
                        .copied()
                    else {
                        return Err(halt);
                    };
                    let handle = match &*self
                        .eval_call_expression_argument(handle_argument, frame)?
                        .borrow()
                    {
                        Value::Int(handle) => *handle,
                        _ => return Ok(Value::Int(0)),
                    };
                    let live = if target == "window_destroy" {
                        self.virtual_live_windows.remove(&handle)
                    } else {
                        self.virtual_live_windows.contains(&handle)
                    };
                    return Ok(Value::Int(i64::from(live)));
                }
                if target == "msg_peek" || target == "msg_translate" || target == "msg_dispatch" {
                    // The virtual host posts no messages: the queue is always
                    // empty (peek = 0) and translate/dispatch have nothing to do.
                    return Ok(Value::Int(0));
                }
                if target == "blit" {
                    // Virtual GDI blit(hdc, dest_w, dest_h, src_w, src_h, pixels,
                    // info): StretchDIBits reports the copied SOURCE scanline
                    // count (probed natively: the source height even when
                    // stretching, even into the memory DC's default 1x1 bitmap).
                    let Some(height) = self
                        .program
                        .expression_table
                        .expression_handles(call.arguments)
                        .get(4)
                        .copied()
                    else {
                        return Err(halt);
                    };
                    return self
                        .eval_call_expression_argument(height, frame)
                        .map(|value| value.borrow().clone());
                }
                return Err(halt);
            }
        };
        let mut args = Vec::new();
        for argument in self
            .program
            .expression_table
            .expression_handles(call.arguments)
        {
            args.push(self.eval_state_argument(*argument, frame)?);
        }
        // Suspend the guard flag while the callee RUNS: distinct same-shaped calls
        // inside its body are genuine repeat calls, not copies of one source
        // expression, and must not memoize against each other.
        let entered_guard_depth = self.guard_depth;
        self.guard_depth = 0;
        let value = self
            .run_state_collect(&machine, &entry_state, instance, args)
            .map(|value| value.unwrap_or(Value::Unit));
        self.guard_depth = entered_guard_depth;
        let value = value?;
        if entered_guard_depth > 0 {
            frame
                .guard_call_results
                .borrow_mut()
                .push((handle, value.clone()));
        }
        Ok(value)
    }

    fn try_float_boundary_value_call(
        &mut self,
        target: &str,
        call: &omega_typed_trees::expression::TableCallExpression,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        if !call.receiver.is_valid() {
            return Ok(None);
        }

        let core_format =
            omega_typed_trees::operator::resolve_named_expression_call(self.program, call)
                .and_then(|operator| {
                    self.program
                        .operator_path_members(operator.name)
                        .first()
                        .and_then(|namespace| match namespace.as_str() {
                            "F32" => Some(SemanticFloatFormat::BINARY32),
                            "F64" => Some(SemanticFloatFormat::BINARY64),
                            _ => None,
                        })
                });
        let compatibility_call =
            core_format.is_none() && matches!(target, "square_root" | "fused_multiply_add");
        if core_format.is_none() && !compatibility_call {
            return Ok(None);
        }

        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments)
            .to_vec();
        let expected_arity = match target {
            "negate"
            | "square_root"
            | "classify"
            | "is_finite"
            | "is_nan"
            | "is_infinite"
            | "is_normal"
            | "is_subnormal"
            | "square_root_toward_zero"
            | "square_root_toward_positive"
            | "square_root_toward_negative" => 1,
            "minimum"
            | "maximum"
            | "add_toward_zero"
            | "add_toward_positive"
            | "add_toward_negative"
            | "subtract_toward_zero"
            | "subtract_toward_positive"
            | "subtract_toward_negative"
            | "multiply_toward_zero"
            | "multiply_toward_positive"
            | "multiply_toward_negative"
            | "divide_toward_zero"
            | "divide_toward_positive"
            | "divide_toward_negative" => 2,
            "multiply_then_add"
            | "fused_multiply_add"
            | "fused_multiply_add_toward_zero"
            | "fused_multiply_add_toward_positive"
            | "fused_multiply_add_toward_negative" => 3,
            _ => return Ok(None),
        };
        if arguments.len() != expected_arity {
            return Ok(None);
        }
        let format = core_format.unwrap_or_else(|| {
            if matches!(
                self.expression_scalar_type(arguments[0], frame),
                Some((PrimitiveType::F32, _))
            ) {
                SemanticFloatFormat::BINARY32
            } else {
                SemanticFloatFormat::BINARY64
            }
        });
        let policy_domain = if core_format.is_some() {
            let mut selected = ArithmeticDomain::Exact;
            for argument in &arguments {
                let Some((_, domain)) = self.expression_scalar_type(*argument, frame) else {
                    continue;
                };
                if domain == ArithmeticDomain::Exact {
                    continue;
                }
                if selected != ArithmeticDomain::Exact && selected != domain {
                    return trap(format!(
                        "mixed arithmetic domains in named float operation `{target}`"
                    ));
                }
                selected = domain;
            }
            selected
        } else {
            // Compatibility Math calls are separate host-boundary operations,
            // not the normalized F32/F64 requirement surface.
            ArithmeticDomain::Exact
        };
        let mut operands = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let value = self.eval_expression(argument, frame)?;
            let Value::Float(value) = value else {
                return Err(Halt::Trap(format!(
                    "{target} expects float arguments, got {value:?}"
                )));
            };
            operands.push(if format == SemanticFloatFormat::BINARY32 {
                FloatSemantics::convert(format, &FloatMeaning::from_f64(value))
            } else {
                FloatMeaning::from_f64(value)
            });
        }

        let float_value = |meaning: FloatMeaning, division: bool| -> EvalResult<Value> {
            let meaning = match policy_domain {
                ArithmeticDomain::Saturating if division => {
                    FloatSemantics::apply_saturating_divide_policy(
                        format,
                        &operands[0],
                        &operands[1],
                        meaning,
                    )
                }
                ArithmeticDomain::Saturating => {
                    let operand_refs = operands.iter().collect::<Vec<_>>();
                    FloatSemantics::apply_saturating_policy(format, &operand_refs, meaning)
                }
                ArithmeticDomain::Trapping => FloatSemantics::apply_trapping_policy(meaning)
                    .map_err(|trap_class| {
                        Halt::Trap(format!(
                            "named float operation `{target}` produced {} in Trapping domain",
                            match trap_class {
                                FloatPolicyTrap::NaNResult => "NaN",
                                FloatPolicyTrap::InfinityResult => "infinity",
                            }
                        ))
                    })?,
                ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => meaning,
            };
            Ok(Value::Float(meaning.to_interpreter_value(format)))
        };
        let value = match target {
            "negate" => float_value(FloatSemantics::negate(format, &operands[0]), false)?,
            "square_root" => float_value(FloatSemantics::square_root(format, &operands[0]), false)?,
            "multiply_then_add" => float_value(
                FloatSemantics::multiply_then_add(format, &operands[0], &operands[1], &operands[2]),
                false,
            )?,
            "fused_multiply_add" => float_value(
                FloatSemantics::fused_multiply_add(
                    format,
                    &operands[0],
                    &operands[1],
                    &operands[2],
                ),
                false,
            )?,
            "minimum" => float_value(FloatSemantics::minimum(&operands[0], &operands[1]), false)?,
            "maximum" => float_value(FloatSemantics::maximum(&operands[0], &operands[1]), false)?,
            "classify" => {
                let (variant_name, negative) = match FloatSemantics::classify(format, &operands[0])
                {
                    SemanticFloatClass::NaN => ("NaN", None),
                    SemanticFloatClass::Infinity { negative } => ("Infinity", Some(negative)),
                    SemanticFloatClass::Normal { negative } => ("Normal", Some(negative)),
                    SemanticFloatClass::Subnormal { negative } => ("Subnormal", Some(negative)),
                    SemanticFloatClass::Zero { negative } => ("Zero", Some(negative)),
                };
                Value::Enum {
                    type_symbol: self
                        .find_data_by_name("FloatClass")
                        .map(|data| data.symbol)
                        .unwrap_or_else(SymbolHandle::invalid),
                    variant_name: variant_name.to_owned(),
                    payload: negative
                        .map(|negative| vec![("negative".to_owned(), Value::Bool(negative).cell())])
                        .unwrap_or_default(),
                }
            }
            "is_finite" => Value::Bool(FloatSemantics::is_finite(&operands[0])),
            "is_nan" => Value::Bool(FloatSemantics::is_nan(&operands[0])),
            "is_infinite" => Value::Bool(FloatSemantics::is_infinite(&operands[0])),
            "is_normal" => Value::Bool(FloatSemantics::is_normal(format, &operands[0])),
            "is_subnormal" => Value::Bool(FloatSemantics::is_subnormal(format, &operands[0])),
            "add_toward_zero" => float_value(
                FloatSemantics::add_toward_zero(format, &operands[0], &operands[1]),
                false,
            )?,
            "add_toward_positive" => float_value(
                FloatSemantics::add_toward_positive(format, &operands[0], &operands[1]),
                false,
            )?,
            "add_toward_negative" => float_value(
                FloatSemantics::add_toward_negative(format, &operands[0], &operands[1]),
                false,
            )?,
            "subtract_toward_zero" => float_value(
                FloatSemantics::subtract_toward_zero(format, &operands[0], &operands[1]),
                false,
            )?,
            "subtract_toward_positive" => float_value(
                FloatSemantics::subtract_toward_positive(format, &operands[0], &operands[1]),
                false,
            )?,
            "subtract_toward_negative" => float_value(
                FloatSemantics::subtract_toward_negative(format, &operands[0], &operands[1]),
                false,
            )?,
            "multiply_toward_zero" => float_value(
                FloatSemantics::multiply_toward_zero(format, &operands[0], &operands[1]),
                false,
            )?,
            "multiply_toward_positive" => float_value(
                FloatSemantics::multiply_toward_positive(format, &operands[0], &operands[1]),
                false,
            )?,
            "multiply_toward_negative" => float_value(
                FloatSemantics::multiply_toward_negative(format, &operands[0], &operands[1]),
                false,
            )?,
            "divide_toward_zero" => float_value(
                FloatSemantics::divide_toward_zero(format, &operands[0], &operands[1]),
                true,
            )?,
            "divide_toward_positive" => float_value(
                FloatSemantics::divide_toward_positive(format, &operands[0], &operands[1]),
                true,
            )?,
            "divide_toward_negative" => float_value(
                FloatSemantics::divide_toward_negative(format, &operands[0], &operands[1]),
                true,
            )?,
            "square_root_toward_zero" => float_value(
                FloatSemantics::square_root_toward_zero(format, &operands[0]),
                false,
            )?,
            "square_root_toward_positive" => float_value(
                FloatSemantics::square_root_toward_positive(format, &operands[0]),
                false,
            )?,
            "square_root_toward_negative" => float_value(
                FloatSemantics::square_root_toward_negative(format, &operands[0]),
                false,
            )?,
            "fused_multiply_add_toward_zero" => float_value(
                FloatSemantics::fused_multiply_add_toward_zero(
                    format,
                    &operands[0],
                    &operands[1],
                    &operands[2],
                ),
                false,
            )?,
            "fused_multiply_add_toward_positive" => float_value(
                FloatSemantics::fused_multiply_add_toward_positive(
                    format,
                    &operands[0],
                    &operands[1],
                    &operands[2],
                ),
                false,
            )?,
            "fused_multiply_add_toward_negative" => float_value(
                FloatSemantics::fused_multiply_add_toward_negative(
                    format,
                    &operands[0],
                    &operands[1],
                    &operands[2],
                ),
                false,
            )?,
            _ => return Ok(None),
        };

        // The named F32/F64 requirements are checked boundary contracts whose
        // current omega-core provider is hermetic, so semantic evaluation may
        // consume them. Compatibility imports still represent an actual host
        // boundary and must trip the dynamic build-time purity backstop.
        if compatibility_call {
            self.host_boundary_touched = true;
            self.non_fs_host_boundary_touched = true;
        }
        Ok(Some(value))
    }

    fn resolve_value_call_target(
        &mut self,
        call: &omega_typed_trees::expression::TableCallExpression,
        target: &str,
        frame: &Frame,
    ) -> EvalResult<(Machine, String, Cell)> {
        // Static-machine specialization rewrites a parameter call to the exact
        // selected ENTRY symbol.  Its human-facing target remains the authored
        // leaf (`TotalOrder`, not `F64::TotalOrder`), so name lookup alone is
        // insufficient for attached or plural satisfiers.  Receiverless calls
        // can dispatch directly through that resolved symbol; receiver calls
        // still need the runtime instance logic below.
        if !call.receiver.is_valid()
            && let Some(resolved) = self.resolve_entry_state_symbol(call.target_symbol, frame)
        {
            return Ok(resolved);
        }

        // Whether this call is on `self` (or receiverless). A NON-self receiver
        // (`self.host.create(..)`) must resolve on the RECEIVER's type, never on
        // a same-named sibling state of the current machine -- else a wrapper
        // machine `Filesystem::create` calling `self.host.create` would recurse
        // into itself. Mirrors the validator's receiver-typed resolution fix.
        let receiver_is_self = if !call.receiver.is_valid() {
            true
        } else {
            match self.program.expression_table.expression(call.receiver) {
                omega_typed_trees::expression::ExpressionNode::Name(path) => {
                    let members = self
                        .program
                        .expression_table
                        .name_path_members(path.members);
                    members.is_empty() || (members.len() == 1 && members[0].as_str() == "self")
                }
                _ => false,
            }
        };

        // (1) Receiver expression resolving to a contained sub-machine / data instance
        // (e.g. `s.code()` where `s: &mut Circle`): run on that instance's machine.
        if call.receiver.is_valid() {
            if let Ok(cell) = self.resolve_place(call.receiver, frame) {
                let cell = self.deref_cell(cell);
                let is_self = Rc::ptr_eq(&cell, &frame.self_cell);
                if !is_self {
                    if let Some(machine) = self.machine_for_instance_state(&cell, target) {
                        return Ok((machine, target.to_owned(), cell));
                    }
                }
            }
        }

        // (1b) TYPE-qualified receiverless call (`Duration::from_milliseconds(x)`):
        // the "receiver" names a type GROUP, not a place, and the callee is the
        // group-qualified machine `<Type>::<target>`. Such a machine takes no
        // receiver (a pure constructor/helper), so the caller's self cell rides
        // along untouched -- same as the free-machine arm (3).
        if call.receiver.is_valid() {
            if let omega_typed_trees::expression::ExpressionNode::Name(path) =
                self.program.expression_table.expression(call.receiver)
            {
                let members = self
                    .program
                    .expression_table
                    .name_path_members(path.members);
                if members.len() == 1
                    && members[0].as_str() != "self"
                    && frame.get(members[0].as_str()).is_none()
                {
                    let group = members[0].as_str();
                    if let Some(machine) = self
                        .program
                        .machines()
                        .iter()
                        .find(|machine| {
                            let mut segments = machine.name.as_str().split("::");
                            segments.next() == Some(group)
                                && machine.name.as_str().ends_with(target)
                                && self.find_state(machine, target).is_some()
                        })
                        .cloned()
                    {
                        return Ok((machine, target.to_owned(), Rc::clone(&frame.self_cell)));
                    }
                }
            }
        }

        // (2) Sibling state of the current machine -- ONLY for self/receiverless
        // calls (a non-self receiver was handled by (1) or falls to the host
        // fallback below).
        if receiver_is_self {
            if let Some(machine) = self.current_machine(frame) {
                if self.find_state(machine, target).is_some() {
                    return Ok((
                        machine.clone(),
                        target.to_owned(),
                        Rc::clone(&frame.self_cell),
                    ));
                }
            }
        }

        // (3) A free helper machine (self/receiverless calls only).
        let machine = self
            .find_machine_for_call(target, frame)
            .filter(|_| receiver_is_self)
            .ok_or_else(|| Halt::Unsupported(format!("unknown value-call target `{target}`")))?;
        let entry_state = self
            .machine_entry_state_name(&machine)
            .ok_or_else(|| Halt::Unsupported(format!("value-call `{target}` has no state")))?;
        Ok((machine, entry_state, Rc::clone(&frame.self_cell)))
    }

    fn eval_call_expression_argument(
        &mut self,
        argument: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<Cell> {
        // Share the same argument-evaluation rules as state calls (incl. reference
        // forwarding for bare-place args that already hold a `&mut`).
        self.eval_argument(argument, frame)
    }

    fn eval_name(&mut self, path: &TableNamePath, frame: &Frame) -> EvalResult<Value> {
        // The boolean keywords `true`/`false` can arrive as single-member name paths in
        // value/transition position (the parser does not always fold them to a literal).
        let members = self
            .program
            .expression_table
            .name_path_members(path.members);
        if members.len() == 1 {
            match members[0].as_str() {
                "true" => return Ok(Value::Bool(true)),
                "false" => return Ok(Value::Bool(false)),
                _ => {}
            }
        }
        // An enum value reference (`CellId::R02` / `Command::Look`) resolves to an Enum.
        if let Some(enum_value) = self.enum_value_from_path(members) {
            return Ok(enum_value);
        }
        let cell = self.resolve_name_place(path, frame)?;
        let value = cell.borrow().clone();
        // A `Ref` read through a name dereferences transparently (param of `&mut T`).
        if let Value::Ref(inner) = value {
            if members.len() == 1
                && let Some(recast) = frame
                    .mutable_scalar_recasts
                    .borrow()
                    .get(members[0].as_str())
                    .cloned()
            {
                return match recast {
                    MutableScalarRecast::Direct { target, .. } => {
                        self.eval_recast(inner.borrow().clone(), Some(target))
                    }
                    MutableScalarRecast::ByteRegion {
                        cells,
                        offset,
                        target,
                    } => self.assemble_scalar_byte_region(&cells, offset, target),
                    MutableScalarRecast::AggregateByteRegion {
                        cells,
                        offset,
                        target_type,
                    } => self
                        .assemble_record_view_type(target_type, &cells, offset)?
                        .ok_or_else(|| {
                            Halt::Trap(format!(
                                "cannot assemble mutable aggregate recast `{}`",
                                self.program.display_type_reference(target_type)
                            ))
                        }),
                    MutableScalarRecast::AggregateTyped {
                        source,
                        source_type,
                        target_type,
                    } => {
                        let cells = self.snapshot_typed_value_bytes(&source, source_type)?;
                        self.assemble_record_view_type(target_type, &cells, 0)?
                            .ok_or_else(|| {
                                Halt::Trap(format!(
                                    "cannot assemble typed mutable aggregate recast `{}`",
                                    self.program.display_type_reference(target_type)
                                ))
                            })
                    }
                };
            }
            return Ok(inner.borrow().clone());
        }
        Ok(value)
    }

    /// Recognize the parser's `Mutable(Cast(RecastMutable))` initializer and
    /// retain either one equal-width scalar cell or the complete indexed byte
    /// region behind the stated scalar view.
    fn mutable_scalar_recast_initializer(
        &mut self,
        initializer: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<Option<(Cell, MutableScalarRecast)>> {
        let cast_handle = match self.program.expression_table.expression(initializer) {
            ExpressionNode::Mutable(inner) => *inner,
            ExpressionNode::Cast(cast)
                if cast.form == omega_core::cast_form::CastForm::RecastMutable =>
            {
                initializer
            }
            _ => return Ok(None),
        };
        let ExpressionNode::Cast(cast) = self.program.expression_table.expression(cast_handle)
        else {
            return Ok(None);
        };
        if cast.form != omega_core::cast_form::CastForm::RecastMutable {
            return Ok(None);
        }
        let target = self.cast_target_primitive(cast.target_type);
        // A forwarded mutable view can acquire another same-target recast at
        // the parameter boundary. Semantically that is still one view over the
        // original storage, so peel the recast-only chain before resolving the
        // source place. Treating the inner Cast as a place is unsupported and,
        // more importantly, would lose the original alias identity.
        let mut source_handle = cast.value;
        loop {
            match self.program.expression_table.expression(source_handle) {
                ExpressionNode::Cast(inner) if inner.form.is_recast() => {
                    source_handle = inner.value;
                }
                ExpressionNode::Mutable(inner)
                    if matches!(
                        self.program.expression_table.expression(*inner),
                        ExpressionNode::Cast(cast) if cast.form.is_recast()
                    ) =>
                {
                    source_handle = *inner;
                }
                _ => break,
            }
        }
        if let ExpressionNode::Indexed(indexed) = self
            .program
            .expression_table
            .expression(source_handle)
            .clone()
        {
            let offset = self.eval_index(indexed.index, frame)?;
            if let Value::Array(cells) = self.eval_expression(indexed.collection, frame)? {
                let source_cell = cells.get(offset).cloned().ok_or_else(|| {
                    Halt::Trap(format!(
                        "mutable interior recast starts at byte {offset} past the region"
                    ))
                })?;
                let recast = match target {
                    Some(target) => MutableScalarRecast::ByteRegion {
                        cells,
                        offset,
                        target,
                    },
                    None => MutableScalarRecast::AggregateByteRegion {
                        cells,
                        offset,
                        target_type: cast.target_type,
                    },
                };
                return Ok(Some((source_cell, recast)));
            }
        }
        if target.is_none() {
            let source = self.resolve_place(source_handle, frame)?;
            let source = self.deref_cell(source);
            let source_type = self.expression_type_reference(source_handle, frame);
            let source_layout = source_type.and_then(|source_type| {
                self.record_view_type_layout(source_type, &mut HashSet::new())
            });
            if let Some(source_type) = source_type
                && source_layout.is_some()
            {
                return Ok(Some((
                    Rc::clone(&source),
                    MutableScalarRecast::AggregateTyped {
                        source,
                        source_type,
                        target_type: cast.target_type,
                    },
                )));
            }
        }
        let Some(target) = target else {
            return Ok(None);
        };
        let Some((source, _)) = self.expression_scalar_type(source_handle, frame) else {
            return Ok(None);
        };
        let source_cell = self.resolve_place(source_handle, frame)?;
        Ok(Some((
            self.deref_cell(source_cell),
            MutableScalarRecast::Direct { source, target },
        )))
    }

    fn mutable_scalar_recast_target(
        &self,
        target: ExpressionHandle,
        frame: &Frame,
    ) -> Option<MutableScalarRecast> {
        let target = match self.program.expression_table.expression(target) {
            ExpressionNode::Mutable(inner) => *inner,
            _ => target,
        };
        let ExpressionNode::Name(path) = self.program.expression_table.expression(target) else {
            return None;
        };
        let members = self
            .program
            .expression_table
            .name_path_members(path.members);
        let [name] = members else {
            return None;
        };
        frame
            .mutable_scalar_recasts
            .borrow()
            .get(name.as_str())
            .cloned()
    }

    /// Recover a mutable recast local and any record-field path projected from
    /// it. Typed expressions use both `Name([view, field])` and nested Member
    /// nodes, so normalize both spellings here.
    fn mutable_recast_path(
        &mut self,
        handle: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<Option<(MutableScalarRecast, Vec<MutableRecordProjectionStep>)>> {
        match self.program.expression_table.expression(handle).clone() {
            ExpressionNode::Mutable(inner) => {
                if let Some((_, recast)) = self.mutable_scalar_recast_initializer(handle, frame)? {
                    return Ok(Some((recast, Vec::new())));
                }
                self.mutable_recast_path(inner, frame)
            }
            ExpressionNode::Cast(cast) if cast.form.is_recast() => Ok(self
                .mutable_scalar_recast_initializer(handle, frame)?
                .map(|(_, recast)| (recast, Vec::new()))),
            ExpressionNode::Name(path) => {
                let members = self
                    .program
                    .expression_table
                    .name_path_members(path.members);
                let Some(root) = members.first() else {
                    return Ok(None);
                };
                let recast = frame
                    .mutable_scalar_recasts
                    .borrow()
                    .get(root.as_str())
                    .cloned();
                Ok(recast.map(|recast| {
                    (
                        recast,
                        members[1..]
                            .iter()
                            .map(|member| {
                                MutableRecordProjectionStep::Field(member.as_str().to_owned())
                            })
                            .collect(),
                    )
                }))
            }
            ExpressionNode::Member(member) => {
                let Some((recast, mut path)) = self.mutable_recast_path(member.receiver, frame)?
                else {
                    return Ok(None);
                };
                path.push(MutableRecordProjectionStep::Field(
                    member.member.as_str().to_owned(),
                ));
                Ok(Some((recast, path)))
            }
            ExpressionNode::Indexed(indexed) => {
                let Some((recast, mut path)) =
                    self.mutable_recast_path(indexed.collection, frame)?
                else {
                    return Ok(None);
                };
                path.push(MutableRecordProjectionStep::Index(indexed.index));
                Ok(Some((recast, path)))
            }
            _ => Ok(None),
        }
    }

    fn read_mutable_record_recast_target(
        &mut self,
        handle: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        let Some((recast, path)) = self.mutable_recast_path(handle, frame)? else {
            return Ok(None);
        };
        let (cells, offset, target_type) = match recast {
            MutableScalarRecast::AggregateByteRegion {
                cells,
                offset,
                target_type,
            } => (cells, offset, target_type),
            MutableScalarRecast::AggregateTyped {
                source,
                source_type,
                target_type,
            } => (
                self.snapshot_typed_value_bytes(&source, source_type)?,
                0,
                target_type,
            ),
            _ => return Ok(None),
        };
        if path.is_empty() {
            return self.assemble_record_view_type(target_type, &cells, offset);
        }
        if matches!(
            path.as_slice(),
            [MutableRecordProjectionStep::Field(field)] if field == "len"
        ) && self.declared_type_is_slice(target_type)
            && let Some(element_type) = self.collection_element_type(target_type)
            && let Some((stride, _)) =
                self.record_view_type_layout(element_type, &mut HashSet::new())
        {
            let remaining = cells.len().saturating_sub(offset);
            if stride == 0 || remaining % stride != 0 {
                return trap("slice recast bytes do not tile the element layout");
            }
            return Ok(Some(Value::Int((remaining / stride) as i64)));
        }
        let Some((field_offset, field_type)) =
            self.record_view_type_projection(target_type, &path, offset, cells.len(), frame)?
        else {
            return trap(format!(
                "cannot project `{}` through mutable aggregate recast `{}`",
                Self::mutable_record_projection_display(&path),
                self.program.display_type_reference(target_type),
            ));
        };
        self.assemble_record_view_type(field_type, &cells, field_offset)
    }

    fn write_mutable_record_recast_target(
        &mut self,
        handle: ExpressionHandle,
        frame: &Frame,
        value: Value,
    ) -> EvalResult<bool> {
        let Some((recast, path)) = self.mutable_recast_path(handle, frame)? else {
            return Ok(false);
        };
        let (cells, offset, target_type, typed_source) = match recast {
            MutableScalarRecast::AggregateByteRegion {
                cells,
                offset,
                target_type,
            } => (cells, offset, target_type, None),
            MutableScalarRecast::AggregateTyped {
                source,
                source_type,
                target_type,
            } => (
                self.snapshot_typed_value_bytes(&source, source_type)?,
                0,
                target_type,
                Some((source, source_type)),
            ),
            _ => return Ok(false),
        };
        if path.is_empty() {
            self.write_record_view_type(target_type, &cells, offset, value)?;
            if let Some((source, source_type)) = typed_source {
                self.commit_typed_value_bytes(&source, source_type, &cells)?;
            }
            return Ok(true);
        }
        let Some((field_offset, field_type)) =
            self.record_view_type_projection(target_type, &path, offset, cells.len(), frame)?
        else {
            return trap(format!(
                "cannot project `{}` through mutable aggregate recast `{}`",
                Self::mutable_record_projection_display(&path),
                self.program.display_type_reference(target_type),
            ));
        };
        self.write_record_view_type(field_type, &cells, field_offset, value)?;
        if let Some((source, source_type)) = typed_source {
            self.commit_typed_value_bytes(&source, source_type, &cells)?;
        }
        Ok(true)
    }

    fn mutable_record_projection_display(path: &[MutableRecordProjectionStep]) -> String {
        let mut rendered = String::new();
        for step in path {
            match step {
                MutableRecordProjectionStep::Field(field) => {
                    if !rendered.is_empty() {
                        rendered.push('.');
                    }
                    rendered.push_str(field);
                }
                MutableRecordProjectionStep::Index(_) => rendered.push_str("[..]"),
            }
        }
        rendered
    }

    /// `Type::Variant` paths whose head is an enum/data symbol with a matching variant.
    fn enum_value_from_path(
        &self,
        members: &[omega_typed_trees::name::Identifier],
    ) -> Option<Value> {
        if members.len() != 2 {
            return None;
        }
        let type_name = members[0].as_str();
        let variant_name = members[1].as_str();
        let data = self.find_data_by_name(type_name)?;
        let is_variant = self.program.data_members(data).iter().any(|member| {
            matches!(member, DataMember::Variant(variant) if variant.name.as_str() == variant_name)
        });
        is_variant.then(|| {
            // MIXED shapes: a bare payload-less case still carries the COMMON
            // fields, zero-initialized (scalar-only by validation, so the
            // primitive default is the zero value). Pure sums add nothing.
            let common: Vec<(String, Cell)> = self
                .program
                .data_members(data)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some((
                        field.name.as_str().to_owned(),
                        self.default_for_type(field.type_reference).cell(),
                    )),
                    _ => None,
                })
                .collect();
            Value::Enum {
                type_symbol: data.symbol,
                variant_name: variant_name.to_owned(),
                payload: common,
            }
        })
    }

    // ---- place resolution ---------------------------------------------------

    /// Resolve an lvalue expression to its storage cell (for assignment / `&mut`).
    fn resolve_place(&mut self, handle: ExpressionHandle, frame: &Frame) -> EvalResult<Cell> {
        match self.program.expression_table.expression(handle).clone() {
            ExpressionNode::Name(path) => self.resolve_name_place(&path, frame),
            ExpressionNode::Member(member) => {
                let receiver = self.resolve_place(member.receiver, frame)?;
                self.field_cell(&receiver, member.member.as_str())
            }
            ExpressionNode::Indexed(indexed) => {
                let collection = self.resolve_place(indexed.collection, frame)?;
                let index = self.eval_index(indexed.index, frame)?;
                self.element_cell(&collection, index)
            }
            ExpressionNode::Mutable(inner) => self.resolve_place(inner, frame),
            other => unsupported(format!("place expression not supported: {other:?}")),
        }
    }

    fn resolve_name_place(&mut self, path: &TableNamePath, frame: &Frame) -> EvalResult<Cell> {
        let members = self
            .program
            .expression_table
            .name_path_members(path.members)
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect::<Vec<_>>();
        if members.is_empty() {
            return trap("empty name path");
        }

        // Head: `self`, a local, or a self-field (implicit self).
        let head = members[0].as_str();
        let mut cell = if head == "self" {
            Rc::clone(&frame.self_cell)
        } else if let Some(local) = frame.get(head) {
            local
        } else {
            // Implicit self-field: `n` means `self.n`.
            self.field_cell(&frame.self_cell, head)?
        };

        // Walk the remaining members as field accesses, dereferencing refs along the way.
        for member in &members[1..] {
            cell = self.deref_cell(cell);
            cell = self.field_cell(&cell, member)?;
        }
        Ok(cell)
    }

    /// True when a declared type is an owned fixed array `[T; N]` -- seeing THROUGH a domain
    /// `Constrained` wrapper (`[i32; N] in Wrapping`) -- as opposed to a slice `&[T]`. Drives
    /// the value-copy gate: a whole-array assignment/`let` into a FixedArray place is a deep
    /// copy, while a slice is a shared view that must NOT be deep-cloned.
    fn declared_type_is_fixed_array(
        &self,
        type_reference: omega_typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        if !type_reference.is_valid() {
            return false;
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            omega_typed_trees::types::TypeReferenceNode::FixedArray { .. } => true,
            omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                self.declared_type_is_fixed_array(*base_type)
            }
            _ => false,
        }
    }

    /// True for the owned variable-fill text carrier `[u8; N] in <domain>`.
    /// This mirrors `omega-layout`'s `BoundedByteBuffer` classification rather
    /// than treating the carrier as an always-full `[u8; N]` array.
    fn declared_type_is_bounded_byte_buffer(
        &self,
        type_reference: omega_typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        if !type_reference.is_valid() {
            return false;
        }
        let omega_typed_trees::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = self
            .program
            .type_reference_table
            .type_reference(type_reference)
        else {
            return false;
        };
        let has_value_domain = self
            .program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .any(|constraint| match constraint {
                omega_typed_trees::types::TypeConstraintNode::Domain(name) => {
                    !omega_typed_trees::wire::is_layout_domain_name(name.as_str())
                        && omega_core::semantics::CarryPermission::from_name(name.as_str())
                            .is_none()
                }
                _ => false,
            });
        if !has_value_domain {
            return false;
        }
        let omega_typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
            self.program.type_reference_table.type_reference(*base_type)
        else {
            return false;
        };
        self.program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
    }

    /// The ELEMENT type of an owned fixed array `[T; N]` -- seeing THROUGH a
    /// domain `Constrained` wrapper (`[u8; N] in Wrapping`). `None` for a slice,
    /// scalar, or invalid reference. Used to wrap an array-element store to the
    /// element's width/domain (the field-store truncation, for `arr[i] = v`).
    fn fixed_array_element_type(
        &self,
        type_reference: omega_typed_trees::types::TypeReferenceHandle,
    ) -> Option<omega_typed_trees::types::TypeReferenceHandle> {
        if !type_reference.is_valid() {
            return None;
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            omega_typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
                Some(*element_type)
            }
            omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                self.fixed_array_element_type(*base_type)
            }
            _ => None,
        }
    }

    /// The (primitive, arithmetic-domain) an assignment target coerces its stored
    /// SCALAR to -- the decision-17 truncation/clamp/trap the interpreter applies
    /// on a write to match the native store. For a FIELD/local place it is the
    /// declared type's own primitive + domain. For an ARRAY ELEMENT `arr[i]` it
    /// is the element's PRIMITIVE with the ARRAY's DOMAIN (`[u8;N] in Saturating`
    /// clamps its elements). `None` for a non-scalar / unresolved target, which
    /// is then left un-coerced.
    fn assignment_target_coercion(
        &mut self,
        handle: ExpressionHandle,
        frame: &Frame,
    ) -> Option<(omega_typed_trees::types::PrimitiveType, ArithmeticDomain)> {
        if let ExpressionNode::Indexed(indexed) =
            self.program.expression_table.expression(handle).clone()
        {
            let array_type = self.assignment_target_type_reference(indexed.collection, frame)?;
            let element_type = self.fixed_array_element_type(array_type)?;
            let primitive = self.program.primitive_type_reference(element_type)?;
            // The arithmetic domain lives on the ARRAY (`[T;N] in D`), not the
            // bare element type, so read it from the array reference.
            let domain = self
                .program
                .arithmetic_domain_for_type_reference(array_type);
            return Some((primitive, domain));
        }
        let type_reference = self.assignment_target_type_reference(handle, frame)?;
        let primitive = self.program.primitive_type_reference(type_reference)?;
        let domain = self
            .program
            .arithmetic_domain_for_type_reference(type_reference);
        Some((primitive, domain))
    }

    /// The CORE value-landing coercion: coerce a stored SCALAR to an already-
    /// resolved (primitive, arithmetic-domain), matching the native store into
    /// that typed slot -- the decision-17 truncate/clamp/trap for an integer, f32
    /// rounding for a float. A non-scalar value (Struct, Array, Ref, ...) passes
    /// through unchanged. Every interpreter value-landing seam funnels here.
    fn coerce_scalar_with(
        &self,
        value: Value,
        primitive: omega_typed_trees::types::PrimitiveType,
        domain: ArithmeticDomain,
    ) -> EvalResult<Value> {
        match &value {
            Value::Int(raw) => Ok(Value::Int(apply_arithmetic_domain(
                *raw, primitive, domain,
            )?)),
            Value::Float(f) if primitive == PrimitiveType::F32 => {
                if f.is_nan() {
                    return Ok(Value::Float(interpreter_f32_from_bits(
                        interpreter_f32_to_bits(*f),
                    )));
                }
                let meaning = FloatMeaning::from_f64(*f);
                Ok(Value::Float(
                    FloatSemantics::convert(SemanticFloatFormat::BINARY32, &meaning)
                        .to_interpreter_value(SemanticFloatFormat::BINARY32),
                ))
            }
            _ => Ok(value),
        }
    }

    /// Coerce a stored SCALAR to a declared TYPE reference (resolves its primitive
    /// + domain, then [`coerce_scalar_with`]). A non-primitive type passes through.
    /// Used where a value lands in a typed slot with the type in hand: struct/case
    /// literal FIELD init + the LocalData store (the type carries its own domain).
    fn coerce_scalar_value(
        &self,
        value: Value,
        type_reference: omega_typed_trees::types::TypeReferenceHandle,
    ) -> EvalResult<Value> {
        match self.program.primitive_type_reference(type_reference) {
            Some(primitive) => {
                let domain = self
                    .program
                    .arithmetic_domain_for_type_reference(type_reference);
                self.coerce_scalar_with(value, primitive, domain)
            }
            None => Ok(value),
        }
    }

    /// Declared integer primitive of an assignment target, when it is a FIELD whose
    /// receiver resolves to a typed struct (`self.c`, `obj.field`, or the equivalent
    /// name path). Used to wrap an assigned integer to the field's declared width,
    /// matching the native backend's truncating store. Returns `None` for bare locals
    /// (whose cells carry no declared type) and non-field places.
    fn assignment_target_type_reference(
        &mut self,
        handle: ExpressionHandle,
        frame: &Frame,
    ) -> Option<omega_typed_trees::types::TypeReferenceHandle> {
        let (receiver, field_name) = match self.program.expression_table.expression(handle).clone()
        {
            ExpressionNode::Member(member) => {
                let receiver = self.resolve_place(member.receiver, frame).ok()?;
                (receiver, member.member.as_str().to_owned())
            }
            ExpressionNode::Name(path) => {
                let names = self
                    .program
                    .expression_table
                    .name_path_members(path.members)
                    .iter()
                    .map(|name| name.as_str().to_owned())
                    .collect::<Vec<_>>();
                match names.as_slice() {
                    [] => return None,
                    [single] => {
                        // A single name is either a local (no declared-type record
                        // here) or an implicit self-field.
                        if single == "self" || frame.get(single).is_some() {
                            return None;
                        }
                        (Rc::clone(&frame.self_cell), single.clone())
                    }
                    [head, middle @ .., last] => {
                        let mut cell = if head == "self" {
                            Rc::clone(&frame.self_cell)
                        } else if let Some(local) = frame.get(head) {
                            local
                        } else {
                            self.field_cell(&frame.self_cell, head).ok()?
                        };
                        for member in middle {
                            cell = self.deref_cell(cell);
                            cell = self.field_cell(&cell, member).ok()?;
                        }
                        (cell, last.clone())
                    }
                }
            }
            _ => return None,
        };
        let receiver = self.deref_cell(receiver);
        let type_symbol = match &*receiver.borrow() {
            Value::Struct { type_symbol, .. } => *type_symbol,
            _ => return None,
        };
        self.field_type_reference(type_symbol, &field_name)
    }

    /// Declared type reference of `field_name` on the data record or machine
    /// identified by `type_symbol`. A machine instance's struct carries the
    /// MACHINE's symbol while its fields come from the attached data (plus the
    /// machine-owned cells), so both field sources are searched. The caller
    /// derives the primitive type and arithmetic domain from the reference.
    fn field_type_reference(
        &self,
        type_symbol: SymbolHandle,
        field_name: &str,
    ) -> Option<omega_typed_trees::types::TypeReferenceHandle> {
        if let Some(data) = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == type_symbol)
        {
            return self.data_field_type_reference(data, field_name);
        }
        if let Some(machine) = self
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == type_symbol)
        {
            if let Some(data) = machine
                .attached_data
                .as_ref()
                .and_then(|name| self.find_data_by_name(name.as_str()))
                && let Some(type_reference) = self.data_field_type_reference(data, field_name)
            {
                return Some(type_reference);
            }
            for owned in self.program.machine_owned_data(machine) {
                if owned.name.as_str() == field_name {
                    return Some(owned.type_reference);
                }
            }
        }
        None
    }

    fn data_field_type_reference(
        &self,
        data: &DataDefinition,
        field_name: &str,
    ) -> Option<omega_typed_trees::types::TypeReferenceHandle> {
        for member in self.program.data_members(data) {
            if let DataMember::Field(field) = member
                && field.name.as_str() == field_name
            {
                return Some(field.type_reference);
            }
        }
        None
    }

    /// If a cell holds a `Ref`, return the referenced cell (so field access on a `&mut`
    /// parameter reaches the aliased place). Otherwise the cell itself.
    fn deref_cell(&self, cell: Cell) -> Cell {
        let inner = match &*cell.borrow() {
            Value::Ref(target) => Some(Rc::clone(target)),
            _ => None,
        };
        inner.unwrap_or(cell)
    }

    /// Evaluate an index expression to a `usize` element index.
    fn eval_index(&mut self, index: ExpressionHandle, frame: &Frame) -> EvalResult<usize> {
        let value = self.eval_expression(index, frame)?;
        let raw = value
            .as_int()
            .ok_or_else(|| Halt::Trap("array index is not an integer".to_owned()))?;
        usize::try_from(raw).map_err(|_| Halt::Trap(format!("array index {raw} out of range")))
    }

    /// Resolve one element CELL of an `Array` place (sharing the same `Rc`, so a write
    /// through the returned cell aliases the array element).
    fn element_cell(&self, container: &Cell, index: usize) -> EvalResult<Cell> {
        let container = self.deref_cell(Rc::clone(container));
        let borrowed = container.borrow();
        match &*borrowed {
            Value::Array(elements) => elements
                .get(index)
                .cloned()
                .ok_or_else(|| Halt::Trap(format!("array index {index} out of bounds"))),
            other => trap(format!("cannot index {other:?}")),
        }
    }

    /// Evaluate a subslice `collection[start..end]` into an `Array` view that SHARES the
    /// collection's element cells (so writes through the subslice alias the original). A
    /// missing start defaults to 0; a missing end to the length; `end_inclusive` extends by
    /// one.
    fn eval_subslice(
        &mut self,
        collection: ExpressionHandle,
        range: &omega_typed_trees::expression::TableRangeExpression,
        frame: &Frame,
    ) -> EvalResult<Value> {
        // A nested subslice base (`sub[1..][1..]`) is not a place — the inner
        // range-indexed expression produces a VIEW value. Evaluate it as a value
        // (recursing through this function) and slice the resulting window;
        // element cells stay shared, matching the fat-descriptor model where a
        // subslice only offsets the pointer.
        let nested_view = if let ExpressionNode::Indexed(inner) =
            self.program.expression_table.expression(collection).clone()
            && matches!(
                self.program.expression_table.expression(inner.index),
                ExpressionNode::Range(_)
            ) {
            Some(self.eval_expression(collection, frame)?)
        } else {
            None
        };
        let elements = match nested_view {
            Some(Value::Array(elements)) => elements,
            Some(other) => return trap(format!("cannot subslice {other:?}")),
            None => {
                let collection_cell = self.resolve_place(collection, frame)?;
                match &*self.deref_cell(collection_cell).borrow() {
                    Value::Array(elements) => elements.clone(),
                    // A Str-backed slice (a `&[u8] in Path` bound to a string
                    // literal) subslices into a byte view: expose each byte as an
                    // Int cell so the shared range logic + the `Array` host-arg arm
                    // (eval_fs_bytes) handle `path[a..b]` uniformly.
                    Value::Str(text) => text
                        .borrow()
                        .iter()
                        .map(|byte| {
                            std::rc::Rc::new(std::cell::RefCell::new(Value::Int(i64::from(*byte))))
                        })
                        .collect(),
                    other => return trap(format!("cannot subslice {other:?}")),
                }
            }
        };
        let len = elements.len();
        let start = if range.start.is_valid() {
            self.eval_index(range.start, frame)?
        } else {
            0
        };
        let mut end = if range.end.is_valid() {
            self.eval_index(range.end, frame)?
        } else {
            len
        };
        if range.end_inclusive {
            end = end.saturating_add(1);
        }
        let end = end.min(len);
        let start = start.min(end);
        Ok(Value::Array(elements[start..end].to_vec()))
    }

    /// Construct a `data` value from a struct literal `Type { field: value, .. }`. Fields
    /// not named take the type's default; named fields override. A case literal
    /// (`Command::Say { text: ... }`) constructs an Enum value instead: the case name is
    /// the tag and the named payload fields fill the case's declared payload.
    fn eval_struct_literal(
        &mut self,
        literal: &omega_typed_trees::expression::TableStructLiteral,
        frame: &Frame,
    ) -> EvalResult<Value> {
        if let Some(case_name) = &literal.case_name {
            return self.eval_case_literal(literal, case_name.as_str(), frame);
        }
        let type_name = literal.type_name.as_str().to_owned();
        let data = self.find_data_by_name(&type_name);
        let (type_symbol, mut fields) = if let Some(data) = data {
            let mut fields = BTreeMap::new();
            self.populate_data_fields(data, &mut fields)?;
            (data.symbol, fields)
        } else {
            (SymbolHandle::invalid(), BTreeMap::new())
        };
        for field in self.program.expression_table.struct_fields(literal.fields) {
            let value = self.eval_expression(field.value, frame)?;
            // Coerce the field value to the field's declared width/domain, matching
            // the native store into the field slot (`Point { x: a+b }` with `a+b`
            // = 300 into a u8 field reads 44). The field type carries its own
            // domain, so resolve it directly.
            let value = match self.field_type_reference(type_symbol, field.name.as_str()) {
                Some(type_reference) => self.coerce_scalar_value(value, type_reference)?,
                None => value,
            };
            fields.insert(field.name.as_str().to_owned(), value.cell());
        }
        Ok(Value::Struct {
            type_symbol,
            type_name,
            fields,
        })
    }

    /// Construct a payload-carrying case value `Type::Case { field: value, .. }`. Payload
    /// cells follow the case's DECLARED field order; unnamed payload fields default, named
    /// literal fields override, and a literal field that is not part of the case's payload
    /// traps.
    fn eval_case_literal(
        &mut self,
        literal: &omega_typed_trees::expression::TableStructLiteral,
        case_name: &str,
        frame: &Frame,
    ) -> EvalResult<Value> {
        let type_name = literal.type_name.as_str();
        let Some(data) = self.find_data_by_name(type_name) else {
            return trap(format!("unknown data type `{type_name}` in case literal"));
        };
        let Some(variant) =
            self.program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str() == case_name => {
                        Some(variant)
                    }
                    _ => None,
                })
        else {
            return trap(format!("`{type_name}` has no case `{case_name}`"));
        };

        let mut payload = Vec::new();
        // MIXED shapes: the COMMON fields exist in every case and come first.
        // Case construction ZERO-initializes them (frozen decision 7's rule;
        // never the declared default -- validation rejects defaults on mixed
        // common fields), unless the literal names them below.
        for member in self.program.data_members(data) {
            let DataMember::Field(common_field) = member else {
                continue;
            };
            let name = common_field.name.as_str().to_owned();
            let value = self.default_value_for_type(common_field.type_reference)?;
            payload.push((name, value.cell()));
        }
        for field in self.program.data_payload_fields(variant) {
            let name = field.name.as_str().to_owned();
            let value = self.default_value_for_type(field.type_reference)?;
            payload.push((name, value.cell()));
        }
        for field in self.program.expression_table.struct_fields(literal.fields) {
            let value = self.eval_expression(field.value, frame)?;
            let Some(slot) = payload
                .iter_mut()
                .find(|(name, _)| name == field.name.as_str())
            else {
                return trap(format!(
                    "case `{type_name}::{case_name}` has no payload field `{}`",
                    field.name.as_str()
                ));
            };
            slot.1 = value.cell();
        }

        Ok(Value::Enum {
            type_symbol: data.symbol,
            variant_name: case_name.to_owned(),
            payload,
        })
    }

    fn field_cell(&self, container: &Cell, field: &str) -> EvalResult<Cell> {
        let container = self.deref_cell(Rc::clone(container));
        let borrowed = container.borrow();
        match &*borrowed {
            Value::Struct {
                fields, type_name, ..
            } => fields
                .get(field)
                .cloned()
                .ok_or_else(|| Halt::Trap(format!("no field `{field}` on `{type_name}`"))),
            // A case value's payload field (`subject.text` after a case-pattern binding
            // rewrote `text`). The cell is SHARED, preserving aliasing.
            Value::Enum {
                variant_name,
                payload,
                ..
            } => payload
                .iter()
                .find(|(name, _)| name == field)
                .map(|(_, cell)| Rc::clone(cell))
                .ok_or_else(|| {
                    Halt::Trap(format!(
                        "case `{variant_name}` carries no payload field `{field}`"
                    ))
                }),
            // `slice.len` / `array.len` (member form, no parens) -> a fresh length cell.
            Value::Array(elements) if field == "len" => {
                Ok(Value::Int(elements.len() as i64).cell())
            }
            // `text.len` where `text` is a string literal flowing into a `&[u8] in
            // Utf8` parameter (the encoding-domain text model, #66): the literal's
            // `&[u8]` view length is its UTF-8 BYTE count, which is exactly the
            // Rust `String::len`. Matches the native fold of `<literal>.len`.
            Value::Str(text) if field == "len" => Ok(Value::Int(text.borrow().len() as i64).cell()),
            other => trap(format!("cannot read field `{field}` of {other:?}")),
        }
    }

    // ---- operators ----------------------------------------------------------

    /// The target `PrimitiveType` of a cast's full type reference.
    fn cast_target_primitive(
        &self,
        target_type: omega_typed_trees::types::TypeReferenceHandle,
    ) -> Option<PrimitiveType> {
        self.program.primitive_type_reference(target_type)
    }

    /// Apply an `as` cast with width/signedness semantics: int<->float conversions and
    /// integer narrowing/widening (wrapping to the target width, sign- or zero-extending on
    /// read per the SOURCE signedness, which the value carries as its width tag).
    fn eval_cast(
        &self,
        value: Value,
        source: Option<PrimitiveType>,
        target: Option<PrimitiveType>,
        domain: ArithmeticDomain,
    ) -> EvalResult<Value> {
        let Some(target) = target else {
            // A cast to a non-primitive (e.g. a trait object) is a no-op identity here.
            return Ok(value);
        };
        match target {
            PrimitiveType::F32 | PrimitiveType::F64 => {
                let format = if target == PrimitiveType::F32 {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                let meaning = match value {
                    Value::Float(source) => {
                        FloatSemantics::convert(format, &FloatMeaning::from_f64(source))
                    }
                    Value::Int(value) => {
                        let value = if source.is_some_and(is_unsigned_integer_primitive) {
                            BigInt::from_u64(value as u64)
                        } else {
                            BigInt::from_i64(value)
                        };
                        FloatSemantics::from_integer(format, &value)
                    }
                    _ => return trap(format!("cast to {target:?} of non-numeric")),
                };
                Ok(Value::Float(meaning.to_interpreter_value(format)))
            }
            PrimitiveType::Bool => Ok(Value::Bool(value.as_bool().unwrap_or(false))),
            integer => {
                // Int -> int reinterprets at the target width; the result
                // keeps the target's width tag so later ops/casts wrap.
                let raw = match value {
                    // FLOAT -> int is one of three distinct conversion
                    // operations. Wrapping has no float reading and is
                    // rejected by validation.
                    Value::Float(value) => {
                        let format = semantic_integer_format(integer)
                            .expect("a primitive integer has a semantic format");
                        let meaning = FloatMeaning::from_f64(value);
                        let converted = match domain {
                            ArithmeticDomain::Saturating => {
                                FloatSemantics::to_integer_saturating(&meaning, format)
                            }
                            ArithmeticDomain::Trapping => FloatSemantics::to_integer_trapping(
                                &meaning, format,
                            )
                            .map_err(|reason| {
                                Halt::Trap(float_to_integer_trap_message(integer, reason, true))
                            })?,
                            ArithmeticDomain::Exact => FloatSemantics::to_integer_exact(
                                &meaning, format,
                            )
                            .map_err(|reason| {
                                Halt::Trap(float_to_integer_trap_message(integer, reason, false))
                            })?,
                            ArithmeticDomain::Wrapping => {
                                return trap(
                                    "Wrapping float-to-integer conversion has no semantic reading"
                                        .to_owned(),
                                );
                            }
                        };
                        return Ok(Value::Int(big_integer_runtime_value(&converted, integer)));
                    }
                    other => other
                        .as_int()
                        .ok_or_else(|| Halt::Trap("cast to integer of non-numeric".to_owned()))?,
                };
                Ok(Value::Int(wrap_to_width(raw, integer)))
            }
        }
    }

    /// §5b recast (`&x as &T`): bit-REINTERPRET, never convert. Validation
    /// (omega-validation recasts.rs, rung A) guarantees equal scalar widths
    /// and fences bool/text/records, so the reinterpretation below is total.
    /// A SNAPSHOT of the source's bits is sound for the shared-only rung:
    /// borrow exclusivity freezes the source while the view lives. Native
    /// needs no twin -- the emitted load already reads the place's bytes
    /// through the stated type.
    /// The interior half of the §5b recast: a LITERAL-indexed read over a
    /// byte array assembles `size_of(target)` bytes little-endian (floats
    /// from the assembled bits). `Ok(None)` when the shape is not the
    /// interior class (the scalar-pun path then evaluates normally).
    fn eval_interior_recast(
        &mut self,
        cast: &omega_typed_trees::expression::TableCastExpression,
        target: Option<PrimitiveType>,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        let ExpressionNode::Indexed(indexed) =
            self.program.expression_table.expression(cast.value).clone()
        else {
            return Ok(None);
        };
        // Literal or RUNTIME offset (rung C1): both evaluate to the byte
        // position the view starts at.
        let offset_value = match self.program.expression_table.expression(indexed.index) {
            ExpressionNode::Integer(literal) => literal.value_i64(),
            _ => self.eval_expression(indexed.index, frame)?.as_int(),
        };
        let Some(offset) = offset_value.and_then(|value| usize::try_from(value).ok()) else {
            return Ok(None);
        };
        let collection = self.eval_expression(indexed.collection, frame)?;
        let Value::Array(cells) = collection else {
            return Ok(None);
        };
        // RUNG C2: a RECORD target assembles field-by-field at
        // natural-alignment offsets (each field at the next multiple of its
        // own size -- LOCKSTEP with the layout rule; the drift canary pins
        // agreement).
        let Some(target) = target else {
            return self.assemble_record_view_type(cast.target_type, &cells, offset);
        };
        self.assemble_scalar_byte_region(&cells, offset, target)
            .map(Some)
    }

    fn assemble_scalar_byte_region(
        &self,
        cells: &[Cell],
        offset: usize,
        target: PrimitiveType,
    ) -> EvalResult<Value> {
        let size = target
            .scalar_byte_size()
            .ok_or_else(|| Halt::Trap("byte-region recast target is not scalar".to_owned()))?;
        let mut bits: u64 = 0;
        for byte_index in 0..size {
            let cell = cells.get(offset + byte_index).ok_or_else(|| {
                Halt::Trap(format!(
                    "interior recast reads byte {} past the region",
                    offset + byte_index
                ))
            })?;
            let byte = cell.borrow().as_int().unwrap_or(0) as u64 & 0xFF;
            bits |= byte << (8 * byte_index);
        }
        let assembled = match target {
            PrimitiveType::F32 => Value::Float(interpreter_f32_from_bits(bits as u32)),
            PrimitiveType::F64 => Value::Float(f64::from_bits(bits)),
            integer => Value::Int(wrap_to_width(bits as i64, integer)),
        };
        Ok(assembled)
    }

    fn write_scalar_byte_region(
        &self,
        cells: &[Cell],
        offset: usize,
        target: PrimitiveType,
        value: Value,
    ) -> EvalResult<()> {
        let size = target
            .scalar_byte_size()
            .ok_or_else(|| Halt::Trap("byte-region recast target is not scalar".to_owned()))?;
        let bits = match target {
            PrimitiveType::F32 => interpreter_f32_to_bits(
                value
                    .as_float()
                    .ok_or_else(|| Halt::Trap("f32 recast write is not numeric".to_owned()))?,
            ) as u64,
            PrimitiveType::F64 => value
                .as_float()
                .ok_or_else(|| Halt::Trap("f64 recast write is not numeric".to_owned()))?
                .to_bits(),
            _ => value
                .as_int()
                .ok_or_else(|| Halt::Trap("integer recast write is not numeric".to_owned()))?
                as u64,
        };
        for byte_index in 0..size {
            let cell = cells.get(offset + byte_index).ok_or_else(|| {
                Halt::Trap(format!(
                    "mutable interior recast writes byte {} past the region",
                    offset + byte_index
                ))
            })?;
            *cell.borrow_mut() = Value::Int(((bits >> (8 * byte_index)) & 0xFF) as i64);
        }
        Ok(())
    }

    /// Rung C2's record view: decode fixed records little-endian from the same
    /// geometry native lowering consumes. Ordinary records use natural
    /// packing; plan-laid records use their validated offsets. Named fields
    /// recurse, permitting a plain wrapper around a plan-laid foreign record.
    fn assemble_record_view_inner(
        &self,
        type_name: &str,
        cells: &[Cell],
        base_offset: usize,
        visiting: &mut HashSet<String>,
    ) -> EvalResult<Option<Value>> {
        if !visiting.insert(type_name.to_owned()) {
            return Ok(None);
        }
        let Some(data) = self.find_data_by_name(type_name) else {
            visiting.remove(type_name);
            return Ok(None);
        };
        let mut field_specs: Vec<(String, TypeReferenceHandle, usize, usize)> = Vec::new();
        for member in self.program.data_members(data) {
            let omega_typed_trees::data::DataMember::Field(field) = member else {
                visiting.remove(type_name);
                return Ok(None);
            };
            let Some((size, align)) = self.record_view_type_layout(field.type_reference, visiting)
            else {
                visiting.remove(type_name);
                return Ok(None);
            };
            field_specs.push((
                field.name.as_str().to_owned(),
                field.type_reference,
                size,
                align,
            ));
        }

        let offsets = if let Some(plan) = self
            .program
            .plan_laid_layouts
            .iter()
            .find(|plan| plan.data_name == type_name)
        {
            if plan.offsets.len() != field_specs.len() {
                visiting.remove(type_name);
                return Ok(None);
            }
            plan.offsets.clone()
        } else {
            let mut offsets = Vec::with_capacity(field_specs.len());
            let mut offset = 0usize;
            for (_, _, size, align) in &field_specs {
                offset = offset.div_ceil(*align) * *align;
                offsets.push(offset);
                let Some(next) = offset.checked_add(*size) else {
                    visiting.remove(type_name);
                    return Ok(None);
                };
                offset = next;
            }
            offsets
        };

        let type_symbol = data.symbol;
        let mut field_values = std::collections::BTreeMap::new();
        for ((name, type_reference, _, _), field_offset) in field_specs.into_iter().zip(offsets) {
            let Some(value) = self.assemble_record_view_type_inner(
                type_reference,
                cells,
                base_offset + field_offset,
                visiting,
            )?
            else {
                visiting.remove(type_name);
                return Ok(None);
            };
            field_values.insert(name, value.cell());
        }
        visiting.remove(type_name);
        Ok(Some(Value::Struct {
            type_symbol,
            type_name: type_name.to_owned().into(),
            fields: field_values,
        }))
    }

    fn assemble_record_view_type(
        &self,
        type_reference: TypeReferenceHandle,
        cells: &[Cell],
        base_offset: usize,
    ) -> EvalResult<Option<Value>> {
        self.assemble_record_view_type_inner(
            type_reference,
            cells,
            base_offset,
            &mut HashSet::new(),
        )
    }

    fn assemble_record_view_type_inner(
        &self,
        type_reference: TypeReferenceHandle,
        cells: &[Cell],
        base_offset: usize,
        visiting: &mut HashSet<String>,
    ) -> EvalResult<Option<Value>> {
        if let Some(primitive) = self.program.primitive_type_reference(type_reference) {
            return self
                .assemble_scalar_byte_region(cells, base_offset, primitive)
                .map(Some);
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Constrained { base_type, .. }
            | TypeReferenceNode::Reference {
                referee: base_type, ..
            } => self.assemble_record_view_type_inner(*base_type, cells, base_offset, visiting),
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                let Some((stride, _)) =
                    self.record_view_type_layout(*element_type, &mut HashSet::new())
                else {
                    return Ok(None);
                };
                let mut values = Vec::with_capacity(*length);
                for index in 0..*length {
                    let Some(offset) = stride
                        .checked_mul(index)
                        .and_then(|delta| base_offset.checked_add(delta))
                    else {
                        return trap("record-view array offset overflow");
                    };
                    let Some(value) = self.assemble_record_view_type_inner(
                        *element_type,
                        cells,
                        offset,
                        visiting,
                    )?
                    else {
                        return Ok(None);
                    };
                    values.push(value.cell());
                }
                Ok(Some(Value::Array(values)))
            }
            TypeReferenceNode::Slice { element_type } => {
                let Some((stride, _)) =
                    self.record_view_type_layout(*element_type, &mut HashSet::new())
                else {
                    return Ok(None);
                };
                let remaining = cells.len().saturating_sub(base_offset);
                if stride == 0 || remaining % stride != 0 {
                    return trap("slice recast bytes do not tile the element layout");
                }
                let length = remaining / stride;
                let mut values = Vec::with_capacity(length);
                for index in 0..length {
                    let offset = stride
                        .checked_mul(index)
                        .and_then(|delta| base_offset.checked_add(delta))
                        .ok_or_else(|| Halt::Trap("slice recast offset overflow".to_owned()))?;
                    let Some(value) = self.assemble_record_view_type_inner(
                        *element_type,
                        cells,
                        offset,
                        visiting,
                    )?
                    else {
                        return Ok(None);
                    };
                    values.push(value.cell());
                }
                Ok(Some(Value::Array(values)))
            }
            TypeReferenceNode::Named { name, .. } => {
                self.assemble_record_view_inner(name.as_str(), cells, base_offset, visiting)
            }
            _ => Ok(None),
        }
    }

    fn record_view_type_layout(
        &self,
        type_reference: TypeReferenceHandle,
        visiting: &mut HashSet<String>,
    ) -> Option<(usize, usize)> {
        if let Some(primitive) = self.program.primitive_type_reference(type_reference) {
            let size = primitive.scalar_byte_size()?;
            return Some((size, size));
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Constrained { base_type, .. }
            | TypeReferenceNode::Reference {
                referee: base_type, ..
            } => self.record_view_type_layout(*base_type, visiting),
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                let (element_size, element_align) =
                    self.record_view_type_layout(*element_type, visiting)?;
                Some((element_size.checked_mul(*length)?, element_align))
            }
            TypeReferenceNode::Named { name, .. } => {
                self.record_view_data_layout(name.as_str(), visiting)
            }
            _ => None,
        }
    }

    fn record_view_data_layout(
        &self,
        type_name: &str,
        visiting: &mut HashSet<String>,
    ) -> Option<(usize, usize)> {
        if !visiting.insert(type_name.to_owned()) {
            return None;
        }
        let data = self.find_data_by_name(type_name)?;
        let mut field_layouts = Vec::new();
        for member in self.program.data_members(data) {
            let omega_typed_trees::data::DataMember::Field(field) = member else {
                visiting.remove(type_name);
                return None;
            };
            let Some(layout) = self.record_view_type_layout(field.type_reference, visiting) else {
                visiting.remove(type_name);
                return None;
            };
            field_layouts.push(layout);
        }
        let result = if let Some(plan) = self
            .program
            .plan_laid_layouts
            .iter()
            .find(|plan| plan.data_name == type_name)
        {
            (plan.offsets.len() == field_layouts.len()).then_some((plan.size, plan.align))
        } else {
            let mut offset = 0usize;
            let mut max_align = 1usize;
            for (size, align) in field_layouts {
                offset = offset.div_ceil(align) * align;
                offset = offset.checked_add(size)?;
                max_align = max_align.max(align);
            }
            Some((offset.div_ceil(max_align) * max_align, max_align))
        };
        visiting.remove(type_name);
        result
    }

    fn record_view_fields(
        &self,
        type_name: &str,
    ) -> Option<Vec<(String, TypeReferenceHandle, usize)>> {
        let data = self.find_data_by_name(type_name)?;
        let mut fields = Vec::new();
        let mut layouts = Vec::new();
        for member in self.program.data_members(data) {
            let omega_typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            let layout = self.record_view_type_layout(field.type_reference, &mut HashSet::new())?;
            fields.push((field.name.as_str().to_owned(), field.type_reference));
            layouts.push(layout);
        }
        let offsets = if let Some(plan) = self
            .program
            .plan_laid_layouts
            .iter()
            .find(|plan| plan.data_name == type_name)
        {
            if plan.offsets.len() != fields.len() {
                return None;
            }
            plan.offsets.clone()
        } else {
            let mut offsets = Vec::with_capacity(fields.len());
            let mut offset = 0usize;
            for (size, align) in layouts {
                offset = offset.div_ceil(align) * align;
                offsets.push(offset);
                offset = offset.checked_add(size)?;
            }
            offsets
        };
        Some(
            fields
                .into_iter()
                .zip(offsets)
                .map(|((name, type_reference), offset)| (name, type_reference, offset))
                .collect(),
        )
    }

    fn record_view_projection(
        &mut self,
        type_name: &str,
        path: &[MutableRecordProjectionStep],
        base_offset: usize,
        region_len: usize,
        frame: &Frame,
    ) -> EvalResult<Option<(usize, TypeReferenceHandle)>> {
        let Some((MutableRecordProjectionStep::Field(field_name), rest)) = path.split_first()
        else {
            return Ok(None);
        };
        let (_, field_type, field_offset) = self
            .record_view_fields(type_name)
            .unwrap_or_default()
            .into_iter()
            .find(|(name, _, _)| name == field_name)
            .ok_or_else(|| {
                Halt::Trap(format!(
                    "record view `{type_name}` has no field `{field_name}`"
                ))
            })?;
        let offset = base_offset
            .checked_add(field_offset)
            .ok_or_else(|| Halt::Trap("record-view field offset overflow".to_owned()))?;
        self.record_view_type_projection(field_type, rest, offset, region_len, frame)
    }

    fn record_view_type_projection(
        &mut self,
        type_reference: TypeReferenceHandle,
        path: &[MutableRecordProjectionStep],
        base_offset: usize,
        region_len: usize,
        frame: &Frame,
    ) -> EvalResult<Option<(usize, TypeReferenceHandle)>> {
        if path.is_empty() {
            return Ok(Some((base_offset, type_reference)));
        }
        let (step, rest) = path.split_first().expect("nonempty projection path");
        match step {
            MutableRecordProjectionStep::Field(_) => {
                let Some(nested_name) = self
                    .record_view_named_type(type_reference)
                    .map(str::to_owned)
                else {
                    return Ok(None);
                };
                self.record_view_projection(&nested_name, path, base_offset, region_len, frame)
            }
            MutableRecordProjectionStep::Index(index_handle) => {
                let node = self
                    .program
                    .type_reference_table
                    .type_reference(type_reference)
                    .clone();
                let (element_type, length) = match node {
                    TypeReferenceNode::Constrained { base_type, .. }
                    | TypeReferenceNode::Reference {
                        referee: base_type, ..
                    } => {
                        return self.record_view_type_projection(
                            base_type,
                            path,
                            base_offset,
                            region_len,
                            frame,
                        );
                    }
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(length),
                    } => (element_type, length),
                    TypeReferenceNode::Slice { element_type } => {
                        let Some((stride, _)) =
                            self.record_view_type_layout(element_type, &mut HashSet::new())
                        else {
                            return Ok(None);
                        };
                        if stride == 0 || base_offset > region_len {
                            return Ok(None);
                        }
                        (element_type, (region_len - base_offset) / stride)
                    }
                    _ => return Ok(None),
                };
                let index = self.eval_index(*index_handle, frame)?;
                if index >= length {
                    return trap(format!(
                        "record-view array index {index} out of bounds for length {length}"
                    ));
                }
                let Some((stride, _)) =
                    self.record_view_type_layout(element_type, &mut HashSet::new())
                else {
                    return Ok(None);
                };
                let offset = stride
                    .checked_mul(index)
                    .and_then(|delta| base_offset.checked_add(delta))
                    .ok_or_else(|| Halt::Trap("record-view array offset overflow".to_owned()))?;
                self.record_view_type_projection(element_type, rest, offset, region_len, frame)
            }
        }
    }

    fn write_record_view(
        &self,
        type_name: &str,
        cells: &[Cell],
        base_offset: usize,
        value: Value,
    ) -> EvalResult<()> {
        let Value::Struct { fields, .. } = value else {
            return trap(format!(
                "mutable record recast `{type_name}` requires a record value"
            ));
        };
        let Some(field_specs) = self.record_view_fields(type_name) else {
            return trap(format!(
                "cannot lay out mutable record recast `{type_name}`"
            ));
        };
        for (name, type_reference, field_offset) in field_specs {
            let Some(field_cell) = fields.get(&name) else {
                return trap(format!(
                    "record value written through `{type_name}` has no field `{name}`"
                ));
            };
            let offset = base_offset
                .checked_add(field_offset)
                .ok_or_else(|| Halt::Trap("mutable record recast offset overflow".to_owned()))?;
            let field_value = field_cell.borrow().clone();
            self.write_record_view_type(type_reference, cells, offset, field_value)?;
        }
        Ok(())
    }

    fn declared_type_is_slice(&self, type_reference: TypeReferenceHandle) -> bool {
        if !type_reference.is_valid() {
            return false;
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Slice { .. } => true,
            TypeReferenceNode::Constrained { base_type, .. }
            | TypeReferenceNode::Reference {
                referee: base_type, ..
            } => self.declared_type_is_slice(*base_type),
            _ => false,
        }
    }

    /// The interpreter stores typed records as semantic field cells rather
    /// than a contiguous byte allocation. A typed record recast nevertheless
    /// aliases the record's representation, so snapshot the established source
    /// through its own layout, operate on those bytes through the target
    /// layout, then decode writes back through the source layout. Validation
    /// has already proved identical geometry and leaf representation sets.
    fn snapshot_typed_value_bytes(
        &self,
        source: &Cell,
        source_type: TypeReferenceHandle,
    ) -> EvalResult<Vec<Cell>> {
        let (size, _) = self
            .record_view_type_layout(source_type, &mut HashSet::new())
            .ok_or_else(|| {
                Halt::Trap(format!(
                    "cannot lay out typed mutable recast source `{}`",
                    self.program.display_type_reference(source_type)
                ))
            })?;
        let cells = (0..size).map(|_| Value::Int(0).cell()).collect::<Vec<_>>();
        self.write_record_view_type(source_type, &cells, 0, source.borrow().clone())?;
        Ok(cells)
    }

    fn commit_typed_value_bytes(
        &self,
        source: &Cell,
        source_type: TypeReferenceHandle,
        cells: &[Cell],
    ) -> EvalResult<()> {
        let value = self
            .assemble_record_view_type(source_type, cells, 0)?
            .ok_or_else(|| {
                Halt::Trap(format!(
                    "cannot restore typed mutable recast source `{}`",
                    self.program.display_type_reference(source_type)
                ))
            })?;
        *source.borrow_mut() = value;
        Ok(())
    }

    fn write_record_view_type(
        &self,
        type_reference: TypeReferenceHandle,
        cells: &[Cell],
        base_offset: usize,
        value: Value,
    ) -> EvalResult<()> {
        if let Some(primitive) = self.program.primitive_type_reference(type_reference) {
            let domain = self
                .program
                .arithmetic_domain_for_type_reference(type_reference);
            let value = self.coerce_scalar_with(value, primitive, domain)?;
            return self.write_scalar_byte_region(cells, base_offset, primitive, value);
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Constrained { base_type, .. }
            | TypeReferenceNode::Reference {
                referee: base_type, ..
            } => self.write_record_view_type(*base_type, cells, base_offset, value),
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                let Value::Array(values) = value else {
                    return trap("mutable record array projection requires an array value");
                };
                if values.len() != *length {
                    return trap(format!(
                        "mutable record array write has length {}, expected {length}",
                        values.len()
                    ));
                }
                let Some((stride, _)) =
                    self.record_view_type_layout(*element_type, &mut HashSet::new())
                else {
                    return trap("cannot lay out mutable record array element");
                };
                for (index, value) in values.into_iter().enumerate() {
                    let offset = stride
                        .checked_mul(index)
                        .and_then(|delta| base_offset.checked_add(delta))
                        .ok_or_else(|| {
                            Halt::Trap("mutable record array offset overflow".to_owned())
                        })?;
                    self.write_record_view_type(
                        *element_type,
                        cells,
                        offset,
                        value.borrow().clone(),
                    )?;
                }
                Ok(())
            }
            TypeReferenceNode::Slice { element_type } => {
                let Value::Array(values) = value else {
                    return trap("mutable slice recast requires an array value");
                };
                let Some((stride, _)) =
                    self.record_view_type_layout(*element_type, &mut HashSet::new())
                else {
                    return trap("cannot lay out mutable slice element");
                };
                let available = cells.len().saturating_sub(base_offset);
                if stride == 0 || available % stride != 0 || values.len() != available / stride {
                    return trap("mutable slice recast write has the wrong length");
                }
                for (index, value) in values.into_iter().enumerate() {
                    let offset = stride
                        .checked_mul(index)
                        .and_then(|delta| base_offset.checked_add(delta))
                        .ok_or_else(|| Halt::Trap("mutable slice offset overflow".to_owned()))?;
                    self.write_record_view_type(
                        *element_type,
                        cells,
                        offset,
                        value.borrow().clone(),
                    )?;
                }
                Ok(())
            }
            TypeReferenceNode::Named { name, .. } => {
                self.write_record_view(name.as_str(), cells, base_offset, value)
            }
            _ => trap("mutable record projection is not a fixed-layout value"),
        }
    }

    fn record_view_named_type(&self, mut type_reference: TypeReferenceHandle) -> Option<&str> {
        loop {
            match self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                TypeReferenceNode::Constrained { base_type, .. }
                | TypeReferenceNode::Reference {
                    referee: base_type, ..
                } => type_reference = *base_type,
                TypeReferenceNode::Named { name, .. } => return Some(name.as_str()),
                _ => return None,
            }
        }
    }

    fn eval_recast(&self, value: Value, target: Option<PrimitiveType>) -> EvalResult<Value> {
        let Some(target) = target else {
            // Unreachable post-validation (targets are scalar primitives).
            return Ok(value);
        };
        // Look through a reference-valued source (a recast of a `&T`-typed
        // local re-views the pointee's bytes).
        let value = match value {
            Value::Ref(cell) => cell.borrow().clone(),
            other => other,
        };
        match target {
            PrimitiveType::F32 => {
                let bits = match value {
                    Value::Float(f) => interpreter_f32_to_bits(f),
                    other => other
                        .as_int()
                        .ok_or_else(|| Halt::Trap("recast to f32 of non-scalar".to_owned()))?
                        as u32,
                };
                Ok(Value::Float(interpreter_f32_from_bits(bits)))
            }
            PrimitiveType::F64 => {
                let bits = match value {
                    Value::Float(f) => f.to_bits(),
                    other => other
                        .as_int()
                        .ok_or_else(|| Halt::Trap("recast to f64 of non-scalar".to_owned()))?
                        as u64,
                };
                Ok(Value::Float(f64::from_bits(bits)))
            }
            integer => {
                let raw: i64 = match value {
                    // A float source's width equals the target's (validated),
                    // so 4-byte targets take the f32 bit pattern, 8-byte the
                    // f64's.
                    Value::Float(f) => match integer.scalar_byte_size() {
                        Some(4) => interpreter_f32_to_bits(f) as i64,
                        _ => f.to_bits() as i64,
                    },
                    other => other
                        .as_int()
                        .ok_or_else(|| Halt::Trap("recast to integer of non-scalar".to_owned()))?,
                };
                // Equal-width int<->int reinterpretation is exactly the
                // width-wrap (`u32` 0xFFFF_FFFF re-viewed as `i32` = -1).
                Ok(Value::Int(wrap_to_width(raw, integer)))
            }
        }
    }

    /// Evaluate an argument that lands in an Omega state parameter. In addition
    /// to the ordinary aliased cell, preserve the interpretation carried by a
    /// mutable recast view. The target parameter receives the same sealed view
    /// metadata under its own name, so forwarding `&mut [u16]` derived from
    /// `[u8; N]` does not degrade into a byte-array reference at a state edge.
    fn eval_state_argument(
        &mut self,
        argument: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<EvaluatedArgument> {
        let initializer = self.mutable_scalar_recast_initializer(argument, frame)?;
        if let Some((source, recast)) = initializer {
            return Ok(EvaluatedArgument {
                cell: Value::Ref(source).cell(),
                mutable_recast: Some(recast),
            });
        }
        let mutable_recast = self
            .mutable_recast_path(argument, frame)?
            .map(|(recast, _)| recast);
        Ok(EvaluatedArgument {
            cell: self.eval_argument(argument, frame)?,
            mutable_recast,
        })
    }

    fn expression_type_reference(
        &self,
        expression: ExpressionHandle,
        frame: &Frame,
    ) -> Option<TypeReferenceHandle> {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Mutable(inner) => self.expression_type_reference(*inner, frame),
            ExpressionNode::Cast(cast) => Some(cast.target_type),
            ExpressionNode::Indexed(indexed) => {
                let collection = self.expression_type_reference(indexed.collection, frame)?;
                self.collection_element_type(collection)
            }
            ExpressionNode::Member(member) => {
                if let ExpressionNode::Name(path) =
                    self.program.expression_table.expression(member.receiver)
                {
                    let members = self
                        .program
                        .expression_table
                        .name_path_members(path.members);
                    if matches!(members, [head] if head.as_str() == "self") {
                        return self.machine_attached_field_type(
                            frame.machine_symbol,
                            member.member.as_str(),
                        );
                    }
                }
                let receiver = self.expression_type_reference(member.receiver, frame)?;
                self.projected_member_type(receiver, member.member.as_str())
            }
            ExpressionNode::Name(path) => {
                let members = self
                    .program
                    .expression_table
                    .name_path_members(path.members);
                let (mut current, rest) = match members {
                    [] => return None,
                    [head] if head.as_str() == "self" => return None,
                    [head, field, rest @ ..] if head.as_str() == "self" => (
                        self.machine_attached_field_type(frame.machine_symbol, field.as_str())?,
                        rest,
                    ),
                    [head, rest @ ..] => {
                        match frame.type_locals.borrow().get(head.as_str()).copied() {
                            Some(local) => (local, rest),
                            None => (
                                self.machine_attached_field_type(
                                    frame.machine_symbol,
                                    head.as_str(),
                                )?,
                                rest,
                            ),
                        }
                    }
                };
                for member in rest {
                    current = self.projected_member_type(current, member.as_str())?;
                }
                Some(current)
            }
            _ => None,
        }
    }

    fn machine_attached_field_type(
        &self,
        machine_symbol: SymbolHandle,
        field_name: &str,
    ) -> Option<TypeReferenceHandle> {
        let machine = self
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)?;
        let name = machine.attached_data.as_ref()?;
        let data = self.find_data_by_name(name.as_str())?;
        self.program.data_members(data).iter().find_map(|member| {
            let DataMember::Field(field) = member else {
                return None;
            };
            (field.name.as_str() == field_name).then_some(field.type_reference)
        })
    }

    fn projected_member_type(
        &self,
        mut type_reference: TypeReferenceHandle,
        member_name: &str,
    ) -> Option<TypeReferenceHandle> {
        loop {
            match self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::Named { name, .. } => {
                    let data = self.find_data_by_name(name.as_str())?;
                    return self.program.data_members(data).iter().find_map(|member| {
                        let DataMember::Field(field) = member else {
                            return None;
                        };
                        (field.name.as_str() == member_name).then_some(field.type_reference)
                    });
                }
                _ => return None,
            }
        }
    }

    fn collection_element_type(
        &self,
        mut type_reference: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        loop {
            match self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => return Some(*element_type),
                _ => return None,
            }
        }
    }

    fn eval_recast_to_type(
        &self,
        value: Value,
        target_type: TypeReferenceHandle,
    ) -> EvalResult<Value> {
        let target = self
            .program
            .type_reference_table
            .type_reference(target_type)
            .clone();
        match target {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => self.eval_recast_to_type(value, referee),
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                let value = match value {
                    Value::Ref(cell) => cell.borrow().clone(),
                    other => other,
                };
                let Value::Array(elements) = value else {
                    return trap("fixed-array recast source is not an array");
                };
                if elements.len() != length {
                    return trap(format!(
                        "fixed-array recast expected {length} elements, found {}",
                        elements.len()
                    ));
                }
                let mut recast = Vec::with_capacity(elements.len());
                for element in elements {
                    let value = element.borrow().clone();
                    recast.push(Rc::new(RefCell::new(
                        self.eval_recast_to_type(value, element_type)?,
                    )));
                }
                Ok(Value::Array(recast))
            }
            TypeReferenceNode::Slice { element_type } => {
                let value = match value {
                    Value::Ref(cell) => cell.borrow().clone(),
                    other => other,
                };
                let Value::Array(elements) = value else {
                    return trap("slice recast source is not an array or slice");
                };
                let mut recast = Vec::with_capacity(elements.len());
                for element in elements {
                    let value = element.borrow().clone();
                    recast.push(Rc::new(RefCell::new(
                        self.eval_recast_to_type(value, element_type)?,
                    )));
                }
                Ok(Value::Array(recast))
            }
            _ => self.eval_recast(value, self.cast_target_primitive(target_type)),
        }
    }

    fn eval_unary(&self, operator: UnaryOperator, operand: Value) -> EvalResult<Value> {
        match operator {
            UnaryOperator::LogicalNot => operand
                .as_bool()
                .map(|value| Value::Bool(!value))
                .ok_or_else(|| Halt::Trap("logical-not of non-boolean".to_owned())),
        }
    }

    /// Best-effort STATIC witness that an expression is u64-classed
    /// (`u64`/`usize`/`addr`), used to give width-8 comparisons UNSIGNED
    /// semantics (matching the native signedness-adjusted compares). FALSE on
    /// any doubt: a false negative keeps the signed compare (today's
    /// behavior); only DECLARED types answer true, so signed compares can
    /// never be corrupted.
    fn expression_is_unsigned64(
        &self,
        expression: omega_checked_trees::expression::ExpressionHandle,
        frame: &Frame,
    ) -> bool {
        primitive_is_unsigned64(
            self.expression_scalar_type(expression, frame)
                .map(|(primitive, _)| primitive),
        )
    }

    /// Best-effort STATIC (primitive, arithmetic-domain) of an expression,
    /// resolved from DECLARED types only (decision 17): a NAME reads the
    /// local/param type recorded at binding, `self.field` reads the attached
    /// data field, a CAST is its target width (`as T` carries no domain
    /// clause, and the absence of one means Exact), a BINARY/UNARY node is
    /// typed by one operand witness (mixed classes are checker-rejected).
    /// `None` on any doubt -- literals are adaptive. This is what lets
    /// `acc + 50` SATURATE at the operation node when `acc` is declared
    /// `i8 in Saturating` regardless of where the result lands; the
    /// landing-seam coercions alone cannot represent an expression whose own
    /// domain differs from its landing slot's (native emits the saturating
    /// ADD itself).
    fn expression_scalar_type(
        &self,
        expression: omega_checked_trees::expression::ExpressionHandle,
        frame: &Frame,
    ) -> Option<(PrimitiveType, ArithmeticDomain)> {
        match self.program.expression_table.expression(expression) {
            // A cast witnesses its target width AND its decision-17 S2
            // domain retag (`x as u8 in Saturating` -- the retag is what lets
            // the value join saturating arithmetic; without a written domain
            // the node carries Exact). The retag must reach fused arithmetic
            // (`(a as u8 in Saturating) + b` in a GUARD has no landing seam);
            // hardcoding Exact here let the wide 300 through while native's
            // witness read the retag and clamped.
            ExpressionNode::Cast(cast) => {
                Some((self.cast_target_primitive(cast.target_type)?, cast.domain))
            }
            ExpressionNode::Mutable(inner) => self.expression_scalar_type(*inner, frame),
            ExpressionNode::Unary(unary) => self.expression_scalar_type(unary.operand, frame),
            // A LANDED float literal witnesses its format (the F2a suffix /
            // F2b destination / F2c comparison stamps): an anonymous constant
            // guard tree (`16777216.0 + 1.0` against an f32 place) has no
            // declared destination, so the stamped literal is the node-width
            // witness that drives per-op f32 rounding in eval_float_binary.
            ExpressionNode::Float(literal) => literal.landing().map(|format| {
                (
                    match format {
                        omega_core::literals::FloatFormat::F32 => PrimitiveType::F32,
                        omega_core::literals::FloatFormat::F64 => PrimitiveType::F64,
                    },
                    ArithmeticDomain::Exact,
                )
            }),
            // A binary node computes in the PROMOTED type: mixed widths
            // auto-promote to the wider operand (u8 + i32 runs at i32 --
            // wrapping 200+100 at the node must yield 300, not the u8 44), so
            // the WIDER witness types the node. Equal widths keep the left
            // witness: add/sub/mul bits agree across signedness at one width,
            // and mixed DOMAIN classes are checker-rejected
            // (fail/expressions/arithmetic_domain_mixed).
            ExpressionNode::Binary(binary) => {
                let left = self.expression_scalar_type(binary.left, frame);
                let right = self.expression_scalar_type(binary.right, frame);
                match (left, right) {
                    (Some(left), Some(right)) => {
                        let left_width = integer_primitive_byte_width(left.0).unwrap_or(8);
                        let right_width = integer_primitive_byte_width(right.0).unwrap_or(8);
                        Some(if right_width > left_width {
                            right
                        } else if left_width > right_width || left.1 != ArithmeticDomain::Exact {
                            left
                        } else {
                            // Equal widths, left Exact: prefer the side that
                            // carries a domain (an S2 retag on the right).
                            right
                        })
                    }
                    (left, right) => left.or(right),
                }
            }
            ExpressionNode::Name(path) => {
                let members = self
                    .program
                    .expression_table
                    .name_path_members(path.members);
                // `self.field` spelled as a two-member path. NOTE: this route
                // also witnesses INDEXED element reads -- trace-verified
                // 2026-07-10n: a RUNTIME-indexed `self.sarr[i]` reaches this
                // witness as a Name([self, sarr]) whose field type peels to
                // the ELEMENT primitive + the ARRAY's domain via
                // primitive_type_reference/arithmetic_domain (a CONST-indexed
                // `self.sarr[1]` arrives as a true Indexed node and returns
                // None from the fallthrough -- its sibling operand's witness
                // covers the pair via `.or()`). Pinned by
                // pass/slices/runtime_saturating_array_element_guard_exit.
                if members.len() == 2 && members[0].as_str() == "self" {
                    return self.attached_field_scalar_type(frame, members[1].as_str());
                }
                if members.len() == 1 && members[0].as_str() != "self" {
                    return frame
                        .scalar_locals
                        .borrow()
                        .get(members[0].as_str())
                        .copied();
                }
                None
            }
            // `self.field` spelled as a Member node.
            ExpressionNode::Member(member) => {
                let receiver_is_self =
                    match self.program.expression_table.expression(member.receiver) {
                        ExpressionNode::Name(path) => {
                            let members = self
                                .program
                                .expression_table
                                .name_path_members(path.members);
                            members.len() == 1 && members[0].as_str() == "self"
                        }
                        _ => false,
                    };
                if !receiver_is_self {
                    return None;
                }
                self.attached_field_scalar_type(frame, member.member.as_str())
            }
            // A resolved value call carries the exact entry-state symbol.
            // Preserve its declared primitive/domain as the witness for an
            // enclosing operation. This matters for u64-returning helpers:
            // their high-bit results must compare and shift unsigned even
            // though the interpreter stores the raw pattern in an i64 cell.
            ExpressionNode::Call(call) if call.target_symbol.is_valid() => {
                let state = self
                    .program
                    .machines()
                    .iter()
                    .flat_map(|machine| self.program.machine_states(machine))
                    .find(|state| state.symbol == call.target_symbol)?;
                let primitive = self.program.primitive_type_reference(state.return_type)?;
                let domain = self
                    .program
                    .arithmetic_domain_for_type_reference(state.return_type);
                Some((primitive, domain))
            }
            _ => None,
        }
    }

    /// The executing machine's attached-data field's declared scalar
    /// (primitive, arithmetic-domain); `None` for a non-scalar field.
    fn attached_field_scalar_type(
        &self,
        frame: &Frame,
        field_name: &str,
    ) -> Option<(PrimitiveType, ArithmeticDomain)> {
        let machine = self
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == frame.machine_symbol)?;
        let data_name = machine.attached_data.as_ref()?;
        let data = self.find_data_by_name(data_name.as_str())?;
        self.program
            .data_members(data)
            .iter()
            .find_map(|candidate| match candidate {
                omega_checked_trees::data::DataMember::Field(field)
                    if field.name.as_str() == field_name =>
                {
                    let primitive = self
                        .program
                        .primitive_type_reference(field.type_reference)?;
                    let domain = self
                        .program
                        .arithmetic_domain_for_type_reference(field.type_reference);
                    Some((primitive, domain))
                }
                _ => None,
            })
    }

    /// Recover the authored named float operation for a root-preserving
    /// intrinsic rewrite. Provider dispatch deliberately rewrites the
    /// expression node in place, so the checked use fact remains the stable
    /// bridge from the executing binary node back to `F32::...` / `F64::...`.
    fn named_float_operation_name(&self, expression: ExpressionHandle) -> Option<&str> {
        let operator_use = self
            .operator_facts?
            .named_uses()
            .find(|operator_use| operator_use.expression == expression)?;
        let operator = self
            .program
            .operators()
            .iter()
            .find(|operator| operator.symbol == operator_use.selected_operator_symbol)?;
        let members = self.program.operator_path_members(operator.name);
        match members.first()?.as_str() {
            "F32" | "F64" => members.last().map(|member| member.as_str()),
            _ => None,
        }
    }

    /// Execute an unnameable provider builtin for a selected ternary float
    /// operation. The named contract chooses fused versus separately rounded
    /// semantics and adapts only the final result using all original operands.
    fn eval_rewritten_ternary_float(
        &mut self,
        arguments: &[ExpressionHandle],
        format: SemanticFloatFormat,
        fused: bool,
        frame: &Frame,
    ) -> EvalResult<Value> {
        let scalar_type = arguments
            .iter()
            .filter_map(|argument| self.expression_scalar_type(*argument, frame))
            .reduce(|left, right| {
                if left.1 != ArithmeticDomain::Exact {
                    left
                } else {
                    right
                }
            });
        let mut operands = Vec::with_capacity(3);
        for operand in arguments {
            let Value::Float(value) = self.eval_expression(*operand, frame)? else {
                return unsupported("selected ternary float operand is not a float");
            };
            operands.push(if format == SemanticFloatFormat::BINARY32 {
                FloatMeaning::from_f32(value as f32)
            } else {
                FloatMeaning::from_f64(value)
            });
        }
        let meaning = if fused {
            FloatSemantics::fused_multiply_add(format, &operands[0], &operands[1], &operands[2])
        } else {
            FloatSemantics::multiply_then_add(format, &operands[0], &operands[1], &operands[2])
        };
        let operation = if fused {
            "fused_multiply_add"
        } else {
            "multiply_then_add"
        };
        let meaning = match scalar_type.map(|(_, domain)| domain) {
            Some(ArithmeticDomain::Saturating) => {
                let operand_refs = operands.iter().collect::<Vec<_>>();
                FloatSemantics::apply_saturating_policy(format, &operand_refs, meaning)
            }
            Some(ArithmeticDomain::Trapping) => FloatSemantics::apply_trapping_policy(meaning)
                .map_err(|trap_class| {
                    Halt::Trap(format!(
                        "named float operation `{operation}` produced {} in Trapping domain",
                        match trap_class {
                            FloatPolicyTrap::NaNResult => "NaN",
                            FloatPolicyTrap::InfinityResult => "infinity",
                        }
                    ))
                })?,
            Some(ArithmeticDomain::Exact | ArithmeticDomain::Wrapping) | None => meaning,
        };
        Ok(Value::Float(meaning.to_interpreter_value(format)))
    }

    fn eval_binary(
        &self,
        operator: BinaryOperator,
        left: Value,
        right: Value,
        unsigned_operands: bool,
        scalar_type: Option<(PrimitiveType, ArithmeticDomain)>,
        named_float_operation: Option<&str>,
    ) -> EvalResult<Value> {
        use BinaryOperator::*;

        // Logical short-circuit-style operators (already fully evaluated here).
        if matches!(operator, And | Or) {
            let l = left
                .as_bool()
                .ok_or_else(|| Halt::Trap("logical operand not boolean".to_owned()))?;
            let r = right
                .as_bool()
                .ok_or_else(|| Halt::Trap("logical operand not boolean".to_owned()))?;
            return Ok(Value::Bool(match operator {
                And => l && r,
                Or => l || r,
                _ => unreachable!(),
            }));
        }

        // Equality / inequality across scalar kinds (incl. enums).
        if matches!(operator, Equal | NotEqual) {
            let equal = self.values_equal(&left, &right)?;
            return Ok(Value::Bool(if operator == Equal { equal } else { !equal }));
        }

        // String concatenation: `a + b` over two strings yields a fresh string.
        if let (Value::Str(a), Value::Str(b)) = (&left, &right) {
            if operator == Add {
                let mut joined = a.borrow().clone();
                joined.extend_from_slice(&b.borrow());
                return Ok(Value::bytes(joined));
            }
        }

        // Float arithmetic / comparison if either operand is float.
        if matches!(left, Value::Float(_)) || matches!(right, Value::Float(_)) {
            let l = left
                .as_float()
                .ok_or_else(|| Halt::Trap("non-numeric float operand".to_owned()))?;
            let r = right
                .as_float()
                .ok_or_else(|| Halt::Trap("non-numeric float operand".to_owned()))?;
            return self.eval_float_binary(operator, l, r, scalar_type, named_float_operation);
        }

        // Integer arithmetic / comparison. A payload-free CASE operand
        // contributes its TAG ordinal: the value-position `match` desugar
        // (parser primary.rs) produces `default + (s == p) * (variant -
        // default)`, which natively IS tag arithmetic -- the oracle must
        // compute the same integers or every errno->ErrorKind classification
        // traps on `Enum - Enum`.
        let l = self.arithmetic_operand_int(&left)?;
        let r = self.arithmetic_operand_int(&right)?;
        // Saturating/Trapping ADD/SUB/MUL clamp/trap at the OPERATION itself
        // (decision 17): compute WIDE in i128 -- two in-bounds operands cannot
        // overflow it, and it also covers the 64-bit widths, which the i64
        // landing seams cannot express (a wrapped u64 MAX+5 arrives at the
        // seam as 4 with the overflow evidence gone; only the node, holding
        // BOTH operands, can clamp). 64-bit UNSIGNED views its `Value::Int`
        // bit patterns as u64 and clamps to [0, u64::MAX]. Other domains and
        // operators keep the wide i64 compute + landing-seam coercion.
        // SIGNED div/mod under a non-Exact domain resolve MIN/-1 at the node
        // (the one overflowing corner: |quotient| otherwise shrinks):
        // Wrapping wraps it back to MIN (matching aarch64 `sdiv` and the
        // x86_64 idiv guard), Saturating clamps it to MAX (`a % -1` is 0
        // either way), Trapping traps. Division by zero keeps the existing
        // trap. Unsigned div/mod never overflow and fall through.
        if matches!(operator, Divide | Modulo) {
            if let Some((
                ty @ (PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64),
                domain @ (ArithmeticDomain::Wrapping
                | ArithmeticDomain::Saturating
                | ArithmeticDomain::Trapping),
            )) = scalar_type
            {
                if r == 0 {
                    return if operator == Divide {
                        trap("integer division by zero")
                    } else {
                        trap("integer modulo by zero")
                    };
                }
                let wide = if operator == Divide {
                    l as i128 / r as i128
                } else {
                    l as i128 % r as i128
                };
                let (min, max) = integer_bounds(ty).unwrap_or((i64::MIN, i64::MAX));
                return match domain {
                    ArithmeticDomain::Wrapping => Ok(Value::Int(wrap_to_width(wide as i64, ty))),
                    ArithmeticDomain::Saturating => {
                        Ok(Value::Int(wide.clamp(min as i128, max as i128) as i64))
                    }
                    ArithmeticDomain::Trapping if wide < min as i128 || wide > max as i128 => {
                        trap(format!(
                            "arithmetic overflow in Trapping domain: {wide} is out of range for {ty:?}"
                        ))
                    }
                    _ => Ok(Value::Int(wide as i64)),
                };
            }
        }
        // A WRAPPING Add/Sub/Mul likewise wraps at the node: with no landing
        // seam (a guard-direct `au + bu == 44`), the full-width comparison
        // would see the wide 300 while native's byte-width compare sees the
        // wrapped 44. Wrapping is congruence-preserving for +/-/* chains, so
        // truncating each intermediate agrees with native's wide-compute +
        // width-sensitive-op truncation everywhere.
        if let Some((ty, ArithmeticDomain::Wrapping)) = scalar_type {
            if matches!(operator, Add | Subtract | Multiply | ShiftLeft | ShiftRight) {
                let wide = match operator {
                    Add => l.wrapping_add(r),
                    Subtract => l.wrapping_sub(r),
                    Multiply => l.wrapping_mul(r),
                    // MASKED COUNT at the operand width (F8, ch5 shift-count
                    // ruling, settled 2026-07-18: Wrapping masks the count to
                    // `k & (width - 1)` -- the genuinely modular reading, and
                    // what the hardware computes anyway). This SUPERSEDES the
                    // 2026-07-13 modular-VALUE semantics (at-width counts no
                    // longer collapse to 0/sign-fill; they shift by the
                    // masked count). Bit-masking the two's-complement count
                    // is well-defined for negative counts too, exactly like
                    // the register-form shifts on both ISAs.
                    ShiftLeft => {
                        let masked = ((r as u64) & (primitive_bit_width(ty) - 1)) as u32;
                        l.wrapping_shl(masked)
                    }
                    ShiftRight => {
                        let masked = ((r as u64) & (primitive_bit_width(ty) - 1)) as u32;
                        if unsigned_operands {
                            ((l as u64).wrapping_shr(masked)) as i64
                        } else {
                            l.wrapping_shr(masked)
                        }
                    }
                    _ => unreachable!(),
                };
                return Ok(Value::Int(wrap_to_width(wide, ty)));
            }
        }
        if let Some((ty, domain @ (ArithmeticDomain::Saturating | ArithmeticDomain::Trapping))) =
            scalar_type
        {
            // Domain-governed SHIFTS. F8c (ch5 shift-count ruling): under
            // TRAPPING an out-of-range count TRAPS -- regardless of the
            // shifted VALUE (`0 << 40` traps; the count is invalid, not the
            // result). Saturating cannot reach an out-of-range count (the
            // F8a validation obligation rejects it), so its floor/clamp arms
            // below only ever see in-range counts.
            if domain == ArithmeticDomain::Trapping
                && matches!(operator, ShiftLeft | ShiftRight)
                && (r as u64) >= primitive_bit_width(ty)
            {
                return trap(format!(
                    "shift count out of range in Trapping domain: the count is not below \
                     the operand width for {ty:?}"
                ));
            }
            // `>>` is floor(x / 2^n) and cannot overflow; the Saturating
            // floor semantics for an (unreachable) at/above-width count stay
            // for robustness.
            if operator == ShiftRight {
                return Ok(Value::Int(wrap_to_width(
                    if (r as u64) >= primitive_bit_width(ty) {
                        if unsigned_operands || l >= 0 { 0 } else { -1 }
                    } else if unsigned_operands {
                        ((l as u64).wrapping_shr(r as u32)) as i64
                    } else {
                        l.wrapping_shr(r as u32)
                    },
                    ty,
                )));
            }
            // `<<` is x * 2^n: Saturating clamps and Trapping traps when the
            // TRUE value leaves the type's range (in-range counts only here
            // -- the count trap above owns the out-of-range face).
            if operator == ShiftLeft {
                let (minimum, maximum, value) = if primitive_is_unsigned64(Some(ty)) {
                    (0i128, u64::MAX as i128, l as u64 as i128)
                } else {
                    let (minimum, maximum) = integer_bounds(ty).unwrap_or((i64::MIN, i64::MAX));
                    (minimum as i128, maximum as i128, l as i128)
                };
                let wide = if (r as u64) >= primitive_bit_width(ty) {
                    // Saturating only (Trapping trapped above): any nonzero x
                    // overflows once the count reaches the width; drive the
                    // clamp below with a synthetic out-of-range value on x's
                    // side of the range.
                    match value.signum() {
                        0 => 0,
                        1 => maximum + 1,
                        _ => minimum - 1,
                    }
                } else {
                    value << (r as u32)
                };
                return match domain {
                    ArithmeticDomain::Saturating => {
                        Ok(Value::Int(wide.clamp(minimum, maximum) as i64))
                    }
                    ArithmeticDomain::Trapping if wide < minimum || wide > maximum => {
                        trap(format!(
                            "arithmetic overflow in Trapping domain: shifted value is out of range for {ty:?}"
                        ))
                    }
                    _ => Ok(Value::Int(wide as i64)),
                };
            }
            if matches!(operator, Add | Subtract | Multiply) {
                let bounds_and_wide = if primitive_is_unsigned64(Some(ty)) {
                    let (lu, ru) = (l as u64 as i128, r as u64 as i128);
                    let wide = match operator {
                        Add => lu + ru,
                        Subtract => lu - ru,
                        Multiply => lu * ru,
                        _ => unreachable!(),
                    };
                    Some((0i128, u64::MAX as i128, wide))
                } else if let Some((min, max)) = integer_bounds(ty) {
                    let wide = match operator {
                        Add => l as i128 + r as i128,
                        Subtract => l as i128 - r as i128,
                        Multiply => l as i128 * r as i128,
                        _ => unreachable!(),
                    };
                    Some((min as i128, max as i128, wide))
                } else {
                    None
                };
                if let Some((min, max, wide)) = bounds_and_wide {
                    return match domain {
                        ArithmeticDomain::Saturating => Ok(Value::Int(wide.clamp(min, max) as i64)),
                        ArithmeticDomain::Trapping if wide < min || wide > max => trap(format!(
                            "arithmetic overflow in Trapping domain: {wide} is out of range for {ty:?}"
                        )),
                        _ => Ok(Value::Int(wide as i64)),
                    };
                }
            }
        }
        self.eval_int_binary(operator, l, r, unsigned_operands)
    }

    /// The VIRTUAL TimeHost read ops (std::time rung 4, D12). The
    /// interpreter's clock is deterministic: `sleep` advances virtual_ticks
    /// by the slept milliseconds and these reads never advance it, so interp
    /// canaries assert EXACT values. Calibration: 1 tick = 1 ms (frequency
    /// 1000); wall clock = 2026-01-01T00:00:00Z + elapsed, already in Unix
    /// units (epoch offset 0). Native rebinds these to real clocks (rung 5)
    /// and its canaries assert inequalities instead.
    fn virtual_time_host_value(&self, target: &str) -> Option<Value> {
        match target {
            "monotonic_ticks" => Some(Value::Int(self.virtual_ticks)),
            "monotonic_ticks_per_second" => Some(Value::Int(1000)),
            "wall_clock_raw" => Some(Value::Int(1767225600000 + self.virtual_ticks)),
            "wall_clock_units_per_second" => Some(Value::Int(1000)),
            "wall_clock_epoch_offset_seconds" => Some(Value::Int(0)),
            _ => None,
        }
    }

    fn eval_int_binary(
        &self,
        operator: BinaryOperator,
        l: i64,
        r: i64,
        unsigned_operands: bool,
    ) -> EvalResult<Value> {
        use BinaryOperator::*;
        Ok(match operator {
            Add => Value::Int(l.wrapping_add(r)),
            Subtract => Value::Int(l.wrapping_sub(r)),
            Multiply => Value::Int(l.wrapping_mul(r)),
            Divide => {
                if r == 0 {
                    return trap("integer division by zero");
                }
                if unsigned_operands {
                    Value::Int(((l as u64).wrapping_div(r as u64)) as i64)
                } else {
                    Value::Int(l.wrapping_div(r))
                }
            }
            Modulo => {
                if r == 0 {
                    return trap("integer modulo by zero");
                }
                if unsigned_operands {
                    Value::Int(((l as u64).wrapping_rem(r as u64)) as i64)
                } else {
                    Value::Int(l.wrapping_rem(r))
                }
            }
            ShiftLeft => Value::Int(l.wrapping_shl(r as u32)),
            // Logical (unsigned) shift when the operand is u64-classed;
            // arithmetic shift otherwise.
            ShiftRight if unsigned_operands => {
                Value::Int(((l as u64).wrapping_shr(r as u32)) as i64)
            }
            ShiftRight => Value::Int(l.wrapping_shr(r as u32)),
            BitwiseAnd => Value::Int(l & r),
            BitwiseOr => Value::Int(l | r),
            BitwiseXor => Value::Int(l ^ r),
            Less if unsigned_operands => Value::Bool((l as u64) < (r as u64)),
            LessOrEqual if unsigned_operands => Value::Bool((l as u64) <= (r as u64)),
            Greater if unsigned_operands => Value::Bool((l as u64) > (r as u64)),
            GreaterOrEqual if unsigned_operands => Value::Bool((l as u64) >= (r as u64)),
            Less => Value::Bool(l < r),
            LessOrEqual => Value::Bool(l <= r),
            Greater => Value::Bool(l > r),
            GreaterOrEqual => Value::Bool(l >= r),
            Equal | NotEqual | And | Or => unreachable!("handled earlier"),
        })
    }

    fn eval_float_binary(
        &self,
        operator: BinaryOperator,
        l: f64,
        r: f64,
        scalar_type: Option<(PrimitiveType, ArithmeticDomain)>,
        named_float_operation: Option<&str>,
    ) -> EvalResult<Value> {
        use BinaryOperator::*;
        // Decode landed values to the shared proof meaning, execute exact
        // arithmetic, and round once through the selected format record. The
        // interpreter's f64 storage is only a lossless window for f32 values;
        // it is not the arithmetic definition.
        let format = if matches!(scalar_type, Some((PrimitiveType::F32, _))) {
            SemanticFloatFormat::BINARY32
        } else {
            SemanticFloatFormat::BINARY64
        };
        let decode = |value: f64| {
            if format == SemanticFloatFormat::BINARY32 {
                FloatMeaning::from_f32(value as f32)
            } else {
                FloatMeaning::from_f64(value)
            }
        };
        let left = decode(l);
        let right = decode(r);
        // F7 policy adapters (float brief §8): SATURATING clamps MAGNITUDE
        // OVERFLOW only; division by zero, invalid results, and non-finite
        // propagation remain non-finite. TRAPPING is result-checked, so a
        // propagated NaN/infinity traps just like one created by this
        // operation. The shared semantic adapter owns those decisions; the
        // interpreter only selects it and renders a specific trap reason.
        let domain = scalar_type.map(|(_, domain)| domain);
        let arith = |meaning: FloatMeaning, division: bool| -> EvalResult<Value> {
            let meaning = match domain {
                Some(ArithmeticDomain::Saturating) if division => {
                    FloatSemantics::apply_saturating_divide_policy(format, &left, &right, meaning)
                }
                Some(ArithmeticDomain::Saturating) => {
                    FloatSemantics::apply_saturating_policy(format, &[&left, &right], meaning)
                }
                Some(ArithmeticDomain::Trapping) => {
                    match FloatSemantics::apply_trapping_policy(meaning) {
                        Ok(finite) => finite,
                        Err(FloatPolicyTrap::NaNResult) => {
                            if let Some(operation) = named_float_operation {
                                return trap(format!(
                                    "named float operation `{operation}` produced NaN in Trapping domain"
                                ));
                            }
                            let reason = if left.is_finite() && right.is_finite() {
                                "invalid float operation in Trapping domain"
                            } else {
                                "non-finite NaN result in Trapping domain"
                            };
                            return trap(reason.to_owned());
                        }
                        Err(FloatPolicyTrap::InfinityResult) => {
                            if let Some(operation) = named_float_operation {
                                return trap(format!(
                                    "named float operation `{operation}` produced infinity in Trapping domain"
                                ));
                            }
                            let reason = if division && right.is_zero() {
                                "float division by zero in Trapping domain"
                            } else if left.is_finite() && right.is_finite() {
                                "float overflow in Trapping domain"
                            } else {
                                "non-finite infinity result in Trapping domain"
                            };
                            return trap(reason.to_owned());
                        }
                    }
                }
                _ => meaning,
            };
            let landed = meaning.to_interpreter_value(format);
            Ok(Value::Float(landed))
        };
        Ok(match operator {
            Add => return arith(FloatSemantics::add(format, &left, &right), false),
            Subtract => return arith(FloatSemantics::subtract(format, &left, &right), false),
            Multiply => return arith(FloatSemantics::multiply(format, &left, &right), false),
            Divide => return arith(FloatSemantics::divide(format, &left, &right), true),
            Less => Value::Bool(FloatSemantics::less(&left, &right)),
            LessOrEqual => Value::Bool(FloatSemantics::less_or_equal(&left, &right)),
            Greater => Value::Bool(FloatSemantics::greater(&left, &right)),
            GreaterOrEqual => Value::Bool(FloatSemantics::greater_or_equal(&left, &right)),
            Modulo | ShiftLeft | ShiftRight | BitwiseAnd | BitwiseOr | BitwiseXor => {
                return unsupported("float modulo/shift/bitwise not supported");
            }
            Equal | NotEqual | And | Or => unreachable!("handled earlier"),
        })
    }

    fn eval_min_max(
        &self,
        name: &str,
        left: Value,
        right: Value,
        unsigned: bool,
    ) -> EvalResult<Value> {
        if matches!(left, Value::Float(_)) || matches!(right, Value::Float(_)) {
            let l = left
                .as_float()
                .ok_or_else(|| Halt::Trap("min/max float".to_owned()))?;
            let r = right
                .as_float()
                .ok_or_else(|| Halt::Trap("min/max float".to_owned()))?;
            // Match the native SSE semantics exactly: `maxsd a, b` returns b
            // when the values are unordered (any NaN) or equal, and the larger
            // otherwise -- i.e. `if a > b { a } else { b }` (partial `>` is
            // false for NaN). `minsd` is the mirror. Rust's `f64::max`/`min`
            // differ (they return the non-NaN operand), which would diverge
            // from the backend on a NaN second operand.
            let left_meaning = FloatMeaning::from_f64(l);
            let right_meaning = FloatMeaning::from_f64(r);
            let pick_left = if name == "max" {
                FloatSemantics::greater(&left_meaning, &right_meaning)
            } else {
                FloatSemantics::less(&left_meaning, &right_meaning)
            };
            // Preserve the selected runtime operand's honest NaN bits even
            // though the base FloatMeaning contract erases their payload.
            return Ok(Value::Float(if pick_left { l } else { r }));
        }
        let l = left
            .as_int()
            .ok_or_else(|| Halt::Trap("min/max int".to_owned()))?;
        let r = right
            .as_int()
            .ok_or_else(|| Halt::Trap("min/max int".to_owned()))?;
        // Compare as u64 when a u64-classed operand is present (the larger/smaller
        // u64 bit pattern IS one of {l, r}, reinterpreted back to i64); signed
        // otherwise. Without this `max`/`min` on an msb-set u64 picks the wrong
        // operand (u64::MAX reads as -1 under signed compare).
        let picked = if unsigned {
            let (lu, ru) = (l as u64, r as u64);
            (if name == "max" {
                lu.max(ru)
            } else {
                lu.min(ru)
            }) as i64
        } else if name == "max" {
            l.max(r)
        } else {
            l.min(r)
        };
        Ok(Value::Int(picked))
    }

    fn values_equal(&self, left: &Value, right: &Value) -> EvalResult<bool> {
        Ok(match (left, right) {
            // Enum equality is a TAG compare only -- a case-pattern guard desugars to
            // `subject == Type::Case` where the right side is a bare (payload-less)
            // case reference, and the native backend compares the constant tag.
            // Payloads participate in `==` only through Equatable synthesis, and the
            // FRONTEND expands that into explicit tag-guarded payload field compares
            // before the interpreter runs, so this compare stays tag-only.
            (
                Value::Enum {
                    variant_name: a, ..
                },
                Value::Enum {
                    variant_name: b, ..
                },
            ) => a == b,
            // A tag INT beside a case value: the value-position `match` desugar
            // computes its result as TAG ARITHMETIC (an Int), which then flows
            // into enum-typed places -- natively both sides are the same tag
            // constant, so the oracle compares the Int against the case's tag
            // ordinal.
            (
                Value::Int(tag),
                Value::Enum {
                    type_symbol,
                    variant_name,
                    ..
                },
            )
            | (
                Value::Enum {
                    type_symbol,
                    variant_name,
                    ..
                },
                Value::Int(tag),
            ) => self.enum_variant_tag(*type_symbol, variant_name) == Some(*tag),
            (Value::Str(a), Value::Str(b)) => *a.borrow() == *b.borrow(),
            (Value::Bool(a), Value::Bool(b)) => a == b,
            _ => {
                if matches!(left, Value::Float(_)) || matches!(right, Value::Float(_)) {
                    let left = FloatMeaning::from_f64(left.as_float().unwrap_or(f64::NAN));
                    let right = FloatMeaning::from_f64(right.as_float().unwrap_or(f64::NAN));
                    FloatSemantics::equal(&left, &right)
                } else {
                    left.as_int() == right.as_int()
                }
            }
        })
    }

    /// An integer ARITHMETIC operand: a scalar's value, or a payload-free case
    /// value's TAG ordinal (the value-position `match` desugar does tag
    /// arithmetic over bare cases -- natively a case IS its tag constant).
    fn arithmetic_operand_int(&self, value: &Value) -> EvalResult<i64> {
        if let Some(int) = value.as_int() {
            return Ok(int);
        }
        if let Value::Enum {
            type_symbol,
            variant_name,
            payload,
        } = value
            && payload.is_empty()
            && let Some(tag) = self.enum_variant_tag(*type_symbol, variant_name)
        {
            return Ok(tag);
        }
        Err(Halt::Trap("non-integer operand".to_owned()))
    }

    /// The tag ORDINAL of a case: resolved WITHIN the declaring type when the
    /// value carries a valid `type_symbol` (tag 0 = the first variant,
    /// matching the ZII zero case and native tag layout), so same-name
    /// variants at different ordinals across enums (`Ok` = 0 in `UnitResult`
    /// but 1 in `MetadataResult`) never cross-resolve. A symbol-less value
    /// (the build-time boundary) falls back to the name-global scan -- the
    /// same name-keyed grain `values_equal` uses for enum equality.
    fn enum_variant_tag(&self, type_symbol: SymbolHandle, variant_name: &str) -> Option<i64> {
        let ordinal_in = |data: &DataDefinition| {
            let mut ordinal: i64 = 0;
            for member in self.program.data_members(data) {
                if let DataMember::Variant(variant) = member {
                    if variant.name.as_str() == variant_name {
                        return Some(ordinal);
                    }
                    ordinal += 1;
                }
            }
            None
        };
        if type_symbol.is_valid() {
            let data = self
                .program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == type_symbol)?;
            return ordinal_in(data);
        }
        self.program.data_definitions().iter().find_map(ordinal_in)
    }

    // ---- lookups ------------------------------------------------------------

    fn find_machine_by_name(&self, name: &str) -> Option<&'program Machine> {
        self.program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
    }

    fn find_state(&self, machine: &Machine, name: &str) -> Option<&State> {
        self.program
            .machine_states(machine)
            .iter()
            .find(|state| state.name.as_str() == name)
    }

    fn find_data_by_name(&self, name: &str) -> Option<&'program DataDefinition> {
        self.program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == name)
    }
}

/// The canonical Console host-boundary method names the interpreter drives directly.
fn is_canonical_host_method(name: &str) -> bool {
    matches!(
        name,
        "write"
            | "write_line"
            | "write_error"
            | "write_error_line"
            | "read_line"
            | "read_byte"
            | "write_byte"
            | "exit_process"
            | "sleep"
            | "tick_count"
            | "key_state"
            | "dc_create"
            | "get_dc"
            | "window_create"
            | "blit"
            | "msg_peek"
            | "msg_translate"
            | "msg_dispatch"
            | "is_window"
            | "window_destroy"
            | "foreground_window"
    )
}

/// Reinterpret an i64 at an integer primitive's width, sign- or zero-extending back to i64
/// so the value carries the same numeric meaning the target type would observe. `u8` 250
/// stays 250; `i8` 250 wraps to -6; `u32` of a negative becomes its 32-bit unsigned value.
/// zigzag(n) = (n << 1) ^ (n >> 63): the signed-scalar pre-step of the
/// compact_binary v0 varint, identical to the native encoders' shift/xor.
/// One CURRENT-era field of a wire schema, as the interpreter's encoder sees
/// it: a directly encodable scalar/String, or a nested message's scalar-only
/// field list (chapter 20).
enum WireInterpField {
    Direct(omega_typed_trees::wire::WireFieldEncoding),
    Nested(Vec<(String, u64, omega_typed_trees::wire::WireScalarEncoding)>),
    Repeated(omega_typed_trees::wire::WireRepeatedEncoding),
    ScalarSlice(omega_typed_trees::wire::WireBorrowedScalarSliceEncoding),
    /// A borrowed byte slice `&[u8]`: encodes as RAW bytes (length varint then
    /// the bytes), reading the field's element array.
    ByteSlice,
}

/// One CURRENT-era field of a wire schema, as the interpreter's decoder sees
/// it. An owned `String` is encode-only, but a borrowed `&[u8]` byte slice
/// decodes ZERO-COPY as a length-prefixed view of the buffer (`ByteSlice`).
enum WireInterpScalarField {
    Scalar {
        encoding: omega_typed_trees::wire::WireScalarEncoding,
        range: Option<omega_core::wire::WireScalarRange>,
    },
    Nested(
        Vec<(
            String,
            u64,
            omega_typed_trees::wire::WireScalarEncoding,
            Option<omega_core::wire::WireScalarRange>,
        )>,
    ),
    Repeated {
        encoding: omega_typed_trees::wire::WireRepeatedEncoding,
        range: Option<omega_core::wire::WireScalarRange>,
    },
    /// A borrowed `&[u8]` field: read a byte-length varint then that many bytes
    /// from the buffer. Stored as an owned `Array` of byte values --
    /// observationally identical to a zero-copy view for any read. The
    /// `predicates` are the slice's declared byte-domain obligations,
    /// evaluated over the UNTRUSTED wire bytes at the decode boundary.
    ByteSlice {
        predicates: Vec<omega_typed_trees::byte_predicates::ByteSequencePredicate>,
    },
}

/// The CURRENT-era (name, number, scalar encoding) list of a nested wire
/// schema, sorted by field number -- validation has already guaranteed the
/// scalar-only child body.
fn wire_nested_scalar_fields(
    program: &TypedTrees,
    child: &omega_typed_trees::wire::WireSchema,
) -> Result<Vec<(String, u64, omega_typed_trees::wire::WireScalarEncoding)>, Halt> {
    use omega_typed_trees::wire::{WireMember, WireScalarEncoding};

    let mut children = Vec::new();
    for member in program.wire_members(child.members) {
        let WireMember::Field(field) = member else {
            continue;
        };
        let scalar = program
            .primitive_type_reference(field.type_reference)
            .and_then(WireScalarEncoding::for_primitive)
            .ok_or_else(|| {
                Halt::Unsupported(format!(
                    "data `{}` nested field `{}` is not a stage 2 scalar",
                    child.name, field.name
                ))
            })?;
        children.push((field.name.as_str().to_owned(), field.number, scalar));
    }
    children.sort_by_key(|(_, number, _)| *number);
    Ok(children)
}

/// Decode-side nested fields additionally carry the destination declaration's
/// range, because the schema primitive alone does not contain that fact.
fn wire_nested_decode_scalar_fields(
    program: &TypedTrees,
    child: &omega_typed_trees::wire::WireSchema,
    value_type: TypeReferenceHandle,
) -> Result<
    Vec<(
        String,
        u64,
        omega_typed_trees::wire::WireScalarEncoding,
        Option<omega_core::wire::WireScalarRange>,
    )>,
    Halt,
> {
    use omega_typed_trees::wire::{WireMember, WireScalarEncoding};

    let mut children = Vec::new();
    for member in program.wire_members(child.members) {
        let WireMember::Field(field) = member else {
            continue;
        };
        let target_type =
            omega_typed_trees::wire::data_field_type(program, value_type, field.name.as_str())
                .ok_or_else(|| {
                    Halt::Unsupported(format!(
                        "data `{}` nested destination has no field `{}`",
                        child.name, field.name
                    ))
                })?;
        let scalar = program
            .primitive_type_reference(field.type_reference)
            .and_then(WireScalarEncoding::for_primitive)
            .ok_or_else(|| {
                Halt::Unsupported(format!(
                    "data `{}` nested field `{}` is not a stage 2 scalar",
                    child.name, field.name
                ))
            })?;
        children.push((
            field.name.as_str().to_owned(),
            field.number,
            scalar,
            omega_typed_trees::wire::scalar_decode_range(program, target_type),
        ));
    }
    children.sort_by_key(|(_, number, _, _)| *number);
    Ok(children)
}

fn wire_argument_declared_type(
    program: &TypedTrees,
    frame: &Frame,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => wire_argument_declared_type(program, frame, *inner),
        ExpressionNode::Member(member) => {
            let receiver = wire_argument_declared_type(program, frame, member.receiver)?;
            omega_typed_trees::wire::data_field_type(program, receiver, member.member.as_str())
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let mut current = *frame.type_locals.borrow().get(members.first()?.as_str())?;
            for member in members.iter().skip(1) {
                current =
                    omega_typed_trees::wire::data_field_type(program, current, member.as_str())?;
            }
            Some(current)
        }
        _ => None,
    }
}

fn wire_scalar_in_range(
    raw: u64,
    encoding: omega_typed_trees::wire::WireScalarEncoding,
    value: &Value,
    range: omega_core::wire::WireScalarRange,
) -> bool {
    if range.signed {
        let value = if encoding.zigzag {
            unzigzag64(raw)
        } else {
            let Some(value) = value.as_int() else {
                return false;
            };
            value
        };
        value >= range.minimum && value <= range.maximum
    } else {
        raw >= range.minimum as u64 && raw <= range.maximum as u64
    }
}

/// The unsigned LEB128 payload a scalar value encodes as -- the same
/// widths/signedness the native encoders apply: load at the source width
/// (zero- or sign-extending), zigzag signed sources at 64 bits.
fn wire_scalar_varint_value(
    raw: i64,
    scalar: omega_typed_trees::wire::WireScalarEncoding,
) -> Result<u64, Halt> {
    match (scalar.byte_size, scalar.zigzag) {
        (1, _) => Ok(u64::from(raw != 0)),
        (4, false) => Ok(u64::from(raw as u32)),
        (8, false) => Ok(raw as u64),
        (4, true) => Ok(zigzag64(i64::from(raw as i32))),
        (8, true) => Ok(zigzag64(raw)),
        _ => Err(Halt::Unsupported(format!(
            "wire scalar of {} bytes",
            scalar.byte_size
        ))),
    }
}

/// The decoded value a raw LEB128 payload produces -- the same
/// widths/signedness the native decoders apply: truncate to the field width,
/// un-zigzag signed targets at 64 bits first.
fn wire_decoded_scalar_value(
    raw: u64,
    encoding: omega_typed_trees::wire::WireScalarEncoding,
) -> Result<Value, Halt> {
    match (encoding.byte_size, encoding.zigzag) {
        (1, _) => Ok(Value::Bool((raw & 0xff) != 0)),
        (4, false) => Ok(Value::Int(i64::from(raw as u32))),
        (8, false) => Ok(Value::Int(raw as i64)),
        (4, true) => Ok(Value::Int(i64::from(unzigzag64(raw) as i32))),
        (8, true) => Ok(Value::Int(unzigzag64(raw))),
        _ => Err(Halt::Unsupported(format!(
            "wire scalar of {} bytes",
            encoding.byte_size
        ))),
    }
}

fn zigzag64(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

/// unzigzag(n) = (n >> 1) ^ -(n & 1): the signed-scalar post-step of the
/// compact_binary v0 varint decode, identical to the native decoders'
/// shift/mask/xor.
fn unzigzag64(value: u64) -> i64 {
    ((value >> 1) ^ (value & 1).wrapping_neg()) as i64
}

/// Inclusive [min, max] of an integer primitive as i64. `None` for widths whose
/// range cannot be represented in i64 (u64/usize) -- their saturating/trapping
/// behaviour is not modelled by the interpreter yet (they fall back to wrap).
fn integer_bounds(ty: PrimitiveType) -> Option<(i64, i64)> {
    match ty {
        PrimitiveType::I8 => Some((i8::MIN as i64, i8::MAX as i64)),
        PrimitiveType::U8 => Some((0, u8::MAX as i64)),
        PrimitiveType::I16 => Some((i16::MIN as i64, i16::MAX as i64)),
        PrimitiveType::U16 => Some((0, u16::MAX as i64)),
        PrimitiveType::I32 => Some((i32::MIN as i64, i32::MAX as i64)),
        PrimitiveType::U32 => Some((0, u32::MAX as i64)),
        PrimitiveType::I64 => Some((i64::MIN, i64::MAX)),
        _ => None,
    }
}

fn semantic_integer_format(ty: PrimitiveType) -> Option<SemanticIntegerFormat> {
    match ty {
        PrimitiveType::I8 => Some(SemanticIntegerFormat::I8),
        PrimitiveType::I16 => Some(SemanticIntegerFormat::I16),
        PrimitiveType::I32 => Some(SemanticIntegerFormat::I32),
        PrimitiveType::I64 => Some(SemanticIntegerFormat::I64),
        PrimitiveType::U8 => Some(SemanticIntegerFormat::U8),
        PrimitiveType::U16 => Some(SemanticIntegerFormat::U16),
        PrimitiveType::U32 => Some(SemanticIntegerFormat::U32),
        PrimitiveType::U64 | PrimitiveType::Addr => Some(SemanticIntegerFormat::U64),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => None,
    }
}

fn is_unsigned_integer_primitive(ty: PrimitiveType) -> bool {
    matches!(
        ty,
        PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
            | PrimitiveType::Addr
    )
}

fn big_integer_runtime_value(value: &BigInt, ty: PrimitiveType) -> i64 {
    if is_unsigned_integer_primitive(ty) {
        value
            .to_u64()
            .expect("checked unsigned conversion fits its target") as i64
    } else {
        value
            .to_i64()
            .expect("checked signed conversion fits its target")
    }
}

fn float_to_integer_trap_message(
    target: PrimitiveType,
    reason: FloatToIntegerError,
    trapping: bool,
) -> String {
    let operation = if trapping { "Trapping" } else { "Exact" };
    let reason = match reason {
        FloatToIntegerError::NonFinite => "the value is not finite",
        FloatToIntegerError::OutOfRange => "the truncated value is out of range",
    };
    format!("float-to-int conversion failed in {operation} domain: {reason} for {target:?}")
}

/// Preserve an f32 NaN's sign and payload while carrying interpreter floats in
/// the existing f64-backed `Value::Float`. Finite values and infinities remain
/// ordinary numeric f64 values; only NaNs use this reversible payload embedding.
fn interpreter_f32_from_bits(bits: u32) -> f64 {
    let value = f32::from_bits(bits);
    if !value.is_nan() {
        return value as f64;
    }

    let sign = ((bits as u64) >> 31) << 63;
    let payload = (u64::from(bits) & 0x007f_ffff) << 29;
    f64::from_bits(sign | 0x7ff0_0000_0000_0000 | payload)
}

fn interpreter_f32_to_bits(value: f64) -> u32 {
    if !value.is_nan() {
        return (value as f32).to_bits();
    }

    let bits = value.to_bits();
    let sign = ((bits >> 63) as u32) << 31;
    let mut payload = ((bits & 0x000f_ffff_ffff_ffff) >> 29) as u32;
    payload &= 0x007f_ffff;
    if payload == 0 {
        payload = 0x0040_0000;
    }
    sign | 0x7f80_0000 | payload
}

/// Apply a write target's arithmetic domain (decision 17) to a raw i64 result,
/// mirroring the native backend so the differential oracle agrees:
/// Exact/Wrapping truncate to width; Saturating clamps to [min, max]; Trapping
/// halts (overflow trap) when the value is out of range.
fn apply_arithmetic_domain(
    raw: i64,
    ty: PrimitiveType,
    domain: ArithmeticDomain,
) -> EvalResult<i64> {
    match domain {
        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => Ok(wrap_to_width(raw, ty)),
        ArithmeticDomain::Saturating => match integer_bounds(ty) {
            Some((min, max)) => Ok(raw.clamp(min, max)),
            None => Ok(wrap_to_width(raw, ty)),
        },
        ArithmeticDomain::Trapping => match integer_bounds(ty) {
            Some((min, max)) if raw < min || raw > max => trap(format!(
                "arithmetic overflow in Trapping domain: {raw} is out of range for {ty:?}"
            )),
            _ => Ok(wrap_to_width(raw, ty)),
        },
    }
}

/// The bit width a WRAPPING shift wraps at (the modular-arithmetic modulus
/// exponent). Pointer-width types are 64-bit in both engines.
fn primitive_bit_width(ty: PrimitiveType) -> u64 {
    match ty {
        PrimitiveType::I8 | PrimitiveType::U8 => 8,
        PrimitiveType::I16 | PrimitiveType::U16 => 16,
        PrimitiveType::I32 | PrimitiveType::U32 => 32,
        _ => 64,
    }
}

fn wrap_to_width(raw: i64, ty: PrimitiveType) -> i64 {
    match ty {
        PrimitiveType::I8 => raw as i8 as i64,
        PrimitiveType::U8 => raw as u8 as i64,
        PrimitiveType::I16 => raw as i16 as i64,
        PrimitiveType::U16 => raw as u16 as i64,
        PrimitiveType::I32 => raw as i32 as i64,
        PrimitiveType::U32 => raw as u32 as i64,
        // 64-bit and pointer-width types keep the full value (unsigned reinterpretation of a
        // u64 is still represented by the same bit pattern in i64).
        PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::Addr => raw,
        // Non-integer primitives do not reach this path.
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => raw,
    }
}

/// What a satisfied transition decided to do next.
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

enum TransitionDecision {
    Terminal,
    SelfTarget,
    Value(Value),
    Named {
        state_name: String,
        machine: Machine,
        instance: Cell,
        args: Vec<EvaluatedArgument>,
    },
}

// `Frame::locals` needs interior mutability so `let` bindings can be added while the
// frame is shared by `&`. Wrap the map in a RefCell.
/// Byte width of an integer primitive -- the PROMOTION rank a mixed-width
/// binary node computes in. `None` for non-integer primitives.
fn integer_primitive_byte_width(ty: PrimitiveType) -> Option<usize> {
    match ty {
        PrimitiveType::I8 | PrimitiveType::U8 => Some(1),
        PrimitiveType::I16 | PrimitiveType::U16 => Some(2),
        PrimitiveType::I32 | PrimitiveType::U32 => Some(4),
        PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::Addr => Some(8),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => None,
    }
}

fn primitive_is_unsigned64(primitive: Option<PrimitiveType>) -> bool {
    matches!(primitive, Some(PrimitiveType::U64 | PrimitiveType::Addr))
}

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
