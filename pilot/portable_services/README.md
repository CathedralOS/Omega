# Pilot: cross-target behaviour tests for std portable services

Measurement prototype only; not for `main`. It asks whether one set of
behaviour tests per portable std service, run on every hosted target, would
find real divergences cheaply enough to adopt.

**Result:** no behaviour probe can run on any target at `1e1e9fc3`. Every
portable filesystem call that takes a path is refused at compile time, and
so is a program that uses std's `Console`. The static cross-family diff
(`portable_diff.py`, under one second) pointed at all six divergences the
audit had found by hand, plus three new ones.

## What ran

| Target | What ran | Result |
| --- | --- | --- |
| linux_x86_64 | `omega run --both` on `probes/fs_probe`, `probes/exit_probe`, the std-copy variant and the CI fixture `tests/fixtures/run/providers/default_console_provider_reached` | All refused before Psi or native run (exit 200). See below. |
| linux_arm64 | nothing | Installing qemu-user-static was not permitted in this session. |
| macos_arm64, windows_x86_64 | nothing | No host. CI has only Linux runners. |
| all four | `portable_diff.py` | 0.9 s, 51 of 52 members flagged, 286 fact rows |

The compiler refused each program for one of these reasons:
- `fs_probe` and `samples/cli/systems/file_journal`: a string literal passed for
  a `&[u8] in Path` parameter needs `as +Path`, but `Path` (std
  `filesystem_host.omg:3`) is not importable from an application
  (`use omega_language_std::Path` and `as +omega_language_std::Path` both refuse).
- The same probe compiled inside a copy of std, where `Path` is visible:
  `not supported yet: boundary contract needs structural domain, reference or
  content publication` at the first `FilesystemService::write_all(... as +Path, ...)`.
- `exit_probe` and the CI std fixture: `not supported yet` in `console.omg`
  (`console_write_bytes` reach ceiling; `LinuxX86ConsoleIo::read` operator).

## Divergences (each read in both adapters)

Audit items, all confirmed. None of them is in `OWNER_CODE_CONCERNS.md`:
1. `open_with` with truncate but no write. On Windows, `filesystem/handles.omg:30`
   refuses it with `InvalidInput`. On Linux, `linux_x86_64/filesystem_impl.omg:42`
   (and arm64:107, macos:58) encodes `O_RDONLY|O_TRUNC`, which empties the file.
2. `set_permissions(Default)` / `set_file_permissions(Default)`. On POSIX,
   `posix_filesystem_adapter.omg:1009,1061` chmods to 0666 without applying
   the umask, so the file becomes world-writable. On Windows,
   `filesystem/metadata.omg:161,180` only clears read-only.
3. POSIX `remove_dir_all` stops after a 4096-step walk budget
   (`posix_filesystem_adapter.omg:1477`, which fails with `-5`, mapped to `Other`) and at a
   directory depth of 16 (`:422`, `:1625`, which fails with `-24`), leaving a partly deleted
   tree. Windows `filesystem_removal/` has neither limit.
4. POSIX `create_dir_all` skips every parent whose `/` is at byte 255 or later
   (`:919`, `:979`, `issue_skip` returns 0 at `:988`). The final `mkdir` then
   fails with `NotFound`, although no component is too long.
5. A read count larger than the buffer. Windows `filesystem/handles.omg:110`
   refuses it with `InvalidInput`. POSIX `linux_*/posix_filesystem_impl.omg`
   (`min_u64(count, buffer.len)`) clamps it to the buffer length. `read_all`, `read_at` and `copy` behave the same way.
6. `sleep(u32::MAX)`. Windows `time_host_impl.omg:119` and the `Clock`
   binding in `build.omg:104` pass it to `Sleep`, where `0xFFFFFFFF`
   (`INFINITE`) never returns. POSIX waits about 49.7 days
   (`in_parts`). Unrelated to Windows: POSIX `Clock::sleep` returns early on a signal,
   while `TimeHost::sleep` retries the full interval. Both are documented in
   comments.

New leads from the static diff:
7. `set_times` range. Windows `filesystem/metadata.omg:110-117` refuses
   times at or before 1601-01-01 and after year ~60056 (`InvalidInput`, or
   `Unsupported` at exactly the epoch). POSIX passes any `i64`
   to `utimensat`. The comment cites the Win32 documentation, so this is
   partly deliberate, but the portable contract does not say so.
8. Directory entry names. Windows `windows_filesystem_directory_entry.omg:140`
   errors on a name whose UTF-8 form is more than 256 bytes. A legal NTFS name
   of 255 UTF-16 units can need up to 765 bytes, so
   `read_dir_nth` and `read_dir_is_empty` fail on such a directory. POSIX allows 255 bytes
   (`posix_filesystem_adapter.omg:1405`), which is the filesystem limit there.
9. `write` of more than 4 GiB in one call. Windows
   `windows_filesystem_transfer.omg:78` refuses it (`-87` → `InvalidInput`).
   POSIX makes a short write. `write_all` splits the data into chunks on both.

Also seen: `ProcessExit::exit_process` takes an `i32`. POSIX keeps only the low
8 bits (256 → 0, which looks like success), while Windows keeps 32 bits. This
is an operating-system property, and no spec text covers it.

## False positives

Of the 286 rows in `portable_diff_output.txt`, about 30 point at the nine items above.
The rest are deliberate or harmless:
- Record layout offsets, flag words and Win32 access masks (`limit < 489`,
  `const create_file(..1048704..)`, …).
- The bounds of the POSIX path NUL-termination helper (`limit < path.len`,
  `< out.len`).
- The POSIX descriptor bound (`<= 2147483648`) beside the Windows zero-handle
  `BadDescriptor`.
- `refuse Unsupported` on Windows for `symlink`, the owner calls and the POSIX
  modes. The portable contract allows `Unsupported`.
- `set_len` with a negative length. Windows refuses it itself, and POSIX
  `ftruncate` returns `EINVAL`. Both give the same `InvalidInput`.
- `refuse InvalidInput` on Windows `open`, `create` and `append`. The truncate
  guard cannot fire for their fixed options.
- macOS `copy` lacks `Unsupported` because it has a creation time (birthtime).
- Narrowing helpers in the macOS record decoders.

To keep the noise down, run the diff against a reviewed baseline and fail only
on new rows.

## Cost per target in CI (estimates)

- Static diff: under 1 s, no build, runs on Linux for all four families.
- Behaviour probes, once they compile: building omega cold took 9.5 min here.
  Each std-importing program took 1–2.5 min to reach its refusal, and the
  post-merge job budgets about 3 min per std program. The current CI has no
  macOS or Windows runner and no arm64 runner. Adding them means a hosted
  macOS runner (about 10× the Linux minute rate), a Windows runner (about 2×) and
  qemu or an arm64 runner. On each one, omega must be built or fetched, then
  roughly 3–5 min of probes run.
