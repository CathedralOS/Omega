# Record: unit_077 (area 14-interpreter)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d`. All paths are under
`source-rust/subsystems/interpreter/`. Every file in the unit was read in full (53 files, 21,853 lines).
"read" = read in this session; "grepped" = counted with the Grep tool over `source-rust/`; "inferred" = reasoned, not run.
Crate map context: `embedding/` is part of `terminal-interpreter`; `host/{clock,filesystem}` are compiled into
`terminal-runtime` by `#[path]` (`host/src/execution/host_imports.rs:42,46`); `host/native-worker` is
`native-provider-runtime`; `host/native-sponsor` is `native-provider-sponsor`; `host/retained-image` is
`retained-image-runtime`. The crate map says `sponsor.rs` is "about 490 lines"; the unit row and my read both give 349.

---

## 1. `embedding/reference_completion_tests.rs` (2303 lines, test-only)

1. Purpose: tests that a host-supplied reference result ("resident" payload) keeps live scalar cells (backings) instead of snapshots, and that malformed provider results are refused before storage is published. Also tests aggregate-reference formation against admitted target geometry.
2. Shape: helpers `ty` (:22), `setup` (:54, builds a tiny program with boundary call op 1 and `EstablishReference` op 2), `Backing` (:167, a `RefCell` value plus `stale` flag implementing `TerminalScalarBacking`), `resident` (:195), `commit` (:225), `read` (:238), `dispatch` (:791), `ReferenceHandler` (:774), `OriginalStorageHandoff` (:1036), `check_aggregate_reference_formation` (:1830) with enum `AggregateReferenceControl` (:1820, seven controls). About 20 `#[test]` fns (:149-2300).
3. Inputs/outputs: builds `TerminalExecution` via `start_with_program` (:138) or a real sealed artifact via `terminal_codec::CanonicalTerminalArtifact::from_parts` + `admit_target_layouts` (:2017-2032). Asserts on private engine fields (`structural_values`, `reference_referents`, `structural_primitive_storage`, `structural_scalar_fields`, `structural_byte_storage`, `element_view_values`, `live_affine_frontier`; e.g. :547-551, :2237-2244).
4. How it decides (what the tests pin): refusal keyed on type identity, path and value: resident roster must match aggregate field paths, order, stored scalar and values (cases 0-8, :477-542); restrictions are re-read on every read (:328-391); a nested Resident under an outer materialized contract is refused (:745-772); a canonical `result_lifetimes` frontier alone does not authorize materialized contents (:951-956); the live-reference owner lookup keys on `opaque_identity` plus path and ignores unreachable cache rows (:1371-1408, :1524-1596).
5. Cases by shape: aggregate formation is exercised only for one fixed type graph (sum, u8 array, packed record, authored 16-byte record, `linux_x86_64` facts) with pointer size 8 or `1 << 63` (:2025). Message text asserted: `stage: "validate aggregate storage view"` and `compatible=false` (:2233-2234), i.e. a test pins a `format!`ed debug string.
6. Same job elsewhere: the engine-side behaviour under test lives in `instructions/` and `memory/` (not read; suspected).
7. Unused/one-caller: none (test file).
8. Stated limits: none quoted; the comment "Existing guest-owned ambiguity must win even though a live reference backing exists" (:1611) states precedence of errors.
9. Fail-open: none seen. Tests assert refusals (`is_err()`) for forged inputs. Note the test at :759-763 has a conditional "if accepted.is_ok()" before asserting `accepted.is_err()`; the assertion at :764 still fails if accepted.

## 2. `host/clock/admission.rs` (189)

1. Purpose: decides which boundary import is a clock/sleep call and checks its declaration shape and ABI.
2. Shape: `select` (:19) returns `(ClockNamespace, ClockImplementation)`; `validate` (:74) returns `ClockOperation`.
3. Inputs/outputs: reads `NormalizedForeignCallBinding` (system call or foreign locator), `TerminalImportSignature`, `NativeBorrowedStorageLayout` list; produces `ClockOperation`; calls `timespec::admit` and `super::super::admit_exact_foreign_call` (:187).
4. How it decides: Linux by (target, syscall number): 228/113 read timespec, 7 poll (x64 only), 35/101 nanosleep (:22-33). macOS by normalized locator equality against `_clock_gettime_nsec_np`, `_clock_gettime`, `_poll`, `_nanosleep` in `libSystem.B.dylib` (:45-70). Windows by `windows::select`. Keyed on identity (target/number, normalized locator). `validate` keys on scalar types, `parameter_order` vectors (:95-145).
5. Cases by shape: five fixed shapes (`WindowsWord`, `WindowsSleep`, `Read`, `ReadTimespec`, `Nanosleep`, `Poll`); selector must be a 32-bit non-address integer (:89-90); poll count may be u32 or u64 (:136); `status_valid` allows unit, i32, or i64 only on Linux (:91-94).
6. Same job elsewhere: Linux syscall-number selection is also done for filesystem in `filesystem/linux.rs:292` and for descriptors via `runtime_abi::descriptor_system_call` (`descriptor_io.rs:218`); each module has its own selector (grepped by reading).
7. Unused code: none (`select`/`validate` have one caller each in `clock/bindings.rs:189,193`, read).
8. Stated limits: header comment lines 1-5 (selection of exact implementations).
9. Fail-open: `select` returns `Ok(None)` for any unrecognised syscall/locator (:31, :37, :43, :71). `None` means "not a clock", so the caller keeps trying other adapters (`host_imports.rs:541`); the import is not accepted by this module. Classification: traced, falls through to later adapters and finally a refusal at `host_imports.rs:627`.

## 3. `host/clock/bindings.rs` (500)

1. Purpose: the clock adapter (`ClockImport`): serves `clock_gettime`, `nanosleep`, `poll` (timeout only), Windows performance-counter/FILETIME/`Sleep` against one shared snapshot or the live host clock.
2. Shape: `TerminalClockSnapshot` (:48, monotonic and unix ns), `TerminalClockEnvironment` (:54, `Snapshot` or `Host`), `ClockNamespace` (:66), `ClockImport` (:174) with `admit` (:182) and `TerminalEffectHandler` impl (:298-438). `selector` (:236), `status_result` (:259), `wait` (:283). One `#[test]` (:458).
3. Inputs/outputs: `Arc<Mutex<TerminalClockEnvironment>>` shared by all clock slots; reads `native_provider_runtime::read_host_clock_nanoseconds` (:41, :139); writes timespec bytes through `timespec.rs`; returns `TerminalEffectResult` unit/scalar.
4. How it decides: selector validity keyed on `(namespace, selector value)`: 0 always, Darwin 4/5/6/8/9, Linux 1/4/7, Windows 1 (:73-83). Live `Host` clock only if the selected target equals `TargetProfile::host_if_supported()` (:213-226). Poll rejects `count & 0xFFFFFFFF != 0` and negative timeout (:423). Snapshot waits advance both clocks; host waits `thread::sleep` (:146-152).
5. Cases by shape: operations are a fixed enum; poll supports only zero descriptors (the count check) and finite timeouts: comment :17-18 "CPU accounting, signals, and descriptor readiness remain unsupported".
6. Same job elsewhere: Windows time is in `clock/windows.rs`; timespec in `clock/timespec.rs` (same module family, not duplication). `matches_platform_c_symbol` (`host_imports.rs:215`) is a shared locator matcher used by seven adapters, but clock uses its own loops (`admission.rs:45`, `windows.rs:48`) rather than it (grepped by reading): same job done twice.
7. Unused code: `TerminalClockSnapshot::capture_host_epoch` (:113) is `pub`; grepped `capture_host_epoch` over `source-rust/`: 1 occurrence (its definition), 0 callers in `source-rust/` (tools/ not grepped).
8. Stated limits: comment lines 17-24 ("remain unsupported because a private snapshot cannot establish native latency or interruption equivalence"); "requires an indefinite-wait model" (`windows.rs:164`).
9. Fail-open: `ClockOperation::ReadTimespec/Nanosleep/WindowsWord` reached through the scalar `handle_effect_result` return an error (:384-388). `selector()` masks signed negatives to their low 32 bits (:252-256): a negative signed clock ID reaches selector validation as a large value and is rejected unless it equals an allowed value. Classification: stated (comment :11-13, "negative signed IDs retain their bits"). Otherwise none seen.

## 4. `host/clock/timespec.rs` (178)

