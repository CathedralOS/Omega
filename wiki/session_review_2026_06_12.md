# Session Review — 2026-06-12 Autonomous Execution Sweep

Read this first. It captures (1) decisions I made WITHOUT you that you should
sanity-check, and (2) decisions still BLOCKING that only you can make. The
factual landing record is in TASKS.md; the per-feature detail is in the
commit messages (range `21ea200f..HEAD`). Suite went 234 → 256 harness tests
(canary corpus much larger), differential oracle fully matched, `cargo test
--workspace` green, everything pushed to main.

## 1. Decisions I made for you (review these)

These were "highly-likely" calls I took to keep moving. None are
irreversible; flag any you dislike.

1. **Accepted the comptime scout recommendations (M1–M6) as-is** to build
   comptime stage 1, except M3 which you'd already corrected. So: purity gate
   = decision-12 inferred effects; `self.[field]` bracket reflection spelling;
   array-lengths as the first const position; target-width emulation
   mandatory; generators must be effect-free; equatable.rs is temporary.
   Only stage-1 (array lengths) is BUILT; the rest is still just the accepted
   direction.

2. **Staged lifetimes as elision-only first** (no tick syntax yet). The win
   landed is real and it closed an actual soundness hole (free-machine view
   returns carried NO loan — mutating the source while the view lived was
   silently accepted). Explicit `<'a>` parameters and struct borrows are
   deferred to stage 2. If you wanted ticks sooner, say so.

3. **Concurrency stage 1 is a parser desugar, not a runtime.** `spawn`
   executes SYNCHRONOUSLY at the spawn site and `Join<T>` ERASES to `T`.
   This is semantically honest *only because no atomics exist yet* (nothing
   can observe interleaving). The cost: `let h: Join<T> = non_spawn_call()`
   also typechecks (Join is transparent). A real scheduler + a synthesized
   Join type replaces this at stage 2. Borrows into spawn and `self` capture
   are rejected for now.

4. **Versioned<T> payload layout is a STRUCT (sum of era sizes), not the
   union-of-eras max-size layout you signed off (decision 14).** This is
   unobservable today (ZII era-0 is the only construction path until a
   boundary decoder exists) and is loudly documented in
   `omega-core/src/versioning.rs`. The true union layout lands with the
   decoder (stage 4). Calling this out because it technically diverges from
   the frozen decision — accepted as an invisible interim, but you should
   know.

5. **FixedVec `pop` returns the value directly (no Option); empty-pop is a
   compile-time proof failure**, following the `String::push_str` precedent.
   Reasonable but it's a real ergonomics stance — confirm.

6. **Spawned/struct-return canaries probe via guard ladders + literal exits**
   and I did NOT add atomics, Mutex, scopes, cancellation, or select — those
   wait for C2–C5 sign-off and a real scheduler.

## 1b. LANDMINE to fix before real concurrency (flagged, not yet fixed)

Atomics stage 1 landed (M1 types + M2 load/store + M3 fetch_add + M4
compare_exchange). CORRECTION (verified from the canary headers): BOTH RMW
ops — fetch_add AND compare_exchange — are currently PARSER DESUGARS into
non-atomic integer read-modify-write sequences, NOT real `LOCK`-prefixed
instructions. (An earlier note here wrongly said fetch_add was a real atomic
RMW; it is not.) They are value-correct and unobservably non-atomic in stage
1 (no scheduler/threads), but each is a silent data race the moment true
parallelism exists. load/store (M2) are genuinely fine — a plain aligned
`mov` IS atomic on x86; only the RMW ops need `LOCK`. BEFORE the scheduler
arc, every atomic RMW (fetch_add, compare_exchange, and any later
swap/fetch_sub/and/or) must become a real `LOCK`-prefixed instruction
(`LOCK xadd` / `LOCK cmpxchg`). Flagged in the canary headers + a tracked
task chip. Do NOT ship concurrency on the desugar RMWs.

TURNKEY RECIPE (do this WITH the scheduler, when atomicity is testable against
real interleaving — not before; there is no observable difference single-threaded):
1. STOP desugaring at parse. `try_parse_atomic_fetch_add_let` /
   `try_parse_atomic_compare_exchange_let` (omega-tokens-to-syntax-trees
   parser/statement.rs) expand into ordinary statement pairs today. Instead emit
   a dedicated atomic-RMW node that survives lowering, carrying {place,
   delta-or-(expected,new), ordering, returns-old}.
2. Add an atomic-RMW storage op-kind and thread it abstract -> target ->
   assigned -> machine-instructions, exactly like the float
   `WriteRuntimeStorageConvert` op-kind was added: a new storage-write op-kind
   must ALSO be added to BOTH emission-planning blocker lists (storage_blockers
   + runtime_text_blockers) or it errors "needs lowering" despite emitting.
