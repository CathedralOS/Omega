mod aggregate_layout;
mod callback_materializations;
mod darwin;
mod linux;
mod plans;
mod stack_realizations;
mod windows;
pub use aggregate_layout::{
    AggregateLayoutError, ConventionalSumCaseLayout, ConventionalSumLayout, PackedFieldLayout,
    evaluate_conventional_sum_layout,
};
pub use callback_materializations::{
    CallbackBinderRequirement, CallbackMaterialization, CallbackMaterializationContext,
    CallbackRequirementId, LayoutPlanId, LayoutSlotId, NativeCallbackDemand,
    NativeParameterApplication, NativeParameterId, NativePlace, StaticMachineBinderId,
    callback_layout_field_slot_id, callback_layout_plan_id, callback_layout_slot_id,
    callback_native_parameter_id, callback_plan_laid_layout_id, callback_requirement_id,
    nominal_callback_native_parameter_id,
};
pub use darwin::{
    DARWIN_COREGRAPHICS_PATH, DARWIN_LIBOBJC_PATH, DARWIN_LIBSYSTEM_PATH, darwin_import_library,
};
pub use linux::{linux_clock_gettime_syscall_number, linux_nanosleep_syscall_number};
pub use plans::{
    BoundaryEntryPlan, BoundaryPlanDiagnostic, BoundaryPlanResult, CallPlan, CallSignature,
    CallingPolicy, CallingPolicyRejection, ConcreteVariadicCallSignature, EntryControl, EntryStack,
    IndirectPointerLocation, MachineRegime, MachineRegister, MachineState, MachineStateSet,
    PlanDiagnostic, Preemption, ProviderExitRealization, RegisterSet, StateFootprintEvidence,
    StatePlan, SystemVEightbyteClass, ValidatedBoundaryEntryPlan, ValueClass, ValueLocation,
    ValuePlacement, ValueShape, compose_state_footprints, evaluate_call_plan,
    evaluate_darwin_aapcs64_variadic_boundary_entry_plan,
    evaluate_darwin_aapcs64_variadic_call_plan, evaluate_freestanding_program_entry_plan,
    evaluate_ordinary_boundary_entry_plan, validate_boundary_entry_plan,
    validate_boundary_entry_plan_with_callback_materializations, validate_boundary_plan_result,
    validate_call_plan, validate_call_return_mechanics_footprint,
    validate_composed_state_footprint, validate_outbound_call_footprint,
    validate_provider_exit_realization, validate_runtime_value_guard_footprint,
    validate_state_footprint,
};
pub use stack_realizations::{
    ArrivalContextId, ArrivalContextRealization, ArrivalContextStackDomain, EntryStackEpoch,
    EntryStackRealization, EntryStackStage, InstalledEntryFactIdentity, StackDomainRef,
    StackOccupancy, ValidatedEntryStackDomainClosure, ValidatedEntryStackRealization,
    ValidatedX86_64InstalledHardwareEntryFacts, X86_64ArrivalMechanism, X86_64GateKind,
    X86_64HardwareStackSelection, X86_64InstalledArrivalContext, X86_64InstalledGateArrival,
    X86_64InstalledGateRealization, X86_64InstalledGateTssRealization,
    X86_64InstalledHardwareEntryFacts, X86_64InstalledInterruptStack,
    X86_64InstalledPrivilegeStack, X86_64InstalledTaskStateSegmentRealization,
    X86_64TargetDerivedHardwareArrival, X86_64TargetProfileIdentity,
    derive_x86_64_hardware_arrival, validate_entry_stack_domain_closure,
    validate_entry_stack_realization, validate_x86_64_installed_hardware_entry_facts,
};
pub use windows::windows_import_library;

use target::NormalizedForeignLocator;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostOperationKey {
    pub capability: HostCapability,
    pub operation: HostOperation,
}

impl HostOperationKey {
    pub const fn new(capability: HostCapability, operation: HostOperation) -> Self {
        Self {
            capability,
            operation,
        }
    }

