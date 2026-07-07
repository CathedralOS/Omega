mod darwin;
mod linux;
mod windows;
pub use windows::windows_import_library;

use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_target::{NativeTarget, ObjectFormat};
use std::sync::Arc;

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
        matches!(
            self.capability,
            HostCapability::Filesystem | HostCapability::Math
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
    pub fn passes_trailing_mode_on_stack(self) -> bool {
        matches!(self.capability, HostCapability::Filesystem)
            && matches!(self.operation, HostOperation::OpenCreate)
    }
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
            _ => Self::Unknown,
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostOperation {
    #[default]
    Unknown,
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
    /// the native lowering must marshal it on the STACK (see D8-open turnkey plan)
    /// -- native lowering is PENDING; the interpreter models it fully today.
    OpenCreate,
    /// `___error()` -- darwin's thread-local errno accessor; returns `int*`.
    /// Takes NO args and its result is DEREFERENCED once (see
    /// `HostOperationKey::dereferences_result`) so the stored value is `errno`
    /// itself (the numeric failure kind), not the pointer.
    ReadErrno,
    /// `Math::round_nearest(x: f64) -> i64` → libm `lround`. The first op with an
    /// `f64` ARGUMENT (passed in v0), proving the arm64 float calling convention.
    RoundNearest,
    Sleep,
    TickCount,
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
            "creat" => Self::Creat,
            "close" => Self::Close,
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
            "sleep" => Self::Sleep,
            "tick_count" => Self::TickCount,
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
            _ => Self::Unknown,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Unknown => "<unknown>",
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
            Self::Creat => "creat",
            Self::Close => "close",
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
            Self::Sleep => "sleep",
            Self::TickCount => "tick_count",
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostAbiPlan {
    pub target: NativeTarget,
    pub bindings: Arena<HostBinding>,
    pub host_operations: Arena<HostOperationReference>,
    pub platform_call_lowerings: Arena<PlatformCallLowering>,
    pub boundary_policies: Arena<HostBoundaryPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostBinding {
    pub operation_key: HostOperationKey,
    pub mechanism: HostBindingMechanism,
    pub boundary_policy: Arc<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HostBoundaryPolicy {
    pub path: Arc<str>,
    pub checked: bool,
}

impl Default for HostBinding {
    fn default() -> Self {
        Self {
            operation_key: HostOperationKey::default(),
            mechanism: HostBindingMechanism::Import {
                library: Arc::from(""),
                symbol: Arc::from(""),
            },
            boundary_policy: Arc::from(""),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostBindingMechanism {
    Import {
        library: Arc<str>,
        symbol: Arc<str>,
    },
    Syscall {
        name: Arc<str>,
        number: u32,
        number_register: u8,
        supervisor_call: u16,
    },
    /// COM/UEFI per-object dispatch (extern brief §12.1): the callee address
    /// is read from the RECEIVER at call time -- `mov rax, [this + index*8];
    /// call rax`. The protocol struct IS the vtable (UEFI SimpleTextOutput:
    /// OutputString at slot 1 = +8). No import thunk, no relocation.
    VtableSlot { index: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformCallLowering {
    pub platform: Arc<str>,
    pub state: Arc<str>,
    pub operations: HandleSpan<HostOperationReference>,
    pub data: PlatformCallData,
}

pub type PlatformCallLoweringHandle = Handle<PlatformCallLowering>;

impl Default for PlatformCallLowering {
    fn default() -> Self {
        Self {
            platform: Arc::from(""),
            state: Arc::from(""),
            operations: HandleSpan::empty(),
            data: PlatformCallData::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlatformCallData {
    #[default]
    None,
    FirstTextArgument {
        append_newline: bool,
    },
    MutableOutputBuffer {
        byte_capacity: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostOperationReference {
    pub key: HostOperationKey,
}

impl Default for HostOperationReference {
    fn default() -> Self {
        Self {
            key: HostOperationKey::default(),
        }
    }
}

pub fn build_host_abi_plan(target: NativeTarget) -> HostAbiPlan {
    let mut plan = HostAbiPlan {
        target,
        bindings: Arena::new(),
        host_operations: Arena::new(),
        platform_call_lowerings: Arena::new(),
        boundary_policies: Arena::new(),
    };

    match target.object_format {
        ObjectFormat::Coff => windows::populate(&mut plan),
        ObjectFormat::Elf => linux::populate(&mut plan),
        ObjectFormat::MachO => darwin::populate(&mut plan),
    }

    plan
}

/// The FREESTANDING (no-host) ABI plan: an EFI application trusts no host
/// boundary packages -- services arrive through the entry's parameters (the
/// UEFI SystemTable), never through host bindings or an import table
/// ("a target = the boundary packages it trusts; absence = denial", extern
/// brief §4). Zero bindings means zero import thunks, so the PE emitter's
/// empty-import-table path produces a clean import-free image; a boundary
/// call in such a program fails with the ordinary missing-lowering
/// diagnostic rather than silently binding to an OS that will not be there.
/// One parsed `provides` arm, threaded from the program source (the extern
/// brief's Binding sum): `<target> provides <Trait> { <method> -> <Binding> }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvidesRow {
    pub trait_name: String,
    pub method: String,
    pub binding: ProvidesBindingKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvidesBindingKind {
    Syscall { number: i64 },
    DllImport { module: String, symbol: String },
    VtableSlot { index: i64 },
}

/// The boundary-policy path provides-sourced bindings live under: the program
/// AUTHORED the binding, so the policy is its own declaration.
pub const PROVIDES_BOUNDARY_POLICY: &str = "omega::host::provides";

pub fn build_freestanding_abi_plan(
    target: NativeTarget,
    provides: &[ProvidesRow],
) -> Result<HostAbiPlan, String> {
    let mut plan = HostAbiPlan {
        target,
        bindings: Arena::new(),
        host_operations: Arena::new(),
        platform_call_lowerings: Arena::new(),
        boundary_policies: Arena::new(),
    };
    if provides.is_empty() {
        return Ok(plan);
    }
    plan.boundary_policies.insert(HostBoundaryPolicy {
        path: PROVIDES_BOUNDARY_POLICY.into(),
        checked: true,
    });

    // KNOWN DEBT: HostOperationKey is a CLOSED enum pair; provides names
    // outside the catalog map to (Unknown, Unknown). One such row is fine --
    // the key just has to be stable between the binding and the call site --
    // but TWO would collide silently, so collide loudly instead. The
    // generalization is string-interned operation keys.
    let mut seen_unknown: Option<(String, String)> = None;
    for row in provides {
        let key = HostOperationKey::from_names(&row.trait_name, &row.method);
        if key.capability_name() == "Unknown" {
            if let Some((prior_trait, prior_method)) = &seen_unknown
                && (prior_trait != &row.trait_name || prior_method != &row.method)
            {
                return Err(format!(
                    "provides rows `{}::{}` and `{}::{}` both fall outside the closed                      operation catalog and would collide; string-keyed operations are                      not built yet",
                    prior_trait, prior_method, row.trait_name, row.method
                ));
            }
            seen_unknown = Some((row.trait_name.clone(), row.method.clone()));
        }
        let mechanism = match &row.binding {
            ProvidesBindingKind::VtableSlot { index } => {
                HostBindingMechanism::VtableSlot { index: *index }
            }
            ProvidesBindingKind::DllImport { module, symbol } => HostBindingMechanism::Import {
                library: module.as_str().into(),
                symbol: symbol.as_str().into(),
            },
            ProvidesBindingKind::Syscall { .. } => {
                return Err(format!(
                    "provides `{}::{}`: Syscall bindings on a freestanding target are not                      wired yet (no syscall plan without a host)",
                    row.trait_name, row.method
                ));
            }
        };
        plan.bindings.insert(HostBinding {
            operation_key: key,
            mechanism,
            boundary_policy: PROVIDES_BOUNDARY_POLICY.into(),
        });
        // The call-site lowering: the receiver's boundary-trait name is the
        // platform, the method name is the state; one operation per call.
        insert_platform_lowering(
            &mut plan,
            "*",
            &row.method,
            [host_operation(&row.trait_name, &row.method)],
            PlatformCallData::None,
        );
    }
    Ok(plan)
}

impl HostAbiPlan {
    pub fn allows_boundary_policy(&self, policy: &str) -> bool {
        self.boundary_policies
            .iter()
            .any(|(_, allowed)| allowed.checked && allowed.path.as_ref() == policy)
    }
}

fn insert_platform_lowering<const COUNT: usize>(
    plan: &mut HostAbiPlan,
    platform: &str,
    state: &str,
    operations: [HostOperationReference; COUNT],
    data: PlatformCallData,
) {
    let operations = plan.host_operations.insert_many(operations);
    plan.platform_call_lowerings.insert(PlatformCallLowering {
        platform: Arc::from(platform),
        state: Arc::from(state),
        operations,
        data,
    });
}

fn host_operation(capability: &str, operation: &str) -> HostOperationReference {
    HostOperationReference {
        key: HostOperationKey::from_names(capability, operation),
    }
}

pub fn host_operation_fixed_leading_immediate(
    plan: &HostAbiPlan,
    operation_key: HostOperationKey,
) -> Option<i64> {
    match (
        plan.target.object_format,
        operation_key.capability,
        operation_key.operation,
    ) {
        (ObjectFormat::Coff, HostCapability::Stdout, HostOperation::GetStdHandle) => Some(-11),
        (ObjectFormat::Coff, HostCapability::Stdin, HostOperation::GetStdHandle) => Some(-10),
        (ObjectFormat::Coff, HostCapability::Stderr, HostOperation::GetStdHandle) => Some(-12),
        _ => None,
    }
}
