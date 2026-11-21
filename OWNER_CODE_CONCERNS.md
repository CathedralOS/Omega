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

### The portable service check stops before comparing targets

- Category: Tests that pin the bug.
- Where: [selected Windows provider](source/library/std/build.omg#L89), [provider declaration](source/library/std/targets/windows_x86_64/filesystem/provider.omg#L26), [adapter scan](tools/xtask/src/portable_services.rs#L407), [post-merge check](.github/workflows/post-merge-check.yml#L89).
- Saw: `check-portable-services` exits 1 on the qualified selection `targets\windows_x86_64\filesystem\FilesystemWindowsProvider`. The scanner looks for that spelling among local declaration names. The declaration it needs is `FilesystemWindowsProvider` in the selected module.
- Cost: Both the enforcing and observe commands stop before comparing any service rows. Resolve the selected module and declaration together, then exercise two modules with the same local provider name.
- Reported: 2026-11-20, Codex, while checking the cross-target filesystem report.

### A saved-Psi compile request reserves space for parsed source

- Category: Knowledge crossing a boundary.
- Where: [compiler request and Program variants](source-rust/subsystems/compiler/compiler.rs#L23), [source input](source-rust/subsystems/compiler/source-to-psi/source_to_psi.rs#L44), [parsed source](source-rust/subsystems/compiler/source-to-psi/driver/stage_sources.rs#L13), [syntax forest](source-rust/subsystems/compiler/source-to-psi/driver/parse_source_batch/parsed_files.rs#L11).
- Saw: The public `Program::Source` stores `StageSource` inline. On the Linux x86-64 toolchain Clippy reports at least 20,752 bytes for that variant and 840 for the next largest. Runtime and native requests also accept an already authorized Psi artifact through the same enum.
- Cost: Even a saved-Psi request reserves the enum’s largest inline layout. Check the size of both `Program` and `CompileRequest` when moving ownership of the large source value. No runtime slowdown was measured.
- Reported: 2026-11-20, Codex, while checking the source and saved-Psi compiler entries.

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

### Repository rules still scan the retired Rust checkout paths

- Category: Zombie machinery.
- Where: [scan roots and dense-ID catalog](tools/xtask/src/rules/ratchets.rs#L9), [current semantic IDs](source-rust/subsystems/compiler/foundation/semantic-records/src/identity.rs), [stored dense-ID counts](tools/ratchets/dense_id_maps.txt).
- Saw: `mbx xtask check-rules` stops while opening the dense-ID catalog under `omega-rust/`. The code-quality scanners still walk that retired tree and their count files use its paths.
- Cost: Maintainers cannot check new code against the existing limits for dense-ID maps, caches and source-text reads. Migrate the scan roots and transfer the recorded limits by file history before reviewing changes in counts.
- Reported: 2026-11-19, Codex, while restoring repository rules after the compiler move.

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

### Descriptor lifetime queries repeat the same graph walk

- Category: One idea, several implementations
- Where: [entry lifetime transport](source-rust/subsystems/compiler/psi-semantics/src/descriptors/descriptor_lifetimes.rs#L513), [all-arrivals transport](source-rust/subsystems/compiler/psi-semantics/src/descriptors/descriptor_lifetimes.rs#L798), [arrival distinction](source-rust/subsystems/compiler/psi-semantics/src/descriptors/descriptor_lifetimes/resource_tests.rs#L193).
- Saw: Entry and successor lifetime queries compute where each descriptor can survive. The solver computes those same locations internally to seed the all-arrivals authorization check. Local lifetime queries repeat the walk when authorization exists.
- Cost: These paths solve the same graph twice. Share the first result while keeping the distinction between a descriptor that can survive on one arrival and one authorized on every arrival.
- Reported: 2026-11-18, Codex, while investigating the Console binding test timeout.

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

### Native operand failures discard the instruction context

- Category: Verifying costs more than writing
- Where: [operand failure](source-rust/subsystems/compiler/psi-to-native/pipeline/06_selected-instructions-to-selected-instructions/analyses/src/machine_effects.rs#L354), [diagnostic formatting](source-rust/subsystems/compiler/psi-to-native/pipeline/06_selected-instructions-to-selected-instructions/analyses/src/machine_effects.rs#L948), [CLI compilation failure and retention](source-rust/application/src/execution/mod.rs#L203).
- Saw: A selected-instruction operand failure still reports only its function-local instruction ID. A missing callable declaration already reports the machine and constraint. The CLI returns on compilation failure before retaining the stage artifacts.
- Cost: The instruction ID alone cannot identify the function or explain which operand was refused. Include that context at the point of failure. The original stored-machine case required a 175-second compilation.
- Reported: 2026-11-17, Codex, while repairing native calls through stored machines

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
- Saw: The locked workspace graph at `da492310be6` has 37 direct normal dependents of `terminal-semantics`. The optimizer uses a declaration lookup there. Target vocabulary uses its structural paths and consumes callable analysis results. The original cache-only edit rebuilt 64 packages in 14m17s.
- Cost: A private storage-analysis change invalidates consumers of small lookup functions in the same crate. Separate the lookup-only edges, keep the full analysis dependency where it is used, and remeasure the original Console edit/build/run loop.
- Reported: 2026-11-16, Codex, while carrying the original Console program through checking and execution

### Omega-written resolution compares integer type arguments by spelling

- Category: Re-deriving
- Where: [machine signature literals](source/subsystems/compiler/source_to_psi/pipeline/syntax_trees_to_symbol_resolved_trees/machine_signatures.omg#L495), [domain literals](source/subsystems/compiler/source_to_psi/pipeline/syntax_trees_to_symbol_resolved_trees/domains.omg#L367), [typed static integers](source/subsystems/compiler/source_to_psi/pipeline/symbol_resolved_trees_to_typed_trees/references.omg#L313).
- Saw: Two resolution queries use `spans_equal` on the source bytes of literal type arguments, so `4` and `0x4` differ there. Each syntax literal already names its decoded integer. Typing uses that value and validity when it constructs a `StaticInteger`.
- Cost: Resolution and typing can disagree about the same array length or type application. Compare the decoded values and preserve the refusal for an invalid literal.
- Reported: 2026-11-16, Claude Code, while moving integer literal decoding into the lexer