    pub fn capability_name(self) -> &'static str {
        self.capability.name()
    }

    pub fn operation_name(self) -> &'static str {
        self.operation.name()
    }

    pub fn from_names(capability: &str, operation: &str) -> Self {
        Self::new(
            HostCapability::from_name(capability),
            HostOperation::from_name(operation),
        )
    }

    /// Whether this host op returns a value into a caller storage place (the
    /// assignment form `self.x = self.h.op(..)`), so the lowering marshals a
    /// leading result operand and the callee's return register is stored back.
    /// The raw `Filesystem` ops each return their syscall result (fd / byte
    /// count / rc). Other value-returning ops (Gui/Input/Clock) are x86_64-only
    /// and handled by the x86_64-specific relocation sites, so only the
    /// aarch64-reachable fs ops need to be recognized here.
    pub fn returns_value(self) -> bool {
        // Clock is OP-AWARE: its value reads return, `sleep`/`sleep_poll` do
        // not. (Capability-keyed only, the aarch64 routing sent the std::time
        // reads to the NON-returning encoder and silently dropped results --
        // TASKS_TIME.md rung 10 recon.)
        matches!(
            self.capability,
            HostCapability::Filesystem
                | HostCapability::Math
                | HostCapability::ObjectiveC
                | HostCapability::CoreGraphics
        ) || matches!(
            (self.capability, self.operation),
            (
                HostCapability::Clock,
                HostOperation::TickCount
                    | HostOperation::MonotonicTicks
                    | HostOperation::MonotonicTicksPerSecond
                    | HostOperation::WallClockRaw
                    | HostOperation::WallClockUnitsPerSecond
                    | HostOperation::WallClockEpochOffsetSeconds
            )
        )
    }

    /// Whether this semantic operation may be supplied by a per-target
    /// constant-result row. Emission treats it as a no-call operation only
    /// when the selected target has no external binding for the key: Windows
    /// therefore keeps QPF as a call, while Linux/Darwin use 10^9. The
    /// lowering row remains the source of the concrete constant.
    pub fn lowers_to_constant_result(self) -> bool {
        matches!(
            (self.capability, self.operation),
            (
                HostCapability::Clock,
                HostOperation::MonotonicTicksPerSecond
                    | HostOperation::WallClockUnitsPerSecond
                    | HostOperation::WallClockEpochOffsetSeconds
            )
        )
    }

    /// Whether this op's callee returns a POINTER whose pointee is the real
    /// result: after the `BL`, the lowering derefs the return register once
    /// (`ldr w0,[x0]`) before storing. Only `Filesystem::read_errno` (darwin
    /// `___error()` returns `int*`, i.e. `&errno`) needs this. The extra load
    /// shifts the result-store's position by 4 bytes, so the width function
    /// (`host_call_sequence_width`) and the result-operand data-address
    /// relocation offset (`data_addresses.rs`) both add 4 when this is set. The
    /// `BL` relocation is unaffected (it precedes the load; `read_errno` has no
    /// args). MUST stay in lockstep across those three sites + the encoder.
    pub fn dereferences_result(self) -> bool {
        matches!(self.capability, HostCapability::Filesystem)
            && matches!(self.operation, HostOperation::ReadErrno)
    }

    /// Whether the concrete host boundary returns a native status/count that
    /// the compiler-owned adapter intentionally discards. The retained plan
    /// still describes and validates that native result, but lowering carries
    /// no leading Omega result place for it.
    pub fn discards_native_result(self) -> bool {
        matches!(
            (self.capability, self.operation),
            (HostCapability::Stdin, HostOperation::Read)
                | (
                    HostCapability::Stdout | HostCapability::Stderr,
                    HostOperation::Write
                )
                | (HostCapability::Clock, HostOperation::SleepPoll)
        )
    }

    /// Whether this op passes its LAST argument (a `mode`) on the STACK rather than
    /// a register: darwin `open(path, flags, ...)` reads the create `mode` via
    /// `va_arg`, and Apple arm64 places variadic args on the stack (`[sp,#0]`).
    /// Only `Filesystem::open_create` needs this. Its aarch64 lowering brackets the
    /// call with `sub sp,sp,#16` … `str w<mode>,[sp]` … `bl` … `add sp,sp,#16`, so
    /// (relative to counting the mode as a register immediate) the sequence grows
    /// by 12 bytes total (sub+str+add) — added in lockstep at the width function
    /// (`host_call_sequence_width`) and the result-store data-address relocation
    /// (`data_addresses.rs`, +12) — and the `BL` sits 8 bytes later (sub+str, the
    /// add is AFTER it) so the external-call relocation adds 8. The `mode` must be
    /// an immediate (no relocation of its own). MUST stay in lockstep across those
    /// three sites + the encoder.
    /// Whether a Linux syscall binding returns a semantic nanosecond clock
    /// value through a caller-owned `timespec` rather than directly in the
    /// syscall result register. The composite lowering owns the temporary,
    /// traps on the impossible fixed-input syscall failure, and combines
    /// `tv_sec * 1_000_000_000 + tv_nsec` before storing the Omega result.
    pub fn uses_linux_timespec_result(self) -> bool {
        matches!(self.capability, HostCapability::Clock)
            && matches!(
                self.operation,
                HostOperation::MonotonicTicks | HostOperation::WallClockRaw
            )
    }

    /// Whether a Linux syscall binding consumes the semantic millisecond
    /// argument through a compiler-owned `timespec`. The external signature is
    /// `nanosleep(timespec*, timespec*) -> status`; the second pointer is null.
    pub fn uses_linux_timespec_argument(self) -> bool {
        matches!(
            (self.capability, self.operation),
            (HostCapability::Clock, HostOperation::Sleep)
        )
    }

    /// Whether this op's callee returns its result in the FLOAT return register
    /// (`d0`/`s0`) rather than `x0`: libm `sqrt`/`hypot` and, later, Core Graphics
    /// `double` getters. After the `BL` the lowering moves the bits back with
    /// `fmov x0, d0` before the normal integer result-store, so — exactly like
    /// `dereferences_result` — the width function (`host_call_sequence_width`) and
    /// the result-operand data-address relocation offset (`data_addresses.rs`) both
    /// add 4 when this is set. Float args precede the `BL`, so the external-call
    /// relocation is unaffected. MUST stay in lockstep across those three sites +
    /// the encoder. (`round_nearest` returns an `i64`, NOT a float, so it is
    /// excluded.)
    pub fn returns_float(self) -> bool {
        (matches!(self.capability, HostCapability::Math)
            && matches!(
                self.operation,
                HostOperation::SquareRoot
                    | HostOperation::Hypotenuse
                    | HostOperation::FusedMultiplyAdd
            ))
            || (matches!(self.capability, HostCapability::CoreGraphics)
                && matches!(
                    self.operation,
                    HostOperation::RectMaxX | HostOperation::RectMaxY
                ))
    }
}

/// The process-wide interner for CUSTOM operation names (M2 blocker 1:
/// the closed catalog generalized). A name outside the built-in catalog
/// interns to a stable index for the life of the process, so the key stays
/// `Copy` and binding/call sites agree by construction. Leaked once per
/// DISTINCT name (bounded by the program's external-binding surface).
fn interned_names() -> &'static std::sync::Mutex<Vec<&'static str>> {
    use std::sync::{Mutex, OnceLock};
    static NAMES: OnceLock<Mutex<Vec<&'static str>>> = OnceLock::new();
    NAMES.get_or_init(|| Mutex::new(Vec::new()))
}

fn intern_custom_name(name: &str) -> u32 {
    let mut names = interned_names()
        .lock()
        .expect("custom-name interner poisoned");
    if let Some(index) = names.iter().position(|existing| *existing == name) {
        return index as u32;
    }
    names.push(Box::leak(name.to_string().into_boxed_str()));
    (names.len() - 1) as u32
}