1. Purpose: reads/writes 16-byte timespecs through captured borrowed storage for `clock_gettime` and `nanosleep`.
2. Shape: `admit` (:20), `read` (:55), `sleep` (:102).
3. Inputs/outputs: `CapturedStorage::capture` (`borrowed_storage.rs:164`) with `StorageTransfer::ReplacePrefix` (read) or `Observe` (sleep); `ClockImport` shared clock.
4. How it decides: layout found by `structural_type` equality (:31-34); access must be `MutableBorrow` for read, a reading-borrow pair for sleep (:35-42); layout needs `byte_size >= 16` and `alignment >= 8` (:47-48). Sleep with `seconds < 0` or nanoseconds outside 0..1e9 returns -22 on Linux and a refusal on Darwin (:158-165).
5. Cases by shape: exactly one structural parameter (:28); two i64 words only.
6. Same job elsewhere: `windows.rs::write_word` repeats the capture/stage pattern for an 8-byte word.
7. Unused: none seen.
8. Stated limits: lines 1-5 and :160 "requires a hosted errno model".
9. Fail-open: none seen.

## 5. `host/clock/windows.rs` (212)

1. Purpose: Windows `QueryPerformanceCounter`, `QueryPerformanceFrequency`, `GetSystemTimePreciseAsFileTime`, `Sleep` over the shared clock.
2. Shape: `WindowsWord` (:29), `select` (:45), `word_loan_valid` (:80), `write_word` (:99), `sleep_nanoseconds` (:161). Two tests (:188, :208).
3. How it decides: locator equality with `kernel32.dll` plus export name (:48-76). Loan must be a primitive scalar of the right carrier, `MutableBorrow`, no qualifications (:80-97). `INFINITE` (0xFFFFFFFF) refused (:162). Counter equals the monotonic reading, frequency is the constant 1e9 (:145), FILETIME is `reading / 100 + 11644473600 * 1e7` (:146-147).
4. Same job elsewhere: locator loop duplicates `admission.rs:45-70` (macOS) in structure.
5. Unused: none seen. Stated limits: header :1-6.
6. Fail-open: none seen. `GetTickCount64` is explicitly not claimed (test :204).

## 6. `host/filesystem/bindings.rs` (1301)

1. Purpose: admission and per-call translation for filesystem imports: libc/Windows imports on a matching host, with a guest descriptor/handle namespace so the worker's OS numbers are never exposed.
2. Shape: `FilesystemContext` (:33: `supplied`, `rooted`, `windows` handles, `linux` fs, `private` fs, `descriptors` map), `NativeDescriptor` (:49 `Open`/`Uncertain`), `DescriptorEffect` (:55), `Formal` (:152, nine formal shapes), `FilesystemCall` (:76 `Libc`/`Windows`), `LibcFilesystemCall` (:68). Entries: `FilesystemContext::admit` (:225), `admit_services` (:514), `matches_signature` (:578), `matches_parameters` (:601), `LibcFilesystemCall::{arguments (:750), result (:811)}`. Tests :878-1300 (one macOS-only, :1141).
3. Inputs/outputs: reads `BoundaryMachineDeclaration`, layouts, `TerminalProgram`; builds `Vec<Formal>` tables; returns `FilesystemCall`. Mutates `descriptors: Rc<RefCell<BTreeMap<i32, NativeDescriptor>>>`.
4. How it decides: (a) target match: Linux only for host == selected target and only for glibc `realpath` (`glibc_realpath` :557, versions `GLIBC_2.3` x64 / `GLIBC_2.17` arm64); macOS only on `cfg!(macos aarch64)` and `MacosArm64` (:249-253). (b) by normalized C symbol through `matches_platform_c_symbol` over a 30-row candidate table (:261-437). (c) service reach: each extra service id must have an identity text equal to one of six `toolchain::Filesystem*` / `DirectoryEnumeration` strings, or `module.core_filesystem_service(service)` must succeed (:528-550): the first is a string compare on identity text ("legacy labels", comment :545-547). (d) descriptor kind: guest fds 0..=2 are refused for non-modeled-stream calls (:790-794); unknown guest descriptor maps to -1 (:805); `-2` passes for `AT_FDCWD`-like directory (:795).
5. Cases by shape: the 30-row table at :261-437 is the complete list of supported libc/Darwin calls (`open`, `__open`, `creat`, `close`, `pread`, `pwrite`, `unlink`, `rmdir`, `realpath`, `mkdir`, `rename`, `link`, `symlink`, `chmod`, `chown`, `lchown`, `openat`, `unlinkat`, `fstatat`, `fstat`, `fchmod`, `fchown`, `ftruncate`, `lseek`, `futimens`, `fsync`, `fdatasync`, `flock`, `dup`, `__getdirentries64`). Paths are `UnsignedArray(4096, 8, SharedBorrow)` only (:258). `O_CREAT` without a mode is refused by checking `flags & 0x200` (:763: the Darwin value; used for the two `flags_without_mode` shapes only).
6. Same job elsewhere: Linux kernel-call table is `filesystem/linux.rs:176-215`; Windows table is `filesystem/windows_contracts.rs:39-453`; three tables of "formal" shapes. Guest descriptor numbering exists three times: here (:864 lowest free from 3), `host_filesystem.rs:179` (lowest free from 0 skipping modeled streams), `private_filesystem.rs:654` (lowest free index). Suspected duplication of the "closed standard descriptors reuse" rule: here refuses 0..=2 (:790) while `host_filesystem.rs` hands out 0.
7. Unused: `FilesystemCall::modeled_stream` (:116), `descriptor_io` (:113) have callers in `native_provider.rs` (read).
8. Stated limits: "pointer-valued optional parameters are rejected" (windows.rs:3-4); "No libc or Windows filesystem import may reach the host" under private namespace (:232-236).
9. Fail-open: (i) `FilesystemContext::new` with a `HostDirectory` that cannot be opened yields `supplied=false` so calls refuse (:195, stated comment :180-182). (ii) `admit_service_ids` accepts a service whose identity text exactly equals one of six labels without checking the schema ("Keep only the existing exact legacy labels", :545) : stated. (iii) `Unprovided` delegation still reaches `admit` but `process_namespace()` errors for the libc path (:468). Otherwise none seen. A wrong program that names a toolchain-labelled service with a legacy label string would obtain filesystem service reach: traced to the exact-string compare; the test at :976 checks near-misses fail.

## 7. `host/filesystem/linux.rs` (609)

1. Purpose: Linux kernel filesystem syscalls (`openat`, `read`-family, etc.) checked against declared ABI and executed either on the host fs (`LinuxFilesystem`) or the in-memory private fs.
2. Shape: `LinuxFilesystemWrite` (:70), `LinuxFilesystemImport` (:263, `admit` :277, handler :402-603), `Operand` enum (:134), `kernel_call` (:176, 24 operations), `Staged` (:393), `metadata_extent` (:607).
3. Inputs/outputs: operands classified `Directory/Descriptor/Value/NullAddress/Path/Input/Metadata/InputView/OutputView`; builds `Vec<KernelOperand>`, calls `private.call` or `LinuxFilesystem::call(number, ...)`; stages `struct stat` back on status 0 (:592-600).
4. How it decides: `matching_host` (:45): a kernel call is accepted when the target is Linux x64/arm64 AND (private fs present OR target == host). Operation by `LinuxFilesystemOperation::from_syscall(target, number)` (:293). Shape match uses `matches_parameters` against the per-operation `Formal` list. `NullAddress` must be 0 (:530). `Path` captured via `CapturedStorage` and `CStr::from_bytes_with_nul(terminated_prefix)` (:36-40).
5. Cases by shape: 24 operations at :182-208 are the complete table; `Utimensat` accepts only a null path (comment :201 "A null path sets the descriptor's own timestamps"). Metadata buffer is `vec![0; 144]` for both targets (:502) with extent 128/144 (:607-609).
6. Same job elsewhere: `private_filesystem.rs::dispatch` (:220) matches the same 24 operations by operand shape; the table at :182 and the dispatch must agree ("The admitted call table and this dispatch name the same operand shapes", private_filesystem.rs:369).
7. Unused: `LinuxFilesystemWrite::models_standard` used by `descriptor_io.rs:501`; `read`/`write` used at `descriptor_io.rs:548,504` (read). `LinuxFilesystemImport` used once (`host_imports.rs:483`).
8. Stated limits: lines 1-4. Comment :306 "A status-only call may discard its result; a count or offset may not".
9. Fail-open: `authority` computed at admission and stored as `Result`; execution fails if it is `Err` (:426); not fail-open. `Operand::Value` passes integers through unchanged including `i64` conversion of u128 (:514-520); a value that does not fit `i64` is refused. None seen.

## 8. `host/filesystem/windows.rs` (458)

