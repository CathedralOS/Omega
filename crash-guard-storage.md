# crash-guard-storage

## still_open

True

## size

medium

## depends_on

['Nothing blocks the core change: the at_call and at_entry storage bindings already work for `requires` (req_pos/req_neg and ver_pos/ver_neg verified on the current binary)', "The match_join pair also hits a separate stage 05 refusal: 'call argument that names storage another argument lends' (`finish(read_seven(@items, 0, true), @items)`, match_join/main.omg:18). That pair cannot pass until that limit lands", 'The root samples/cli/facts/indexed_crash_guard/main.omg is compiled as one package that includes every subdirectory, invalid_* twins included (the root check reports errors in `call_result::maybe_zero` and `invalid_match_join::main`). The root positive cannot pass while the twins are modules of the root package; that sample layout question is outside this item', 'target_body routes through a boundary trait requirement (Declarer::Requirement, the same Crashes branch). It should follow from steps 1 and 11 through `Callee::Selected`, but nobody has run it']

## evidence

The item is still open on 898d56a20796. I ran the worker-6 binary as `omega --check --target windows_x86_64 <path> < /dev/null`; all logs are in C:/Development/Omega/.agents/worktree/worker-6/build/diag-crashguard/.

1. All 10 positives and the 4 twins I tried (local, field, parameter, call_result, match_join, const_name, other_state, other_machine, target_body; invalid_local, invalid_parameter, invalid_field, invalid_other_machine) stop before the checker with exit 1 and this message (all.log):
   "error: not supported yet: contract clause that projects a formal or result while reading the signature of `maybe_zero` ... main.omg:4:9"
   "error: not supported yet: machine a call of a machine whose signature is refused above (`maybe_zero`) while reading the body of `read_seven`"
   So the twins currently fail for the wrong reason: stage 05 refuses them before they reach the crash coverage check.
2. The same program shape already works when the guard is replaced by a `requires`. This shows the storage mechanism exists for calls:
   - req_pos: `nonzero(items: @[u64;2], index) requires items[index] != 0`, called from `read_seven ... requires items[index] == 7`. Exit 0.
   - req_neg: deletes `requires items[index] == 7`. Exit 1: "cannot prove requires contract for call nonzero from read_seven: items[index] != 0".
   - ver_pos / ver_neg: a local `q: Pair` with `q.b = 0` or `q.a = 0` before `nonzero(@q)` with `requires p.a != 0`. Exit 0 and exit 1 respectively, so the call already sees the exact version of each field.
3. A crash guard over plain values already works:
   - guard_pos: `maybe_zero(value: u64) crashes Trap value == 0`, called as `maybe_zero(items[index])`. Exit 0.
   - guard_neg: "error: call from `read_seven` to `maybe_zero` has an uncovered Trap crash route; publish a same-cause route whose guard covers this invocation. (no proof found ...: the goal is false)".

Code read:
- Stage 05 read/machine.rs:3629-3638 builds an `entry` scope with `formal_fields: false`. machine.rs:3755 reads each crash guard with that scope: `self.clause(id, &entry, ...)`. The `requires` scope (`required`, :3641-3645) sets `formal_fields: true`.
- read/clause_terms.rs:722-723 then refuses any Member or Indexed term: "clause that projects a formal or result".
- read/contracts.rs:72-73 documents the rule: "Crash guards retain their separate admission rule."
- Stage 05 lower.rs:774 and lower/calls.rs:512 and :600 name the storage that clauses read (`name_clause_contents` / `name_call_clause_contents`, `observes_storage`) for requires and ensures only, not crash routes.
- canonical.rs:65 `substituted_routes` and the wire's contract_forms.rs:110 substitute crash continuations with `places = &[]`, so a field term is refused ("clause names storage that is not a structural formal").
- Stage 06 crashes.rs:205-218 restates a call's route through `ScalarActuals`. Its `field` returns "clause term outside the scalar subset" (lib.rs:2235-2245), and its default `structural_read` gives "indexed clause needs a current observation of its selected storage" (psi/semantics/src/substitution/clauses.rs:40).
- crashes.rs:74 uses the machine's own published routes raw, without restating them over its entry storage.
- For comparison, `CallRequirement` (lib.rs:2354-2410) uses `storage.at_call(...)` (places.rs:1641) inside `ErasedTerms`, and machine requires use `storage.at_entry` (lib.rs:1268, places.rs:1624). AtEntry and AtCall already implement `field` and `structural_read` (places.rs:7073-7106, 7840-7890).

