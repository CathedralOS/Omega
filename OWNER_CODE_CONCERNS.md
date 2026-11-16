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

### The proof kernel admits quotient effectivity for any relation, so a certificate can prove false
- Category: A verdict with nothing behind it
- Where: [effective_declaration](omega-rust/psi/compilation/proof-admission/src/mathematical_core.rs#L1907), [pi_description](omega-rust/psi/compilation/proof-admission/src/mathematical_core.rs#L2868)
- Saw: [Set-quotient foundation](wiki/spec/proofs/quotients.md#set-quotient-foundation) starts the interface from `A` and `R` "with selected proofs that every `R(a,b)` is a mere proposition and that `R` is an equivalence relation"; effectivity (`Id Q (project a) (project b) → R(a,b)`) is consistent only under those hypotheses. `pi_description` binds only `A` and `R`, and `effective_declaration` assumes the effectivity statement for every relation. With `A = Two` and the constant relation `R := λ_ _. Id Two zero one`, `effective(Two, R, x, x, refl(Q, project x))` has type `Id Two zero one`, and its assumption closure is only `{Q, project, effective}`. The tests use only `R = Id Two`, which happens to be an equivalence.
- Cost: The admitted quotient assumptions that receivers are told to accept are jointly inconsistent; any certificate citing them can conclude anything. Add the equivalence and mere-proposition proofs as explicit hypotheses of `effective`, and add a negative test with a non-reflexive relation.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/proofs/quotients.md

### A store to `items[j]` does not retire the known case of `items[0]`
- Category: One idea, several implementations
- Where: [case retirement](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places.rs#L614), [may_overlap](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/place_paths.rs#L6), [membership-test retirement](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places.rs#L2786), [payload-access judgment](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places.rs#L3884), [runtime-index store](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lower/places.rs#L4251)
- Saw: [store vocabulary](wiki/spec/terminal-psi/structural_access.md#L639) says a store crossing a runtime element "overlaps every element of that array"; [case observation](wiki/spec/terminal-psi/structural_access.md#L355) grants "no payload-access refinement after intervening mutation" and says a membership on a runtime element "keys no case fact". Content facts retire through `overlap`/`may_overlap`, which treats `[items, RuntimeIndex(j)]` as overlapping `[items, FixedIndex(0)]`. Case facts use a second, narrower relation: `Asked::Case(sum).ended_by` retires only on a write to the sum or a prefix of it. So in `if items[0] in Choice::Left { items[j] = Choice::Right {}; return items[0].item.value; }` the payload read is accepted, and at run time with `j == 0` it reads `Right`'s bytes as `Left`'s payload. `edge_selects` also keys a case fact from a membership on `[items, RuntimeIndex(i)]`. Traced through stages 05 and 06; the program was not compiled.
- Cost: The lowered-form checker can accept a payload read after an aliasing element replacement, and the only test covers a direct write to the sum. Retire case facts on any row that `overlap`s the sum (except a payload store under the active case), stop keying case facts from runtime-index memberships, and add an `items[0]`/`items[j]` twin.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/terminal-psi/structural_access.md

### `==` selects an equality implementation from every package, ignoring visibility and dependencies
- Category: Knowledge crossing a boundary
- Where: [family from every conformance](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_preparation.rs#L45), [selection by type identity](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_preparation.rs#L316), [synthesized call](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_preparation.rs#L641)
- Saw: [declaration and selection](wiki/spec/language/conformances.md) says a conformance "is private unless declared `public`" and cross-package selection "requires public visibility and ordinary direct-dependency authority"; core equality acquisition says "There is no ambient search." `prepare_selections` collects every core `Equatable` conformance in the program into one list keyed only by the operand's package-qualified type, never reads the conformance's visibility or declaring package, and rewrites `==` into a call after symbol resolution, so no visibility check sees it. A private `PointEq satisfies Equatable<Point>` in dependency `geo` is called by an application's `a == b`; an unrelated package declaring a second public `Equatable<Point>` makes every package comparing `Point` fail with "multiple Equatable implementations".
- Cost: A dependency's private implementation becomes public behavior, and adding a separately named map anywhere breaks unrelated packages. Restrict candidates to conformances visible at the comparing site, reusing the exposure check Psi 02 applies to authored selections.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/conformances.md

### `x == S::C` on a payload-bearing sum becomes a bare tag test when Psi 03 cannot type `x`
- Category: One idea, several implementations
- Where: [untyped comparison left in place](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_preparation.rs#L217), [no type for a receiver call](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/typing_queries.rs#L1425), [read as a case test](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/expressions.rs#L500), [case test without payload check](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/places.rs#L1412)
- Saw: conformances.md says "Records and payload-bearing sums require an explicit named conformance", and data_and_literals.md forbids "silently changing tag equality into field equality". Psi 03 enforces that only when `DeclaredExpressionTypes` types both operands; otherwise it `continue`s, and the query returns `None` for receiver calls, calls with machine arguments and `match` expressions. Psi 05 then reads any `==` whose right side names a case as `IsCase`, without checking for payload cases or a selected `Equatable`. With `data Cmd { Stop; Move { dx: i32; } }`, `Codecs::decode(&bytes) == Cmd::Stop` compiles as a tag test while `let c: Cmd = Codecs::decode(&bytes); c == Cmd::Stop` is refused (or calls the custom `Equatable<Cmd>`).
- Cost: The compiler accepts an equality the spec refuses, and `==` means different things depending on how the operand is spelled. Let Psi 05 read `==` as a case test only for payload-free sums, or have Psi 03 refuse a comparison it cannot type.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/conformances.md

### The parser accepts `usize`, `isize`, `nat` and `Nat` literal suffixes and throws them away
- Category: A verdict with nothing behind it
- Where: [suffix list](omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/tokens_and_storage.rs#L337), [strip_integer_suffix](omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/tokens_and_storage.rs#L309), [test pinning it](omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/tokens_and_storage.rs#L399), [sample](samples/cli/probes/literal_suffixes/main.omg#L5), [guide example](wiki/language_guide/chapter_5_expressions_evaluation.md#L11)
- Saw: [separate roles](wiki/spec/language/counts_and_addresses.md#L11) says "`usize` and `isize` are not Omega carriers", and [integer radix spellings](wiki/spec/language/data_and_literals.md#L447) admits only concrete widths such as `17u13` and bare `i`/`u`. `strip_integer_suffix` first checks `UNLANDED_INTEGER_SUFFIXES` and returns the digits with no landing type, so `let a: i8 = 9usize;` and `let x: u8 = 3nat;` compile and nothing later can see the suffix. A unit test asserts that `7usize` has no landing, `samples/cli/probes/literal_suffixes` claims "`usize` is its own primitive", and the guide says "A suffix fixes the type" beside `3nat`.
- Cost: Authors are taught spellings that look like carrier choices but mean nothing. Delete `UNLANDED_INTEGER_SUFFIXES` so they hit "unknown integer literal suffix", flip the test, change the sample to `9u64`, and fix the guide example.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/data_and_literals.md

### `omega run --both` reports agreement it did not observe
- Category: A verdict with nothing behind it
- Where: [declined comparison returns the native status](omega-rust/omega/application/src/commands/run.rs#L117), [declined cases](omega-rust/omega/application/src/execution/mod.rs#L552), [what counts as unavailable](omega-rust/omega/interpreter/terminal-runtime/src/execution.rs#L114), [low-byte status match](omega-rust/omega/application/src/execution/process_exit.rs#L50), [test pinning it](omega-rust/omega/application/src/execution/process_exit.rs#L127)
- Saw: [compiler tests](wiki/spec/build/testing.md#L308) say the native exit status must equal the interpreted one, [process exit observations](wiki/spec/terminal-psi/observations.md#L154) say "Host status presentation cannot weaken argument comparison", and [process_exit.md](wiki/spec/language/process_exit.md#L152) says fewer host status bits do not authorize truncation. Two paths break this:
  - `compare_interpretation` returns `InterpreterComparison::Declined` when the Psi run exhausts its work budget, waits on the host, returns a non-process result, or raises `UnsupportedSemanticVariant` partway through; `commands::run` and `run_cases` then exit with the native code. A program that exits 0 natively while its Psi run stops partway makes `--both` exit 0.
  - On Unix, `matches_host_presentation` compares `semantic_status & 255` with the native code and returns `Agrees`. Psi `exit_process(256)` agrees with a native exit of 0, and the unit test pins native 212 as matching Psi -44.
- Cost: AGENTS.md rule 4 relies on `--both` to prove native/Psi agreement; on these paths it cannot catch a wrong native exit argument or an unfinished Psi run. Only `mbx xtask sample-gate` is protected, because [samples.rs](tools/xtask/src/samples.rs#L794) requires an `interp exit:` line. Give a declined comparison its own non-zero status, report a truncated host status as unverified rather than `Agrees`, and update both tests.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/testing.md and wiki/spec/language/process_exit.md

### An artifact-only Build accepts provider selections and native bindings and then drops them
- Category: A verdict with nothing behind it
- Where: [artifact-only check](omega-rust/omega/build-system/build-settings/src/activation/preparation.rs#L238), [selections passed on unchecked](omega-rust/omega/build-system/build-settings/src/activation/preparation.rs#L287), [artifact-only report skips the product](omega-rust/omega/compilation/products/src/checked_compilation_report.rs#L137)
- Saw: [scoped execution](wiki/spec/build/scoped_execution.md#L710) and [project role](wiki/spec/build/declarations.md#L134) say artifact-only mode rejects executable root and provider selections rather than silently ignoring them. `Preparation::complete` refuses only when `completed.root_bindings` is non-empty, although its message says "cannot select executable entries or providers". It passes `completed.product_selections` and `completed.native_bindings` forward unchecked, and `ProductReportPlan::prepare` never builds the product they would apply to. A `build.omg` that calls `Build::artifact_only` and then `Build::select_provider<Console, CapturedConsole>` or `Build::bind` compiles, and the selection has no effect.
- Cost: The author believes a provider choice or native binding took effect, and the refusal message claims a check that does not exist. Refuse provider selections and native bindings in artifact-only mode too, with a negative twin beside `tests/fixtures/pass/build/captured_named_input_required_output`.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/scoped_execution.md

### Without a lock, permission admission accepts assumed bindings by comparing the Build with itself
- Category: A verdict with nothing behind it
- Where: [lockless branch](omega-rust/omega/compilation/compiler/src/product_execution.rs#L492), [from_public_build](omega-rust/omega/packages/review-evidence/src/permission_acceptance.rs#L47), [self-comparison](omega-rust/omega/packages/review-evidence/src/permission_acceptance.rs#L154), [sample README](samples/cli/facts/assumed_binding_result_bound/README.md#L33)
- Saw: [assumed binding contract acceptance](wiki/spec/packages/acceptance.md#L185) says root authorization "is not itself package acceptance" and missing acceptance "prevents use of the assumed relationship"; [locks.md](wiki/spec/packages/locks.md#L141) says an absent lock supplies no accepted bindings or permissions. When no lock exists, `admit_permissions` builds the "accepted" rows with `TerminalPermissionAcceptance::from_public_build` from the same checked program, and `validate` then compares those rows with rows rebuilt from the same input, so the check cannot fail. Today `product_declarations::admit` refuses every `assume_binding_contract` with "no product route yet", so the branch is not reachable from a `build.omg`; the sample README nevertheless documents "No `omega packages update` lock is required" as the intended route.
- Cost: The moment the product route lands, the highest-risk mechanism needs no project decision, only a missing lock. Refuse runtime products carrying assumed bindings when no lock exists, delete `from_public_build`, and correct the sample README.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/packages/acceptance.md

### Every build.omg runs before the lock is compared with the sources and compiler
- Category: Knowledge crossing a boundary
- Where: [resolution runs Builds first](omega-rust/omega/build-system/orchestration/src/build.rs#L91), [settle before admission](omega-rust/omega/build-system/orchestration/src/project/project_compilation.rs#L44), [admission after settle](omega-rust/omega/build-system/orchestration/src/project/project_compilation.rs#L69), [identity comparison](omega-rust/omega/packages/package-manager/src/operations/accepted_compilation.rs#L446)
- Saw: [ordinary compilation](wiki/spec/packages/acceptance.md#L82) says that when a lock exists "its exact target and occurrence inputs must match before Build execution", and sources.md says no requested code executes before admission completes. `build()` calls `resolve_project_sources`, which runs the root and every dependency `build.omg`; `compile_returned` settles them; only then does `admit_accepted_contexts` call `require_current_acceptance`. After `omega packages update`, editing the root `build.omg` or switching compilers lets the changed Build run, including staged writes, before "source identity changed" or "compiler identity changed".
- Cost: A stale or unreviewed Build runs its effects on an accepted project before the refusal the spec promises. Compare compiler and root source identity right after local project preparation, and each dependency's identity before its activation is scheduled.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/packages/acceptance.md

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

### Every module decode runs the proof kernel, although product.md says receipt does not replay it
- Category: The documents disagree
- Where: [decode calls it](omega-rust/psi/compilation/representations/terminal-psi/wire/src/semantic_module.rs#L5006), [kernel check](omega-rust/psi/compilation/representations/terminal-psi/wire/src/identity_formations.rs#L44), [product receipt](wiki/spec/terminal-psi/product.md#L200), [decoding does not re-decide](wiki/spec/terminal-psi/encoding.md#L154), [Id formations](wiki/spec/terminal-psi/encoding.md#L1141)
- Saw: product.md says mathematical certificates are canonically decoded and "receipt does not require kernel replay", and encoding.md L154 says decoding "does not re-decide the encoded judgment". encoding.md L1141 says the shared Id-formation certificate checks a closed `TwoZero : Two` judgment, and `decode_closed_parts` follows that: it always calls `validate_identity_formations`, which runs `verify_mathematical_certificate` and `weak_head_normalize` under `Budget::default()`, on every load including inspection of untrusted bytes.
- Cost: Readers of the admission policy believe no kernel runs at decode, and a certificate needing more than the default budget is refused at load. Either state this as an exception in product.md or move the check to source-bound `verify_identity_formations`.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/terminal-psi/product.md

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

### encoding.md's byte tables have drifted from the Terminal Psi codec
- Category: The documents disagree
- Where: [scalar route tags](wiki/spec/terminal-psi/encoding.md#L214), [structural route tags](wiki/spec/terminal-psi/encoding.md#L2152), [route wire enum](omega-rust/psi/compilation/representations/terminal-psi/wire/src/module_wire.rs#L2564), [comparison-kind tags](wiki/spec/terminal-psi/encoding.md#L522), [comparison decoder](omega-rust/psi/compilation/representations/terminal-psi/wire/src/structural_wire.rs#L49), [operation table](wiki/spec/terminal-psi/encoding.md#L925), [tags 133–137](omega-rust/psi/compilation/representations/terminal-psi/wire/src/block_wire.rs#L545), [access-view row](wiki/spec/terminal-psi/encoding.md#L2187), [access-view record](omega-rust/psi/compilation/representations/terminal-psi/wire/src/module_wire.rs#L2401), [register event authority](wiki/spec/terminal-psi/encoding.md#L3076)
- Saw: The page says its tables give "the physical layout of every operation, terminator, scalar-term, and proposition form the codec accepts", and that a tag absent from a closed table rejects. The codec differs:
  - Domain establishment routes are one shared enum with tags 4/5/6 and two `u32` indices, not scalar tags 0/1/2 with one index and structural tags 1/2/3 with a string. Both domain rows also carry an optional `required_memberships` field and an optional route list, whose absence the `Vacuous` and `Proved` origins depend on.
  - `IeeeFloatComparisonKind` accepts 3–6 (`LessThan` through `NotLessOrEqual`); the table lists only 1 and 2 and calls them propositions only.
  - The operation table skips 133–137 (`GlobalDescriptorTable`, `ActivateTaskRegister`, `ReadTaskRegister`, `ReloadSegmentDescriptors`, `ReadSegmentSelector`), which the codec decodes; the module-table order omits `scalar_predicate_definitions` between tables 35 and 36.
  - The access-view input row lists identity strings where the codec writes `u32` indices, and omits `runtime_position` and `carrier_identity`. The page calls the roster positional at L2187 and requires canonical order at L3045; the code checks neither order, and checks duplicates twice with different keys (`foundation_validation.rs#L11824`, `semantics/src/access.rs#L2077`).
  - The register-event section says the no-authority observation is "exactly `SystemRead(Rflags)`", but `requires_service_authority` also exempts `MemoryFence`, as the catalog paragraph at L937 says.
- Cost: An encoder, decoder or refusal test written from the spec produces bytes the codec refuses, or treats valid forms as malformed. Rewrite the tables from the codec, pick one access-view order rule and one duplicate check, and add a test that compares each tagged table with its decoder.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/terminal-psi/encoding.md

### Three Terminal Psi spec pages still credit judgments to a checker that ran before lowering
- Category: The documents disagree
- Where: [loans.md](wiki/spec/terminal-psi/loans.md#L8), [loans.md restoration](wiki/spec/terminal-psi/loans.md#L124), [structural_access.md refutation](wiki/spec/terminal-psi/structural_access.md#L667), [dynamic_dispatch.md](wiki/spec/terminal-psi/dynamic_dispatch.md#L104), [checker declines refutation](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places.rs#L2736)
- Saw: loans.md says "lowering consumes the checked opening/reborrow plans" and "the checked lifetime and restoration relationships"; dynamic_dispatch.md says "Checked output supplies explicit callable construction/call plans … Lowering consumes them"; structural_access.md relies "on source checking for the case-refutation judgment". [Check on the lowered form](wiki/drafts/designs/check_on_lowered_form.md#L24) says "Lower first", and AGENTS.md says the tree checker is gone. In the code, `edge_selects` says the false edge "establishes nothing" and refutation "is not decided here"; no other stage decides it.
- Cost: Readers look upstream for checked plans and a refutation judgment that do not exist, and may assume "source checking" covered a program. Reword the pages so the lowered-form checker owns each judgment, and put case refutation on the LOWERED-FORM-CHECKER item as a gap.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/terminal-psi/loans.md

### The kernel admits `liftPre` as an eighth quotient assumption that two spec pages exclude
- Category: The documents disagree
- Where: [lift_precondition_declaration](omega-rust/psi/compilation/proof-admission/src/mathematical_core.rs#L2241), [quotients derive list](wiki/spec/proofs/quotients.md#L78), [classicality seven assumptions](wiki/spec/proofs/classicality.md#L127), [kernel metatheory liftPre](wiki/spec/proofs/kernel_metatheory.md#L312)
- Saw: quotients.md says lifting is derived "rather than adding separate default axioms", and classicality.md lists exactly seven admitted assumptions (`Q, project, setQ, sound, effective, elim, beta`). The kernel also assumes `QUOTIENT_LIFT_PRECONDITION`, and its test admitting policy lists eight. Only kernel_metatheory.md admits `liftPre`, while still calling the interface "the seven interface assumptions".
- Cost: A receiver policy built from quotients.md or classicality.md disagrees with what the kernel records. Derive `liftPre`, or ratify it as an eighth named assumption in all three pages; the second adds an assumption, so it needs an owner ruling in `OWNER_QUESTIONS.md`.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/proofs/quotients.md

### The trusted rule `IntegerPowerOfTwoOrder` has no row in the proof-rule audit table
- Category: The documents disagree
- Where: [classicality.rs foundation](omega-rust/psi/compilation/proof-admission/src/classicality.rs#L57), [provisional marker](omega-rust/psi/compilation/proof-admission/src/classicality.rs#L74), [checker emits it](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/prove/widths.rs#L606), [rule table](wiki/spec/proofs/classicality.md#L35)
- Saw: classicality.md calls its table "the audit of every proof rule an accepted certificate can cite", and [admitting an arithmetic rule](wiki/spec/proofs/foundation.md#L232) requires each provisional rule to land with its row. `AcceptedProofRule::IntegerPowerOfTwoOrder` (`m ≤ n ⊢ 2^m ≤ 2^n`) is provisional, believed rather than derived ("the core has no power of two to derive it from"), and the lowered-form checker emits it. It is the only one of the 36 rules with no row, and nothing under `wiki/` names it.
- Cost: Reviewers see an incomplete trusted surface, with no recorded side conditions, test or rule tag for this rule. Add its row: side conditions, the program that needs it, its exhaustive arithmetic test with a near-miss twin, and its tag.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/proofs/classicality.md

### review.md says a missing lock stops native production; locks.md, acceptance.md and the code allow it
- Category: The documents disagree
- Where: [review.md](wiki/spec/packages/review.md#L20), [locks.md](wiki/spec/packages/locks.md#L141), [acceptance.md](wiki/spec/packages/acceptance.md#L78), [package-manager README](omega-rust/omega/packages/package-manager/README.md#L142), [no lock, no refusal](omega-rust/omega/packages/package-manager/src/operations/accepted_compilation.rs#L449)
- Saw: review.md says "missing accepted state stops native production before Build", and the package-manager README says an unlocked check creates no native-compiler authority. locks.md and acceptance.md say an absent lock permits development compilation including native production and `omega run`, and the code does that.
- Cost: A reader of review.md expects unlocked projects to be check-only, and a "fix" toward that page would break `omega run` on every unlocked sample. Bring review.md and the README in line with locks.md.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/packages/review.md

### sources.md still names the retired `depend_as` and `build_depend_as` calls
- Category: The documents disagree
- Where: [request forms](wiki/spec/packages/sources.md#L8), [aliases](wiki/spec/packages/sources.md#L24), [replacement](wiki/spec/build/scoped_execution.md#L84), [dependency reader](omega-rust/omega/packages/package-manager/src/declarations/dependencies/read/mod.rs#L1)
- Saw: sources.md describes `depend*`/`build_depend*` requests and says "`depend_as` and `build_depend_as` supply validated aliases". scoped_execution.md says `Build::dependency` and `Build::build_dependency` replace those names, and the code, topology.md and the samples accept only the new calls with `dependency::with_alias`.
- Cost: A `build.omg` written from sources.md is refused. Rewrite both sentences in terms of `Build::dependency`, `Build::build_dependency` and `dependency::with_alias`.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/packages/sources.md

### The language guide says `terminates` promises a return; the spec says it does not
- Category: The documents disagree
- Where: [guide chapter 9](wiki/language_guide/chapter_9_proof_obligations.md#L127), [termination.md](wiki/spec/language/termination.md#L3), [permitted endpoints](wiki/spec/language/termination.md#L10), [process_exit.md](wiki/spec/language/process_exit.md#L62)
- Saw: Chapter 9 says "`ensures` describes the result if the machine returns. `terminates` promises that it does". termination.md says termination "does not promise a particular result" and permits a covered crash or a call that cannot return, such as the process-exit transfer; process_exit.md says `terminates` "neither authorizes that outcome nor promises normal return". A `terminates` helper that calls `ProcessExit::exit_process` on one branch is valid, and its `ensures` never holds there.
- Cost: Agents read the guide first and learn that `terminates` plus `ensures` is total correctness, which misleads contract reviews of exit and crash paths. Say instead that `terminates` promises the machine finishes, by returning, crashing on an allowed route or calling something that never returns, and link the endpoint list.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/termination.md

### Language guide chapter 14 teaches two forms the spec refuses
- Category: The documents disagree
- Where: [operator under a data type](wiki/language_guide/chapter_14_traits.md#L181), [record with open contracts](wiki/language_guide/chapter_14_traits.md#L245), [Codec requirements](wiki/language_guide/chapter_14_traits.md#L60)
- Saw: The operator example declares `machine < Card::less_by_power(...)`, but [machines.md](wiki/spec/language/machines.md#L28) retires machines declared under a data carrier and [expressions.md](wiki/spec/language/expressions.md#L146) says "Data is not a code namespace"; it also calls `Ranked::compare`, which `Ranked` never declares. The dispatch example passes a `Codec<T>` record at run time although its requirements omit `reaches` and `crashes`; effects.md says "An open contract in a closed position rejects".
- Cost: The omega-language skill routes agents to the guide, so they copy refused forms into fixtures and samples. Declare the operator as a free `machine <`, and close the `Codec` requirements or type `format` with a closed refinement.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/machines.md

### numeric_values.md puts `min`, `max`, `abs`, `clamp` and `sqrt` in `std`; the library keeps them in `core`
- Category: The documents disagree
- Where: [numeric helpers](wiki/spec/language/numeric_values.md#L14), [library responsibilities](wiki/spec/build/declarations.md#L179), [core/math.omg](source/library/core/math.omg#L1), [std importing from core](source/library/std/targets/linux_arm64/posix_filesystem_impl.omg#L10)
- Saw: numeric_values.md says these helpers "belong in `std`'s math surface". declarations.md defines `std` as interfaces and adapters to the execution environment and `core` as foundational language-facing mechanisms; the helpers live in `source/library/core/math.omg`, `std` itself imports `omega::language::core::min_u64`, and no `std` module exposes math.
- Cost: An agent following numeric_values.md would move the helpers into the OS-adapter library. Change numeric_values.md to say `core`.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/numeric_values.md

### Repeated `--target` is deduplicated by `omega` but refused by `omega packages`
- Category: One idea, several implementations
- Where: [compile](omega-rust/omega/application/src/arguments/compile.rs#L151), [packages install/update](omega-rust/omega/application/src/arguments/packages.rs#L93), [packages audit](omega-rust/omega/application/src/arguments/audit.rs#L74)
- Saw: [exact target requests](wiki/spec/build/configuration.md#L176) says an explicit selection is "normalized to canonical order with duplicates removed". `omega --target linux_x86_64 --target linux_x86_64` removes the repeat through `ExplicitTargetSet`; `omega packages install|update|audit` with the same flags fails with "duplicate target" and keeps the typed order.
- Cost: One rule has three parsers that disagree. Parse every `--target` list through `ExplicitTargetSet::from_caller_names` and delete the per-command checks.
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