3. x86_64 emission: `LOCK XADD r/m,r` (F0 0F C1 /r) for fetch_add; `LOCK
   CMPXCHG r/m,r` (F0 0F B1 /r, rax = expected, ZF/result handling) for
   compare_exchange. Add the width fn + relocation record (reloc offset fns live
   in BOTH omega-instruction-selection widths.rs AND omega-relocations/offsets —
   keep in lockstep).
4. aarch64: `LDADD`/`CAS` (ARMv8.1-LSE) or an LL/SC `LDXR`/`STXR` loop, or a loud
   "not implemented" error if no canary needs it yet.
5. Interpreter: model the op (single-threaded, value semantics already match the
   old desugar — keep returning the prior value). Register a RUN canary in
   canary_suite.rs AND differential.rs (drift guard).

## 1c. Float-codegen pending canaries — investigation 2026-06-14

In-session diagnosis of the three open f32 pending canaries (the env's
agent-death cycle made agent fixes unviable; investigated directly). Findings
sharpen the roots but no fix landed — recorded so the next attempt is targeted:

- The three do NOT share a root, and NONE is the nested-operand-width gap I
  first suspected. A speculative fix for that gap (deriving f32 width in
  `append_runtime_value_operand`'s nested float-binary arm instead of the
  hardcoded `8` at isa-x86_64 lib.rs ~4298) fixed ZERO of them and was
  reverted (no dead code).
- `f32_field_binary_to_local_cast_wrong` (`let c: f32 = self.a + self.b`):
  a TOP-LEVEL f32 storage-binary-write whose target is a LOCAL slot. f32
  writes to a FIELD target work (runtime_f32_arithmetic passes), so the root
  is in the f32 storage-binary-write LOCAL-target path
  (encode_runtime_storage_binary_write / its target-place resolution), not
  the operand path.
- `f32_to_f64_local_cast_wrong` (`f32 as f64` to a local): the cast-to-local
  width/convert path; likely shares the local-target machinery with the
  above.