1. Purpose: Windows file-operation call adapter with a per-invocation guest HANDLE table.
2. Shape: `Handles` (:19), `Call` (:69), `admit` (:76), `Call::{arguments (:239), result (:332), output_scalar (:364), prepare_readonly_record (:412)}`, `translate_attributes` (:157). One test (:446).
3. How it decides: only for `WindowsX64` locators and only on a Windows x86_64 host (:83-87). `arguments` checks, per symbol name, exact constant arguments (e.g. `CreateFileW`: security attributes null, template null, flags without `FILE_FLAG_OVERLAPPED` 0x40000000; `NtOpenFile` access 1114240/1114241, share 7, options 2113568; `MoveFileExW` flags `& !0x2b == 0`) by matching `self.symbol` bytes (:250-311). Keyed on source spelling of the symbol (static `&[u8]` taken from the contract row). `translate_attributes` checks `OBJECT_ATTRIBUTES` length 48, root handle nonzero (:161-186). Paths need a WCHAR terminator inside the captured array (:230-235).
4. Cases by shape: complete list of 22 symbol-specific argument rules (:250-310). `DuplicateHandle` demands current-process pseudo handles (-1) (:259-263).
5. Same job elsewhere: guest handle numbering `acquire` (:42) is the third guest-number allocator (see §6).
6. Unused: none seen.
7. Stated limits: lines 1-4 (pseudo-handles, async requests, pointer-valued optional parameters rejected).
8. Fail-open: `translate` maps unknown guest 0 to native 0 (:35) deliberately (null handle); `acquire` returns native `0` and `-1` unchanged without registering (:43-45). Unknown symbols produce `safe = false` and a refusal (:310-313). Classification: `translate(0)=0` open (no comment; reached when a program passes 0 as a handle, which is the null handle; hand-off to the OS).

## 9. `host/filesystem/windows_contracts.rs` (549)

1. Purpose: the table of exact Windows entry contracts (kernel32/ntdll) and the check that a binding's calling plan matches the Microsoft x64 C word placement.
2. Shape: `Contract` (:16), `select` (:21: 27-row table at :39-453), `validate_entry` (:482).
3. How it decides: row chosen by `matches_platform_c_symbol(binding, symbol, &[], Some(library))` (:456), then `matches_signature` over `Formal` lists. If the symbol matched but no row's signature matched it errors with the list of candidates (:466-478). `validate_entry` re-evaluates `evaluate_call_plan(MicrosoftX64, ..)` and compares with `binding.boundary_entry_plan.call` (:534); `ReadFile`/`WriteFile` are special-cased by hand-editing the plan (:500-520, `plan.parameters.remove(2)`).
4. Cases by shape: 27 rows; `GetFileInformationByHandleEx` has three class variants (0, 1, 9), `SetFileInformationByHandle` three (class 0, 6, disposition), `NtQueryDirectoryFile` two (12, 1).
5. Same job elsewhere: the calling-plan check duplicates `admit_exact_foreign_call` pattern (host_imports.rs:113).
6. Unused: none seen. Stated limits: none quoted beyond header.
7. Fail-open: `Ok(None)` when the symbol is not found in any row (:479): means "not a Windows fs call". None seen otherwise.

## 10. `host/native-sponsor/sponsor.rs` (349)

1. Purpose: launches the isolated worker with bounded process limits and installs compiler-produced adapter images by identity.
2. Shape: `for_adapters` (:22), `PublishedAdapterSession` (:36), `lookup_adapter` (:65), `install_image` (:76), `launch` (:91), `spawn_worker` (:105), `test_support` module (:182, feature gated), one test (:286).
3. Inputs/outputs: `Arc<NativeForeignAdapterImage>` roster; produces `NativeProviderCommand`; sends packet tag 1 with ordinal, export, bytes, digest, storage geometry (:82-87).
4. How it decides: `lookup_adapter` uses `candidate.matches_runtime_identity(requested)` (:71); the test checks that digest, application, locator, target, calling plan and scalar parameters each change the match (:330-347). Deadline must be in (0, 3600s] (:109). Process limits are fixed numbers (`120, 8 GiB, 64 MiB, 128, 16, 1 GiB, 2 GiB`, :139-146) with no named labels at the site.
5. Cases by shape: none beyond fixed limits.
6. Same job elsewhere: `NativeProviderProcess::from_sponsor` (`native_provider_runtime.rs:168`) re-checks the same deadline range (:174); era counter `NEXT_ERA` is duplicated at `sponsor.rs:112` and `native_provider_runtime.rs:173` (two separate statics).
7. Unused: `for_adapters` callers: grepped `for_adapters|read_host_clock_nanoseconds|PrivateLinuxFilesystem`: `application/src/execution/compilation.rs` has 1 hit (inferred to be `for_adapters`) plus the sponsor's own and test uses.
8. Stated limits: module line 1 "explicitly authorized native host".
9. Fail-open: none seen; unmatched identity returns an error (:73).

## 11. `host/native-worker/src/channel.rs` (344)

1. Purpose: private duplex channel between host and worker: Unix socketpair duplicated above fd 3 with `F_DUPFD_CLOEXEC`; Windows overlapped named pipe with process-id verification.
2. Shape: `unix::{PreparedChannel, prepare (:30), receive (:51)}`, `windows::{PreparedChannel, Channel, prepare (:144), accept (:179), receive (:210)}`; `Channel` implements `Read`/`Write` with overlapped I/O, `complete` (:244) is `unsafe`.
3. How it decides: Unix `receive` rejects raw fd < 3 and non-`SOCK_STREAM` (:52, :77). Windows checks `GetNamedPipeClientProcessId == child_process` (:200) and server process id equals parent (:230). On Unix there is no peer-identity check (the descriptor is inherited).
4. Cases by shape: n/a. Stated limits: "The pipe name supplies uniqueness rather than authentication" (:14-15).
5. Fail-open: none seen.

## 12. `host/native-worker/src/host_clock.rs` (52)

Purpose: `read_host_clock_nanoseconds(selector: u32)` (:12). Unix: `clock_gettime(selector as clockid_t)` then checks nonnegative and `tv_nsec < 1e9`. Non-Unix: only selector 0 (realtime) and 1 (monotonic via `OnceLock<Instant>` anchor); other selectors give `Unsupported` (:42-46). Decides by selector number. On Unix any clock id the host accepts passes; the caller `ClockNamespace::validate_selector` (`clock/bindings.rs:73`) is what narrows it (stated at lines 3-4). Fail-open: none seen.

## 13. `host/native-worker/src/host_filesystem.rs` (461)

1. Purpose: `LinuxFilesystem`: executes classified Linux syscalls on a Linux host with a guest descriptor table; optional rooted directory as `AT_FDCWD`.
2. Shape: `KernelOperand` (:36), `KernelResult` (:58), `LinuxFilesystem` (:65: `modeled_standard: [bool;3]`, `root`, `descriptors: BTreeMap<i32, Option<OwnedFd>>`), `call` (:120), `read`/`write` (:196,:211), `close` (:226), `kernel_status` (:287). Three Linux-only tests (:324-460).
3. How it decides: `libc::syscall` with up to six `c_long` arguments (:129-171); descriptors translate through the table, unknown guest -> -1 (:281), tombstone `None` after failed close (:244-246). Status -1 maps to `-errno` only for errno 1..=4095 else an error (:291-296).
4. Cases by shape: `KernelOperand::InputView/OutputView` expand to two words (:144-151); more than six words refused (:154).
5. Same job elsewhere: guest descriptor allocation (see §6).
6. Unused: `LINUX_AT_FDCWD` is re-exported and used by `private_filesystem.rs:24`.
7. Fail-open: `descriptor(guest)` returns `Ok(-1)` for an unknown guest (:281), passed to the kernel which answers EBADF: stated (comment :238-240). `libc::syscall(libc::SYS_close, -1)` for an unknown guest (:240). Non-Linux builds return `Unsupported` for everything.

## 14. `host/native-worker/src/lib.rs` (58)

Re-export list and `transport` module; `physical_tests` under `#[cfg(test)]` (:50). Comment block at :12-22 states why the worker is a separate process.

## 15. `host/native-worker/src/native_code.rs` (188)

Purpose: loads a compiler-produced adapter library and calls it through `NativeForeignFrame` (`arguments: *const *const u8`, `result`). Shape: `NativeValue` (:24, aligned zeroed allocation), `LoadedFrame` (:72: `load` unsafe :79, `call` unsafe :97), `Library` (dlopen/LoadLibraryExW). Decides nothing semantic. Libraries are never unloaded (`ManuallyDrop`, :74). `dlopen` with `RTLD_NOW|RTLD_LOCAL` (:119). Windows uses `LOAD_WITH_ALTERED_SEARCH_PATH` (:163). Stated: "Foreign C signatures are never cast at runtime" (:4). Fail-open: none seen; trust is documented as the authenticated parent (:77-78).