Board and drafts: TASKS.md:310 names this item and the 18 programs. wiki/drafts/designs/proof_kernel_core_audit.md:1361-1386 records a trial that found the same three layers. The stage 05 README (lines 725-727, 1054, 1109, 1798-1807) still lists the refusal.

## spec

wiki/spec/terminal-psi/calls_and_outcomes.md#crash: a published bucket "contains a canonical disjunction of predicates over the same lowered values and places as executable Terminal Psi". The same page, under argument substitution, says "Declared requirements and published crash routes describe invocation entry." wiki/spec/terminal-psi/fact_validity.md#entry-facts-and-crash-coverage covers borrowed guards: "borrowed observations still use their appropriate storage revisions". wiki/spec/language/effects.md#guarded-crashes covers composition across calls: "substitute actuals, conjoin current path facts, and discard disproved routes".

## cause

A crash guard is checked over values only, at three points in the pipeline.

1. Stage 05 reads crash guards with a scope that cannot read storage. read/machine.rs:3755 calls `self.clause(id, &entry, ...)` with `entry.formal_fields == false` (machine.rs:3636), so clause_terms.rs:722-723 refuses `items[index]`. Lowering never names the storage leaves a crash guard reads, in the callee (lower.rs:774) or at the call (lower/calls.rs:512, :600). A call's crash continuations are substituted with scalar actuals only (canonical.rs:65, lower/calls.rs:880 and :1100; wire contract_forms.rs:110, checked again at stage 06 minted.rs:685).

2. Stage 06 decides a call's route with `ScalarActuals` (crashes.rs:205-218), which has no storage to read the argument at the call. Requirements use `storage.at_call` for exactly this (lib.rs:2386-2408).

3. Inside the callee, stage 06 places the machine's own published route into the coverage goal raw (crashes.rs:74). It is never restated over the storage the formal held at entry, as `fill_knowledge` does for `requires` (lib.rs:1266-1269).

The spec admits these guards. wiki/spec/terminal-psi/calls_and_outcomes.md, "Crash": a published bucket "contains a canonical disjunction of predicates over the same lowered values and places as executable Terminal Psi". The same page says "Declared requirements and published crash routes describe invocation entry." wiki/spec/terminal-psi/fact_validity.md, "Entry facts and crash coverage", adds that "borrowed observations still use their appropriate storage revisions".

## code_locations

['source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/machine.rs:3629-3645 (entry vs required scope)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/machine.rs:3748-3760 (crash guard read with &entry)', "source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/clause_terms.rs:722-723 (refusal 'clause that projects a formal or result')", 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/contracts.rs:69-74 (formal_fields doc: crash guards excluded)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/lower.rs:774-780 (name_clause_contents: requires and ensures only)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/lower/calls.rs:512, 579-602 (observes_storage, name_call_clause_contents at calls)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/lower/calls.rs:880-885, 1100-1105 (crash continuations substituted with scalars only)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/canonical.rs:65-118 (substituted_routes; duplicate ScalarFormals)', 'source-rust/subsystems/compiler/psi/artifact/wire/src/contract_forms.rs:110-145 (wire substituted_routes, places = &[])', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/crashes.rs:57-100, 179-221 (coverage_goal, call_site with ScalarActuals)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/minted.rs:685-697 (retained continuations equality check)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/lib.rs:2186-2257 (ScalarActuals: no storage), 2354-2410 (CallRequirement via storage.at_call), 1266-1269 (requires via at_entry)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places.rs:1624-1690 (at_entry, at_call), 7054-7106 (AtEntry field/structural_read), 7840-7890 (AtCall field/structural_read)', 'source-rust/subsystems/compiler/psi/semantics/src/substitution/clauses.rs:39-41 (default structural_read refusal)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/README.md:725-727, 1054, 1109, 1798-1807 (stale refusal rows)', 'TASKS.md:310']

## repair_plan

