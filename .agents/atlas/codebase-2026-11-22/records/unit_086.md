# Record: unit_086 (area 17-omega-source)

Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Read-only pass; only file reads and greps. Every range of the unit was read in full (21,956 lines, 95 files). Paths are relative to `source/` unless stated. "read" = I read the lines; "grepped" = a Grep with the pattern given; "inferred" = not read at the other site.

The unit has three parts:
- A. `library/std/targets/**` and `library/std/*.omg` target files: UEFI x86_64 tables/entry/handoff, the Windows x86_64 provider closure (calling plans, console, clock, time, filesystem, removal, GUI), and `std/time.omg`.
- B. `subsystems/compiler/{alpha,driver,request}`: Alpha tape emitter, the driver, the OCREQ request decoder.
- C. `subsystems/compiler/source_to_psi/**` (package `psi`): integers, scalar lowering (checked trees to scalar operations), lexer, and the typer (symbol-resolved trees to typed trees).

Crate map not repeated. In `.omg` code a "machine" is a function; `transition X { pat -> target }` is its branching form; nested `machine` blocks are private states. Contract words (`requires`, `ensures`, `crashes`, `terminates`, `reaches`, `blocks`) are as the crate map describes.

---------------------------------------------------------------------------------------------------

## A. library/std (UEFI and Windows targets, time)

### A1. `library/std/targets/uefi_x86_64/integrity.omg` (53 lines)
1. Purpose: checks that a firmware table's bytes carry a correct CRC-32, with the stored CRC field treated as zero (comment `:1-5`). Contents only, not pointer validity.
2. Shape: `read_u32` `:7` (requires `bytes.length >= 4`); `table_crc` `:18` (`terminates by remaining`, byte loop); `table_byte` `:28`; `crc_bits` `:37` with nested `bit` `:45` (reflected polynomial `0xEDB88320`, `:49`).
3. I/O: reads a byte view; returns `u32`. Used by `std/uefi_x86_64_integrity.omg:1-2` (read).
4. Decision: byte positions 16..20 (the CRC field) are fed as zero (`:31-33`). Loop stops at `position < bytes.length && remaining > 0` (`:21`).
5. Shape cases: fixed header layout, CRC field at bytes 16-19.
6. Same job elsewhere: none seen.
7. Unused: `read_u32`, `table_crc` each have one external caller (`uefi_x86_64_integrity.omg`, grepped via reading).
8. Stated limits: `:3-4` "checks contents, not pointer validity, firmware authority or lifetime."
9. Fail-open: none seen.

### A2. `library/std/targets/uefi_x86_64/tables.omg` (137 lines)
1. Purpose: firmware table geometry authored as checked schemas plus layout policies: Loaded Image protocol, EFI table header, system table view, boot services view.
2. Shape: `data EfiLoadedImage` `:21`; `EfiLoadedImageLayout satisfies FixedLayout` `:37` with `plan(schema, target)` `:38`; `EfiTableHeader` `:77`; `EfiSystemTableView` `:85`, layout `:101`; `EfiBootServicesView` `:123` (`services: [u64; 44]`), layout `:128`.
3. I/O: `plan` reads `schema.fields[i].key` and returns a `LayoutPlan` of fixed `FieldPlan::At { offset }` entries.
4. Decision: none; offsets are literals (for example `:42-54` 0,8,...,88; `size_fixed: 96`, `:55`). The `target` argument is not read.
5. Cases by shape: each layout is hard-coded for one declaration (13, 13 and 2 entries); the entries table is `[FieldEntry; 64]` (`:41,105,132`).
6. Same job: the comment (`:3-8`) says the Rust side binds to the evaluated report; the Rust-side catalog was not read (suspected duplicate catalog is stated NOT to exist).
7. Unused: not applicable (declarations).
8. Stated limits: `:74-76` "schema reflection caps a declaration at 32 members, so the block is one fixed `services` array".
9. Fail-open: `plan` indexes `schema.fields[k]` for k up to 12 with no check that the schema has that many fields (`:42-54`); a wrong schema would reach compiler array-bounds handling (inferred). open.

### A3. `library/std/targets/windows_x86_64.omg` (4 lines)
Comment only: names the target definition file shape (`:1-4`). Nothing decided.

### A4. `library/std/targets/windows_x86_64/calling.omg` (176 lines)
1. Purpose: Microsoft x64 calling policy for foreign calls with up to four scalar or sized-reference parameters.
2. Shape: `WindowsX64ScalarCalling satisfies CallingPolicy` `:21` (`plan` `:22`); `windows_x64_scalar_signature` `:34` with states `completion/result_index/result_shape/result_integer/result_place`; `windows_x64_scalar_parameters` `:78` (recursive, `terminates by remaining`); `windows_x64_scalar_argument_register` `:129` (rcx, rdx, r8, r9); `windows_x64_call_entry` `:139`; `windows_x64_register_place` `:168`.
3. I/O: reads a `BoundarySignature` (`parameter_count`, `native_parameter_count`, `shapes`, `callback_*`); produces `BoundaryPlanResult::Accepted { plan }` or `Rejected { reason }`.
4. Decision keyed on shape facts: `parameter_count == native_parameter_count && parameter_count <= 4 && callback_binder_count == 0 && callback_demand_count == 0` (`:36-38`); result class `Integer` with `byte_size` 4 or 8 and `alignment == byte_size` (`:59-65`); parameter class `Integer` (4 or 8 bytes, aligned to size, `:107`) or `Reference` (8 bytes, align 8, `:115`); shape indexes need `< shape_count && < 256` (`:51,90`).
5. Cases by shape: comment `:19-20` "Fixed Microsoft x64 calls with up to four scalar or sized-reference formals. Stack arguments, narrow register scalars and callbacks require a richer policy." Rejection text `:28`: "at most four 32/64-bit integer or sized-reference parameters, Unit or 32/64-bit integer result, and no callbacks".
6. Same job elsewhere: Microsoft x64 clobber list (13 registers: rax, rcx, rdx, r8-r11, xmm0-5) is spelled out in `calling.omg:145-158`, `targets/windows_x86_64/entry.omg:123-149` (read), and a 7-register version without xmm in `std/uefi_x86_64_entry.omg:114-121` (read). The `permitted_transitive_use` flags differ: here only general/vector/flags (`:160-162`); `entry.omg:150-158` adds instruction_pointer, stack_pointer, control_state. Helper `windows_x64_call_entry` is reused by `windows_filesystem_calling.omg:51` (read).
7. Unused/one-caller: `windows_x64_register_place` called by `calling.omg:73,123` and `windows_filesystem_calling.omg:53,64,73`. Not grepped further.
8. Stated limits: `:19-20` above.
9. Fail-open: none seen (every unmatched shape returns false, `:41,61,68,85,101`).

### A5. `library/std/targets/windows_x86_64/clock_impl.omg` (20 lines)
Declares `boundary trait WindowsClockWait { sleep(milliseconds: u32) reaches Clock blocks crashes none }` `:14-20`. Comment `:2-9`: kernel32 `Sleep(0xFFFFFFFF)` is `INFINITE`, so `WindowsX8664Clock::sleep` is the checked adapter. Grepped `WindowsClockWait|WindowsX8664Clock` across `source/`: 14 hits in 5 files (build.omg 2, clock.omg 2, clock_impl.omg 3, time_host_impl.omg 1, windows_x86_64_clock.omg 6). Fail-open: none seen.

### A6. `library/std/targets/windows_x86_64/console_impl.omg` (96 lines)
1. Purpose: Windows console byte I/O over MSVCRT descriptors in binary mode.
2. Shape: `trait WindowsConsoleIo: Calling<WindowsX64ScalarCalling>` `:15` with `write/read/set_input_mode/set_output_mode`; `windows_console_read_byte` `:48`; `windows_console_write_byte` `:80`; `WindowsConsoleBytes satisfies ConsoleByteOperations` `:88`.
3. I/O: stdin descriptor 0 / stdout 1; returns `ByteRead::{Byte, Eof, Error}`.
4. Decision on values: `set_input_mode(0, 32768)` returns `-1` -> Error (`:54-57`); `read` count `1` -> Byte, `0` -> Eof, other -> Error (`:62-66`). Contract `requires descriptor == 1 && count <= 4` and `mode == 32768` (`:17,36,41`).
5. Cases: only descriptors 0/1, mode `_O_BINARY` (0x8000).
6. Same job elsewhere: other targets' console providers live in other units (std `console.omg`; not read).
7. Unused: none checked.
8. Stated limits: none.
9. Fail-open: `windows_console_write_byte` ignores both the `set_output_mode` result and the `write` result (`:84-85` bind `mode` and `written` but never read them). A failed write or a failed binary-mode change is silently dropped; no comment says why. open.

### A7. `library/std/targets/windows_x86_64/entry.omg` (195 lines)
1. Purpose: process-entry calling policy for a Windows hosted application.
2. Shape: `boundary trait WindowsProcessEntry { enter() -> u32 ... }` `:15`; `WindowsX86_64CallingPolicy` `:19`; `windows_x86_64_entry_plan` `:25` with states `physical`, `physical_index`, `physical_shape`, `build_physical`, `semantic`, `semantic_indices`, `semantic_shapes`, `build_semantic`, `finish`, `seal`, `reject`; `windows_x86_64_entry_extent_reference` `:173`; `boundary trait WindowsX86_64Application` `:195`.
3. I/O: `BoundarySignature` in; `BoundaryEntryPlan` out. Calls `|shapes|::hosted_entry_integer_shape`, `hosted_entry_extent_shape`, `hosted_entry_named_word` (module `entry_shapes.omg`, not in this unit; `:2`).
4. Decision keyed on parameter count: 0 -> physical entry (needs `has_result`, result shape a 4-byte integer, `:51`), 2 -> semantic entry (needs no result and both parameters extent-shaped, `:91-92`), anything else -> reject (`:28-32`).
5. Cases: exactly two entry shapes; message `:167` "WindowsX86_64Application requires exact physical or program-storage entry shapes".
6. Same job elsewhere: UEFI equivalent `std/uefi_x86_64_entry.omg:33-229` (below). Clobber/state setup duplicated with A4.
7. Unused: grepped not done.
8. Stated limits: `:10-14` "This calling contract supplies no bridge, loader admission, receiver, or numeric stack guarantee."
9. Fail-open: none seen; unmatched counts and shapes go to `reject()` (`:31,53,72,84,96`).

### A8. `library/std/targets/windows_x86_64/filesystem/copy.omg` (92 lines)
1. Purpose: bounded file copy: read the caller-bounded prefix of the source into a caller buffer, then write it to the destination.
2. Shape: `windows_filesystem_copy(from, to, buffer, cap) -> IoResult` `:21` with states `acquire`, `metadata`, `failed`, `read`, `bounded`, `destination`, `write`, `restore`.
3. I/O: uses `windows_filesystem_open/metadata/read/close/write_complete/set_file_permissions`, `WindowsFilesystemHost::create_file`; returns `IoResult::{Ok{count}, Error{kind}}`.
4. Decision: `cap <= buffer.length` else `InvalidInput` (`:24-27`); `count <= buffer.length` else `Other` (`:56-59`). Destination opened with literal flags `create_file(to, 1073741952, 7, 2, 1048704)` (`:67`; comment `:63-66`: GENERIC_WRITE + FILE_READ_ATTRIBUTES, disposition 2 = create-always).
5. Cases: portable read-only bit only restored (`:13-14`).
6. Same job elsewhere: acquisition-close sequencing repeated in `sequences.omg:77-112`.
7. Unused: none checked.
8. Stated limits: `:10` "Copy the caller-bounded initialized prefix, not an unbounded CopyFileW."
9. Fail-open: none seen; earlier error wins over close error by design (`:12`).

### A9. `library/std/targets/windows_x86_64/filesystem/directory.omg` (65 lines)
Purpose: portable directory operations over raw host calls. `windows_filesystem_read_dir_count` `:19` (names only; comment `:10-11` an unrepresentable name still counts), `read_dir_is_empty` `:26`, `read_dir_nth` `:42`, `windows_filesystem_directory_entry` `:55`.
Decision: `record.name_len > 0 && record.name_len <= 765` else `Error{Other}` (`:56-63`); `is_dir = (attributes & 1040) == 16`, `is_file = (attributes & 1040) == 0` (`:60-61`) (1040 = DIRECTORY 16 + REPARSE_POINT 1024). A reparse entry is neither file nor directory (comment `:39-41`).
Fail-open: none seen.

