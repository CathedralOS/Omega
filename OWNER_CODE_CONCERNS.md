# Code Repair Queue

Agents fix obvious code-quality and architecture defects without waiting for
the owner. This queue lists concrete structural problems found outside an
agent's current repair, so another agent can fix, validate and land them. An
entry is neither a new language rule nor a reason to stop the current work.

Use [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md) only for truly unsettled language or
high-level architecture choices. Apply the existing
[architecture rules](AGENTS.md#conventions-and-workflow) directly; wide scope,
conflicting edits and difficulty need no decision. Plain missing functionality belongs in its existing outcome in
[TASKS.md](TASKS.md), not here. Refusing with "not implemented yet" fails
closed; it is not an unsound acceptance.

## How to add one

Add an entry when something would mislead the next agent, or when fixing it
properly reaches beyond the crate you are in. An ordinary bug, a style
preference, or anything you can repair inside your own task does not belong
here: just fix it.

The best time is your first hour in an area, before you have your own change to
defend. Name the broken mechanism and what it costs, not a speculative
framework or a list of unimplemented features.

- Add the entry at the top of the list under "Entries"; the list is flat, newest first.
- Name one category from the list below on its `Category` line.
- Keep it to the lines of the template. Link the file and line.
- Add it in the pull request you are already landing; never open one for it.
- Merge overlapping reports into one entry for the defect that remains. If an
  active `TASKS.md` item already owns the repair and its acceptance checks,
  add the structural evidence there.
- Whoever fixes it removes the entry when the fix lands. Remove a stale report
  after checking that its code or premise is gone; keep no log of finished
  work, and do not mistake deleting a report for implementing its feature.

```markdown
### <one line naming the concern>
- Category: <one of the ten below>
- Where: <path:line, crate, or tool>
- Saw: <what is there, concretely>
- Cost: <who it misleads or what it makes expensive>
- Reported: <date>, <agent>, while <what you were doing>
```

## Categories

**Structure**

1. **One idea, several implementations.** Your fix had to be applied in more
   than one place to work.
2. **Knowledge crossing a boundary.** A stage imports something other than its
   input representation, its output representation or a foundation crate.
3. **Branch per shape.** Logic keyed on where a value sits (local, field,
   parameter, match arm) instead of what it is.
4. **Re-deriving.** A later stage recomputes what an earlier one knew, or a
   stage checks its own output again.
5. **Too big to read.** You had to search the file because you could not read
   it.

**Honesty**

6. **A verdict with nothing behind it.** Something is accepted by default, by
   a fallback, or because a lookup returned nothing.
7. **Tests that pin the bug.** A fixture that would still pass if the feature
   were broken, or a check that quietly stopped running.
8. **Zombie machinery.** Code that only tests call, a retired path that still
   receives input, a fallback nobody can remove.

**Direction**

9. **The documents disagree.** You can cite a rule for either answer, or a
   document approves code that is decaying.
10. **Verifying costs more than writing.** You waited longer than you worked.

## Entries

### The Psi interpreter compares and offsets host-returned addresses that the spec says it must refuse
- Category: A verdict with nothing behind it
- Where: [integer equality](omega-rust/psi/interpreter/src/engine_execution.rs#L3598), [integer order](omega-rust/psi/interpreter/src/engine_execution.rs#L3612), [compare_with](omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs#L627), [address subtraction](omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs#L385), [address cast](omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs#L419), [host address result](omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/native_provider/original_homes.rs#L205), [host address ledger](omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/allocation.rs#L115)
- Saw: [host result validation](wiki/spec/terminal-psi/boundary_calls.md#L230) says the interpreter "never dereferences or offsets" a host-returned `addr` and "refuses any comparison or arithmetic that combines it with an interpreter-created address". Host provenance lives only in the terminal-runtime ledger, read only when an address goes back to a native import. `host_address_result` stores the value as a plain integer, and `IntegerEqual`, `IntegerLessThan`, `ExactIntegerAdd`, `ExactIntegerSubtract` and `IntegerExactCast` compute results on it, refusing only on a carrier mismatch. With `p` from a native import and `q = address_of(&local)`, `p < q` runs in Psi, ordering a stand-in identity against a real pointer, and `p == address_of(&x)` can be true in Psi alone.
- Cost: `omega run --both` can report agreement or disagreement on a comparison with no defined meaning across the two runs. Carry host provenance on the scalar and refuse mixed comparison, arithmetic and offsets in the core operations.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/terminal-psi/boundary_calls.md

### A shared byte view of an array gives its elements different addresses in the Psi interpreter
- Category: Re-deriving
- Where: [shared view is a snapshot](omega-rust/psi/interpreter/src/boundary_inputs.rs#L1403), [shared reborrow snapshot](omega-rust/psi/interpreter/src/storage_bytes_and_views.rs#L1017), [fresh namespace](omega-rust/psi/interpreter/src/storage_bytes_and_views.rs#L3003), [view element address](omega-rust/psi/interpreter/src/storage_bytes_and_views.rs#L3077), [array element address](omega-rust/psi/interpreter/src/storage_primitives.rs#L6016)
- Saw: [address exposure](wiki/spec/language/counts_and_addresses.md#L35) says compatible views of the same location "expose the same address", and [interpreted address identities](wiki/spec/language/counts_and_addresses.md#L107) says changing a view's type "cannot change the referent's address". A shared view `&bytes[..]` of a `[u8; N]` array, and a shared reborrow of a mutable view, copy the bytes into `ByteSequenceView::new`, which draws a new exposure namespace; `address_of(&view[i])` encodes that namespace while `address_of(&bytes[i])` encodes the array's identity. `var bytes: [u8; 4]; let view = &bytes[..]; exit_process(if address_of(&bytes[0]) == address_of(&view[0]) { 0 } else { 1 });` exits 0 natively and 1 under Psi. Traced in the interpreter; the lowering of `address_of` on a view place was not confirmed.
- Cost: Psi gives the wrong answer on a well-formed program. Compute view-element addresses from the `OriginalByteArrayWindow` the view already carries. TASKS.md ADDRESS-EXPOSURE owns completing `address_of` and may absorb this; this is a wrong answer from the part already in use, not a refusal.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/counts_and_addresses.md

### `Clock::sleep` promises it never blocks a worker, but every provider blocks for the whole wait
- Category: A verdict with nothing behind it
- Where: [Clock::sleep](source/library/std/clock.omg#L10), [LinuxClockWait::poll](source/library/std/targets/linux_x86_64/clock_impl.omg#L15), [LinuxX64TimeRead::poll](source/library/std/targets/linux_x86_64/time_host_impl.omg#L39), [Windows Clock binding](source/library/std/build.omg#L104), [WindowsTimeRead::sleep](source/library/std/targets/windows_x86_64/time_host_impl.omg#L51)
- Saw: [implementation boundaries](wiki/spec/language/effects.md) says that on published surfaces "omitted `blocks` means never blocks a worker". `Clock::sleep` omits `blocks` although its comment says "Block the caller for `milliseconds`", and so do its raw waits `LinuxClockWait::poll`, `LinuxArmClockWait::nanosleep` and `DarwinClockWait::poll`. The same native calls are bound elsewhere with `blocks`: syscall 7 is also `LinuxX64TimeRead::poll` (entered as `block ...`), and kernel32 `Sleep` is also `WindowsTimeRead::sleep`.
- Cost: The task runtime and synchronization checks read `blocks` to decide whether a call can hold a worker; `Clock::sleep(60000)` holds one for a minute with no evidence. Declare `blocks` on `Clock::sleep` and its three raw waits, as `TimeHost::sleep` does, and mark the call sites with `block`.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/effects.md

### On Windows, sleeping for `u32::MAX` milliseconds never returns
- Category: One idea, several implementations
- Where: [Windows Clock binding](source/library/std/build.omg#L104), [Windows TimeHost sleep](source/library/std/targets/windows_x86_64/time_host_impl.omg#L119), [POSIX split of large waits](source/library/std/targets/linux_x86_64/clock_impl.omg#L30), [sleep_for saturates](source/library/std/time.omg#L1065)
- Saw: [three layers](wiki/spec/build/platform_services.md) requires a portable service to mean the same thing on every target. The Linux and Darwin providers split a count above `i32::MAX` into finite waits, so `sleep(4294967295)` returns after about 49.7 days. Windows binds `Clock::sleep` and `WindowsTimeRead::sleep` straight to `Sleep(DWORD)`, where 0xFFFFFFFF is `INFINITE`. `sleep_for` saturates any duration of 49.7 days or more to exactly that value, and its comment says the saturated request "is RETURNED so the caller can observe it".
- Cost: A long sleep hangs forever on Windows only. Give Windows a checked adapter that never passes 0xFFFFFFFF, for example by splitting the wait as the POSIX providers do.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/platform_services.md

### POSIX and Windows filesystem adapters give five portable calls different meanings
- Category: One idea, several implementations
- Where: [Windows open_with](source/library/std/targets/windows_x86_64/filesystem/handles.omg#L30), [POSIX open_with](source/library/std/posix_filesystem_adapter.omg#L1191), [POSIX Default permissions](source/library/std/posix_filesystem_adapter.omg#L1009), [Windows Default permissions](source/library/std/targets/windows_x86_64/filesystem/metadata.omg#L161), [POSIX removal budget](source/library/std/posix_filesystem_adapter.omg#L1477), [POSIX removal stack](source/library/std/posix_filesystem_adapter.omg#L422), [POSIX create_dir_all prefix](source/library/std/posix_filesystem_adapter.omg#L919), [Windows read](source/library/std/targets/windows_x86_64/filesystem/handles.omg#L110), [Linux read clamp](source/library/std/targets/linux_x86_64/posix_filesystem_impl.omg#L148)
- Saw: [three layers](wiki/spec/build/platform_services.md) says the portable service "is identical on every target" and all translation to the OS shape happens in the adapter. Each family decides these cases itself:
  - `open_with` with `truncate` and no write access (or with `append`): Windows refuses with `InvalidInput`; POSIX encodes `O_RDONLY|O_TRUNC` and empties the file.
  - `set_permissions(p, Permissions::Default)`: Windows only clears read-only; POSIX calls `chmod(p, 0o666)`, which strips execute, set-ID and sticky bits (a 0755 script stops running, a directory loses its search bit), unlike its own `Readonly { value: false }` path.
  - `remove_dir_all`: POSIX stops after 4096 steps or 16 levels, after deleting part of the tree, with `Other`; Windows has no budget.
  - `create_dir_all`: POSIX skips any parent prefix ending at byte 255 or later and ignores every intermediate `mkdir` failure; Windows creates every prefix and stops at the first real error.
  - `read`, `read_all` and `copy` with a count larger than the buffer: Windows refuses with `InvalidInput`; POSIX clamps to the buffer.
- Cost: The same call is harmless on one target and destroys data or half-finishes on another. Decide each rule once in the portable layer and make both families follow it.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/platform_services.md

### POSIX std reads `errno` by `memcpy` from a raw address and declares it cannot crash
- Category: A verdict with nothing behind it
- Where: [copy_error](source/library/std/posix_filesystem_adapter.omg#L1813), [posix_canonical_error](source/library/std/posix_filesystem_adapter.omg#L1875), [c_copy_from_address](source/library/std/targets/macos_arm64/posix_filesystem_impl.omg#L156), [darwin_failure_i32](source/library/std/targets/macos_arm64/posix_filesystem_impl.omg#L161), [memcpy bindings](source/library/std/build.omg#L359)
- Saw: [inert addresses and marshaling](wiki/spec/build/foreign_storage.md) says `addr` is numerical data that ordinary code cannot dereference, and a binding materializes a pointer "only from established storage claims"; thread-local storage must be copied or mediated by such a claim. std declares `copy_error(destination, source: u64, count)` and `c_copy_from_address` as `reaches none crashes none`, binds both to `memcpy`, and passes the bare `u64` from `__errno_location`/`___error` as the source. That is a general read of any address, and `source = 0` faults. The two copies also decode differently: one assembles four bytes, the other keeps only `code[0]`.
- Cost: Reviewers see a crash-free boundary that is really an unchecked dereference. Provide one shared errno requirement whose provider returns the `int`, or an admitted view of the thread-local slot, for both callers.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/foreign_storage.md

### The Omega-written compiler counts effectful guards as complementary coverage
- Category: One idea, several implementations
- Where: [opposed_guards](source/psi/pipeline/03_symbol-resolved-trees-to-typed-trees/coverage.omg#L243), [same_tree](source/psi/pipeline/03_symbol-resolved-trees-to-typed-trees/coverage.omg#L386), [unmatched-arm trap](source/psi/pipeline/05_checked-trees-to-scalar-operations/statements.omg#L327), [Rust refusal test](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs#L377)
- Saw: [coverage and domain tests](wiki/spec/language/patterns.md#L197) says missing coverage "is a compile error, not an implicit trap or fallthrough", and complementary `==`/`!=` guards cover only "over the same unchanged subjects"; [expressions.md](wiki/spec/language/expressions.md#L32) says an effectful guard "cannot be replayed as a fact about operands it may have changed". `opposed_guards` pairs any `l == r`/`l != r` or `x == true`/`x == false` arms, and `same_tree` compares only spellings, never the arms between them. With `machine flip(v: &mut bool) -> bool { v = true; false }`, `transition { cell == true -> 1  flip(&mut cell) == true -> 2  cell == false -> 3 }` counts as covered; from `cell = false` no arm matches, and stage 05 lowers that path to `Trap`. The Rust compiler refuses this shape ("the call can change the pointee between alternatives").
- Cost: The two compilers disagree, and the Omega-written one turns a coverage error into an undeclared runtime trap. Refuse a guard pair when any guard between them can write storage the pair reads, as Rust's `guard_may_change_storage` does.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/patterns.md

### The Omega-written compiler lets a case pattern with a `field: value` test cover the whole case
- Category: A verdict with nothing behind it
- Where: [case_pattern](source/psi/pipeline/03_symbol-resolved-trees-to-typed-trees/coverage.omg#L206), [valued field typing](source/psi/pipeline/03_symbol-resolved-trees-to-typed-trees/statements.omg#L484)
- Saw: [patterns.md](wiki/spec/language/patterns.md#L30) says "`field: value` is ordinary projected equality", so `Reading::Value { level: 0 }` matches only some `Value` readings. `case_pattern` checks only that the arm names the case and ignores the pattern's fields, so `transition r { Reading::Value { level: 0 } -> 0  Reading::Missing -> 1 }` passes coverage and `Value { level: 5 }` matches nothing.
- Cost: The coverage verdict has nothing behind it. Only an arm whose fields are all binders or `as _` should close its case.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/patterns.md

### The Omega-written resolver accepts type names spelled like builtin integers
- Category: One idea, several implementations
- Where: [builtin_shadow](source/psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/resolver.omg#L288), [builtin_type](source/psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/resolver.omg#L1470), [type_binding](source/psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/resolver.omg#L1224), [Rust check](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_builtin_type_names.rs#L23)
- Saw: [builtin type names](wiki/spec/language/modules.md#L146) makes the integer family "every `i` or `u` followed by decimal digits" and says a data declaration or generic type binder with such a spelling "rejects where it is introduced" (`data u7 { value: u32; } // Rejected`). The Omega-written resolver checks only root type declarations against a fixed list (`u8`…`u64`, `i8`…`i64`, …), so `data u7` and `data u128` are accepted, and it never checks generic binders, so in `machine pick<u32>(x: u32) -> u32` the binder silently hides the builtin. The Rust compiler refuses both with the open-ended `is_builtin_type_spelling`.
- Cost: The same source means different types in the two compilers. Use the open-ended family test for every type introducer, generic binders included.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/modules.md

### Omega-written typing re-scans integer literals from source bytes
- Category: Re-deriving
- Where: [literal_value](source/psi/pipeline/03_symbol-resolved-trees-to-typed-trees/literals.omg#L15), [suffix_carrier](source/psi/pipeline/03_symbol-resolved-trees-to-typed-trees/literals.omg#L180), [lexer](source/psi/pipeline/00_source_files_to_tokens.omg#L944), [token kind](source/psi/representations/tokens.omg#L109)
- Saw: AGENTS.md says stages consume their declared input representation and must not bypass the pipeline with "source re-analysis". The lexer already records each integer literal's base, digit run and suffix presence in `TokenKind::Integer`; stage 03's `literal_value` reopens the source bytes at the span, detects the base again, re-scans digits and separators, and matches the suffix against eight spellings.
- Cost: Two scanners must agree on literal grammar, so a change to one (such as the spec's `u13` suffixes) can split them silently. Decode value and carrier once in the lexer or parser and carry them forward.
- Reported: 2026-11-16, Claude Code, while auditing code against omega-rust/pipeline.md

### The compiler has no `InternalFailure` outcome, so a defect it detects in itself exits as "source rejected"
- Category: A verdict with nothing behind it
- Where: [outcome enum](omega-rust/omega/compilation/products/src/compile_outcomes.rs#L109), [non-diagnostic errors become Reject](omega-rust/omega/build-system/orchestration/src/lib.rs#L233), [validator failure called a compiler defect](omega-rust/omega/build-system/orchestration/src/lib.rs#L215), [exit status](omega-rust/omega/application/src/commands/compile.rs#L244), [reserved-codes comment](omega-rust/omega/application/src/arguments/mod.rs#L33)
- Saw: [outcomes](wiki/spec/build/compiler_request.md#L249) says compilation "preserves four outcome distinctions", including `InternalFailure` for "a compiler/checker malfunction or violated implementation invariant", with halt value 3; [independent stage validators](wiki/spec/build/command_line.md) says to "distinguish compiler invariant failures from invalid authored source". `CompilerRequestOutcome` has only `Complete`, `Reject` and `Incomplete`, and `BuildError::outcome` maps every non-diagnostic error (`StageValidation`, `Publication`, `BuildDirectory`, `CheckWorkspace`) to `Reject`. A failing `omega --validator TargetOperationCorrespondence main.omg` prints "this is a compiler defect, not an error in the source" and exits 1. The Omega-written compiler has the case (`source/omega/application/ocreq/outcome.omg`).
- Cost: Scripts reading `$?` cannot tell a compiler bug from a rejected program. Add `InternalFailure` (exit 3), classify compiler-invariant errors as it, and exit with it.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/compiler_request.md

### A failed stage validator never reaches the `--report-file` report
- Category: A verdict with nothing behind it
- Where: [first failure drops the rows](omega-rust/omega/build-system/orchestration/src/project/stage_validators.rs#L201), [exit before the report](omega-rust/omega/application/src/commands/compile.rs#L158), [report write](omega-rust/omega/application/src/commands/compile.rs#L161)
- Saw: [independent stage validators](wiki/spec/build/command_line.md#L152) says to report validator names and "passed/failed results in the existing compile report; `--report-file` retains those rows". `StageValidatorRequest::run_with` returns `Err(BuildError::StageValidation(row))` at the first failure and drops the rows already gathered, and `present_single` exits on refusal before `write_report_or_exit`. `omega --every-validator --report-file r.txt main.omg` with one failing validator writes no `r.txt`; with several targets the batch report keeps a `failed:` line but loses that target's passed rows. No report can contain a failed row.
- Cost: The report is missing exactly when validation fails. Carry the complete row list out with the error and write the report before choosing the exit status.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/command_line.md

### `omega packages audit --help` documents exit 2 for invalid arguments; the code exits 64
- Category: The documents disagree
- Where: [audit usage text](omega-rust/omega/application/src/arguments/audit.rs#L13), [parse errors exit 64](omega-rust/omega/application/src/main.rs#L62), [bad adapter exits 64](omega-rust/omega/application/src/commands/packages.rs#L125)
- Saw: The usage text promises "2: invalid arguments". Every parse error exits with `USAGE_ERROR_EXIT` (64), as does a bad adapter, and the comment in `arguments/mod.rs` explains that 2 is the compiler request contract's `Incomplete` status. `omega packages audit --bogus; echo $?` prints 64.
- Cost: A script written from `--help` never sees its argument errors. Change the help text to 64, or state the audit statuses in command_line.md.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/command_line.md

### `x == S::C` on a payload-bearing sum becomes a bare tag test when Psi 03 cannot type `x`
- Category: One idea, several implementations
- Where: [untyped comparison left in place](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_preparation.rs#L217), [no type for a receiver call](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/typing_queries.rs#L1425), [read as a case test](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/expressions.rs#L500), [case test without payload check](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/places.rs#L1412)
- Saw: conformances.md says "Records and payload-bearing sums require an explicit named conformance", and data_and_literals.md forbids "silently changing tag equality into field equality". Psi 03 enforces that only when `DeclaredExpressionTypes` types both operands; otherwise it `continue`s, and the query returns `None` for receiver calls, calls with machine arguments and `match` expressions. Psi 05 then reads any `==` whose right side names a case as `IsCase`, without checking for payload cases or a selected `Equatable`. With `data Cmd { Stop; Move { dx: i32; } }`, `Codecs::decode(&bytes) == Cmd::Stop` compiles as a tag test while `let c: Cmd = Codecs::decode(&bytes); c == Cmd::Stop` is refused (or calls the custom `Equatable<Cmd>`).
- Cost: The compiler accepts an equality the spec refuses, and `==` means different things depending on how the operand is spelled. Let Psi 05 read `==` as a case test only for payload-free sums, or have Psi 03 refuse a comparison it cannot type.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/conformances.md

### The locator kind chooses a bound requirement's calling policy
- Category: Re-deriving
- Where: [native_calling_applications](omega-rust/omega/compilation/realization/provider-planning/src/calling_policy_plans/selected_application.rs#L270), [declared policy refused](omega-rust/omega/compilation/realization/provider-planning/src/calling_policy_plans/selected_application.rs#L257), [calling_plans.md](wiki/spec/build/calling_plans.md#L180)
- Saw: [identity and selection](wiki/spec/build/calling_plans.md#L180) says a `Build::bind` row retains the requirement's calling-policy application and "A locator, syscall number, or target label cannot select calling policy". The code refuses any requirement with a declared policy, and gives every other requirement `CallingPolicy::native_for_target` for an import locator or `native_syscall_for_target` for a syscall locator. On linux_x86_64 the same four-argument requirement passes its fourth argument in rcx when bound to an import and in r10 when bound to a syscall.
- Cost: The ABI stops belonging to the requirement, and a requirement that declares its policy, as the spec intends, cannot be bound. Let the requirement declare the syscall policy and have the binding check the locator against it, or change the spec.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/calling_plans.md

### The test runner cannot run, because Build admission refuses every test group
- Category: Zombie machinery
- Where: [registration refused](omega-rust/psi/compilation/build-time-evaluation/src/product/product_declarations.rs#L48), [groups always empty](omega-rust/psi/compilation/build-time-evaluation/src/product/product_declarations.rs#L65), [test discovery](omega-rust/omega/compilation/compiler/src/checked/test_roots.rs#L74), [native verdict](omega-rust/omega/build-system/orchestration/src/compile/test_execution.rs#L260), [std slot found by package name](omega-rust/omega/compilation/compiler/src/checked/test_roots.rs#L40)
- Saw: `product_declarations::admit` refuses any Build that uses `TestGroupRegistration` or `TestGroupService`, and otherwise returns `test_groups: Vec::new()`. So `select_test_roots`, `test_execution::run_tests` and the private-filesystem runner that [TEST-GROUP-FILESYSTEM-TARGETS](TASKS.md#L308) describes as working cannot be reached; no fixture registers a test group. The unreachable code also departs from [testing.md](wiki/spec/build/testing.md): `execute_native` passes any test whose process exits 0, although the outcome table says a process exit instead of returning to the runner is "Not a pass"; and `group_filesystem` recognizes the std `FilesystemService` slot by the package name `omega_language_std`, although [declarations.md](wiki/spec/build/declarations.md#L151) says a package named `std` cannot confer compiler custody.
- Cost: Readers of the runner and TASKS.md assume test groups run; admitting registration later makes both latent bugs live. Deliver group admission with both bugs fixed, or say on the TASKS item that registration is refused and own both bugs there.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/testing.md

### The observation-profile code has no users, but the wire README says acceptance checks profiles
- Category: Zombie machinery
- Where: [profile types](omega-rust/psi/compilation/representations/terminal-psi/src/observation.rs#L159), [trace builder](omega-rust/psi/compilation/representations/terminal-psi/src/observation.rs#L180), [README claim](omega-rust/psi/compilation/representations/terminal-psi/wire/README.md#L86), [section list](omega-rust/psi/compilation/representations/terminal-psi/wire/src/sections.rs#L3)
- Saw: [reconstructed rows](wiki/spec/terminal-psi/observations.md#L100) says checking derives the observation profile from the validated module and compares it with the decoded one, and the wire README says acceptance does so and the encoder writes the terminal-external group. No `.rs` file outside `observation.rs` uses `TerminalTraceV1Profile` or `begin_trace`, and the string `observation-profile` appears in no Rust file; the roughly 690 lines before the module's tests are called only by those tests.
- Cost: Agents believe trap and boundary-crash sites are reconstructed and checked against a profile when nothing does it. Delete the unused types and correct the README and `sections.rs`, or add the deriver and codec under a `TASKS.md` item.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/terminal-psi/observations.md

### Default compilation re-checks the image it just wrote, which one spec page requires and two forbid
- Category: The documents disagree
- Where: [default image emission](omega-rust/omega/compilation/backend/images/image-emission/src/image.rs#L1851), [validate_terminal_image](omega-rust/omega/compilation/backend/images/image-emission/src/image.rs#L803), [machine-state evidence](wiki/spec/build/machine_state_evidence.md#L24), [verification placement](omega-rust/pipeline.md#L65), [optimizations](wiki/spec/build/optimizations.md#L59)
- Saw: machine_state_evidence.md says "Final-artifact validation is mandatory". pipeline.md says a compile "does not revalidate, replay or rebuild output the same compile just produced", and optimizations.md repeats it. Every ordinary native build calls `validate_terminal_image`, which re-checks import and relocation counts, region inventories and literal-pool bytes, and decodes x86 and AArch64 address fields from the final bytes to compare them with the object plan, without any `--validator` flag.
- Cost: Agents can cite one rule to delete this work and another to extend it, and every native build pays for it. Decide whether final-artifact validation is an exception to verification placement; then move the checks behind `--validator` or write the exception into pipeline.md and optimizations.md.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/machine_state_evidence.md

### `Build.freestanding` is one untargeted mutable flag that two spec pages forbid and a third relies on
- Category: The documents disagree
- Where: [Build declaration](omega-rust/omega/packages/build-declarations/src/prelude.rs#L269), [settings field](omega-rust/omega/packages/package-compilation/src/build_execution/build_value/configuration.rs#L22), [assembly authority](omega-rust/omega/compilation/compiler/src/checked/checking/source_checking.rs#L243), [configuration.md](wiki/spec/build/configuration.md#L53), [uefi_entry.md](wiki/spec/build/uefi_entry.md#L3), [permissions.md](wiki/spec/build/permissions.md#L233)
- Saw: configuration.md says the hosted/freestanding environment belongs to the target profile, "not duplicated mutable Build flags", and that grants are rows keyed by target; uefi_entry.md calls freestanding "an immutable selected target/provider property". permissions.md instead names "the evaluated `Build.freestanding` selection" as machine-owner admission, and the code follows it: source checking turns the single untargeted bool into `AsmAuthorityAdmission::MACHINE_OWNER` for the whole program. With `--target uefi_x86_64 --target linux_x86_64`, machine-control assembly is admitted for the hosted Linux realization too. The field comment ("no ambient host packages or import thunks") describes behavior the flag does not have.
- Cost: Agents following different pages delete or extend the field. Settle the page, then make the grant a per-target row or rewrite configuration.md and uefi_entry.md, and correct the comment.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/configuration.md

### Register allocation picks its route from optimization history
- Category: One idea, several implementations
- Where: [allocation entrance](omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/register_allocation.rs#L29), [history routes](omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/assignment.rs#L691), [history output](omega-rust/omega/compilation/representations/selected-instructions/analyses/src/lib.rs#L495)
- Saw: `stage_register_allocation` matches `SelectedOptimizationOutput::{Identity, SelectedLowering, PreAllocation}`. Each arm runs its own assign, runtime-spill and retain sequence with its own error variant (`Homes`, `TransformedHomes`, `PreAllocationHomes`). `CompletedSelectedOptimization` also hands Omega 05 the pre-rewrite `StagedOptimizedAllocationLegality` as `source`. [pipeline.md](omega-rust/pipeline.md#crate-placement-rule) says optimization history "must not select a different downstream representation"; [optimizations.md](wiki/spec/build/optimizations.md#phase-and-product-boundaries) says history alone is not a representation boundary.
- Cost: Every new Omega 04 phase needs another allocation route, spill route, error type and route test. Give Omega 05 one current selected program with its analyses and keep the run as separate evidence.
- Reported: 2026-11-16, Claude Code, while checking a backend review against the spec

### Omega 04 replays its own rewrites on every optimized compile
- Category: Re-deriving
- Where: [output construction](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/selected_optimization.rs#L35), [fold replay](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/literal_fold_rewrites.rs#L415), [pre-allocation replay](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites.rs#L1934)
- Saw: `SelectedInstructionOptimizationOutput::from_evidence` calls `validate_selected_lowering_optimization_custody` or `validate_pre_allocation_optimization_custody` before returning. The first rebuilds every fold step with the same `build_step` the run used; the second calls the same `run_passes` again and compares. This runs in production whenever an Omega 04 rule is selected. [optimizations.md](wiki/spec/build/optimizations.md#phase-and-product-boundaries) keeps re-validation off the default compile path, and [independent validation](wiki/spec/build/optimizations.md#atomic-candidates-and-independent-validation) forbids a producer acting as its own validator.
- Cost: Optimized compiles pay for each rewrite twice, and the check proves only that the code is deterministic. Move it behind [validator selection](wiki/spec/build/command_line.md#independent-stage-validators) and the tests, as `retain_replay_for_validation` already does for function realization.
- Reported: 2026-11-16, Claude Code, while checking a backend review against the spec

### Allocation recovery dispatches on optimization names, not catalog entries
- Category: One idea, several implementations
- Where: [recovery selector](omega-rust/omega/compilation/representations/selected-instructions/analyses/src/lib.rs#L425), [name dispatch](omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/register_allocation.rs#L44)
- Saw: `selected_allocation_recovery_rule` finds the catalog entry, then returns only its `Optimization` name. `stage_register_allocation` maps that name to a route in its own `match`, with a `_ => UnsupportedComposition` arm. [Exact selection](wiki/spec/build/optimizations.md#exact-selection): "Custody coordinators consume the selected entry, not a duplicate name-to-rule table."
- Cost: Adding a recovery rule means editing both the catalog and this match. A catalog row with no arm passes selection and refuses only at allocation.
- Reported: 2026-11-16, Claude Code, while checking a backend review against the spec

### Allocation route tests pin source spelling instead of allocation
- Category: Tests that pin the bug
- Where: [`route_tests`](omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/register_allocation.rs#L101)
- Saw: Five tests `include_str!` `register_allocation.rs` or `assignment.rs` and assert that strings such as `SelectedOptimizationOutput::SelectedLowering(run)` appear in a given order. They pass whether or not the homes are correct.
- Cost: They defend the per-history routes above and fail when those routes merge. Replace them with tests that allocate programs after each Omega 04 phase, with and without register pressure.
- Reported: 2026-11-16, Claude Code, while checking a backend review against the spec

### Legalization replaces its failure causes with a bare custody error
- Category: Verifying costs more than writing
- Where: [scalar graph](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/native-preparation/src/legalization_scalar_graph.rs#L1084) and four sibling `legalization_*.rs` files in `native-preparation`
- Saw: 39 calls to `map_err(|_| LegalizationError::custody())` drop the underlying error. The result records the Rust call site and nothing about which argument, storage or result failed.
- Cost: Each new legalization refusal needs a diagnostic build to find its cause. Keep the cause in the error; this changes no accepted program.
- Reported: 2026-11-16, Claude Code, while checking a backend review against the spec

### Callable analysis hides its storage-flow refusal
- Category: Verifying costs more than writing
- Where: [callable projection](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/callable_values/terminal.rs#L124)
- Saw: `terminal_callable_values` replaces the reference graph's error with `callable provenance cannot reconstruct current Terminal storage flow`. The unchanged [indexed-case program](tests/fixtures/run/calls/indexed_case_discriminator/main.omg) produces that refusal before native compilation on both current-form source `70dfe3f` and clean landed base `d3deb345`; the underlying graph error is lost.
- Cost: A caller cannot name the failed operation or storage relationship without another diagnostic build. Preserve the graph's reason through the callable query and its callers; keep the program and its oracle unchanged.
- Reported: 2026-11-16, Codex, while qualifying current-form frame publication

### Changing storage analysis rebuilds the native compiler
- Category: Verifying costs more than writing
- Where: [semantic analysis package](omega-rust/psi/compilation/representations/terminal-psi/semantics/Cargo.toml), [declaration lookup in optimization](omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/structural_places.rs#L619), [target descriptor inputs](omega-rust/omega/compilation/representations/target-operations/target-vocabulary/src/descriptor_input.rs)
- Saw: Changing only the private descriptor replay cache and its error reporting rebuilt 64 packages and took 14m 17s before the original Console test could run. Some native consumers need only declaration or structural-path lookups from the same package; ownership analysis and explicit image validators genuinely need the full analysis. `target-operations` also declares an unused direct dependency, although removing it alone leaves a transitive dependency. Source and build evidence remains in claim `18e85a23de177c780000000018e83683`, under `post-3776-original-binding-control-49/native-terminal-semantics-coupling.json`.
- Cost: Edits to the analysis invalidate native consumers of small lookup functions. Move the declaration and path queries out of the full analysis crate, keep each judgment implemented once, and remove unused direct dependencies. Measure the same Console edit/build/run loop before claiming a build-time improvement.
- Reported: 2026-11-16, Codex, while carrying the original Console program through checking and execution

### Source correspondence repeats signature searches
- Category: Verifying costs more than writing
- Where: [source correspondence](omega-rust/psi/compilation/representations/typed-trees/src/service_declarations.rs#L497)
- Saw: For each projected method, correspondence validation scans every inherited signature and derives its callable identity before interning it. Finding one match still exhausts the scan. The original Console binding replay43 took 329.112s before its next refusal. A brief sample caught this work, but captured only two samples; its share of total time is unknown. Raw evidence remains in claim `18e85a23de177c780000000018e83683`.
- Cost: Repeated identity normalization grows with both the method and signature counts. Prepare the exact callable/package identities once per inherited declaration set. Preserve source authentication, complete applications, and refusals for missing or ambiguous requirements. Validate with the unchanged Console program and refusal controls; measure the identity derivations before claiming a speedup.
- Reported: 2026-11-16, Codex, while carrying the original Console binding and stored-machine programs through checking and execution

### Every native caller evaluates constants between two driver calls
- Category: one idea, several implementations
- Where: [native completion](omega-rust/omega/build-system/orchestration/src/compile/native_completion.rs#L123), [scalar completion](omega-rust/omega/compilation/native-compiler/src/scalar_completion.rs), [Build's completer](omega-rust/omega/build-system/build-settings/src/activation/scalar_completion.rs#L20)
- Saw: Omega 00 now runs whole inside `native_compiler::compile`, but callers still call `native_compiler::prepare`, run `build_settings::scalar_completion::complete` on every `NativeScalarCompletion`, then call `compile`. Preparation reads only the retained Terminal product and its native proposal (`TerminalNativeRealizationProposal::prepare_native_execution`, `PreparedNativeExecution::scalar_application_demand`). Product checking separately completes the same artifact's conditions with an empty demand (`ProducedTerminalProduct::check_conditions`).
- Cost: Orchestration, the review example and compiler tests each repeat the prepare, evaluate, compile sequence, and conditions are evaluated twice. Select the execution and complete its demand once where the retained native product is formed, pass the completed scalars in `NativeCompileRequest`, and delete `NativeScalarCompletion` and the split entry.
- Reported: 2026-11-16, pi, while checking that only the native driver calls Omega stages.

### Emission libraries keep earlier stage outputs beside the final layout
- Category: knowledge crossing a boundary
- Where: [fragment source](omega-rust/omega/compilation/backend/machine-emission/src/fragment_emission.rs#L211), [driver join](omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L227), [image call binding](omega-rust/omega/compilation/backend/images/image-emission/src/callable_calls.rs#L8), [object accessors](omega-rust/omega/compilation/backend/object-file/src/fragment_container.rs#L204), [runtime-only route](omega-rust/omega/compilation/native-compiler/src/realization.rs#L172), [stage-call exceptions](tools/xtask/src/boundary.rs)
- Saw: `StagedOptimizedFunctionFragmentEmissionSource::new` joins the stage 09 realization with stage 02's whole output, and frame staging between stages 06 and 07 runs in machine-emission on types defined inside stage crates. The source keeps that `Arc<ValidatedOptimizedTargetOperations>` on every compile; `pre_physical_manifest()` reads stage 01's output through `optimized_target().optimized()`. Image emission's `callable_calls::prepare_from` reads stage 02's execution plan through it, and `object-file` reaches it through `source().source().source()`. Function-realization replay is now opt-in (`retain_replay_for_validation`); this field is not. [pipeline.md](omega-rust/pipeline.md#portable-materialization-and-consumer-ownership) keeps producer history "off the path ordinary consumers walk to get current data". `admit_terminal_runtime` runs stage 01, then calls stage 02's `capture_runtime_borrowed_storage_layouts` on the side. `native-artifact` reads Omega 00's `VerifiedNativeArtifactInput` for runtime provisions.
- Cost: Emission can read anything an earlier stage knew, so a missing fact never has to be carried forward. Move the stage output types emission names into `compilation/representations`, carry the rows emission needs in each stage's output, and delete each `STAGE_CALL_ALLOWANCES` row as its edge goes.
- Reported: 2026-11-16, pi, while checking that only the native driver calls Omega stages.

### Borrowed field recasts lose the target carrier layout of the enclosing record
- Category: re-deriving
- Where: [reference view geometry](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references/representation_geometry.rs#L147), [sum geometry](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references.rs#L4450)
- Saw: A recast checks the enclosing record's alignment, but its geometry query has no carrier layout for a sibling sum. The checker supplies the retained target's record resolver without its case-tag layout. An exact `PosixFilesystem` reduction refuses a whole aligned 144-byte field. Changing only the sibling `ErrorKind` field to `u32` yields native and Psi exit 0 on macOS arm64.
- Cost: The standard-library POSIX stat decoders still refuse the whole buffer. Pass the existing target carrier layout to projected storage checks, preserve the short and misaligned refusals, and replay the unchanged [note vault](https://github.com/CathedralOS/Omega/pull/3722).
- Reported: 2026-11-15, Codex, while validating shared recasts of an aligned record field.

### Specialization failures discard the reason they stopped
- Category: Verifying costs more than writing
- Where: [specialization](omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/passes/specialization.rs)
- Saw: many specialization checks still return `Stop::Unserved(None)`. The Console binding replay reached an anonymous nested-parameter mismatch; adding the missing reason required another optimized compiler build and replay. The current repair gives that mismatch a reason, but adjacent shape, lookup and argument checks still discard theirs.
- Cost: the next failure names only the unavailable application, hiding the failed check. Carry each check's reason through the existing refusal path so one replay identifies the repair, without changing which programs are accepted.
- Reported: 2026-11-15, Codex, while restoring Console preparation for stored-machine execution

### Focused compiler controls spend twenty-two minutes in code generation
- Category: Verifying costs more than writing
- Where: [compiler test targets](omega-rust/omega/compilation/compiler/Cargo.toml#L110), [compiler feature alignment](omega-rust/omega/compilation/compiler/Cargo.toml#L102), and the `coherent-native-controls-24-01` receipt in continuation claim `18e81ab82af520780000000018e96239`
- Saw: On macOS arm64 at `a827cf2c269`, a fixed-source nextest selection of 24 controls across 12 binaries spent 22m17s building before 176.905s of test execution. The command used the retained CLI feature union, optimization level 1, two codegen units and three jobs; 1,219 tests were not selected. Actual compiler processes remained active. After integrating main `a2684962ce882`, 13 selected unit controls across six crates spent 15m49s building and 0.414s running on the same host and budget (`merge-a268-direct-controls-01`, with exact source patch and input records retained in the active claim). The source of this build cost has not been established.
- Cost: The recorded twelve-binary replay spent twenty-two minutes building before exposing the next semantic refusal. A later three-binary replay at c16df641c12 built in 2m51s; that smaller selection does not identify the earlier cost. Measure the actual invalidation and compiled-target work, then repair the responsible build or test ownership while retaining the selected controls and exact-source optimized producer checks.
- Reported: 2026-11-14, Codex, while carrying stored-machine programs through Console and native-binding preparation

### Callback and interrupt records lose the machine that owns their operation
- Category: Re-deriving
- Where: [terminal-psi/src/lib.rs:352](omega-rust/psi/compilation/representations/terminal-psi/src/lib.rs#L352), [executable preparation](omega-rust/omega/compilation/compiler/src/executable.rs), and [codec/src/foundation_validation.rs:2261](omega-rust/psi/compilation/representations/terminal-psi/codec/src/foundation_validation.rs#L2261)
- Saw: each lowered machine allocates operation IDs from its own fresh `MachineSources`, but callback and interrupt records retain only `OperationId`. Executable preparation searches every reached machine for that number, while codec interrupt validation collects all machines into a map keyed by the same bare ID. Stage05 currently emits neither roster; the existing constructors are fixtures and decoding, and no failing customer execution was observed.
- Cost: equal operation numbers in different machines can select an unrelated reached use or overwrite an installation site. Preserve the owning machine with the operation through the record, encoding and consumers so each registration names its actual call.
- Reported: 2026-11-14, Codex, while reviewing stored-machine executable preparation

### Selected-body activation copies retained side records
- Category: re-deriving
- Where: [body activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L669), [continuation storage clone](omega-rust/psi/compilation/representations/typed-trees/src/lib.rs#L1295), [owned arena clone](omega-rust/psi/foundation/arena/src/arena.rs#L748)
- Saw: Replacing symbols on a shared typed tree can copy side records, normalized identities and owned containers even with no bodies selected. The [call and machine record repair](https://github.com/CathedralOS/Omega/pull/3747) measured empty activation and two nonempty batches on both Windows x86-64 routes. Empty activation requested 2,182,456 bytes in 12,456 allocations and cloned 36 arenas per route on fixed `570d707` with the repair and observer (opt-level 1). See the [data-child repair](https://github.com/CathedralOS/Omega/pull/3720) for the earlier baseline.
- Cost: Share immutable storage where measurements still show copies. Limit writes to changed records or new batches. Preserve source application preparation, projection, authority checks, and signature and inferred-summary dependencies. Re-measure empty activation and nonempty completion on both [scalar and borrowed-root workloads](wiki/drafts/measurements/selected_body_copies.md#workloads-and-boundary), counting copied storage and new output separately. See the [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-15, Codex, while measuring continuation construction and body completion.
