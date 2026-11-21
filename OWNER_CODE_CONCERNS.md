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

### Extending a resolved program still requires its original parser trees

- Category: Knowledge crossing a boundary.
- Where: [extension inputs](source-rust/subsystems/compiler/source-to-psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/generated_source.rs#L19), [driver handoff](source-rust/subsystems/compiler/source-to-psi/driver/resolve_and_type.rs#L465), [saved checkpoint](source-rust/subsystems/compiler/source-to-psi/driver/resolve_and_type/storage.rs#L7), [alias test](source-rust/subsystems/compiler/source-to-psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/pre_resolution/initializers.rs#L338).
- Saw: The resolver receives the previous resolved program, the new syntax and the previous syntax. Its test holds the resolved base and new file fixed. `Holder` can use the base's `Pair<Byte>` when the old forest is supplied. An empty forest produces `selected type alias has no retained source declaration`. The driver also writes that forest to its checkpoint.
- Cost: An extension cannot use the resolved artifact alone. Give the resolved declaration its reusable alias body, parameter bindings, lexical owner and diagnostic spans. Have a consumer extend a saved base using only the resolved base and new input, check the instantiated field type and reject an invalid application at its use. Then retire the old forest argument and its checkpoint storage.
- Reported: 2026-11-20, Codex, while tracing the Rust compiler at `d94ef8e20f1e3`.

### Build and Product each wire lowering to the same checker

- Category: One idea, several implementations.
- Where: [Build invocation](source-rust/subsystems/compiler/source-to-psi/driver/compile_build.rs#L199), [Product roster](source-rust/subsystems/compiler/source-to-psi/driver/compile_product/judge_product_roster.rs#L307), [invocation judgment](source-rust/subsystems/compiler/source-to-psi/driver/judge_lowered_source.rs#L129), [Product judgment](source-rust/subsystems/compiler/source-to-psi/driver/judge_lowered_source.rs#L152), [shared lowerer](source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/source_roster.rs#L149).
- Saw: The Product driver root reads clearly as resolve, judge, publish. Inside it, two functions independently call stage 05, attribute its diagnostics, call the same stage 06 checker, attribute its diagnostics and turn acceptance into different publication types. The Product wrapper calls the same lowerer and attaches the exact roster it used.
- Cost: A change to stage order or diagnostic attribution has two call sites. Put the stage sequence in one function over prepared trees and a request that identifies invocation or Product roots. Return one source-bound judgment. Product needs its roster for provider retries, and Build needs invocation scope and accounting. Exercise both callers through the public entry, including the stage and span of a refusal, then remove the second sequence.
- Reported: 2026-11-20, Codex, while tracing the Rust compiler at `d94ef8e20f1e3`.

### Build's package format imports its provider records from the Product driver

- Category: Knowledge crossing a boundary.
- Where: [driver export](source-rust/subsystems/compiler/source-to-psi/source_to_psi.rs#L20), [stored selection](source-rust/subsystems/compiler/source-to-psi/driver/compile_product/provider_declarations.rs#L29), [package encoding](source-rust/subsystems/compiler/build/inputs/package/provider_selection_codec.rs#L16), [package decoding](source-rust/subsystems/compiler/build/inputs/package/provider_selection_codec.rs#L374).
- Saw: The Product driver defines `RetainedProviderSelection`, including the declaration, closed application, target, composition mode and origin. Build imports that type through `compile_product` and constructs it when reading a package. The same driver module also exports the operations that select and rejoin providers.
- Cost: Changing the driver can force a package reader to change with it. Give the retained selection a home in the source representations. Let Build's codec depend on the record and let the source compiler own its construction and resolution against the receiving source. Read existing encoded selections in a consumer test. Preserve an operator family and reject a mismatched source identity, then remove the import through the driver.
- Reported: 2026-11-20, Codex, while tracing the Rust compiler at `d94ef8e20f1e3`.

### The Build host opens typed trees to identify the receiver it may lend

- Category: Knowledge crossing a boundary.
- Where: [Build host admission](source-rust/subsystems/compiler/build/inputs/build_execution/build_admission.rs#L56), [source type walk](source-rust/subsystems/compiler/build/inputs/build_execution/build_receiver.rs#L22), [lowered receiver correspondence](source-rust/subsystems/compiler/build/inputs/build_execution/build_receiver.rs#L69), [published receiver check](source-rust/subsystems/compiler/build/inputs/build_execution/build_receiver.rs#L125).
- Saw: Before lending its Build value, the host reads the source machine, its first state and its type-reference nodes to identify the toolchain's `&mut Build`. It then opens the accepted lowering to match the receiver to the executable parameter. `BuildReceiver::parameter_for` already checks the published invocation, entry, parameter and storage identity.
- Cost: The Build host has to understand the typer's representation before it can lend a value. Have source checking and publication issue the evidence that connects the exact toolchain declaration to the executable receiver. Build should verify that evidence against the invocation and own the actual loan and filesystem grants. Exercise admission after dropping the typed producer, including rejection of a lookalike Build type and a receiver from a different invocation.
- Reported: 2026-11-20, Codex, while tracing the Rust compiler at `d94ef8e20f1e3`.

### Source checkpoints carry both a package permission callback and its recorded answer

- Category: Knowledge crossing a boundary.
- Where: [scope interface](source-rust/subsystems/compiler/foundation/declaration-scope/declaration_scope.rs#L10), [Product request](source-rust/subsystems/compiler/build/planning/product.rs#L161), [checkpoint writer](source-rust/subsystems/compiler/source-to-psi/driver/resolve_and_type/storage.rs#L11), [source reuse](source-rust/subsystems/compiler/source-to-psi/driver/resolve_and_type/activation.rs#L88), [evaluation checkpoint](source-rust/subsystems/compiler/source-to-psi/driver/compile_time_invocations/frontier.rs#L293).
- Saw: Product hands the source driver a live package-visibility interface. Saving a checkpoint asks it for a package table. Reuse checks the old callback or the saved table against the current callback for selected requester/owner pairs. Compile-time evaluation keeps its own live and recorded forms and rejoins them on load.
- Cost: Changes to visibility have to work in both live queries and checkpoint replay. Have the package owner issue an immutable declaration scope for the requested Build or Product purpose. Pass the same data to resolution, evaluation and checkpoint comparison. Check that fresh and reopened checkpoints give identical answers when an alias names different packages in the two scopes, and that changing a relevant grant invalidates reuse. Retire the live/recorded alternatives for this lookup.
- Reported: 2026-11-20, Codex, while tracing the Rust compiler at `d94ef8e20f1e3`.

### Checking an evaluation charge reopens the generated caller's body

- Category: Re-deriving.
- Where: [receipt fields](source-rust/subsystems/compiler/source-to-psi/representations/evaluation-receipts/accounting.rs#L85), [recording a call](source-rust/subsystems/compiler/source-to-psi/driver/compile_time_invocations/calls/execution.rs#L52), [charge validation](source-rust/subsystems/compiler/source-to-psi/representations/evaluation-receipts/accounting.rs#L332), [Product accounting](source-rust/subsystems/compiler/source-to-psi/driver/compile_product.rs#L234).
- Saw: An invocation receipt keeps the requesting typed program, the frozen caller and the published program alive. Checking its charge reads the caller's sole state and last transition to establish that it returned the recorded call. Product checks the accumulated charges after settling provider work.
- Cost: A completed receipt keeps trees alive for later accounting. Make the source producer issue the checked association between the requester, call and emitted site while those trees are available. Bind it to the exact executed artifact, inputs, sponsor, outcome and measured work. Test a substituted site or charge and measure retained memory over many calls before removing the source-tree references from completed receipts.
- Reported: 2026-11-20, Codex, while tracing the Rust compiler at `d94ef8e20f1e3`.

### The CLI owns workspace dependency planning and provider lookup

- Category: Knowledge crossing a boundary.
- Where: [project host](source-rust/application/src/project_sources.rs#L76), [workspace member reuse](source-rust/application/src/project_sources.rs#L217), [provider alias selection](source-rust/application/src/project_sources/services.rs#L176), [recursive Product handoffs](source-rust/application/src/project_sources/services.rs#L238), [package command caller](source-rust/application/src/commands/packages/transaction.rs#L2).
- Saw: `project_sources` suggests file loading. It keeps the active, observed and settled Builds, reuses the actual workspace member invocation, decides whether a provider alias is ambiguous, and recursively chooses the host or deployment target for each dependency handoff. Compile, run and package review reach this code through the binary.
- Cost: A library caller cannot use that project preparation without reproducing part of the CLI. Expose project preparation from the package manager using Build's existing planning requests and returns. Keep acquisition, aliases and graph traversal with the package manager and execution with Build. Test preparation without linking the CLI, including exactly-once workspace execution, alias ambiguity and distinct host/deployment targets. The CLI can then pass options and render the result.
- Reported: 2026-11-20, Codex, while tracing the Rust compiler at `d94ef8e20f1e3`.

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

### Build preparation misses a suspended helper used as a transition subject

- Category: One idea, several implementations.
- Where: [Build's syntax walk](source-rust/subsystems/compiler/source-to-psi/driver/compile_build/waits/mod.rs#L318), [resolution's transition subject capture](source-rust/subsystems/compiler/source-to-psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/declarations/dispatches.rs#L86), [ordinary suspension placement check](source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/operational.rs#L142).
- Saw: At `98fb39c09558c`, `ready` prepared a dependency and returned whether its public machine returned 7. Binding `suspend ready(builder, registration)` to a local and branching on it passed with native/Psi agreement. Putting the same call directly after `transition` reported that the dependency machine had no visible declaration inside `ready`. The direct form passed when the helper still prepared the dependency and returned `true` without importing it.
- Cost: The Build planner recognizes a call in a transition arm's target but misses the subject that ordinary resolution captures. Follow the authored call through that capture so both forms stop at the same preparation and admit the helper's return in the mounted scope.
- Reported: 2026-11-20, Codex, while comparing ordinary and staged suspension placement.

### Copying a deep scalar proof expression consumes the Rust call stack

- Category: One idea, several implementations.
- Where: [scalar expressions](source-rust/subsystems/compiler/foundation/semantic-records/src/scalar_terms.rs#L976), [integer math expressions](source-rust/subsystems/compiler/foundation/semantic-records/src/proposition.rs#L1191), [logical clone handing off to scalar clone](source-rust/subsystems/compiler/foundation/semantic-records/src/proposition.rs#L1663), [deep scalar admission control](source-rust/subsystems/compiler/psi-semantics/proofs/bounded/proof.rs#L4912).
- Saw: The logical clone walker delegates scalar operands to a derived recursive clone. Copying an iteratively constructed, left-nested `ExactIntegerAdd` on a 2 MiB Rust thread succeeded at 8,192 levels and overflowed at 16,384 during clone. The control excluded recursive disposal.
- Cost: A consumer copying a single arithmetic clause can abort its process. The separate 8,192-level Boolean test passed both clone and kernel-backed admission. The arithmetic control starts with a vocabulary value, so source-level reachability remains unverified.
- Reported: 2026-11-20, Codex, while checking field identities in suspended Build contracts.

### Field and export judgments still describe the earlier source tree

- Category: The documents disagree.
- Where: [contract judgments](tools/xtask/pipeline-contracts.json), [comparison against current declarations](tools/xtask/src/boundary_contracts.rs#L359).
- Saw: At `da492310be6`, the full `xtask check-boundary` reports 395 discrepancies: 173 changed signatures, 114 declarations missing a judgment and 108 stale judgments. The separate ownership and stage-reader check passes.
- Cost: The full check cannot certify the maintained tree until the owners review the differences. Preserve each field’s reason when correcting its location and judge each changed declaration against the current source.
- Reported: 2026-11-19, Codex, while restoring the compiler ownership check.

### Callable analysis loses arguments passed to checked providers

- Category: Re-deriving.
- Where: [native stage 01 selected body](source-rust/subsystems/compiler/psi-to-native/pipeline/01_admitted-native-inputs-to-target-bound-program/src/execution.rs#L124), [native callable analysis](source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/verified-optimization-input/src/lib.rs#L528), [optimization boundary case](source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit-semantics/src/callable_values.rs#L293), [interpreter capabilities](source-rust/subsystems/interpreter/instructions/execution.rs#L288).
- Saw: Native stage 01 now binds the selected boundary to the checked body and can propagate its actual arguments. The optimization analysis still treats boundary calls as opaque and activates the checked provider separately with external inputs. The interpreter’s capability check also discovers the provider separately from the caller's arguments.
- Cost: An indirect call in the provider needs the actual machine passed by its caller. The original report reached native stack planning. A later run on the #4790 CLI checked the modernized source, then stopped at a repeated stage-07 selection before either engine could exercise the remaining paths. Recheck both consumers with the same selected call.
- Reported: 2026-11-18, Codex, while migrating Build-selected dispatch to machine values.

### Invocation inference loses the parameter behind a callable alias

- Category: One idea, several implementations.
- Where: [callable ceiling inference](source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/services.rs#L484), [checked callable origins](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/ceilings.rs#L506), [local alias fixture](samples/cli/facts/dynamic_trait_dispatch/local/main.omg).
- Saw: Stage 05 adds a callable parameter to the ceiling when the selected slot is a formal. Stage 06 also follows copies back to their formal. In a replay of the local dispatch fixture on the #4790 CLI, the direct call and the explicitly annotated alias passed. Only the unannotated alias was refused: `invokes none` versus `parameter 1`.
- Cost: The inferred ceiling is empty for `let alias = selected; alias(...)`, so checking rejects a valid call. Inference and checking need the same callable origin information.
- Reported: 2026-11-18, Codex, while migrating the local runtime selection sample.

### Package review recovery cannot verify structural parameter indices

- Category: A verdict with nothing behind it.
- Where: [recovered borrow and qualification indices](source-rust/subsystems/package-manager/review/evidence/src/encoding/recovery/policy/public_api/traits.rs#L64), [public trait validation](source-rust/subsystems/package-manager/review/evidence/src/record/public_policy/validation/declarations.rs#L75).
- Saw: Recovery reads a parameter number from each returned borrow and each qualification. It checks ordering and domain identity, and accepts a number beyond the entire formal list. The policy holds each formal's type name, while source checking knows which formals use structural storage.
- Cost: A recovered policy can describe a borrow or qualification on a nonexistent parameter. Retain the checked structural parameter list so recovery can validate both kinds of index.
- Reported: 2026-11-18, Codex, while restoring package review for stored machine calls.

### Case splits rebuild unchanged proof facts

- Category: Verifying costs more than writing
- Where: [case-split prover construction](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/prove.rs#L1752), [assumption indexing](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/prove.rs#L1084), [fact substitution](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/known.rs#L801).
- Saw: The remaining fallback splitter clones the assumptions and constructs a new prover and fact index for each alternative. In the original multiplication-table run, 65 provers made 80,080 substitutions. The replay took 1.23 seconds and equation checking took 22 milliseconds. A 128-entry substitution cache made that run 12% slower.
- Cost: Each branch rebuilds facts from its parent, even when its new assumption leaves those facts unchanged. Measure how much of that work branches can share while keeping their assumptions and search budget separate.
- Reported: 2026-11-18, Codex, while investigating slow range checks in the portable filesystem and standard library.

### Individual proof citations copy their entire compound premise

- Category: Verifying costs more than writing
- Where: [premise reconstruction](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/known.rs#L178), [copied conclusion](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/known.rs#L211), [owned proof nodes](source-rust/subsystems/compiler/psi/proofs/certificates.rs#L96).
- Saw: Selecting a conjunct reconstructs the parent's proof and clones its complete proposition. In the original multiplication-table capture (machine 7, obligation 26), 1,408 assumption citations contributed to a 3,798-node proof. Its debug rendering occupied 1.95 GB and the checked core certificate occupied 212 KB.
- Cost: Borrowing the proof during verification removes one extra copy, but building it still copies these large premises. Any sharing must count every logical use, keep its original scope and publish the complete predicate. Certificates attached to operations still refer to whole premise rows, so those assumptions cannot simply be replaced with selected conjuncts.
- Reported: 2026-11-18, Codex, while carrying the original multiplication table through proof checking and execution.

### Focused compiler checks rebuild the implementation three times

- Category: Verifying costs more than writing
- Where: [integration test dependency](source-rust/subsystems/compiler/Cargo.toml#L29), [forwarded test features](source-rust/subsystems/compiler/source-to-psi/Cargo.toml#L11), [provider-choice integration test](source-rust/subsystems/compiler/tests/source_provider_choices.rs#L12).
- Saw: The locked Cargo unit graphs for a source-compiler unit test, the compiler’s `source_provider_choices` integration test and a CLI build contain three distinct `source_compiler` artifacts: a test executable, a library with `test-support`, and a normal library.
- Cost: These controls cannot reuse a single compiled implementation. A historical macOS run on `a827cf2c269` spent 22m17s building 24 controls across 12 binaries and 177s executing them. Measure the same selection and feature sets before attributing that time to this split.
- Reported: 2026-11-18, Codex, while fixing repeated machine-type declarations

### Capture comparisons leave their time scope implicit

- Category: Knowledge crossing a boundary.
- Where: [singleton and containment comparisons](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places/backing.rs#L78), [captured path prefix](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places/backing.rs#L204), [capture coordinates](source-rust/subsystems/compiler/psi-semantics/src/storage/borrowed_storage.rs#L581), [crossed-entry check](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places.rs#L888).
- Saw: A singleton runtime-indexed path compares captured selectors by static block and instruction position. Some callers separately check whether control crossed an entry. The position itself can recur on a later loop entry.
- Cost: A reviewer must establish which execution each comparison covers. A control that retires a binding and changes its index at the same instruction on re-entry would distinguish the two cases. No accepted invalid program has been demonstrated.
- Reported: 2026-11-17, Codex, while checking reference transport through nested-machine loops

### Three more portable filesystem calls mean different things on Windows and POSIX

- Category: One idea, several implementations
- Where: [portable service contract](wiki/spec/build/platform_services.md), [timestamp conversion](source/library/std/targets/windows_x86_64/filesystem/metadata.omg#L120), [portable directory entry](source/library/std/filesystem.omg#L66), [Windows name conversion](source/library/std/targets/windows_x86_64/windows_filesystem_directory_entry.omg#L143), [Windows transfer counts](source/library/std/targets/windows_x86_64/windows_filesystem_transfer.omg#L78), [POSIX adapter](source/library/std/posix_filesystem_adapter.omg#L528), [reviewed differences](tools/ratchets/portable_services.txt).
- Saw: The portable contract promises identical behavior across targets. The current adapters differ in three places.

  - Windows refuses times at or before the 1601 epoch and above 1,833,029,933,770 Unix seconds. POSIX forwards signed seconds to the host, which can still refuse a particular time.
  - Windows refuses a decoded directory entry name longer than 256 UTF-8 bytes. The portable `DirEntry.name` also has a 256-byte capacity. A legal NTFS name can need more space than either provides.
  - A plain read or write with a valid buffer and count above `u32::MAX` is refused on Windows. The same limit applies to positioned reads, positioned writes and copy. POSIX can return a short transfer. Windows `write_all` already chunks its requests.
- Cost: A program using the same portable call encounters different limits. Set a common contract or reconcile the adapters, then remove the corresponding `known` rows. Plain read and write have no such rows today.
- Reported: 2026-11-16, Claude Code, while piloting a cross-target library diff

### The Omega-written compiler binds static arguments by kind instead of position

- Category: One idea, several implementations
- Where: [Omega argument selection](source/subsystems/compiler/source_to_psi/pipeline/symbol_resolved_trees_to_typed_trees/calls.omg#L337), [Rust positional binding](source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/constructor_applications.rs#L1324), [argument position rule](wiki/spec/language/generics.md#L387).
- Saw: Omega’s `bind_static_arguments` partitions actual arguments by kind and takes the next argument of the matching kind for each formal. Rust zips the supplied prefix with the declared formals. In the historical `pick<T, machine K>` example, an actual naming a machine can skip `T` in the Omega binder. The original example uses retired binder syntax.
- Cost: The two compilers give the same call different meanings. Bind positionally, as Rust does, and refuse an argument whose kind does not fit its position.
- Reported: 2026-11-16, Claude Code, while testing a diff-triggered review checklist

<a id="a-build-wide-flag-admits-machine-control-for-every-output"></a>

### Build still validates retired hardware settings

- Category: Zombie machinery
- Where: [authored Build field](source-rust/subsystems/compiler/build/inputs/declarations/prelude/declarations/builder.omg#L42), [Build initialization](source-rust/subsystems/compiler/build/inputs/build_execution/build_value/initial.rs#L54), [return decoder](source-rust/subsystems/compiler/build/inputs/build_execution/build_value/configuration.rs#L108), [missing-field and Boolean controls](source-rust/subsystems/compiler/build/inputs/build_execution/build_value/configuration.rs#L260), [per-output authority](wiki/spec/build/permissions.md#where-the-operation-can-run).
- Saw: The prelude still defines `freestanding` and initializes it to false. Build return decoding requires a Boolean in that field. Its regression refuses a missing field and obtains identical normalized settings for true and false.
- Cost: A Build return can fail for omitting a retired setting that has no effect on its outputs. Remove the field and its decoding requirement while preserving each output’s target and provider authority checks, including the mixed firmware/hosted case.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/configuration.md

### Changing storage analysis rebuilds the native compiler

- Category: Verifying costs more than writing
- Where: [shared semantics crate](source-rust/subsystems/compiler/psi-semantics/Cargo.toml), [optimizer’s declaration lookup](source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit/src/structural_places.rs#L619), [target path lookup](source-rust/subsystems/compiler/psi-to-native/representations/target-operations/target-vocabulary/src/descriptor_input.rs#L21), [target execution facts](source-rust/subsystems/compiler/psi-to-native/representations/target-operations/target-vocabulary/src/execution_facts.rs#L13).
- Saw: The locked workspace graph at `da492310be6` has 37 direct normal dependents of `terminal-semantics`. Target vocabulary uses its structural paths and consumes callable analysis results. The original cache-only edit rebuilt 64 packages in 14m17s.
- Cost: A private storage-analysis change invalidates consumers of small lookup functions in the same crate. Separate the lookup-only edges, keep the full analysis dependency where it is used, and remeasure the original Console edit/build/run loop. The optimizer still depends on storage analysis through the structural-storage and Psi wire crates. Check those imports before splitting the next dependency.
- Reported: 2026-11-16, Codex, while carrying the original Console program through checking and execution

### Omega-written resolution compares integer type arguments by spelling

- Category: Re-deriving
- Where: [machine signature literals](source/subsystems/compiler/source_to_psi/pipeline/syntax_trees_to_symbol_resolved_trees/machine_signatures.omg#L495), [domain literals](source/subsystems/compiler/source_to_psi/pipeline/syntax_trees_to_symbol_resolved_trees/domains.omg#L367), [typed static integers](source/subsystems/compiler/source_to_psi/pipeline/symbol_resolved_trees_to_typed_trees/references.omg#L313).
- Saw: Two resolution queries use `spans_equal` on the source bytes of literal type arguments, so `4` and `0x4` differ there. Each syntax literal already names its decoded integer. Typing uses that value and validity when it constructs a `StaticInteger`.
- Cost: Resolution and typing can disagree about the same array length or type application. Compare the decoded values and preserve the refusal for an invalid literal.
- Reported: 2026-11-16, Claude Code, while moving integer literal decoding into the lexer