## 16. `host/native-worker/src/native_provider_runtime.rs` (584)

1. Purpose: `NativeProviderProcess`: the parent-side handle to one worker incarnation: installs adapters, creates shared homes, calls, closes.
2. Shape: ids `NativeAdapterId`/`NativeHomeId` (`era`, `ordinal`) (:36,:41); `NativeParameterStorage` (:47); `NativeArgument` (:53); `NativeProviderSession` trait (:110); `NativeProviderCommand` (:127); `NativeProviderProcess` (:154: `homes: BTreeMap<u64, SharedBacking>`, `adapters: Vec<NativeResultStorage>`, `provider_references`, `faulted`); `from_sponsor` (:168), `install_published_with_result` (:213), `install_admitted` (:230), `create_home` (:301), `call` (:367), `call_packet` (:399), `close` (:446), `exchange` (:476), `finish_exchange` (:484), `retire_process` (:508), `geometry` (:540). Test fixture in `test_session.rs`.
3. How it decides: era equality for ids (:336, :385, :415); `geometry()` accepts `OriginalReference` only when `shape == integer(8,8)` or a matching borrowed reference (:542-560), `OriginalByteView` only for `integer(16,8)` (:561), `Value` only for non-borrowed classes (:568-573). `create_home` refuses extent 0, > 16 MiB, non power-of-two alignment (:306). Any exchange failure sets `faulted` and retires the child (:489-504). Adapter must target `NativeTarget::host()` (:237).
4. Same job elsewhere: dense indexes (`homes.len()+1`, `adapters.len()`) are used as ordinals; the worker mirrors them (`worker.rs:194,256`).
5. Unused: `NativeProviderOperation`, `NativeAdapterStorage` are consumed in `sponsor.rs`.
6. Stated limits: comment :13-15 on polling `try_wait`.
7. Fail-open: `call()` for adapters with provider-reference results refuses (:372). Stale era ids refuse. None seen.

### `native_provider_runtime/test_session.rs` (93)
Test-only `TestProviderProcess` wrapping a `FixtureWorker`; `install` always errors in the session trait and real installs go via `install_admitted` closure (:83-88). No decisions.

## 17. `host/native-worker/src/physical_tests.rs` (281) and `physical_tests/` (2 files)

Real-process tests: `sponsorship_probe`/`worker_does_not_pass_its_handshake_to_children` (:26-67), shared-home page persistence (:70-193), pointer links reject foreign incarnations (:195-281). `native_calls.rs` (563): compiles C fixtures with `cc`/`clang` and tests retention, trap without replay (SIGILL/SIGTRAP => 4 on x86_64, 5 on arm64, :245-251; Windows `0xc000001d`), errno survival; reads fixture `samples/cli/facts/foreign_retained_shared_borrow/fixture/provider.c` (:43) and files `src/physical_tests/{process_exit,errno}.c` (referenced :87, :350; not in this unit). `provider_references.rs` (456): provider-cell occurrence, bounded reads, framing errors (nine malformed packets with asserted reason strings :367-376 = string compare on error text). Tests need an installed C compiler; macOS/Linux-only subtests are `cfg`-gated. Nothing here is fail-open.

## 18. `host/native-worker/src/pointer_links.rs` (170)

Purpose: host asks the worker to write or compare a pointer-valued slot inside a shared home without mapping addresses crossing IPC. Shape: `NativePointerSlot`/`NativePointerTarget` (:20,:25), `link_pointer` (:33), `pointer_matches` (:44), `returned_pointer_matches` (:57), worker side `Worker::pointer_link` (:138), `Worker::returned_pointer_matches` (:120). Decides by range checks (`SharedBacking::pointer(offset, extent, alignment)`) and byte equality with the worker-side address (:135, :167). Slot must be 8 bytes aligned 8 (:99, :159). Fail-open: `pointer_matches` returns `true` only on exact address equality; none seen.

## 19. `host/native-worker/src/private_filesystem.rs` (1417)

1. Purpose: in-memory Linux-kernel-shaped filesystem for test invocations ("Nothing here can reach a host path").
2. Shape: `PrivateLinuxFilesystem` (:130: `nodes: Vec<Option<Node>>`, `descriptions`, `guests: Vec<Guest>`, `stored`, `arm64`), `Node`/`Contents` (:96, :87), `dispatch` (:220), path `lookup`/`walk` (:449-560), `openat` (:743), `stat` (:952, two struct-stat layouts), `getdents64` (:1017), `flock` (:1232). Five tests (:1307-1416).
3. How it decides: dispatch by `(operation, operand slice pattern)` (:227-372), any non-matching shape gives `EINVAL` (:371). Permission checks use only owner bits (`permits`, :415). Limits are constants: `NAME_MAX 255`, `PATH_MAX 4096`, `SYMLINK_DEPTH 40`, `DESCRIPTOR_LIMIT 1024`, `CAPACITY 64 MiB` (:68-73); these are modeled kernel numbers, not invented ones (comment :12-18). `chown` accepts only ids `-1`, `0`, `u32::MAX` (:1070).
4. Cases by shape: 24 operations; target selects open-flag values (`directory_flag`, `nofollow_flag` :744-748) and stat layout (:982-1000) between x86_64 and arm64.
5. Same job elsewhere: same 24 operations as `host_filesystem.rs::call` and `linux.rs::kernel_call` (host vs private implementation).
6. Unused: `Default` impl (:146) plus `new` (:154).
7. Stated limits: comment :16-18 "dup shares one open file description, and a removed file lives until its last description closes"; `flock`: blocking conflicts report `EDEADLK` "rather than hang the runner" (:1228-1230).
8. Fail-open: `fsync/fdatasync` return 0 after only checking the descriptor (:344-347); `utimensat` ignores its flags (`K::Value(_flags)` :341); `chmod` accepts any mode and `fchownat` ignores `AT_EMPTY_PATH`. These accept inputs the real kernel would refuse in some cases (e.g. `chmod` by a non-owner is impossible here because the process owns all nodes: comment :413-414 "The process owns every node"). Classification: stated for ownership; open for the ignored `utimensat` flags.

## 20. `host/native-worker/src/provider_references.rs` (140)

Purpose: parent-side API for retained result "provider references" (an occurrence handle into the worker). `NativeResultStorage` (:12), `NativeProviderReference` (:20), `call_provider_reference` (:39), `read_provider_reference` (:75), `release_provider_reference` (:106). Decides by era, adapter ordinal equality, `occurrence_index` dense check (:31), and bounds `offset+len <= byte_size` (:86-92). Occurrences are `provider_references.len()+1` (:48-53), never reused. Fail-open: none seen.

## 21. `host/native-worker/src/shared_backing.rs` (216)

Purpose: `SharedBacking`: file-backed shared mapping (`mmap`/`MapViewOfFile`) with bounds/alignment checked pointer, and copy read/write. `create` uses `create_new` and mode 0600 (:145-150); `attach` requires exact file length (:158). `pointer` checks power-of-two alignment and `offset+extent <= extent` (:182-198). Fail-open: none seen. Files are never truncated (header :5-6).

## 22. `host/native-worker/src/wire.rs` (207)

Purpose: length-prefixed packet framing. `MAXIMUM_PACKET_BYTES = 16 MiB` (:6, a named cap used throughout), `send`/`receive`/`exchange` (:10,:18,:30), `Packet` (:49), `Cursor` (:87), `Geometry`/`BorrowGeometry` (:136,:143). `Geometry::read` re-validates extents, power-of-two alignment, `Fixed` borrow requires extent 8 and alignment 8, `ByteView` requires 16/8 (:174-200). Response tag 0 = ok, 1 = error text, else malformed (:34-42). The cap is a named invented limit (see "Stated limits"); not checked against any spec page. Fail-open: none seen.

## 23. `host/native-worker/src/worker.rs` (480)