- `f32_deep_chain_binary_wrong` (`a+b+c+d`, depth 3): a DISTINCT root —
  depth-2 works, depth-3 fails, f64 depth-4 works, and the GUARD compare is
  wrong (so the XMM/r10 result is wrong, not just the store). Not width
  (the width fix didn't touch it). Suspect the push/pop r10 nesting or an
  xmm clobber that only manifests at 3+ nested levels.
- SEPARATELY CONFIRMED latent gap (code-comment-acknowledged at isa-x86_64
  lib.rs ~4298): nested float binary operands hardcode f64 width (`addsd`)
  regardless of f32. No current canary exercises it (the 3 above hit other
  paths); worth its own canary + the width-derivation fix when touched.

## 2. Blocking decisions — only you (still open in TASKS.md register)

- **C2–C5** (concurrency): task unit = spawned machine; structured Join
  scopes; atomics-only sharing with Mutex as a library type; C11 intrinsics +
  memory model. All consistent with decision 16; I treated them as
  rubber-stamps but did NOT implement past the synchronous MVP, because the
  real scheduler is the point where these bite.
- **S1–S6** (separate compilation): package = component, hermetic static
  composition, etc. Untouched — this is the big backend revamp.
- **M1–M6** beyond stage 1: const args, field defaults, const type params,
  the trait-generator framework, retiring equatable.rs.
- **A1–A5** beyond stage 1: the real `Region<'r>` runtime, `Vec<'r, T>`,
  pluggable allocators.
- **Decision 11 residue**: VALUE-position machine calls bypass argument
  validation entirely, so a `[copy]` bound on `let r = self.pick(&self.h)`
  is NOT enforced (pending canary `generics/machine_bound_value_call_unchecked`).
  Needs value-position calls to gain argument validation — a real frontend
  gap, not just a missing check.

## 2b. NEW bug found by end-to-end sample authoring (FIXED)

> RESOLVED 2026-06-12 (commit a2b961f8). Root cause was narrower than the
> "write-back" framing below: `InlineBranching` argument materialization had
> no `StructLiteral` handler, so the by-value case ARGUMENT was never written
> into the callee's parameter slot at all — the case tag stayed 0, dispatch
> always took the zero-case arm, and the self-write simply never ran. Same
> family as the struct-arg/return fixes but a distinct code path (argument
> materialization, not the leaf terminal path). Pending canary promoted to
> `pass/calls/by_value_case_param_self_write_exit` (RUN, exit 70);
> vending_machine now runs to 70 end to end. The narration below is kept for
> the method record.


Authoring `samples/vending_machine` (an event-driven case-payload state
machine) surfaced an 8th native miscompile, distinct from the seven below:

> A `&mut self` machine taking a **by-value case-bearing parameter** loses
> writes to `self.<field>` made in a dispatched substate — the caller
> observes the pre-call value.

Narrowed by probes: scalar args persist the write correctly, so it's the
`&mut self` WRITE-BACK leg of the by-value aggregate-parameter family (the
arg/return-value legs were fixed today in c8519251 / 04bf00d9). Captured as
`canaries/pending/calls/by_value_case_param_self_write_lost` (exit 80 = lost,
should be 70) + a tracked task chip. NOT registered in canary_suite yet
(deferred past the in-flight registry refactor). The sample documents it as
its blocking issue. This is the headline reason end-to-end samples matter:
single-feature canaries each passed, but their COMBINATION (aggregate param
+ &mut self + dispatched write) was untested and broken.

## 3. Bugs found and FIXED this session (no action needed, FYI)

ELEVEN native miscompiles surfaced by the new features + the canary sweep +
end-to-end sample authoring; ALL eleven fixed and oracle-verified (suite 260,
ACTIVE_PENDING_CANARIES empty):
1. by-value struct ARGS to free machines (3 stacked instruction-selection bugs)
2. by-value struct RETURNS
3. trailing bare-local-name returns (storage planner dropped the slot)
4. String `!=` dropped the text term
5. guard-position String `==` was unlowered
6. versioned-match exhaustiveness was unchecked
7. const-eval misparsed a parenthesized bare-call arm
8. by-value CASE param → dispatched self-write lost (StructLiteral arg never
   materialized into the param slot — a2b961f8; found via vending_machine)
9. sequential value-calls clobber result slots (callee-1 internal `let` +
   callee-2 more args — found via shapes_area; a982fe66)
10. i32 `let`-local through a nested state arg re-folded to its
    post-mutation initializer (found via bank_ledger; bbcb81b3)
11. f64 value through a state arg never materialized (Float literal gap in
    argument materialization — found via particle_sim; bdf1d674)

The recurring root across 8 of the 11: **value materialization in
instruction selection picks a wrong/overlapping/missing slot or width, and
the bad write is silent**. Each was a distinct trigger (struct arg, struct
return, bare-local return, case-param arg, sequential-call slot, nested
let-local fold, f64 literal) but the family is "the leaf/argument
materialization path didn't handle this shape and emitted nothing or the
wrong move." A blanket hard error there is NOT safe (structure review, §4),
so the family must keep being closed shape-by-shape — end-to-end sample
authoring is the most effective net for finding the remaining ones.

The versioned "more fields than current" suspected bug turned out NOT to
exist (added regression coverage instead). Also landed: value-position calls
now run argument/bound validation (was a silent gap — decision-13 residue
closed); `omega-names` (2152-line orphan crate) deleted; structure review +
leaf.rs documentation.

Samples filled this session (empty dirs → working programs, all exit 70):
bounded_counter, vending_machine, shapes_area, wire_protocol, traffic_light,
score_tracker (+ float/array round-2 lane running). This is the "graduate to
ironclad" surface — combinations the canary corpus didn't exercise.

## 4. The pattern worth a structural fix (see structure review)

Four of the seven miscompiles were the SAME shape: instruction-selection
resolution returns `None` and the entire write silently vanishes — no
diagnostic, just a wrong result. I hoped a "no write strategy selected" hard
error in the leaf terminal-value pipeline could convert this whole bug class
into compile errors. The structure-review lane checked and **it is NOT safe**:
that fallthrough fires on legal programs that use text-guard lowering through
refs/params (the existing `guard_contains_string_literal` carve-out). So the
blanket guard is off the table; the bug class has to be closed case by case
(four down, one — the by-value-case-param write-back, §2b — to go). The
leaf.rs pipeline is now documented as a four-layer stack (commit `e3a227a5`).
Full structure report: `wiki/architecture/structure_review_2026_06_12.md`.

Structure review also flagged **`omega-names` as a 2127-line orphaned crate
with zero consumers** (resolution moved to
`omega-syntax-trees-to-symbol-resolved-trees` long ago). A removal lane is
running; it's pure dead-inventory deletion.

## 5. Operational note

Background agents lose their task records on app restart, orphaning locked
worktrees and sometimes losing uncommitted work (cost us two relaunches).
All agents are now instructed to COMMIT INCREMENTALLY. Default-model
(`claude-fable-5`) agent spawns also hit intermittent access errors mid-run;
relaunching with an explicit model override (sonnet) worked around it.
