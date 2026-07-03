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
