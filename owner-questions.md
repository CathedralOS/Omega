# owner-questions

## still_open

True

## size

medium

## depends_on

['Item 1 (boundary/service fence) depends on nothing; it uses the existing service-reach closure in psi/semantics/src/synchronous_calls/service_reach.rs', 'Relaxing the fence for `reaches none` boundary calls and deciding whether `blocks` alone fences: owner ruling B', 'Level binder spelling: owner ruling A; blocks representation_agreement only', 'Quotient evidence migration: TASKS.md PROOF-KERNEL-CORE step 7 (spec ruling #5704 already landed)', 'Floor arm-count repair: none; spec contracts.md#required-automatic-discharge decides it']

## evidence

Worktree C:/Development/Omega/.agents/worktree/worker-6 is at 898d56a20796 (2026-11-24 22:01). Scratch files and logs are in build/diag-owner-questions/.

**A. The quotient question is answered.**
- PR #5704 (fc824bd9c7d2, "docs: select all four quotient laws through one conformance", merged 2026-11-24) removed OWNER_QUESTIONS.md entry 2, "Where does a quotient declaration select the proof that its relation is a mere proposition?" (anchor `quotient-mere-proposition-evidence`).
- The ruling is in wiki/spec/proofs/quotients.md:220: "A quotient declaration selects one closed `MereEquivalence<C, R>` conformance." Lines 237-247 define `IsMere`, `MereRelation` and `MereEquivalence`. Lines 266-272 fix the checker's behavior:
  - accept `proofs_agree(a, b)` as evidence for the relation;
  - reject a conformance proved for a definitionally unequal relation;
  - erasure supplies no proof of the law;
  - effectivity still needs its admission or a checked derivation.
- OWNER_QUESTIONS.md at HEAD has only three questions, none about proofs: build network and subprocess grants, Build library bindings, and release-tool entry reach. OWNER_QUESTIONS_BOOTSTRAP.md says "None currently recorded."
- The implementation is not done. `grep -rl 'MereEquivalence|MereRelation|IsMere'` over source/, source-rust/ and samples/ finds nothing. source/library/core/relation.omg:44 still declares `public trait Equivalence<C, proposition Relation>`. TASKS.md:312 (PROOF-KERNEL-CORE step 7) still lists "implement quotient evidence selection".