fn custom_name(index: u32) -> &'static str {
    interned_names()
        .lock()
        .expect("custom-name interner poisoned")
        .get(index as usize)
        .copied()
        .unwrap_or("<custom>")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostCapability {
    #[default]
    Unknown,
    Process,
    Stdin,
    Stdout,
    Stderr,
    Clock,
    Input,
    /// The windowed-renderer surface: device contexts, windows, framebuffer
    /// blits (user32/gdi32 imports on Windows).
    Gui,
    /// Filesystem access: open/read/write/close/unlink over file descriptors
    /// (libSystem imports on darwin, syscalls on linux).
    Filesystem,
    /// Floating-point math (libm/libSystem `lround`, `sqrt`, …). Its ops carry
    /// `f64` arguments/returns — the first host boundary to exercise the arm64
    /// float calling convention (v-register args), the foundation for calling
    /// Cocoa/Core Graphics `CGFloat`/`double` methods.
    Math,
    /// The Objective-C runtime (`objc_getClass`, `sel_registerName`,
    /// `objc_msgSend`) in `/usr/lib/libobjc.A.dylib` — the FIRST boundary that
    /// binds against a SECOND dylib (see `darwin_import_library` + the Mach-O
    /// multi-dylib load commands). The gateway to Cocoa/AppKit.
    ObjectiveC,
    /// CoreGraphics — the `CG*` C API. `CGRect`-taking geometry functions exercise
    /// the arm64 HFA calling convention (a `CGRect` = 4 doubles passed in v0–v3);
    /// `CGImageCreate`/`CGColorSpace…` build the blit surface.
    CoreGraphics,
    /// A string-interned source capability (an external binding whose trait
    /// name is outside the built-in catalog; M2 blocker 1). The index
    /// resolves through the process-wide interner, so the key stays `Copy`
    /// and binding/call sites agree by construction.
    Custom(u32),
}

