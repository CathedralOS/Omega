use crate::{
    CallSignature, CallingPolicy, ConcreteVariadicCallSignature, HostAbiPlan, HostBinding,
    HostBindingMechanism, HostBoundaryPolicy, PlatformCallData, ValueShape,
    evaluate_darwin_aapcs64_variadic_boundary_entry_plan, evaluate_ordinary_boundary_entry_plan,
    host_operation, insert_platform_lowering,
};

/// The canonical libc/libm umbrella every ordinary darwin import resolves through.
pub const DARWIN_LIBSYSTEM_PATH: &str = "/usr/lib/libSystem.B.dylib";
/// The Objective-C runtime dylib (`objc_msgSend`, `objc_getClass`,
/// `sel_registerName`, …) — a SEPARATE `LC_LOAD_DYLIB` from libSystem.
pub const DARWIN_LIBOBJC_PATH: &str = "/usr/lib/libobjc.A.dylib";
/// CoreGraphics — the `CG*` C API (`CGRectGetMaxX`, `CGImageCreate`,
/// `CGColorSpaceCreateDeviceRGB`, …). A directly-CALLED framework (unlike
/// Foundation/AppKit which load only for objc class registration).
pub const DARWIN_COREGRAPHICS_PATH: &str =
    "/System/Library/Frameworks/CoreGraphics.framework/Versions/A/CoreGraphics";

/// The absolute dylib path a darwin import symbol binds against (its
/// `LC_LOAD_DYLIB`). The Mach-O backend derives each import's dylib ordinal from
/// this, so a program that calls into the Objective-C runtime emits a second load
/// command for libobjc. Everything else (libc/libm) is the libSystem umbrella.
/// Mirrors `windows_import_library`; extends as CoreGraphics/AppKit/Foundation
/// symbols are added. Symbols are the Mach-O `_`-prefixed spellings.
pub fn darwin_import_library(symbol: &str) -> &'static str {
    if symbol.starts_with("_objc_")
        || symbol.starts_with("_sel_")
        || symbol.starts_with("_class_")
        || symbol.starts_with("_object_")
        || symbol.starts_with("_method_")
        || symbol.starts_with("_ivar_")
        || symbol.starts_with("_protocol_")
    {
        return DARWIN_LIBOBJC_PATH;
    }
    // The CoreGraphics C API is `CG`-prefixed (`_CGRectGetMaxX`, `_CGImageCreate`).
    if symbol.starts_with("_CG") {
        return DARWIN_COREGRAPHICS_PATH;
    }
    DARWIN_LIBSYSTEM_PATH
}

