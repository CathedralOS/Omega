# worker-12 handoff, LOWERED-FORM-CHECKER: generics, then traits and conformances

## Done
- PR #2797 (merged, `cd9fe4c29`): a generic body that calls another generic machine is judged.
- Item 0, soundness: `psi: refuse a generic machine that calls itself instead of accepting it unjudged`
  (branch `work/psi-generic-call-cycle-refused`; PR number and merge commit are in the handback).
  `judge_generic_forms` in `lowered-discharge/src/verdict.rs` now gives every generic template that
  reaches itself (`reaches_itself`) the refusal `generic machine <name> on a call cycle`, instead of
  judging its form. Test: `a_generic_body_that_applies_another_generic_machine_is_judged` in
  `compiler/tests/lowered_route_subjects.rs` (three new staged programs).
  Observed on the unpatched checker: `machine forever<T>(item: T, limit: u32) -> u32 terminates
  { forever<T>(item, limit) }` was `Accepted(2)`.
  Limit: the refusal is whole; a generic tail-recursive loop the spec allows is refused too.

## Measured
Nothing new (no census, by instruction). One finding from the reproduction: on the unpatched checker
`forever<T> ... { forever<T>(item, limit) + 1 }` was Rejected (the `+ 1` overflows), so a cycle
reproduction must not add arithmetic.

## Item 1 (traits and conformances): NOT started in code. Design read out of the source, ready to write

Facts established by reading (not by running):
- An inline member of a `satisfies` block is an ordinary machine in `typed.machines()` with an EMPTY
  `machine.satisfies` (stage 02 `lowering.rs` ~line 2201 copies only the member's own authored edge),
  so `machine_shape` does not refuse it. Its row is found with
  `typed.owned_conformance_realization(symbol)`; all rows are `Conformance::rows`
  (`declaring_trait`, `requirement`, `realization_machine`, `source: Inline|Reference|TraitDefault|Synthesized`).
- A closed specialization (`advance<Hold>`) calls the selected member directly with the member's own
  contract (`read/calls.rs`, requirement dispatch, `self.call(.., target, ..)`), and is judged as a
  closed body. So the closure never relies on refinement; refinement is a validity check of the
  conformance declaration, and a generic form's requirement rows can be skipped soundly.
- `lowered_route.rs` (`RouteJudgment::all`) takes any number of parts from `judge_compilation`.
- The old messages to keep (pinned by `fail/traits/member_omits_requirement_ensures`,
  `member_requires_beyond_requirement`, `selection_row_contract_not_refined`,
  `crashes_none_requirement_member_crashes`):
  "machine `M` realizes `Trait::req` but does not guarantee its `ensures C`" and
  "machine `M` realizes `Trait::req` but requires `C`, which the requirement does not imply"
  (`validation/src/contract_entailment.rs` ~15658, frozen). `M` is typed `Machine.name`.

Exact next step, one PR:
1. `structure-elaboration/src/read/subjects.rs`: drop `trait` and `conformance` from the `unjudged`
   table; call two new methods (new file `read/conformances.rs`, additive):
   - `subject_trait`: refuse (`Unserved::Declaration`) a boundary trait, any type/lifetime parameter,
     conformance bound, parent (`requires`), `refines`, `refinement_clauses`; then read each
     requirement in a scratch `Program::new(typed, false)` with `witnessed_signature(sig, &[])` so an
     unserved requirement is refused by name.
   - `subject_conformance`: refuse parameters, trait arguments, `TraitDefault`/`Synthesized` rows,
     rows whose `declaring_trait` is not the conformance's trait; call `self.machine(row.realization_machine)`
     so the member body is a judged machine of the closure; push
     `RequirementBinding { declaring_trait, requirement, implementation }` onto a new
     `Program::requirement_bindings`, copied to a new `Elaboration::requirement_bindings` in
     `finish_reading` (like `generic_templates`).
2. `structure-elaboration/src/lib.rs`: `pub fn elaborate_binding_witness(typed, &RequirementBinding)`:
   `program.witnessed(trait, requirement, &[])`, then
   `program.callable_contract(CallableDeclaration::Machine(implementation))` (contract only, no body),
   then `elaborate_witness_call`. Refuse a requirement whose `StateSignature::termination_guarantee`
   is not `NoGuarantee` (`requirement_signature` drops it today: `Termination::Silent`), and make
   `elaborate_witness_call` also compare `borrowed` and `erased`. Set the entry's
   `names.machine` to `Trait::req`.
3. `lowered-discharge/src/verdict.rs`: in `judge_compilation` take `requirement_bindings` from the
   closure and push one verdict per binding:
   `judge_owing(elaborate_binding_witness(..), AppliedContracts::OwnCheckForms)`. Change the skip in
   `judge_owing` from `elaboration.applications.contains(&machine)` to
   `elaboration.requirements.contains(&machine)` (requirements include applications), which also
   leaves a generic form's requirement rows (through an implementation parameter) to the conformance
   witnesses; rename the variant to say so.
4. `lowered-discharge/src/describe.rs`: when the site is
   `SourceOrigin::Unpositioned(Unpositioned::ConformanceWitness)`, state `CallRequirement`,
   `NormalGuarantee`/`PublishedGuarantee` and `CrashCoverage` in the old messages' words above.
5. Tests: `judged_parts` in `compiler/tests/lowered_route_subjects.rs` with
   `trait Step { machine apply(value: u32) -> u32 requires value <= 10 ensures result <= 20 crashes none; }`
   + `Hold satisfies Step { ... ensures result <= 10 ... }` (accepted) and a member with
   `ensures result <= 30` / `requires value <= 5` (rejected); update
   `each_kind_of_unused_subject_declaration_is_judged_or_handed_back` (`declaration trait` row).
   Probes: `run/generics/implementation_parameter_contract`,
   `fail/generics/implementation_parameter_contract_only_holds_for_selection`.
6. Later PRs, one each: generic traits (`trait Shrink<T>` with `Grow satisfies Shrink<Scale>`: bind `T`
   in `Program::scope` while the requirement is read), requirements that promise `terminates`
   (a member inherits the promise: termination.md, "An implementation satisfying a requirement
   inherits its guarantee"), parent traits, declared edges (`machine m() satisfies T::r`,
   `machine.satisfies` non-empty, refused in `machine_shape`), the open-const family.

Not decided by the spec text I read, check before relying on it: whether a member that omits
`requires`/`ensures` inherits the requirement's (the old checker did not: see
`member_omits_requirement_ensures`).

## Open PRs from other sessions to stay clear of
#2777 (`lowered-discharge` `lib.rs`, `prove*`, `joins.rs`, `entry_inputs.rs`, `result_summaries.rs`).
#2765 has landed (`0682e01d9`) and changed `judge_owing` in `verdict.rs`.

## Slot state
`C:\Projects\Omega\.agents\worktree\worker-12`: clean at the item 0 PR head, nothing running.
