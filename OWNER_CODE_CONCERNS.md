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

<a id="resuming-a-typing-from-retained-source-costs-more-than-typing-afresh"></a>
### Resuming a typing from retained source costs more than typing afresh
- Category: 4. Re-deriving
- Where: [`select_retained_source`](source-rust/subsystems/compiler/source-to-psi/driver/resolve_and_type/activation.rs#L123) and [`extend_declarations`](source-rust/subsystems/compiler/source-to-psi/driver/resolve_and_type.rs#L338)
- Saw: on `omega --check samples/cli/arithmetic/popcount/main.omg` it resumes none of the five `std` typings. Product requests start with an empty frontend list ([`into_request`](source-rust/subsystems/compiler/source-to-psi/driver/stage_sources.rs#L76)), each package gets a fresh `SourcePreparation` ([`SourceHost::execute`](source-rust/subsystems/package-manager/project/mod.rs#L215)), and the two Build rechecks miss on imports because the Product forest gives `core/extent.omg` an extra `ProductFileAlias` binding. An experiment that shared one preparation per invocation and let popcount's Product resume from `std`'s handoff typing (257 files) did resume, but Product Psi 02+03 rose by about 1 s: `complete_selected` 2.4 s, `activate_sources` 0.6 s, `extend_declarations` 2.1 s to append one file, then `complete_authored` 2.3 s.
- Cost: every Build typing scans its candidates for nothing, and wiring more requests into the mechanism slows them down until activation and extension cost less than a fresh typing.
- Reported: 2026-11-25, Claude Code, while making retained-source reuse fire for popcount's `std` typings.

<a id="every-machine-check-replays-the-summaries-of-all-its-callees-again"></a>
### One-walk affine checks replay the summaries of all their callees again
- Category: 4. Re-deriving
- Where: [`Replay::summary`](source-rust/subsystems/compiler/psi/semantics/src/descriptors/replay/engine.rs#L3970), reached from `plain_affine` walks that get no `AffineSummaries` table: the one-walk entry points [`plan_in`, `plan_reference_ends`, `verify_in`, `plan_claims_in` and `verify_claims_in`](source-rust/subsystems/compiler/psi/semantics/src/plain_affine.rs#L1218), called from Psi 05 (`lib.rs:2189`, `claim_publication.rs:80`, `reference_ends.rs:88`), from `content_transformations/` and from Psi 06 `places/loans.rs:2367`
- Saw: each of these walks starts an empty replay, so a callee reached from many machines has its summary replayed once per caller. These walks now share a table: Psi 06's two `verdict.rs` walks and its `minted.rs` claim walk (`AffineSummaries`), Psi 06 machine checks (`ModuleSummaries`), and Psi 05's per-machine analyses in `default_obligations.rs` and `reference_results.rs::publish_parents`. In a release check of `samples/cli/arithmetic/popcount/main.omg` (Windows, 2026-11-25), counting summary replays by the walk that ran them in one binary with an environment switch for this sharing, total replays fell from 23,038 to 18,197:
  - the `verdict.rs` walks went from 3,016 to 504, and the `minted.rs` walk from 1,508 to 586;
  - Psi 05 `default_obligations.rs` went from 1,420 to 332;
  - the one-walk entry points still replay 5,772: `plan_in` 2,292, `plan_claims_in` 2,292, `verify_claims_in` and `verify_in` 984, and `plan_reference_ends` 204;
  - Psi 06 machine checks still replay 1,775 without a table (entry-input inference, closures with a dynamic call);
  - about 9,600 come from analyses that count did not tell apart.
- Cost: still a large share of checking time for std-using programs. Before #5807, `Replay::summary` was on the stack in 19.6% of all samples of a perf profile of the same check (`omega-linux`, 20,286 samples). Psi 05's `plan_in` and `plan_claims_in` each walk every machine of the module, so passing them one `MachineAffine` per machine and an `AffineSummaries` table, as `verdict.rs` does, is the next step.
- Reported: 2026-11-25, Claude Code, while profiling Psi 06 places and loans.

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
