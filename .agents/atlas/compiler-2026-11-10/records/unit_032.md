# Notes for unit 32

Unit: 32. Files covered (83 rows in unit_032.tsv): all under `omega-rust/omega/`:
- interpreter/terminal-runtime: `src/execution/host_imports/{borrowed_storage.rs, borrowed_storage/geometry.rs, buffered_output.rs, clock.rs, clock/{admission,timespec,windows}.rs, descriptor_io.rs, descriptor_mode.rs, filesystem.rs, filesystem/linux.rs, filesystem/windows.rs, filesystem/windows/contracts.rs, integer_absolute.rs, integer_division.rs, integer_parsing.rs, native_provider.rs, native_provider/{original_homes,record_fields,record_shape,record_shape/tests,reference_outputs,service_reach,values}.rs, native_word.rs, network_address.rs, numeric.rs, reference_state.rs, scalar_codec.rs, scalar_record.rs, standard_input.rs}`, `src/execution/native_code_layouts.rs`, `src/lib.rs`, `src/runtime_boundary.rs`, `tests/retained_runtime.rs`
- packages/build-declarations/src/*, packages/build-output/src/*, packages/package-compilation/{build.rs, src/**}

Paths below are relative to `omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/` (written HI/) unless stated.

## HI/borrowed_storage.rs (435 lines, read in full)
- Purpose: joins semantic storage loans (what the interpreter hands an import as boundary storage buffers) to native bytes. It builds a byte image of a loan, lets an adapter read or write that image, and then writes decoded values back into the loan. Top comment :1-11 says zeroed holes in staging memory are never published as values.
- Shape: `StorageTransfer {Observe, ReplacePrefix}` :29; `CapturedStorage<'layout>` :34 (bytes, covered ranges, cells, layout, captured_paths, primitive_byte_root); `PointerOutputPlan {selected, overlaps}` :43 with `prepare` :49 and `validate` :136; `CapturedStorage::capture` :163, `terminated_prefix` :304, `byte_path_at` :322, `stage` :340; free fns `invalid_operands` :156, `covered_range` :423.
- Inputs/outputs: reads `TerminalBoundaryStorageBuffer` (exactly one buffer, :171), a `NativeBorrowedStorageLayout` (from runtime_abi), `geometry::expand`. Produces `bytes` plus a coverage list; `stage` mutates the buffer via `buffer.replace(...)` (:359, :419).
- How it decides: keyed on exact structural type identity (`layout.structural_type == buffer.structural_type()` :196), argument index, access mode (:206-212), and canonical path vectors (`Vec<CanonicalStructuralPathSegment>`) sorted and binary-searched (:178, :222). `Observe` requires the byte prefix `0..byte_count` to be fully covered by value cells (:266); `ReplacePrefix` requires that the possible cells (all layout cells that could be supplied) cover the prefix (:271-281) and requires `MutableBorrow` (:208-209). `stage` decodes an active cell only if every byte was supplied by the read or retained (:379-383). Case cells decode a 4-byte little-endian tag via the layout's `discriminator_cases` (:396-411) and fail if the tag names no declared case (:406-410).
- Cases by shape: scalar buffer allowed only when `layout.is_none()` (:183), structural only when a layout is present (:194-196); anything else is `invalid_operands` (:204). "primitive_byte_root" is exactly one empty-path 8-bit non-address integer cell (:213-218). Discriminator tags are fixed at 4 bytes (`NativeCell::byte_size` geometry.rs:32). `PointerOutputPlan::prepare` accepts only a reference-typed tip whose referent is a primitive scalar (the "lanes" address+codec shape `ScalarReferenceLayout::NATIVE`, :73-88) or any other referent, in which case descriptor size = pointer size (:89).
- Same job elsewhere: byte/offset geometry is also in `HI/scalar_codec.rs` (`encode`/`decode`/`scalar_extent`, read below) which this file calls; `HI/native_provider/record_shape.rs` does record layout matching (read below).
- Unused / one-caller: `PointerOutputPlan` users grepped later (see native_provider entry). `scalar_extent` re-exported `pub(super) use` :15.
- Stated limits: ":2 without inventing backing bytes"; ":271 input accesses padding or bytes without a scalar or discriminator recipe" (error text :279).
- Fail-open risks: `stage` with `written` bytes treats bytes 0..written as covered (:352) without checking they were actually produced by the host call; callers pass the host-reported byte count (traced in descriptor_io.rs:529-582, bounded by `count`, and `written > self.bytes.len()` is rejected :345). Disposition: traced, bounded. `byte_path_at` returns the unique byte-typed cell at an offset or error (:333-336); none otherwise seen.

## HI/borrowed_storage/geometry.rs (184 lines, read in full)
- Purpose: expands the layout's fixed-array "repetition" recipes into one cell per element and answers whether a path is active under the current case selection.
- Shape: `NativeCellKind {Scalar(&Field), Case(&Discriminator)}` :17; `NativeCell {path, byte_offset, kind}` :22 with `byte_size` :29 and `encode` :36; `expand` :65; `input_coverage` :102; `expand_coordinates` :136 (recursive over `NativeStorageRepetition`); `is_active` :165.
- Inputs/outputs: reads `NativeBorrowedStorageLayout` fields/discriminators/opaque_regions and their repetitions. Outputs `Vec<NativeCell>` or coverage ranges; no mutation.
- How it decides: `encode` accepts a scalar cell only when stored scalar and logical scalar type equal the field's (:48-49) and case cells by mapping the case id to the native tag (:54-58). `expand_coordinates` requires the path coordinate at `dimension.path_position` to be `FixedIndex(0)` (:150). `is_active` walks path segments; at each `Case(c)` segment it binary-searches `values` for the selector path prefix and compares its active case to `c` (:169-181); a missing selector row is an error, not "active".
- Cases by shape: only fixed-index repetitions (`FixedIndex(0)` placeholder). `input_coverage` rejects any active opaque region that begins inside the transferred prefix (:114-118): "input accesses unmodeled permission or byte-descriptor storage".
- Same job elsewhere: `borrowed_storage.rs:94-98,99-126` calls the same `expand`/`expand_coordinates` for pointer-output selection.
- Fail-open risks: none seen; `is_active` returns Err when the selector row is missing (:173).

## HI/buffered_output.rs (194 lines, read in full)
- Purpose: invocation-owned stdout/stderr byte buffers and the adapter that appends line or byte writes from a bound output import.
- Shape: `BufferedBytes(Arc<Mutex<Vec<u8>>>)` :16; `BufferedStreams {stdout, stderr}` :19 with `append(descriptor, bytes)` :32 (1 -> stdout, 2 -> stderr, else reject :40); `BufferedOutput {signature, kind: OutputKind, output}` :68; `TerminalEffectHandler::handle_effect` :91; one unit test :148.
- Inputs/outputs: `OutputKind` from the parent module (grep'd below); output is appended to the shared buffer; `snapshot()` clones it.
- How it decides: operand check compares scalar types to the bound signature and requires every structural argument to carry a byte-sequence payload (:95-104); `Line` requires exactly one byte payload, appends bytes then `\n` (:110-118); `Byte` requires one signed integer, narrows to i32 and writes the low 8 bits `value as u8` (:119-135).
- Cases by shape: "line output" and "byte output" only (`OutputKind`), exactly one argument each.
- Fail-open risks: none seen. Out-of-i32 byte values are rejected (:131), tested at :179-185.

## HI/clock.rs (437 lines, read in full)
- Purpose: models clock reads and sleeps for Darwin, Linux and Windows imports using either a fixed snapshot (waits advance the snapshot) or the live host clock (waits really sleep). Header comment :1-25 lists what is and is not modeled (CPU accounting, signals and readiness unsupported).
- Shape: `TerminalClockSnapshot {monotonic_nanoseconds, unix_nanoseconds}` :48; `TerminalClockEnvironment {Snapshot, Host}` :54; `ClockNamespace {Darwin, Linux, Windows}` :66 with `validate_selector` :73; `ClockImplementation` :156, `ClockOperation` :165 (carries layouts), `ClockImport` :174 with `admit` :182, handler impl :297.
- Inputs/outputs: `admission::select`/`validate` choose the operation; `timespec::{read,sleep}` and `windows::write_word` write results into loans; `read_host_clock_nanoseconds` from `native_provider_runtime` for live reads :41,:139. A shared `Arc<Mutex<TerminalClockEnvironment>>` is passed in.
- How it decides: selector validity by namespace and number: `(_,0)`, Darwin 4|5|6|8|9, Linux 1|4|7, Windows `windows::MONOTONIC` (=1) :75-78. Selector 0 returns unix time, anything else monotonic (:93-97). Live supply requires `selected_target == host_target` (:213-226). Poll accepts only non-negative timeout and `count & 0xFFFFFFFF == 0` (:422), otherwise "requires descriptor custody".
- Cases by shape: signed or unsigned 32-bit non-address selector only (:244-248); poll's first argument must be an unsigned value and its content is ignored (:405-409).
- Same job elsewhere: `windows::sleep_nanoseconds` (clock/windows.rs:161) and Poll path :427-432 both convert ms to ns with `checked_mul(1_000_000)`.
- Stated limits: ":17 CPU accounting, signals, and descriptor readiness remain unsupported".
- Fail-open risks: Poll ignores its pointer argument value (the `_` at :407) and `count` low 32 bits must be zero, so a poll with a nonzero pointer and zero count is treated as a pure sleep. Disposition: stated by the header (:16-17 "descriptor readiness remain unsupported") only partly; I did not trace whether the pointer is validated elsewhere, so "open".

## HI/clock/admission.rs (187 lines, read in full)
- Purpose: picks which clock implementation a foreign call binding is, and checks the declaration and native ABI against it.
- Shape: `select` :19 and `validate` :74.
- How it decides: Linux system-call numbers per target (228/113 clock_gettime, 7 poll (x64 only), 35/101 nanosleep) :23-30; macOS libSystem symbol names compared by normalised locator equality (:45-69); Windows delegated to `windows::select`. Anything else returns `Ok(None)` (not ours), not an error.
- Cases by shape: exact parameter shapes per implementation (:95-142), e.g. Poll = [u64 pointer, u32|u64 count, i32 timeout] (:134-137). Statuses: unit, i32, or (Linux only) i64 (:91-94).
- Same job elsewhere: other adapters repeat this shape (select binding -> validate declaration -> `admit_exact_foreign_call`), e.g. descriptor_io.rs:186-317, descriptor_mode.rs:30-77, integer_absolute.rs:26-119.
- Fail-open risks: `select` returning `Ok(None)` for unmatched numbers leaves the call to other adapters (not accepting). none seen.

## HI/clock/timespec.rs (178 lines, read in full)
- Purpose: reads/writes a 16-byte timespec (two i64) in borrowed storage for clock_gettime and nanosleep.
- Shape: `admit` :20, `read` :55, `sleep` :102.
- How it decides: loan access must be MutableBorrow for read and a reading-borrow pair for sleep (:36-40); layout `byte_size >= 16` and `alignment >= 8` (:47-48); sleep rejects `seconds < 0` or nanoseconds outside 0..1e9: Darwin rejects, Linux returns -22 (:158-165).
- Cases by shape: fixed 16-byte timespec at offset 0 only.
- Fail-open risks: none seen.

## HI/clock/windows.rs (212 lines, read in full)
- Purpose: Windows kernel32 time exports over the shared clock.
- Shape: `WindowsWord {Counter, Frequency, FileTime}` :29; `select` :45; `word_loan_valid` :80; `write_word` :99; `sleep_nanoseconds` :161; two tests :188,:208.
- How it decides: exports matched by normalised PE locator (`kernel32.dll` + name) :48-75; Counter writes i64 reading, Frequency writes 1e9, FileTime writes `reading/100 + FILETIME_UNIX_EPOCH_INTERVALS` (:141-150). Sleep rejects 0xFFFFFFFF (:162).
- Fail-open risks: none seen. Note FileTime uses the *Unix* reading selector 0 + 1601 offset, so a snapshot's `unix_nanoseconds` is what is reported (:146).

## HI/descriptor_io.rs (588 lines, read in full)
- Purpose: trusted read/write over descriptors: 0 reads the finite replay stream, 1/2 append to private output buffers, other descriptors go to Linux filesystem delegation when supplied; supports scalar, byte-view and aggregate buffers across Windows CRT, Darwin libSystem, Linux libc and Linux kernel ABIs.
- Shape: `DescriptorBuffer {Scalar, Aggregate(layout), Bytes}` :43; `DescriptorContract` :49; `DescriptorIoImport` :59 with `admit` :67, `for_filesystem` :110, `validate_native_transfer` :143; `select_binding` :186 (uses `runtime_abi::descriptor_system_call`/`descriptor_import`); `admit_contract` :198; `validate_transfer_count` :319 (cap 0x7fff0000 for Linux ABIs).
- How it decides: ABI/operation from runtime_abi by system-call number or locator; width 32 for Windows CRT, else 64 (:211-216); buffer kind from the formal's structural shape (:223-253). Write: descriptor not 1|2 on Windows CRT returns -1 (:467-471); on Linux with filesystem delegation goes to the host (:472-477); otherwise `BufferedStreams::append` which rejects descriptors other than 1/2 (buffered_output.rs:40). Read: non-zero descriptor is rejected except Windows CRT returns -1 (:485-492) or Linux kernel with filesystem delegation (:482-484); input errors return -1 on Windows/Darwin and are rejected on Linux (:559-575).
- Cases by shape: aggregate buffer only if `record_or_fixed_array_shape_pattern!` or `sum_or_mixed_shape_pattern!` (:242-245); byte-view buffers only for Darwin/Linux ABIs (:233-238).
- Same job elsewhere: `HI/standard_input.rs` and `HI/native_provider.rs` also touch stdin/streams (read below). `validate_native_transfer` duplicates the capture call at :449-459.
- Stated limits: ":14-19" no errno update on input errors; "The host explicitly assumes these implementations are equivalent".
- Fail-open risks: Read from descriptor 0 on Linux kernel when `linux_filesystem` is Some and descriptor is 0 goes to the replay stream (`linux_file` is false when descriptor == 0, :482); fine. Windows CRT write to descriptors other than 1/2 reports failure -1 rather than rejecting (:467-471): stated by the header ("Unsupported descriptors refuse before I/O" is not exactly this; the -1 path is traced to be the CRT error result, no I/O occurs).

## HI/descriptor_mode.rs (136 lines, read in full)
- Purpose: MSVCRT `_setmode` model: per-descriptor mode table initialised to `_O_TEXT` 0x4000, returns the previous mode.
- Shape: `DescriptorModeImport {signature, result_type, modes: BTreeMap<i128,i128>}` :23; `admit` :30; handler :98.
- How it decides: locator equals normalised `msvcrt.dll!_setmode` on WindowsX64 (:40-51); descriptor outside 0..=2 returns -1 (:130-132); mode value is stored verbatim without validation beyond i32 range (:126,133).
- Same job elsewhere: `BTreeMap` keyed by small descriptor where AGENTS.md says dense ids use flat tables; same pattern as `filesystem.rs:37` (descriptors map).
- Fail-open risks: any 32-bit mode is accepted and stored; stated by the header (":4 returns the previous mode verbatim").

## HI/filesystem.rs (800 lines, read in full)
- Purpose: matching-host filesystem delegation with an isolated guest descriptor/handle namespace across Darwin, Linux, Windows. Darwin entries are a table of libc symbols and Formal parameter recipes.
- Shape: `FilesystemContext {supplied, windows: Rc<RefCell<windows::Handles>>, linux: Rc<RefCell<LinuxFilesystem>>, descriptors: Rc<RefCell<BTreeMap<i32, NativeDescriptor>>>}` :33; `NativeDescriptor {Open, Uncertain}` :40; `DescriptorEffect` :46; `DarwinFilesystemCall` :59; `FilesystemCall {Darwin, Windows}` :67 with forwarding methods :72-140; `Formal` :143; `FilesystemContext::admit` :178, `admit_services` :427; `matches_signature` :465, `scalar_matches` :476, `matches_parameters` :488; `DarwinFilesystemCall::{validate_path, arguments, result}` :601-744; 2 tests :769,:785.
- How it decides: Darwin candidates are chosen by C symbol name + dylib install name through `matches_platform_c_symbol` (:360-365); it only admits when target is MacosArm64 and the compiler itself runs on macOS aarch64 (`cfg!` :191-194). `admit_services` accepts an extra service only when its identity text is one of six `toolchain::Filesystem...`/`DirectoryEnumeration` strings (:446-456), i.e. keyed on source spelling of service identity text. Guest descriptor translation: guest 0..=2 rejected (:672-676), unknown guest descriptor maps to native -1 (:687), uncertain rejected. Guest descriptors allocated from the lowest free number >= 3 (:733).
- Cases by shape: the 26-row Darwin candidate table :201-358 (open, __open, creat, close, unlink, rmdir, mkdir, rename, link, symlink, chmod, chown, lchown, openat, unlinkat, fstatat, fstat, fchmod, fchown, ftruncate, lseek, futimens, fsync, fdatasync, flock, dup, __getdirentries64). Path formal is exactly `UnsignedArray(4096, 8, SharedBorrow)` (:199). `flags & 0x200` (O_CREAT) on non-mode open forms is rejected (:645).
- Same job elsewhere: Linux guest-fd handling is in `filesystem/linux.rs` via `native_provider_runtime::LinuxFilesystem`; Windows handle table in `filesystem/windows.rs:18-51` (same Open/Uncertain scheme and lowest-free-number allocation, duplicated shape of `NativeDescriptor`/`State`).
- Stated limits: ":42 Failed close may have retired the OS number" (Uncertain state).
- Fail-open risks: (1) `arguments` maps an unknown guest descriptor to native -1 and lets the host call proceed (:687); the OS then returns EBADF, no wrong access, "traced" as safe because -1 is never a valid fd; but `-2` is passed through for `current_directory` (AT_FDCWD) :677. (2) `self.supplied` is true for any `HostNamespace` delegation; `admit_services` only checks that *additional* services are in the toolchain list, skipping the owning service (:437-439). "open".

## HI/filesystem/linux.rs (521 lines, read in full)
- Purpose: executes Linux kernel filesystem syscalls (openat, close, lseek, unlinkat, mkdirat, newfstatat, fstat, getdents64) through `native_provider_runtime::LinuxFilesystem`, plus the read/write adapter used by descriptor_io.
- Shape: `LinuxFilesystemWrite` :62 (admit :68, write :80, read :94); `OperationKind` :109; `Operation` :120; `LinuxFilesystemImport` :134 (admit :143, handler :347); `metadata_extent` :516 (128 on LinuxArm64 host, else 144).
- How it decides: syscall numbers per target (x64/arm64) :154-172; `matching_host` requires call target == host target and Linux x64/arm64 (:41-49); `authority` requires `context.supplied` and `admit_services` (:51-60). Path loans are captured, NUL-terminated prefix extracted (`terminated_path` :21-39). Result narrowing: i128 -> i32 `narrow` rejects out of range (:405).
- Cases by shape: eight operations only; byte windows with `minimum: 1` for paths and `metadata_extent()` for stat buffers (:176-237).
- Same job elsewhere: `HI/filesystem.rs:201-358` (Darwin table) and `filesystem/windows/contracts.rs` (Windows table) do the same selection with different tables.
- Fail-open risks: `Operation::MkdirAt` branch does not check `structural_arguments[0].structural_type == layout.structural_type` while OpenAt/UnlinkAt do (:408,:420 vs :430); the generic loop check at :384-388 compares every structural argument's type to the signature formal, so a mismatched type is still rejected. Traced: covered by the earlier check.

## HI/filesystem/windows.rs (427 lines, read in full)
- Purpose: Windows synchronous file operations with a guest HANDLE table; rejects async, process pseudo-handles and pointer-valued optional parameters.
- Shape: `Handles {entries: BTreeMap<i64, State>}` :18 (`translate` :28, `acquire` :41); `Effect` :54; `Call` :68; `admit` :75; `Call::{path_argument,validate_path,arguments,result,output_scalar,readonly_record_argument,prepare_readonly_record}` :215-426; `translate_attributes` :158; free helpers `signed`, `handle_bits`, `scalar`, `directory_arguments`, `replace_handle`.
- How it decides: per-symbol argument whitelists keyed on the symbol bytes (`self.symbol`, :249-310), e.g. CreateFileW requires scalar args[2]==0 and args[5]==0 (inferred: the security-attributes and template-handle words, since the path is structural and not in the scalar list) and flags without bit 0x40000000 (inferred: the overlapped flag) :250-255; NtOpenFile access mask 1114240|1114241, share 7, options 2113568 :276-281; MoveFileExW flags `& !0x2b == 0` :306. Path buffers must be exactly 8192 bytes with a 0x0000 WCHAR inside :229. Guest handle 0 translates to native 0 even when not in the table (:34); -1 pass-through only for acquire results.
- Cases by shape: 24 Windows symbols (see contracts.rs); `_ => false` rejects unknown symbols (:309).
- Same job elsewhere: guest handle table duplicates `NativeDescriptor` logic in filesystem.rs:40,:672-738.
- Fail-open risks: `translate` maps unknown guest 0 to native NULL (:34). A guest value 0 reaching a native call becomes NULL handle; OS rejects. Disposition: traced as passing NULL only. `output_scalar` (:384-399) for Duplicate/Pipe acquires only if `status != 0` (success BOOL); for NT open acquires if status == 0 (NTSTATUS success), consistent.

## HI/filesystem/windows/contracts.rs (542 lines, read in full)
- Purpose: static table of 28 Windows entry contracts (kernel32/ntdll) with their `Formal` parameter recipes and `Effect` kind; checks the declaration shape and the Microsoft x64 calling plan.
- Shape: `Contract {effect, symbol}` :16; `select` :21 (table :32-446); `validate_entry` :475.
- How it decides: row picked by `matches_platform_c_symbol(binding, symbol, &[], Some(library))` (:449); then `matches_signature`. If the symbol matches but no row's signature matches, errors with the list of candidates (:459-470); if no symbol matches, `Ok(None)`. ReadFile/WriteFile get a special physical plan (five 8-byte words with the 2nd and 3rd merged into a 16-byte byte view) :493-526.
- Cases by shape: fixed table; e.g. NtQueryDirectoryFile only for classes 12 (8208 bytes) and 1 (8256 bytes) :350-353,:389-392; GetFileInformationByHandleEx only classes 0, 1, 9 (:171-214); SetFileInformationByHandle only classes 0, 6 and a disposition byte (:248-266, :444).
- Same job elsewhere: Darwin table filesystem.rs:201; Linux in linux.rs:154-237.
- Fail-open risks: none seen; unmatched rows are rejected or `Ok(None)`.

## HI/integer_absolute.rs (233 lines, read in full)
- Purpose: executes C `abs` (libSystem or msvcrt) either on a scalar i32 or on an owned one-cell structural carrier; uses `runtime_abi::host_integer_absolute`.
- Shape: `IntegerAbsoluteImport {signature, payload: Option<(StructuralTypeId, Vec<Path>)>}` :20; `admit` :26; handler :124.
- How it decides: symbol `abs` in `/usr/lib/libSystem.B.dylib`, `libSystem.B.dylib`, or `msvcrt.dll` (:32-40); native word must be 4 bytes aligned 4 (:99); calling application must match the binding plan, nonzero commitment, no opaque applications (:102-112). `i32::MIN` returns an error (`host_integer_absolute` None :221-223).
- Same job elsewhere: `integer_division.rs` and `integer_parsing.rs` share `native_word` helpers (read below).
- Fail-open risks: none seen. Error messages embed debug formats (`{:?}`, :175-184) but only for diagnostics.
## HI/integer_division.rs (109 lines, read in full)
- Purpose: executes C `lldiv` (libSystem or `api-ms-win-crt-utility-l1-1-0.dll`) returning quotient and remainder as a two-field record, via `runtime_abi::host_integer_division`.
- Shape: `type IntegerDivisionImport = scalar_record::ScalarRecordAdmission<2>` :17; `admit` :20; handler :51.
- How it decides: symbol match through `matches_platform_c_symbol` :26-31 (libSystem and the CRT dll named here; the helper at `host_imports.rs:215-225` (outside unit, read) adds Linux `libc.so.6` with GLIBC_2.2.5/2.17 itself, and returns false when the executing host cannot supply the target :202-204); both arguments must be signed 64-bit (:73-79); division by zero or overflow gives None -> rejection (:82-85).
- Same job elsewhere: shares `scalar_record::admit_c_word_record` with `integer_absolute`-like record imports (read below at scalar_record.rs).
- Fail-open risks: none seen.

## HI/integer_parsing.rs (305 lines, read in full)
- Purpose: executes C `strtol` over captured typed storage: parses the NUL-terminated input, returns the long, and writes the `endptr` reference output so it points to the byte cell where parsing stopped.
- Shape: `IntegerParsingImport` :24; `admit` :35; handler :189; `matches_locator` :269 (macOS libSystem `_strtol`, Linux `libc.so.6` `strtol` version GLIBC_2.2.5 (x64) / GLIBC_2.17 (arm64), and only when the target is the executing host :276,:283).
- How it decides: declaration must be exactly [Structural input, Structural output, Scalar i32 base] -> i64 (:55-73); one reference output to parameter 1 whose referent is an 8-bit non-address integer (:96-110); layouts must be unique per type (:123-147); `PointerOutputPlan::prepare(..., 8)` hardcodes 8-byte pointers (:148-153). At run time `parse_host_long` over the captured NUL-terminated prefix gives `(value, offset)`, then `byte_path_at(offset)` must resolve to exactly one captured byte cell (:247-251).
- Cases by shape: pointer size 8 only; base is i32; Windows has no entry (the `_ => return Ok(false)` :294).
- Same job elsewhere: pointer-output selection also appears in `native_provider/reference_outputs.rs:249-339` (`complete`) for generic native imports.
- Fail-open risks: none seen; unmatched target returns `Ok(None)`.

## HI/native_provider.rs (874 lines, read in full)
- Purpose: admits and runs an import through the "private native worker" (a separate process hosting shared-memory homes), using the produced native adapter product. Also hosts the Windows/Darwin filesystem and descriptor-IO variants (they flow through the same `NativeImport`).
- Shape: `ReadonlyRecord` :53; `Parameter {Scalar, Structural, ByteView}` :117; `ResultCodec {Unit, Scalar, Record, Reference}` :133; `NativeImport<'host>` :143 with `admit` :160; `admit_parameters` :387; `validate_scalar_shape` :370; handler impl :541-851 (`handle_effect_with_buffers` :597); `actual_trap` :853.
- Inputs/outputs: reads `TerminalRuntimeHostProvisions` (calling application, native foreign adapter product, borrowed storage layouts), a `NativeContext` (sponsor process handle), optional `FilesystemCall`. Calls `process.install`, `call`, `read_home`, `write_home`; produces `TerminalEffectResult` (Unit/Scalar/ScalarAggregate/CapturedReference/Crash).
- How it decides: admission keyed on identity of locator/target (`locator.target().native_target() == host` :178), `declaration.completion == NormalReturn` and no program-local root introductions (:208-216), calling application/product commitments equal (:228-246). Parameters admitted by native-telescope ordinal and semantic formal ordinal, each repeated or changed correspondence rejected (:412-436). `actual_trap` treats unix signals 4 and 5, or Windows status 0xc000001d / 0x80000003, as a guest `Crash(Trap)` (:857-867) and every other native failure as a rejection.
- Cases by shape: owned records need `record_shape::validate` (:492); borrowed access only Shared/Mutable (:505); the result may be unit, scalar, reference (a captured-input pointer with exactly the declared result regions, :312-325) or record (:330-342). Result reference regions must have empty path and `Parameter` origin whose parameter is a borrow (:318-323).
- Same job elsewhere: stdin/stdout modeling duplicates `descriptor_io.rs` (this file delegates: `modeled_descriptor` :156, :606-616); Windows file ops are routed from here through `FilesystemCall`. Pointer-output selection in strtol (`integer_parsing.rs:148`) vs `reference_outputs.rs:249`.
- Stated limits: ":9 The private worker supplies no additional service authority on its own".
- Fail-open risks: `if filesystem.is_none() { service_reach::admit_additional_services(...) }` (:186-207): when a filesystem call exists (Windows/Darwin filesystem or descriptor-io) the generic additional-service check is skipped and the filesystem path's own `admit_services` (filesystem.rs:427) is the only gate. That is by design per the call structure, "traced" to filesystem.rs:383-387 (Darwin, but only if descriptor is not `Io`!). For `DescriptorEffect::Io` (Darwin libSystem read/write) `namespace_admission` is only checked later in `arguments` (filesystem.rs:636) which runs for every native call through :637-641, but `modeled_stream` (descriptors 0..=2) calls bypass `arguments` (:606-616); so a Darwin read/write on std streams runs without the service check. Stated by test `modeled_stream_supply_does_not_authorize_native_descriptor_namespace` (filesystem.rs:770). Disposition: stated.

## HI/native_provider/original_homes.rs (692 lines, read in full)
- Purpose: one shared-memory "home" per interpreter root in the sponsor process, scalar backings that read/write through it, and pointer-slot linking/verification.
- Shape: `Sponsor {command, process, failure}` :36 (`with` :43 launches lazily with 30s timeout :55, `call` :69, `finish` :85); `NativeContext {sponsor, homes: Rc<RefCell<BTreeMap<(u64,u64), OriginalHome>>>, available, services}` :100; `OriginalHome` :107; `Resident` :117 implementing `TerminalScalarBacking`; `NativeContext::{prepare :185, prepare_scalar :306, reference_slot :391, record_reference :414, validate_references :437, loan :460, byte_loan :526}`; free `exact_layout` :587, `scalar_location` :603, `reference_location` :661.
- How it decides: homes keyed by `(root.namespace(), root.allocation())` (:211); an existing home must have the same root/type/extent/alignment or reject (:228-236); existing cells must keep offset, carriers and backing identity (`Rc::ptr_eq`) and current content equal the interpreter's value (:337-358). Scalar location resolved by matching the layout field path with FixedIndex repetition (:603-659). A loan's base is derived from every field's cell offset minus codec offset; all must agree (:471-493).
- Cases by shape: byte owners (no layout) must have unsigned 8-bit cells with `FixedIndex` single-segment paths (:318-331) and no reference slots (:201); reference slots must be 8 bytes aligned 8 (:674-676).
- Same job elsewhere: `scalar_location`'s path matching duplicates the layout expansion in `borrowed_storage/geometry.rs:65-163` (expand) in a different form.
- Fail-open risks: `Sponsor::finish` returns `Ok(())` without closing when a prior failure exists (:86-88): failure is already recorded so not accepting a bad run; "traced" (the failure string is returned by later `with` calls). `Sponsor::call` uses `.expect("installed native process")` (:77) - a panic path if called before `with` launched; callers always call `install` first (native_provider.rs:344-348, inferred).

## HI/native_provider/record_fields.rs (358 lines, read in full)
- Purpose: derives the full list of scalar leaves and reference leaves of a record or fixed-array structural type (paths and logical types) with an explicit-stack walk (no recursion).
- Shape: `SemanticScalarField`, `SemanticReferenceField`, `SemanticFields` :15-41; `Schema::derive` :91; `scalar_type` :244; `fields` :258.
- How it decides: rejects quotient carriers (:114), AccessView/Accessor record storage (:128-136), and any shape other than primitive scalar, reference, record, fixed array with literal length (:152, :220). Erased fields (`relevance.is_erased()`) are skipped (:140,:182). Cycle detected when a node's `shape` is still None on re-encounter (:101-105).
- Same job elsewhere: leaf-path enumeration is also done by `record_shape.rs` (:409-762) and `borrowed_storage/geometry.rs:65` from the layout side.
- Fail-open risks: none seen.

## HI/native_provider/record_shape.rs (781 lines, read in full)
- Purpose: checks that an owned aggregate's public ABI graph (`BoundarySignatureGeometry`) matches the semantic declaration and the storage codec exactly, and offers small structural matchers used by the OS-entry tables.
- Shape: `validate` :409; `validate_instances` :685; `matches_signed_record_array` :98; `matches_integer_storage` :180; `matches_integer_record` :236; `IntegerRecordField {Scalar, Array, ReferenceRecord, ReferenceArray}` :170; `Schema`/`SchemaVisit` :36-96.
- How it decides: public class vs semantic shape (Integer/Float, Record{first_field,field_count}, FixedArray{element,length}) :64-74,:478-486,:542-550; child intervals must fit and not overlap :513-524; leaf count equals `codec.fields.len()` :671; every leaf's full path, offset and logical type compared in order :709.
- Cases by shape: only plain scalar, finite record, fixed array of literal length (`_ =>` rejects at :579-583,:614-618); `matches_signed_record_array` requires all words same signed width (:107-141); `matches_integer_record` requires reference fields to be 8 bytes aligned 8 and checks nested expected shapes (:263-322).
- Same job elsewhere: `record_fields::fields` (above) and `values.rs` `StorageCodec::admit` also walk the same schema.
- Stated limits: the test file name lists "cycle rejection" at tests.rs:3.
- Fail-open risks: none seen; the matchers return `false` on any unknown shape (so the caller treats the import as not matching).

## HI/native_provider/record_shape/tests.rs (651 lines, read in full)
- Purpose: unit tests for `validate` using a hand-built fixture of nested records, arrays, zero-length arrays, erased fields, cycles and authored stored widths. No production code. 15 `#[test]` fns (Grep count mode on the file = 15).

## HI/native_provider/reference_outputs.rs (354 lines, read in full)
- Purpose: matches pointers the native worker produced (in output slots or returned values) to the captured typed endpoints of the current invocation, then tells the interpreter which input reference they correspond to.
- Shape: `Capture` :20; `CapturedInput` :28; `ReferenceOutputs {types, parameters, outputs, result_regions, codecs: BTreeMap<StructuralTypeId, StorageCodec>, captures}` :35 with `admit` :45, `capture` :71, `input_reference` :84, `anchored` :156, `returned_reference` :191, `complete` :249; `covers` :350.
- How it decides: pointer equality only selects a candidate (`pointer_matches`, `returned_pointer_matches`) and then interpreter-side checks (header :1-4); a candidate must have equal referent type (`capture.view == field.referent`), access that `covers` the requested (equal, or Mutable covering Shared) :350-354, and a lifetime anchor (`BoundaryLifetimeDependency::Referent` or `Stored`) :156-189. The first matching capture wins (`break` at :314).
- Same job elsewhere: strtol endptr selection `integer_parsing.rs:247-261` uses byte offset instead of pointer equality.
- Fail-open risks: `ReferenceOutputs::admit` drops layouts whose codec fails to admit (`filter_map ... .ok()` :61-65) silently; a later lookup failure then rejects (":283 no exact produced codec"), so a bad layout leads to rejection not acceptance: traced.

## HI/native_provider/service_reach.rs (193 lines, read in full)
- Purpose: decides whether a native import may reach services beyond its own owning trait: only under `MatchingHost` delegation and only when every reached service and its parents belong to the same declaring package.
- Shape: `admit_additional_services` :31; `package_owner` :11; `identity_path` :17; tests :87-193 (6 tests).
- How it decides: keyed on the identity text of each service: the owner is the substring before the first `::` and must start with `"package:"` (:9-14). So a service's package is judged by identity spelling.
- Cases by shape: delegation is `MatchingHost` only (:44); owning service must be package-owned (:53-63).
- Fail-open risks: none seen (unknown service -> refused list :66-69).

## HI/native_provider/values.rs (295 lines, read in full)
- Purpose: `StorageCodec`: the byte layout (scalar fields and 8-byte reference slots) of a record/array/primitive/reference type, built from the layout recipe plus the semantic roster, with encode/decode.
- Shape: `ScalarField`, `ReferenceField`, `StorageCodec` :17-37; `admit` :40; `encode` :200; `decode_fields` :242; `aggregate` :270.
- How it decides: layout must be unique (via `exact_layout`), carry no discriminators (:46), shape in {Record, FixedArray, PrimitiveScalar, Reference} (:72-82); each semantic leaf needs exactly one layout recipe at its path with equal logical type (:105-113); overlap/extent checks (:116-127); reference slots must be 8 bytes aligned 8 and not overlap others (:153-166); number of opaque regions must equal number of references (:178).
- Same job elsewhere: field-overlap geometry duplicates `borrowed_storage.rs:246-259` coverage logic (suspected same intent, different data).
- Fail-open risks: `encode`/`aggregate` refuse when references exist (:204,:274).
## HI/native_word.rs (172 lines, read in full)
- Purpose: verifies that a native ABI shape is a 'word envelope' (an integer reached through chains of single-field records at offset 0) and finds the scalar leaf type/path of an owned single-payload record.
- Shape: `NativeWordEnvelope {byte_size, alignment, contains_record}` :9; `native_word_envelope` :15; `scalar_leaf_integer` :62; `scalar_payload_path` :118; `scalar_payload` :128.
- How it decides: each node must share extent and alignment with the root (:33); record nodes must have `field_count: 1` and offset 0 (:38-50); cycle guarded by a loop bound of `shapes.len()` (:28) and by `visiting` lists (:67,:136). Atomic fields or fields with authored `storage` are refused (:90-96, :155-160).
- Callers (grepped names in unit): `integer_absolute.rs:90,97`, `network_address.rs:221`, `reference_state.rs:183,208,251`.
- Fail-open risks: none seen.

## HI/network_address.rs (431 lines, read in full)
- Purpose: host implementations of libc `inet_makeaddr` and `inet_lnaof`. The C return value (`HostInternetAddress`) is kept in an invocation-scoped store and the guest only holds an opaque minted token.
- Shape: `AddressOperation {Make, Host}` :35; `OwnedAddress` :40; `AddressContract` :46; `AddressValues {values: Vec<Option<OwnedAddress>>}` :53 with `argument` :58; `NetworkAddressImport` :81 (`admit` :89); `select` :109; `validate` :128; handler :274; one test :378.
- How it decides: symbol `inet_makeaddr` / `inet_lnaof` via `matches_platform_c_symbol` with libSystem and `runtime_abi::WINDOWS_INTERNET_PROVIDER_LIBRARY` (:112-123). Token identity is `u64::MAX - ordinal` (:319-322, :354); the stored application commitment and multiplicity must equal the contract's, and affine/linear values are removed on use while unrestricted values are copied (:72-77). Slots are never recycled (`values.push`, :328).
- Cases by shape: exactly two u32 scalars -> structural result, or one owned structural arg -> u32; the carrier must be a four-byte C record ending in u32 (`contains_record` required :222); exactly one opaque application with non-zero commitments (:227-231).
- Fail-open risks: none seen; a stale or wrong token errors (`argument` :67-71).

## HI/numeric.rs (378 lines, read in full)
- Purpose: executes admitted IEEE-754 realizations (arithmetic, comparisons, classification, conversions, directed rounding, min/max, negate, int<->float) using the shared `numerics` crate semantics; trapping float-to-int returns `Crash(Trap)`.
- Shape: `NumericImport {signature, realization: NearestIeeeFloatArithmeticRealization, classification}` :23 (`for_import` :36); handler :96; `evaluate` :169; `directed_square_root` :328; `directed` :339; `encode` :369.
- How it decides: the operation is whatever `realization.operation` says (an enum from `target_operations`); classification result must satisfy `conventional_float_class_layout` (:51); truncation results go through `to_i64`/`to_u64` and fail if out of range (:270-284) - so 128-bit targets cannot come out of this path; `Minimum`/`Maximum` pick operand 0 when `less`/`greater` holds else operand 1 (:219-226) so a NaN case returns operand 1.
- Cases by shape: `TruncateToInteger` only up to 64 bits (:273-282); classification result is a 5-case sum with 4 fields (`cases[index + 1].fields[0]` :67).
- Fail-open risks: none seen. Note `Negate` flips the sign bit even for NaN (:211-217), payload-preserving by the header comment.

## HI/reference_state.rs (279 lines, read in full)
- Purpose: zero-argument provider-state references: `__errno_location` (mutable i32 cell starting 0), `gnu_get_libc_version` (shared cell holding value 2), `localeconv` (record of shared references each to a byte holding 46, ASCII '.'), materialised without FFI addresses.
- Shape: `ReferenceStateImport` :27 (`admit` :33); `select` :82 (`matches_platform_c_symbol(binding, symbol, &[], None)` :99, so only Linux ELF locators match since there are no darwin install names and no windows library); `validate` :106; `scalar_state` :202; `locale_view` :219.
- How it decides: access is fixed by operation (mutable for errno, shared for the others) :133-138; the referent record's every relevant member must be a shared reference to a byte-integer carrier and each gets the value 46 (:233-269), regardless of which libc `lconv` member it is. The libc version cell is `2` of whatever integer leaf the referent has (:185-186).
- Cases by shape: three symbols only (:85-98).
- Fail-open risks: `application` is `Option` and the calling-application checks run only `if let Some(application)` (:159-167); when None the application checks are skipped. Disposition: traced: the caller passes `provisions.and_then(|p| p.host_calling_application(*slot))` (`host_imports.rs:512-516`, read), so the application is `None` when no provisions are supplied; the checks at :159-167 are then skipped, while `admit_exact_foreign_call` (:158) still runs. The four sibling adapters (`integer_absolute.rs:63`, `integer_division` via `scalar_record.rs:92`, `network_address.rs:136`) return an error when the application is `None`; `reference_state` is the one that tolerates it.

## HI/scalar_codec.rs (127 lines, read in full)
- Purpose: scalar <-> little-endian byte conversion with explicit stored and logical types.
- Shape: `scalar_extent` :12, `encode` :29, `decode` :63, `integer_value_with_sign` :114.
- How it decides: boolean decode accepts only 0 or 1 (:72-76); integers up to 128 bits and multiple of 8, non-address (:17-24); encode requires logical bits >= stored bits and re-checks admission in the stored type (:44-52); decode sign-extends from stored sign (:95-99) then converts sign to the logical type (:101).
- Same job elsewhere: used by `borrowed_storage.rs:15-16`, `original_homes.rs:12`, `values.rs:7`, `native_provider.rs:30`.
- Fail-open risks: none seen.

## HI/scalar_record.rs (342 lines, read in full)
- Purpose: shared admission of C records made of N 64-bit scalar words (used for `lldiv` = 2 words) checking semantic fields, native parameter order, per-word geometry and the calling plan.
- Shape: `ScalarRecordAdmission<const FIELD_COUNT>` :17; `admit_c_word_record` :23; `word_class` :230; `has_c_scalar_storage` :242; two tests :264, :310.
- How it decides: every field must be relevant, unrestricted scalar/float with storage `None`, `Scalar{byte_offset == ordinal*8}`, or `Direct` at that offset (:242-257); native shapes must be 8 bytes aligned 8 with class Integer (64-bit non-address) or Float (Binary64) (:230-239).
- Cases by shape: 64-bit integer or binary64 words only (:232-235); `TargetLayout` storage refused (:60); `Repeated`/`Fragmented` storage refused (:255).
- Users: `integer_division.rs:37` only (grep: `admit_c_word_record` is called from 1 non-test site in this unit; suspected same elsewhere not grepped).
- Fail-open risks: none seen.

## HI/standard_input.rs (143 lines, read in full)
- Purpose: the hosted `read byte` import over the finite stdin snapshot: one `read` call, EOF case, one-byte case, or `Crash(Trap)` on any other outcome.
- Shape: `ReadByteImport<'input>` :24 (`for_import` :35); handler :97.
- How it decides: result must be a 2-case sum [empty, byte] with the byte case carrying one signed 32-bit field bounded to include 0 and 255 (:52-81), result multiplicity Affine. At run time `read` returning anything other than Ok(0) or Ok(1) (including `Err`) returns Crash(Trap) (:130).
- Same job elsewhere: `descriptor_io.rs:534-576` also reads from the same finite stream type (`Arc<Mutex<Box<dyn Read + Send>>>`) for descriptor 0.
- Fail-open risks: none seen. A reader error becomes a guest trap, stated by header (:3-4).

## src/execution/native_code_layouts.rs (62 lines, read in full)
- Purpose: builds the interpreter's native-code-layout environment from a retained native image's layout inspection.
- Shape: `inspect` :11.
- How it decides: returns `Ok(None)` when no `native_image` is present (:16-18); requires the image's psi artifact identity and target to equal the request's (:28-34); keeps layouts with `private_function.is_none()` and `terminal_psi == semantic` (:40).
- Fail-open risks: none seen.

## src/lib.rs (42 lines, read in full)
- Purpose: crate root and re-exports; `#![forbid(unsafe_code)]` :26; `runtime_boundary` module compiled only for tests :30-31; header explains why runtime integration is separate from `terminal-interpreter` and `compiler` (:13-24).
- Public re-exports: `execute`, `execute_sliced`, `bind_admitted_intrinsics`, clock/delegation/request types, `capture_host_x86_flags`, `run_native_provider_worker_if_requested`, memory ordering and execution result types.

## src/runtime_boundary.rs (27 lines, read in full)
- Purpose: two source-text tests: (1) `include_str!("execution.rs")` must contain `TerminalProgram::borrow(artifact)` and must not contain `TerminalProgram::load(` or `semantic_bytes()` :6-9; (2) compiler/cli Cargo.toml and lib.rs must not reference runtime crates :14-26.
- How it decides: by substring search over source text (reads files with `include_str!`).

## tests/retained_runtime.rs (91 lines, read in full)
- Purpose: integration test: builds `tests/omega/pass/calls/machine_self_call_recursion_compile/main.omg` for the host target into a retained native artifact, runs `execute` and `execute_sliced` (work_budget 100_000, slice size 1) and asserts: wrong target gives `InvalidProvision("runtime provisions name another deployment target")`, both runs complete with `Unit`, no stdout/stderr, `work_suspensions > 0` for sliced, and `terminal_codec::module_decode_count()` is unchanged (:63,:90).
- How it decides: asserts on exact error message string (:71-73).
---------------------------------------------------------------------
# Part B: omega/packages/build-declarations/src (paths relative to `omega-rust/omega/packages/build-declarations/src/`, written BD/)

Crate-level shape (lib.rs, read): two ways to learn a project's role from `build.omg`: interpreted Build activations (`BuildRequest`) and a "conservative syntax projection" that reads direct `Build::<op>(builder, ...)` statements without evaluation (BD/lib.rs:5-18).

## BD/build_declaration.rs (151 lines, read in full)
- Purpose: validated project role types and name newtypes.
- Shape: constants `BUILD_FILE_NAME` "build.omg" :12, `BUILD_TYPE_NAME` "Build" :16, machine-name constants :14-26; `ProjectName(String)` :29 (`parse` :36); `WorkspaceMemberPath(String)` :57 (`is_valid` :60); `BuildDeclaration {Package, Application, Workspace}` :104; `PackageDeclaration {name}`, `ApplicationDeclaration {name, artifact_only}` :143, `WorkspaceDeclaration {members}` :149.
- How it decides: `ProjectName` valid iff `is_snake_case` (syntax_projection/mod.rs:440-459: first byte lowercase ASCII, no trailing `_`, no `__`, bytes lowercase/digit/`_`); `WorkspaceMemberPath` valid iff non-empty, no leading/trailing `/`, no `\`, no control bytes, no empty or `.`/`..` components, every byte alphanumeric or `.`/`-`/`_` (:60-71).
- Fail-open: none seen. Observation: error text for the name rule says "package identity ... snake_case" (:42) while two diagnostics call the expected name `"kebab_name"` (declaration_errors/mod.rs:87,96,135); the tests reject hyphenated names (tests/mod.rs:229-234). Recorded as a text inconsistency only.

## BD/build_request.rs (29 lines, read in full)
- Purpose: evaluated requests produced by running `build.omg`: `BuildRequest {Package, Application, Member, ArtifactOnly, Dependency{purpose, alias, source}}` :9; `BuildDependencySource {Path(String), Git{repository, revision, package: Option<ProjectName>}}` :22.
- Users: grep `BuildRequest` is used heavily elsewhere (not counted here); within unit see package-compilation entries below.

## BD/declaration_errors/mod.rs (175 lines, read in full)
- Purpose: one enum `BuildDeclarationError` (:9-48, 40 variants) and its `Display` text (:50-173). Pure data; no logic.

## BD/dependencies/mod.rs (333 lines, read in full)
- Purpose: row grammar for dependency statements `Build::depend`, `depend_as`, `build_depend`, `build_depend_as` in the root build entry.
- Shape: `DependencyPurpose {Product, Build}` :26; `DependencyOperation` :47 (`classify(&str)` :62); `CONDITIONAL_DEPENDENCY_CALL_NAMES` :93 (four `*_when` names); `is_dependency_call_name` :100; `DependencyRow` :106; `DependencyRowError` :136; `project_dependency_rows` :156; 4 tests :221-332.
- How it decides: keyed on the call target's source spelling (`"depend"`, `"depend_as"`, `"build_depend"`, `"build_depend_as"` :62-69) for statements in the root entry; operands via `direct_build_call_operands`; alias rows need exactly 2 operands, others exactly 1 (:175-185). Conditional `*_when` names are recognised only by `is_dependency_call_name` and are never rows (test :296-319).
- Cases by shape: "direct unconditional" statements only; statements that are not `StatementNode::Call` are skipped (:163-165), as are calls whose target is not one of the four names (:166-169).
- Same job elsewhere: `package-manager/src/declarations/dependencies/read/{policy.rs, projection/mod.rs, projection/calls.rs}` use these names (grep combined count for `project_dependency_rows|package_authored_type_name|project_build_declaration_in_entry|is_dependency_call_name` = 23 lines across 6 files; 6 of those in package-manager; suspected same-job reading there not read).
- Fail-open risks: dependency calls nested in other states or expressions are not seen by `project_dependency_rows` (it iterates only the entry's statements :162); whether another check rejects them is "open" for this crate (the header says callers reject conditional calls :12-14; unconditional hidden dependency calls inside helpers are not rejected here).

## BD/direct_build_call.rs (43 lines, read in full)
- Purpose: the single shared check for `Build::<op>(builder, ...)`: static call, receiver path is exactly `[Build]`, no machine/evidence/operational/discard modifiers, first operand is the name `builder`.
- Shape: `DirectBuildCallError {WrongReceiver, WrongArguments}` :12; `direct_build_call_operands` :17.
- How it decides: source spelling of identifiers: `BUILD_TYPE_NAME` and `BUILDER_PARAMETER_NAME` (:22,:38). Users: `syntax_projection/mod.rs:23,215` and `dependencies/mod.rs:23,171` (2 callers, read).

## BD/lib.rs (64 lines, read in full)
- Purpose: crate docs (:1-35) and public re-exports (:49-64). `#![forbid(unsafe_code)]` :37.

## BD/prelude.rs (421 lines, read in full)
- Purpose: holds the toolchain-authored `Build` API as Omega source text (`BUILD_PRELUDE` :20-421) and `build_prelude_source()` :12 which splices target and optimization catalogs into it; `default_build_source()` :16 returns `"machine build(builder: &mut Build) {}\n"`.
- Contents of the text (read): data `Subsystem`, `CompositionMode`, `CrashCause`, `PhysicalAuthorityClass` (14 cases :45-60), `X86DeploymentFeatures`, `BuildPath/BuildSource/BuildInputs/BuildOutput/BuildLog/BuildProduct/ProductEntryRef`, boundary traits `BuildProductOperations` :98, `BuildBindingOperations` :110, `BuildRequestOperations` :240, `BuildFilesystemOperations` :316, `Build` data (fields :211-229), `Source` and `PackageSelection`, public `Build::depend/...` wrappers :249-291, `Build::exclude_crash`, `Build::exclude_physical_authority`, `Build::accept_component_assumption` with empty bodies :298-313 ("the declared body is the evaluator no-op" :310-311).
- How it decides: n/a (data). Slot strings: `// compiler-owned target declarations` (:65 replaced by targets.rs), optimization slots :66, :419, :420.
- Same job elsewhere: `build_exclusion_physical_code` :126-143 maps the 14 physical-authority cases to numeric codes 0..13 by hand; the same ids appear in `toolchain::Filesystem...` service identity strings checked in HI/filesystem.rs:449-454 (names only partially coincide; suspected duplicate vocabulary, not compared further).
- Stated limits: ":298-303 `Build::exclude_crash` ... body empty": semantics live in the evaluator ("typing normalizes the declaration to the authenticated selection protocol" :296).
- Users: `build_prelude_source` 22 matched lines in 7 files (Grep count; 12 in `psi/interpreter/checked-interpreter/src/build_machine.rs`); `default_build_source` has 1 external caller (`package-compilation/src/source/source_assembly.rs`).

## BD/prelude/optimizations.rs (63 lines, read in full)
- Purpose: generates the `Optimization` data cases, `Optimizations {human_report, selected: [u8 in Saturating; N]}`, `optimization_selection_slot` and `Optimizations::enable` from `optimization_core::Optimization::ALL`.
- How it decides: `assert_eq!(base.matches(slot).count(), 1)` for each of three slot comments (:17-23); panics otherwise.

## BD/prelude/targets.rs (37 lines, read in full)
- Purpose: generates `BuildTarget` cases, one `public const` per `ProductTarget::ALL` (canonical name), and `build_selection_target` mapping each to `target.code()` from `language_semantics::declaration_selection::ProductTarget`.
- How it decides: `prelude.replace("// compiler-owned target declarations", ...)` replaces all matches without counting (:36), unlike optimizations.rs which asserts exactly one.

## BD/syntax_projection/mod.rs (463 lines, read in full)
- Purpose: finds the root `machine build(builder: &mut Build)` and reads `Build::{package,application,member,artifact_only}` statements; refuses any other role call shape anywhere in the file.
- Shape: `BuildDeclarationSyntaxProjection` :35, `BuildEntrySyntaxProjection` :41; `project_build_declaration_from_source` :74; `project_build_entry_syntax` :93; `project_build_declaration_syntax` :181; `project_build_declaration_in_entry` :195; `DeclarationOperation` :302; `reject_authored_toolchain_vocabulary` :348; `package_authored_type_name` :378; `reject_unprojected_build_declaration_syntax` :420; `is_snake_case` :440; `is_portable_path_byte` :461.
- How it decides: by source spelling: machine leaf name `build` (:101, `machine_leaf_name` takes text after the last `::` :390), parameter named `builder` and not const, type `&mut Build` with an exclusive+readable access and no lifetime, referee named `Build` (:152-173); role calls matched on call target text `package|application|member|artifact_only` (:310-316). Hidden role calls are caught by scanning every expression node in the file for a call with a role target whose argument-handle slice is not among `accepted` slices (:424-436), i.e. keyed on slice equality of expression handles.
- Cases by shape: exactly one kind among package/application/workspace (:263-268); `artifact_only` allowed only with an application (:270-274,:290-294); `artifact_only` is accepted before or after the application (tests :247-265); duplicates counted (:247-261).
- Same job elsewhere: dependency calls are projected separately in dependencies/mod.rs but `reject_unprojected_build_declaration_syntax` checks only role names, not dependency names.
- Fail-open risks: a role-named call whose target text equals e.g. `member` but belongs to another receiver in a helper is rejected (fail-closed). A call that happens to have the same argument-handle slice as an accepted one would pass (`accepted.contains(&arguments)` :433); slices are handle ids into the expression table, and the comment at :9-15 says the accepted call is "the unique occurrence" with that slice. Disposition: stated.

## BD/tests/mod.rs (339 lines, read in full)
- Purpose: unit tests over `project_build_declaration_from_source` covering success shapes, error variants, spoofed vocabulary and artifact_only modifiers. Test count by Grep count mode on `#[test]` = 13.
---------------------------------------------------------------------
# Part C: omega/packages/build-output/src (paths relative to `omega-rust/omega/packages/build-output/src/`, written BO/)

Crate-level (lib.rs, 54 lines, read in full): `#![forbid(unsafe_code)]` :36; `#![cfg_attr(windows, feature(windows_by_handle))]` :37 (nightly feature gate on Windows); `digest_encoder` re-exported from `optimization_core` :39; `tree_from_entries` compiled only for tests :45-46. Docs :1-34 state that `BuildStagedOutputTree` fields are private and commitments recomputed on reconstruction (:13-17).

## BO/capture/mod.rs (297 lines, read in full)
- Purpose: walks a sponsored private output directory on the host and produces a `BuildStagedOutputTree` (bytes, executable flag), refusing anything but directories and regular files.
- Shape: `CapturedFileContent` :30, `StagedOutputEntryKind` :35, `StagedOutputEntry` :40; `capture(root, sponsor)` :45; `capture_file` :159; `has_single_link`, `is_executable`, `same_file_observation` per-OS :242-297.
- How it decides: symlink and non-regular entries rejected (:112-116, :144-148); entry count ceiling = min(sponsor `maximum_entries`, `MAX_STAGED_OUTPUT_ENTRIES`=4096) (:82-85); per-file `maximum_object_extent`; total bytes ceiling = min(256 MiB, sponsor limit) (:129-142; note this sums file lengths without de-duplication while the commitment counts unique content); a file must have link count 1 and identical identity/metadata (dev, ino, len, mode, mtime, ctime on Unix; volume serial, file index, len, attributes, last-write on Windows) before and after the read (:178-189, :227-235).
- Cases by shape: on Windows `is_executable` is always false (:262-265); on non-unix non-windows `has_single_link` and `same_file_observation` are always false (:252-255, :294-297), so capture refuses every file there.
- Same job elsewhere: link-count / identity checks are not repeated in materialization (it writes with `create_new`).
- Stated limits: header :9-15 says capture "refuses symlinks, hard-link aliases, and mid-read mutations".
- Fail-open risks: none seen. Note `String::from_utf8_lossy` only in diagnostics (:165).

## BO/captured_source/mod.rs (263 lines, read in full)
- Purpose: immutable inventory of a source root (directories, file bytes+digest, symlink target spelling) checked against a `CanonicalFilesystemMetadataIndex`.
- Shape: `CapturedSourceEntry {Directory, File{bytes,digest,executable}, Symlink{target,digest}}` :26; `CapturedBuildSourceInput {canonical_source_metadata, entries: Arc<BTreeMap<Vec<u8>, CapturedSourceEntry>>, file_bytes}` :127 with `from_capture_rows` :135 (marked `#[doc(hidden)]`), `is_subset_of` :208, `entries` :214, `file` :245.
- How it decides: each entry's path must be canonical and listed in the index, with equal kind, file executable flag and byte length (`matches_row_kind` :64-84), no duplicates (:170), and every non-root index row must be present (:176-188). `is_subset_of` compares exact `CapturedSourceEntry` values (bytes and digests), not lengths (:208-212; header :10-14).
- Callers outside the crate (grepped `CapturedBuildSourceInput::from_capture_rows`, uncapped Grep): package-compilation `source/source_snapshot.rs:260,326,361` and `build_execution/filesystem_scope/runtime.rs:831` (4 call sites).
- Fail-open risks: none seen. Note the digest stored in a `CapturedSourceEntry::File` is computed in `file()` (:43), but `from_capture_rows` accepts entries built directly via the enum variant with any `digest` (the enum fields are public); `matches_row_kind` ignores `digest` (:68-75). Disposition: open (not traced whether a caller constructs variants directly).

## BO/captured_source/tests.rs (136 lines, read in full)
- Two tests: subset compares bytes not only paths/lengths (:66-95); entries listed in canonical unsigned byte order with root row counted in `entry_count` but not listed (:97-136).

## BO/materialization/mod.rs (356 lines, read in full)
- Purpose: write a retained tree into an existing empty host directory (directories, `create_new` files, relative symlinks); plus a test-only validator `validate_retained_tree` (:74-161).
- Shape: `materialize_retained_tree` :28; `validate_empty_destination` :163; `retained_native_path` :197; per-OS `set_materialized_file_mode` :275-299 (0o755 / 0o644 on Unix; non-Unix rejects executable files) and `create_materialized_symlink` :315-339 (non-Unix rejects every symlink).
- How it decides: destination must be an existing concrete directory with no children (:172-194); each path re-validated as canonical UTF-8 slash path with portable components (:197-221).
- Cases by shape: `validate_retained_tree`, `reserve_materialization_path_bytes`, `validate_retained_executable_mode`, `validate_retained_symlink_materialization` are all `#[cfg(test)]` (:74, :233, :261-273, :301-313): the production build does not run the strict-order, ancestor-directory, symlink-target-canonicality or commitment-recompute checks in this function at materialization time; production relies on tree construction (`from_retained_parts`, `capture`, `select_files`).
- Fail-open risks: `materialize_retained_tree` does not recompute the commitment of the tree it writes, it returns `tree.commitment` (:71). A tree whose fields were built only through the crate's constructors carries a recomputed commitment (staged_output_tree.rs:169-176), so the risk is "traced" as bounded by the private-field construction (lib.rs:13-17); a wrong entry order would make `create_dir`/`create_new` of a child before its parent fail, i.e. error not accept.

## BO/portable_paths/mod.rs (154 lines, read in full)
- Purpose: canonical relative path and symlink-target rules shared by capture and materialization; Windows-portability restrictions applied on every host.
- Shape: `canonical_relative_path` :14; `canonical_symlink_target` :45 (`#[cfg(test)]`); `validate_portable_component` :87; `reserve_path_bytes` :129; one test :148.
- How it decides: component bytes < 0x20 or one of `\ : * ? " < > |`, trailing `.` or space, empty/`.`/`..`, and reserved device stems CON, PRN, AUX, NUL, COM1-9, LPT1-9, CONIN$, CONOUT$ compared case-insensitively on the part before the first `.` (:91-119). Path total ceiling 16 MiB via `MAX_STAGED_OUTPUT_PATH_BYTES` (:135).
- Same job elsewhere: `WorkspaceMemberPath::is_valid` in `build-declarations/src/build_declaration.rs:60-71` implements a different portable-path rule: components may use only ASCII alphanumerics and `.`, `-`, `_` (`is_portable_path_byte` syntax_projection/mod.rs:461-463) and must not be `.`/`..`/empty, but it does not reject trailing dots or reserved device names such as `CON` or `NUL`. Two portable-path rules with different strictness (both read).
- Fail-open risks: none seen.

## BO/staged_output_tree.rs (524 lines, read in full)
- Purpose: the canonical retained tree type, its domain-separated commitment, subset selection (`select_files`, `select_included_sources`), and byte-size ceilings.
- Shape: constants :34-52 (domain `OMEGA-BUILD-STAGED-OUTPUT-TREE\0`, schema version 2, ceilings 4096 entries / 256 MiB unique bytes / 16 MiB path bytes); `BuildStagedOutputTreeCommitment {digest, entry_count, file_bytes}` :55; `BuildStagedOutputTree {commitment, entries}` :85 with `from_retained_parts` :130, `materialize_into` :199, `select_files` :234; `PackageGeneratedSource` :91; `select_included_sources` :369; `finish_commitment` :443; `commitment_for_retained_entries` :466.
- How it decides: `from_retained_parts` checks only canonical path form (via `filesystem_authority::canonical_filesystem_metadata_path_is_canonical`), no duplicates, and that the recomputed commitment equals the expected one (:136-176). It does not check that parent directories exist or that symlink targets are canonical (those checks are test-only, see materialization above). `select_files` requires each requested path to exist, be a non-executable regular file, not repeat, and pulls in every ancestor which must be a directory (:234-292). `select_included_sources` keys on file-name spelling: every file whose last component ends in `.omg` with length > 4 must be listed (:377-387); listed names must be `.omg`, not `build.omg`/`main.omg`, regular and non-executable (:395-432).
- Cases by shape: file commitments dedupe by (content digest, length) (:487-491); symlinks are hashed by target bytes (:501-505).
- Same job elsewhere: `tree_from_entries.rs` (test-only) recomputes a tree with parent-before-child validation that production `from_retained_parts` lacks.
- Fail-open risks: `from_retained_parts` accepts a tree with a missing parent directory or a symlink whose target escapes the output root, as long as the commitment matches the recorded one (:136-176 vs test-only :74-161). What reaches it: decoded persistence rows from `package-compilation/src/build_execution/build_record_codec.rs:233` (the only non-test caller grepped). Disposition: traced to that one caller; whether a malformed but commitment-matching tree can exist depends on how the commitment was originally computed (it was computed over the same entries, so a malformed tree would have to have been committed that way). Open.

## BO/staged_output_tree/tests.rs (18 lines) and tests/file_selection.rs (76 lines), read in full
- Fixture `baseline_tree` (6 entries) and 2 tests for `select_files` (ordering, ancestors, `Arc::ptr_eq` byte sharing; rejection of missing/dir/duplicate/non-canonical paths).

## BO/tree_from_entries.rs (311 lines, read in full)
- Purpose: test-only constructor `from_entries` building a tree from in-memory entries, requiring parent directories first, checking namespace collisions, then calling `validate_retained_tree`.
- Shape: `OutputTreeEntry` :20; `from_entries` :75; 6 tests :209-310.
- How it decides: ceilings (4096 entries, 256 MiB, 16 MiB) and `retained_native_path`/`canonical_symlink_target` (:95, :165).
- Same job elsewhere: parent-before-child and symlink-target canonicality checks exist only here and in `validate_retained_tree`, not in production constructors.
---------------------------------------------------------------------
# Part D: omega/packages/package-compilation (paths relative to `omega-rust/omega/packages/package-compilation/`, written PC/)

## PC/build.rs (27 lines, read in full)
- Purpose: build-script custody of the bundled library root. If `OMEGA_RELEASE_LIBRARY_SHA256` is set it must be 64 hex digits and is exported; otherwise it searches ancestors of `CARGO_MANIFEST_DIR` for a directory containing `source/library/core` and exports `OMEGA_DEVELOPMENT_LIBRARY_ROOT` (:17-25); panics with `expect` if not found.

## PC/src/build_execution.rs (178 lines, read in full)
- Purpose: module root for the pre- and post-execution boundaries of `Build` evaluation; `CapturedBuildObservations` collects filesystem attempts, included-source handoffs, output obligations/receipts and log, then `settle` verifies against the staged output tree.
- Shape: `CapturedBuildObservations<'scope>` :67 (`new` :105, `retain_filesystem_effects` :81, `retain_build_log` :121, `settle` :131); `BuildObservationSummary::select_generated_sources` :168; free `message` :63.
- How it decides: `retain_filesystem_effects` rejects effects from another scope (`effects.scope != *self.scope` :85) and a second retention; `settle` errors if filesystem host state was observed without admitted reach (:138-142), then `verify_required_outputs` and `settle_build_output_obligations`.
- Fail-open risks: `retain_build_log` rejects a second retention only when the first log was non-empty (:122): retaining an empty log twice is accepted; harmless to content. Disposition: traced (no data loss).
- Unused code check: `message` fn :63 (private); grep not run (private, same file).

## PC/src/build_execution/build_admission.rs (156 lines, read in full)
- Purpose: choose the `build` machine and admit it: at most one build machine; it may reach only compiler-owned `Build` protocols (project requests and `BuildFilesystemOperations`); `Pure` vs `Granted` execution mode by filesystem reach.
- Shape: `SelectedBuildMachine` :18; `select_build_machine` :29; `admit_build_machine` :50.
- How it decides: build machine = machine named `build` whose symbol source span is the selected `build.omg` SourceId (`is_build_machine`, see build_value/vocabulary.rs:13-28). Services reached are compared by protocol symbol via `language_semantics::build_requests::is_project_request_protocol` / `is_build_protocol(.., "BuildFilesystemOperations")` (:89-96; the second compares a string name). Any other service in `inferred_transitive` is an error (:114-121). `filesystem_reachable` = presence of the filesystem protocol (:122-133).
- Fail-open risks: `service_reach_plan.for_machine(machine.symbol).map(...).unwrap_or(&[])` (:78-81): if the plan has no row for the build machine its reach is treated as empty, so the machine is admitted as `Pure` with no restrictions. What reaches it: a build machine symbol that the reach plan did not cover. Disposition: open (plan construction not read).
- Debug hook: `std::env::var_os("OMEGA_DEBUG_BUILD_CONFIG")` prints to stderr (:107-113).

## PC/src/build_execution/build_record_codec.rs (811 lines, read in full)
- Purpose: binary encode/decode for persisted build-execution records in the compiled-review cache: `BuildEvaluationUsage` (magic `BEU\x01`), `BuildObservationSummary`, `RestrictedBuildRequest`. Decoders return `None` on any malformation so callers treat corrupt cache entries as misses (:1-6).
- Shape: `encode_build_evaluation_usage` :40 / `recover_build_evaluation_usage` :89; `encode_observation_summary` :344 / `recover_observation_summary` :417; `encode_restricted_build_request` :603 / `recover_restricted_build_request` :655; helpers `encode_attempt`/`recover_attempt` :236/:282, `recover_staged_output_tree` :202, `cursor_count` :724; tests module `evaluation_usage_controls` :728-811 (5 tests).
- How it decides: enum tag bytes with `_ => return None` (e.g. :93-100, :216-220, :292-297); paths checked with `canonical_filesystem_metadata_path_is_canonical(..., false)` (:299, :472, :485, :685); `usage.validate().ok()?` (:138); staged tree rebuilt by `BuildStagedOutputTree::from_retained_parts(&commitment, inputs)` which recomputes the digest (:233).
- Cases by shape: grant roots `SourceInventory|StagedOutput|Other(u32)` (:582-587); attempt roots `Source|Output|NamedInput(u32)` (:292-297).
- Fail-open risks: counts read from the cache (`cursor_count`) are used directly as `Vec::with_capacity(count)` (:209, :288, :310, :320, :327, :426, :457, :469, :482, :662, :667, :682) before the entries are read, so a corrupt cache entry with a huge count requests a huge allocation rather than returning None. This is a robustness hazard on corrupt input, not an accept path. Disposition: traced (read).
- Observation: decoded `RestrictedBuildRequest`/`BuildObservationSummary` values are rebuilt with `from_retained`/struct literals without re-running the admission that originally produced them; staged tree is the only member whose digest is recomputed. Whether the cache key binds them is not in this file ("open").

## PC/src/build_execution/build_value.rs (75 lines, read in full)
- Purpose: `BuildValueAdmission` retains the pre-evaluation admission of the `Build` vocabulary (optimization vocabulary, toolchain activation flag) so that extraction after evaluation decodes only the exact toolchain `Build` (comment :8-10: a lookalike struct must not smuggle settings).
- Shape: `BuildValueAdmission {optimization, has_toolchain_build_activation}` :34 (`admit` :40, `initial_value` :50, `extract` :59); re-exports `EvaluatedBuildSettings`, `PrivilegedServicesGrants`, `is_build_machine`.

## PC/src/build_execution/build_value/configuration.rs (280 lines, read in full)
- Purpose: decodes the evaluated `Build` struct (`BuildTimeValue`) into `EvaluatedBuildSettings`.
- Shape: `EvaluatedBuildSettings` :18; `ElfInterpreterSelections` :30 (`explicit` :35, `effective` :42, `from_build_rows` :51); `PrivilegedServicesGrants` :75; `X86DeploymentClaim` :96 (`admit_for` :103); `extract_build_settings` :131.
- How it decides: by field name strings (`"x86_deployment_features"`, `"subsystem"`, `"identifier"`, `"freestanding"`, `"privileged_services"`, `"elf_interpreter"`) found in the struct's `(String, BuildTimeValue)` rows (:145-151); enum cases by the last `::` segment of the variant text: Console->3, Gui->2, EfiApplication->10 (None intent), `Unspecified{value}` -> u16 (:177-198); x86 cases Baseline / AvxFma3 (:164-171). `AvxFma3` with a non-x86_64 profile or no profile admits nothing and returns `Ok(None)` (:108-113).
- Cases by shape: missing optional fields default (`Err(_) => default`) for `identifier`, `privileged_services`, `elf_interpreter` (:217, :250, :262) and `x86_deployment_features` when `has_activation_vocabulary` is false (:173-175); header :3-10 states "default cleanly when omitted by builds that predate those surfaces".
- Fail-open risks: (1) a `Case` whose variant spelling ends in `Console` from any enum is accepted because only the last `::` segment is compared (:179); traced: the Build struct was admitted against the exact prelude before evaluation (build_value.rs:8-10), so the type is fixed; case spelling lookalikes from another enum cannot be assigned to this field by the type checker (inferred, not traced to the checker). (2) `AvxFma3` silently admits nothing on AArch64 targets (:110-113) by design (header :6-9, stated).

## PC/src/build_execution/build_value/initial.rs (128 lines, read in full)
- Purpose: builds the starting `Build` struct for the evaluator with defaults and facet markers.
- How it decides: fields included only if the prelude facet exists: `has_exact_toolchain_build_facet(typed, "BuildLog"|"BuildProduct"|"PrivilegedServices"|"ElfInterpreterSelection"|"BuildSource"+"BuildOutput"|"BuildInputs")` (:51-121) which checks `definition.name == name && symbols.is_build_prelude_symbol(...)` (vocabulary.rs:30-34); `identifier` included if the build machine's parameter type `Build` declares a field named `identifier` (vocabulary.rs:52-71, keyed on source spelling of the data name `Build` and field name). Private marker type names `$OmegaBuildLogFacet`, `$OmegaBuildProductFacet`, `$OmegaBuildSourceRoot`, `$OmegaBuildOutputRoot` are string literals (:55, :64, :113, :117); source/output roots carry grant-root identities `BUILD_SOURCE_ROOT_IDENTITY`/`BUILD_OUTPUT_ROOT_IDENTITY`.
- Same job elsewhere: `build_time_evaluation::admit_toolchain_build_activation` (outside unit, not read) decides `has_toolchain_build_activation`.

## PC/src/build_execution/build_value/optimization.rs (45 lines), optimization/selection.rs (131 lines), optimization/vocabulary.rs (84 lines), vocabulary.rs (71 lines), all read in full
- Purpose: optimization vocabulary admission and counter extraction.
- How it decides: `classify` (optimization/vocabulary.rs:18) returns `Canonical` if the single `Build` data declaration has an `optimizations` field of the exact toolchain `Optimizations` type and `Build` itself is the toolchain Build (:54-82), else `LegacyWithoutField` when there is no such field. If there is no toolchain `Build` among the named `Build` definitions, it falls back to the authored ones (:31-35) and requires exactly one. In `LegacyWithoutField`, extraction returns default selections and `Suppressed` report (selection.rs:46-51). Counters must be 0 or 1; >1 or negative error (selection.rs:76-87, :111-125); `selected` length must equal `Optimization::ALL.len()` (:96-102).
- Cases by shape: `Build.optimizations` field types other than the exact toolchain type are errors (:60-75).
- Fail-open risks: `LegacyWithoutField` means an authored `Build` without the optimizations field gets "no selections, report suppressed" instead of an error (:83, selection.rs:46-51): stated by header ("legacy empty-selection behavior" optimization.rs:3-7).

## PC/src/build_execution/completed_outputs.rs (123 lines, read in full)
- Purpose: `RetainedBuildOutputs`: the completed required-output file set bound to its `BuildObservationIdentity`, with a manifest and identity digest, and publication to `<build_dir>/completed/<hex>`.
- Shape: `RetainedBuildOutputs {observation, tree, manifest, identity, published_directory}` :15; `from_observation` :24; `publish` :86.
- How it decides: only paths in `required_output_settlements` enter the tree (`select_files(&names)` :33-43); manifest = domain `OMEGA-COMPLETED-BUILD-OUTPUTS-V1\0` + observation identity + tree digest + count + per-file (path, length, content digest) (:45-59). `publish`: fresh set -> `materialize_into` + `publish_exact_file_bytes`; existing directory -> `verify_existing_set` byte-for-byte (:95-119). The set name is the hex of the manifest identity (:90).
- Callers outside this file (Grep count of `RetainedBuildOutputs::from_observation|.publish(` excluding this file = 16 lines over 14 files, mostly unrelated `.publish(` calls; the `from_observation` callers were not separated): suspected users in `omega/compilation/products/src/evaluated_build.rs` and `build-system/build-settings/src/activation/preparation.rs`; not read.
- Fail-open risks: `AlreadyExists` on `create_dir` is treated as "reuse after verification" (:97, :112-119); verification is strict (see below). If the directory exists but is half-written (files present, manifest missing), `verify_existing_set` reports `missing entry` since `manifest.bin` is in `expected` :43-52. none seen.

## PC/src/build_execution/completed_outputs/verification.rs (243 lines, read in full)
- Purpose: capability-relative (cap-std) verification of an existing completed-output directory against the retained tree and manifest.
- Shape: `verify_existing_set` :28; `verify_directory` :86; `verify_file` :126; `require_same` :179; per-OS `is_executable`/`has_single_link` :217-243.
- How it decides: walks the directory removing each found entry from an `expected` map keyed by `files/...` byte paths; any extra entry fails; any left-over expected entry fails (:68-73); files must be regular, non-symlink, link count 1, equal length/executable bit/bytes; directory/file identity (dev+ino or volume+file index), ctime (Unix), type, len, permissions and mtime must not change during the check (:179-215). On Windows `is_executable` is always false (:223-226) so an executable retained file would fail on Windows (`is_executable(&initial) != executable`, :144) if the tree says executable.
- Same job elsewhere: `capture/mod.rs:178-189,227-235` in build-output does similar link-count/identity checks for the staging tree.
- Fail-open risks: none seen.

## PC/src/build_execution/execution_inputs.rs (46 lines, read in full)
- `BuildExecutionContract {profile, snapshot}` :13; `BuildExecutionInputs` (profile, directory, filesystem_sponsor, evaluation_sponsor, snapshot, restricted_grants) :19; `resolve_compilation_defaults` :36 defaults `profile` to `TargetProfile::host_if_supported()` and `snapshot` to `BuildSnapshotRequest::automatic_package_inventory(inputs)`.
- Same job elsewhere: automatic snapshot selection is also done in `filesystem_scope/preparation.rs:90-92` (`retained_input.map(|_| BuildSnapshotRequest::new(empty))`), with different conditions (retained custody vs presence of canonical source metadata `snapshot_request.rs:37-40`).

## PC/src/build_execution/filesystem_scope.rs (648 lines, read in full)
- Purpose: `BuildMachineFilesystemScope`: the filesystem authority of the admitted `build.omg`: a Source root (captured snapshot or a live capability-bound directory), an Output root (private staging directory under a sponsor), and named read-only input roots numbered from 3.
- Shape: constants `BUILD_SOURCE_ROOT_IDENTITY`=1 :46, `BUILD_OUTPUT_ROOT_IDENTITY`=2 :51; struct :58-73; `NamedBuildInput` :76; `overlap_key` :85; `resolved_write_fence` :127; constructors `for_root` :149, `for_admitted_root` :179, `with_captured_source_input` :222, `with_scoped_package_source_input` :238, `with_named_inputs` :261, `with_required_outputs` :326; `filesystem_access` :381; `is_private_snapshot_execution` :428; write-root admission `ensure_write_roots`/`admit_write_roots`/`ensure_established_write_root` :446-557; `verify_required_outputs` :568; `staged_output_tree` :623; `captured_inventory` :637.
- How it decides: write root vs source root overlap by a spelling-independent `overlap_key` (absolute, `..` folded lexically, longest existing prefix canonicalised, nonexistent tail appended) compared with `starts_with` (:470, :458); re-checked after creating the directory (:550). Required outputs: canonical path, <= 4096, none equal to or an ancestor/descendant of another (:332-358). Named input slot `root` ids are `index + 3` by BTreeMap order of slot names (:268-277).
- Cases by shape: access mode chosen by what is bound: any captured roots -> `CapturedScoped`; else sponsor -> `RealScopedSponsored`; else `RealScoped` (:412-425). Private snapshot execution = captured input present, no read roots, the sponsor owns the build dir, and the access equals the freshly computed access (:428-444).
- Same job elsewhere: `overlap_key` fencing is the only path-overlap implementation in the unit (grep not run for others); the required-output collision test (:343-358) is repeated at run time in `runtime.rs:586-592` (`is_descendant`).
- Stated limits: header :14-16 says unreachable-filesystem runs return `Some(empty())`.
- Fail-open risks: (1) `overlap_key` returns the lexically folded absolute path (not canonicalised) when `..` underflows (:99-101) and falls back to the raw path if `current_dir` fails (:91): an odd path then compares by spelling only. Disposition: open. (2) `staged_output_tree` returns `Ok(None)` when the filesystem is reachable but no sponsor exists (:630-632); downstream `verify_required_outputs` rejects only if required outputs exist (:573-580) and `settle_build_output_obligations` rejects obligations (output_custody.rs:68-80); a build that writes outputs without any sponsor would get `None` and no error from this function alone. Disposition: traced to be unreachable for the Terminal runtime path because `begin_filesystem_runtime` requires a private sponsor (runtime.rs:75-81), other paths not traced (open). (3) `with_captured_source_input` accepts an input when no canonical metadata is bound yet and adopts the input's metadata (:232-234); when metadata was bound, mismatch is rejected (:226-231). stated by the branch structure.

## PC/src/build_execution/filesystem_scope/live_source.rs (66 lines, read in full)
- Purpose: `LiveBuildSource(Arc<cap_std Dir>)` opened at admission; `open_read(path)` walks components with `open_dir_nofollow`, requires the final entry to be a regular file via `symlink_metadata` and again via `file.metadata()`, opens with `FollowSymlinks::No` and `nonblock(true)` (:34-60). Equality is `Arc::ptr_eq` (:19-23).
- Fail-open risks: none seen (empty path rejected :61-64; non-UTF-8 rejected :35-36).

## PC/src/build_execution/filesystem_scope/preparation.rs (229 lines, read in full)
- Purpose: `prepare_filesystem_scope`: turn a compilation request (package inputs or anonymous source, build dir, sponsor, snapshot request) into a `BuildMachineFilesystemScope`.
- How it decides: package inputs + anonymous source together is an error (:34-38); retained custody (package or anonymous captured input) triggers snapshot mode automatically (:90-92); in snapshot mode without a sponsor it creates a private output sponsor under `std::env::temp_dir()/omega-build-output-<pid>-<counter>` retrying on `AlreadyExists` (:95-113) and replaces `build_dir` with `<sponsor session root>/output` (:119-122); `PackageInventory` snapshot needs package custody with canonical metadata else error (:125-143); `Scoped` snapshots resolve required members via the invocation filesystem and make the scoped input a subset of the complete one (:145-199, :200-226).
- Cases by shape: authored members = selected files with `origin == User`, `resolution_stratum == Base` and `package_root == source_root` (:156-160).
- Fail-open risks: when there is no retained input and no snapshot request (plain standalone build, `for_root`), no snapshot is taken: the scope falls back to a live source (`establish_filesystem_access` binds `LiveBuildSource` filesystem_scope.rs:137-145). That is stated by the `retained custody never falls back to a real-host grant` header (:7-8) which only covers retained custody.

## PC/src/build_execution/filesystem_scope/runtime.rs (938 lines, read in full; tests :799-935 and `live_source_tests` mod :937-938)
- Purpose: `BuildFilesystemRuntime`: executes the evaluated `Build` filesystem calls (open/read/close of Source and named inputs, create/write/close of Output, required-output obligations, `include_source`) charging an `EvaluationInvocationAccount` and recording `BuildFilesystemOperationAttempt` rows.
- Shape: `BuildFilesystemEffects` :28; `OpenFile`/`FileStorage {Captured, LiveSource, Output}` :37-56; `BuildFilesystemRuntime` :58; `begin_filesystem_runtime` :68; methods `resolve` :116, `begin_attempt` :153, `open` :261, `read` :341, `create` :409, `write` :467, `close` :510, `require` :581, `required_path` :614, `can_complete` :626, `complete_output` :638, `fail_output` :674, `include_source` :706, `finish` :727; helpers `logical_handle` :776, `is_descendant` :785, `copy_bytes` :790.
- How it decides: descriptors are `index + 3` into a `Vec<Option<OpenFile>>` (:312-315, :418-421); a descriptor must belong to the expected root (:556-569). `open` with nonzero `flags` always returns -1 with errno 13 if write-like (`wants_write(flags) || flags & 3 != 0`) else 45 (:278-295; numeric errnos are literals 13, 45, 21, 40, 2, 5 at :293, :303, :306, :308, :325). Captured open maps File -> ok, Directory -> errno 21, Symlink -> errno 40, other -> errno 2 (:300-309). Output create uses `OpenOptions::create_new` on `build_dir.join(path)` guarded by the sponsor (:427-452). A required output completes only if its path was sealed by a `close` (`sealed` list) (:626-636).
- Cases by shape: provider tag is `RealScoped` for Output and live Source, otherwise `Captured` (:221-227).
- Same job elsewhere: `LiveBuildSource::open_read` (live_source.rs:34) opens live reads while the sponsor-backed output uses plain `std::fs` with sponsor reservations; required-output collision check duplicated with `filesystem_scope.rs:343-358`.
- Fail-open risks: (1) `close` on a non-output descriptor frees the slot and records a close attempt even though nothing was sealed (:532-549); harmless. (2) `fail_output` marks the obligation Failed and always returns `Err` (:698-703). (3) `require` limit check uses `filesystem_authority::MAX_BUILD_OUTPUT_OBLIGATIONS` (:583) while `with_required_outputs` uses a local `REQUIRED_BUILD_OUTPUT_LIMIT` 4096 (filesystem_scope.rs:83): two separate constants (value of the first not read). none seen as fail-open.
- Tests in file: 2 (:861, :909) plus 3 in `live_source_tests.rs`.

## PC/src/build_execution/filesystem_scope/runtime/live_source_tests.rs (151 lines, read in full)
- 3 tests: live reads see changes after admission and report provider `RealScoped` (:46-88); root and descendant symlink retargeting cannot redirect live reads (unix only, :90-130); two independent live-root admissions cannot exchange effects (:132-151).

## PC/src/build_execution/observation_identity.rs (251 lines, read in full)
- Purpose: 32-byte `BuildObservationIdentity` digest over every field of a `BuildObservationSummary` under domain `OMEGA-PACKAGE-BUILD-OBSERVATION-COMPARISON\\0` (the last two bytes are a literal backslash and `0`, :1-16).
- Shape: `BuildObservationIdentity([u8;32])` :19; `BuildObservationSummary::identity` :32; tag functions `filesystem_provider_tag` :203, `grant_access_tag` :212, `filesystem_root_tag` :219, `logical_handle_kind_tag` :227, `grant_refusal_reason_tag` :235, `declaration_role_tag` :244.
- Same job elsewhere: `declaration_role_tag` (this file :244-251: None=0, Package=1, Application=2, Workspace=3) vs `declaration_kind_tag` imported from `crate::reuse::review_record_encoding` in `build_record_codec.rs:8-9` (not read): two tag tables for `BuildDeclarationKind` (suspected duplicate). `profile.target_name().as_bytes()` here (:59) vs `crate::target_profile_tags::target_profile_tag(profile)` in the codec (:377): two encodings for target profile.
- Fail-open risks: none seen (pure hashing).

## PC/src/build_execution/observations.rs (293 lines, read in full)
- Purpose: durable observation record types (`BuildObservationSummary`, `BuildActivation`, `BuildFilesystemOperationAttempt`, inventories, settlements, handoffs) and `BUILD_OBSERVATION_SCHEMA_VERSION` = 82 (:24). Re-exports filesystem-authority types under `Build*` names.
- Shape: struct fields are `pub(crate)`; accessors only.

## PC/src/build_execution/output_custody.rs (113 lines, read in full)
- Purpose: `settle_build_output_obligations` and `select_generated_sources`.
- How it decides: each obligation must be on the Output root, `Completed`, with a receipt whose index, root and path agree, with no post-completion mutations, and whose path is a sealed regular file in the staged tree (:22-80). `select_generated_sources`: with a tree, defers to `build_output::select_included_sources`; with no tree and no handoffs returns empty; with no tree and handoffs errors (:98-111).
- Fail-open risks: none seen.

## PC/src/build_execution/restricted_requests.rs (265 lines, read in full)
- Purpose: normalized description of what a `Build` run was granted (read/write grant roots, captured inventories, sponsor limits, required outputs, artifact-only flag), reported in package vocabulary.
- Shape: `RestrictedBuildGrants` trait (`admit`) :15; `RestrictedBuildOperation` :23; `RestrictedBuildGrantRoot` :29; `RestrictedBuildGrant` :36; `RestrictedBuildBounds` :70; `RestrictedBuildRequest` :113; `BuildMachineFilesystemScope::restricted_build_requests` :174.
- How it decides: returns an empty list for `Pure` mode, for private-snapshot execution (:183-185) and for `Virtual` access (:213); otherwise one request, `UnscopedFilesystemExecution` only for `RealUnscoped` (:207-212). `narrowed` = the grant root has canonical metadata (:230), but every captured input root is hard-coded `narrowed: true` (:242).
- Unused/one-caller: `restricted_build_requests` has 1 call here (definition) and callers in `build-settings/src/admission/program.rs` (3 lines), `build_evaluation.rs` (1), and review code in `package-manager` and `review-evidence` (Grep count over `omega-rust` = 11 lines across 9 files; the caller lines for `restricted_build_requests(` in those files were counted, not read).
- Fail-open risks: a private-snapshot build (the common retained-custody path) reports no restricted request, so a grant-consent guard (`RestrictedBuildGrants`, execution_inputs.rs:25) is never consulted for it. Stated by the design that a private snapshot has no real-host grant (preparation.rs:7-8); "traced" to `is_private_snapshot_execution` requiring a captured input, no read roots and a sponsor owning the staging root (filesystem_scope.rs:428-444).

## PC/src/build_execution/snapshot_request.rs (119 lines, read in full)
- Purpose: `BuildSnapshotRequest {required_outputs, capture: PackageInventory | Scoped(BuildSourceCaptureRequest), inputs}` and the named-input limit checks.
- How it decides: named input slot names must be UTF-8 and canonical (:81-87), unique (:88-92), at most 4096 slots, total entries <= `CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT`, total file bytes <= 512 MiB, total name bytes <= `FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT` (:104-111).
- Same job elsewhere: limits 4096 and 512 MiB are literals here (:104, :108) while the staging tree uses 4096 / 256 MiB constants in `build-output/src/staged_output_tree.rs:48-50`.

## PC/src/codec_bytes.rs (15 lines) and PC/src/cursor.rs (5 lines), read in full
- `codec_bytes.rs`: re-exports `optimization_core::codec_bytes::push_encoded`; defines `push_u64` and `push_bytes` (:8-15). Grep (count mode) of `push_u64\(|push_bytes\(|codec_bytes::push_u64|codec_bytes::push_bytes` in package-compilation: codec_bytes.rs 3 lines, `reuse/checked_source_cache.rs` 5 lines; so the two local helpers are used by one outside file (`checked_source_cache.rs`).
- `cursor.rs`: re-exports `Cursor, TakeError, Truncated` from `optimization_core::cursor` (:5). Header calls it the "shared bounded byte cursor" for wire decoders. Grep count of `crate::cursor|mod cursor|...` shows 1 user outside lib.rs/cursor.rs (`reuse/checked_source_cache.rs`). `build_record_codec.rs` does NOT use it; it decodes through `filesystem_authority::observation_codec::ObservationCursor as Cursor` (build_record_codec.rs:11). So the "shared" cursor has one outside user and a second cursor type exists in this unit's largest decoder.

## PC/src/lib.rs (110 lines, read in full)
- Purpose: crate root; large re-export surface of package compilation inputs, build execution, source custody, reuse digests. `#![forbid(unsafe_code)]` :35. `capture_anonymous_compilation_source` :86 (captures the parent directory of the root file, or the root itself if it is a directory, via `package_source::local::operations::capture_local_source_input` with default limits).
- Same job elsewhere: anonymous capture root selection (:89-96) vs `preparation.rs:50-59` source-root selection (parent of `root_path` with "." fallback).

---------------------------------------------------------------------
## Coverage
- Read in full: all 83 rows of unit_032.tsv, every line of every range (sum of line counts: 21,688). The row list: 35 files under `omega-rust/omega/interpreter/terminal-runtime` (11,780 lines), 11 files under `packages/build-declarations` (2,118 lines), 10 files under `packages/build-output` (2,189 lines), 27 files under `packages/package-compilation` (5,601 lines).
- Skimmed: none. Not read: none.
- Read-only extra lookups outside the unit: `host_imports.rs:185-247` (`matches_platform_c_symbol`) and `:490-585` (adapter admission call sites); greps listed in the sections above.

## Five things a maintainer most needs to know about this unit
1. Every host import adapter in `terminal-runtime` re-validates its declaration against a fixed table of shapes and then compares the ABI via `admit_exact_foreign_call`; OS entries are selected by normalised locator or syscall number, not by guest names (e.g. `clock/admission.rs:19-72`, `filesystem/linux.rs:154-172`, `filesystem/windows/contracts.rs:32-446`). The Darwin and Windows filesystem adapters only admit when the compiler itself runs on that host (`filesystem.rs:191-194`, `filesystem/windows.rs:82-86`).
2. Service permission for filesystem delegation is keyed on identity text: six `toolchain::Filesystem...` strings (`filesystem.rs:446-456`) and the `package:` prefix before `::` for native-service delegation (`native_provider/service_reach.rs:9-14`).
3. `borrowed_storage.rs`/`geometry.rs` is the one place that turns an interpreter storage loan into bytes and back; `strtol` (the only `PointerOutputPlan` user) and every descriptor/clock/filesystem adapter go through `CapturedStorage` (`Observe` needs fully covered bytes; `ReplacePrefix` needs a scalar or discriminator recipe for each byte).
4. In `build-output`, production constructors do not run the parent-directory, strict-order and symlink-target checks; those exist only under `cfg(test)` (`materialization/mod.rs:74-161`, `tree_from_entries.rs`). `from_retained_parts` relies on the recomputed commitment matching (`staged_output_tree.rs:130-181`), and its only non-test caller is the cache decoder (`build_record_codec.rs:233`).
5. `build-declarations` reads `build.omg` role and dependency calls by spelling (`package`, `application`, `member`, `artifact_only`, `depend*`, parameter `builder`, type `Build`) with a syntax-only projection, and embeds the `Build` API as a Rust string with spliced target/optimization catalogs (`prelude.rs:20-421`, `prelude/targets.rs`, `prelude/optimizations.rs`).

## What I am unsure about
- Whether `service_reach_plan.for_machine(...)` can return `None` for a selected build machine (`build_admission.rs:78-81`); I did not read the plan builder.
- Whether the one-of-two tag tables for `BuildDeclarationKind` and target profile (`observation_identity.rs:244`, `target_profile_tags`, `review_record_encoding`) agree; the codec side was not read.
- Whether `CapturedSourceEntry::File` values with a wrong `digest` can reach `from_capture_rows` (variant fields are public, `captured_source/mod.rs:28-32,159`); callers `source_snapshot.rs:260,326,361` not read.
- The exact ordering of adapter admission in `host_imports.rs` (only :490-585 was read) and therefore which adapter wins when two could match.
- `Sponsor::call` uses `expect` (`original_homes.rs:77`); I inferred that `install` always runs first but did not trace every path.
