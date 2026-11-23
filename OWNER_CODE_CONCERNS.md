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

### A pattern's field reads get their fields in typing and its roster gets them again in preparation

- Category: One idea, several implementations
- Where: [typing's rows](source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/data_patterns.rs), [typing's member selection](source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs), [preparation's pass](source-rust/subsystems/compiler/source-to-psi/pipeline/04_typed-trees-to-prepared-typed-trees/passes/record_patterns.rs#L62)
- Saw: For `let { a, b as _ } = pair;` typing selects the field of each read `pair.a` from the receiver's declared type, and stage 04 selects the field of each roster row by looking the row's name up in the record it finds for the subject. A positional pattern now gets its rows' names and symbols in typing, and stage 04 then overwrites those symbols by name. The two lookups use different routes to the record (`declared_type` on resolved trees, `closed_data_definition` on typed trees) and agree only because both end at a name.
- Cost: A read and its roster row can select different fields and nothing compares them: lowering checks coverage on the roster and reads through the member. No program is known to separate them. Typing already has the subject's record for every pattern it places, so it can write every row's symbol and the marker's subject, and the stage 04 pass can go.
- Reported: 2026-11-22, Claude Code, while adding positional and headed data patterns

### Typing lowers a local's member initializer twice and leaves the first copy in the expression table

- Category: Re-deriving
- Where: [the first lowering](source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/statement_lowering.rs#L472), [the test support that had to work around it](source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/tests/support/positional_fields.rs)
- Saw: `infer_initializer_type` lowers an initializer that is a member, only to ask whether it is a collection's length, and `lower_local` then lowers the same initializer again for the local. Both copies stay in the typed expression table; no statement reaches the first. `let (alpha, _) = pair;` therefore leaves two identical reads of `pair.zeta`.
- Cost: Every `let x = a.b;` stores its read twice. A test or tool that scans all typed expressions sees reads no machine executes, and changing the first copy changes nothing, which reads as a consumer ignoring its input. Ask the selection question on the resolved member, or reuse the lowered handle for the local.
- Reported: 2026-11-22, Claude Code, while testing which field typing gives each place of a positional pattern

### Clippy with `-D warnings` cannot lint an application test because four crates it builds carry warnings

- Category: Tests that pin the bug
- Where: [package manager](source-rust/subsystems/package-manager/review/restricted_build_grants.rs), [review evidence](source-rust/subsystems/package-manager/review/evidence/src/encoding/encode/selected_providers/signature.rs#L4), [target-bound program](source-rust/subsystems/compiler/psi-to-native/representations/target-bound-program/scalar_demand.rs), [target operations](source-rust/subsystems/compiler/psi-to-native/pipeline/04_abstract-operations-to-target-operations/src/preparation/legalization_scalar_graph.rs).
- Saw: `mbx clippy -p omega --test computed_array_extent -- -D warnings` stops in the application's dependencies: `review-evidence` (an unused import), `target-bound-program` (two needless borrows), `abstract-operations-to-target-operations` (an unused import) and `package-manager` (ten warnings: one unused import and nine about functions, methods or structs that nothing uses). The same command without `-D warnings` finishes and lists 14 warnings in those crates.
- Cost: Every test under `source-rust/application/tests/` builds those crates, so the required lint cannot pass for any of them whatever the test contains; the check is skipped or run without `-D warnings` and a new warning in the application or its tests goes unseen. Remove the three unused imports and the two unneeded `&`, and delete or wire up the unused package-manager code.
- Reported: 2026-11-22, Claude Code, while adding an application test for an array sized by a computed constant

### A wrong case in a dispatch is reported as a write-frame mismatch in the first machine

- Category: A verdict with nothing behind it
- Where: [write-frame check](source-rust/subsystems/compiler/psi/semantics/src/storage/borrowed_storage.rs#L36), [its message](source-rust/subsystems/compiler/psi/semantics/src/storage/borrowed_storage.rs#L126), [the checker's use of it](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/lib.rs#L503), [the checker's rule for a roster](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places.rs#L4989), [the test that pins it](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/variant_parent_identity.rs).
- Saw: The lowered-form checker has refusals that name a dispatch edge selecting a case of the wrong sum: "a dispatch lists a case of another sum" and "a case is not of the sum it is applied to". A machine that publishes a write frame never reaches them, and all three matching machines in the test program publish one.

  - `validate_write_frames` runs first. The storage-flow reader cannot read the malformed body and says why ("descriptor arrival case payload fields have no exact scalar types"). The write-frame check drops that reason, has no frame to compare, and reports "published write frame for MachineId(2) differs from its body".
  - The checker places every write-frame refusal at the module's first machine. A wrong edge in `other` is reported "in machine `choose`".
- Cost: The program is still refused, so nothing unsound is accepted. A person debugging a lowering fault is sent to the write-frame code and to the wrong machine, and this applies to any body the storage-flow reader cannot read. Return the machine and the reader's reason from `validate_write_frames`, and use them for the refusal's site and text. Then change `a_write_frame_refusal_is_reported_at_the_first_machine` to expect `other`.
- Reported: 2026-11-22, Claude Code, while testing what lowering hands the checker about a case's sum

### Boolean patterns keep a copy of the constant they match

- Category: One idea, several implementations
- Where: [the pattern test that pins it](source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/tests/prepared_boolean_patterns.rs#L257).
- Saw: A pattern such as `east\ON` reaches lowering as a Boolean literal copied into the use, plus a selection naming the declaration. The declaration carries the same value again as `evaluated_value` and its materialized initializer. Name resolution writes the copy for a literal initializer and the source driver's evaluation writes it for a computed one. Lowering reads only the copy at the use. `a_use_takes_no_value_from_its_declaration` changes the declaration consistently, leaves the use alone, and lowering accepts the product and compares with the old value.
- Cost: The two copies agree only because two separate writers make them agree. A change to either writer can make a use disagree with the constant it names, and no stage from lowering on would refuse it. No program is known to separate them today. Keep one value, on the declaration, and have lowering read it through the use's selection; then delete the copy and update that test.
- Reported: 2026-11-22, Claude Code, while porting V2's prepared-constant handoff test to V1 lowering

### Publication still links earlier source stages instead of reading their products

- Category: Knowledge crossing a boundary
- Where: [retained links](tools/xtask/src/boundary/linear_pipeline.rs), [source correspondence](source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/publication.rs), [checked publication](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/publication.rs), [publication's accepted input](source-rust/subsystems/compiler/source-to-psi/pipeline/07_checked-program-to-psi/src/accepted.rs).
- Saw: `check-boundary` refuses a production link from one numbered source stage to another, with two retained links from publication (07) to lowering (05) and the checker (06). Publication reads their source correspondence and checked-closure types, including deferred advisory state. The source driver already runs lowering and judgment; the calls inside publication's `judge_fixture` are test-only.
- Cost: Publication can still call its producers' implementation instead of reading independent products. Move the remaining source correspondence and checked publication data under `source-to-psi/representations/`, with checked construction at their owners, then delete the two remaining pairs from `RETAINED_SOURCE_STAGE_LINKS`. Tests that construct source through parsing, resolution and typing belong at the source driver.
- Reported: 2026-11-22, Claude Code, while tightening the source stage dependency rule

<a id="a-build-wide-flag-admits-machine-control-for-every-output"></a>
