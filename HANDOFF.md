# Handoff: finish the proof layer map

Written 2026-11-25 at origin/main `ba6e34f90206`. The map is https://claude.ai/artifact/KL7xm6h7bYqvq9ZhbDmT75 (version 29). Its local source was a scratchpad file. If you can't edit that artifact from your account, keep this list instead.

## How to work

- Follow AGENTS.md exactly. Use mbx for builds, tests and Clippy, with output redirected to a log, never piped. Squash-merge by hand after `admission` passes; never use auto-merge. The PR body ends with the `cargo xtask provenance` Agent line.
- Owner (Zac) rulings are never asked in chat. Record the question in `OWNER_QUESTIONS.md` through a docs PR that follows that file's rules. Before calling a question open, fetch origin/main and re-read the file.
- Cloud sessions are preferred. Run each item below as its own session; they are independent unless marked.
- The diagnoses from 2026-11-24 are on branch `work/notes-proof-map-diagnoses` (not for merge). Fetch them with `git fetch origin work/notes-proof-map-diagnoses`, then `git show FETCH_HEAD:<name>.md`. Main has moved since; verify before acting.

## Remaining items (each is on TASKS.md, PROOF-KERNEL-CORE unless noted)

1. **Level binders, then quotient evidence.**
   - Build `core\Level` binders in stages 02, 03 and 05 (audit C2, `wiki/drafts/designs/proof_kernel_core_audit.md`) for binders not spelled `u`.
   - Then add `IsMere`, `MereRelation` and `MereEquivalence` to `source/library/core/relation.omg` per `wiki/spec/proofs/quotients.md` (#5704), and make the quotient reader consume them.
   - Also needed: a derived proof that plain data is a set (the kernel has no UIP).
   - Done before this: #5835 (relation families declared by type). The step 7 bullet states the rest.
2. **Quotients that run.** The 19 programs under `samples/cli/facts/quotient_closed_literal/`: quotient data in the ordinary reader (a class stored as its representative), and a conformance applied at a proposition. Read the step 7 bullet.
3. **Computed index and erased scalar field.**
   - A computed index (`Vector<Length + 1, ..>`) waits on typed preparation (stage 04) evaluating an operator in a static argument (`inner::<Count + 1>()`).
   - An erased scalar field read in a clause (`data Capped requires value <= cap { value: u64; cap [erased]: u64; }`).
   - Type binders in definitions are still refused.
   - Done before this: #5795 (evidence in an erased field) and #5804 (definitions with value binders). Diagnosis: `declarations-binders.md`.
4. **Guarded crash routes for private machines and constant initializers.** Stage 05 publishes an omitted private crash route as `Trap` with guard `Truth` (`outcome.rs` `published`). That blocks two samples:
   - `samples/cli/facts/indexed_crash_guard/other_state` (the step 5 bullet);
   - `target_layout_derived_constant/controls/skipped_crashing_helper`, once `main` reads its `BAD` (the EARLY-TERMINAL-JOBS bullet: "Admit an invocation whose guarded crash route is false for its actual operands").

   Infer the route's guard from the path facts instead. Never accept an unproved route. An experiment that made every initializer `crashes none` wrongly refused `inspection_scalar_library`, so don't do that.
5. **Record identity over opaque fields.** An erased, atomic, bounded, view, reference, sum or array field is still one opaque value, so a record holding one has its identity proved only as a whole. Also owed:
   - the same result when a citation supplies a field identity;
   - negative twins showing that equal addresses, a stale observation and a true user-defined comparison do not substitute for a field identity.

   Read the "Automatic record identity" bullet and #5796.
6. **Owner question 2** (may a level binder be named `u`) waits on Zac. Don't decide it. It blocks only `samples/cli/facts/representation_agreement`.
7. **Fleet baseline rerun**, after 1–5 land. Run `mbx xtask sweep fleet start` on the LAN machines (`~/omega-agent/fleet.toml`). The last run was `20261125-0034-2b6e425ebf-windows_x86_64`: 1,804 checked, 33 negatives wrongly accepted, all 33 since resolved. Checking every row since #5728 makes each check about 2.5 times as expensive.
8. **Optional: cut the every-row check cost.** Check each target's shared standard-library entry adapter once and reuse the result, without checking less. Context is in #5728's body.

## Done on 2026-11-25 (for the map)

#5708, #5720, #5722 and #5728 (the 33 negatives); #5760; #5787; #5812, #5816 and #5819; #5823, #5828 and #5852 (vacant storage, ruled in #5851); #5831; #5794 (case tests); #5796, #5803 and #5808 (equality forms); #5795 and #5804; #5835; #5842 and #5846 (floor); #5840, #5843, #5847 and #5849 (crash guards); #5857 (owner question 2 recorded).