1. Purpose: the worker loop (`run_worker_if_requested` :100): consumes the sponsor handshake from env, serves tags 1 install, 2 attach, 3 call, 4 close, 5/6 pointer link/compare, 7 returned-pointer compare, 8/9 provider-reference read/release.
2. Shape: `Worker` (:54: `adapters`, `homes`, `provider_references`, `guest_errno`), env names `CHANNEL_ENV/PARENT_ENV/ROOT_ENV/TIMEOUT_ENV` (:32-35), `take_sponsorship` (:81), `install` (:192), `attach` (:252), `call` (:267), `capture_provider_reference` (:373), `read_provider_reference` (:410).
3. How it decides: install verifies `Sha256(bytes) == digest` where the digest arrives in the same packet (:200); ordinal must equal `adapters.len()` (:194); home id must equal `homes.len()+1` (:256). Calls match each parameter's wire tag (0 value, 1 fixed loan, 2 byte view) against its installed geometry (:280-310). A provider reference result must be non-null, aligned and non-overflowing (:385-392). Reads use `ptr::with_exposed_provenance(address+offset)` and `copy_nonoverlapping` with a provider fault contained to the worker (:432-439). Any request error answers once and ends the worker (:179-184). The handshake env vars are removed after reading so child processes do not become workers (:81-97).
4. Cases by shape: tag list above; result geometry must not be `ByteView` or zero-extent fixed (:218-227).
5. Same job elsewhere: `SharedBacking::pointer` bounds checks are called from `native_provider_runtime.rs:314` and worker; dense-ordinal checks duplicate across host and worker.
6. Stated limits: "Code and mappings remain resident ... until `std::process::exit`" (:6-7).
7. Fail-open: the digest check proves integrity of the received bytes against a digest sent by the same sender; it does not identify which published adapter it is (the parent admitted it). Classification: stated (comment `native_code.rs:77-78`: "the isolated worker has authenticated its producing parent"). On Unix, authentication of the parent is only descriptor inheritance (`channel.rs:51`).

## 24. `host/retained-image/execution.rs` (142)

1. Purpose: runs a program through `terminal_runtime` after turning an optional retained native image's code layout into `TerminalNativeCodeLayoutEnvironment`.
2. Shape: `execute` (:23), `execute_sliced` (:30), `inspect` (:50), `require_valid_inspection` (:98). Two tests (:109,:122).
3. How it decides: image used only if both artifact manifest identity and view identity equal the request's, and the image target equals the selected target (:67-74). Failed inspection is an error, not "unavailable" (comment :96-97). With `image = None` it forwards unchanged (:35-37).
4. Same job elsewhere: `terminal_runtime::execute_sliced` (`execution.rs:185`) does the prepare/admit steps without layouts; this file repeats them with layouts (:38-47) and `execute_with_image_installation` (`execution.rs:200`) re-admits (so `admit_runtime_supplies` runs twice on the image path: :42 and `execution.rs:208`).
5. Fail-open: none seen. Target comes from `request.target` or provisions (:55-59).

### `host/retained-image/tests/retained_runtime.rs` (150)
One test: loads a checked-in fixture `tests/fixtures/calls/run/scalar_contract_route_exit/linux_x86_64.psi` (:14-16), compiles natively, then asserts: a differently-published artifact with equal view identity but different manifest identity is refused by both runtime paths (:90-108), wrong target refused (:109-124), `normal`, `sliced`, `published`, `published_sliced` all complete unit with equal suspension counts and no decode (`module_decode_count` unchanged :149). Fixture is a binary `.psi` file; not read.

## 25. `host/src/execution.rs` (421)

1. Purpose: public entry for running a Terminal artifact under a host request; owns `TerminalRuntimeRequest`, `TerminalRuntimeObservation`, `TerminalRuntimeError`, `HostedTerminalProgram::prepare`.
2. Shape: delegation enums `TerminalDomainExitDelegation` (:28), `TerminalFilesystemDelegation` (:35: `Unprovided`/`HostNamespace`/`HostDirectory`/`Private`), `TerminalNativeServiceDelegation` (:54); `execute` (:179), `execute_sliced` (:185), `execute_with_image_installation` (:200), `HostedTerminalProgram::{prepare (:230), supplies (:355)}`, `capture_host_x86_flags` (:375), `layout_target_facts` (:389).
3. How it decides: `prepare` refuses when the entry names another artifact (:236), entry and provisions disagree (:241-249), provisions do not match the artifact (:250-256), provisions target differs from request target (:257-263), retained provider installation differs from proposed (:290-297). `comparison_unavailable` classifies errors by variant (:119-134); `unavailable_before_guest_effects` excludes `Execution` and the `native-provider-retirement` capability (:136-146); the capability is compared by string constant (:139).
4. Cases by shape: `TerminalRuntimeError` classification lists four unsupported `TerminalInterpretError` variants as "unavailable" (:127-133).
5. Same job elsewhere: `components.rs::prepare` repeats the target-machine/layout admission for components (:30-37).
6. Unused: `into_request` (:347); `capture_host_x86_flags` is used by tests (`retained_runtime.rs:63`).
7. Fail-open: `comparison_unavailable` returns `true` for every `MissingProvision` (:121): the caller treats the run as "not comparable" rather than a failure; `unavailable_before_guest_effects` excludes a retirement failure. Classification: stated by the unit test at :411-420. A wrong program that leaves a `MissingProvision` raised after guest effects other than retirement would still be labelled unavailable-before-effects; traced to the single exception list.

## 26. `host/src/execution/components.rs` (98)

Purpose: recursive preparation of producing component instances. `prepare` (:12) checks runtime companions name this occurrence (identity, entry, authority subject :22-28), builds target context and layouts, admits supplies, registers child providers, binds children recursively (:83-88), and binds host imports (:90-96). Decision keyed on identity equality. Recursion depth is the component dependency depth (no explicit limit). Fail-open: none seen. Skips host provisions realized as `Component` mechanism (:42-47), same skip as `execution.rs:320-325` (duplicated condition).

## 27. `host/src/execution/entry.rs` (109)

Purpose: builds `EstablishedEntry` from checked private entry RAM recipes. `establish` (:34) requires no scalar parameters, the count of access-view inputs equals recipe count (:45), target schema `HostedApplication` (:51-53). For each structural parameter finds the recipe by `runtime_position` (:67-69), checks `terminal`, `machine` and `carrier` equality (:74-85), mints descriptor and original identities with `private_entry_ram_identity_pair` (:86-96). Stated: no zeroed backing inferred from a signature alone (:2-3). Fail-open: none seen. The entry machine is looked up twice (:39, :76-79).

## 28. `host/src/execution/host_imports.rs` (681)

1. Purpose: the import binder: for each demanded boundary slot picks and constructs the typed adapter from its admitted realization.
2. Shape: macros `import_signature_via_clone!` (:16), `handle_effect_requires_dispatch!` (:26); `HostBoundarySelections` (:71); `admit_exact_foreign_call` (:113); `CallOperands`/`call_operands` (:164,:172); `matches_platform_c_symbol` (:215); `HostContext` (:274); `HostBindingTarget` trait (:325); `bind` (:409-681); `bind_numeric_import_slot` (:395). Submodules listed :37-55 (`native_word`, `network_address`, `scalar_codec`, `scalar_record`, `service_reach`, `values` not in this unit).
3. How it decides: by `BoundarySettlementRealization` (`Builtin(...)` vs `NormalizedForeignCall`): builtins `LinuxWriteLine`, `HostedWriteByteI32` become `BufferedOutput`; `HostedReadByte` is retired with an error (:448-452); numeric floats bind through `interpreter_numeric_imports`. For a foreign call, a fixed ordered chain of 14 `admit` attempts, first `Some` wins (:483-626): Linux fs, libc/Windows fs (and descriptor-I/O via native), abs (`AbsoluteValueImport`), allocation, clock, descriptor I/O, affine transform, abs (`IntegerAbsoluteImport`), `lldiv`, `strtol`, network address, `_setmode`, native provider; otherwise an error naming the boundary identity text (:627-639). Emulated allocation is on when any admitted slot is `calloc` (:424-432).
4. Cases by shape: the ordered chain above is the complete list.
5. Same job elsewhere: `AbsoluteValueImport` (`absolute_value.rs`) and `IntegerAbsoluteImport` (`integer_absolute.rs`) both implement C `abs` over i32 and share `matches_*` locator sets (see §29, §41). Grepped `IntegerAbsoluteImport|AbsoluteValueImport`: 11 occurrences in 3 files (`host_imports.rs`:2, `absolute_value.rs`:5, `integer_absolute.rs`:4). Because `absolute_value.rs`'s locator set includes every target `integer_absolute.rs`'s `matches_platform_c_symbol` accepts (Windows msvcrt, Linux glibc, Darwin libSystem) and runs first, the second is unreachable (inferred; not run).
6. Unused: see above. `HostContext::{new, finish_native_provider}` have callers outside this unit (`hosted_invocation.rs`, not read).
7. Stated limits: lines 5-13 ("Modeled services pass `admit_exact_foreign_call` ... plus family-specific shape rules").
8. Fail-open: `admit_exact_foreign_call` accepts a call with no `normal_qualifications`, no `content_guarantees`, no `program_local_root_introductions`, no `crash_routes` (:137-145) and normal completion (:146-150); it then validates the plan (:151). `bind` returns `Err` if no adapter matches (:627). In the builtin path an unknown `BoundarySettlementRealization` gets `slot ... needs a runtime adapter` (:641-648). None seen.

