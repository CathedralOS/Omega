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

### Interpreter admission runs native abstract optimization to obtain borrowed storage layouts

- Category: Knowledge crossing a boundary.
- Where: [runtime stop](source-rust/subsystems/compiler/psi-to-native/driver/mod.rs#L121), [interpreter provisions](source-rust/subsystems/compiler/psi-to-native/driver/runtime/mod.rs#L116), [optimizer call](source-rust/subsystems/compiler/psi-to-native/driver/compile_body.rs#L51), [layout computation](source-rust/subsystems/compiler/psi-to-native/pipeline/04_abstract-operations-to-target-operations/src/target_lowering/mod.rs#L475), [host consumer](source-rust/subsystems/interpreter/host/src/execution/host_imports.rs#L505).
- Saw: The runtime path passes its input through the native abstract optimizer before asking stage 04 only for the layout of borrowed values. The host bridge consumes these layouts. The computation checks the target and uses the plan's structural types and lowering facts. An optimizer failure can therefore stop runtime admission before the interpreter starts.
- Cost: Define the exact target-bound storage description the host and native code require. Prove both consumers obtain the same layouts for a borrowed value passed to a provider, including refusal of a mismatched layout. Then make ordinary interpreter admission consume that description directly. Keep actual foreign adapter emission in the native compiler.
- Reported: 2026-11-20, Codex, while tracing the Rust compiler at `d94ef8e20f1e3`.

### Shared Psi semantics live outside the portable program's directory

- Category: The documents disagree.
- Where: [Psi program](source-rust/subsystems/compiler/psi/README.md), [shared semantics](source-rust/subsystems/compiler/psi-semantics/README.md).
- Saw: Source checking, native compilation and interpretation read the shared meanings of Psi operations from a directory outside the portable program. A maintainer following the program, artifact and execution-input owners still has to leave Psi to find those rules.
- Cost: Move the existing semantics crates under `psi/semantics/`. Keep their Cargo boundaries, check the shared consumers, and update the ownership guides and relative links. The [storage analysis concern](#changing-storage-analysis-rebuilds-the-native-compiler) tracks the separate dependency cost.
- Reported: 2026-11-20, Codex, while tracing the Rust compiler at `d94ef8e20f1e3`.

### Callable analysis loses arguments passed to checked providers

- Category: Re-deriving.
- Where: [native stage 01 selected body](source-rust/subsystems/compiler/psi-to-native/pipeline/01_admitted-native-inputs-to-target-bound-program/src/execution.rs#L124), [native callable analysis](source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/verified-optimization-input/src/lib.rs#L528), [optimization boundary case](source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit-semantics/src/callable_values.rs#L293), [interpreter capabilities](source-rust/subsystems/interpreter/instructions/execution.rs#L288).
- Saw: Native stage 01 now binds the selected boundary to the checked body and can propagate its actual arguments. The optimization analysis still treats boundary calls as opaque and activates the checked provider separately with external inputs. The interpreter’s capability check also discovers the provider separately from the caller's arguments.
- Cost: An indirect call in the provider needs the actual machine passed by its caller. The original report reached native stack planning. A later run on the #4790 CLI checked the modernized source, then stopped at a repeated stage-07 selection before either engine could exercise the remaining paths. Recheck both consumers with the same selected call.
- Reported: 2026-11-18, Codex, while migrating Build-selected dispatch to machine values.

### Focused compiler checks rebuild the implementation three times

- Category: Verifying costs more than writing
- Where: [integration test dependency](source-rust/subsystems/compiler/Cargo.toml#L29), [forwarded test features](source-rust/subsystems/compiler/source-to-psi/Cargo.toml#L11), [provider-choice integration test](source-rust/subsystems/compiler/tests/source_provider_choices.rs#L12).
- Saw: The locked Cargo unit graphs for a source-compiler unit test, the compiler’s `source_provider_choices` integration test and a CLI build contain three distinct `source_compiler` artifacts: a test executable, a library with `test-support`, and a normal library.
- Cost: These controls cannot reuse a single compiled implementation. A historical macOS run on `a827cf2c269` spent 22m17s building 24 controls across 12 binaries and 177s executing them. Measure the same selection and feature sets before attributing that time to this split.
- Reported: 2026-11-18, Codex, while fixing repeated machine-type declarations

### Three more portable filesystem calls mean different things on Windows and POSIX

- Category: One idea, several implementations
- Where: [portable service contract](wiki/spec/build/platform_services.md), [timestamp conversion](source/library/std/targets/windows_x86_64/filesystem/metadata.omg#L120), [portable directory entry](source/library/std/filesystem.omg#L66), [Windows name conversion](source/library/std/targets/windows_x86_64/windows_filesystem_directory_entry.omg#L143), [Windows transfer counts](source/library/std/targets/windows_x86_64/windows_filesystem_transfer.omg#L78), [POSIX adapter](source/library/std/posix_filesystem_adapter.omg#L528), [reviewed differences](tools/ratchets/portable_services.txt).
- Saw: The portable contract promises identical behavior across targets. The current adapters differ in three places.

  - Windows refuses times at or before the 1601 epoch and above 1,833,029,933,770 Unix seconds. POSIX forwards signed seconds to the host, which can still refuse a particular time.
  - Windows refuses a decoded directory entry name longer than 256 UTF-8 bytes. The portable `DirEntry.name` also has a 256-byte capacity. A legal NTFS name can need more space than either provides.
  - A plain read or write with a valid buffer and count above `u32::MAX` is refused on Windows. The same limit applies to positioned reads, positioned writes and copy. POSIX can return a short transfer. Windows `write_all` already chunks its requests.
- Cost: A program using the same portable call encounters different limits. Set a common contract or reconcile the adapters, then remove the corresponding `known` rows. Plain read and write have no such rows today.
- Reported: 2026-11-16, Claude Code, while piloting a cross-target library diff

<a id="a-build-wide-flag-admits-machine-control-for-every-output"></a>

### Changing storage analysis rebuilds the native compiler

- Category: Verifying costs more than writing
- Where: [optimization unit identity](source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit/src/identity.rs#L2062), [native structural type catalog](source-rust/subsystems/compiler/psi-to-native/representations/structural-storage/src/structural_type_catalog.rs#L64), [abstract operations' storage queries](source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/src/control_flow.rs#L12).
- Saw: A private edit to the storage analysis still rebuilds the optimizer. The optimizer imports the Psi codec to encode a structural domain for its identity. Its abstract operations also import the native structural catalog, which encodes access and quotient declarations. Cargo therefore builds the full Psi wire validator for these callers.
- Cost: Keep the canonical declaration encodings available to these consumers without compiling the storage checks that validate an incoming Psi artifact. Preserve the bytes used for optimizer identity. Measure a warm compiler rebuild after editing the private descriptor cache and run the original Console package.
- Reported: 2026-11-16, Codex, while carrying the original Console program through checking and execution