**B. The audit's list of seven is now six.** The list is in wiki/drafts/designs/proof_kernel_core_audit.md:1630-1676, "Recorded, not answered". At 2b6e425ebf68 it had seven entries; #5663 (61130d2a4bda) removed "One mechanism for Identity over data". Status of each:
- **Identity over data (old #2).** Answered by #5663: contracts.md:62-66, "Record identity from field identities", requires the checker to derive `core\Identity::<R>(a, b)` from field identity evidence.
- **#1 Level binder spelling.** Open, and a real spec inconsistency.
  - modules.md:127-143 (`role-separated-builtin-type-names`) reserves `u` and lists the introduced names that may use a builtin spelling. A `core\Level` binder is not in that table.
  - mathematical_bindings.md:27, 67, 214, 242, 249 and 274 still write `u: core\Level` and `core\Type<u>`. The newer quotients.md:237 sidesteps the problem by writing `v`.
  - It is not on OWNER_QUESTIONS.md.
- **#2 Where the signature lives.** Not an owner question. OWNER_QUESTIONS.md:19-22 says "Psi representation and encoding design is delegated ... a schema change needed for accepted behavior are implementation work". The owner's 2026-11-23 ruling that judged laws are not published (audit:1656-1658) settles the law-row part.
- **#3 Floor scope.** The spec decides it (contracts.md:834-905).
  - "An operand's bounds are the tightest literal bounds order reasoning gives it" (:888), and order reasoning includes equality substitution (:879-882). So `prefix` proved equal to 360 has bounds [360,360], and `(prefix - 360) + last` receives `last`'s bounds. A literal-bound goal over that sum is in the floor. Stage 06 README:1267-1276 records 15 programs rejected for this.
  - The 64 limit counts "arm combinations per obligation" (:896), and only for disjunctions that "share a subject with the goal" (:894). prove.rs:83 has `CASE_BRANCH_BUDGET = 64`. prove.rs:1803 spends one unit for every alternative visited, including alternatives of failed cuts. prove.rs:1746-1754 tries every disjunctive fact.
- **#4 Float ranges.** The spec decides it: no propagation is required. The floor covers integer terms only (contracts.md:874-888, "compares two integer terms"; "Each integer value defined by ..."). Contracts.md:841 says "A checker may discharge more; it may not discharge less". A grep of wiki/spec for float interval or range propagation finds no requirement.
- **#5 A `requires` that can never hold.** Mostly decided.
  - Soundness is not affected: "contradiction of an unreachable path" is in the floor (contracts.md:882), and every caller must still prove the impossible `requires`.
  - state_contracts.md:296-306 already requires an advisory warning for an authored branch that contradicts the entry contract and says the warning is "not an inherent language error".
  - Only whether a whole-machine warning is wanted remains, and it is a diagnostic choice.
- **#6 Boundary calls while storage is vacant.** Open. This turn reproduced it with the existing binary (target/debug/omega.exe). Each probe below is `omega --check --target windows_x86_64 build/diag-owner-questions/<v>/build.omg < /dev/null`. In every probe, `machine refill(h: @mutable Holder)` runs `let t: Token = h.token;`, then the probe call, then `h.token = t;`.
  - Controls:
    - `no_restore`: exit 1, "unestablished storage: an owned value was moved out of borrowed storage, and the place does not hold a value again where the machine returns the borrow ..." at main.omg:21:9.
    - `lend_holder` (`Probe::look(h)`): exit 1, "owned storage is used after its value was transferred or consumed" at 25:12.
    - `boundary_suspends`: exit 1, "not supported yet: a possibly suspending call while an owned value is out of borrowed storage" at 23:12.
  - Accepted (exit 0, "compiled 154 source file(s)"):
    - `boundary_none`: `Probe::ping(1u64)` where ping is declared `reaches none crashes none`;
    - `boundary_service`: ping declared `reaches Clock`, with a body that calls `Clock::tick()`;
    - `boundary_blocks`: `block Probe::ping(1u64)` where ping is declared `reaches none blocks crashes none`;
    - `helper_service`: an ordinary `machine helper() reaches Clock { Clock::tick(); }` called inside the window;
    - `lend_sibling`: `Probe::count(@mutable h.n)`.
  - The code has one fence: plain_affine.rs:2490-2517 refuses only when `operation.suspension_crossing.is_some()`. The same crate already computes service reach (psi/semantics/src/synchronous_calls/service_reach.rs:10-60 reads `declared_service_reach`, `PortWrite`, `RegisterAccess` and boundary `published_service_ceiling`). `OperationKind::BoundaryCall` is at psi/machines/operation.rs:451.
  - Spec text that applies:
    - ownership.md:268-272: calls "may consume the detached value or use disjoint storage only when their complete access and outcome contracts cannot expose the incomplete owner. Unknown access is not evidence of non-observation. Boundary/capability calls and other invariant-window fences remain in force."
    - dependent_values.md:283-292 fences "Boundary/capability-carrying calls that may observe or expose the affected storage, including through authority not named as an argument". It says "Include argument evaluation, callbacks and authorized capability state" and "Unknown access/effects are treated conservatively".
    - effects.md:3-6 and 12-16 make reach, `invokes` and `blocks` separate axes and hold authority in values. Neither page lists blocking itself as a fence.
  - The window rule was owner-ratified on 2026-09-17 (bc801f80010e). Nothing on OWNER_QUESTIONS.md, TASKS.md or any merged PR since 2026-11-20 rules on it; the 1,200 merged titles were read in build/diag-owner-questions/prs.txt and prs2.txt.

**C. The failing test is identified and was already repaired.**
- PR #5348's Proof line says that after merging main, the stage 06 suite had one failure: `checking::tests::byte_view_elements::a_declared_qualified_byte_range_is_not_taken_as_builtin`. It stopped at "typing borrow without a reference destination" and was "not confirmed on main alone".
- PR #5361 (f76875c4106c, merged 2026-11-23 16:22, 23 minutes after #5348) confirmed it on main. Its body says "The unchanged test passed at 8e8ec8b60d7e and failed at its immediate successor, 681058a1c347 (#5323), and on main at 7dc7bb55b8a7". It repaired the test: the original program is kept and now pins that exact borrow diagnostic, and a twin without the outer `@` must check and call `alternative`.
- At HEAD, source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/tests/byte_view_elements.rs:756-794 carries that assertion. Later changes were #5484 (verdict helper only) and #5653 (crate move).
- It is a Rust unit test, so per the job rules it was not run. An `omega --check` version of its program (bytes_borrowed, bytes_direct) stops earlier, at the unimported `byte_observation` name, so it proves nothing either way.

## spec

wiki/spec/language/ownership.md#borrowed-storage-invariant-windows: "Unknown access is not evidence of non-observation. Boundary/capability calls and other invariant-window fences remain in force." wiki/spec/language/dependent_values.md#invariant-windows: fences "Boundary/capability-carrying calls that may observe or expose the affected storage, including through authority not named as an argument." wiki/spec/proofs/quotients.md#formation-and-representation: "A quotient declaration selects one closed `MereEquivalence<C, R>` conformance." wiki/spec/proofs/contracts.md#required-automatic-discharge: "for up to 64 arm combinations per obligation".

## cause

Only one fence is built for an open borrowed-storage window. While an owned field is moved out of a borrowed holder, the ownership walk in source-rust/subsystems/compiler/psi/semantics/src/plain_affine.rs:2490-2517 refuses a suspending call and nothing else. Lending the holder is refused separately, as an unavailable use.

The checker never applies the boundary/capability fence that ownership.md:271 keeps "in force" and dependent_values.md:283-292 defines. It does not look at an operation's `OperationKind::BoundaryCall` (psi/machines/operation.rs:451), at a callee's transitive service reach (`declared_service_reach`, `PortWrite`, `RegisterAccess`, a boundary's `published_service_ceiling`; see psi/semantics/src/synchronous_calls/service_reach.rs:10-60), or at `invokes` callbacks.

As a result, these calls are accepted inside the window:
- a boundary call with any reach;
- an ordinary helper that reaches a service;
- a blocking boundary call.

The deleted checker refused all three (wiki/drafts/reference/deleted_checker_full_read/reports/037.md:63-64).

The other items have no code cause:
- The quotient question was answered by #5704; only the migration is left.
- The failing test was fixed by #5361.
- Audit #3 is an implementation gap. prove.rs:1803 counts visited branches, but contracts.md:896 asks for arm combinations of goal-relevant disjunctions. Separately, an operand whose bounds are proved equal is never used as that literal (stage 06 README:1267-1276).
- Audit #1 is a spec inconsistency between modules.md:140-143 and mathematical_bindings.md:27 onward.

## code_locations

['source-rust/subsystems/compiler/psi/semantics/src/plain_affine.rs:2490-2517 (only the suspension fence exists)', 'source-rust/subsystems/compiler/psi/semantics/src/plain_affine.rs:26-48 (Unavailable record and messages)', 'source-rust/subsystems/compiler/psi/machines/operation.rs:451 (OperationKind::BoundaryCall)', 'source-rust/subsystems/compiler/psi/semantics/src/synchronous_calls/service_reach.rs:10-60 (existing service-reach closure)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/prove.rs:83,1745-1803 (case budget counts visited branches)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/README.md:1267-1276', 'source/library/core/relation.omg:44 (Equivalence still on proposition Relation; no MereEquivalence)', 'wiki/spec/language/ownership.md:251-274', 'wiki/spec/language/dependent_values.md:283-296', 'wiki/spec/proofs/quotients.md:218-272', 'wiki/spec/proofs/contracts.md:62-66,834-905', 'wiki/spec/language/modules.md:127-143 vs wiki/spec/proofs/mathematical_bindings.md:27', 'wiki/spec/language/state_contracts.md:296-306', 'wiki/drafts/designs/proof_kernel_core_audit.md:453,1630-1676', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/tests/byte_view_elements.rs:756-794', 'C:/Development/Omega/.agents/worktree/worker-6/build/diag-owner-questions/ (probe programs and logs)']

## repair_plan

WHAT CAN BE DONE NOW (the spec already decides these):

1. **Boundary and service fence during an open window.** Use lane `psi`; the spec is ownership.md#borrowed-storage-invariant-windows and dependent_values.md#invariant-windows.
   - Mechanism: in plain_affine.rs, move the "window is open" test out of the suspension check (lines 2498-2511, the `residences`/`roots` + `region.available` test) into one helper, `fn window_open(&self, state) -> Option<PlaceId>`. Use it for both fences, so locals, fields, parameters and captured residences get the same treatment.
   - Refuse an operation while the window is open when it is any of:
     - (a) an `OperationKind::BoundaryCall`;
     - (b) a `PortWrite` or `RegisterAccess`;
     - (c) a direct call whose callee closure has a nonempty service reach, a boundary edge or an `invokes` callback.
   - Compute (c) once per module with the existing `SynchronousCallGraph::service_reach_from` closure (or one per-machine reach table built beside it) and pass it into the walk. Do not recompute it per call.
   - When `module` is `None`, refuse every call that is not provably plain (when unsure, refuse).
   - Report it as a program error, not "not supported yet". Add a reason to `Unavailable`, for example a `UnavailableReason { Use, Unrestored, FencedCall }` enum in place of `unrestored: bool`. The spec says "An unclosed window rejects with both the opening write ... and the failing consumption point", so the refusal names the call site and the move. Proposed text: "cannot make a boundary or service call while an owned value is moved out of borrowed storage: store a replacement into the vacant place before this call".
   - This refuses `boundary_service`, `helper_service` and every boundary call, `reaches none` included, until the owner rules (item B below). That is the literal reading of ownership.md:271 and matches the deleted checker.
   - Disjoint-sibling loans to ordinary checked calls stay accepted. ownership.md:253-254 says "Disjoint established siblings remain usable".

2. **Floor arm count.**
   - prove.rs:1745-1803 should spend the 64-unit budget on arm combinations of disjunctions that share a subject with the goal (contracts.md:894-896). It should not charge every visited alternative, including those of failed cuts, and it should not split unrelated disjunctions.
   - Range propagation should land an operand on literal bounds that order reasoning proves equal (contracts.md:888). Customer: the 15 `(prefix - 360) + last` programs.

3. **Floats.** Nothing to build. Record in the audit that contracts.md:874-888 makes the floor integer-only.

4. **Quotient evidence.** Implement #5704, which is already on TASKS.md:312:
   - add `IsMere`, `MereRelation` and `MereEquivalence` to source/library/core/relation.omg;
   - move `Equivalence` from `proposition Relation` to `R: C -> C -> |core|::Type`;
   - make the quotient reader consume the selected `MereEquivalence` conformance;
   - refuse one proved for a different relation.

5. **Audit cleanup** (lane `docs`) in the same PR as item 1 or separately:
   - mark audit #3 and #4 as decided by contracts.md, and #2 as delegated by OWNER_QUESTIONS.md:19-22;
   - remove the "fails on main" note, since #5361 fixed it.

**Twins for item 1.** Both are refill programs on windows_x86_64 rooted at build.omg, with `machine helper() reaches Clock { Clock::tick(); }`. They differ only in where the call sits:
- Positive: `let t: Token = h.token; h.token = t; helper();` must check (exit 0).
- Negative: `let t: Token = h.token; helper(); h.token = t;` must be refused at the `helper()` call with the text above.
- Add a second negative with `Probe::ping(1u64)` declared `reaches Clock`, plus a control for a disjoint sibling (`count(@mutable h.n)` to an ordinary checked machine), which stays accepted.

**Tests.**
- Unit cases in source-rust/subsystems/compiler/psi/semantics/src/plain_affine/tests.rs for a boundary call inside the window, a reaching direct call inside the window and the same calls after restoration. Run them with `mbx test -p terminal-semantics`.
- The twin pair under the stage 06 crate's tests/fixtures/ownership/{run,fail}/<case>/. Run it with `mbx test -p lowered-program-to-checked-program`.
- `mbx clippy -p terminal-semantics -p lowered-program-to-checked-program --all-targets -- -D warnings`.
- Rerun the probes in build/diag-owner-questions/.
- For item 2: prove.rs unit tests with 64 relevant combinations and extra irrelevant disjunctions, plus one of the 15 programs.

NEEDS THE OWNER (record each in OWNER_QUESTIONS.md with the cited contracts):

A. **Level binder spelling** (audit #1). Is a `core\Level` binder a type-role name, which may not be spelled `u`, so that mathematical_bindings.md's examples change? Or is it a separate role that may use a builtin spelling, so that modules.md:140-143 gains a row? This blocks only `representation_agreement`.

B. **Relaxing the vacant-storage fence** (audit #6). Is a boundary requirement's declared signature a "complete access contract" under ownership.md:268-270? The case is parameters disjoint from the holder, `reaches none`, no `invokes`. If yes, the compiler may admit that call during a window. Must `blocks` alone also fence? effects.md lists it as an independent axis and neither fence page names it. Until answered, item 1 refuses these calls.

C. **Warning for a whole machine with a contradictory `requires`** (audit #5). This is optional and advisory only. It would extend state_contracts.md:296-306 from contradictory branches to a contradictory entry contract.

## twin_programs

Shared setup, on windows_x86_64:
- `data Token { v: u64; }` and `data Holder { token: Token; n: u64; }`
- `boundary trait Clock { machine tick() reaches none crashes none; }` with provider `TickImpl`
- `machine helper() reaches Clock { Clock::tick(); }`
- `machine refill(h: @mutable Holder) reaches Clock { ... }`
- `main` builds a Holder and calls `refill(@mutable h)`
- build.omg binds `windows_x86_64\ProgramEntry` to `main` and runs `Build::select_provider::<Clock, TickImpl>(builder, windows_x86_64)`

Positive: the refill body is `let t: Token = h.token; h.token = t; helper();`. Expected: exit 0, "compiled ... source file(s)".

Negative twin (only the call position differs): the refill body is `let t: Token = h.token; helper(); h.token = t;`. Expected: refused at the `helper();` line in machine `refill` with "cannot make a boundary or service call while an owned value is moved out of borrowed storage: store a replacement into the vacant place before this call".

Today the binary accepts the negative (build/diag-owner-questions/helper_service: exit 0). The second negative, boundary_service, uses `Probe::ping(1u64)` with ping declared `reaches Clock` and should get the same refusal. The control lend_sibling, which lends a disjoint sibling `@mutable h.n`, stays accepted. The probes boundary_none and boundary_blocks stay accepted only if owner ruling B admits `reaches none` boundary calls; otherwise they get the same refusal.

## tests

New tests:
- plain_affine unit tests in source-rust/subsystems/compiler/psi/semantics/src/plain_affine/tests.rs:
  - inside the window, a BoundaryCall, a PortWrite and a direct call to a callee that reaches a service are refused;
  - after restoration, the same calls are accepted;
  - a sibling loan to a plain callee is accepted;
  - with no module, a boundary call inside the window is refused.
- A stage 06 fixture pair (run/fail) holding the twin above, with the exact refusal text pinned.

Commands:
- `mbx test -p terminal-semantics < /dev/null`
- `mbx test -p lowered-program-to-checked-program < /dev/null`
- `mbx clippy -p terminal-semantics -p lowered-program-to-checked-program --all-targets -- -D warnings`
- rerun the 9 probes in build/diag-owner-questions/ with `omega --check --target windows_x86_64`

For the floor repair: prove.rs tests for 64 goal-relevant combinations beside unrelated disjunctions, and one `register_stack_arg` program.

## risks

Refusing every boundary call during a window may reject library or sample code that today calls `reaches none` boundaries between a move-out and its restoring store. Before landing, grep source/library for move-out/restore pairs around boundary calls. The fence is the spec's literal reading; relaxing it needs owner ruling B, not a local exception.

Computing callee reach inside plain_affine must reuse the existing closure, computed once per module. Building it per call would hit the cost rules in AGENTS.md.

The failing test was not re-run, because cargo is forbidden in this job. Its status rests on #5361's recorded validation and on the assertion present at HEAD.