## 29. `host/src/execution/host_imports/absolute_value.rs` (267)

Purpose: C `abs` (`AbsoluteValueImport`): i32 scalar or owned one-scalar record envelope; result i32 or discarded unit. `admit` (:31), handler (:127), `matches_locator` (:221: msvcrt, glibc 2.2.5/2.17, libSystem both install names). Computes absolute with `checked_abs` in Rust (:202-205). Requires calling application: plan equals binding plan, commitment nonzero, no opaque applications (:82-87). Same job as `integer_absolute.rs` (see §41). Fail-open: none seen; `i32::MIN` refused (:202-209).

## 30. `host/src/execution/host_imports/affine_transform.rs` (111)

Purpose: macOS CoreGraphics `_CGAffineTransformMake` through `scalar_record::ScalarRecordAdmission<6>`. Only on `cfg!(macos aarch64)` and `MacosArm64` locator (:29-33), locator equality (:46). Calls `runtime_abi::host_affine_transform` (:87), returns an aggregate of six binary64 fields (:91-108). A single hardcoded framework path (:37). Fail-open: none seen.

## 31. `host/src/execution/host_imports/allocation.rs` (501) + `allocation/admission.rs` (266) + `allocation/backing.rs` (135)

1. Purpose: emulated `calloc`/`free`/`memset(0)` over invocation-owned virtual storage; addresses never recycle; opaque host addresses are recorded in the same ledger and kept disjoint.
2. Shape: `AllocationValues` (:49: `allocations: Vec<Allocation>`, `next_address`, `host_addresses: Vec<u64>`), `allocate` (:70), `backing_for_range` (:56), `admit_host_address` (:115), `is_host_address` (:134), `release` (:138), `AllocationImport` (:156). `admission.rs`: `select` (:16), `validate` (:45), `AddressLocation` (:39). `backing.rs`: `AllocationStorage::resolve_range` (:40) mints `ExtentRootGrant`/`ResourceProfileGrant` for a live range. Tests in all three (:434, :110).
3. How it decides: addresses start at 16 and align up to 16 (:82-86); overflow or failed reservation returns address 0 (like C) (:76-102); `free(0)` is `Ok` (:139); unknown free is refused (:146-150); `memset` accepts only fill value 0 (:407) and a range inside one allocation (:411). Linear scans over `Vec<Allocation>` (:58-67, :144). `emulates_allocation` gates `free`/`memset` admission: without emulated `calloc`, they are left for the native import (:177-179). Ledger issuer is `call.boundary.get()` (:281). Resource profile: one region, `Exclusive`, `ReadWrite`, no atomic, no external (`backing.rs:90-100`).
4. Cases by shape: `calloc` result is a bare scalar or a one-scalar record (`RecordResult` :42); `free` argument bare scalar or owned record; `memset` takes owned record + i32 + u64 and returns a mutable reference (`admission.rs:164-253`). Native word envelopes must be 8/8 or 4/4 (`admission.rs:92,152,222`).
5. Same job elsewhere: the "single scalar owned record carrying an address" extraction is `native_word::scalar_payload_path` (not read) used by abs, free, memset, calloc; `AllocationValues` dense-id style is not used (Vec scan; "no `HashMap`" rule is respected, but lookup is linear: `find` at :60-66, suspected O(n) per access).
6. Unused: `AllocationValues::is_host_address` used by `original_homes.rs:285`; `admit_host_address` by `original_homes.rs:309` (read).
7. Stated limits: comment lines 8-9 about empty-range ownership; `backing.rs:1-4` receipt identities "scoped to this host store".
8. Fail-open: `allocate` returns `Ok(0)` rather than an error when the size overflows or memory cannot be reserved (:76-102): matches C `calloc`; stated by the function. The `ExtentProviderIssuance` identity array is filled with `id`/`issuer` repeated (`backing.rs:66-69`: thirteen entries, 7 times `id`, 6 times `issuer`): normalized identity assignment without names. None seen otherwise.

## 32. `host/src/execution/host_imports/borrowed_storage.rs` (439) + `borrowed_storage_geometry.rs` (184)

1. Purpose: joins semantic storage loans to native byte placement. `CapturedStorage::capture` (:164) encodes captured scalar/case cells into a byte vector at layout offsets, checks coverage, and `stage` (:344) decodes the written bytes back into `TerminalBoundaryStorageBuffer` values. `PointerOutputPlan` (:44) validates a pointer-output field.
2. Shape: `StorageTransfer { Observe, ReplacePrefix }` (:30), `CapturedStorage` (:35), `terminated_prefix` (:308), `byte_path_at` (:326), `covered_range` (:427). Geometry: `NativeCell`/`NativeCellKind` (`Scalar` | `Case`), `expand` (:65), `expand_coordinates` (:136), `input_coverage` (:102), `is_active` (:165).
3. How it decides: paths compared as `Vec<Path>`; cells sorted by path and duplicates refused (:179-182); overlap or past-layout offsets refused (:256-258); `Observe` requires the first `byte_count` bytes fully covered (:270-274); `ReplacePrefix` needs possible coverage by scalar or discriminator recipes (:275-286); stage decodes only active cells and requires coverage or "retained" bytes (:383-387); discriminator tag must name a declared case (:407-414). `is_active` binary-searches the discriminator value for each `Case` path segment and errors if absent (:171-173).
4. Cases by shape: `layout=None` supports only one scalar at the empty path (:184-194, :232); layouts need `FixedIndex(0)` placeholders for repetition (geometry :150).
5. Same job elsewhere: `PointerOutputPlan::prepare` repeats path expansion of `expand` for opaque regions (:100-127).
6. Unused: none; `byte_path_at` has one caller (`integer_parsing.rs:251`, read).
7. Stated limits: lines 1-11. Fail-open: `covered_range(.., start, end)` returns true when `end <= start` (:427-438), meaning an empty requested range is covered: fine for zero-length reads. In `capture`, `Scalar` buffers are accepted only when no layout is given (:184). None seen.

## 33. `host/src/execution/host_imports/buffered_output.rs` (209)

Purpose: invocation `stdout`/`stderr` buffers (`Arc<Mutex<Vec<u8>>>`). `append(descriptor, bytes)` accepts 1 or 2 only (:37-43). `BufferedOutput` writes a line plus `\n` or the low 8 bits of an i32 (:134). Operands checked against the signature (:95-108). Byte output rejects values outside i32 (:131). Test :148. Fail-open: none seen.

## 34. `host/src/execution/host_imports/descriptor_io.rs` (649)

1. Purpose: descriptor `read`/`write` over finite stdin replay and private output, optionally delegating non-standard descriptors to the Linux guest fs.
2. Shape: `DescriptorIoImport` (:88), `DescriptorContract` (:78), `admit` (:96), `for_filesystem` (:130), `validate_native_transfer` (:171), `admit_contract` (:226), handler (:386-603), `input_error_status` (:46), `validate_transfer_count` (:347).
3. How it decides: ABI/operation from `runtime_abi::descriptor_system_call` or `descriptor_import` on the binding (:217-223); buffer kind from the structural type shape (scalar, byte view, record/array/sum via layout; :251-281); access: read needs `MutableBorrow`, write a reading-borrow pair (:282-287). Linux transfers over `0x7fff0000` bytes are refused (:351). Read path: descriptor 0 reads stdin; other descriptors refuse unless Windows CRT (returns -1) or Linux fs delegation is present (:509-521). Write path: Windows CRT writes to descriptors other than 1/2 return -1 (:495-499); otherwise non-1/2 descriptors reach `output.append`, which refuses (`buffered_output.rs:40`). Errors map by ABI: Windows/Darwin -1; Linux kernel keeps a host errno only on Linux hosts, else refuse (:46-70).
4. Cases by shape: `DescriptorBuffer::{Scalar, Aggregate, Bytes}`.
5. Same job elsewhere: guest descriptor bookkeeping duplicates `filesystem/bindings.rs`.
6. Unused: none seen.
7. Stated limits: comment :14-22; "The host explicitly assumes these implementations are equivalent" (:21).
8. Fail-open: Windows CRT `read`/`write` on an unsupported descriptor returns -1 instead of an error (:498, :520): a program that writes to fd 3 under Windows CRT sees an I/O failure, not a refusal. Classification: stated (module comment says unsupported descriptors "refuse before I/O"; this path returns a status, so the comment and code differ: observed, open).

## 35. `host/src/execution/host_imports/descriptor_mode.rs` (136)

Purpose: MSVCRT `_setmode` with a private per-descriptor mode map initialised to `O_TEXT 0x4000` (:21). Only for Windows locators `msvcrt.dll`/`_setmode` (:40-52); descriptors outside 0..=2 return -1 (:130). Stores any i32 mode. Fail-open: any mode value is accepted and returned later; none checked against known `_O_*` flags (open; no comment).

