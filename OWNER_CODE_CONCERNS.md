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


### Learned incremental build state can fill the disk despite Cargo being disabled
- Category: Verifying costs more than writing
- Where: mbx `learned_incremental` setting and the host build environment in [build slots](tools/docs/build_slot.md).
- Saw: This Mac accumulated 23 GiB of mbx incremental state while the chains explicitly set `CARGO_INCREMENTAL=0`. mbx still had `learned_incremental=true` and injected `-Cincremental`. Disk exhaustion stopped the byte CLI build after its 19 tests passed, and stopped two other chains before tests began. Protected collection removed 18.65 GB of logical incremental data while preserving targets and shared objects.
- Cost: Repeated foundational builds regenerate enough private data to stop builds and even Git fetches. Current owned chains set the supported `MBX_LEARNED_INCREMENTAL=false` override; the resumed byte build kept about 17 GiB free. Host setup should respect explicit incremental settings or bound learned state without deleting live outputs. The audit is `/tmp/omega-protected-gc-sixth-20261116.json`; the completed producer records the override.
- Reported: 2026-11-16, Codex, while completing the draft PR takeovers.

### Editing bundled Core source recompiles most of the Rust compiler
- Category: Verifying costs more than writing
- Where: [bundled source in language-core](omega-rust/psi/foundation/language-core/src/sealed_intrinsics.rs#L221), [core vocabulary sources](omega-rust/psi/foundation/language-core/src/core_vocabulary.rs)
- Saw: Restoring Run/Vec library source at `849dc367b3952f2e2e6145f7fd104d178ea01bf1` changed no Rust code after the warm landed-source build at `37c2dfda95c`. Because `language-core` embeds the source bytes, the next optimized CLI build rebuilt 93 packages and took 343.56 seconds. The original program then refused during source discovery in 0.15 seconds. Cargo artifact records, source identities and timings are in `worker-2/build/generic-run-vec/restored-source-baseline/`.
- Cost: Each library repair invalidates Rust consumers that need vocabulary types but do not inspect bundled source. Separate immutable vocabulary from source-catalog data while preserving exact toolchain-source authentication; do not work around this by using a compiler with mismatched embedded source.
- Reported: 2026-11-16, Codex, while completing the generic library Vec


### Unpatched mbx bypasses the cache for checksum-enabled compilations
- Category: Verifying costs more than writing
- Where: [.cargo/config.toml](.cargo/config.toml), mbx 1.29.2 Rust compiler adapter
- Saw: Worker 1's ordinary focused build recorded `unknown-flag` for `-Zchecksum-hash-algorithm=blake3` in `/tmp/omega-worker1-cache-bypasses.jsonl`. Cargo emits that flag for the repository's content-based fingerprints. The adapter in upstream mbx 1.30.0 also rejects it. The shared cache therefore supplies no objects for these compilations.
- Cost: Independent worktrees rebuild unchanged dependencies. This Mac now uses a locally repaired 1.29.2 adapter that retains the checksum option in its input identity and preserves checksum dependency output. An isolated probe reused unchanged inputs and rejected a same-size, same-mtime source change; subsequent ordinary Omega builds stored cache objects without the checksum-flag bypass. The patch, original binary and receipts are retained in `worker-1/build/mbx-workflow-tooling-20261116` under the local worktree pool. Upstream releases and fresh hosts still need the repair; do not disable content fingerprint checks.
- Reported: 2026-11-16, Codex, during the draft PR takeovers

### Three more portable filesystem calls mean different things on Windows and POSIX
- Category: One idea, several implementations
- Where: [Windows timestamp range](source/library/std/targets/windows_x86_64/filesystem/metadata.omg#L110), [Windows directory name limit](source/library/std/targets/windows_x86_64/windows_filesystem_directory_entry.omg#L140), [Windows transfer limit](source/library/std/targets/windows_x86_64/windows_filesystem_transfer.omg#L78), [pilot diff](https://github.com/CathedralOS/Omega/tree/work/pilot-library-cross-target)
- Saw: [three layers](wiki/spec/build/platform_services.md) says a portable service "is identical on every target". A static diff of the two adapter families found three differences that #3864 did not cover:
  - `set_times` accepts any `i64` on POSIX, but Windows refuses times at or before 1601 and after about year 60056. The comment cites Win32, but the portable contract does not state the limit.
  - `read_dir_nth` errors on Windows for an entry name over 256 bytes of UTF-8, while a legal NTFS name can need up to 765, so a directory holding one cannot be listed.
  - A single `write` over 4 GiB is a short write on POSIX and `InvalidInput` on Windows. `read_at`, `write_at` and `copy` refuse a count over 4 GiB on Windows the same way.
  - `mbx xtask check-portable-services` tracks these as `known` rows in `tools/ratchets/portable_services.txt`; delete them with this entry.
- Cost: The same portable call fails on one target and succeeds on another. Either state each limit in the portable contract and apply it on every target, or lift it on Windows (larger name buffer, split transfers).
- Reported: 2026-11-16, Claude Code, while piloting a cross-target library diff

### The Omega-written compiler binds static arguments by kind instead of position
- Category: One idea, several implementations
- Where: [bind_static_arguments](source/psi/pipeline/03_symbol-resolved-trees-to-typed-trees/calls.omg#L379), [Rust positional binding](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/constructor_applications.rs#L1211)
- Saw: [applications and specialization](wiki/spec/language/generics.md#L383) matches arguments "by exact constructor and parameter position", and allows omitting an argument only where inference determines one substitution (L396). The Omega-written `bind_static_arguments` gives a static argument naming a machine to the next machine binder and any other argument to the next other binder, accepting when the counts match. `pick<double>(b)` against `pick<T, machine K>` is accepted there with `double` bound to `K`; the Rust compiler binds supplied arguments positionally, so `double` lands at `T`. Found by a blind checklist replay of abb38f673db and confirmed on main at fdc89014463. The `<machine K>` spelling is itself retired under TYPED-STATIC-PARAMETERS.
- Cost: The two compilers give the same call different meanings. Bind positionally, as Rust does, and refuse an argument whose kind does not fit its position.
- Reported: 2026-11-16, Claude Code, while testing a diff-triggered review checklist

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

### Changing storage analysis rebuilds the native compiler
- Category: Verifying costs more than writing
- Where: [semantic analysis package](omega-rust/psi/compilation/representations/terminal-psi/semantics/Cargo.toml), [declaration lookup in optimization](omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/structural_places.rs#L619), [target descriptor inputs](omega-rust/omega/compilation/representations/target-operations/target-vocabulary/src/descriptor_input.rs)
- Saw: Changing only the private descriptor replay cache and its error reporting rebuilt 64 packages and took 14m 17s before the original Console test could run. Some native consumers need only declaration or structural-path lookups from the same package; ownership analysis and explicit image validators genuinely need the full analysis. `target-operations` also declares an unused direct dependency, although removing it alone leaves a transitive dependency. Source and build evidence remains in claim `18e85a23de177c780000000018e83683`, under `post-3776-original-binding-control-49/native-terminal-semantics-coupling.json`.
- Cost: Edits to the analysis invalidate native consumers of small lookup functions. Move the declaration and path queries out of the full analysis crate, keep each judgment implemented once, and remove unused direct dependencies. Measure the same Console edit/build/run loop before claiming a build-time improvement.
- Reported: 2026-11-16, Codex, while carrying the original Console program through checking and execution

### Emission libraries still name earlier stage outputs
- Category: knowledge crossing a boundary
- Where: [emission facts](omega-rust/omega/compilation/backend/machine-emission/src/emission_facts.rs), [function realization](omega-rust/omega/compilation/backend/machine-emission/src/function_realization.rs), [runtime-only route](omega-rust/omega/compilation/native-compiler/src/realization.rs#L172), [stage-call exceptions](tools/xtask/src/boundary.rs)
- Saw: Fragment emission no longer keeps stage 02's output, and machine emission no longer depends on stages 05 to 09: it reads stage 09's output as `machine_code::OptimizedMachineLayout` and runs stage validators only through the `FunctionStageAudit` the driver attaches. Two reaches remain. `EmissionFacts` carries the whole optimized `AbstractOperationPlan`, because image emission's attribution and stack code looks up abstract functions by machine instead of reading rows the current form carries. `admit_terminal_runtime` runs stage 01, then calls stage 02's `capture_runtime_borrowed_storage_layouts` on the side.
- Cost: Emission can still read anything the abstract program knew, so a missing fact need not be carried into the selected form. Carry the abstract-operation rows image emission reads in the current form, move the runtime-only route's layout capture into the stages, and delete each `STAGE_CALL_ALLOWANCES` row as its edge goes. The [Atlas extra-arrow audit](atlas/plans/foundation-and-backend.md#start-with-the-extra-arrows) rechecks these mechanisms at `fe6e3d2e0104859a25ebfd1b0bfe4b424885e75f` and links related source/evaluation, handler, generated-code and verifier repairs. Its contracts remain plan-first work for one coordinated cutover; consult the current plan before restructuring these routes.
- Reported: 2026-11-16, pi, while checking that only the native driver calls Omega stages; narrowed 2026-11-16 by Claude Code after emission stopped holding stage 02's output, and by pi after machine emission stopped naming stages 05 to 09.

### Borrowed field recasts lose the target carrier layout of the enclosing record
- Category: re-deriving
- Where: [reference view geometry](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references/representation_geometry.rs#L147), [sum geometry](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references.rs#L4450)
- Saw: A recast checks the enclosing record's alignment, but its geometry query has no carrier layout for a sibling sum. The checker supplies the retained target's record resolver without its case-tag layout. An exact `PosixFilesystem` reduction refuses a whole aligned 144-byte field. Changing only the sibling `ErrorKind` field to `u32` yields native and Psi exit 0 on macOS arm64.
- Cost: The standard-library POSIX stat decoders still refuse the whole buffer. Pass the existing target carrier layout to projected storage checks, preserve the short and misaligned refusals, and replay the unchanged [note vault](https://github.com/CathedralOS/Omega/pull/3722).
- Reported: 2026-11-15, Codex, while validating shared recasts of an aligned record field.

### Focused compiler controls spend twenty-two minutes in code generation
- Category: Verifying costs more than writing
- Where: [compiler test targets](omega-rust/omega/compilation/compiler/Cargo.toml#L110), [compiler feature alignment](omega-rust/omega/compilation/compiler/Cargo.toml#L102), and the `coherent-native-controls-24-01` receipt in continuation claim `18e81ab82af520780000000018e96239`
- Saw: On macOS arm64 at `a827cf2c269`, a fixed-source nextest selection of 24 controls across 12 binaries spent 22m17s building before 176.905s of test execution. The command used the retained CLI feature union, optimization level 1, two codegen units and three jobs; 1,219 tests were not selected. Actual compiler processes remained active. After integrating main `a2684962ce882`, 13 selected unit controls across six crates spent 15m49s building and 0.414s running on the same host and budget (`merge-a268-direct-controls-01`, with exact source patch and input records retained in the active claim). The source of this build cost has not been established.
- Cost: The recorded twelve-binary replay spent twenty-two minutes building before exposing the next semantic refusal. A later three-binary replay at c16df641c12 built in 2m51s; that smaller selection does not identify the earlier cost. Measure the actual invalidation and compiled-target work, then repair the responsible build or test ownership while retaining the selected controls and exact-source optimized producer checks.
- Reported: 2026-11-14, Codex, while carrying stored-machine programs through Console and native-binding preparation

### Selected-body activation copies retained side records
- Category: re-deriving
- Where: [body activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L670), [continuation storage clone](omega-rust/psi/compilation/representations/typed-trees/src/lib.rs#L1337), [owned arena clone](omega-rust/psi/foundation/arena/src/arena.rs#L748)
- Saw: Empty selected-body activation still copies owned side records. The matched scalar and borrowed-root capture at `977465a3625923c98ce743cd9a2ca4ff90cf6fba` includes deferred-refusal rows (24,192 requested bytes), applied-domain applications (22,736), authored service-reach rows (20,680), and callable identities (9,768). Smaller raw row arenas request another 15,192 bytes in ten allocations. The derived composite cache separately copies on its first new key: 30,555 bytes in 433 allocations in the observed body-completion window. These are thread-local allocation requests at that source on Windows x86-64, not current-main or whole-process qualification; the [retained identity and conformance repair](https://github.com/CathedralOS/Omega/pull/3911) records the exact scopes, inputs and limits.
- Cost: Share unchanged payload under its actual owner rather than copy it when selecting bodies. Distinguish record copies from ownership-directory metadata, derived-cache detachment and genuine new output; parent and child owner counters overlap. Preserve source application preparation, projection, authority checks, and signature and inferred-summary dependencies. Use the same [scalar and borrowed-root workloads](wiki/drafts/measurements/selected_body_copies.md#workloads-and-boundary) for any follow-up comparison. See the [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-15, Codex, while measuring continuation construction and body completion.

### Omega-written resolution compares integer type arguments by spelling
- Category: Re-deriving
- Where: [same-form literal rows](source/psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/resolver.omg#L2239), [same head](source/psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/domains.omg#L342)
- Saw: both compare two `Literal` type rows (`[u8; 4]`, `Buffer<u8, 4>`) with `spans_equal` on their source bytes, so `4` and `0x4` differ there, while typing interns the same rows as one `StaticInteger` by decoded value. Each `Literal` row now names its lexer-decoded value in `SyntaxTrees::integer_literals`.
- Cost: resolution and typing can disagree on whether two types are the same. Compare the decoded rows' values (and reject an invalid literal) instead of the spelling.
- Reported: 2026-11-16, Claude Code, while moving integer literal decoding into the lexer
