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

### Native failure reports discard the instruction context
- Category: Verifying costs more than writing
- Where: [operand errors](omega-rust/omega/compilation/representations/selected-instructions/analyses/src/machine_effects.rs#L946), [compile and retention order](omega-rust/omega/application/src/execution/mod.rs#L201)
- Saw: The stored-machine program failed with only `SelectedInstructionId(4)`. That number is local to a function, but the error drops the function, instruction kind, operands and expected constraint. Failed compilation returns before artifact retention, and the temporary directory is removed.
- Cost: Finding the failed instruction can require repeating the 175-second compilation. Preserve the available instruction and constraint context in the error, or retain the failed stage input for debugging.
- Reported: 2026-11-17, Codex, while repairing native calls through stored machines


### Capture comparisons leave their time scope implicit
- Category: A verdict with nothing behind it
- Where: [address comparisons](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places/backing.rs#L78), [captured path comparison](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/storage.rs#L2551)
- Saw: Captured paths record static `(BlockId, u32)` positions. `Backing::same_single_region` and `Backing::definitely_contains` compare those paths without receiving an entry version. Lifetime and whole-value consumption checks live elsewhere; a static position alone does not distinguish separate loop entries.
- Cost: Reviewers must trace each caller to establish which execution the comparison covers. A loop that retires a binding and changes its index at the same capture point remains unverified; no accepted invalid program was observed. Make that time-scope premise explicit in the comparison API or establish one common caller rule.
- Reported: 2026-11-17, Codex, while checking reference transport through nested-machine loops

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

### A build-wide flag admits machine control for every output
- Category: A verdict with nothing behind it
- Where: [source admission](omega-rust/omega/compilation/compiler/src/checked/checking/source_checking.rs), [instruction reach](omega-rust/psi/foundation/symbols/src/builtin.rs), [mask helper](source/library/core/interrupt.omg), [terminal classification](omega-rust/omega/compilation/realization/provider-admission/src/terminal_authority_policy/classification.rs), [settled rule](wiki/spec/build/permissions.md#freestanding-machine-owner-authority)
- Saw: Source checking derives machine ownership from `Build.freestanding` for all bound targets. The interrupt-mask intrinsics, core helper and physical classifier still report `MachineControl`.
- Cost: Firmware selection can admit machine control in a hosted sibling, and a mask-only package already appears to have broad machine reach. Retire the field, check each selected output's provider authority, and carry `InterruptControl` through direct instructions, helpers, package review and physical classification. Exercise a mixed firmware/hosted build and a package update that adds a page-table write.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/configuration.md

### Changing storage analysis rebuilds the native compiler
- Category: Verifying costs more than writing
- Where: [semantic analysis package](omega-rust/psi/compilation/representations/terminal-psi/semantics/Cargo.toml), [declaration lookup in optimization](omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/structural_places.rs#L619), [target descriptor inputs](omega-rust/omega/compilation/representations/target-operations/target-vocabulary/src/descriptor_input.rs)
- Saw: Changing only the private descriptor replay cache and its error reporting rebuilt 64 packages and took 14m 17s before the original Console test could run. Some native consumers need only declaration or structural-path lookups from the same package; ownership analysis and explicit image validators genuinely need the full analysis. `target-operations` also declares an unused direct dependency, although removing it alone leaves a transitive dependency. Source and build evidence remains in claim `18e85a23de177c780000000018e83683`, under `post-3776-original-binding-control-49/native-terminal-semantics-coupling.json`.
- Cost: Edits to the analysis invalidate native consumers of small lookup functions. Move the declaration and path queries out of the full analysis crate, keep each judgment implemented once, and remove unused direct dependencies. Measure the same Console edit/build/run loop before claiming a build-time improvement.
- Reported: 2026-11-16, Codex, while carrying the original Console program through checking and execution

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

### Omega-written resolution compares integer type arguments by spelling
- Category: Re-deriving
- Where: [same-form literal rows](source/psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/resolver.omg#L2239), [same head](source/psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/domains.omg#L342)
- Saw: both compare two `Literal` type rows (`[u8; 4]`, `Buffer<u8, 4>`) with `spans_equal` on their source bytes, so `4` and `0x4` differ there, while typing interns the same rows as one `StaticInteger` by decoded value. Each `Literal` row now names its lexer-decoded value in `SyntaxTrees::integer_literals`.
- Cost: resolution and typing can disagree on whether two types are the same. Compare the decoded rows' values (and reject an invalid literal) instead of the spelling.
- Reported: 2026-11-16, Claude Code, while moving integer literal decoding into the lexer