## 36. `host/src/execution/host_imports/embedding.rs` (41)

`HostContext::{bind_instance, prepare_component, output}` for embedders. Pure delegation. Callers: grepped `execute_with_image_installation|bind_instance|prepare_component|establish_host_entry` over `source-rust`: 11 occurrences in 5 files, so `bind_instance`/`prepare_component` have outside use only through `hosted_invocation.rs` (4) (inferred; I did not read it).

## 37. `host/src/execution/host_imports/integer_absolute.rs` (233)

Same job as `absolute_value.rs` (§29): C `abs`, result checked through `runtime_abi::host_integer_absolute` (:221) rather than a Rust `checked_abs`; includes verbose `format!` debug strings in refusals (:174-184, :162). Locator via `matches_platform_c_symbol` (:32), requires host target. See §28.5: probably unreachable because the earlier adapter claims the same locators (inferred).

## 38. `host/src/execution/host_imports/integer_division.rs` (109)

`lldiv` (`ScalarRecordAdmission<2>`): selects via `matches_platform_c_symbol` (Linux uses glibc, Darwin libSystem, Windows `api-ms-win-crt-utility-l1-1-0.dll`, :26-31). Inputs two i64; calls `runtime_abi::host_integer_division` (:83) and returns an aggregate with quotient and remainder fields. Refuses undefined (zero divisor/overflow) results via `None` (:83-85). Fail-open: none seen.

## 39. `host/src/execution/host_imports/integer_parsing.rs` (305)

Purpose: C `strtol` over captured typed storage; the end pointer becomes a typed reference output. `admit` (:35), handler (:203), `matches_locator` (:269: Linux/macOS host only, glibc 2.2.5/2.17, libSystem). Requires one authenticated normal-completion reference output with destination `Parameter(1)` and byte referent (:79-114), unique layouts for input/output (:123-147), `PointerOutputPlan::prepare(.., 8)` (:148). Parsing uses `runtime_abi::parse_host_long` on the captured NUL-terminated prefix (:247); endpoint selection by `byte_path_at(offset)` (:251). Cases: input shared or mutable borrow; output mutable only. Fail-open: none seen.

## 40. `host/src/execution/host_imports/native_provider.rs` (979)

1. Purpose: `NativeImport`: admits and runs a produced native adapter in the worker; handles scalar, record, reference and provider-reference results, borrowed original homes, filesystem/descriptor virtualization, crash-as-`Trap`.
2. Shape: `Parameter` (:120: Scalar, Structural, ByteView), `ResultCodec` (:136: Unit, Scalar, Record, Reference, ProviderReference), `NativeImport` (:150), `admit` (:179-449), `admit_parameters` (:471), `validate_scalar_shape` (:454), handler `handle_effect_with_buffers` (:681-955), `actual_trap` (:958). Re-exports `NativeContext`, `rejection`, `IntegerRecordField` etc. (:23-27).
3. How it decides: requires `native.available()` (explicit sponsor); host native target == locator target (:197); descriptor operations require a filesystem object (:200); additional services checked by `service_reach::admit_additional_services` (:206, not read); `provisions.matches_host_requirement`, calling application and native product equality (commitment, locator, bridge target, plan) (:233-267); native telescope must be the exact semantic formal order with no private parameters (:496-520); results: unit/scalar/reference/record/provider-reference by shape (:299-418). Runtime: `actual_trap` maps worker exit signal 4 or 5 (SIGILL/SIGTRAP) or NT codes `0xc000001d`/`0x80000003` to `TerminalEffectResult::Crash(Trap)` (:823-825, :958-978) , every other worker error becomes a refusal.
4. Cases by shape: comment :6 "Admission joins the boundary declaration..." ; provider reference result needs `SharedBorrow` access plus a safe frontier with an empty result path (:366-377); owned records go through `record_shape::validate` (:576); borrowed access limited to Shared/Mutable (:589-599); other access kinds: "native import requires an initialized readable input codec" (:602).
5. Same job elsewhere: `Parameter::Structural` byte-for-byte equals the codec in `values.rs` (not read); the post-call loop over MutableBorrow parameters (:856-920) repeats `ReferenceOutputs::complete`'s buffer walk (`reference_outputs.rs:249`).
6. Unused: `NativeImport::admit` called once (`host_imports.rs:502,614`).
7. Stated limits: lines 9-13 ("The private worker supplies no additional service authority on its own").
8. Fail-open: `if filesystem.is_none() { service_reach::admit_additional_services(...) }` (:205-222): the service-reach check is skipped when a filesystem call object is present; there the `FilesystemContext::admit` path performed its own `admit_services` (`bindings.rs:470`) but the descriptor-I/O branch bypasses its result (`bindings.rs:471-474` ignores `namespace_admission` for `Io`). Classification: traced to reach only for descriptor I/O imports whose standard streams are modeled; stated in `descriptor_io.rs:11-14`. Crash mapping: only two signal numbers become guest `Trap`; SIGSEGV does not. A trap-like exit with another signal is a refusal.

## 41. `host/src/execution/host_imports/native_provider/original_homes.rs` (1054)

1. Purpose: `NativeContext`/`Sponsor`: lazily start the worker, create one shared "home" per authenticated interpreter root, keep resident scalar backings, link and verify pointer slots, lease provider-result references.
2. Shape: `Sponsor` (:36: `command`, `process`, `failure`, `references`), `NativeContext` (:172: `homes: Rc<RefCell<BTreeMap<(u64,u64), OriginalHome>>>`), `OriginalHome` (:181), `Resident` backing (:191), `prepare` (:326), `prepare_scalar` (:447), `reference_slot` (:532), `record_reference` (:555), `validate_references` (:578), `loan` (:601), `byte_loan` (:667), `exact_layout` (:728), `scalar_location` (:744), `reference_location` (:802), `host_address_value` (:841).
3. How it decides: first use of a root creates a home sized to `extent.max(1)`; later requests must match root, structural type, extent and alignment (:353-377). `prepare_scalar` finds the cell by exact path; a repeated cell must keep offset/codec and return `Rc::ptr_eq` backing and an equal current value (:478-497). `loan` derives the home base offset by subtracting each field offset from its cell's offset, all fields must agree (:613-634), then checks alignment, extent (:654-663). Worker start has a fixed 30-second timeout (:56). Host addresses: only `HostAddress` values or integer 0 reach a native import (:841-861). A native provider failure is sticky (`failure` string) so later calls refuse (:48-50).
4. Cases by shape: raw byte homes use alignment 1 and only `u8` unsigned bytes at `[FixedIndex(i)]` (:459-473); reference slots must be 8-byte regions aligned to 8 (:815-818); byte views must be contiguous windows (:700-704).
5. Same job elsewhere: `scalar_location` (:744-800) and `expand` in `borrowed_storage_geometry.rs:65` both expand layout fields with fixed-index repetition; `loan` and `byte_loan` repeat the offset-agreement walk.
6. Unused: none seen.
7. Stated limits: lines 1-10.
8. Fail-open: `validate_references` returns an error if a slot has no target (:582). `Sponsor::finish` with `failure.is_some()` drops the process and returns `Ok` (:152-156) without reaping the worker via `close`; stated by the test at :1020-1030 and comment :145-146. A fixed 30 s deadline is a hardcoded literal (:56). Test at :891 refers to `crate::run_native_provider_worker_if_requested`, defined at `host/src/lib.rs:45` (grepped).

## 42. `host/src/execution/host_imports/native_provider/provider_results.rs` (133)

Purpose: live leaf backings for a provider reference result. `LeaseRegistry` (:19: `active: Cell<usize>`, `released: RefCell<Vec<_>>`), `Lease` (:55) whose `Drop` only decrements and queues (:61-68), `Scalar` backing (:70) reading through `Sponsor::read_provider_reference`, writing is refused (:98-102), `complete` (:105). Decides nothing semantic; custody only. Fail-open: none seen.

## 43. `host/src/execution/host_imports/native_provider/record_fields.rs` (358)

Purpose: derive the complete scalar and reference leaf roster of a record/array type with explicit stack (no recursion). `Schema::derive` (:91), `fields` (:258). Only records, fixed arrays with `ArrayExtent::Literal`, scalar fields (`scalar_type` :244 accepts `Scalar`, `BoundedInteger`, `RefinedScalar`, `IeeeFloat`) and references; quotient carriers, access-view/accessor storage, sums refuse (:114-157, :220-224). Cycle detection by `shape.is_none()` (:101). Linear `nodes.iter().find` by identity (:100). Same job as `record_shape.rs::Schema` (§44): two schema walkers over the same type graph. Fail-open: none seen.

