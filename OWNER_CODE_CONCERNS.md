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

<a id="packed-simd-spill-has-no-home"></a>
### Native register allocation fails on a large machine full of packed results
- Category: 10. Verifying costs more than writing
- Where: [runtime spill](source-rust/subsystems/compiler/psi-to-native/pipeline/07_selected-instructions-to-register-homes/src/allocation/runtime_spill_and_split.rs) after [`home_assignment`](source-rust/subsystems/compiler/psi-to-native/pipeline/07_selected-instructions-to-register-homes/src/allocation/home_assignment.rs) reports `NoCompatibleHome`
- Saw: one `main` that computes 21 SIMD results and then checks all 136 of their lanes before one `transition` fails native compilation with `RegisterAllocation(RuntimeSpill(Homes(NoCompatibleHome { .. })))`; Psi runs it. Any third of the checks compiles, and a scalar program with as many checks compiles. Reproduce it by putting every group of `every_lane_of_the_simd_library_matches_a_scalar_reference_on_psi_and_native` in [`packed_integer.rs`](source-rust/application/tests/packed_integer.rs) into one program.
- Cost: a long SIMD kernel can stop compiling natively once register pressure forces a spill the allocator cannot place, with an error that names no source.
- Reported: 2026-11-26, Claude Code, while checking every SIMD library lane against a scalar reference.

