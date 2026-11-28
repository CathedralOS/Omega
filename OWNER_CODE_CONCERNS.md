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

### Register allocation obtains its input from the preceding implementation crate
- Category: 2. Knowledge crossing a boundary
- Where: [stage 07 allocation](source-rust/subsystems/compiler/psi-to-native/pipeline/07_selected-instructions-to-register-homes/src/allocation/register_allocation.rs#L39), [stage 06 output](source-rust/subsystems/compiler/psi-to-native/pipeline/06_selected-instructions-to-selected-instructions/src/selected_rewrites/selected_optimization.rs), and [allocation input](source-rust/subsystems/compiler/psi-to-native/pipeline/06_selected-instructions-to-selected-instructions/analyses/src/allocation_input.rs)
- Saw: stage 07 imports stage 06's `SelectedInstructionOptimizationOutput` and extracts allocation input and optional replay evidence. `SelectedAllocationInput` belongs to stage 06's analysis crate and its `into_legality` method can run analysis; it is not just a representation of completed facts. These are production dependencies, not just test or validator dependencies.
- Cost: allocation links producer machinery to receive its input, so the representation directory does not define the full handoff. Publish issued selected-program facts in the owning representation, separate optional replay evidence in the driver, and keep analysis execution in its owning transformation. Moving the analysis crate unchanged into `representations/` would preserve the hidden pipeline. Preserve admission and the allocator's ability to derive legality for its required register availability.
- Reported: 2026-11-27, Codex, while auditing pipeline ownership on `0c2c612084c53`; narrowed after separating the stage 07-to-08 product. No performance measurement.

### Typed-tree storage also orchestrates preparation of whole-program applications
- Category: 2. Knowledge crossing a boundary
- Where: [stage 04 entry](source-rust/subsystems/compiler/source-to-psi/pipeline/04_typed-trees-to-prepared-typed-trees/src/lib.rs#L49), [conformance header preparation](source-rust/subsystems/compiler/source-to-psi/representations/typed-trees/callables/callable_applications.rs#L7714), and [constant settlement](source-rust/subsystems/compiler/source-to-psi/representations/typed-trees/type_records/data_applications.rs#L171)
- Saw: the representation crate contains passes, not just records and local substitution operations. `prepare_conformance_requirement_headers` walks conformances and machines, selects requirements, materializes arguments and creates applications. `settle_const_argument` scans the type table, discovers dependent domains, extents and applications, mutates nodes, discards old views and prepares replacements. Stages 03 and 04 and the compile-time invocation driver invoke preparation through this shared crate. The callable file hides namespace, domain, signature, selection and requirement subsystems as inline modules among its `TypedTrees` implementations; the folder and root entry do not expose that sequence.
- Cost: callers must know which representation methods constitute a pass and which tables they invalidate. This ownership problem is separate from the existing operand-owner-table timing concern. Give preparation an explicit stage-owned request and result, keep reusable substitution primitives below it, and publish completed application records in the representation. Expose those responsibilities in the directory and entry point; merely splitting the large file leaves the hidden preparation path intact.
- Reported: 2026-11-27, Codex, while tracing stage and driver callers on `0c2c612084c53`; shared substitution itself is not the defect.

### Evaluated type arguments return to identifier text before becoming values again
- Category: 4. Re-deriving
- Where: [invocation result settlement](source-rust/subsystems/compiler/source-to-psi/driver/compile_time_invocations/constants/arguments.rs#L421), [type-node replacement](source-rust/subsystems/compiler/source-to-psi/representations/typed-trees/type_records/data_applications.rs#L221), and [literal recovery](source-rust/subsystems/compiler/source-to-psi/representations/typed-trees/callables/callable_applications.rs#L5743)
- Saw: the invocation driver decodes a canonical integer or Boolean, calls `to_string`, and passes that spelling to `settle_const_argument`. Settlement replaces the argument with `TypeReferenceNode::Named` containing an invalid symbol and generated identifier; an array extent parses the spelling as `usize`. `static_const_literal_from_type_reference` later recognizes an invalid-symbol name, parses its sign and radix, and constructs an integer literal. Stage 04 specialization also sends decimal text through this settlement API.
- Cost: a known value becomes a name-shaped node, so consumers recover its meaning and obtain its carrier from surrounding context. Use an explicit settled constant operand carrying the canonical value and carrier, with names reserved for declarations; preserve exactness and range refusals. Coordinate with `EARLY-TERMINAL-JOBS` and `OPERATION-ARITHMETIC-POLICY` in `TASKS.md`: those own additional evaluation and policy behavior; this entry concerns the existing successful result's storage and handoff. No incorrect acceptance or measured speedup is asserted.
- Reported: 2026-11-27, Codex, while following a decoded value into and back out of typed source on `0c2c612084c53`.


<a id="resuming-a-typing-from-retained-source-costs-more-than-typing-afresh"></a>
### Resuming a typing from retained source costs more than typing afresh
- Category: 4. Re-deriving
- Where: [`close_typed_cohort`](source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L2066), run once per body completion and per extension; Psi 02 `finish_resolution` and `assign_body_symbols`, run once per demand round ([`resolve_eager_bodies`](source-rust/subsystems/compiler/source-to-psi/driver/resolve_and_type.rs#L516)); and [`complete_selected`](source-rust/subsystems/compiler/source-to-psi/driver/resolve_and_type.rs#L564), called by [`select_retained_source`](source-rust/subsystems/compiler/source-to-psi/driver/resolve_and_type/activation.rs#L132)
- Saw: every round repeats whole-program passes, whatever it adds. Timers in a temporary release binary on `omega --check` of popcount (Windows, 2026-11-27) counted 35 Psi 02 rounds per check: 11 for each of two Build typings, 4 for `std`'s handoff typing and 3 for the Product, each one layer of the call-graph demand closure. A `std` or Product round costs Psi 02 about 50–130 ms whether it adds 13 bodies or 505:
  - `assign_contract_reference_symbols` re-resolves every machine's contracts (about 14 ms);
  - `selections::finalize` takes 15–20 ms, a third of it in `normalize_closed_conformance_blocks`;
  - `assign_statement_reference_symbols`, `constants::finalize` and `route_named_implementation_calls` take 4–11 ms each.
  
  `finalize_authored_expression_selections` cost 14–21 ms per round, 143 ms per check, because it scanned the whole selection ledger once per candidate. It now reads the ledger's span index (`AuthoredDeclarationSelections::at_source_span`) and costs 34 ms per check. Scoping the contract pass to the completed machines is not safe as written: a contract can name another machine's state, whose symbol appears only when that machine's body completes.
  
  Psi 03 runs four fresh cohorts per check. In each one `retain_static_argument_contexts`, both `construction_destinations::prepare` calls and `prepare_applied_callable_signatures` take 8–22 ms each, and each of the four owner tables about 4 ms. `bind_calls` now returns early, about 5 ms over all four cohorts. `source_jobs` runs 7 times per check at 4–46 ms, 110 ms in all. Across alternating whole-check runs the change stayed inside host noise: best runs fell 133 ms in dependency handoff and 53 ms in resolving sources.
  
  `std`'s handoff typing keeps its 1,730 bodies deferred, and `complete_selected` must type them before 03 activation narrows the program. So a resumed Product runs four rounds (those bodies, the extension, its Build bodies, the authored bodies) where a fresh Product typing runs one cohort: `ResolveAndTypeRequest::prepare` resolves all of a Product's bodies before its first typed cohort. A Build recheck that resumes pays for all 1,730 Product bodies it never uses. With one `SourcePreparation` shared per invocation (kept by [`SourceHost`](source-rust/subsystems/package-manager/project/mod.rs#L183) for every `project_build::prepare` call) and the Product's frontends placed in its request by `StageSource::resolve`, popcount's Product resumed `std`'s handoff typing (257 files). Product Psi 02+03 rose from 2.35–2.60 s to 3.48–3.81 s, and Build Psi 03 rose from 0.67–0.74 s to 2.2–2.6 s (Windows release, alternating runs, 2026-11-25; not measured again).
- Cost: resuming saves only `std`'s declaration work (about 0.65 s) and adds activation (0.4 s) plus at least one extra round, so it cannot beat a fresh typing. The sharing stays unwired until a round costs in proportion to the bodies and files it adds, or deferred bodies survive activation.
- Reported: 2026-11-25, Claude Code, while making retained-source reuse fire for popcount's `std` typings; measured again while wiring it. Updated 2026-11-27, Claude Code, after timing each pass on current `main` and replacing the ledger scans.

<a id="every-machine-check-replays-the-summaries-of-all-its-callees-again"></a>
### Analyses without a shared table replay the summaries of all their callees again
- Category: 4. Re-deriving
- Where: [`Replay::summary`](source-rust/subsystems/compiler/psi/semantics/src/descriptors/replay/engine.rs#L4157), reached from analyses that get no `ModuleSummaries` or `AffineSummaries` table: Psi 06 inference replays through [`with_prepared_machine`](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/lib.rs#L2122), and the one-walk entry points `verify_in` and `verify_claims_in`, called from `content_transformations/`
- Saw: each of these analyses starts an empty replay, so a callee reached from many machines has its summary replayed once per caller. Psi 05 now passes a table to all its per-machine walks: `machine_order::plan_then_apply` shares one for the storage and claim walks over each unchanged module, and `reference_ends::install` shares one from `candidates_sharing` through the walks up to the first machine that gains reference ends. In a release check of `samples/cli/arithmetic/popcount/main.omg` (Windows, 2026-11-25), counting each summary replay by the call site that built its analysis in a temporary binary, total replays fell from 15,406 to 13,511:
  - Psi 05 storage walks went from 1,508 to 505, claim walks from 1,508 to 587, and reference-end walks from 176 to 116;
  - Psi 05's `candidates` still replays 643: it is the first analysis on its table;
  - Psi 06 still replayed 1,288 in machine checks without a table (813 with one) and 984 in `minted.rs` walks without one (462 with one). `verdict.rs` now runs `candidates_sharing` on the table its walks use, so the 507 replays of its walks mostly come from that table: in a perf profile on `omega-linux` (2026-11-25), `Replay::summary` under `plan_affine_machines` fell from 0.97% to 0.05% of samples.
  - The `minted.rs` walks without a table were the claim walks of inference replays. Those replays no longer walk claims (`minted::ClaimRows::Deferred`, 2026-11-27): inference reads no claim row, and the machine check of the settled module reads the claim rows the affine check received over the shared analyses. The new counts were not measured.
  - The machine checks without a table are the inference replays `entry_inputs::describe_calls`, which edits callee contracts between machines (`require`), `result_summaries`, which edits the module after every machine, and `crashes::infer`. Each still builds its machine's borrowed-storage closure and replays its callees alone. In the same profile, summary replays under `describe_calls` were 0.01% of samples.
- Not shareable this way: `write_frames.rs::derive_write_frames` (4,265) and `membership_facts.rs` (2,072) each run one analysis over the whole module per call, so each callee already replays once per call. Psi 06's scalar and structural qualification checks now share one membership analysis (`BodyMemberships`); write frames still run once in Psi 05 and once in Psi 06, over different modules. Write frames drop scalar premises from their rows, and membership uses reference slots, so neither can use a `ModuleSummaries` table, which serves only unedited borrowed-storage rows.
- Cost: still a large share of checking time for std-using programs. Before #5807, `Replay::summary` was on the stack in 19.6% of all samples of a perf profile of the same check (`omega-linux`, 20,286 samples).
- Reported: 2026-11-25, Claude Code, while profiling Psi 06 places and loans; updated the same day after Psi 05 shared its walks' tables.

<a id="stage-03-rebuilds-the-whole-program-operand-owner-table-in-each-pass"></a>
### Stage 03 rebuilds the whole-program operand-owner table in each pass
- Category: 4. Re-deriving
- Where: [cohort typing](source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs) and the [operand-owner walk](source-rust/subsystems/compiler/source-to-psi/representations/typed-trees/expressions/references.rs).
- Saw: each pass that asks which declarations hold an operand needs an answer for the program it reads. Preparing a constructor or cast can add an owner to an existing expression or type. Reusing the constructor pass's table in all later passes returned no owner for 12,784 expression queries per pass on popcount where a fresh walk found one. Such a miss drops the refusal owed by the holding declaration.
- Cost: at `3f94bb48575f`, three optimized dev-profile Windows checks of [popcount](samples/cli/arithmetic/popcount/main.omg) each built and queried 24 whole-program owner tables across four fresh cohorts. The three table-build totals were 62 ms, 73 ms and 7.4 s. The last check spent 186 s overall on the shared host. A continuation visits its selected declarations. To reuse a table across a write, the compiler would have to track which declarations could reach the changed operands.
- Reported: 2026-11-25, Claude Code, while profiling stages 02 to 04 on popcount.