pub(crate) fn populate(plan: &mut HostAbiPlan) {
    let policy: std::sync::Arc<str> = "omega::host::targets::darwin".into();
    plan.boundary_policies.insert(HostBoundaryPolicy {
        path: std::sync::Arc::clone(&policy),
        checked: true,
    });

    plan.bindings.insert_many([
        darwin_word_import("Stdin", "read", "_read", 3, true, &policy),
        darwin_word_import("Stdout", "write", "_write", 3, true, &policy),
        darwin_word_import("Stderr", "write", "_write", 3, true, &policy),
        darwin_typed_import(
            "Process",
            "exit",
            "_exit",
            CallSignature {
                parameters: vec![ValueShape::integer(4, 4)],
                result: None,
            },
            &policy,
        ),
        darwin_filesystem_import("open", "_open", &[8, 4], 4, &policy),
        darwin_filesystem_import("creat", "_creat", &[8, 4], 4, &policy),
        darwin_filesystem_import("read", "_read", &[4, 8, 8], 8, &policy),
        darwin_filesystem_import("write", "_write", &[4, 8, 8], 8, &policy),
        darwin_filesystem_import("pread", "_pread", &[4, 8, 8, 8], 8, &policy),
        darwin_filesystem_import("pwrite", "_pwrite", &[4, 8, 8, 8], 8, &policy),
        darwin_filesystem_import("close", "_close", &[4], 4, &policy),
        darwin_filesystem_import("unlink", "_unlink", &[8], 4, &policy),
        darwin_filesystem_import("lseek", "_lseek", &[4, 8, 4], 8, &policy),
        darwin_filesystem_import("mkdir", "_mkdir", &[8, 4], 4, &policy),
        darwin_filesystem_import("rmdir", "_rmdir", &[8], 4, &policy),
        darwin_filesystem_import("openat", "_openat", &[4, 8, 4], 4, &policy),
        darwin_filesystem_import("unlinkat", "_unlinkat", &[4, 8, 4], 4, &policy),
        darwin_filesystem_import("chmod", "_chmod", &[8, 4], 4, &policy),
        darwin_filesystem_import("fchmod", "_fchmod", &[4, 4], 4, &policy),
        darwin_filesystem_import("rename", "_rename", &[8, 8], 4, &policy),
        darwin_filesystem_import("link", "_link", &[8, 8], 4, &policy),
        darwin_filesystem_import("symlink", "_symlink", &[8, 8], 4, &policy),
        darwin_filesystem_import("readlink", "_readlink", &[8, 8, 8], 8, &policy),
        darwin_filesystem_import(
            "getdirentries64",
            "___getdirentries64",
            &[4, 8, 8, 8],
            8,
            &policy,
        ),
        darwin_filesystem_import("stat", "_stat", &[8, 8], 4, &policy),
        darwin_filesystem_import("fstat", "_fstat", &[4, 8], 4, &policy),
        darwin_filesystem_import("lstat", "_lstat", &[8, 8], 4, &policy),
        darwin_filesystem_import("realpath", "_realpath", &[8, 8], 8, &policy),
        darwin_filesystem_import("ftruncate", "_ftruncate", &[4, 8], 4, &policy),
        darwin_filesystem_import("futimens", "_futimens", &[4, 8], 4, &policy),
        darwin_filesystem_import("fsync", "_fsync", &[4], 4, &policy),
        darwin_filesystem_import("dup", "_dup", &[4], 4, &policy),
        darwin_filesystem_import("flock", "_flock", &[4, 4], 4, &policy),
        darwin_filesystem_import("chown", "_chown", &[8, 4, 4], 4, &policy),
        darwin_filesystem_import("lchown", "_lchown", &[8, 4, 4], 4, &policy),
        darwin_filesystem_import("fchown", "_fchown", &[4, 4, 4], 4, &policy),
        darwin_open_create_import(&policy),
        darwin_typed_import(
            "Filesystem",
            "read_errno",
            "___error",
            CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::integer(4, 4)),
            },
            &policy,
        ),
        // First float-arg op: `Math::round_nearest(x: f64) -> i64` → libm `lround`.
        // Proves the arm64 float calling convention (double in v0, long in x0).
        darwin_typed_import(
            "Math",
            "round_nearest",
            "_lround",
            CallSignature {
                parameters: vec![ValueShape::float(8)],
                result: Some(ValueShape::integer(8, 8)),
            },
            &policy,
        ),
        // First float-RETURN op: `Math::square_root(x: f64) -> f64` → libm `sqrt`
        // (double in v0, double out d0). `hypotenuse(x, y) -> f64` → `_hypot`
        // adds a second float arg (v0, v1) alongside the float return.
        darwin_typed_import(
            "Math",
            "square_root",
            "_sqrt",
            CallSignature {
                parameters: vec![ValueShape::float(8)],
                result: Some(ValueShape::float(8)),
            },
            &policy,
        ),
        darwin_typed_import(
            "Math",
            "hypotenuse",
            "_hypot",
            CallSignature {
                parameters: vec![ValueShape::float(8); 2],
                result: Some(ValueShape::float(8)),
            },
            &policy,
        ),
        // Three f64 args (v0, v1, v2) → libm `fma`: proves the v-register sequence
        // reaches v2, i.e. an HFA of ≤4 doubles (NSRect) marshals into v0–v3.
        darwin_typed_import(
            "Math",
            "fused_multiply_add",
            "_fma",
            CallSignature {
                parameters: vec![ValueShape::float(8); 3],
                result: Some(ValueShape::float(8)),
            },
            &policy,
        ),
        // First SECOND-dylib import: `ObjectiveC::get_class(name) -> u64` →
        // libobjc `objc_getClass`. `darwin_import_library` routes `_objc_*` to
        // libobjc, so the Mach-O emits a 2nd LC_LOAD_DYLIB and binds this at
        // ordinal 2. (The binding's `library` string below is unused on darwin —
        // the Mach-O backend derives the dylib from the symbol name.)
        darwin_word_import(
            "ObjectiveC",
            "get_class",
            "_objc_getClass",
            1,
            true,
            &policy,
        ),
        darwin_word_import(
            "ObjectiveC",
            "register_selector",
            "_sel_registerName",
            1,
            true,
            &policy,
        ),
        // `send`/`send_scalar`/`send_string` share the `_objc_msgSend` symbol; the
        // op arm decides how many args to marshal (`[recv, sel, …]`).
        darwin_word_import("ObjectiveC", "send", "_objc_msgSend", 2, true, &policy),
        darwin_word_import(
            "ObjectiveC",
            "send_scalar",
            "_objc_msgSend",
            3,
            true,
            &policy,
        ),
        darwin_word_import(
            "ObjectiveC",
            "send_string",
            "_objc_msgSend",
            3,
            true,
            &policy,
        ),
        // The MIXED HFA-plus-scalar send: NSRect (4 doubles → v0–v3) + 3 trailing
        // scalars (→ x2–x4) for `initWithContentRect:styleMask:backing:defer:`.
        darwin_typed_import(
            "ObjectiveC",
            "send_rect",
            "_objc_msgSend",
            CallSignature {
                parameters: vec![
                    ValueShape::integer(8, 8),
                    ValueShape::integer(8, 8),
                    ValueShape::float(8),
                    ValueShape::float(8),
                    ValueShape::float(8),
                    ValueShape::float(8),
                    ValueShape::integer(8, 8),
                    ValueShape::integer(8, 8),
                    ValueShape::integer(8, 8),
                ],
                result: Some(ValueShape::integer(8, 8)),
            },
            &policy,
        ),
        // Four scalar args (x2–x5) for the event pump's
        // `nextEventMatchingMask:untilDate:inMode:dequeue:`.
        darwin_word_import(
            "ObjectiveC",
            "send_scalar4",
            "_objc_msgSend",
            6,
            true,
            &policy,
        ),
        // Scalar + NSSize (2 doubles → v0,v1) for `initWithCGImage:size:`.
        darwin_typed_import(
            "ObjectiveC",
            "send_image_size",
            "_objc_msgSend",
            CallSignature {
                parameters: vec![
                    ValueShape::integer(8, 8),
                    ValueShape::integer(8, 8),
                    ValueShape::integer(8, 8),
                    ValueShape::float(8),
                    ValueShape::float(8),
                ],
                result: Some(ValueShape::integer(8, 8)),
            },
            &policy,
        ),
        // Runtime byte-buffer string send (`initWithUTF8String:` over the samples'
        // title bytes, NUL-terminated by construction). Shares `_objc_msgSend`.
        darwin_word_import(
            "ObjectiveC",
            "send_byte_string",
            "_objc_msgSend",
            3,
            true,
            &policy,
        ),
        // The pump's autorelease-pool scope: dequeued NSEvents are autoreleased and
        // the pump runs outside any Cocoa-managed pool, so without a pool they leak.
        darwin_word_import(
            "ObjectiveC",
            "pool_push",
            "_objc_autoreleasePoolPush",
            0,
            true,
            &policy,
        ),
        darwin_word_import(
            "ObjectiveC",
            "pool_pop",
            "_objc_autoreleasePoolPop",
            1,
            true,
            &policy,
        ),
        // CoreGraphics geometry: a `CGRect` (4 doubles) is passed as an HFA in
        // v0–v3 (`_CG*` routes to CoreGraphics via `darwin_import_library`). The
        // run-verified proof that 4 doubles land in v0–v3.
        darwin_typed_import(
            "CoreGraphics",
            "rect_max_x",
            "_CGRectGetMaxX",
            CallSignature {
                parameters: vec![ValueShape::float(8); 4],
                result: Some(ValueShape::float(8)),
            },
            &policy,
        ),
        darwin_typed_import(
            "CoreGraphics",
            "rect_max_y",
            "_CGRectGetMaxY",
            CallSignature {
                parameters: vec![ValueShape::float(8); 4],
                result: Some(ValueShape::float(8)),
            },
            &policy,
        ),
        // The blit path: framebuffer → CGImage via a bitmap context (all
        // integer/pointer args, no stack spill — vs `CGImageCreate`'s 11).
        darwin_word_import(
            "CoreGraphics",
            "color_space_rgb",
            "_CGColorSpaceCreateDeviceRGB",
            0,
            true,
            &policy,
        ),
        darwin_word_import(
            "CoreGraphics",
            "bitmap_context",
            "_CGBitmapContextCreate",
            7,
            true,
            &policy,
        ),
        darwin_word_import(
            "CoreGraphics",
            "bitmap_context_image",
            "_CGBitmapContextCreateImage",
            1,
            true,
            &policy,
        ),
        darwin_word_import(
            "CoreGraphics",
            "image_width",
            "_CGImageGetWidth",
            1,
            true,
            &policy,
        ),
        // Blit-lifecycle releases: the per-frame context and CGImage snapshot are
        // Create-rule owned; without these every presented frame leaks both.
        darwin_word_import(
            "CoreGraphics",
            "context_release",
            "_CGContextRelease",
            1,
            true,
            &policy,
        ),
        darwin_word_import(
            "CoreGraphics",
            "image_release",
            "_CGImageRelease",
            1,
            true,
            &policy,
        ),
        // `Input.key_state` backing: `CGEventSourceKeyState(state_id, keycode) -> bool`.
        darwin_word_import(
            "CoreGraphics",
            "event_source_key_state",
            "_CGEventSourceKeyState",
            2,
            true,
            &policy,
        ),
        // `Clock::sleep(milliseconds)` → libc `poll(NULL, 0, milliseconds)`: with
        // zero fds, `poll`'s timeout IS a millisecond sleep (correct units, no
        // <1s cap — unlike `usleep`). Bound under the distinct `sleep_poll` op so
        // its operand arm places `[NULL, 0, ms]` in x0/x1/x2 (the shared `Sleep`
        // arm marshals a single arg into x0 for Win32 `Sleep`). `_poll` is in the
        // libSystem umbrella, so no new dylib.
        darwin_typed_import(
            "Clock",
            "sleep_poll",
            "_poll",
            CallSignature {
                parameters: vec![
                    ValueShape::integer(8, 8),
                    ValueShape::integer(4, 4),
                    ValueShape::integer(4, 4),
                ],
                result: Some(ValueShape::integer(4, 4)),
            },
            &policy,
        ),
        // std::time seam (TASKS_TIME.md rung 10): ONE symbol serves both the
        // monotonic and wall reads; the clockid argument comes from each
        // lowering row's ConstantArgument. The calibration ops are
        // ConstantResult rows (no import at all): POSIX nanosecond units.
        darwin_word_import(
            "Clock",
            "monotonic_ticks",
            "_clock_gettime_nsec_np",
            1,
            true,
            &policy,
        ),
        darwin_word_import(
            "Clock",
            "wall_clock_raw",
            "_clock_gettime_nsec_np",
            1,
            true,
            &policy,
        ),
        // The inline-Clock `tick_count()` spelling (the pre-std samples/
        // canaries): same symbol, same CLOCK_UPTIME_RAW clockid. RAW HOST
        // UNITS by doctrine (a boundary trait is the host's own surface):
        // windows GetTickCount64 ticks are MILLISECONDS, darwin ticks are
        // NANOSECONDS -- monotonic opaque ticks either way; the portable
        // calibrated surface is std::time (TimeHost), not this row.
        darwin_word_import(
            "Clock",
            "tick_count",
            "_clock_gettime_nsec_np",
            1,
            true,
            &policy,
        ),
    ]);

    insert_platform_lowering(
        plan,
        "*",
        "write_line",
        [host_operation("Stdout", "write")],
        PlatformCallData::FirstTextArgument {
            append_newline: true,
        },
    );
    insert_platform_lowering(
        plan,
        "*",
        "write",
        [host_operation("Stdout", "write")],
        PlatformCallData::FirstTextArgument {
            append_newline: false,
        },
    );
    insert_platform_lowering(
        plan,
        "*",
        "write_error_line",
        [host_operation("Stderr", "write")],
        PlatformCallData::FirstTextArgument {
            append_newline: true,
        },
    );
    insert_platform_lowering(
        plan,
        "*",
        "write_error",
        [host_operation("Stderr", "write")],
        PlatformCallData::FirstTextArgument {
            append_newline: false,
        },
    );
    insert_platform_lowering(
        plan,
        "*",
        "read_line",
        [host_operation("Stdin", "read")],
        PlatformCallData::MutableOutputBuffer { byte_capacity: 256 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "read_byte",
        [host_operation("Stdin", "read")],
        PlatformCallData::SingleByteRead,
    );
    insert_platform_lowering(
        plan,
        "*",
        "write_byte",
        [host_operation("Stdout", "write")],
        PlatformCallData::SingleByteWrite,
    );
    insert_platform_lowering(
        plan,
        "*",
        "exit_process",
        [host_operation("Process", "exit")],
        PlatformCallData::None,
    );
    // std::fs — the RAW, VALUE-RETURNING boundary layer (each op returns its
    // syscall result: fd / byte count / rc; a thin Omega layer wraps these into
    // File/result enums). HUMAN method names (create/open/read/write/close/
    // remove) — NO legacy C abbreviations in the Omega surface; the ugly libc
    // spellings (`_creat`,`_unlink`) live only in the binding symbols above.
    // Registered under the raw trait `FilesystemHost` (not `*`) so `write`/`read`
    // win the exact-platform lookup over Console's wildcard `write`. All marshal
    // declared args straight through (PlatformCallData::None); the value-returning
    // result store is driven by `HostOperationKey::returns_value()`.
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "open",
        [host_operation("Filesystem", "open")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "create",
        [host_operation("Filesystem", "creat")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "read",
        [host_operation("Filesystem", "read")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "write",
        [host_operation("Filesystem", "write")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "read_at",
        [host_operation("Filesystem", "pread")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "write_at",
        [host_operation("Filesystem", "pwrite")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "close",
        [host_operation("Filesystem", "close")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "remove",
        [host_operation("Filesystem", "unlink")],
        PlatformCallData::None,
    );
    // The TRUSTED plain-path removal twins (D-at trust class, the
    // create_dir_name precedent) -- same native rows as remove/remove_dir.
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "remove_name",
        [host_operation("Filesystem", "unlink")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "remove_dir_name",
        [host_operation("Filesystem", "rmdir")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "seek",
        [host_operation("Filesystem", "lseek")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "create_dir",
        [host_operation("Filesystem", "mkdir")],
        PlatformCallData::None,
    );
    // The TRUSTED plain-name variant (D-at trust class; create_dir_all's
    // NUL-terminated prefix scratch) -- same native row.
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "create_dir_name",
        [host_operation("Filesystem", "mkdir")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "remove_dir",
        [host_operation("Filesystem", "rmdir")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "open_at",
        [host_operation("Filesystem", "openat")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "unlink_at",
        [host_operation("Filesystem", "unlinkat")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "set_permissions",
        [host_operation("Filesystem", "chmod")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "set_file_permissions",
        [host_operation("Filesystem", "fchmod")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "rename",
        [host_operation("Filesystem", "rename")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "hard_link",
        [host_operation("Filesystem", "link")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "symlink",
        [host_operation("Filesystem", "symlink")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "read_link",
        [host_operation("Filesystem", "readlink")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "read_dir",
        [host_operation("Filesystem", "getdirentries64")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "read_metadata",
        [host_operation("Filesystem", "stat")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "read_file_metadata",
        [host_operation("Filesystem", "fstat")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "read_symlink_metadata",
        [host_operation("Filesystem", "lstat")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "canonicalize",
        [host_operation("Filesystem", "realpath")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "set_len",
        [host_operation("Filesystem", "ftruncate")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "set_file_times",
        [host_operation("Filesystem", "futimens")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "sync",
        [host_operation("Filesystem", "fsync")],
        PlatformCallData::None,
    );
    // `sync_data` (Rust `File::sync_data`) maps to `fsync` on darwin -- the same op
    // as `sync`. (macOS has no `fdatasync`; Rust itself falls back to fsync there.)
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "sync_data",
        [host_operation("Filesystem", "fsync")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "duplicate",
        [host_operation("Filesystem", "dup")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "lock_file",
        [host_operation("Filesystem", "flock")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "change_owner",
        [host_operation("Filesystem", "chown")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "change_owner_no_follow",
        [host_operation("Filesystem", "lchown")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "change_file_owner",
        [host_operation("Filesystem", "fchown")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "open_create",
        [host_operation("Filesystem", "open_create")],
        PlatformCallData::None,
    );
    // errno accessor: `___error()` returns `&errno`; the value-returning lowering
    // derefs the returned pointer once (see `dereferences_result`) so the stored
    // result is the errno integer, not the pointer. No args.
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "errno",
        [host_operation("Filesystem", "read_errno")],
        PlatformCallData::None,
    );

    // `Math::round_nearest(x: f64) -> i64` → `_lround`: the first host op with an
    // f64 ARGUMENT (marshalled into v0 by the RuntimeScalarFloat operand). The
    // value-returning shape is identical to the scalar fs ops (result in x0);
    // only the argument register file differs.
    insert_platform_lowering(
        plan,
        "Math",
        "round_nearest",
        [host_operation("Math", "round_nearest")],
        PlatformCallData::None,
    );

    // `Math::square_root(x: f64) -> f64` → `_sqrt`: the first host op whose result
    // comes back in the FLOAT register `d0`; the aarch64 lowering moves it to x0
    // with `fmov x0,d0` before the normal result store (see `returns_float`).
    insert_platform_lowering(
        plan,
        "Math",
        "square_root",
        [host_operation("Math", "square_root")],
        PlatformCallData::None,
    );

    // `Math::hypotenuse(x: f64, y: f64) -> f64` → `_hypot`: two f64 args (v0, v1)
    // plus a float return — proves multi-float-arg register sequencing.
    insert_platform_lowering(
        plan,
        "Math",
        "hypotenuse",
        [host_operation("Math", "hypotenuse")],
        PlatformCallData::None,
    );

    // `Math::fused_multiply_add(x, y, z) -> f64` → `_fma`: three f64 args (v0, v1,
    // v2) + a float return — proves the v-register sequence extends to v2 (HFA).
    insert_platform_lowering(
        plan,
        "Math",
        "fused_multiply_add",
        [host_operation("Math", "fused_multiply_add")],
        PlatformCallData::None,
    );

    // `ObjectiveC::get_class(name) -> u64` → `_objc_getClass`: a NUL-terminated
    // class-name string (materialized like an fs path) + a pointer result — the
    // first call into a second dylib (libobjc).
    insert_platform_lowering(
        plan,
        "ObjectiveC",
        "get_class",
        [host_operation("ObjectiveC", "get_class")],
        PlatformCallData::None,
    );
    // `ObjectiveC::register_selector(name) -> u64` → `_sel_registerName`: same
    // string-arg/pointer-result shape as `get_class`.
    insert_platform_lowering(
        plan,
        "ObjectiveC",
        "register_selector",
        [host_operation("ObjectiveC", "register_selector")],
        PlatformCallData::None,
    );
    // `ObjectiveC::send(recv, sel) -> u64` and `send_string(recv, sel, text) -> u64`
    // → `_objc_msgSend`: the message-send workhorse (recv→x0, sel→x1, then args).
    insert_platform_lowering(
        plan,
        "ObjectiveC",
        "send",
        [host_operation("ObjectiveC", "send")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "ObjectiveC",
        "send_scalar",
        [host_operation("ObjectiveC", "send_scalar")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "ObjectiveC",
        "send_string",
        [host_operation("ObjectiveC", "send_string")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "ObjectiveC",
        "send_rect",
        [host_operation("ObjectiveC", "send_rect")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "ObjectiveC",
        "send_scalar4",
        [host_operation("ObjectiveC", "send_scalar4")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "ObjectiveC",
        "send_image_size",
        [host_operation("ObjectiveC", "send_image_size")],
        PlatformCallData::None,
    );
    // Runtime byte-buffer string send (`initWithUTF8String:` over title bytes).
    insert_platform_lowering(
        plan,
        "ObjectiveC",
        "send_byte_string",
        [host_operation("ObjectiveC", "send_byte_string")],
        PlatformCallData::None,
    );
    // The pump's autorelease-pool scope (push returns the token pop consumes).
    insert_platform_lowering(
        plan,
        "ObjectiveC",
        "pool_push",
        [host_operation("ObjectiveC", "pool_push")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "ObjectiveC",
        "pool_pop",
        [host_operation("ObjectiveC", "pool_pop")],
        PlatformCallData::None,
    );
    // `CoreGraphics::rect_max_x/y(x, y, w, h) -> f64` → `CGRectGetMaxX`/`MaxY`: the
    // 4-double CGRect marshals as an HFA into v0–v3; the CGFloat result is in d0.
    insert_platform_lowering(
        plan,
        "CoreGraphics",
        "rect_max_x",
        [host_operation("CoreGraphics", "rect_max_x")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "CoreGraphics",
        "rect_max_y",
        [host_operation("CoreGraphics", "rect_max_y")],
        PlatformCallData::None,
    );
    // The blit path (all int/ptr args → registers, results in x0).
    insert_platform_lowering(
        plan,
        "CoreGraphics",
        "color_space_rgb",
        [host_operation("CoreGraphics", "color_space_rgb")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "CoreGraphics",
        "bitmap_context",
        [host_operation("CoreGraphics", "bitmap_context")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "CoreGraphics",
        "bitmap_context_image",
        [host_operation("CoreGraphics", "bitmap_context_image")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "CoreGraphics",
        "image_width",
        [host_operation("CoreGraphics", "image_width")],
        PlatformCallData::None,
    );
    // Blit-lifecycle releases (Create-rule drops for the per-frame context/image).
    insert_platform_lowering(
        plan,
        "CoreGraphics",
        "context_release",
        [host_operation("CoreGraphics", "context_release")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "CoreGraphics",
        "image_release",
        [host_operation("CoreGraphics", "image_release")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "CoreGraphics",
        "event_source_key_state",
        [host_operation("CoreGraphics", "event_source_key_state")],
        PlatformCallData::None,
    );
    // `Clock::sleep(milliseconds)` → the distinct `sleep_poll` op (poll-based
    // millisecond sleep). The `"*"` platform matches the sample's `clock.sleep(ms)`
    // boundary call regardless of the receiver type.
    // std::time seam (rung 10): monotonic = CLOCK_UPTIME_RAW (8), wall =
    // CLOCK_REALTIME (0), both through `clock_gettime_nsec_np` -- the
    // lowering layer never does arithmetic (D11), units are POSIX 10^9.
    insert_platform_lowering(
        plan,
        "*",
        "monotonic_ticks",
        [host_operation("Clock", "monotonic_ticks")],
        PlatformCallData::ConstantArgument { value: 8 },
    );
    // Inline-Clock tick_count: raw nanosecond ticks (see the import row).
    insert_platform_lowering(
        plan,
        "*",
        "tick_count",
        [host_operation("Clock", "tick_count")],
        PlatformCallData::ConstantArgument { value: 8 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "monotonic_ticks_per_second",
        [host_operation("Clock", "monotonic_ticks_per_second")],
        PlatformCallData::ConstantResult { value: 1000000000 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "wall_clock_raw",
        [host_operation("Clock", "wall_clock_raw")],
        PlatformCallData::ConstantArgument { value: 0 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "wall_clock_units_per_second",
        [host_operation("Clock", "wall_clock_units_per_second")],
        PlatformCallData::ConstantResult { value: 1000000000 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "wall_clock_epoch_offset_seconds",
        [host_operation("Clock", "wall_clock_epoch_offset_seconds")],
        PlatformCallData::ConstantResult { value: 0 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "sleep",
        [host_operation("Clock", "sleep_poll")],
        PlatformCallData::None,
    );
}

fn darwin_word_import(
    capability: &str,
    operation: &str,
    symbol: &str,
    parameter_count: usize,
    has_result: bool,
    policy: &std::sync::Arc<str>,
) -> HostBinding {
    let word = ValueShape::integer(8, 8);
    let signature = CallSignature {
        parameters: vec![word; parameter_count],
        result: has_result.then_some(word),
    };
    darwin_typed_import(capability, operation, symbol, signature, policy)
}

fn darwin_typed_import(
    capability: &str,
    operation: &str,
    symbol: &str,
    signature: CallSignature,
    policy: &std::sync::Arc<str>,
) -> HostBinding {
    let boundary_entry_plan =
        evaluate_ordinary_boundary_entry_plan(CallingPolicy::Aapcs64, &signature)
            .expect("the built-in Darwin import signature must have an AAPCS64 plan")
            .plan()
            .clone();
    HostBinding {
        operation_key: crate::HostOperationKey::from_names(capability, operation),
        mechanism: HostBindingMechanism::Import {
            library: "libSystem.B.dylib".into(),
            symbol: symbol.into(),
        },
        boundary_policy: std::sync::Arc::clone(policy),
        boundary_entry_plan,
    }
}

fn darwin_filesystem_import(
    operation: &str,
    symbol: &str,
    parameter_widths: &[u16],
    result_width: u16,
    policy: &std::sync::Arc<str>,
) -> HostBinding {
    darwin_typed_import(
        "Filesystem",
        operation,
        symbol,
        CallSignature {
            parameters: parameter_widths
                .iter()
                .copied()
                .map(|width| ValueShape::integer(width, width))
                .collect(),
            result: Some(ValueShape::integer(result_width, result_width)),
        },
        policy,
    )
}

fn darwin_open_create_import(policy: &std::sync::Arc<str>) -> HostBinding {
    let signature = ConcreteVariadicCallSignature {
        fixed_parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(4, 4)],
        variadic_parameters: vec![ValueShape::integer(4, 4)],
        result: Some(ValueShape::integer(4, 4)),
    };
    let boundary_entry_plan = evaluate_darwin_aapcs64_variadic_boundary_entry_plan(&signature)
        .expect("the built-in Darwin open_create signature must have an Apple variadic plan")
        .plan()
        .clone();
    HostBinding {
        operation_key: crate::HostOperationKey::from_names("Filesystem", "open_create"),
        mechanism: HostBindingMechanism::Import {
            library: "libSystem.B.dylib".into(),
            symbol: "_open".into(),
        },
        boundary_policy: std::sync::Arc::clone(policy),
        boundary_entry_plan,
    }
}