<a id="flattened-asm-regions-drop-their-target"></a>
### Straight-line asm regions of place instructions lose their x86-64 requirement
- Category: 6. A verdict with nothing behind it
- Where: [`desugar` flattening](source-rust/subsystems/compiler/source-to-psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/declarations/assembly/mod.rs#L315) and the [native reachability review](source-rust/subsystems/compiler/psi-to-native/verification/provider-admission/src/terminal_authority_review/reachability.rs)
- Saw: an `asm` region holding only `mov`/`ldr`/`str` or packed instructions over typed places becomes ordinary statements in stage 02, so the machine's `assembly_target_requirement` stays `Any`. The native reachability review now refuses packed integer operations on non-x86-64 targets by operation kind, but `--check --target linux_arm64` still accepts them and a flattened `mov` region keeps no target at all.
- Cost: the catalog's target is enforced for packed instructions only at native admission, by a second rule beside the catalog.
- Reported: 2026-11-26, Claude Code, while moving the SIMD library onto checked asm instructions.

<a id="every-bundled-custody-query-compares-the-whole-file"></a>
### Every bundled-declaration custody query compares the whole source file
- Category: 4. Re-deriving
- Where: [`CoreVocabularyCatalog::classify` and `source`](source-rust/subsystems/compiler/source-to-psi/representations/source-files/bundled-declarations/src/core_vocabulary.rs#L10) and `SealedIntrinsicCatalog::classify`, reached through [`toolchain_site`](source-rust/subsystems/compiler/source-to-psi/representations/symbol-resolved-trees/symbol-table/source_symbols.rs#L63) from `SymbolTable::sealed_intrinsic`, `core_source` and `core_vocabulary`
- Saw: each query strips the file's package root and then tests `crate::bytes(file) == site.source`, a byte comparison of the whole bundled file, every time. Stage 03 asks per operator candidate and per call site: `is_sealed_toolchain_float_operator` makes three such queries for one candidate (two catalog lookups and a second `display_path`), and `retain_static_argument_contexts` asks `core_vocabulary` per argument. A release profile of popcount `--check` on omega-linux (2026-11-25) put about 0.06 s in `__memcmp` and path parsing under `normalize_qualification_casts` and `retain_static_argument_contexts`.
- Cost: whether a file is the exact bundled one is fixed when the source is admitted, but every query recomputes it. Source admission could record it once on the `SourceFile`.
- Reported: 2026-11-25, Claude Code, while profiling Psi 02/03 on popcount.

<a id="resuming-a-typing-from-retained-source-costs-more-than-typing-afresh"></a>
### Resuming a typing from retained source costs more than typing afresh
- Category: 4. Re-deriving
- Where: [`close_typed_cohort`](source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L1881), run once per body completion and per extension, and [`complete_selected`](source-rust/subsystems/compiler/source-to-psi/driver/resolve_and_type.rs#L250), called by [`select_retained_source`](source-rust/subsystems/compiler/source-to-psi/driver/resolve_and_type/activation.rs#L132)
- Saw: each completion or extension round costs about 0.75 s on popcount's 259-file program, whatever its size: completing 5 bodies cost 0.63 s in `close_typed_cohort` alone. The round's passes walk the whole program: `collect_program_operand_owners`, rebuilt separately by `constructor_applications::prepare`, both `construction_destinations::prepare` calls, `equality_preparation::prepare` and `retain_indexed_operand_types` (about 40 ms each); `bind_calls`, twice (about 85 ms each); `retain_static_argument_contexts` (70 ms); `prepare_applied_callable_signatures` (90 ms); and in 02 `assign_body_symbols` and `finish_resolution` (150 ms). Each typing also rescans `source_jobs` 3 or 4 times (100 ms each). `std`'s handoff typing keeps its 1,730 bodies deferred, and `complete_selected` must type them before 03 activation narrows the program. So a resumed Product runs four rounds (those bodies, the extension, its Build bodies, the authored bodies) where a fresh Product typing runs one: `ResolveAndTypeRequest::prepare` resolves all of a Product's bodies before its first typed cohort. A Build recheck that resumes pays for all 1,730 Product bodies it never uses. With one `SourcePreparation` shared per invocation (kept by [`SourceHost`](source-rust/subsystems/package-manager/project/mod.rs#L183) for every `project_build::prepare` call) and the Product's frontends placed in its request by `StageSource::resolve`, popcount's Product resumed `std`'s handoff typing (257 files). Product Psi 02+03 rose from 2.35–2.60 s to 3.48–3.81 s, and Build Psi 03 rose from 0.67–0.74 s to 2.2–2.6 s (Windows release, alternating runs, 2026-11-25).
- Cost: resuming saves only `std`'s declaration work (about 0.65 s) and adds activation (0.4 s) plus at least one extra round, so it cannot beat a fresh typing. The sharing stays unwired until a round costs in proportion to the bodies and files it adds, or deferred bodies survive activation.
- Reported: 2026-11-25, Claude Code, while making retained-source reuse fire for popcount's `std` typings; measured again while wiring it.

<a id="every-machine-check-replays-the-summaries-of-all-its-callees-again"></a>
### Analyses without a shared table replay the summaries of all their callees again
- Category: 4. Re-deriving
- Where: [`Replay::summary`](source-rust/subsystems/compiler/psi/semantics/src/descriptors/replay/engine.rs#L3970), reached from analyses that get no `ModuleSummaries` or `AffineSummaries` table: Psi 06 machine checks ([`lib.rs:1658`](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/lib.rs#L1658)) and the `minted.rs` claim walk where they run without one, and the one-walk entry points `verify_in` and `verify_claims_in`, called from `content_transformations/` and Psi 06 `places/loans.rs:2367`
- Saw: each of these analyses starts an empty replay, so a callee reached from many machines has its summary replayed once per caller. Psi 05 now passes a table to all its per-machine walks: `machine_order::plan_then_apply` shares one for the storage and claim walks over each unchanged module, and `reference_ends::install` shares one from `candidates_sharing` through the walks up to the first machine that gains reference ends. In a release check of `samples/cli/arithmetic/popcount/main.omg` (Windows, 2026-11-25), counting each summary replay by the call site that built its analysis in a temporary binary, total replays fell from 15,406 to 13,511:
  - Psi 05 storage walks went from 1,508 to 505, claim walks from 1,508 to 587, and reference-end walks from 176 to 116;
  - Psi 05's `candidates` still replays 643: it is the first analysis on its table;
  - Psi 06 still replays 1,288 in machine checks without a table (813 with one) and 984 in `minted.rs` walks without one (462 with one). `verdict.rs` now runs `candidates_sharing` on the table its walks use, so the 507 replays of its walks mostly come from that table: in a perf profile on `omega-linux` (2026-11-25), `Replay::summary` under `plan_affine_machines` fell from 0.97% to 0.05% of samples.
  - The machine checks without a table are `entry_inputs::describe_calls`, which edits callee contracts between machines (`require`), and `result_summaries`, which edits the module after every machine. In the same profile, summary replays under `describe_calls` were 0.01% of samples.
- Not shareable this way: `write_frames.rs::derive_write_frames` (4,265) and `membership_facts.rs` (2,072) each run one analysis over the whole module per call, so each callee already replays once per call. Psi 06's scalar and structural qualification checks now share one membership analysis (`BodyMemberships`); write frames still run once in Psi 05 and once in Psi 06, over different modules. Write frames drop scalar premises from their rows, and membership uses reference slots, so neither can use a `ModuleSummaries` table, which serves only unedited borrowed-storage rows.
- Cost: still a large share of checking time for std-using programs. Before #5807, `Replay::summary` was on the stack in 19.6% of all samples of a perf profile of the same check (`omega-linux`, 20,286 samples).
- Reported: 2026-11-25, Claude Code, while profiling Psi 06 places and loans; updated the same day after Psi 05 shared its walks' tables.

<a id="the-checker-walks-each-machines-affine-ownership-three-times"></a>
### The checker walks each machine's affine ownership three times
- Category: 4. Re-deriving
- Where: [`affine_plan`](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/verdict.rs#L1213) runs `MachineAffine::plan_reference_ends` and then `MachineAffine::verify`; [`follow_the_lowered_form`](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/minted.rs#L769) runs `MachineAffine::verify_claims` for the same machine
- Saw: the two `verdict.rs` walks now share one borrowed-storage graph and, with every walk of the module, one table of callee summaries. Each still runs the ownership walk ([`analyze`](source-rust/subsystems/compiler/psi/semantics/src/plain_affine.rs#L1537)) from scratch, in different modes: reference-end candidates change which roots it tracks, and verification adds claims and checks the stored schedule. The `minted.rs` walk runs after `prepare_replay` (`lib.rs:536`) has edited the module in place (`theorems::finalize_citations`), so it cannot reuse the graph or table of the `verdict.rs` walks and builds its own. Before this repair, the three walks were 2.7%, 2.3% and 2.6% of a popcount perf profile.
- Cost: less than the 7.6% measured before; the graph build and most callee replays are no longer repeated, and the remaining cost is not profiled.
- Reported: 2026-11-25, Claude Code, while profiling Psi 06 places and loans.

<a id="a-stated-case-is-decided-twice"></a>
### A stated case is decided by a table beside the case names
- Category: 1. One idea, several implementations
- Where: `source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places.rs` (`stated_cases`, `Storage::entry_cases` and `returned_cases` beside `entry_case_names` and `returned_case_names`)
- Saw: `requires x in S::C` stated as a conjunct is answered by the `stated_cases` table as a structural case, while the same test beside an alternative is a proposition over the boundary's case name (`ValueOrigin::StorageCase`). The conjunct could be the entry fact `name == true` instead, but payload reads (`case_at(..) != Some(CaseAnswer::Holds(true))`) accept only a structural case, so removing the table would refuse payload reads under `requires x in Result::Ok`.
- Cost: two answers to "which case does this formal hold at entry" that must agree; a fix to one does not reach the other.
- Reported: 2026-11-25, Claude Code, while naming a sum's case at storage boundaries.

<a id="the-changed-line-format-check-reports-enums-it-did-not-change"></a>
### The changed-line format check reports enums it did not change
- Category: 10. Verifying costs more than writing
- Where: `tools/xtask/src/fmt/changed.rs` (`--file-lines` ranges passed to the pinned rustfmt 1.10.0-nightly)
- Saw: any range inside `ValueOrigin` (`representations/lowered-program/tables/values.rs`) or stage 06's `Reached` (`places.rs`), even one unchanged line (`--file-lines '[{"file":..,"range":[60,60]}]'`), makes rustfmt propose joining the enum's opening line with its first variant's doc comment (`pub enum ValueOrigin {    /// An invocation formal..`). `mbx xtask fmt --check --base origin/main` therefore fails on every change that adds a variant to such an enum.
- Cost: a check agents are told to run before pushing fails on correctly formatted code, so its other findings are easy to dismiss.
- Reported: 2026-11-25, Claude Code, while adding `ValueOrigin::StorageCase`.

<a id="stage-03-rebuilds-the-whole-program-operand-owner-table-in-each-pass"></a>
### Stage 03 rebuilds the whole-program operand-owner table in each pass
- Category: 4. Re-deriving
- Where: `collect_program_operand_owners` callers in `source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/` (`constructor_applications.rs:25`, `:254`, `:446`; `call_and_product_selections.rs:1788`, `:1964`; `equality_preparation.rs:160`; `expression_meanings.rs:115`)
- Saw: each pass of `close_typed_cohort` (`lowerer.rs`) visits the whole typed program six times for expression and type-reference owners; `omega --check` on popcount (259 std files) builds 48 tables, and every one is queried, so building lazily saves nothing. In each pass the tables built by the second `construction_destinations::prepare`, by `equality_preparation.rs:160` and by `constructor_applications.rs:254` came out identical on popcount, but the passes between them do write fields the walk reads (constructed value types, call targets, constructor application types), and `TypedTrees` keeps plain `Vec` fields with no revision, so nothing cheap proves that one table still answers exactly. Each walk now skips the name lookups whose only result is a discarded symbol and finds alias applications through an index; the rest of its cost is spread across about 100k node visits.
- Cost: about 0.8–1.2 s of owner-table builds per popcount `omega --check` (measured in-process against the former walk, which cost 1.2–1.6 s on the same states). The same table sequence is also built twice for some cohorts, once on each of two threads and once more in sequence, which comes from how Build schedules typing rather than from stage 03. A repair needs the passes to state which ownership they change, or a table that extends from an append frontier.
- Reported: 2026-11-25, Claude Code, while profiling stages 02–04 on popcount; walk cost updated 2026-11-25

<a id="a-build-wide-flag-admits-machine-control-for-every-output"></a>
