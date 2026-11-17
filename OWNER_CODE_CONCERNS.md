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

### Claim verification hides an already identified restoration error
- Category: Re-deriving
- Where: [verdict.rs:939](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/verdict.rs#L939), [minted.rs:754](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/minted.rs#L754), and [plain_affine.rs:786](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/plain_affine.rs#L786).
- Saw: `examine` reconstructs the exact unavailable borrowed storage and its restoration flag. Later, `minted` calls `verify_claims_in`, which converts that same unavailable row into a blanket Unsupported before the ordinary restoration diagnostic is returned. In `a_whole_local_owned_home_is_repaired_before_its_reference_ends`, the restored positive passes and removing only `local = next;` gets this generic refusal.
- Cost: The invalid program still refuses, but its strict restoration regression test fails and callers cannot distinguish a broken restoration obligation from missing compiler support. Keep one availability owner and carry its actual reason through claim validation.
- Reported: 2026-11-16, Codex, while carrying Room A1 and the multiplication table through byte-observer checking.
### Saved native inputs do not reproduce the product they were saved from
- Category: tests that pin the bug
- Where: [round-trip check](omega-rust/omega/build-system/orchestration/examples/installation_assumption_review/saved_project.rs#L88), [recovery](omega-rust/omega/compilation/representations/abstract-operations/verified-optimization-input/src/retained_terminal_codec.rs#L1876), [evidence attachment](omega-rust/omega/compilation/native-compiler/src/application.rs#L44)
- Saw: `installation_assumption_review prepare tests/fixtures/run/calls/scalar_contract_route_exit/main.omg windows_x86_64 DIR` refuses with "saved native inputs do not reproduce the checked project" on `main`. The recovered `RetainedTerminalArtifact` differs from the original in two places. Recovery attaches no operation evidence, while `TerminalApplication` attaches it with `with_evidence`. And the proposal's provider-identity table numbers its rows in a different order: the first differing line maps `named(..Extent)` where the original has `trait-application(..ProgramStorageEntry)`.
- Cost: The review workflow the native-compiler README documents cannot prepare a saved project, and nothing on the Windows host tests the round trip; the only test of it (`closed_native_binding_reloads_and_executes_the_descriptor_callback`) runs on macOS arm64 only. Save or re-derive the evidence, encode provider identities in their canonical order, and run the round trip in a host-independent test.
- Reported: 2026-11-17, pi, while moving the saved native-input codec out of Omega 00.

### Unpatched mbx bypasses the cache for checksum-enabled compilations
- Category: Verifying costs more than writing
- Where: [.cargo/config.toml](.cargo/config.toml), mbx 1.29.2 Rust compiler adapter
- Saw: Worker 1's ordinary focused build recorded `unknown-flag` for `-Zchecksum-hash-algorithm=blake3` in `/tmp/omega-worker1-cache-bypasses.jsonl`. Cargo emits that flag for the repository's content-based fingerprints. The adapter in upstream mbx 1.30.0 also rejects it. The shared cache therefore supplies no objects for these compilations.
- Cost: Independent worktrees rebuild unchanged dependencies. This Mac now uses a locally repaired 1.29.2 adapter that retains the checksum option in its input identity and preserves checksum dependency output. An isolated probe reused unchanged inputs and rejected a same-size, same-mtime source change; subsequent ordinary Omega builds stored cache objects without the checksum-flag bypass. The patch, original binary and receipts are retained in `worker-1/build/mbx-workflow-tooling-20261116` under the local worktree pool. Upstream releases and fresh hosts still need the repair; do not disable content fingerprint checks.
- Reported: 2026-11-16, Codex, during the draft PR takeovers

### macOS arm64 packs a record stack argument at its natural alignment instead of a whole 8-byte slot
- Category: A verdict with nothing behind it
- Where: [Darwin stack packing](omega-rust/omega/compilation/backend/calling-conventions/src/plans/call_plan_evaluation.rs#L767), [ValueShape](omega-rust/omega/compilation/backend/calling-conventions/src/plans/vocabulary.rs#L144), [clang cross-check](omega-rust/omega/compilation/backend/calling-conventions/src/tests/clang_crosscheck.rs)
- Saw: `DarwinAapcs64` packs every stack argument at its natural alignment, records included. AAPCS64 B.5 rounds a Composite Type's size up to a multiple of 8 and C.14 rounds the stack address up to "the larger of 8 or the Natural Alignment of the argument's type"; Apple's packing covers the scalars its document shows, and clang keeps B.5 and C.14 for non-HFA records on Apple targets. So after eight register arguments, `{i16,i8}` goes at sp+2 under Omega and at sp+8 under clang, and every later stack argument shifts. The clang cross-check finds 24 of its 1,440 macOS signatures with this cause. `ValueShape` gives a small record and a scalar of the same size the same `Integer` class, so the evaluator cannot tell them apart; the fix needs the shape to say it is a record.
- Cost: Default-ABI imports and callbacks on macOS arm64 that pass a small non-float record on the stack exchange wrong values. Delete the `DarwinRecordStackSlot` entry in `KNOWN_MISMATCHES` with the fix.
- Reported: 2026-11-16, Claude Code, while turning the ABI pilot into a maintained clang cross-check

### The Omega 04 replay validator reruns the optimizer instead of checking it
- Category: A verdict with nothing behind it
- Where: [validator](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/selected_optimization.rs#L95), [fold replay](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/literal_fold_rewrites.rs#L655), [pre-allocation replay](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites.rs#L1934), [spec text](wiki/spec/build/command_line.md#L169)
- Saw: `SelectedOptimizationReplay::validate`, run as `SelectedInstructionRewriteReplay`, calls `validate_selected_lowering_optimization_custody` and `validate_pre_allocation_optimization_custody`. Those rebuild each step with the producer's own `build_step` and `run_passes` and compare the results. The comments and the new `command_line.md` paragraph call this independent. [Independent validation](wiki/spec/build/optimizations.md#atomic-candidates-and-independent-validation) says "A matcher or producer cannot attest to itself by serving as its own validator", and [pipeline.md](omega-rust/pipeline.md#portable-materialization-and-consumer-ownership) lets a checker share small predicates "but not the decision procedure that produces that output".
- Cost: `--every-validator` reports it as passed, but a rewrite bug that is deterministic passes it every time. Check each rewrite's preconditions and result with code that does not call the rewrite, or report the validator as a determinism check and fix the spec paragraph.
- Reported: 2026-11-16, Claude Code, while checking #3833 against the spec

### An optional Omega 04 rewrite shrinks the registers allocation may use
- Category: Knowledge crossing a boundary
- Where: [Omega 04 legality](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/selected_optimization.rs#L210), [allocation input](omega-rust/omega/compilation/representations/selected-instructions/analyses/src/lib.rs#L652)
- Saw: When a SelectedLowering or PreAllocation rule runs, Omega 04 builds legality under `FramelessLeafCallerSavedViews` and hands that analysis to Omega 05, which allocates on it. With no rewrite selected, `SelectedAllocationInput::into_legality` builds legality under `AllEnvironmentAllocatableViews`. So selecting an optional profitability rewrite restricts the whole program to caller-saved registers. This predates #3833, which kept it. [pipeline.md](omega-rust/pipeline.md#crate-placement-rule) says optimization history "must not select a different downstream representation", and [exact selection](wiki/spec/build/optimizations.md#exact-selection) treats allocation as mandatory "whether or not optional profitability rewrites are selected".
- Cost: The same program allocates under different constraints, and may be refused, depending on an unrelated optimization choice. Let the rewrite run on its own analysis and have Omega 05 build legality under the availability allocation needs, whatever ran before.
- Reported: 2026-11-16, Claude Code, while checking #3833 against the spec

### Allocation admission re-checks choices Omega 05 just made
- Category: Re-deriving
- Where: [admission](omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/output.rs#L250)
- Saw: On every compile, `validate_recovery_selection` checks that the recovery rule that ran equals the selection `stage_register_allocation` picked it from, and `capture_publication_facts` checks the manifest Omega 05 built against the record it built it from. [Verification placement](omega-rust/pipeline.md#verification-placement) says a default compile does not revalidate output the same compile just produced.
- Cost: The checks are cheap, but they cannot fail unless the same function is wrong twice, and they add error variants every reader must handle. Move them into the allocation validator or a test.
- Reported: 2026-11-16, Claude Code, while checking #3833 against the spec

### Three more portable filesystem calls mean different things on Windows and POSIX
- Category: One idea, several implementations
- Where: [Windows timestamp range](source/library/std/targets/windows_x86_64/filesystem/metadata.omg#L110), [Windows directory name limit](source/library/std/targets/windows_x86_64/windows_filesystem_directory_entry.omg#L140), [Windows transfer limit](source/library/std/targets/windows_x86_64/windows_filesystem_transfer.omg#L78), [pilot diff](https://github.com/CathedralOS/Omega/tree/work/pilot-library-cross-target)
- Saw: [three layers](wiki/spec/build/platform_services.md) says a portable service "is identical on every target". A static diff of the two adapter families found three differences that #3864 did not cover:
  - `set_times` accepts any `i64` on POSIX, but Windows refuses times at or before 1601 and after about year 60056. The comment cites Win32, but the portable contract does not state the limit.
  - `read_dir_nth` and `read_dir_is_empty` error on Windows for an entry name over 256 bytes of UTF-8, while a legal NTFS name can need up to 765, so a directory holding one cannot be listed.
  - A single `write` over 4 GiB is a short write on POSIX and `InvalidInput` on Windows. `read_at`, `write_at` and `copy` refuse a count over 4 GiB on Windows the same way.
  - `mbx xtask check-portable-services` tracks these as `known` rows in `tools/ratchets/portable_services.txt`; delete them with this entry.
- Cost: The same portable call fails on one target and succeeds on another. Either state each limit in the portable contract and apply it on every target, or lift it on Windows (larger name buffer, split transfers).
- Reported: 2026-11-16, Claude Code, while piloting a cross-target library diff

### A qualified collection gets builtin `[..]` indexing without a declared operator
- Category: A verdict with nothing behind it
- Where: [builtin meaning check](omega-rust/psi/compilation/representations/typed-trees/src/operator_declarations.rs#L2135), [typing selection](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/constructor_applications.rs#L74), [byte-view tests](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/byte_view_elements.rs#L593)
- Saw: [operands](wiki/spec/language/domains.md#L752) says builtin operators accept unqualified operands only, and indexing "follows the same rule through its operator declarations ... a qualified collection or index has such a declaration or is cast" (L777). Typing selects builtin indexing when `has_builtin_spelled_expression_meaning` holds, but that function never checks operand qualification. So with `domain [u8]::Path requires no_nul(self)`, `let x: &[u8] = &path[0..p];` on a `&[u8] in Path` is accepted as builtin indexing with no declaration and no cast, and the byte-view tests assert that acceptance. A declared range operator is still honoured; only the no-declaration case falls through. Found by a blind checklist replay of #3789 and confirmed on main at fdc89014463.
- Cost: Qualified values lose the rule that their operators come from declarations, and a test pins the gap. Refuse builtin meaning for qualified operands in `has_builtin_spelled_expression_meaning`, require a declaration or cast, and turn the pinning test into a refusal twin.
- Reported: 2026-11-16, Claude Code, while testing a diff-triggered review checklist

### The Omega-written compiler binds static arguments by kind instead of position
- Category: One idea, several implementations
- Where: [bind_static_arguments](source/psi/pipeline/03_symbol-resolved-trees-to-typed-trees/calls.omg#L379), [Rust positional binding](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/constructor_applications.rs#L1211)
- Saw: [applications and specialization](wiki/spec/language/generics.md#L383) matches arguments "by exact constructor and parameter position", and allows omitting an argument only where inference determines one substitution (L396). The Omega-written `bind_static_arguments` gives a static argument naming a machine to the next machine binder and any other argument to the next other binder, accepting when the counts match. `pick<double>(b)` against `pick<T, machine K>` is accepted there with `double` bound to `K`; the Rust compiler binds supplied arguments positionally, so `double` lands at `T`. Found by a blind checklist replay of abb38f673db and confirmed on main at fdc89014463. The `<machine K>` spelling is itself retired under TYPED-STATIC-PARAMETERS.
- Cost: The two compilers give the same call different meanings. Bind positionally, as Rust does, and refuse an argument whose kind does not fit its position.
- Reported: 2026-11-16, Claude Code, while testing a diff-triggered review checklist

### A read through a runtime index gets a field equation the spec says it never supplies
- Category: The documents disagree
- Where: [read leaf](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places.rs#L2251), [read_equation](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places.rs#L3514), [runtime-indexed store logged as value](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places/log.rs#L479)
- Saw: [store vocabulary](wiki/spec/terminal-psi/structural_access.md#L556) says a runtime-element read "supplies no field equation", and such a store "publishes no field equation" (L639). The write log records a store at `[RuntimeIndex i]` as `Stored::Value(v)`, and `read_equation` gives a later read of the same path `r == v`, so `a[i] = 7; need7(a[i]);` is accepted. This is sound (same selector, overlap-based retirement), but contradicts the page.
- Cost: A change made to match the spec would silently refuse programs that pass today. Either allow the equation for an identical selector in the spec, or drop it for runtime-index paths.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/terminal-psi/structural_access.md

### Specialization never checks a type parameter's `[copy]`, `[linear]` or `carry` bound against the supplied type
- Category: One idea, several implementations
- Where: [Operand::read](omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/passes/specialization.rs#L1095), [named_type](omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/passes/specialization.rs#L1537), [generate](omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/passes/specialization.rs#L2257), [callable applications refuse bounds](omega-rust/psi/compilation/representations/typed-trees/src/callable_applications.rs#L419)
- Saw: [applications and specialization](wiki/spec/language/generics.md) says "Every application validates the complete argument tuple, including kinds, ranges, and constraints of unused parameters", and [property declarations](wiki/spec/language/ownership.md) says every use of `T` receives the property fixed by the declaration and application. `Operand::read` binds a type argument through `named_type` without reading `TypeParameter::bounds`, `generate` checks only callable binders, and the checker re-checks the generated copy as a closed machine. So `machine keep<T [copy]>(x: T) -> T { x }` called as `keep<Token>(t)` with a non-`[copy]` `Token` is accepted. Three other application paths refuse any non-default bound as unjudged; machine specialization is the one that skips it.
- Cost: A `[copy]` bound in a generic API is not enforced at its callers. Compare the argument's multiplicity and carry with the binder's bounds in `Operand::read`, and refuse bounds it cannot judge, as the other paths do.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/generics.md

### A shared byte view of an array gives its elements different addresses in the Psi interpreter
- Category: Re-deriving
- Where: [shared view is a snapshot](omega-rust/psi/interpreter/src/boundary_inputs.rs#L1403), [shared reborrow snapshot](omega-rust/psi/interpreter/src/storage_bytes_and_views.rs#L1017), [fresh namespace](omega-rust/psi/interpreter/src/storage_bytes_and_views.rs#L3003), [view element address](omega-rust/psi/interpreter/src/storage_bytes_and_views.rs#L3077), [array element address](omega-rust/psi/interpreter/src/storage_primitives.rs#L6016)
- Saw: [address exposure](wiki/spec/language/counts_and_addresses.md#L35) says compatible views of the same location "expose the same address", and [interpreted address identities](wiki/spec/language/counts_and_addresses.md#L107) says changing a view's type "cannot change the referent's address". A shared view `&bytes[..]` of a `[u8; N]` array, and a shared reborrow of a mutable view, copy the bytes into `ByteSequenceView::new`, which draws a new exposure namespace; `address_of(&view[i])` encodes that namespace while `address_of(&bytes[i])` encodes the array's identity. `var bytes: [u8; 4]; let view = &bytes[..]; exit_process(if address_of(&bytes[0]) == address_of(&view[0]) { 0 } else { 1 });` exits 0 natively and 1 under Psi. Traced in the interpreter; the lowering of `address_of` on a view place was not confirmed.
- Cost: Psi gives the wrong answer on a well-formed program. Compute view-element addresses from the `OriginalByteArrayWindow` the view already carries. TASKS.md ADDRESS-EXPOSURE owns completing `address_of` and may absorb this; this is a wrong answer from the part already in use, not a refusal.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/counts_and_addresses.md

### The Omega-written compiler lets a case pattern with a `field: value` test cover the whole case
- Category: A verdict with nothing behind it
- Where: [case_pattern](source/psi/pipeline/03_symbol-resolved-trees-to-typed-trees/coverage.omg#L206), [valued field typing](source/psi/pipeline/03_symbol-resolved-trees-to-typed-trees/statements.omg#L484)
- Saw: [patterns.md](wiki/spec/language/patterns.md#L30) says "`field: value` is ordinary projected equality", so `Reading::Value { level: 0 }` matches only some `Value` readings. `case_pattern` checks only that the arm names the case and ignores the pattern's fields, so `transition r { Reading::Value { level: 0 } -> 0  Reading::Missing -> 1 }` passes coverage and `Value { level: 5 }` matches nothing.
- Cost: The coverage verdict has nothing behind it. Only an arm whose fields are all binders or `as _` should close its case.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/patterns.md

### The compiler has no `InternalFailure` outcome, so a defect it detects in itself exits as "source rejected"
- Category: A verdict with nothing behind it
- Where: [outcome enum](omega-rust/omega/compilation/products/src/compile_outcomes.rs#L109), [non-diagnostic errors become Reject](omega-rust/omega/build-system/orchestration/src/lib.rs#L233), [validator failure called a compiler defect](omega-rust/omega/build-system/orchestration/src/lib.rs#L215), [exit status](omega-rust/omega/application/src/commands/compile.rs#L244), [reserved-codes comment](omega-rust/omega/application/src/arguments/mod.rs#L33)
- Saw: [outcomes](wiki/spec/build/compiler_request.md#L249) says compilation "preserves four outcome distinctions", including `InternalFailure` for "a compiler/checker malfunction or violated implementation invariant", with halt value 3; [independent stage validators](wiki/spec/build/command_line.md) says to "distinguish compiler invariant failures from invalid authored source". `CompilerRequestOutcome` has only `Complete`, `Reject` and `Incomplete`, and `BuildError::outcome` maps every non-diagnostic error (`StageValidation`, `Publication`, `BuildDirectory`, `CheckWorkspace`) to `Reject`. A failing `omega --validator TargetOperationCorrespondence main.omg` prints "this is a compiler defect, not an error in the source" and exits 1. The Omega-written compiler has the case (`source/omega/application/ocreq/outcome.omg`).
- Cost: Scripts reading `$?` cannot tell a compiler bug from a rejected program. Add `InternalFailure` (exit 3), classify compiler-invariant errors as it, and exit with it.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/compiler_request.md

### `x == S::C` on a payload-bearing sum becomes a bare tag test when Psi 03 cannot type `x`
- Category: One idea, several implementations
- Where: [untyped comparison left in place](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_preparation.rs#L217), [no type for a receiver call](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/typing_queries.rs#L1425), [read as a case test](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/expressions.rs#L500), [case test without payload check](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/places.rs#L1412)
- Saw: conformances.md says "Records and payload-bearing sums require an explicit named conformance", and data_and_literals.md forbids "silently changing tag equality into field equality". Psi 03 enforces that only when `DeclaredExpressionTypes` types both operands; otherwise it `continue`s, and the query returns `None` for receiver calls, calls with machine arguments and `match` expressions. Psi 05 then reads any `==` whose right side names a case as `IsCase`, without checking for payload cases or a selected `Equatable`. With `data Cmd { Stop; Move { dx: i32; } }`, `Codecs::decode(&bytes) == Cmd::Stop` compiles as a tag test while `let c: Cmd = Codecs::decode(&bytes); c == Cmd::Stop` is refused (or calls the custom `Equatable<Cmd>`).
- Cost: The compiler accepts an equality the spec refuses, and `==` means different things depending on how the operand is spelled. Let Psi 05 read `==` as a case test only for payload-free sums, or have Psi 03 refuse a comparison it cannot type.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/conformances.md

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

### Native products evaluate their scalar conditions twice
- Category: re-deriving
- Where: [product checking](omega-rust/omega/compilation/compiler/src/terminal/terminal_artifact.rs#L94), [native completion](omega-rust/omega/build-system/build-settings/src/activation/scalar_completion.rs#L25)
- Saw: `ProducedTerminalProduct::check_conditions` completes the artifact's scalar conditions for the target with no application roots. `complete_native` then completes them again with the program's application roots and the native demand before `native_compiler::compile`. The two closures differ in scope, so neither result can stand in for the other as built.
- Cost: Every native compile with scalar conditions evaluates the deployed conditions twice. Extend one completed closure with the native demand and application roots, or complete once with both where the retained native product is formed, keeping product checking's refusals and their order.
- Reported: 2026-11-16, pi, while making native compilation one call.

### Emission libraries still name earlier stage outputs
- Category: knowledge crossing a boundary
- Where: [emission facts](omega-rust/omega/compilation/backend/machine-emission/src/emission_facts.rs), [function realization](omega-rust/omega/compilation/backend/machine-emission/src/function_realization.rs), [runtime-only route](omega-rust/omega/compilation/native-compiler/src/realization.rs#L172), [stage-call exceptions](tools/xtask/src/boundary.rs)
- Saw: Fragment emission no longer keeps stage 02's output, and machine emission no longer depends on stages 05 to 09: it reads stage 09's output as `machine_code::OptimizedMachineLayout` and runs stage validators only through the `FunctionStageAudit` the driver attaches. Two reaches remain. `EmissionFacts` carries the whole optimized `AbstractOperationPlan`, because image emission's attribution and stack code looks up abstract functions by machine instead of reading rows the current form carries. `admit_terminal_runtime` runs stage 01, then calls stage 02's `capture_runtime_borrowed_storage_layouts` on the side, and `native-artifact` reads Omega 00's `VerifiedNativeArtifactInput` for runtime provisions.
- Cost: Emission can still read anything the abstract program knew, so a missing fact need not be carried into the selected form. Carry the abstract-operation rows image emission reads in the current form, move the runtime-only route's layout capture into the stages, and delete each `STAGE_CALL_ALLOWANCES` row as its edge goes.
- Reported: 2026-11-16, pi, while checking that only the native driver calls Omega stages; narrowed 2026-11-16 by Claude Code after emission stopped holding stage 02's output, and by pi after machine emission stopped naming stages 05 to 09.

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
- Where: [body activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L670), [continuation storage clone](omega-rust/psi/compilation/representations/typed-trees/src/lib.rs#L1295), [owned arena clone](omega-rust/psi/foundation/arena/src/arena.rs#L748)
- Saw: Replacing symbols on a shared typed tree can copy other side records, normalized identities and owned containers even with no bodies selected. A matched pair on fixed `673a8aa` plus the declaration-selection sharing repair and numeric observer recorded 376,901 requested bytes, 1,582 allocations and 36 arena clones per route on Windows x86-64 (opt-level 1); this is historical measurement, not current-main qualification. See the [call and machine record repair](https://github.com/CathedralOS/Omega/pull/3747) and [data-child repair](https://github.com/CathedralOS/Omega/pull/3720) for earlier baselines.
- Cost: Share immutable storage where measurements still show copies. Limit writes to changed records or new batches. Preserve source application preparation, projection, authority checks, and signature and inferred-summary dependencies. Re-measure empty activation and nonempty completion on both [scalar and borrowed-root workloads](wiki/drafts/measurements/selected_body_copies.md#workloads-and-boundary), counting copied storage and new output separately. See the [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-15, Codex, while measuring continuation construction and body completion.

### Omega-written resolution compares integer type arguments by spelling
- Category: Re-deriving
- Where: [same-form literal rows](source/psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/resolver.omg#L2239), [same head](source/psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/domains.omg#L342)
- Saw: both compare two `Literal` type rows (`[u8; 4]`, `Buffer<u8, 4>`) with `spans_equal` on their source bytes, so `4` and `0x4` differ there, while typing interns the same rows as one `StaticInteger` by decoded value. Each `Literal` row now names its lexer-decoded value in `SyntaxTrees::integer_literals`.
- Cost: resolution and typing can disagree on whether two types are the same. Compare the decoded rows' values (and reject an invalid literal) instead of the spelling.
- Reported: 2026-11-16, Claude Code, while moving integer literal decoding into the lexer
