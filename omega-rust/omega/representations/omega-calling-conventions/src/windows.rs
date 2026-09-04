/// The Windows import catalog rows: (capability, operation, library, symbol).
/// Single source of truth for the PE import table's symbol-to-library grouping.
pub const WINDOWS_IMPORT_ROWS: &[(&str, &str, &str, &str)] = &[
    ("Stdin", "get_std_handle", "Kernel32.dll", "GetStdHandle"),
    ("Stdin", "read_file", "Kernel32.dll", "ReadFile"),
    ("Stdin", "read", "Kernel32.dll", "ReadFile"),
    ("Stdout", "get_std_handle", "Kernel32.dll", "GetStdHandle"),
    ("Stdout", "write", "Kernel32.dll", "WriteFile"),
    ("Stdout", "write_file", "Kernel32.dll", "WriteFile"),
    ("Stderr", "get_std_handle", "Kernel32.dll", "GetStdHandle"),
    ("Stderr", "write", "Kernel32.dll", "WriteFile"),
    ("Stderr", "write_file", "Kernel32.dll", "WriteFile"),
    ("Process", "exit_process", "Kernel32.dll", "ExitProcess"),
    ("Clock", "sleep", "Kernel32.dll", "Sleep"),
    ("Clock", "tick_count", "Kernel32.dll", "GetTickCount64"),
    // std::time TimeHost seam (rung 5): out-param u64 reads (the constants
    // wall_clock_units_per_second / wall_clock_epoch_offset_seconds have NO
    // import row -- they lower as target constants, with no call at all).
    (
        "Clock",
        "monotonic_ticks",
        "Kernel32.dll",
        "QueryPerformanceCounter",
    ),
    (
        "Clock",
        "monotonic_ticks_per_second",
        "Kernel32.dll",
        "QueryPerformanceFrequency",
    ),
    (
        "Clock",
        "wall_clock_raw",
        "Kernel32.dll",
        "GetSystemTimePreciseAsFileTime",
    ),
    ("Input", "key_state", "User32.dll", "GetAsyncKeyState"),
    ("Gui", "dc_create", "Gdi32.dll", "CreateCompatibleDC"),
    ("Gui", "get_dc", "User32.dll", "GetDC"),
    ("Gui", "window_create", "User32.dll", "CreateWindowExA"),
    ("Gui", "blit", "Gdi32.dll", "StretchDIBits"),
    ("Gui", "msg_peek", "User32.dll", "PeekMessageW"),
    ("Gui", "msg_translate", "User32.dll", "TranslateMessage"),
    ("Gui", "msg_dispatch", "User32.dll", "DispatchMessageW"),
    ("Gui", "is_window", "User32.dll", "IsWindow"),
    ("Gui", "window_destroy", "User32.dll", "DestroyWindow"),
    (
        "Gui",
        "foreground_window",
        "User32.dll",
        "GetForegroundWindow",
    ),
    // std::fs raw seam (the windows_x64 mirror of darwin's libSystem rows):
    // msvcrt's POSIX-shaped CRT calls match the raw seam's value-returning
    // fd/count/rc surface directly (same arg shapes as the darwin libc calls),
    // so the general import-call encoder marshals them unchanged. The ops with
    // NO clean msvcrt equivalent (pread/pwrite, *at, link/symlink/readlink,
    // read_dir, flock, chown, futimens, realpath) keep the clean "no native
    // lowering" diagnostic. The stat family IS wired (2026-07-08): the wrapper's
    // `decode_metadata` projects the target's checked `StatLayout`, so
    // `_stat64`/`_fstat64` land.
    // `read_symlink_metadata` stays fenced (msvcrt has no `lstat`; mapping it to
    // `_stat64` would silently FOLLOW symlinks -- wrong, not just approximate).
    ("Filesystem", "open", "msvcrt.dll", "_open"),
    // `open_create` = `_open(path, flags, mode)` -- unfenced 2026-07-08 now that
    // the checked windows target encoder composes msvcrt flag words
    // (create_new/open_with no longer emit darwin O_CREAT 0x200,
    // which is msvcrt O_TRUNC). msvcrt `_open` takes the create `mode` as a
    // trailing variadic int; on win64 it lands in a normal arg register, so the
    // general import-call encoder marshals all three args like any Win64 call.
    ("Filesystem", "open_create", "msvcrt.dll", "_open"),
    ("Filesystem", "creat", "msvcrt.dll", "_creat"),
    ("Filesystem", "read", "msvcrt.dll", "_read"),
    ("Filesystem", "write", "msvcrt.dll", "_write"),
    ("Filesystem", "close", "msvcrt.dll", "_close"),
    ("Filesystem", "unlink", "msvcrt.dll", "_unlink"),
    ("Filesystem", "lseek", "msvcrt.dll", "_lseeki64"),
    ("Filesystem", "mkdir", "msvcrt.dll", "_mkdir"),
    ("Filesystem", "rmdir", "msvcrt.dll", "_rmdir"),
    ("Filesystem", "rename", "msvcrt.dll", "rename"),
    ("Filesystem", "dup", "msvcrt.dll", "_dup"),
    ("Filesystem", "fsync", "msvcrt.dll", "_commit"),
    ("Filesystem", "chmod", "msvcrt.dll", "_chmod"),
    // `_stat64(path, &_stat64)` / `_fstat64(fd, &_stat64)` -- the 64-bit-time
    // stat variant matching the wrapper's per-target `_stat64` offset layout.
    ("Filesystem", "stat", "msvcrt.dll", "_stat64"),
    ("Filesystem", "fstat", "msvcrt.dll", "_fstat64"),
    // The find-enumeration trio (fs portable-contract rung 3a): the windows
    // dir-walk paradigm. FindFirstFileA(pattern, &data) returns a HANDLE
    // (INVALID_HANDLE_VALUE = -1); FindNextFileA/FindClose return BOOL.
    ("Filesystem", "find_first", "Kernel32.dll", "FindFirstFileA"),
    ("Filesystem", "find_next", "Kernel32.dll", "FindNextFileA"),
    ("Filesystem", "find_close", "Kernel32.dll", "FindClose"),
    // The hard-link primitive (session slice 3): msvcrt has no `link`;
    // CreateHardLinkA takes (NEW link, existing, NULL security attrs) --
    // the designed seam op mirrors that exact shape (BOOL return) and the
    // windows Filesystem::hard_link impl swaps the portable arg order.
    (
        "Filesystem",
        "create_hard_link",
        "Kernel32.dll",
        "CreateHardLinkA",
    ),
    // Direct kernel HANDLE open/close. Unlike msvcrt `_open`, CreateFileA
    // with FILE_FLAG_BACKUP_SEMANTICS can open directories for metadata and
    // final-path queries.
    (
        "Filesystem",
        "open_path_handle",
        "Kernel32.dll",
        "CreateFileA",
    ),
    ("Filesystem", "close_handle", "Kernel32.dll", "CloseHandle"),
    // The handle bridge (session slice 4a): _get_osfhandle surfaces the OS
    // HANDLE behind a CRT fd; GetFinalPathNameByHandleA resolves an open
    // handle to its final DOS path (the honest windows canonicalize --
    // GetFullPathNameA is lexical-only and never left the ledger).
    (
        "Filesystem",
        "get_osfhandle",
        "msvcrt.dll",
        "_get_osfhandle",
    ),
    (
        "Filesystem",
        "final_path_name_by_handle",
        "Kernel32.dll",
        "GetFinalPathNameByHandleA",
    ),
    // The set_times leg over the bridge (slice 4b): stamp an open handle's
    // access/write times from wrapper-composed FILETIME buffers.
    ("Filesystem", "set_file_time", "Kernel32.dll", "SetFileTime"),
    // Whole-file advisory locks (session slice 4c). The wrapper supplies a
    // zero-offset OVERLAPPED and u64::MAX length; GetLastError is captured
    // immediately after non-blocking contention.
    ("Filesystem", "lock_file_ex", "Kernel32.dll", "LockFileEx"),
    ("Filesystem", "unlock_file", "Kernel32.dll", "UnlockFile"),
    (
        "Filesystem",
        "get_last_error",
        "Kernel32.dll",
        "GetLastError",
    ),
    // set_len -> `_chsize_s(fd, __int64 size)` (ftruncate's msvcrt analogue). The
    // 64-bit variant so the i64 length is not truncated to `_chsize`'s 32-bit
    // `long`; returns 0 on success like ftruncate (the wrapper checks rc == 0 and
    // reads errno on the error arm). Same fd+i64 marshalling as `_lseeki64`.
    ("Filesystem", "ftruncate", "msvcrt.dll", "_chsize_s"),
    ("Filesystem", "read_errno", "msvcrt.dll", "_errno"),
];

/// The DLL a Windows import symbol belongs to, per the catalog. `None` for
/// symbols outside the catalog (callers default to KERNEL32.dll, the
/// historical single-DLL behavior).
pub fn windows_import_library(symbol: &str) -> Option<&'static str> {
    WINDOWS_IMPORT_ROWS
        .iter()
        .find(|(_, _, _, row_symbol)| *row_symbol == symbol)
        .map(|(_, _, library, _)| *library)
}
