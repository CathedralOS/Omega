/*
We maintain the keyed catalog of host capabilities (`HostCapability`) and host
operations (`HostOperation`) with process-wide interning for custom authored
names, the external binding rows (`ExternalBindingRow`, `ExternalBindingKind`)
that connect a bodyless external leaf to its foreign locator or table dispatch
mechanism, and x86-64 interrupt gate address fragment assembly
(`InterruptGateAddress`, `interrupt_gate_address`). Both directions of our capability and operation
catalogs read one table so authored spellings and enum keys never drift, while
custom names outside the built-in catalog intern to a stable index for the life
of the process.
*/

use crate::plans::BoundaryEntryPlan;
use layout_plans::LayoutPlacementReport;
use semantic_vocabulary::{AuthoredBitFragment, AuthoredBitFragments};
use target::NormalizedForeignLocator;

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

fn row_for_spelling<T: Copy>(rows: &[(T, &'static str)], name: &str) -> Option<T> {
    rows.iter()
        .find_map(|(row, spelling)| (*spelling == name).then_some(*row))
}

fn spelling_for_row<T: PartialEq>(rows: &[(T, &'static str)], value: T) -> Option<&'static str> {
    rows.iter()
        .find_map(|(row, spelling)| (*row == value).then_some(*spelling))
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
    Gui,
    Filesystem,
    Math,
    ObjectiveC,
    CoreGraphics,
    Custom(u32),
}

const HOST_CAPABILITY_ROWS: &[(HostCapability, &str)] = &[
    (HostCapability::Process, "Process"),
    (HostCapability::Stdin, "Stdin"),
    (HostCapability::Stdout, "Stdout"),
    (HostCapability::Stderr, "Stderr"),
    (HostCapability::Clock, "Clock"),
    (HostCapability::Input, "Input"),
    (HostCapability::Gui, "Gui"),
    (HostCapability::Filesystem, "Filesystem"),
    (HostCapability::Math, "Math"),
    (HostCapability::ObjectiveC, "ObjectiveC"),
    (HostCapability::CoreGraphics, "CoreGraphics"),
];

impl HostCapability {
    pub fn from_name(name: &str) -> Self {
        row_for_spelling(HOST_CAPABILITY_ROWS, name)
            .unwrap_or_else(|| Self::Custom(intern_custom_name(name)))
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Unknown => "<unknown>",
            Self::Custom(index) => custom_name(index),
            built_in => spelling_for_row(HOST_CAPABILITY_ROWS, built_in)
                .expect("every built-in capability carries its spelling in the catalog"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostOperation {
    #[default]
    Unknown,
    Custom(u32),
    Exit,
    ExitGroup,
    ExitProcess,
    GetStdHandle,
    Read,
    ReadFile,
    Write,
    WriteFile,
    PRead,
    PWrite,
    Open,
    Creat,
    Close,
    CreateFile,
    CloseHandle,
    Unlink,
    Seek,
    MakeDir,
    RemoveDir,
    OpenAt,
    UnlinkAt,
    Chmod,
    Fchmod,
    Rename,
    Link,
    Symlink,
    ReadLink,
    ReadDir,
    FindFirst,
    FindNext,
    FindClose,
    CreateHardLink,
    GetOsfHandle,
    FinalPathNameByHandle,
    SetFileTime,
    LockFileEx,
    UnlockFile,
    GetLastError,
    Stat,
    FStat,
    LStat,
    Realpath,
    SetLen,
    SetFileTimes,
    Sync,
    Dup,
    Flock,
    Chown,
    LChown,
    Fchown,
    OpenCreate,
    ReadErrno,
    RoundNearest,
    SquareRoot,
    Hypotenuse,
    FusedMultiplyAdd,
    GetClass,
    RegisterSelector,
    MsgSend,
    MsgSendScalar,
    MsgSendString,
    MsgSendRect,
    MsgSendScalar4,
    MsgSendImageSize,
    MsgSendByteString,
    PoolPush,
    PoolPop,
    RectMaxX,
    RectMaxY,
    ColorSpaceRgb,
    BitmapContext,
    BitmapContextImage,
    ImageWidth,
    ContextRelease,
    ImageRelease,
    EventSourceKeyState,
    Sleep,
    SleepPoll,
    TickCount,
    MonotonicTicks,
    MonotonicTicksPerSecond,
    WallClockRaw,
    WallClockUnitsPerSecond,
    WallClockEpochOffsetSeconds,
    KeyState,
    DcCreate,
    GetDc,
    WindowCreate,
    Blit,
    MsgPeek,
    MsgTranslate,
    MsgDispatch,
    IsWindow,
    WindowDestroy,
    ForegroundWindow,
}

const HOST_OPERATION_ROWS: &[(HostOperation, &str)] = &[
    (HostOperation::Exit, "exit"),
    (HostOperation::ExitGroup, "exit_group"),
    (HostOperation::ExitProcess, "exit_process"),
    (HostOperation::GetStdHandle, "get_std_handle"),
    (HostOperation::Read, "read"),
    (HostOperation::ReadFile, "read_file"),
    (HostOperation::Write, "write"),
    (HostOperation::WriteFile, "write_file"),
    (HostOperation::PRead, "pread"),
    (HostOperation::PWrite, "pwrite"),
    (HostOperation::Open, "open"),
    (HostOperation::CreateFile, "open_path_handle"),
    (HostOperation::Creat, "creat"),
    (HostOperation::Close, "close"),
    (HostOperation::CloseHandle, "close_handle"),
    (HostOperation::Unlink, "unlink"),
    (HostOperation::Seek, "lseek"),
    (HostOperation::MakeDir, "mkdir"),
    (HostOperation::RemoveDir, "rmdir"),
    (HostOperation::OpenAt, "openat"),
    (HostOperation::UnlinkAt, "unlinkat"),
    (HostOperation::Chmod, "chmod"),
    (HostOperation::Fchmod, "fchmod"),
    (HostOperation::Rename, "rename"),
    (HostOperation::Link, "link"),
    (HostOperation::Symlink, "symlink"),
    (HostOperation::ReadLink, "readlink"),
    (HostOperation::ReadDir, "getdirentries64"),
    (HostOperation::FindFirst, "find_first"),
    (HostOperation::FindNext, "find_next"),
    (HostOperation::FindClose, "find_close"),
    (HostOperation::CreateHardLink, "create_hard_link"),
    (HostOperation::GetOsfHandle, "get_osfhandle"),
    (
        HostOperation::FinalPathNameByHandle,
        "final_path_name_by_handle",
    ),
    (HostOperation::SetFileTime, "set_file_time"),
    (HostOperation::LockFileEx, "lock_file_ex"),
    (HostOperation::UnlockFile, "unlock_file"),
    (HostOperation::GetLastError, "get_last_error"),
    (HostOperation::Stat, "stat"),
    (HostOperation::FStat, "fstat"),
    (HostOperation::LStat, "lstat"),
    (HostOperation::Realpath, "realpath"),
    (HostOperation::SetLen, "ftruncate"),
    (HostOperation::SetFileTimes, "futimens"),
    (HostOperation::Sync, "fsync"),
    (HostOperation::Dup, "dup"),
    (HostOperation::Flock, "flock"),
    (HostOperation::Chown, "chown"),
    (HostOperation::LChown, "lchown"),
    (HostOperation::Fchown, "fchown"),
    (HostOperation::OpenCreate, "open_create"),
    (HostOperation::ReadErrno, "read_errno"),
    (HostOperation::RoundNearest, "round_nearest"),
    (HostOperation::SquareRoot, "square_root"),
    (HostOperation::Hypotenuse, "hypotenuse"),
    (HostOperation::FusedMultiplyAdd, "fused_multiply_add"),
    (HostOperation::GetClass, "get_class"),
    (HostOperation::RegisterSelector, "register_selector"),
    (HostOperation::MsgSend, "send"),
    (HostOperation::MsgSendScalar, "send_scalar"),
    (HostOperation::MsgSendString, "send_string"),
    (HostOperation::MsgSendRect, "send_rect"),
    (HostOperation::MsgSendScalar4, "send_scalar4"),
    (HostOperation::MsgSendImageSize, "send_image_size"),
    (HostOperation::MsgSendByteString, "send_byte_string"),
    (HostOperation::PoolPush, "pool_push"),
    (HostOperation::PoolPop, "pool_pop"),
    (HostOperation::RectMaxX, "rect_max_x"),
    (HostOperation::RectMaxY, "rect_max_y"),
    (HostOperation::ColorSpaceRgb, "color_space_rgb"),
    (HostOperation::BitmapContext, "bitmap_context"),
    (HostOperation::BitmapContextImage, "bitmap_context_image"),
    (HostOperation::ImageWidth, "image_width"),
    (HostOperation::ContextRelease, "context_release"),
    (HostOperation::ImageRelease, "image_release"),
    (HostOperation::EventSourceKeyState, "event_source_key_state"),
    (HostOperation::Sleep, "sleep"),
    (HostOperation::SleepPoll, "sleep_poll"),
    (HostOperation::TickCount, "tick_count"),
    (HostOperation::MonotonicTicks, "monotonic_ticks"),
    (
        HostOperation::MonotonicTicksPerSecond,
        "monotonic_ticks_per_second",
    ),
    (HostOperation::WallClockRaw, "wall_clock_raw"),
    (
        HostOperation::WallClockUnitsPerSecond,
        "wall_clock_units_per_second",
    ),
    (
        HostOperation::WallClockEpochOffsetSeconds,
        "wall_clock_epoch_offset_seconds",
    ),
    (HostOperation::KeyState, "key_state"),
    (HostOperation::DcCreate, "dc_create"),
    (HostOperation::GetDc, "get_dc"),
    (HostOperation::WindowCreate, "window_create"),
    (HostOperation::Blit, "blit"),
    (HostOperation::MsgPeek, "msg_peek"),
    (HostOperation::MsgTranslate, "msg_translate"),
    (HostOperation::MsgDispatch, "msg_dispatch"),
    (HostOperation::IsWindow, "is_window"),
    (HostOperation::WindowDestroy, "window_destroy"),
    (HostOperation::ForegroundWindow, "foreground_window"),
];

impl HostOperation {
    pub fn from_name(name: &str) -> Self {
        row_for_spelling(HOST_OPERATION_ROWS, name)
            .unwrap_or_else(|| Self::Custom(intern_custom_name(name)))
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Unknown => "<unknown>",
            Self::Custom(index) => custom_name(index),
            catalog => spelling_for_row(HOST_OPERATION_ROWS, catalog)
                .expect("every catalog operation carries its spelling in the catalog"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalBindingRow {
    pub target_name: String,
    pub trait_name: String,
    pub method: String,
    pub requirement_identity: String,
    pub provider_type: String,
    pub calling_applications: Vec<crate::BoundaryCallingApplication>,
    pub binding: ExternalBindingKind,
}

impl ExternalBindingRow {
    pub fn calling_application(
        &self,
        abi: &semantic_vocabulary::CallingAbiIdentity,
    ) -> Option<&crate::BoundaryCallingApplication> {
        let mut applications = self
            .calling_applications
            .iter()
            .filter(|application| &application.abi == abi);
        let application = applications.next()?;
        applications.next().is_none().then_some(application)
    }

    pub fn boundary_entry_plan(
        &self,
        abi: &semantic_vocabulary::CallingAbiIdentity,
    ) -> Option<&BoundaryEntryPlan> {
        self.calling_application(abi)
            .map(|application| application.plan.plan())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionFieldLocation {
    pub selector: target::ForeignLocatorIdentityDigest,
    pub formal: u32,
    pub byte_offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalBindingKind {
    FunctionField(FunctionFieldLocation),
    Syscall { number: i64 },
    Import { locator: NormalizedForeignLocator },
    CompilerIntrinsic { machine: String },
    VtableSlot { index: i64 },
    VtableField { field: String, byte_offset: u32 },
    TableFunction { field: String, byte_offset: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptGateAddress {
    pub fragments: AuthoredBitFragments,
    pub envelope_offset: u32,
    pub gate_byte_offset: u32,
}

pub fn interrupt_gate_address(
    placements: &[LayoutPlacementReport],
    table_bytes: u32,
) -> Result<InterruptGateAddress, &'static str> {
    let mut fragments = Vec::with_capacity(placements.len());
    let mut covered = 0u64;
    let mut gate = None;
    for placement in placements {
        let LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } = *placement
        else {
            return Err("interrupt entry address must retain its scattered hardware fragments");
        };
        if width == 0
            || container_width == 0
            || container_width > 64
            || container_width % 8 != 0
            || destination_lsb
                .checked_add(width)
                .is_none_or(|end| end > container_width)
            || source_lsb.checked_add(width).is_none_or(|end| end > 64)
            || container
                .checked_add(container_width / 8)
                .is_none_or(|end| end > u64::from(table_bytes))
        {
            return Err("interrupt address fragment exceeds its source or table container");
        }
        for bit in 0..width {
            let source = source_lsb + bit;
            let flag = 1u64 << source;
            if covered & flag != 0 {
                return Err("interrupt address source bit is duplicated");
            }
            covered |= flag;
            let hardware_bit = match source {
                0..=15 => source,
                _ => source + 32,
            };
            let physical = container
                .checked_mul(8)
                .and_then(|base| base.checked_add(destination_lsb + bit))
                .ok_or("interrupt address bit coordinate overflowed")?;
            let base = physical
                .checked_sub(hardware_bit)
                .ok_or("interrupt address fragment precedes its gate")?;
            if base % 128 != 0 || gate.is_some_and(|old| old != base) {
                return Err("private address fragments do not describe one aligned interrupt gate");
            }
            gate = Some(base);
        }
        fragments.push(AuthoredBitFragment {
            container_byte_offset: container,
            container_width_bits: container_width,
            destination_lsb,
            source_lsb,
            width,
        });
    }
    if covered != u64::MAX {
        return Err("interrupt address does not supply all 64 bits");
    }
    let gate_byte_offset = u32::try_from(gate.ok_or("interrupt address has no gate")? / 8)
        .map_err(|_| "interrupt gate offset exceeds native storage")?;
    if gate_byte_offset
        .checked_add(16)
        .is_none_or(|end| end > table_bytes)
    {
        return Err("interrupt gate exceeds the supplied table");
    }
    let fragments = AuthoredBitFragments::new(fragments);
    let envelope_offset = u32::try_from(
        fragments
            .byte_envelope()
            .ok_or("interrupt address has no container envelope")?
            .0,
    )
    .map_err(|_| "interrupt address envelope exceeds native storage")?;
    Ok(InterruptGateAddress {
        fragments,
        envelope_offset,
        gate_byte_offset,
    })
}