The general mechanism: a crash guard is an entry clause, exactly like a `requires`. It is read with the same scope, the same storage leaves are named in lowering, and the checker restates it through the same bindings `requires` uses (`AtEntry` in the callee, `AtCall` at each call). Facts and loans then follow each field path at its exact version through the existing `held_at` / write log (AGENTS rule 3). No new data structure is needed.

Stage 05, reading the signature:
1. read/machine.rs:3755: read each crash guard with `&required` instead of `&entry` (formal_fields true; result stays Absent, so `result` in a guard is still refused).
2. Delete the `entry` binding if it is then only a base for `required` and `exit`.
3. Rewrite the contracts.rs:72-73 comment: guards read invocation-entry storage like requires.

Stage 05, lowering:
4. lower.rs:774: also call `name_clause_contents` for every predicate in the machine's declared crash routes (`signature.declared_crash_routes()`). Without this, `AtEntry::structural_read` fails with "no operation captures this indexed input at entry" when the body never reads that element itself, as in other_machine's `forward`.
5. lower/calls.rs:512: make `observes_storage` also true when the callee's published routes are non-empty.
6. lower/calls.rs:600: chain the callee's published route predicates (the `outcome.crash_routes` already in hand) into `name_call_clause_contents`.

Stage 05 and the wire, crash continuations:
7. Substitute structural formals as well as scalars. Change terminal_codec `contract_forms::substituted_routes` (psi/artifact/wire/src/contract_forms.rs:110) to take the call's structural arguments and map storage formal k to argument k's place.
   - When an argument has a non-empty path, prefix that path to the term's path.
   - When the path cannot be stated as a term, refuse with a named reason. Never drop the route.
8. Make stage 05 `canonical::substituted_routes` (canonical.rs:65) delegate to the wire version instead of keeping its own `ScalarFormals` copy (rule 6).
9. Pass the arguments at lower/calls.rs:880 and :1100, and at stage 06 minted.rs:685 (`inputs.structural_arguments`), so the retained continuations still equal the substituted contract.

Stage 06, crashes.rs:
10. Give `coverage_goal` the `(storage, references)` pair that `stated_goal` already holds (lib.rs:2448).
11. In `call_site`, always restate the callee's published `call.contract.crash_routes` for the cause through `storage.at_call(references, tables, &row.actuals, call.structural_inputs(), site, concerns)`, wrapped in `ErasedTerms` exactly as `CallRequirement` does (lib.rs:2362-2408).
    - Use plain `crate::substitute`, never `substitute_goal`. The route is the premise of an implication, and dropping a disjunct there would be unsound. An unread storage alternative must refuse.
    - The `crash_continuations` copy stays a retained, minted-checked row; it is no longer the input to judgment.
    - Delete the `ScalarActuals` route branch.
12. Restate `coverage(&lowered.contract.crash_routes, cause)` through `storage.at_entry(references, tables)` with `substitute_goal`. Coverage appears only in goal positions (conjunct, conclusion or disjunct of the goal), so an unstated disjunct may count as false there.

Documentation and board, in the same PR:
13. Update the stage 05 README rows at 725-727, 1054, 1109 and item 27 (1798-1807) so a crash guard over a formal's field or element is no longer listed as refused. `ensures` and `result` projections keep their own rows.
14. Delete the crash-guard clause from TASKS.md:310.
15. Fix the indexed_crash_guard README: it cites a non-existent `private_omission/` and twin paths `<place>/invalid/main.omg`, but the files are `invalid_<place>/main.omg`. Do not add or remove `.omg` files under samples/cli/facts/.

Never weaken anything: an unbound storage term at a call keeps refusing the call.

## twin_programs

Primary pair (already in the repo, differs by one line):
- Positive: samples/cli/facts/indexed_crash_guard/parameter/main.omg.
  `maybe_zero(items: @[u64; 2], index: u64) -> u64 requires index < 2 crashes Trap items[index] == 0 { transition items[index] == 0 { true -> bad(), false -> (items[index]) } ... }`
  `public machine read_seven(items: @[u64; 2], index: u64) -> u64 requires index < 2 requires items[index] == 7 { maybe_zero(items, index) }`
  Expected: `omega --check` exit 0; `omega run --both` exit 0 with matching stdout and stderr bytes.