## 44. `host/src/execution/host_imports/native_provider/record_shape.rs` (782) and `record_shape_tests.rs` (655)

1. Purpose: joins an owned aggregate's public ABI shape graph (`BoundarySignatureGeometry`) to its semantic tree and `StorageCodec`; also pure shape predicates used by filesystem tables.
2. Shape: `validate` (:409), `validate_instances` (:685), `matches_integer_storage` (:180), `matches_integer_record` (:236), `matches_signed_record_array` (:98), `IntegerRecordField` (:170, `Scalar`, `Array`, `ReferenceRecord`, `ReferenceArray`).
3. How it decides: every scalar leaf's full path, offset, logical type and stored carrier must equal the codec field at that leaf index (:709-714); class (integer vs float) and byte size must match (`validate_scalar_shape`); children must lie inside the record extent and not overlap (:513-525); array stride times length equals extent (:563-566); cycles refused through `active_types`/`active_shapes` (:438-441); only plain scalar or finite record/array storage (`_ =>` arm :579-583).
4. Cases by shape: the predicates `matches_*` encode fixed shapes (integers 8,16,32,64; arrays of fixed count). Used by `filesystem/bindings.rs` (grepped `matches_signed_record_array|matches_integer_record\(|matches_integer_storage\(`: 11 occurrences in 3 files).
5. Same job elsewhere: `record_fields.rs::Schema` (see above).
6. Tests (`record_shape_tests.rs`, 15 `#[test]`): each pins one refusal (repeated children, empty records, class/width/offset changes, cycles, unsupported descendants, arrays, zero arrays, array of arrays, authored stored width, restrictions as payload not ABI, standalone roots). No `.omg` fixtures; types built in code.
7. Fail-open: `matches_*` return `false` on any mismatch; `validate` returns `Err`. None seen.

## 45. `host/src/execution/host_imports/native_provider/reference_outputs.rs` (348)

Purpose: correlates actual worker pointers with captured typed endpoints for reference outputs and returned references. `ReferenceOutputs::admit` (:45) clones the whole `program.module().types.structural_types` vector (:51), builds `codecs` for every layout dropping failures via `.ok()` (:59-66); `input_reference` (:84), `returned_reference` (:191), `complete` (:249). Decisions: a candidate capture is selected only if its type equals the output's referent, access admits (`admits_referent`), the output's lifetime dependencies anchor it (`anchored` :156) and the worker confirms pointer equality (`pointer_matches`, :122, :307; `returned_pointer_matches`, :235). The comment says pointer equality only selects a candidate and "Core authenticates" the rest (:1-4). First match in capture order wins (`break` :313). Fail-open: failed codec admission silently omitted from `codecs` (:59-66): later lookups produce a refusal ("has no exact codec"), traced; none otherwise.

---

## Cross-cutting counts and greps (commands run)

- `Grep IntegerAbsoluteImport|AbsoluteValueImport` (source-rust, count): 11 occurrences / 3 files.
- `Grep run_native_provider_worker_if_requested`: 4 hits (`application/src/main.rs:11,50`; `host/src/lib.rs:45`; `original_homes.rs:892`).
- `Grep execute_with_image_installation|bind_instance|prepare_component|establish_host_entry`: 11 / 5 files.
- `Grep for_adapters|read_host_clock_nanoseconds|PrivateLinuxFilesystem`: 22 / 8 files.
- `Grep capture_host_epoch|.models_standard(|link_pointer(|returned_pointer_matches(`: 23 / 9 files (capture_host_epoch: 1 definition hit in `clock/bindings.rs`).
- `Grep matches_signed_record_array|matches_integer_record\(|matches_integer_storage\(`: 11 / 3 files.

## Coverage

Read in full (53 files, 21,853 lines):
`embedding/reference_completion_tests.rs` 1-2303; `host/clock/{admission 1-189, bindings 1-500, timespec 1-178, windows 1-212}`; `host/filesystem/{bindings 1-1301, linux 1-609, windows 1-458, windows_contracts 1-549}`; `host/native-sponsor/sponsor.rs` 1-349; `host/native-worker/src/{channel 1-344, host_clock 1-52, host_filesystem 1-461, lib 1-58, native_code 1-188, native_provider_runtime 1-584, native_provider_runtime/test_session 1-93, physical_tests 1-281, physical_tests/native_calls 1-563, physical_tests/provider_references 1-456, pointer_links 1-170, private_filesystem 1-1417, provider_references 1-140, shared_backing 1-216, wire 1-207, worker 1-480}`; `host/retained-image/{execution 1-142, tests/retained_runtime 1-150}`; `host/src/execution.rs` 1-421; `host/src/execution/{components 1-98, entry 1-109, host_imports 1-681}`; `host/src/execution/host_imports/{absolute_value 1-267, affine_transform 1-111, allocation 1-501, allocation/admission 1-266, allocation/backing 1-135, borrowed_storage 1-439, borrowed_storage_geometry 1-184, buffered_output 1-209, descriptor_io 1-649, descriptor_mode 1-136, embedding 1-41, integer_absolute 1-233, integer_division 1-109, integer_parsing 1-305, native_provider 1-979, native_provider/original_homes 1-1054, native_provider/provider_results 1-133, native_provider/record_fields 1-358, native_provider/record_shape 1-782, native_provider/record_shape_tests 1-655, native_provider/reference_outputs 1-348}`.
Skimmed: none. Not read: none. (Modules referenced but outside this unit and not read: `host_imports/{native_word, network_address, scalar_codec, scalar_record}.rs`, `native_provider/{service_reach, values}.rs`, `hosted_invocation.rs`, the C fixtures under `native-worker/src/physical_tests/`.)

## Five things a maintainer most needs to know about this unit

1. Import admission is a first-match chain of 14 adapters, each keyed on a normalized foreign locator (target + library + symbol) or a syscall number, then validated against a fixed shape table; anything unmatched ends in a refusal naming the boundary (`host_imports.rs:483-639`). The same C symbol list (libc `abs`, `calloc`, `strtol`, ...) is re-spelled per adapter, and `matches_platform_c_symbol` (:215) is used by some adapters but not by the clock family (`clock/admission.rs:45`, `clock/windows.rs:48`).
2. There are three guest descriptor/handle tables with different numbering rules: libc path (`filesystem/bindings.rs:864`, from 3, refuses 0-2), Linux kernel host table (`host_filesystem.rs:179`, from 0 skipping modeled streams, closing a modeled stream frees its number), private fs (`private_filesystem.rs:654`). Windows has its own (`windows.rs:42`).
3. The worker process is trusted by construction: on Unix the channel is an inherited socket with no peer check, the worker checks the adapter digest against a digest sent in the same packet (`worker.rs:200`), and a failed exchange kills the child and marks the incarnation faulted (`native_provider_runtime.rs:489`). Only worker exit signals 4/5 (or NT 0xc000001d/0x80000003) become a guest `Trap`; every other worker failure is a refusal (`native_provider.rs:958-978`).
4. Two C `abs` adapters exist (`absolute_value.rs`, `integer_absolute.rs`); the second is shadowed by the first in the chain (inferred). Two schema walkers also exist over the same record/array type graph (`record_fields.rs` and `record_shape.rs`).
5. Several limits are literals with no named source at the site: worker start timeout 30 s (`original_homes.rs:56`), process limits `120, 8 GiB, 64 MiB, 128, 16, 1 GiB, 2 GiB` (`sponsor.rs:139-146`), `MAXIMUM_PACKET_BYTES` 16 MiB (`wire.rs:6`), deadline cap 3600 s (`sponsor.rs:109`, `native_provider_runtime.rs:174`, `worker.rs:116`). The private fs constants (`private_filesystem.rs:68-76`) are documented as modeled tmpfs/kernel values.

## What I am unsure about

- Whether `IntegerAbsoluteImport` is truly unreachable: I reasoned from the locator sets and chain order and did not run it.
- Whether `capture_host_epoch` has callers in `tools/` (I grepped `source-rust/` only).
- Whether `application/src/execution/compilation.rs`'s one hit in the `for_adapters|...` grep is `for_adapters` (inferred, line not read).
- The `unavailable_before_guest_effects` semantics beyond `execution.rs:136-146` (callers live in code I did not read).
- I did not read `scalar_record.rs`, `service_reach.rs`, `values.rs`, `native_word.rs`, so claims about `ScalarRecordAdmission`, service-reach checks and `StorageCodec` are limited to how this unit calls them.
- The descriptor_io write path for non-Windows ABIs with descriptors other than 1/2 relies on `BufferedStreams::append` refusing (`buffered_output.rs:40`); I did not run it.