### A10. `library/std/targets/windows_x86_64/filesystem/errors.omg` (69 lines)
Purpose: Win32 error code to portable `ErrorKind`. `windows_filesystem_error` `:11` (22 numeric codes; `_ -> Other`, `:35`), `windows_filesystem_status_error` `:39` (negated u32 range), `windows_filesystem_unit` `:50`, `windows_filesystem_transferred` `:57`.
Decision keyed on numeric value. Comment `:4-5` "Unknown codes remain Other; neither a zero last-error nor a malformed raw status can manufacture success." Status `0` -> Ok else Error (`:51-54`); transferred count accepted only if `status >= 0 && count <= requested` (`:58-67`).
Fail-open: none seen.

### A11. `library/std/targets/windows_x86_64/filesystem/handles.omg` (255 lines)
1. Purpose: open, read, write, close, sync, clone, set-length for file handles; handle `0` is "absent".
2. Shape: `windows_filesystem_opened` `:22`; `windows_filesystem_open_with` `:34`; `_open/_create/_create_new/_append` `:83,90,98,106`; `windows_filesystem_transfer_size` `:116` (`ensures result <= requested && result <= 4294967295`); `_read` `:125`; `_write` `:145`; `_read_at` `:162`; `_write_at` `:182`; `_close` `:202`; `_sync` `:214`; `_clone` `:226`; `_set_length` `:238`.
3. I/O: `WindowsFilesystemHost` (create_file, read_file, write_file, read_at, write_at, close_handle, flush_file_buffers, duplicate_handle, set_file_length, last_error); returns `OpenResult/IoResult/UnitResult`.
4. Decision (flags by value): `open_options_truncate_conflict` -> `InvalidInput` (`:37`); desired access `2147483648` when `read || (!write && !append)` (`:42-46`); append adds `1179924` (`:49`), write adds `1073741824` (`:55`); disposition `1` create_new, `2` create+truncate, `4` create, `5` truncate-existing, `3` open-existing (`:59-76`); share 7, flags `1048704` (`:78`). `handle != 0` guards every operation, `0` -> `BadDescriptor` (`:128-142`, `:148-150`, ...). Positioned ops need `offset >= 0` (`:170,190`).
5. Cases: five-way option decoding is a fixed table (`:59-76`).
6. Same job elsewhere: nothing in unit.
7. Unused: `windows_filesystem_clone` has one caller, `provider.omg:223` (grepped). It calls `WindowsFilesystemHost::duplicate_handle` without `block` (`:233`), consistent with `duplicate_handle` being declared non-blocking (host `:153`).
8. Stated limits: `:10-16` comment (zero is absent; append omits FILE_WRITE_DATA).
9. Fail-open: none seen.

### A12. `filesystem/locking.omg` (30 lines)
Purpose: portable lock operations over `lock_file/unlock_file`. `windows_filesystem_try_lock_result` `:24`: status `0` -> Acquired, `-33` -> WouldBlock, else `Error`. Comment `:5-7`: only ERROR_LOCK_VIOLATION denotes contention. Fail-open: none seen.

### A13. `filesystem/metadata.omg` (204 lines)
1. Purpose: decode native metadata to portable `Metadata`; timestamps; permissions.
2. Shape: `windows_filesystem_seconds` `:23`; `windows_filesystem_kind` `:28`; `windows_filesystem_metadata_result` `:44`; `_metadata` `:70`; `_metadata_path` `:83`; `_exists` `:90`; `_try_exists` `:97`; `data WindowsTimestampEncoding` `:113`; `windows_filesystem_encode_timestamp` `:124`; `_set_times` `:143`; `_set_permissions` `:172`; `_set_file_permissions` `:186`.
3. Decision: `ticks / 10000000 - 11644473600` (`:24-25`); a reparse point (`attributes & 1024`) is `Symlink` only for tags `2684354572` or `2684354563` (`:29-35`), other tags `Other`; directory flag 16 -> Directory else File. Timestamp encode refuses `seconds == -11644473600` and anything outside `(-11644473600, 1833029933770]` with `Unsupported` (`:125-134`). `Permissions::Posix` -> `Unsupported` (`:178,197`); `Default` -> writable, `Readonly{value}` -> that value.
4. Cases: only symlink and junction reparse tags known (comment `:15-18`).
5. Fail-open: `windows_filesystem_exists` returns `false` for every error, not only not-found (`:94`), the portable `bool` form; no comment. Wrong input: a permission error reads as "does not exist". open (portable `try_exists` at `:97-110` keeps errors).