- Negative twin: samples/cli/facts/indexed_crash_guard/invalid_parameter/main.omg. It deletes only `requires items[index] == 7`. Expected refusal, from stage 06 rather than stage 05:
  "error: call from `read_seven` to `maybe_zero` has an uncovered Trap crash route; publish a same-cause route whose guard covers this invocation"
  Its sample.json already pins "call from `read_seven` to `maybe_zero`" and "uncovered Trap crash route".

Exact-version pair (one input differs; pins per-field tracking, AGENTS rule 3):
`public data Pair { a: u64; b: u64; }`
`machine maybe_zero(p: @Pair) -> u64 crashes Trap p.a == 0 { transition p.a == 0 { true -> bad(), false -> (p.a) } machine bad() -> u64 { crash Trap; } }`
`public machine run() -> u64 { let mutable q: Pair = Pair { a: 7, b: 11 }; q.b = 0; maybe_zero(@q) }`
- Positive: as written, with `q.b = 0` (a write to the sibling field). Must be accepted.
- Negative twin: `q.a = 0`. Must be refused with "call from `run` to `maybe_zero` has an uncovered Trap crash route; publish a same-cause route whose guard covers this invocation".

The `requires` analog of this pair already gives exactly these verdicts on the current binary (build/diag-crashguard/ver_pos and ver_neg).

## tests

Add accepted/rejected pairs as inline-source tests in source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/indexed_and_member_contracts.rs, which already pairs indexed and member clauses with one-input twins:
(a) The parameter pair above: a dynamic-index guard on a borrowed array, discharged by the caller's `requires items[index] == 7`.
(b) The Pair field-version pair: `q.b = 0` accepted, `q.a = 0` refused.
(c) A forwarder pair (other_machine shape). The middle machine publishes `crashes Trap items[index] == 0`, never reads the element itself, and forwards to `maybe_zero`. It must be accepted. Its twin drops the forwarder's guard and must be refused with "call from `forward` to `maybe_zero` has an uncovered Trap crash route". This pins step 4 (naming the entry storage leaves) and the callee-side restatement through at_entry.

In stage 05, add a unit test beside read.rs:2726-2758: a crash guard reading `items[index]` produces a StructuralRead term, and a guard naming `result` is still refused.

Do not add `.omg` files under samples/cli/facts/.

Run:
- `mbx test -p <stage-06 crate> --test indexed_and_member_contracts < /dev/null`
- the stage 05 and stage 06 lib tests whose names contain `crash` (for example crashes_* and a_requirement_the_guard_does_not_contradict_leaves_the_crash_site_uncovered)
- the wire crate's contract_forms tests
- `mbx clippy -p` on the three touched crates with `--all-targets -- -D warnings`

Then check with the built omega: parameter, local, field, call_result, const_name, other_state, other_machine and target_body each give `--check` exit 0, and each matching invalid_* twin gives the uncovered-crash-route text. Also run `omega run --both` on parameter/main.omg and compare the captured bytes explicitly; BUILD-TEST-COMPARISON has not landed.

## risks

- **Soundness:** restate the call's route as a premise with `substitute`, not `substitute_goal`. Dropping an unread disjunct of the route would make the implication easier to prove than the contract allows.
- **Unnamed storage leaves:** if step 4 or step 6 is missed, `AtEntry::structural_read` or `AtCall` returns Unproved. The program then fails closed (refused), not unsoundly accepted, but the positives will not pass.
- **Projected structural arguments:** an argument with a non-empty path needs its prefix carried into the continuation term. Where it cannot be stated, refuse by name.
- **Duplicate substitution code:** the stage 05 `canonical::substituted_routes` and the wire's copy must change together. Folding them into one removes the risk.
- **Other refusals in the sample set:** match_join (the 'names storage another argument lends' refusal) and the root package (which includes the invalid_* modules) will still be refused for causes outside this item, so expect 14 of the 18 programs to change verdict, not all 18.
- **Unmeasured cost:** each call with storage-guarded routes adds AtCall lookups and named leaves. I did not measure it.
- **Untested routes:** boundary or selected callees (target_body) take the `Callee::Selected` path, which no test has exercised yet.

