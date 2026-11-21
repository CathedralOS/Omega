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

### The compiler map points to native lowering for a shared Psi input

- Category: The documents disagree.
- Where: [ownership map](source-rust/README.md#L28), [Build scalar dependency](source-rust/subsystems/compiler/build/evaluation/scalars/Cargo.toml#L16), [native consumer](source-rust/subsystems/compiler/psi-to-native/driver/inputs/mod.rs#L22), [interpreter consumer](source-rust/subsystems/interpreter/embedding/program.rs#L282), [artifact trust](source-rust/subsystems/compiler/psi-artifact/authorization/authorization.rs#L43), [wire format guide](source-rust/subsystems/compiler/psi-artifact/wire/README.md#L3).
- Saw: The map sends a Build maintainer to native stage 00 for admitted scalar values. Build's scalar evaluator uses `psi-scalar-applications` directly, as do native preparation and the interpreter. The wire guide's two specification links also miss the checkout. The wire codec owns the bytes, the receiving host authorizes their producer, and both execution engines consume the admitted inputs.
- Cost: Group the portable contracts under `psi/` so its root leads to `artifact/`, `execution-inputs/` and `semantics/` alongside the program definition. Keep the Cargo boundaries for wire encoding, producer trust and input checks. Document which owner admits a supplied value and which stage computes it. After each move, check the shared consumers and relative links. The existing [storage analysis concern](#changing-storage-analysis-rebuilds-the-native-compiler) tracks the cost of the semantics dependency.
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
- Where: [shared semantics crate](source-rust/subsystems/compiler/psi-semantics/Cargo.toml), [optimizer’s declaration lookup](source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit/src/structural_places.rs#L619), [target path lookup](source-rust/subsystems/compiler/psi-to-native/representations/target-operations/target-vocabulary/src/descriptor_input.rs#L21), [target execution facts](source-rust/subsystems/compiler/psi-to-native/representations/target-operations/target-vocabulary/src/execution_facts.rs#L13).
- Saw: The locked workspace graph at `da492310be6` has 37 direct normal dependents of `terminal-semantics`. Target vocabulary uses its structural paths and consumes callable analysis results. The original cache-only edit rebuilt 64 packages in 14m17s.
- Cost: A private storage-analysis change invalidates consumers of small lookup functions in the same crate. Separate the lookup-only edges, keep the full analysis dependency where it is used, and remeasure the original Console edit/build/run loop. The optimizer still depends on storage analysis through the structural-storage and Psi wire crates. Check those imports before splitting the next dependency.
- Reported: 2026-11-16, Codex, while carrying the original Console program through checking and execution