impl HostCapability {
    pub fn from_name(name: &str) -> Self {
        match name {
            "Process" => Self::Process,
            "Stdin" => Self::Stdin,
            "Stdout" => Self::Stdout,
            "Stderr" => Self::Stderr,
            "Clock" => Self::Clock,
            "Input" => Self::Input,
            "Gui" => Self::Gui,
            "Filesystem" => Self::Filesystem,
            "Math" => Self::Math,
            "ObjectiveC" => Self::ObjectiveC,
            "CoreGraphics" => Self::CoreGraphics,
            // M2 blocker 1: authored names intern to stable Custom keys.
            _ => Self::Custom(intern_custom_name(name)),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Unknown => "<unknown>",
            Self::Process => "Process",
            Self::Stdin => "Stdin",
            Self::Stdout => "Stdout",
            Self::Stderr => "Stderr",
            Self::Clock => "Clock",
            Self::Input => "Input",
            Self::Gui => "Gui",
            Self::Filesystem => "Filesystem",
            Self::Math => "Math",
            Self::ObjectiveC => "ObjectiveC",
            Self::CoreGraphics => "CoreGraphics",
            Self::Custom(index) => custom_name(index),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostOperation {
    #[default]
    Unknown,
    /// A string-interned AUTHORED method name (M2 blocker 1); index into
    /// the process-wide interner.
    Custom(u32),
    Exit,
    ExitGroup,
    ExitProcess,
    GetStdHandle,
    Read,
    ReadFile,
    Write,
    WriteFile,
    /// `pread(fd, buf, count, offset)` -- read at an absolute file offset WITHOUT
    /// moving the descriptor's cursor (Rust `FileExt::read_at`). Same as `read`
    /// plus a trailing offset scalar: `[result, fd, buffer ptr, count, offset]`.
    PRead,
    /// `pwrite(fd, buf, count, offset)` -- write at an absolute file offset WITHOUT
    /// moving the cursor (Rust `FileExt::write_at`). Same as `write` plus a trailing
    /// offset scalar: `[result, fd, buffer ptr, length, offset]`.
    PWrite,
    /// `open`/`openat` -- open a path, returning a file descriptor (or -errno).
    Open,
    /// `creat(path, mode)` -- create/truncate a path for writing. Unlike `open`
    /// its `mode` is a NAMED (non-variadic) parameter, so it marshals in a
    /// register on arm64 (open's variadic mode would go on the stack).
    Creat,
    /// `close(fd)` -- release a file descriptor.
    Close,
    /// `CreateFileA(path, access, share, security, disposition, flags,
    /// template)` -- open a Win32 path as a kernel HANDLE. Windows-only.
    CreateFile,
    /// `CloseHandle(handle)` -- release a Win32 kernel HANDLE. Windows-only.
    CloseHandle,
    /// `unlink`/`remove` -- delete a path.
    Unlink,
    /// `lseek(fd, offset, whence)` -- reposition the descriptor, returning the
    /// resulting absolute offset (so `seek(fd, 0, SEEK_END)` yields the size).
    Seek,
    /// `mkdir(path, mode)` -- create a directory (Rust `create_dir`).
    MakeDir,
    /// `rmdir(path)` -- remove an empty directory (Rust `remove_dir`).
    RemoveDir,
    /// `openat(dirfd, name, flags)` -- open `name` RELATIVE to the directory
    /// `dirfd` (Rust `openat`, the `*at` family used by `remove_dir_all`).
    /// `[result, dirfd, name POINTER (NUL-terminated), flags]`.
    OpenAt,
    /// `unlinkat(dirfd, name, flags)` -- remove `name` RELATIVE to `dirfd`
    /// (`flags & AT_REMOVEDIR` = rmdir, else unlink). Same operand shape.
    UnlinkAt,
    /// `chmod(path, mode)` -- set a path's permission bits (Rust
    /// `set_permissions`). Same shape as `mkdir`: a path pointer + a NAMED
    /// (register) mode scalar.
    Chmod,
    /// `fchmod(fd, mode)` -- set an OPEN file's permission bits (Rust
    /// `File::set_permissions`). Same shape as `set_len`: two scalars (fd, mode).
    Fchmod,
    /// `rename(from, to)` -- move/rename a path (Rust `rename`). Two path args.
    Rename,
    /// `link(original, link)` -- create a hard link (Rust `hard_link`): a second
    /// directory entry for the same inode. Two path args, same shape as `rename`.
    Link,
    /// `symlink(target, linkpath)` -- create a symbolic link (Rust
    /// `os::unix::fs::symlink`). Two path args, same shape as `rename`/`link`.
    Symlink,
    /// `readlink(path, buf, bufsize)` -- read a symlink's target into a caller
    /// buffer (Rust `fs::read_link`), returning the byte count. Same shape as
    /// `read` but with a PATH pointer instead of an fd.
    ReadLink,
    /// `__getdirentries64(fd, buf, bufsize, &position)` -- read directory entries
    /// (darwin arm64's dir-read primitive; classic `getdirentries` is unavailable
    /// with 64-bit inodes) into a caller buffer as packed `dirent` records
    /// (d_reclen@16, d_namlen@18, d_type@20, d_name@21), returning the byte count
    /// (0 at end). Underpins Rust `fs::read_dir`. Four args incl. an in/out
    /// position pointer.
    ReadDir,
    /// `FindFirstFileA(pattern, &find_data)` -- open a windows directory
    /// enumeration for `pattern` (typically `dir\*`) and fill the FIRST entry's
    /// WIN32_FIND_DATAA record (attributes u32 @0, directory bit 0x10;
    /// NUL-terminated cFileName @44; record 320 bytes). Returns the find HANDLE
    /// as i64, or -1 (INVALID_HANDLE_VALUE). The windows half of the fs
    /// portable-contract dir-walk (rung 3a); posix targets never lower it.
    FindFirst,
    /// `FindNextFileA(handle, &find_data)` -- fill the NEXT entry of an open
    /// enumeration (BOOL: 1 = filled, 0 = end).
    FindNext,
    /// `FindClose(handle)` -- release a find handle (BOOL).
    FindClose,
    /// `CreateHardLinkA(link, existing, security_attributes)` -- the windows
    /// hard-link primitive (session slice 3). Arg order is (NEW link,
    /// existing) -- reversed from posix `link(existing, new)` -- with a
    /// trailing security-attributes pointer the API requires as NULL (the
    /// designed op passes 0). Returns BOOL (non-zero success); failure
    /// reasons live in GetLastError, not msvcrt errno. Posix targets never
    /// lower it (they bind `Link`).
    CreateHardLink,
    /// `_get_osfhandle(fd)` -- the msvcrt fd -> Win32 HANDLE bridge (session
    /// slice 4a): the OS HANDLE behind an open CRT descriptor (i64; -2 for a
    /// bad fd). Unlocks the HANDLE-keyed kernel32 surface. Windows-only.
    GetOsfHandle,
    /// `GetFinalPathNameByHandleA(handle, buffer, capacity, flags)` --
    /// resolve an open handle to its final DOS path (windows canonicalize).
    /// Returns the length written (no NUL), the required capacity (with NUL)
    /// when too small, or 0 on failure. Windows-only.
    FinalPathNameByHandle,
    /// `SetFileTime(handle, creation, last_access, last_write)` -- stamp an
    /// open handle's times from FILETIME buffers (the windows set_times leg;
    /// session slice 4b). `creation` rides as a NULL-able scalar (0 = leave
    /// alone); BOOL result. Windows-only.
    SetFileTime,
    /// `LockFileEx(handle, flags, reserved, length_low, length_high,
    /// overlapped)` -- acquire a Win32 byte-range lock. The std wrapper uses
    /// offset zero and the full u64 length to provide whole-file locking.
    LockFileEx,
    /// `UnlockFile(handle, offset_low, offset_high, length_low, length_high)` --
    /// release a Win32 byte-range lock. Windows-only.
    UnlockFile,
    /// `GetLastError()` -- read the calling thread's Win32 last-error value.
    /// Unlike `ReadErrno`, this returns the value directly (no dereference).
    GetLastError,
    /// `stat(path, buf)` -- fill a `struct stat` buffer for a PATH (Rust
    /// `fs::metadata`). A path pointer + a buffer pointer (the kernel writes the
    /// 144-byte darwin stat record through it); the Omega layer reads `st_size`
    /// (off 96, i64) and `st_mode` (off 4, u16) back out.
    Stat,
    /// `fstat(fd, buf)` -- fill a `struct stat` buffer for an OPEN fd (Rust
    /// `File::metadata`). Same buffer as `stat`, but keyed by descriptor instead of
    /// path: operand shape `[result, fd scalar, buffer pointer]` (like `read`
    /// without the count). Never touches the file offset.
    FStat,
    /// `lstat(path, buf)` -- like `stat`, but does NOT follow a final symlink (Rust
    /// `fs::symlink_metadata`). On a symlink the `st_mode` file-type field is
    /// `S_IFLNK` (0o120000), so `Metadata::is_symlink()` is true. Identical operand
    /// shape to `Stat` (path pointer + buffer pointer).
    LStat,
    /// `realpath(path, resolved)` -- resolve a path to its canonical absolute form,
    /// following every symlink, into a caller buffer (>= PATH_MAX). Rust
    /// `fs::canonicalize`. Returns the `resolved` pointer (non-NULL) on success or
    /// NULL on error, so the stored i64 is just a success flag (no deref). Same
    /// operand shape as `Stat` (path pointer + buffer pointer).
    Realpath,
    /// `ftruncate(fd, length)` -- set a file's length (Rust `File::set_len`).
    SetLen,
    /// `futimens(fd, times[2])` -- set a file's access + modification timestamps
    /// (Rust `File::set_times`). `times` is a caller buffer holding two
    /// `struct timespec` (atime then mtime, each {tv_sec i64, tv_nsec i64}); the
    /// operand shape is the SAME as `fstat` (`[result, fd, buffer pointer]`).
    SetFileTimes,
    /// `fsync(fd)` -- flush a file's buffered data + metadata to the storage
    /// device (Rust `File::sync_all`). Same shape as `close`: one fd arg, rc.
    Sync,
    /// `dup(fd)` -- duplicate a file descriptor onto the lowest free fd, sharing
    /// the underlying open file description (Rust `File::try_clone`). Same shape as
    /// `close`: one fd arg, returns the NEW fd (or -1 on error).
    Dup,
    /// `flock(fd, operation)` -- apply or remove an advisory lock on an open file
    /// (Rust `File::lock`/`lock_shared`/`try_lock`/`unlock`). `operation` is a
    /// bitmask: LOCK_SH=1, LOCK_EX=2, LOCK_NB=4, LOCK_UN=8. Same operand shape as
    /// `ftruncate` (`[result, fd, operation]` -- fd + one scalar), returns 0 / -1.
    Flock,
    /// `chown(path, uid, gid)` -- change a file's owner/group, following symlinks
    /// (Rust `os::unix::fs::chown`). uid/gid of -1 leaves that component
    /// unchanged. Operand shape `[result, path pointer, uid, gid]` (path + two
    /// scalars). Returns 0 / -1 (EPERM if the caller may not change ownership).
    Chown,
    /// `lchown(path, uid, gid)` -- like `chown` but does NOT follow a final
    /// symlink (Rust `os::unix::fs::lchown`). Same operand shape as `chown`.
    LChown,
    /// `fchown(fd, uid, gid)` -- change owner/group by open descriptor (Rust
    /// `os::unix::fs::fchown`). Same operand shape as `lseek` (`[result, fd, uid,
    /// gid]` -- fd + two scalars).
    Fchown,
    /// `open(path, flags, mode)` with O_CREAT -- the CREATING open (Rust
    /// `File::create_new`, `OpenOptions.create`/`.create_new`). Unlike `creat`
    /// (fixed O_WRONLY|O_CREAT|O_TRUNC, register mode) this passes ARBITRARY flags
    /// (O_CREAT|O_EXCL|O_RDWR|...) plus a `mode`. `mode` is VARIADIC on darwin, so
    /// the retained Apple AAPCS64 plan marshals it on the outgoing stack rather
    /// than in the next fixed-parameter register.
    OpenCreate,
    /// `___error()` -- darwin's thread-local errno accessor; returns `int*`.
    /// Takes NO args and its result is DEREFERENCED once (see
    /// `HostOperationKey::dereferences_result`) so the stored value is `errno`
    /// itself (the numeric failure kind), not the pointer.
    ReadErrno,
    /// `Math::round_nearest(x: f64) -> i64` → libm `lround`. The first op with an
    /// `f64` ARGUMENT (passed in v0), proving the arm64 float calling convention.
    RoundNearest,
    /// `Math::square_root(x: f64) -> f64` → libm `sqrt`. The first op with an `f64`
    /// RESULT (returned in d0), proving the arm64 float RETURN convention (see
    /// `HostOperationKey::returns_float`).
    SquareRoot,
    /// `Math::hypotenuse(x: f64, y: f64) -> f64` → libm `hypot`. First op with TWO
    /// `f64` arguments (v0, v1) AND a float return — proves multi-float-arg
    /// register sequencing alongside the float return.
    Hypotenuse,
    /// `Math::fused_multiply_add(x: f64, y: f64, z: f64) -> f64` → libm `fma`
    /// (`x*y + z`). THREE `f64` args (v0, v1, v2) — proves the float-arg sequence
    /// extends past v1 to v2, i.e. that a homogeneous-float aggregate (`NSRect` = 4
    /// doubles → v0–v3) marshals correctly, since an HFA and N separate double args
    /// occupy the SAME consecutive v-registers on AArch64 AAPCS.
    FusedMultiplyAdd,
    /// `ObjectiveC::get_class(name: &[u8] in Path) -> u64` → libobjc `objc_getClass`.
    /// Takes a NUL-terminated C string (the class name, materialized like an fs
    /// path) and returns the `Class` pointer in x0. The first op binding a SECOND
    /// dylib (libobjc) — proves multi-dylib linking.
    GetClass,
    /// `ObjectiveC::register_selector(name) -> u64` → libobjc `sel_registerName`.
    /// A NUL-terminated selector name → the interned `SEL` pointer (x0). Same
    /// operand shape as `get_class`.
    RegisterSelector,
    /// `ObjectiveC::send(recv: u64, sel: u64) -> u64` → libobjc `objc_msgSend`.
    /// The zero-argument message send: `recv`→x0, `sel`→x1, result `id`/scalar in
    /// x0. The Objective-C workhorse (`[recv sel]`).
    MsgSend,
    /// `ObjectiveC::send_scalar(recv, sel, arg: u64) -> u64` → `objc_msgSend` with
    /// one integer/pointer argument (`recv`→x0, `sel`→x1, `arg`→x2). For selectors
    /// taking a scalar: `respondsToSelector:` (SEL), `setActivationPolicy:` (int),
    /// `activateIgnoringOtherApps:` (BOOL). Shares `_objc_msgSend` with `send`.
    MsgSendScalar,
    /// `ObjectiveC::send_string(recv, sel, text) -> u64` → `objc_msgSend` with one
    /// C-string argument (`recv`→x0, `sel`→x1, `char*`→x2). For selectors like
    /// `initWithUTF8String:` (window titles etc.). Shares `_objc_msgSend`. Only
    /// provable once Foundation is loaded (NSString registered).
    MsgSendString,
    /// `ObjectiveC::send_rect(recv, sel, x, y, w, h, a, b, c) -> u64` →
    /// `objc_msgSend` with an `NSRect`/`CGRect` (4 doubles) HFA argument plus three
    /// trailing integer/pointer args: `recv`→x0, `sel`→x1, the rect→v0–v3, then
    /// `a`→x2, `b`→x3, `c`→x4. The MIXED HFA-plus-scalar call the window's
    /// `initWithContentRect:styleMask:backing:defer:` needs (rect + styleMask +
    /// backing + defer). Shares the `_objc_msgSend` symbol.
    MsgSendRect,
    /// `ObjectiveC::send_scalar4(recv, sel, a, b, c, d) -> u64` → `objc_msgSend` with
    /// FOUR integer/pointer args: `recv`→x0, `sel`→x1, then `a`→x2, `b`→x3, `c`→x4,
    /// `d`→x5. For the event pump's `nextEventMatchingMask:untilDate:inMode:dequeue:`
    /// (mask, NSDate*, mode NSString*, BOOL). Shares the `_objc_msgSend` symbol.
    MsgSendScalar4,
    /// `ObjectiveC::send_image_size(recv, sel, image, w, h) -> u64` → `objc_msgSend`
    /// with a pointer arg plus an `NSSize` (2 doubles): `recv`→x0, `sel`→x1,
    /// `image`→x2, then the size in v0,v1. For `NSImage initWithCGImage:size:` —
    /// the mixed scalar-plus-2-float call that wraps a `CGImage` for display.
    /// Shares the `_objc_msgSend` symbol.
    MsgSendImageSize,
    /// `ObjectiveC::send_byte_string(recv, sel, text: &[u8]) -> u64` → `objc_msgSend`
    /// with one RUNTIME byte-buffer pointer argument (`recv`→x0, `sel`→x1, ptr→x2).
    /// For `initWithUTF8String:` over runtime bytes (the samples' window titles),
    /// which `send_string`'s compile-time CString literal argument cannot express.
    /// The callee reads to the first NUL, so the buffer must be NUL-terminated by
    /// construction; tighten to a proven CString domain once the reference-domain
    /// mint lands. Shares the `_objc_msgSend` symbol.
    MsgSendByteString,
    /// `ObjectiveC::pool_push() -> u64` → libobjc `objc_autoreleasePoolPush`: opens
    /// an autorelease-pool scope and returns its token. No args. The gui event pump
    /// runs outside any Cocoa-managed pool, so dequeued autoreleased NSEvents would
    /// otherwise leak every frame.
    PoolPush,
    /// `ObjectiveC::pool_pop(pool: u64) -> u64` → libobjc `objc_autoreleasePoolPop`:
    /// closes the pool scope opened by `pool_push`, draining every object
    /// autoreleased inside it (a void C call; the result register is scratch).
    PoolPop,
    /// `CoreGraphics::rect_max_x(x, y, w, h) -> f64` → `CGRectGetMaxX`. Takes a
    /// `CGRect` = 4 doubles passed as an HFA in v0–v3, returns `origin.x +
    /// size.width` (v0 + v2) as a `CGFloat` in d0. The run-verified proof that 4
    /// doubles land in v0–v3 (`RectMaxY` returns v1 + v3).
    RectMaxX,
    /// `CoreGraphics::rect_max_y(x, y, w, h) -> f64` → `CGRectGetMaxY` = `origin.y +
    /// size.height` (v1 + v3).
    RectMaxY,
    /// `CoreGraphics::color_space_rgb() -> u64` → `CGColorSpaceCreateDeviceRGB`. NO
    /// args; returns a `CGColorSpaceRef` in x0. The colorspace for the blit's
    /// bitmap context.
    ColorSpaceRgb,
    /// `CoreGraphics::bitmap_context(data, w, h, bpc, stride, space, info) -> u64` →
    /// `CGBitmapContextCreate`. SEVEN integer/pointer args (x0–x6): `data` is a
    /// pointer to the framebuffer, then width/height/bitsPerComponent/bytesPerRow
    /// (ints), the colorspace (ptr), and the `CGBitmapInfo` (int). Returns a
    /// `CGContextRef` backed by the framebuffer. Chosen over `CGImageCreate` (11
    /// args, 3 on the stack) because all 7 fit in registers.
    BitmapContext,
    /// `CoreGraphics::bitmap_context_image(ctx) -> u64` →
    /// `CGBitmapContextCreateImage`: snapshots the bitmap context into a
    /// `CGImageRef`. One ptr arg.
    BitmapContextImage,
    /// `CoreGraphics::image_width(img) -> i64` → `CGImageGetWidth`: the width of a
    /// `CGImageRef` (a `size_t` in x0). Used to run-verify the blit path.
    ImageWidth,
    /// `CoreGraphics::context_release(ctx) -> u64` → `CGContextRelease`: drops the
    /// blit's per-frame bitmap context (Create-rule ownership from
    /// `CGBitmapContextCreate`). One ptr arg; a void C call — the result register
    /// is scratch. Without it every presented frame leaks a CGContext.
    ContextRelease,
    /// `CoreGraphics::image_release(img) -> u64` → `CGImageRelease`: drops the
    /// per-frame CGImage snapshot (Create-rule ownership from
    /// `CGBitmapContextCreateImage`). Same shape as `context_release`.
    ImageRelease,
    /// `CoreGraphics::event_source_key_state(state_id, keycode) -> u64` →
    /// `CGEventSourceKeyState(CGEventSourceStateID, CGKeyCode)`: is a physical key
    /// currently down? Two scalar args (state_id → x0, keycode → x1), returns a BOOL
    /// (0/1) in x0. The macOS backing for the samples' `Input.key_state` (the
    /// `MacosInput` provider maps a Win32 virtual-key to the macOS keycode). In the
    /// CoreGraphics framework (`_CG*` → CoreGraphics dylib).
    EventSourceKeyState,
    Sleep,
    /// `Clock::sleep(milliseconds)` on darwin → `poll(NULL, 0, milliseconds)`:
    /// with zero fds `poll` is a portable millisecond sleep whose timeout is ALREADY
    /// in milliseconds (unlike `usleep`, which is microseconds and rejects ≥1s), so
    /// no unit scaling is needed. A DISTINCT op (not `Sleep`) so its operand arm can
    /// place `[NULL, 0, ms]` in x0/x1/x2 without a per-target branch in the shared
    /// `Sleep` arm (which marshals a single arg into x0 for Win32 `Sleep(ms)`).
    SleepPoll,
    TickCount,
    /// std::time monotonic source (TimeHost seam, TASKS_TIME.md rung 5).
    /// windows: `QueryPerformanceCounter(&ticks)` -- an OUT-PARAM u64; darwin:
    /// `clock_gettime_nsec_np(CLOCK_UPTIME_RAW)` returns the u64 directly.
    MonotonicTicks,
    /// Ticks-per-second calibration for `MonotonicTicks`. windows:
    /// `QueryPerformanceFrequency(&freq)` (out-param); darwin: constant 1e9.
    MonotonicTicksPerSecond,
    /// Wall clock as ONE raw u64 read. windows:
    /// `GetSystemTimePreciseAsFileTime(&ft)` (out-param FILETIME); darwin:
    /// `clock_gettime_nsec_np(CLOCK_REALTIME)`.
    WallClockRaw,
    /// Units-per-second for `WallClockRaw`. Windows uses 1e7.
    WallClockUnitsPerSecond,
    /// Platform-epoch -> Unix-epoch shift in seconds -- also a per-target
    /// `ConstantResult`. windows 11_644_473_600 (1601 -> 1970); darwin 0.
    WallClockEpochOffsetSeconds,
    KeyState,
    /// `CreateCompatibleDC(0)` -- a memory device context (the CI-safe,
    /// differential-testable blit target).
    DcCreate,
    /// `GetDC(hwnd)` -- a window's device context.
    GetDc,
    /// `CreateWindowExA` through the built-in `"STATIC"` window class (no
    /// WNDCLASS registration, no WndProc, no message pump for a short-lived
    /// window).
    WindowCreate,
    /// `StretchDIBits` -- blit a top-down 32bpp DIB framebuffer into a device
    /// context.
    Blit,
    /// `PeekMessageW(&msg, 0, 0, 0, PM_REMOVE)` -- poll one queued message into
    /// a caller-owned MSG buffer; 0 when the queue is empty.
    MsgPeek,
    /// `TranslateMessage(&msg)` -- produce character messages from key messages.
    MsgTranslate,
    /// `DispatchMessageW(&msg)` -- route the message to the window procedure
    /// (DefWindowProc via the built-in "STATIC" class), which is what makes a
    /// window draggable, hoverable, and closable.
    MsgDispatch,
    /// `IsWindow(hwnd)` -- liveness: 0 once the user (or the app) destroyed it.
    IsWindow,
    /// `DestroyWindow(hwnd)`.
    WindowDestroy,
    /// `GetForegroundWindow()` -- the focused top-level window (0 when none).
    /// Lets a pump scope global key state (GetAsyncKeyState) to its own window.
    ForegroundWindow,
}

impl HostOperation {
    pub fn from_name(name: &str) -> Self {
        match name {
            "exit" => Self::Exit,
            "exit_group" => Self::ExitGroup,
            "exit_process" => Self::ExitProcess,
            "get_std_handle" => Self::GetStdHandle,
            "read" => Self::Read,
            "read_file" => Self::ReadFile,
            "write" => Self::Write,
            "write_file" => Self::WriteFile,
            "pread" => Self::PRead,
            "pwrite" => Self::PWrite,
            "open" => Self::Open,
            "open_path_handle" => Self::CreateFile,
            "creat" => Self::Creat,
            "close" => Self::Close,
            "close_handle" => Self::CloseHandle,
            "unlink" => Self::Unlink,
            "lseek" => Self::Seek,
            "mkdir" => Self::MakeDir,
            "rmdir" => Self::RemoveDir,
            "openat" => Self::OpenAt,
            "unlinkat" => Self::UnlinkAt,
            "chmod" => Self::Chmod,
            "fchmod" => Self::Fchmod,
            "rename" => Self::Rename,
            "link" => Self::Link,
            "symlink" => Self::Symlink,
            "readlink" => Self::ReadLink,
            "getdirentries64" => Self::ReadDir,
            "find_first" => Self::FindFirst,
            "find_next" => Self::FindNext,
            "find_close" => Self::FindClose,
            "create_hard_link" => Self::CreateHardLink,
            "get_osfhandle" => Self::GetOsfHandle,
            "final_path_name_by_handle" => Self::FinalPathNameByHandle,
            "set_file_time" => Self::SetFileTime,
            "lock_file_ex" => Self::LockFileEx,
            "unlock_file" => Self::UnlockFile,
            "get_last_error" => Self::GetLastError,
            "stat" => Self::Stat,
            "fstat" => Self::FStat,
            "lstat" => Self::LStat,
            "realpath" => Self::Realpath,
            "ftruncate" => Self::SetLen,
            "futimens" => Self::SetFileTimes,
            "fsync" => Self::Sync,
            "dup" => Self::Dup,
            "flock" => Self::Flock,
            "chown" => Self::Chown,
            "lchown" => Self::LChown,
            "fchown" => Self::Fchown,
            "open_create" => Self::OpenCreate,
            "read_errno" => Self::ReadErrno,
            "round_nearest" => Self::RoundNearest,
            "square_root" => Self::SquareRoot,
            "hypotenuse" => Self::Hypotenuse,
            "fused_multiply_add" => Self::FusedMultiplyAdd,
            "get_class" => Self::GetClass,
            "register_selector" => Self::RegisterSelector,
            "send" => Self::MsgSend,
            "send_scalar" => Self::MsgSendScalar,
            "send_string" => Self::MsgSendString,
            "send_rect" => Self::MsgSendRect,
            "send_scalar4" => Self::MsgSendScalar4,
            "send_image_size" => Self::MsgSendImageSize,
            "send_byte_string" => Self::MsgSendByteString,
            "pool_push" => Self::PoolPush,
            "pool_pop" => Self::PoolPop,
            "rect_max_x" => Self::RectMaxX,
            "rect_max_y" => Self::RectMaxY,
            "color_space_rgb" => Self::ColorSpaceRgb,
            "bitmap_context" => Self::BitmapContext,
            "bitmap_context_image" => Self::BitmapContextImage,
            "image_width" => Self::ImageWidth,
            "context_release" => Self::ContextRelease,
            "image_release" => Self::ImageRelease,
            "event_source_key_state" => Self::EventSourceKeyState,
            "sleep" => Self::Sleep,
            "sleep_poll" => Self::SleepPoll,
            "tick_count" => Self::TickCount,
            "monotonic_ticks" => Self::MonotonicTicks,
            "monotonic_ticks_per_second" => Self::MonotonicTicksPerSecond,
            "wall_clock_raw" => Self::WallClockRaw,
            "wall_clock_units_per_second" => Self::WallClockUnitsPerSecond,
            "wall_clock_epoch_offset_seconds" => Self::WallClockEpochOffsetSeconds,
            "key_state" => Self::KeyState,
            "dc_create" => Self::DcCreate,
            "get_dc" => Self::GetDc,
            "window_create" => Self::WindowCreate,
            "blit" => Self::Blit,
            "msg_peek" => Self::MsgPeek,
            "msg_translate" => Self::MsgTranslate,
            "msg_dispatch" => Self::MsgDispatch,
            "is_window" => Self::IsWindow,
            "window_destroy" => Self::WindowDestroy,
            "foreground_window" => Self::ForegroundWindow,
            // M2 blocker 1: authored names intern to stable Custom keys.
            _ => Self::Custom(intern_custom_name(name)),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Unknown => "<unknown>",
            Self::Custom(index) => custom_name(index),
            Self::Exit => "exit",
            Self::ExitGroup => "exit_group",
            Self::ExitProcess => "exit_process",
            Self::GetStdHandle => "get_std_handle",
            Self::Read => "read",
            Self::ReadFile => "read_file",
            Self::Write => "write",
            Self::WriteFile => "write_file",
            Self::PRead => "pread",
            Self::PWrite => "pwrite",
            Self::Open => "open",
            Self::CreateFile => "open_path_handle",
            Self::Creat => "creat",
            Self::Close => "close",
            Self::CloseHandle => "close_handle",
            Self::Unlink => "unlink",
            Self::Seek => "lseek",
            Self::MakeDir => "mkdir",
            Self::RemoveDir => "rmdir",
            Self::OpenAt => "openat",
            Self::UnlinkAt => "unlinkat",
            Self::Chmod => "chmod",
            Self::Fchmod => "fchmod",
            Self::Rename => "rename",
            Self::Link => "link",
            Self::Symlink => "symlink",
            Self::ReadLink => "readlink",
            Self::ReadDir => "getdirentries64",
            Self::FindFirst => "find_first",
            Self::FindNext => "find_next",
            Self::FindClose => "find_close",
            Self::CreateHardLink => "create_hard_link",
            Self::GetOsfHandle => "get_osfhandle",
            Self::FinalPathNameByHandle => "final_path_name_by_handle",
            Self::SetFileTime => "set_file_time",
            Self::LockFileEx => "lock_file_ex",
            Self::UnlockFile => "unlock_file",
            Self::GetLastError => "get_last_error",
            Self::Stat => "stat",
            Self::FStat => "fstat",
            Self::LStat => "lstat",
            Self::Realpath => "realpath",
            Self::SetLen => "ftruncate",
            Self::SetFileTimes => "futimens",
            Self::Sync => "fsync",
            Self::Dup => "dup",
            Self::Flock => "flock",
            Self::Chown => "chown",
            Self::LChown => "lchown",
            Self::Fchown => "fchown",
            Self::OpenCreate => "open_create",
            Self::ReadErrno => "read_errno",
            Self::RoundNearest => "round_nearest",
            Self::SquareRoot => "square_root",
            Self::Hypotenuse => "hypotenuse",
            Self::FusedMultiplyAdd => "fused_multiply_add",
            Self::GetClass => "get_class",
            Self::RegisterSelector => "register_selector",
            Self::MsgSend => "send",
            Self::MsgSendScalar => "send_scalar",
            Self::MsgSendString => "send_string",
            Self::MsgSendRect => "send_rect",
            Self::MsgSendScalar4 => "send_scalar4",
            Self::MsgSendImageSize => "send_image_size",
            Self::MsgSendByteString => "send_byte_string",
            Self::PoolPush => "pool_push",
            Self::PoolPop => "pool_pop",
            Self::RectMaxX => "rect_max_x",
            Self::RectMaxY => "rect_max_y",
            Self::ColorSpaceRgb => "color_space_rgb",
            Self::BitmapContext => "bitmap_context",
            Self::BitmapContextImage => "bitmap_context_image",
            Self::ImageWidth => "image_width",
            Self::ContextRelease => "context_release",
            Self::ImageRelease => "image_release",
            Self::EventSourceKeyState => "event_source_key_state",
            Self::Sleep => "sleep",
            Self::SleepPoll => "sleep_poll",
            Self::TickCount => "tick_count",
            Self::MonotonicTicks => "monotonic_ticks",
            Self::MonotonicTicksPerSecond => "monotonic_ticks_per_second",
            Self::WallClockRaw => "wall_clock_raw",
            Self::WallClockUnitsPerSecond => "wall_clock_units_per_second",
            Self::WallClockEpochOffsetSeconds => "wall_clock_epoch_offset_seconds",
            Self::KeyState => "key_state",
            Self::DcCreate => "dc_create",
            Self::GetDc => "get_dc",
            Self::WindowCreate => "window_create",
            Self::Blit => "blit",
            Self::MsgPeek => "msg_peek",
            Self::MsgTranslate => "msg_translate",
            Self::MsgDispatch => "msg_dispatch",
            Self::IsWindow => "is_window",
            Self::WindowDestroy => "window_destroy",
            Self::ForegroundWindow => "foreground_window",
        }
    }
}

/// One selected bodyless external leaf retained for target realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalBindingRow {
    /// The leaf's target identifier, retained for exact target realization.
    pub target_name: String,
    pub trait_name: String,
    pub method: String,
    /// Exact canonical overload identity. The human method name remains only
    /// readable drift data, including for singleton requirements.
    pub requirement_identity: String,
    /// The attached provider data type that owns the table layout. Empty for
    /// free leaves and required for table-field bindings.
    pub table_type: String,
    /// Canonical source-selected plan for this concrete service method.
    pub boundary_entry_plan: Option<BoundaryEntryPlan>,
    pub binding: ExternalBindingKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalBindingKind {
    Syscall {
        number: i64,
    },
    Import {
        locator: NormalizedForeignLocator,
    },
    StringBackedImportBootstrap {
        module: String,
        symbol: String,
    },
    /// Select an existing compiler-known platform lowering. The retained
    /// string is the exact normalized realization-machine overload identity.
    CompilerIntrinsic {
        machine: String,
    },
    VtableSlot {
        index: i64,
    },
    /// Dispatch by a fn-ptr field of the row's `table_type`; the layout plan
    /// supplies its byte offset.
    VtableField {
        field: String,
    },
    /// Dispatch by fn-ptr FIELD like `VtableField`, but the table pointer is
    /// DISPATCH-ONLY -- never a wire argument (EFI table services take no
    /// This; protocol/COM methods do).
    TableFunction {
        field: String,
    },
}