### A14. `filesystem/paths.omg` (264 lines)
1. Purpose: namespace ops (remove, rename, link, canonicalize, create-dir-all) and Windows root parsing.
2. Shape: `windows_filesystem_extended_path` `:34` (prefix `\\?\`); `_separator` `:44`; `_root_length` `:51`; `_root_components` `:101` (`terminates by budget`); `_create_dir` `:128`; `_remove_dir` `:135`; `_remove` `:142`; `_rename` `:149`; `_existing_directory` `:156`; `_hard_link` `:172`; `_canonicalize` `:179`; `_create_dir_existing` `:186`; `_create_dir_all` `:202`; `_create_components` `:232`.
3. Decision keyed on bytes: drive letter + `:` (`:60-61`), `\\?\UNC\` (case-insensitive `UNC`, `:85-89`), separators (`/` accepted only when the path is not an extended path, `:45`). Create-dir-all treats a root as an existing namespace and requires a followed directory query to turn "exists" into success (`:156-169,210-222`; comment `:9-12`).
4. Cases: three root forms (drive, UNC, extended UNC) plus rooted-single-separator; malformed roots "left to Windows" (comment `:48-50`).
5. Stated limits: `:228-231` "a finite component scan must not publish `terminates` for the whole operation."
6. Fail-open: `windows_filesystem_root_components` returns `path.length` when it runs out of input or budget (`:106`), meaning "the whole path is the root"; for malformed UNC input `create_dir_all` then checks the whole path as an existing directory (`:210`). traced (read): wrong input becomes a metadata query, not an accept.

### A15. `filesystem/provider.omg` (353 lines)
1. Purpose: the complete Windows implementation of `FilesystemService`.
2. Shape: `FilesystemWindowsProvider satisfies FilesystemService` `:26` with 46 members (counted by reading), each forwarding to a `windows_filesystem_*` machine with a `reaches` row.
3. Decision: three members return fixed errors: `symlink` `:169` (comment `:167-168` "Windows creation needs file/directory intent; this portable contract does not carry it ... Do not guess it."), `set_owner` `:227`, `set_owner_no_follow` `:234`, `set_file_owner` `:241` all `Unsupported` (comment `:226`). `sync_data` forwards to `windows_filesystem_sync` `:216` (comment `:211`). `try_clone` calls `windows_filesystem_clone` without `block` `:223`.
4. Same job elsewhere: POSIX provider (`posix_filesystem_adapter.omg`, not in this unit).
5. Fail-open: none seen.

### A16. `filesystem/sequences.omg` (113 lines)
Purpose: acquisition/close sequencing. `windows_filesystem_finish_unit` `:17`, `_finish_read` `:24`, `_write_complete` `:37` (windowed loop of at most 4294967295 bytes; progress must be `> 0` and `<= end - position`, zero -> `WriteZero`, `:56-74`), `_write_all` `:77`, `_read_all` `:94` (`count <= buffer.length` else `InvalidInput`). Comment `:8-10`: earlier error wins over close failure. Fail-open: none seen.

### A17. `windows_x86_64/filesystem_impl.omg` (3 lines)
Comment only.

### A18. `filesystem_removal/acquisition.omg` (149 lines)
1. Purpose: open the root of a recursive removal and open each child relative to its parent handle.
2. Shape: `windows_removal_open_root` `:19` / `windows_removal_acquire_root` `:25`; `windows_removal_component` `:47`; `windows_removal_open_child` `:75` / `windows_removal_acquire_child` `:83`.
3. Decision: wide path conversion `units > 0` else `c_set_last_error(87)`, return 0 (`:31-34`); root opened with `c_create_file(@path, 1114241, 7, 0, 3, 36700160, 0)` (`:42`); child name must be length 1..4095 and not `.`/`..` and contain none of 0, `/`, `\`, `:` (`:47-72`); access `1114241` (directory) or `1114240` by `(entry.attributes & 16) != 0` (`:99-102`); `NtOpenFile` shared 7 options 2113568. An opened handle must be nonzero and not -1 (`:138`), else `corrupt()` sets error 31.
4. Cases: enumeration attributes only select access; actual attributes are queried later (comment `:10-11`).
5. Fail-open: none seen.

### A19. `filesystem_removal/attributes.omg` (35 lines)
`windows_removal_attributes` `:17` classifies the handle as a disk file, then queries attribute-tag class 9 size 8 (`:26`); returns `Error{code}` or `Value{attributes}`. Fail-open: none seen.

### A20. `filesystem_removal/completion.omg` (55 lines)
`windows_removal_error` `:11` (zero -> 31), `_status` `:15`, `_close` `:25` (closes once; original error wins, else close result), `_dispose` `:42` (sets delete disposition class 4 size 1, then close). Comment `:3-6`: first error wins; CloseHandle never retried. Fail-open: none seen.

### A21. `filesystem_removal/native.omg` (55 lines)
`data WindowsNativeUnicodeString` `:16` (`requires length > 0 && length <= 8190 && length % 2 == 0 && capacity == 8192`), `WindowsNativeObjectAttributes` `:24` (`requires length == 48 && root != 0 && attributes == 4096 && security == 0 && quality == 0`), `trait WindowsFilesystemRemovalIo` `:38` with `open/dispose/close` and exact `requires` (`access in {1114240,1114241}`, `sharing == 7`, `options == 2113568`, `information_class == 4`).

### A22. `filesystem_removal/removal.omg` (48 lines)
`windows_remove_tree` `:16`: open root, query attributes; directory and not reparse (`(attributes & 16) != 0 && (attributes & 1024) == 0`, `:33`) -> walk, otherwise refuse with code 267 (`:35`). Comment `:4-8`: reparse root refused; child reparse objects deleted as leaves.

### A23. `filesystem_removal/traversal.omg` (179 lines)
1. Purpose: depth-first deletion using a stack of retained directory handles.
2. Shape: `data WindowsRemovalFrame [copy]` `:18`; `windows_removal_walk` `:23` (frames `[WindowsRemovalFrame; 4096]`; states `next/query/name/extent/open/attributes/kind/capacity/push/leaf/pop/parent`); `windows_removal_unwind` `:147`.
3. Decision: depth >= 4096 -> `failed_child(..., 8)` (`:110-113`); child extent must be 1..4095 else code 122 (`:65-68`); directory child (`attr & 16`, not reparse) is pushed, otherwise disposed as a leaf (`:102-105`); `.`/`..` skipped via `windows_directory_entry_dot` (`:57`).
4. Stated limits: `:10-12` "No work budget silently cuts off wide directories. The finite handle stack fails explicitly with ERROR_NOT_ENOUGH_MEMORY."
5. Fail-open: none seen (invalid frame handle -> unwind with 31, `:39-41`).

### A24. `windows_x86_64/time_host_impl.omg` (56 lines)
`boundary trait WindowsTimeRead` `:42` with `query_performance_counter`, `query_performance_frequency`, `system_time_as_file_time`, `sleep` (each `reaches TimeHost`). Stated limit `:32-34` "NATIVE EXECUTION IS UNVERIFIED. This host is macOS arm64 and cannot execute a windows_x86_64 product." Line 36 imports `narrow_i64_to_u64_saturating`, which this file does not use (read; it is used in `windows_x86_64_time_host.omg`). Fail-open: none seen.

### A25. `windows_filesystem_calling.omg` (88 lines)
`windows_filesystem_transfer_signature` `:16` and `windows_filesystem_transfer_entry` `:50`. Exactly four parameters and a result; shapes: handle integer 8, bytes reference 16 (align 8), transferred reference 8, overlapped integer 8, result integer 4 (`:39-43`). Plan: handle rcx, bytes in rdx+r8 (16 bytes in two register locations, `:57-63`), count pointer r9, overlapped at stack offset 32. Comment `:9-14` "This plan supplies representation, never storage authority or completion." Fail-open: none seen (all else returns false).

### A26. `windows_filesystem_control.omg` (225 lines)
1. Purpose: synchronous native handle/path/timestamp/namespace operations; each reads `GetLastError` right after its call.
2. Shape: `windows_filesystem_native_failure_i32/_i64` `:20,28`; `..._normalize_handle` `:36`; `..._create_file` `:54`; `..._close_handle` `:66`; `..._last_error` `:80`; `..._flush_file_buffers` `:85`; `..._set_file_pointer` `:90`; `..._duplicate_handle` `:97`; `..._move_file` `:105`; `..._rename_file` `:121`; `..._hard_link` `:136`; `..._set_file_time` `:156`; `..._get_file_time` `:167`; `..._set_file_attributes` `:182`; `..._create_directory` `:193`; `..._remove_directory` `:204`; `..._delete_file` `:215`.
3. Decision: a handle is invalid only if `0` or `-1` (`:37`); `create_file` refuses `FILE_FLAG_OVERLAPPED` (`flags & 1073741824`) with error 87 (`:60-63`); path converts with `windows_wide_path`, `count > 0` else status `-87`; zero or oversized native error becomes `-31` (`:23-26`); `duplicate_handle` uses options `2` (`:102`).
4. Same job elsewhere (inside unit): the same 5-line preamble (`let mutable wide: [u16; 4096]; windows_wide_path(...); wide[4095] = 0; ... as +WindowsNativePath`) appears at 11 call sites in this file and 6 more in `acquisition.omg:29`, `file_control.omg:50`, `metadata.omg:36`, `path_query.omg:15`, `reparse.omg:31` (grepped `windows_wide_path\(`: 17 matches, one is the definition in `windows_filesystem_paths.omg:17`).
5. Unused: host members `get_file_time`, `set_file_pointer`, `set_file_attributes`, `move_file` (host trait `windows_filesystem_host.omg:154,151,159,187`) have 0 call sites of the form `WindowsFilesystemHost::<name>` in `source/` (grepped `WindowsFilesystemHost::(get_file_time|set_file_pointer|set_file_attributes|move_file)`: 0). The native bodies `windows_filesystem_native_set_file_pointer` is called by `windows_filesystem_position.omg:41,86` (read).
6. Fail-open: none seen.

### A27. `windows_filesystem_directory.omg` (168 lines)
1. Purpose: names-only directory enumeration over a private synchronous directory handle.
2. Shape: `data WindowsDirectoryStep` `:18`; `windows_directory_open` `:28` / `_acquire` `:34`; `windows_directory_next` `:77`; `windows_directory_decode` `:104`; `windows_directory_dot` `:120`; `windows_filesystem_native_directory_count` `:130`.
3. Decision: open with `create_file(path, 1048577, 7, 3, 34603008)` then query file-mode class 16 and require `(mode & 32) != 0 && returned == 4` else close and set error 87 (`:39-59`); end of listing on status `-2147483642` or `-1073741809` (`:93`); a record is valid only if `returned` in 12..=8208 and `(control & 0xFFFFFFFF) == 0 && bytes > 0 && bytes <= 8192 && bytes % 2 == 0 && bytes <= returned - 12` (`:105-115`), else error 122 / 31. Count increments up to i64 max, else `-534` (`:156-159`).
4. Same job elsewhere: `windows_directory_entry_dot` in `windows_filesystem_directory_entry.omg:114` is the same test over a different record type; `windows_directory_decode_entry` `:98` there mirrors `windows_directory_decode` here with 64 instead of 12.
5. Stated limits: `:5-6` "Directory changes may be observed during enumeration; no snapshot is promised."
6. Fail-open: none seen.

### A28. `windows_filesystem_directory_entry.omg` (172 lines)
`windows_filesystem_native_directory_nth` `:18` (open, select ordinal, close; each call re-opens and re-enumerates from the start, `:20,44-75`), `windows_directory_entry_error` `:36`, `windows_directory_select` `:44`, `windows_directory_next_entry` `:77`, `windows_directory_decode_entry` `:98` (returned 64..=8256), `windows_directory_entry_dot` `:114`, `windows_directory_encode_entry` `:124` (characters 1..=255, then `WindowsFilesystemUtf8Io::utf8::<4096, 765>(65001, 128, ...)`, `written > 0`, `count <= 765`), `windows_directory_copy_entry` `:150`, `windows_directory_copy_name` `:159`. Decision: a name over 255 UTF-16 units gives code 31 (`:125-128`). Fail-open: none seen.

### A29. `windows_filesystem_directory_native.omg` (52 lines)
Structs `WindowsNativeDirectoryNameInfo` `:15`, `WindowsNativeDirectoryEntryInfo` `:24`; `trait WindowsFilesystemDirectoryIo` `:37` with `next_entry` (capacity 8256, class 1) and `next_name` (capacity 8208, class 12) and exact `requires`.

### A30. `windows_filesystem_file_control.omg` (98 lines)
`windows_filesystem_native_set_file_length` `:16` (duplicate handle, end-of-file class 6 size 8, close), `_set_handle_readonly` `:37`, `_set_path_readonly` `:47` (open attributes with access 384), `windows_set_readonly_owned` `:62`, `windows_update_readonly` `:69`. Decision: read-only sets bit 1 and clears bit 128 (`& 4294967167 | 1`), writable clears bit 1 and uses `128` (normal) if no other attribute remains (`:83-89`). Comment `:3-6`: not atomic against another writer. Fail-open: none seen.

### A31. `windows_filesystem_file_control_native.omg` (26 lines)
`WindowsNativeFileEndOfFileInfo` `:10`; `trait WindowsFilesystemFileControlIo` `:12` (`end_of_file` class 6 size 8; `basic` class 0 size 40; `open_attributes` access 384).

### A32. `windows_filesystem_impl.omg` (113 lines)
`NativeWindowsFilesystem satisfies WindowsFilesystemHost` `:22` forwarding 31 members (counted) to the native machines, each with its own `reaches` row (`remove_tree` -> `windows_remove_tree` from `filesystem_removal`, `:6`). Comment `:19-21`: forwarding preserves operation capability rows.

### A33. `windows_filesystem_lock.omg` (60 lines)
`windows_filesystem_native_lock_file` `:10`, `_unlock_file` `:41`, `windows_lock_completion` `:56`. Decision: flags `2` exclusive blocking, `3` exclusive try, `0` shared, `1` shared try (`:26,30`); whole-file range length `0xFFFFFFFF_FFFFFFFF` (`:35,50`); duplicate handle first (`windows_synchronous_duplicate`), closed after. Comment `:1-7`: duplicate, never ReOpenFile; locks do not protect memory-mapped access. Fail-open: none seen.

### A34. `windows_filesystem_lock_native.omg` (34 lines)
`WindowsNativeLockRequest` `:11` (OVERLAPPED layout); `trait WindowsFilesystemLockIo` `:19` (`reaches none`, exact `requires` including zeroed request and whole-file length).

### A35. `windows_filesystem_metadata.omg` (176 lines)
`windows_filesystem_native_metadata` `:21` (duplicate handle, query, close), `_read_metadata` `:33` (open with flags `36700160` no-follow or `34603008`), `windows_metadata_owned_handle` `:62`, `windows_classify_metadata_disk_file` `:86` and `windows_metadata_disk_file` `:94` (file type `1` = disk accepted; `0` consults last error; any other type refused with 50), `windows_query_metadata` `:137` (basic 40 bytes, standard 24, attribute-tag 8). Comment `:82-85`: GetFileInformationByHandleEx excludes pipes, so classification precedes the query. The established domain `WindowsDiskFileOrAbsent` has issuer `windows_classify_metadata_disk_file` (declared `metadata_native.omg:16-18`). Fail-open: none seen (0 means failed classifier).

### A36. `windows_filesystem_metadata_native.omg` (57 lines)
`domain i64::WindowsDiskFileOrAbsent requires self != -1 established by windows_classify_metadata_disk_file` `:16-18`; structs `WindowsNativeFileBasicInfo/StandardInfo/AttributeTagInfo` `:20-36`; `trait WindowsFilesystemMetadataIo` `:38` with exact class/size `requires`.

### A37. `windows_filesystem_native.omg` (75 lines)
`domain [u16; 4096]::WindowsNativePath requires self[4095] == 0` `:12`; `WindowsFilesystemIoStatus` `:14`; `trait WindowsFilesystemIo` `:19` with 20 `c_*` leaves (`c_rename_file` requires `flags == 1`, comment `:38-39`).

### A38. `windows_filesystem_path_query.omg` (90 lines)
`windows_filesystem_native_canonicalize` `:12` (open attributes, final path, close), `windows_canonical_query` `:32`, `windows_copy_canonical_bytes` `:72`. Decision: `characters < 4096` else `-122` (`:44-47`; comment `:42-43` too-small result includes the NUL, a success excludes it); output must hold the UTF-8 bytes including NUL (`count <= output.length` else `-122`, `:63-66`); terminator checked before writing (`source[count - 1] == 0` else `-31`, `:79`). Fail-open: none seen.

### A39. `windows_filesystem_path_query_native.omg` (28 lines)
`trait WindowsFilesystemPathIo` `:11` (`final_path`) and generic `trait WindowsFilesystemUtf8Io` `:21` (`utf8<const Units, const Bytes>`, `requires code_page == 65001 && flags == 128 ...`).

### A40. `windows_filesystem_paths.omg` (169 lines)
Purpose: UTF-8 path to UTF-16 with terminator. `windows_wide_path` `:17`; `windows_path_encode` `:29` (`terminates by remaining`); `windows_path_step` `:45`; `windows_path_character` `:97`. Decision: NUL (`first > 0`), overlong (lead bytes 194..244 only), surrogates (`point >= 2048 && (point < 55296 || point > 57343)`, `:145`), and `point` outside 65536..=1114111 for four-byte (`:158`) all return `valid: false`; returns the unit count or `-1` (`:38-41`). Comment `:95-96`.
Same job elsewhere: UTF-8 decoding is also in `source_to_psi/.../lexer.omg:72-307` (different structure; read both, not shared).
Fail-open: none seen.

### A41. `windows_filesystem_position.omg` (97 lines)
`windows_filesystem_native_read_at` `:9`, `_write_at` `:54`. Decision: needs `offset >= 0 && count <= buffer.length && count <= 4294967295` (read) / `bytes.length <= 4294967295` (write); pins the handle, queries the granted access mask, and refuses with `-5` unless `(granted & 1048577) == 1048577` (read) or `(granted & 1048578) == 1048578` (write) (`:23,68`); then reopens, sets the file pointer and checks `positioned == offset`. Comment `:1-4`: an append-only handle cannot gain FILE_WRITE_DATA. Fail-open: none seen.

### A42. `windows_filesystem_position_native.omg` (35 lines)
`WindowsNativeObjectBasicInfo` `:13` (56 bytes); `trait WindowsFilesystemPositionIo` `:21` (`object_access` class 0 size 56; `reopen_read` access 1048577; `reopen_write` access 1048578).

### A43. `windows_filesystem_position_pins.omg` (77 lines)
`data WindowsPinnedAccessResult` `:7`, `WindowsPositionedFile` `:11`; `windows_positioned_error` `:16`; `windows_positioned_pin` `:20` (duplicate, NtQueryObject, `returned == 56` else refuse 31); `windows_positioned_reopened` `:52`; `windows_positioned_seek_failed` `:74`. Comment `:3`: "Zero-initialized outcomes fail closed".

### A44. `windows_filesystem_private_handles.omg` (13 lines)
`domain i64::WindowsPrivateNonalertFileOrAbsent requires self != -1 established by windows_directory_open, windows_open_reparse, windows_removal_open_root, windows_removal_open_child` `:10-13`.

### A45. `windows_filesystem_reparse.omg` (110 lines)
`windows_open_reparse` `:14`, `windows_filesystem_native_read_link` `:26`, `windows_reparse_query` `:53`, `windows_reparse_copy_bytes` `:90`. Decision: `count > 0 && count <= output.length` else `-87`; query control `589992` into a 16384-byte record; target decoded, `units > 0 && units < 8192`; UTF-8 encode to 24576 bytes; copies `min(length, count)`. Fail-open: none seen.

### A46. `windows_filesystem_reparse_data.omg` (104 lines)
`windows_reparse_target` `:14`. Decision by tag: `2684354572` symbolic link, `2684354563` mount point, other `-50`; symbolic flags must be `<= 1`, else `-50`; absolute NT `\??\` paths become `\\?\`, any other absolute namespace `-50` (`:94-98`); relative links retain spelling; embedded NUL `-31`. Comment `:1-6`.

### A47. `windows_filesystem_reparse_native.omg` (31 lines)
`WindowsNativeReparseData` `:12`; `trait WindowsFilesystemReparseIo` `:23` (`control == 589992`, capacity 16384).

### A48. `windows_filesystem_transfer.omg` (170 lines)
1. Purpose: read/write through a pinned, synchronous duplicate of the caller's handle.
2. Shape: `windows_synchronous_duplicate` `:26` (the issuer of domain `WindowsSynchronousFileOrAbsent`); `windows_query_synchronous_duplicate` `:31`; `windows_filesystem_native_write_file` `:76`, `_read_file` `:94`; `windows_transfer_completion` `:117`; `windows_write_pinned` `:124`; `windows_read_pinned` `:149`; `windows_read_window` `:162`.
3. Decision: duplicate handle must have `(mode & 48) != 0` (FILE_SYNCHRONOUS_IO_ALERT or NONALERT) (`:53-56`), else closed and error 87; byte counts `<= 4294967295`.
4. Observed difference: this accepts ALERT or NONALERT (`mode & 48`) while `windows_directory_acquire` requires NONALERT only (`windows_filesystem_directory.omg:57`, `mode & 32`).
5. Stated limits: `:22-25` "The OS meaning of FILE_SYNCHRONOUS_IO_* is a reviewed native premise, not an arithmetic theorem"; `:11-14`.
6. Fail-open: none seen.

### A49. `windows_filesystem_transfer_native.omg` (28 lines)
`domain i64::WindowsSynchronousFileOrAbsent requires self != -1 established by windows_synchronous_duplicate` `:13-15`; `trait WindowsFilesystemTransferIo: Calling<WindowsFilesystemTransferCalling>` `:17`.

### A50. `windows_gui_impl.omg` (307 lines)
1. Purpose: user32 window and message adapters and the explicit `NativeWindowsGui` provider.
2. Shape: `trait WindowsGuiIo` `:19` (12 leaves); `windows_gui_native_key_state` `:95`, `_window_create` `:104`, `_foreground_window` `:133`, `_is_window` `:138`, `_window_destroy` `:144`, `_pump_message` `:156`, `_get_dc/_release_dc/_client_rectangle/_stretch_dibits` `:176-200`, `_window_create_client` `:209`; `NativeWindowsGui satisfies WindowsGuiHost` `:235`.
3. Decision: window creation needs non-empty class and title, each ending in a NUL byte, and both fit a 4096-byte terminated copy (`posix_terminated_path`, `:121-126`); otherwise returns handle `0`. Client sizing calls `adjust_rectangle` then needs `adjusted != 0 && width > 0 && height > 0` and outer sizes in `1..=0x7fff_ffff` (`:222-226`). Key state is `(raw & 32768) != 0` (`:101`).
4. Same job elsewhere: the identical 17-service `reaches` list is typed out at `:23-29, 38-44, 47-53, 59-65, 105-111, ...` and again in the trait `windows_gui_host.omg` (A55) and `NativeWindowsGui` (`:238-301`).
5. Fail-open: `windows_gui_native_pump_message` binds `translated` and `result` and ignores both (`:168-169`); a failed translate or dispatch is not reported (no comment). open.

### A51. `library/std/time.omg` (1068 lines)
1. Purpose: pure `Duration`, `Instant`, `SystemTime` values with checked and saturating arithmetic, plus the clock-reading machines over `TimeHost`.
2. Shape: types `Ordering` `:38`, `Duration requires subsecond_nanoseconds <= 999_999_999` `:44`, `DurationResult` `:49`, `TotalResult` `:58`, `Instant` `:472`, `InstantResult` `:477`, `SystemTime` `:661`, `SystemTimeDifference` `:679`, `SystemTimeResult` `:767`; constructors `:70-108`; `checked_as_scaled_total` `:127`; `duration_checked_add` `:181`, `_subtract` `:239`, `_multiply` `:289`, `_divide` `:354`, saturating forms `:392,404`, compare `:414`; `instant_*` `:491-656`; `system_time_*` `:687-907`; `nonzero_clock_frequency` `:915`; `instant_now` `:930`; `system_time_now` `:956`; `elapsed_since` `:998`; `sleep_for` `:1056`.
3. I/O: values in, values out; clock machines call `TimeHost::monotonic_ticks/_per_second/wall_clock_raw/_units_per_second/_epoch_offset_seconds/sleep` (`reaches TimeHost`).
4. Decision keyed on numeric comparisons: overflow found by the wrapped-compare idiom (`sum < a`, comment `:22-24`); signed overflow in biased space (`1 << 63`, `:772-781`); division by zero is the `Overflow` result (`:351-352`).
5. Cases by shape: the carry/borrow idiom is written out per type: add at `:181`, `:491`, `:779`; subtract/difference at `:239`, `:534`, `:577`, `:697`, `:831`, `:998` (inline in `elapsed_since`); tick normalization at `:930`, `:956`, `:998`.
6. Same job elsewhere: Windows `Sleep` splitting of `0xFFFFFFFF` is in `windows_x86_64_clock.omg` and `windows_x86_64_time_host.omg` (A57, A60).
7. Unused: not grepped per machine.
8. Stated limits: `:913` "A host that reports zero degrades to one tick per second"; `:949-955` and `:972` the `% 9223372036854775807` is "only tames the prover". Comments cite `TASKS_TIME.md` (`:2,56,57,678`); a Glob for `TASKS_TIME.md` finds no such file in this checkout.
9. Fail-open: `nonzero_clock_frequency` returns `1` for a zero frequency (`:915-921`, stated `:913`); `system_time_now` returns `UNIX_EPOCH` when the host clock is before 1970 (`:975-985`, stated `:955`); `elapsed_since` returns `ZERO` when `start` is ahead of now (`:1045-1047`, stated); `instant_now` saturates the scaled remainder (`:938-940`, stated). All traced to stated "degrade, never fault"; a wrong host reading yields a plausible time rather than a refusal.

### A52. `library/std/time_host.omg` (86 lines)
`public boundary trait TimeHost` `:44` with `monotonic_ticks`, `monotonic_ticks_per_second`, `wall_clock_raw`, `wall_clock_units_per_second`, `wall_clock_epoch_offset_seconds`, `sleep`; each lists the per-target raw traits it `invokes`. Stated limit `:34-36`: "uefi_x86_64 has no Console or ProcessExit provider either and keeps its settlement gap; it is not a hosted target."

### A53. `library/std/uefi_x86_64_entry.omg` (232 lines)
1. Purpose: UEFI x86-64 physical entry contract and calling policy.
2. Shape: `public data EfiImageHandle` `:13`, `public boundary data EfiSystemTable` `:17`, `EfiStatus` `:19`; `trait UefiPhysicalEntry` `:23`; `UefiX86_64CallingPolicy` `:33`; `uefi_x86_64_plan` `:39` with `select_surface`, `accept_semantic`, `build`, `accept_physical`, `build_physical`, `reject`; `trait UefiApplication` `:231`.
3. Decision: `parameter_count == 2` else reject (`:44-47`); `has_result` picks physical (image handle, system table, status) vs semantic (image, initial storage) (`:50-62`); only index `< 256` is checked (`:70,147`).
4. Cases: exactly two parameters. Message `:225` "UefiApplication requires two program-storage roots".
5. Same job elsewhere: Windows `entry.omg` (A7). Differences: A7 checks `index < signature.shape_count` and shape class/size through `hosted_entry_*_shape` (`entry.omg:44,51,79-92`); this file checks only `< 256`, copies `byte_size` and `alignment` from `signature.shapes[...]` without testing them (`:87-88,96-97,165-166,186-187`), and lists 7 clobbers (`:114`).
6. Fail-open: a two-parameter UEFI signature of any shapes yields an `Accepted` plan with whatever sizes the signature carried; an index at or above `shape_count` but under 256 reads a default shape. No comment says why. Whether the Rust caller validates shapes before calling was not read. open.

### A54. `library/std/uefi_x86_64_handoff.omg` (365 lines)
1. Purpose: the authored boot-services handoff cycle (get memory map, grow buffer, exit boot services, retry on stale key).
2. Shape: constants `:10-26` (`EFI_*`, `UEFI_OS_HANDOFF_ATTEMPT_BOUND = 4`); `trait UefiBootServices` `:34`; `data EfiBootServicesTable` `:62`; `trait UefiOsHandoffTermination` `:104`; `UefiOsHandoffLegs<'table>` `:120`; `UefiOsHandoffCycle<'table>` `:158`; `UefiOsHandoffDisposition` `:167`; `uefi_os_handoff_cycle_complete_boot_services` `:171`; `uefi_os_handoff_cycle_run` `:347`; terminal machines `:137,143`.
3. I/O: leg machines (`uefi_os_handoff_cycle_acquire/_attempt_exit/_grow/_free_map/_transfer/_firmware_return`) and `valid_map_geometry` are imported from `targets\uefi_x86_64\` and are defined in `targets/uefi_x86_64/handoff.omg` (found by grep at `:88,95,123,130`; not in this unit).
4. Decision (status values): success -> validate map geometry -> exit; `BUFFER_TOO_SMALL` with `map_size > map_capacity` -> grow; invalid geometry -> `INVALID_PARAMETER`; exit rejected with `INVALID_PARAMETER` and `remaining > 1` -> retry, else exhaustion status; any other status -> rejected with that status (`:197-337`). `check_growth` needs `map_size < 0xFFFFFFFFFFFFF000` (`:236`). `started` guards re-entry (`:177-180`).
5. Cases: bound of 4 stale-key attempts (`:25`).
6. Stated limits: `:31-33` "No package binds these requirements yet; the Build-owned table binding ... is pending"; `:100-103` the termination edges are "compiler-supplied primitives ... the compiler's catalog has no entry for them yet"; `:154-157` "This is a lowering limitation".
7. Fail-open: `Ready` is returned only after `legs.status == EFI_SUCCESS` following the exit leg (`:297-298`); the status is written by the leg machines I did not read. open for leg correctness.

### A55. `library/std/uefi_x86_64_integrity.omg` (40 lines)
`public machine validate_table(bytes, signature, minimum_revision, minimum_size) -> bool` `:3` (`terminates`). Accepts only if `bytes.length >= 24`, assembled signature equals the argument, `revision >= minimum_revision`, `reserved == 0`, `header_size >= 24 && >= minimum_size && <= bytes.length`, and `table_crc(...) == expected_crc` (`:26-38`). Fail-open: none seen.

### A56. `library/std/windows_filesystem_host.omg` (193 lines)
`public boundary trait WindowsFilesystemHost` `:74` with 31 members (counted); data `WindowsFileTimeResult` `:32`, `WindowsFileMetadata` `:41`, `WindowsMetadataResult` `:51`, `WindowsDirectoryEntry` `:58` (name `[u8; 765]`), `WindowsDirectoryEntryResult` `:63`. Stated limits: `:157-158` "Flags include MOVEFILE_COPY_ALLOWED: cross-volume moves can copy data and attributes before deleting, so this raw ceiling is not rename-only." `:72-73` empty reach on CloseHandle proves no release classification.

### A57. `library/std/windows_gui_host.omg` (120 lines)
`data WindowsMessage { words: [u64; 6] }` `:26`; `WindowsRectangle` `:27`; accessors `windows_message_window/_kind/_word_parameter` `:31-39`; `public boundary trait WindowsGuiHost` `:41` (12 members each listing its `reaches`).

### A58. `library/std/windows_x86_64_clock.omg` (25 lines)
`public WindowsX8664Clock satisfies Clock` `:4`; `sleep` splits `milliseconds == 4294967295` into `2147483647 + 2147483648` (`:10-23`). Same job: `windows_x86_64_time_host.omg:64-83` has the identical split, over `WindowsTimeRead::sleep` rather than `WindowsClockWait::sleep`.

### A59. `library/std/windows_x86_64_filesystem_calling.omg` (13 lines)
`public WindowsFilesystemTransferCalling satisfies CallingPolicy` `:3` using `windows_filesystem_transfer_signature/_entry`; rejection text `:9`.

### A60. `library/std/windows_x86_64_time_host.omg` (84 lines)
`public WindowsTimeHost satisfies TimeHost` `:4`. `monotonic_ticks` returns `0` when `query_performance_counter` returns 0 (`:11-19`, comment `:5`), `narrow_i64_to_u64_saturating` of the counter (negative -> 0); `monotonic_ticks_per_second` same; `wall_clock_raw` `:38`; units `10000000` `:52`; epoch offset `11644473600` `:58`; `sleep` splits the maximum `:64-83`. Fail-open: a zero tick/frequency becomes 0, which `time.omg` degrades to one tick per second (see A51; traced).

---------------------------------------------------------------------------------------------------

## B. subsystems/compiler: alpha, driver, request

### B1. `alpha/build.omg` (6 lines)
`Build::package(builder, "compiler_alpha")` and one path dependency `../source_to_psi` `:3-5`.

### B2. `alpha/emit.omg` (701 lines)
1. Purpose: writes a `ScalarProgram` as Alpha tape bytes in two passes (layout, then write) so forward calls and branches need no patch table (comment `:15-19`).
2. Shape: `public data AlphaEmitter` `:20` (`bytes [u8; 2097152]`, `labels [u64; 10240]`, `operation_labels [u64; 16384]`, `marks [u64; 65536]`, `mark_count`, `trap`, `layout`, `complete`); byte/word/register writers `:34-96`; `alpha_call` `:97`; slot helpers `:121-138`; marks `:140-217`; `alpha_load_width` `:229`; `alpha_store_width` `:274`; `alpha_copy_bytes` `:307`; `alpha_zero_bytes` `:325`; `alpha_equal` `:342`; `alpha_unsigned_less` `:357`; `public alpha_emit` `:382`; `alpha_pass` `:409`; `alpha_parameters` `:492`; `alpha_call_arguments` `:527`; `alpha_return` `:553`; `alpha_keeps_value` `:563`; `alpha_operation` `:586`.
3. I/O: reads `ScalarProgram` via `scalar_machine_at/scalar_operation/scalar_parameter_at/scalar_layout_at/scalar_argument_at`; mutates `AlphaEmitter`.
4. Decision keyed on `ScalarOperationKind` and `ScalarLayoutKind`. `alpha_emit` runs only when `program.status == ScalarStatus::Complete` (`:388-391`). A full tape, too many marks or too many operations sets `emitter.complete = false`, not a crash (`:61,152,489`). Load/store width must be 1, 2, 4 or 8 else `crash Trap` (`:232-238, 277-283`). Frame size is preflighted against `67108864` and a trap address (`:438-444`). `alpha_call` in the write pass needs `callee < 10240` else Trap (`:105-114`).
5. Cases by shape: `alpha_operation` handles IntegerConstant, LowBytes, Call, FrameAddress, AddressOffset, AddressIndex, Load, Store, Copy, Zero, BooleanNot, BranchZero/NonZero, ReturnValue, ReturnUnit, Label, Trap (`:589-607`); other kinds go to `operands`, which handles ExactIntegerAdd/Subtract/Multiply/Divide/Remainder, IntegerEqual, UnsignedLess and sends the rest to `invalid()` = `crash Trap` (`:684-693`). Parameters: Record/Array -> copy, Unit -> skip, other -> store by width (`:509-514`).
6. Same job elsewhere: the lowerer's kind list (A-side C) in `scalar_*`; layout widths mirror `layout.omg`.
7. Unused: `alpha_emit` has 1 caller in `source/` (`driver/compiler.omg:182`) and none in `source-rust/`, `tools/` (grepped `compilation_finish|compilation_add_unit|alpha_emit` outside `source/`, `wiki/`, `.agents/`: hits only under `_recycling/` and `bootstrap/gamma/.../alpha_backend.gamma`).
8. Stated limits: `:412-417` tape at most 2 MiB; activation frames in [32,64) MiB; initial stack 256 MiB.
9. Fail-open: emission performs no overflow, bounds or divide-by-zero checks for `ExactInteger*`, `AddressIndex` or `Load`; the operation names (`ExactIntegerAdd`) and comments (`:271-273`, `:691-692` in lowerer) rely on the checker having proved them. inferred (checker not read); the emitter trusts any program with `status == Complete`.

### B3. `driver/build.omg` (9 lines)
Package `omega_compiler`; path dependencies `../source_to_psi` and `../alpha`.

### B4. `driver/compiler.omg` (196 lines)
1. Purpose: collect units, lex and parse each as it arrives, then on finish select the entry machine, resolve, type, check, lower and emit.
2. Shape: `CompilationStatus` `:30` (Collecting, Complete, Capacity, Lexical, Parsing, Resolution, Typing, Checking, Unsupported, Entry); `Compilation` `:43`; `compilation_clear` `:51`; `compilation_add_unit` `:62`; `compilation_finish` `:112` with `find/candidate/choose/finish_selection/resolve_program/type_program/check/lower/emit`.
3. I/O: consumes `Lexer`, `ModulePath`, package and unit ids, `entry_unit`, `entry_name`; produces `Compilation` with `output: AlphaEmitter` and `diagnostic: SourceSpan`.
4. Decision: unit identity must equal the next unit index (`lexer.source.id.value == compilation.sources.unit_count`, `:70`) else `Entry`; an entry is a machine with `item.parent == 0`, source unit equal to `entry_unit`, and name span spelling equal to `entry_name` (`span_spells`, `:127-129`) (source spelling); a second match gives `Entry`, no match gives `Entry` (`:136-146`). Lowering `Complete` -> emit; `Capacity` -> Capacity; anything else -> Unsupported (`:175-179`). A failure is sticky (comment `:59-61`).
5. Cases: one top-level machine by unit and spelled name.
6. Unused/single: comment `:26-29` "Both the construction application and diagnostic callers use this single semantic pipeline." Grepped: `compilation_add_unit` has 1 external caller (`application/construction/main.omg:513`) and `compilation_finish` 1 (`main.omg:574`); no other callers in `source/`, `source-rust/` or `tools/`.
7. Fail-open: none seen (every `accepted == false` maps to a failing status).

### B5. `driver/slot_review.omg` (15 lines)
`public data OmegaSlotReview {}` `:5`; `prepare` `:7` (empty body); `admits(review, checks)` `:9` builds a fresh `SlotReview`, calls `psi\...\prepare` and `admits` (`:12-14`), so the `review` argument is not read. Unused: grepped `OmegaSlotReview` in `source/`: 3 hits, all in this file; in `source-rust/`: 0.

### B6. `request/build.omg` (6 lines)
Package `compiler_request`; one dependency, `../../../foundation/wire`.

### B7. `request/outcome.omg` (643 lines)
1. Purpose: typed tables for OCOUT outcomes: tags, coordinate spaces, 97 Reject codes, 35 Provisions with limits, and the frame byte writer.
2. Shape: `OutcomeTag` `:11`, `CoordinateSpace` `:19`, `RejectCode` `:29`, `Provision` `:132`, `Outcome` `:172`; `malformed_request` `:184`; `outcome_incomplete` `:200`; `coverage_request_semantics` `:219`; `reject_code_number` `:223` (1..97); `reject_code_space` `:329`; `provision_number/_limit/_space` `:349,393,437`; `frame_length` `:482`; `frame_byte` `:497`; `published` `:578` (`min(quantity, 2147483647)`); `outcome_tag_value` `:594`; `byte_of` `:621`.
3. Decision: each table is a match over the enum; spaces: MalformedRequest -> Request, MissingEntry -> None, others CanonicalSource (`:329-334`); Provision spaces `:437-457` (`_ -> canonical_source`); frame length 48 for CanonicalSource else 40 (`:482-486`). Frame byte layout comment `:493-496`.
4. Same job elsewhere: Rust-side OCOUT encoder (suspected; not read). Comment `:8-10` says wire numbers are "embedded rather than read from any sidecar".
5. Unused: grepped `reject_code_space|reject_code_number|CoordinateSpace::|frame_length\(|outcome_tag_value|outcome_incomplete|malformed_request\(` in `source/`: 30 hits (ocreq/main.omg 3, outcome.omg 24, protocol.omg 3). `reject_code_space` has only its definition (grepped, 0 callers). `reject_code_number` is called only from `malformed_request` (`:185`); the other 96 reject codes are never constructed in this unit.
6. Stated limits: `:216-218` semantic phases not implemented, "carries no coordinate and no requested amount".
7. Fail-open: none seen; `byte_of` Traps for position >= 8 (`:621-643`).

### B8. `request/protocol.omg` (544 lines)
1. Purpose: decode a staged OCREQ V1 request: envelope then field/tag shape.
2. Shape: `u32_at` `:4`; `identity_byte` `:15`; `Sections` `:32`; `Verdict` `:45`; `public request_envelope` `:64`; `SectionReader` `:187` with `open/fail/advance/take_u32/take_count/take_tag/take_bytes/finish` `:194-357`; `public request_structure` `:366`; `public compile_request` `:532`.
3. I/O: reads `Request`; returns `Verdict` / `Outcome`.
4. Decision: eight identity bytes compared by position (`:80-94`); two length words with high bit clear (`:103-117`); extents compared subtractively, exact end (`:119-140`); declared subject <= 67108864 and invocation <= 1048576 else Incomplete (`:143-155`); then structure: tag ranges (`take_tag(low, high)`): lineage 1..2, role 1..2, snapshot kind 1..3, executable 0..1, scope 1..2, product 1..4, admission kind `1..0` (empty, refuses any admission row, comment `:489-490`); count must fit `remaining / minimum_row` (`:290-309`).
5. Cases: row minimum widths 36 (package), 8 (snapshot, admission), 16 (edge).
6. Stated limits: `:359-365` "semantic phases (package keys, ordering, graph, snapshots, admissions, commitment) are not implemented yet, so it publishes the `coverage_request_semantics` provision rather than a verdict it cannot defend."
7. Fail-open: none seen: a request that survives structure always returns `coverage_request_semantics` (`:526-528`), never a success. The subject/invocation limit branches at `:144,151` cannot be reached with a staged request of at most 65536 bytes (inferred from `request.omg:9` and `:68-71`).

### B9. `request/request.omg` (73 lines)
`public data Request requires length <= 65536` `:7` (`bytes [u8; 65536]`, `length`, `observed`); `clear` `:16`; `request_stage` `:22` (counts every byte in `observed`, stores only while `length < 65536`); `staged` `:42`; `request_byte` `:47` (offset `< length`, restated `< 65536`, else Trap). Comment `:55-56` states the array bound is restated because the length range does not transport.

---------------------------------------------------------------------------------------------------

## C. source_to_psi (package `psi`)

### C1. `source_to_psi/build.omg` (3 lines)
`Build::package(builder, "psi")`.

### C2. `foundation/integers.omg` (447 lines)
1. Purpose: exact integers (sign + magnitude below 2^64) and exact rationals for literal folding and bounds.
2. Shape: `ExactInteger` `:10`, `ExactResult` `:15`, `Rational` `:308`, `RationalResult` `:313`; add/sub/mul/div/remainder `:28,65,71,101,285`; `exact_integer_at_most` `:128`; `minimum` `:151`; `maximum` `:171`; `is_carrier` `:191`; `exact_integer_fits` `:214`; `wrap` `:227`; `saturate` `:259`; `common_divisor` `:323`; `rational_*` `:342-447`.
3. Decision keyed on `BuiltinType`: only I8..I64 and U8..U64 are carriers (`:193-201`); `minimum` returns `0` for every non-signed type including Bool and floats (`:157`); `maximum` returns `0` for unlisted types, 1 for Bool (`:173,182`). `exact_integer_fits` is false when not a carrier (`:220`). Results that leave u64 magnitude, inexact quotients, zero divisors give `valid: false`.
4. Cases: carriers fixed to the eight integer builtins.
5. Same job elsewhere: other "is a fixed integer" predicates in the typer: `fixed_integer` (`references.omg:423`), `integer_type` (`expressions.omg:832`), `numeric_type` (`expressions.omg:1608`), and the lowerer's `scalar_plain_unsigned` (`checked_trees_to_scalar_operations/expressions.omg:577`).
6. Unused: grepped `exact_integer_remainder|\bsaturate\(|\bwrap\(|rational_add|exact_integer_divide` in `source/subsystems`: 12 hits in 4 files (integers.omg 6, typer expressions.omg 2, checker `arithmetic.omg` 2, `evaluation.omg` 2).
7. Fail-open: none seen.

### C3. `checked_trees_to_scalar_operations/layout.omg` (258 lines)
1. Purpose: compute a size and per-field offsets for each type the scalar route supports.
2. Shape: `ScalarLayoutState` `:19`; `scalar_layout_clear` `:28`; `scalar_require_layout` `:55` (explicit DFS; states `require/color/push/next/top/form/builtin/boolean/scalar/array/reference/record/fields/field/finish`).
3. I/O: `Typing` type table + `SyntaxTrees` data/field tables in; `lowerer.output.layouts`, `field_offsets`, `layout_*` out.
4. Decision keyed on `TypeKind` and `DomainBinding`. Supported: Unit; Bool (Unqualified, size 1); U8/U16/U32/U64 (sizes 1/2/4/8); arrays whose extent is `StaticInteger` and element is not a reference; references (Unqualified, referent not itself a reference); data records with Unqualified domain, no arguments, no generics, no cases, not boundary, not Linear (`:194-196`). Signed integers, floats and everything else -> `Unsupported` (`:124`). Policy domains Unqualified/Wrapping/Saturating/Trapping pass the row test (`:97-99`) but references, arrays and records further require Unqualified. Sizes capped at 33554432 -> `Capacity` (`:159-168,231`). Field handle and cursor `< 1024` (`:212,238`), types `< 8192`.
5. Cases by shape: comment `:51-54` "Recursive owned representations, generic layouts, sums, linear cleanup and unknown qualification remain coverage refusals. Reference cycles are also outside this first bounded representation; they never become zero-sized records."
6. Same job elsewhere: Alpha-side widths in `alpha/emit.omg:229-303` assume the same 1/2/4/8 set.
7. Stated limits: `:177-180` "Nested reference projection and assignment need a retained path of dereferences."
8. Fail-open: none seen (a `Visiting` re-entry is `unsupported`, `:71`).

### C4. `checked_trees_to_scalar_operations/scalar.omg` (431 lines)
1. Purpose: top-level scalar lowering: select the entry, require supported roots and machines, lower every machine, reject recursion.
2. Shape: `ScalarExpressionStage` `:37`; `ScalarLowerer` `:43` (fixed arrays: `values [..; 131072]`, `locals 16384`, `work/ready/place_mode/stages/pending/temporaries 4096`, `colors/cursors/calls 10240`); `scalar_append` `:69` (operations limit 16384); `scalar_value/_local/_bind` `:90,103,116`; `scalar_entry_result` `:131`; `scalar_machine_clauses` `:153`; `public scalar_lower` `:184`; `scalar_result_type` `:327`; `scalar_acyclic` `:342`.
3. I/O: `SyntaxTrees`, `NameResolution`, `Typing`, `Checks`, entry index in; `ScalarProgram` out (`lowerer.output`).
4. Decision: starts at status `Unchecked`, sets `Unsupported`, becomes `Complete` only after the entry passes (`:187-222`). Needs `syntax.accepted && resolution.accepted && typing.accepted && checks.accepted` (`:193`); `entry < syntax.machine_count <= 10240`, `erased_binding_count == 0`, `parameter_count <= 32768` (`:199-200`); entry machine `parent == 0 && parameter_count == 0 && !has_owner && container == Root`, result `Unit` or unqualified `U8` (`:131-149`). Roots: Use and Machine ok; Data ok if no generics, no cases, not boundary, not Linear; Trait only if `is_boundary && !is_refinement` and no generics, machines, parents or contracts; any other root kind -> Unsupported (`:234-257`). EVERY machine (not only those reachable from the entry) must satisfy `has_body && generic_parameter_count == 0 && !is_boundary && !has_target && !has_owner && container == Root && contract_count == 0 && component_count == 0 && scalar_machine_clauses` (`:272-274`), else the whole program is Unsupported. `scalar_machine_clauses` accepts only `CrashesNone`, `Reaches` and `Crashes Trap`; any other clause kind (including `Requires`, `Ensures`, `Terminates`, `Satisfies`) returns false (`:165-170`; comment `:151-152`). Result layout must be Scalar or Unit (`:289`).
5. Cases: comment `:338-340` "Runtime recursive cycles require proper tail transfer. Until that terminator is lowered, refuse every cycle instead of emitting frame-growing recursion." The DFS colours machines and refuses an edge to a machine in state 1 (`:418-421`).
6. Same job elsewhere: `scalar_machine_clauses` is the lowerer's own clause filter; the checker has its own clause handling (other unit).
7. Unused: `scalar_value` / `scalar_local` / `scalar_bind` are used across the lowerer files (read).
8. Stated limits: `:338-339`; comment `:182-183` "The synthetic entry has no caller-provided arguments or premises."
9. Fail-open: none seen in lowering itself; whether `checks.accepted` implies every `requires` was discharged is the checker's job (not read). Note the interplay: the lowerer refuses any machine with a `requires` clause (above).

### C5. `checked_trees_to_scalar_operations/statements.omg` (388 lines)
1. Purpose: lower parameters, locals, assignments, tail returns, `crash Trap` and `transition` arms to scalar operations.
2. Shape: `scalar_parameters` `:35`; `scalar_statements` `:72` (states `local/allocate/initialize/zero/assign/assigned/transfer/discard/terminal/returned/transition_statement/crash_statement/unit_end`); `scalar_destination_type` `:210`; `scalar_return` `:223`; `scalar_patch_branch` `:241`; `scalar_transition` `:259`; `scalar_trap` `:372`; `scalar_optional_branch` `:379`.
3. Decision keyed on `StatementKind`: Let, Assign, Expression, Discard, Tail, Crash, Transition handled; others -> Unsupported (`:90-99`). Tail and Transition must be the last statement (`:170,180`); `crash` only `CrashCause::Trap` and last (`:189`); a body with no terminal statement needs a `Unit` result (`:195-201`). A parameter must have a Reference layout or be copyable (`:57-58`). Assignment to a computed reference (reference-typed target that is not a `Name`) -> Unsupported (`:152-153`, comment `:149-151`).
4. Cases by shape: arms `Binder` and `Wildcard` are unconditional, `Guard` and `Literal` are conditional, any other `ArmPatternKind` -> Unsupported (`:284-289`); target kind `Unit` emits `ReturnUnit`, every other target kind is lowered as an expression and returned (`:340-343,349-352`); if no arm matches, `Trap` (`:367`).
5. Fail-open: the `Binder` arm is lowered exactly like `Wildcard`: it emits no operation that binds the matched subject to the binder name (`:285`, `guard` with `skip = 0`). A transition arm whose body reads its binder is handled elsewhere only if typing resolves such a name as `ValueBinding::MatchedSubject` (expressions `:125-129`); I did not trace whether a `Binder` arm's name resolves that way. open.

### C6. `checked_trees_to_scalar_operations/storage.omg` (164 lines)
Purpose: frame storage, copies and snapshots. `scalar_copyable` `:21` (Builtin and Unit yes; Data only when `item.multiplicity == Unrestricted` `:37`; Array by element with a depth counter; else no); `scalar_storage_allocate` `:49` (total storage capped at 33554432, else `Capacity`); `scalar_temporary` `:76`; `scalar_transfer` `:90` (Record/Array -> Copy, Scalar/Reference -> Store, Unit -> nothing, else Unsupported); `scalar_read_storage` `:123`. Stated limit `:17-19` "Checked affine moves need retained move disposition before they can use this copying path, so they remain explicitly unsupported here." Fail-open: none seen.

### C7. `checked_trees_to_scalar_operations/expressions.omg` (872 lines)
1. Purpose: lower expression trees (explicit work stack, source evaluation order) to scalar operations.
2. Shape: `public scalar_expression` `:51` (states `node/typed/classify/form/literal/boolean/name/field_child/...`); `scalar_place_mode` `:457`; `scalar_push_expression` `:469`; `scalar_push_collection` `:493`; `scalar_collection_type` `:501`; `scalar_runtime_type` `:516`; `scalar_observe_reference` `:538`; `scalar_plain_unsigned` `:577`; `scalar_mask_width` `:585`; `scalar_binary` `:601`; `scalar_call` `:722`; `scalar_construct` `:793`.
3. I/O: typed tables (`typing_expression_type/_selection/_adaptation/_value/_denominator/_landed_integer`, `construct_target`) in; operations, values per expression handle out.
4. Decision keyed on `ExpressionKind`, `Selection`, `Adaptation`, `OperatorKind`: handled kinds Integer, Boolean, Name, Field, Index, Unary, Cast, Binary, Call, Array, Construct (`:89-101`); others -> Unsupported. Literal must be non-negative with `denominator <= 1` (`:107`). Names: MatchedSubject, Local, Parameter only (`:124-128`). Field needs `Selection::Field` of a `Data` base, or `Selection::Length` of an Array layout (`:251-254`). Index needs `Selection::None`, an Unqualified Array base, and a plain unsigned selector (`:295-300`). Unary: only `Not` and the three borrows (`:171-173`). Cast: both Scalar or both Reference layouts; the value is passed through unchanged (`:354-361`). Binary: comparisons on plain unsigned integers, or booleans with `==`/`!=`; arithmetic needs plain unsigned operands of one builtin; `Add/Subtract/Multiply` direct, `Divide/Modulo` only when the builtin is not U64 (`:693-697`, comment `:691-692` "Alpha division is signed"), `BitAnd` only when one side is a constant mask 0xFF/0xFFFF/0xFFFF_FFFF (`:585-599,666-690`); all other operators (shift, BitOr, BitXor, Negate, BitNot, Dereference) -> Unsupported. `And`/`Or` short-circuit (`:195-199,378-411`). Call needs `Selection::Machine` and `expression.count == item.parameter_count` (`:727,732`).
5. Cases by shape: construction supports `Construct` of Record layouts and `Array` literals whose count equals the layout length (`:798-799`); a construct field target must be `MemberTarget::Common` (payload fields -> Unsupported, `:840-841`).
6. Limits (fixed capacities): work stack 4096 (`:473,209`), values 131072, arguments 65536 (`:738,766`), operations 16384.
7. Fail-open: `cast_value` returns the operand's slot unchanged for any scalar-to-scalar cast (`:359-361`); a narrowing cast emits no mask or range check, relying on the typer/checker (`typer expressions.omg:1700-1702`: "checking proves numeric representability"). inferred (checker not read). `Index` emits `AddressIndex` with no bound check operation (`:316`); same reliance, inferred.

### C8. `source_files_to_tokens/digits.omg` (81 lines)
`hex_digit_value` `:4` (returns 4294967295 for a non-hex byte, `:38`); `lexer_digit_in_base` `:42`; `lexer_digit_value` `:68`. Decision by byte range per base. Fail-open: none seen.

### C9. `source_files_to_tokens/lexer.omg` (1423 lines)
1. Purpose: turn one source unit's bytes into a `TokenStream`, per the closed ASCII lexical profile.
2. Shape: `Lexer` `:26`; `lexer_initialize` `:48`; `append_source_byte` `:56`; `decode_scalar` `:72`; `validate_utf8` `:338`; `classify_keyword` `:445`; `lex_identifier` `:482`; `lex_whitespace` `:530`; `lex_line_comment` `:559`; `lex_block_comment` `:590` (nested); `lex_cooked_string` `:674`; `lexer_accumulate_digit` `:818`; `integer_suffix` `:843`; `consume_digits` `:892`; `lex_number` `:977`; `lex_punctuation` `:1125`; `is_raw_string_candidate` `:1316`; `lex_next` `:1347`; `public tokenize` `:1402`.
3. I/O: `SourceUnit` bytes in; `TokenStream` tokens and decoded bytes out; rejections through `token_stream_reject`.
4. Decision on bytes: first, the whole unit must be valid UTF-8 (overlong, surrogates, > U+10FFFF rejected, `:72-307, 338-369`); any scalar `>= 128` outside a string is `OutsideLexicalProfile` (`:1349-1356`); whitespace is 9, 10, 13, 32; identifiers `[A-Za-z_][A-Za-z0-9_]*`; keywords decided by a string-compare chain `token_equals` over 30 words (as, calling_convention, capability, contains, data, else, false, foreign, host, if, let, library, loop, machine, match, owns, platform, public, return, self, struct, target, transition, true, use, when, while, case, copy, linear; `:445-476`), so keyword identity is source spelling; `in`, `requires` etc. are identifiers (comment `:510-511`). Strings: escapes `\" \\ \n \r \t \0 \xNN`; `\u` -> `OutsideLexicalProfile`; any other escape -> `UnsupportedEscape`; raw strings `r#"` -> `OutsideLexicalProfile` (`:1269-1313`); a newline inside a string -> `OutsideLexicalProfile` (`:696-697`). Integers: bases 0b/0o/0x, `_` separators, magnitude accumulated with an exactness flag (overflow clears `literal_exact` instead of wrapping, `:818-840`); suffix is any identifier-shaped run, mapped by `integer_suffix` to U8/U16/U32/U64/I8/I16/I32/I64 else Unknown (`:843-888`); floats via `.` or exponent. Punctuation: 28 forms, `>>` stays two tokens (comment `:1202-1203`).
5. Cases by shape: the 30-keyword list and the 28 punctuation forms are fixed; a decoded scalar byte not in the table -> `OutsideLexicalProfile` (`:1192`).
6. Same job elsewhere: escape decoding is repeated in the typer: `quoted_roster` (`literals.omg:74-245`), `quoted_length` (`literals.omg:328-362`), `typer_escape_byte` (`literals.omg:283-297`), and UTF-8 classification again in `quoted_roster` (`:185-239`). UTF-8 validation also in Windows path encoder (A40).
7. Unused: `tokenize` has 1 caller in `source/` (`driver/compiler.omg:83`); `lexer_initialize` and `append_source_byte` are called by the construction application (crate map).
8. Stated limits: `:19-24` closed ASCII profile; unterminated comments and strings reject.
9. Fail-open: none seen (every unmatched case rejects).

### C10. `symbol_resolved_trees_to_typed_trees/admission.omg` (515 lines)
1. Purpose: decide whether a value of one type may be used where another type is expected.
2. Shape: `public data Admission [copy]` `:31` (Rejected, NotAValue, OutOfRange, Exact, Landed, SharedLend, SharedReborrow, Qualified, Read, Fraction, FractionalIntermediate); `public typer_admission` `:47`; `public literal_admission` `:255`; `public typer_admit` `:337`; `blamed_value` `:449`; `public admissible` `:469`; `public views_array` `:495`.
3. Decision keyed on type-table handle identity first (`actual == destination` -> Exact, `:57`), then on `read` type, then kinds: anonymous integer/float -> `literal_admission`; anonymous choice/array -> `typer_landing`; a `String` expression into an unqualified `u8` array with extent equal to the quoted length -> Landed (`:85-100`); `Never` -> Exact (`:115`); same carrier but different qualifiers -> Rejected (`:122-125`); reference destinations get array-to-slice views (`views_array`), shared lends, shared reborrow, write-only attenuation (`:129-205`); generic data unapplied -> Exact when the data has applied parameters (`:208-220`). Integer landing needs a fixed integer destination, an integral value and a fit (`exact_integer_fits`, `:282-309`); an unqualified float destination always lands (`:267-268`). `@write` destinations need the argument to be spelled `@write` (`typer_admit`, `:348-351`, `ImplicitAttenuation`).
4. Cases: `lend` implicit shared lending only for call arguments or an index range slice (`:409-416`).
5. Unused: `Admission::Qualified` is never produced; it appears only as match arms at `:364` and `:478` (grepped `Admission::Qualified` in `source/`: 2 hits).
6. Fail-open: none seen.

### C11. `symbol_resolved_trees_to_typed_trees/arrays.omg` (79 lines)
`type_array` `:19`: fixed array of the joined element type; elements of no fixed type -> `AnonymousArray`; `[]` -> anonymous array of `AnonymousChoice`. Stated: `:14-18`. Fail-open: see C20 on `fixed_join` and non-value elements.

### C12. `symbol_resolved_trees_to_typed_trees/binders.omg` (417 lines)
1. Purpose: static machine binders: unify a binder's `requires machine` contract with the argument machine's signature, and type calls through a binder.
2. Shape: `CallableSignature` `:38`; `bind_machine` `:62`; `machine_bound` `:83`; `binder_signature` `:113`; `argument_signature` `:140`; `signature_result` `:178`; `signature_parameter` `:186`; `unify_machine_arguments` `:197`; `machine_arguments_refine` `:278`; `type_binder_call` `:386`.
3. Decision: a binder argument and contract must have the same parameter count (`:215,309`), and each parameter type and the result must be equal after substitution (`substituted == given`, `:348,368`). A generic machine as an argument has no signature (`:151-154`, `none()`), so it fails refinement. Binders live in a table of 256 (`:65,86`) with generation stamps.
4. Stated limits: `:34-35` "A generic machine as an argument is not supported yet."
5. Fail-open: none seen (missing contract or argument -> `unrefined`).

### C13. `symbol_resolved_trees_to_typed_trees/calls.omg` (1060 lines)
1. Purpose: type calls, constructions, proposition applications, layout queries and byte predicates.
2. Shape: `StaticArguments` `:66`; `type_call` `:71`; `substitute_if` `:190`; `match_overloads` `:214`; `candidate_admits` `:239`; `unsupplied_callee` `:324`; `bind_static_arguments` `:336`; `unify_arguments` `:406`; `type_proposition` `:459`; `bind_arguments` `:521`; `type_construct` `:554`; `construct_fields` `:662`; `infer_construction` `:766`; `bound_application` `:821`; `repeats_field` `:882`; `member_target` `:914`; `member_field_type` `:966`; `target_reference` `:975`; `type_layout_query` `:1005`; `type_byte_predicate` `:1031`.
3. Decision: overloads are tried in declaration order; exactly one admitting candidate is selected, none -> `NoMatchingMachine`, more than one -> `AmbiguousMachine` (`:149-156, 229-234`). A candidate is excluded when its container has generic parameters, or is a trait or refinement, or `has_target` (`unsupplied_callee`, `:324-332`). Fields in a construction and pattern are matched by SOURCE SPELLING of the field name (`spans_equal`, `:896, 928, 952`); duplicate field -> `DuplicateConstructionField`; unknown -> `UnknownConstructionField` (the maximum handle is the "none" sentinel, `:713,994-996`). A call selection is re-admitted after matching and traps if re-admission fails (`:160-167`). Generic data needs every parameter bound by some field (`:766-818`). `size_of<T>()`/`align_of<T>()` need exactly one static argument and no value arguments; the result is `u64` (`:1005-1027`); a byte predicate needs one argument that is a `u8` array or slice or a reference to one (`:1031-1059`).
4. Cases: comment `:1-8` of calls selects among visible overloads; static arguments bind a declaration prefix (comment `:334`).
5. Same job elsewhere: "earlier field has the same spelling" loops: `repeats_field` (`:882`) and the compare loop in `patterns.omg:41-49`.
6. Fail-open: none seen.

### C14. `symbol_resolved_trees_to_typed_trees/clauses.omg` (484 lines)
1. Purpose: type `satisfies` clauses and `terminates by` witnesses.
2. Shape: `type_clauses` `:48`; `type_satisfied_requirement` `:134`; `same_satisfied_signature` `:330`; `type_ranking` `:390`.
3. Decision: a satisfier's signature must equal the trait requirement's with the trait's type parameters substituted by the clause's static arguments: same parameter count, equal types, equal result (`:339-382`); the requirement must be a non-generic machine owned by a trait (`:161`); trait parameters other than types and lifetimes -> `UnsupportedTypeForm` (`:232`). A `terminates by` measure: `argument_count == 1`; a measure with `component_count > 0` must be given data whose NAME SPELLS the same as the measure's owner (`spans_equal(sources, data.name.source_span, measure.owner.source_span)`, `:465`); otherwise the subject is admitted at the measure's first parameter type (`:448-452`).
4. Stated limits: `:44-47` "A requirement with generic parameters of its own, or a trait with parameters other than types and lifetimes, rejects as unsupported."
5. Fail-open: `measured` reads `syntax_trees_argument(syntax, clause.first_argument)` before checking `clause.argument_count == 1` (`:433-435`), and `limit` reads `syntax_trees_argument(syntax, written.first)` with no count check (`:479`); the accessors are bound-checked per crate convention, so I did not find a path that accepts; inferred.

### C15. `symbol_resolved_trees_to_typed_trees/coverage.omg` (704 lines)
1. Purpose: decide whether a transition or value-position match covers its subject.
2. Shape: `public covers` `:43`; `closing_arm` `:117`; `arm_present` `:163`; `tests_a_field` `:268`; `typer_named_case` `:294`; `opposed_guards` `:307`; `arms_may_change_storage` `:402`; `guard_may_change_storage` `:429`; `node_may_change_storage` `:451`; `typer_complementary` `:499`; `boolean_comparison` `:547`; `public same_tree` `:579`; `same_node` `:612`.
3. Decision: covered when an unguarded `_`, binder, or the guard `true` exists (`:140-154`); over a subject: for a `Data` subject every case must be named by an unguarded arm that tests no field (`:83-104,264-286`); for Bool both literals; over guards, two guards `x == true`/`x == false` or `l == r`/`l != r` with structurally equal sides and no storage-writing guard between them (`:307-387`). A node may change storage if it selects a machine or binder, or if its kind is not on a read-only list (`:451-494`). `same_tree` compares kind, operator, flag, count and type per node, names/paths/fields by SOURCE SPELLING (`spans_equal`, `:640,668,686`), integer literals by value, and treats any other node (a call) as never the same (`:649`).
4. Cases: comment `:32-42`: "Any other ladder of literals, comparisons or predicates closes with `_`."
5. Fail-open: `guard_may_change_storage` treats `Index` and `Binary` nodes as pure (`:467-468`) unless they selected a machine (`:453`). Two names with the same spelling in two guards compare as the same variable (`:640`); I did not trace shadowing between guards. open.

### C16. `symbol_resolved_trees_to_typed_trees/expressions.omg` (2004 lines)
1. Purpose: type every expression node: names, literals, fields, indexes, unary/binary operators, casts, domain casts, borrows, places.
2. Shape: `public type_tree` `:88`; `type_expression` `:129`; `binding_type` `:306`; `type_field` `:487`; `type_index` `:605`; `type_range` `:717`; `type_endpoint` `:749`; `public place_type` `:797`; `public integer_type` `:832`; `signed_type` `:852`; `type_unary` `:865`; `public is_place` `:1059`; `public through_shared` `:1101`; `public writable_place` `:1135`; `type_binary` `:1199`; `public integer_like/float_like` `:1591,1599`; `numeric_type` `:1608`; `public byte_sequence` `:1623`; `intrinsic_equality` `:1646`; `type_cast` `:1703`; `type_domain_cast` `:1835`; `record_scalar_read` `:1934`; `public substitute` `:1970`.
3. Decision: a tree is typed once in table order, children first (`type_tree`, `:88-116`); `.length` on an array or slice is recognized by the SPELLING `"length"` (`span_spells(sources, expression.name.source_span, "length")`, `:508`); a field of a record or a payload field of any case (first case with that spelling) is matched by spelling (`:558,573`, comment `:535-536`: "The Rust producer still checks the preceding case conjunct before use."); suffix and unsuffixed literals (`:186-222`); `-` needs a signed integer, float, or anonymous (`:929-956`); `@mutable place` rejects a place reached through a shared reference (`:989-1003`); `@write` requires a writable place (`:1008-1027`); writable means the path passes an exclusive reference or the root is a `let mutable` local or `mutable` parameter (`:1135-1189`). Binary: data operands with an arithmetic/order token go through `type_operator_call` (`:1231-1236`); otherwise unqualified builtin rules, a bounded "policy" path for Wrapping/Saturating/Trapping with an unqualified or same-policy peer (`:1256-1317`), exact folding of two anonymous operands (`:1454-1457,1515-1532`) with `Modulo` of anonymous values rejected (`:1529`). Equality allowed for equal types that are intrinsically comparable, or two plain byte sequences (`:1356-1363`). Domain cast: one name, recognized through `known_domain` (spelling), only Wrapping/Trapping/Saturating, adding/removing/retaining one policy (`:1835-1928`; comment `:1846-1847` "Resolution rejects authored domains with these reserved spellings").
4. Cases by shape: comment `:1831-1834` "Named domains and simultaneous multi-entry accounting require more representation than this slice retains and fail explicitly".
5. Same job elsewhere: `integer_type` (`:832`) vs `fixed_integer` (`references.omg:423`) vs `is_carrier` (`integers.omg:191`) vs `numeric_type` (`:1608`).
6. Unused: `lesser` (`:118`): grepped `\blesser\(|machine lesser` in `source/subsystems`: only the definition (0 callers).
7. Stated limits: `:1247-1248`, `:1830-1834`.
8. Fail-open: `substitute` and `type_field` on an unapplied owner return the field type as written (`:1973-1988`, comment `:1967-1969`). `type_field`'s payload lookup does not check which case is active (`:535-536`, stated delegated to "the Rust producer"). traced to stated.

### C17. `symbol_resolved_trees_to_typed_trees/facts.omg` (136 lines)
`type_facts` `:30`: types each `requires`/`ensures` fact of machines, contracts, domains and data, and requires each to be Boolean (`fact_type == typer.output.boolean`, `:124`; skipped when the typer has already rejected). A data `requires self` is typed as the declaration applied to its own parameters (comment `:27-28`). Fail-open: a fact is passed silently if `!typer.output.accepted` (`:124`) because a rejection already exists; traced (read).

### C18. `symbol_resolved_trees_to_typed_trees/generics.omg` (529 lines)
Purpose: generic bindings, substitution and unification over the interned type table. `begin_bindings` `:31` (increments a generation); `typer_bind/bound_type` `:35,54` (256-slot table with stamps); `bind_data_arguments` `:82`; `all_bound` `:123`; `substitute_bound` `:158` (post-order walk with per-call memo, `work` stack of 4096, memo of 8192); `typer_unify` `:337`. Decision: unification binds a generic parameter only if it is within `[first, first+count)`, not yet bound, and the given type is concrete and not unlanded (`:366-374`); conflicts are not reported (comment `:333-336`: "the caller admits each argument at the substituted parameter type afterwards"). Capacity: `type_table_push_argument` failure -> `TypeCapacityExceeded` (`:317-319`). Fail-open: `substitute_bound` leaves unbound parameters as written (`:156-157`, stated).

### C19. `symbol_resolved_trees_to_typed_trees/joins.omg` (189 lines)
`fixed_join` `:27` (type of the first value that is neither unlanded nor `Never`, else 0); `admit_joined` `:56` (each value must be Exact, Landed or SharedReborrow at the join type; others rejected); `unlanded_join` `:116`; `scalar_or_array` `:177` (shape: AnonymousArray -> its own type; anonymous integer/float/choice -> the choice; every other type -> 0). Comment `:15-24`.
Fail-open: `scalar_or_array` returns shape 0 for any type that is not an anonymous kind, including a non-value (type handle 0) element; a value with shape 0 is compatible with every join (`:135`). Because `fixed_join` returns the first value's type only when it is not unlanded and not `Never` (`:44-46`), an element with type 0 (not a value) could be chosen or skipped depending on `typer_unlanded(0)`/`never_type(0)`, which are defined elsewhere (typer file not in this unit). open.

### C20. `symbol_resolved_trees_to_typed_trees/landing.omg` (351 lines)
Purpose: unlanded values (anonymous numbers, choices, arrays) take their type from the destination. `typer_unlanded` `:32`; `unlanded_join_type` `:41`; `shape_depth` `:49` (cap 64); `peeled` `:68`; `landing_scalar` `:89` (needs each anonymous array level to match an unqualified array of the same extent); `typer_landing` `:124` (walks nodes from the root down to the tree's lowest row, tests each anonymous number with `literal_admission`); `typer_land` `:222`. Comment `:17-30`. Fail-open: none seen (`landing_scalar` returns 0 on mismatch and `typer_landing` returns Rejected).

### C21. `symbol_resolved_trees_to_typed_trees/literals.omg` (362 lines)
`LiteralValue` `:17`; `literal_value` `:27` (valid iff `exact && !empty_digits` and a suffix that is not `Unknown`); `quoted_roster` `:74` (bits: valid_utf8 1, no_nul 2, ascii_only 4, non_empty 8; UTF-8 classes by lead byte with `pending`/`low`/`high`); `typer_escape_byte` `:283` (`n r t 0` mapped, any other escape kind maps to its own byte, `:291`); `hex_value` `:300` (returns 0 for a non-hex byte, `:319`); `quoted_length` `:328` (`\x` counts as 1 byte and skips 4 source bytes; other escapes skip 2).
Same job elsewhere: escape and UTF-8 decoding duplicated from the lexer (C9). Fail-open: `typer_escape_byte` and `hex_value` silently map unsupported escapes and non-hex digits instead of refusing (`:291,319`); the lexer refuses both first (`lexer.omg:738,761-775`), and `compilation_add_unit` always tokenizes before parsing (`driver/compiler.omg:82-95`). traced: not reachable for a unit that went through the driver.

### C22. `symbol_resolved_trees_to_typed_trees/matches.omg` (140 lines)
`type_match` `:26`: subject must be a value and not unlanded (`:38`); each arm pattern is typed by `pattern_head` (defined elsewhere); coverage via `covers` -> else `UncoveredDispatch` (`:96-105`); arm values joined (`fixed_join`/`admit_joined`) or, with no fixed type, an unlanded join. Comment `:16-25`. `pattern_head` is called here (`:75`) but defined outside this unit (not read).

### C23. `symbol_resolved_trees_to_typed_trees/memberships.omg` (79 lines)
`type_membership` `:26`: result is Boolean; a case membership requires the value's data to be the case's data (`CaseOfOtherData`, `:47-51`); a declared-domain membership requires the domain's carrier to be the same carrier as the value's (`same_carrier`, `:62-71`); any other path -> `UnsupportedExpression`. It does not test the domain's predicates (those belong to later stages). `value_binding(resolution, index)` is read before `value_type > 0` is tested (`:34-35`). Stated `:23-25`: "Joined domains (`&`, `|`) bind no domain at resolution yet."

### C24. `symbol_resolved_trees_to_typed_trees/operators.omg` (266 lines)
`type_operator_call` `:36`; `operator_matches` `:94`; `operator_admits` `:148`; `homed_operand` `:220`; `typer_data_type` `:239`; `operator_token` `:249`. Decision: candidates are ALL machines in the table (`candidate < syntax.machine_count`, `:105`) whose `item.operator == token`, with two parameters and no generics, homed at an operand's data, visible (public or same package), and admitting both operands (`:160-212`); exactly one must match (`:47-49`). Tokens: Add, Subtract, Multiply, Divide, Modulo, Less, LessEqual, Greater, GreaterEqual; `==` and `!=` have none (`:249-261`, comment `:246-248`). Stated `:34-35`: "A generic token machine is no candidate yet." One caller of `type_operator_call` (`expressions.omg:1324`), grepped.

### C25. `symbol_resolved_trees_to_typed_trees/patterns.omg` (68 lines)
`validate_pattern_fields` `:16`: duplicate field names (by source spelling, `:44`) -> `DuplicatePatternField`; unless `..` is present, `arm.field_count != expected` -> `MissingPatternField` (`:52`). Comment `:12-15` "Every field resolves before this check." Fail-open: none seen.

### C26. `symbol_resolved_trees_to_typed_trees/references.omg` (778 lines)
1. Purpose: type every written type row (builtins, data, generics, references, arrays, slices, literals, trait/machine/conformance arguments) and check static-argument slots.
2. Shape: `type_references` `:42`; `domain_admitted` `:395`; `fixed_integer` `:423`; `applied_parameter_count` `:431`; `const_parameter` `:460`; `collection_qualifier` `:481`; `trait_slot_argument` `:503`; `runtime_binder_count` `:553`; `slot_accepts` `:583`; `check_trait_telescope` `:662`; `static_parameter_annotation` `:719`; `trait_slot_accepts` `:732`; `plain_static_name` `:773`.
3. Decision keyed on `TypeBinding` and `DomainBinding`: the row match ends with `_ -> unsupported` (`:72`); a domain must suit its builtin: policies qualify fixed integers, `Finite` a float, `Declared` anything, `Case` never a builtin (`:395-402`); array lengths must be a static integer row or a `const` generic parameter (`:280`); static arguments must fit their slot kind (`slot_accepts`: Evidence takes Conformance or an evidence binder; Type takes builtin/data/reference/array/slice/type binder; Const takes literal or const binder; Value takes literal or value/const binder; Machine takes machine or machine binder; `:583-656`); a trait argument in a typed static slot must name a non-generic trait with a conformance of the same trait (`:732-771`). Capacity 65536 rows (`:49`).
4. Stated limits: `:93-95` "A call's static argument naming a conformance stays unsupported: this typer does not yet bind a machine's separate evidence telescope"; `:37-41` "a named static value (a `const` parameter) has no value before evaluation exists and rejects".
5. Same job elsewhere: see C2 item 5 for `fixed_integer`.
6. Fail-open: none seen.

---------------------------------------------------------------------------------------------------

## Coverage

Read in full, every line, in order (21,956 lines, 95 files; counts from the unit TSV):
- A: `targets/uefi_x86_64/integrity.omg` 53; `tables.omg` 137; `targets/windows_x86_64.omg` 4; `windows_x86_64/calling.omg` 176; `clock_impl.omg` 20; `console_impl.omg` 96; `entry.omg` 195; `filesystem/{copy 92, directory 65, errors 69, handles 255, locking 30, metadata 204, paths 264, provider 353, sequences 113}`; `filesystem_impl.omg` 3; `filesystem_removal/{acquisition 149, attributes 35, completion 55, native 55, removal 48, traversal 179}`; `time_host_impl.omg` 56; the 24 `windows_filesystem_*` files (88, 225, 168, 172, 52, 98, 26, 113, 60, 34, 176, 57, 75, 90, 28, 169, 97, 35, 77, 13, 110, 104, 31, 170, 28); `windows_gui_impl.omg` 307; `std/time.omg` 1068; `std/time_host.omg` 86; `uefi_x86_64_entry.omg` 232; `uefi_x86_64_handoff.omg` 365; `uefi_x86_64_integrity.omg` 40; `windows_filesystem_host.omg` 193; `windows_gui_host.omg` 120; `windows_x86_64_clock.omg` 25; `windows_x86_64_filesystem_calling.omg` 13; `windows_x86_64_time_host.omg` 84.
- B: `alpha/build.omg` 6; `alpha/emit.omg` 701; `driver/{build 9, compiler 196, slot_review 15}`; `request/{build 6, outcome 643, protocol 544, request 73}`.
- C: `source_to_psi/build.omg` 3; `foundation/integers.omg` 447; `checked_trees_to_scalar_operations/{expressions 872, layout 258, scalar 431, statements 388, storage 164}`; `source_files_to_tokens/{digits 81, lexer 1423}`; `symbol_resolved_trees_to_typed_trees/{admission 515, arrays 79, binders 417, calls 1060, clauses 484, coverage 704, expressions 2004, facts 136, generics 529, joins 189, landing 351, literals 362, matches 140, memberships 79, operators 266, patterns 68, references 778}`.
Skimmed: none. Not read: none.

Greps run (commands as Grep tool patterns): `machine (valid_map_geometry|uefi_os_handoff_cycle_*)` over `*.omg`; `Admission::Qualified` in `source/`; `compilation_finish|compilation_add_unit` in `source/`; `OmegaSlotReview|slot_review` count in `source/` and `OmegaSlotReview` in `source-rust/`; `WindowsFilesystemHost::(get_file_time|set_file_pointer|set_file_attributes|move_file)` in `source/`; `WindowsFilesystemHost::duplicate_handle|windows_filesystem_clone`; `exact_integer_remainder|saturate|wrap|rational_add|exact_integer_divide` in `source/subsystems`; `WindowsClockWait|WindowsX8664Clock`; `compilation_finish|compilation_add_unit|alpha_emit` outside `source/`, `wiki/`, `.agents/`; `scalar_binary|scalar_call|...` in `source/`; `lesser`; `reject_code_space|...` counts; `windows_wide_path\(` count; Glob `TASKS_TIME.md`.

## Five things a maintainer most needs to know about this unit

1. The Omega-written compiler lowers only a small closed subset. `scalar_lower` (`checked_trees_to_scalar_operations/scalar.omg:258-278`, `:272-274`) applies the machine filter to EVERY machine in the source set, not only those reachable from the entry, and `scalar_machine_clauses` (`:153-180`) accepts only `crashes none`, `reaches` and `crashes Trap` clauses. A single machine with a `requires`, `ensures`, `terminates` or `satisfies` clause anywhere makes the whole program `Unsupported`. Only unsigned 8/16/32/64-bit integers, Bool, arrays, references and plain records have layouts (`layout.omg:108-126`).
2. The emitter and lowerer emit plain operations named `ExactInteger*`, `AddressIndex`, `Load` and unchanged-value casts with no runtime overflow, bounds or narrowing checks (`emit.omg:684-693`, `scalar expressions.omg:316,359-361`); they rely on `checks.accepted` and typing. The checker was not read here, so the safety of that handoff is not established by this unit.
3. Name, field and keyword identity in the typer and lexer is by source spelling: keywords by a 30-step string-compare chain (`lexer.omg:445-476`), record/payload fields and `.length` by `spans_equal`/`span_spells` (`calls.omg:896,928,952`, `expressions.omg:508,558,573`), complementary guards and measure owners by spelling (`coverage.omg:640-686`, `clauses.omg:465`).
4. The OCREQ request decoder never accepts: every well-framed request ends in `coverage_request_semantics` (`protocol.omg:526-528`); 96 of the 97 Reject codes and `reject_code_space` are never produced (`outcome.omg`; grepped).
5. The Windows provider closure is large and layered (about 45 files) with the same shapes repeated: the wide-path preamble 17 times (`windows_wide_path(` grep), the 17-service `reaches` list in the GUI files, the clobber list in 3 places, directory-record decoding twice, `Sleep` splitting twice. Several host members (`get_file_time`, `set_file_pointer`, `set_file_attributes`, `move_file`) have no callers (grepped), and the file `time_host_impl.omg` says native execution is unverified (`:32-34`). UEFI plan acceptance checks fewer shape facts than the Windows one (`uefi_x86_64_entry.omg:70,147` vs `entry.omg:44,51,79-92`).

## What you are unsure about

- Whether the checker proves every obligation the lowerer and emitter skip (overflow, bounds, narrowing casts, divide by zero); not read.
- Whether a transition `Binder` arm's name is bound for later expressions (`statements.omg:285` emits no binding); depends on how `ValueBinding::MatchedSubject` is created, outside this unit.
- Whether the Rust caller validates UEFI signature shapes before `uefi_x86_64_plan` copies sizes from them (`uefi_x86_64_entry.omg:87-97`).
- The behaviour of `typer_unlanded(0)` / `never_type(0)` for an element of type 0 in `fixed_join` (`joins.omg:40-47`); those are defined in the typer entry file outside this unit.
- The leg machines `uefi_os_handoff_cycle_*` and `valid_map_geometry` (`targets/uefi_x86_64/handoff.omg`) decide when `Ready` is reached; not in this unit.
- Counts of members (46 in the Windows filesystem provider, 31 in the host trait, 30 keywords) come from reading, not a command.
