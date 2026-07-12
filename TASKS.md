> OWNER_QUESTIONS.md (repo root) consolidates all lanes' pending owner decisions — batch-answerable.

# Tasks

Working backlog only. Finished work lives in the git log; canary headers carry
each fix's story. (Condensed 2026-07-12; re-condensed + NEXT TASKS queue
loaded 2026-07-18 per owner directive.)

## Current Strategic Focus

Omega's first real consumer is the Cathedral OS (`../Cathedral`). The gap
analysis lives in [wiki/cathedral_alignment.md](wiki/cathedral_alignment.md) —
Tier 1 items there (ZII guarantee, wire data semantics, versioned data,
separate-compilation awareness, concurrency/atomics decisions, freestanding
target, enum payloads) bias which vertical slices get picked next.

## NEXT TASKS — design-unblocked, agent-ready (loaded 2026-07-18, owner directive)

Claim an item, work its rungs in order, canaries per rung, push per rung.
Collision map: the MAIN LANE owns RECAST/M2; the FS LANE owns the
dispatch-region + receiver-phase family. Everything queued here avoids both.

1. **Math roster ladder N1→N4** (section below) — zero backend/codegen
   contact; N1 LANDED 2026-07-11 (proof-only classification + all faces);
   N2 rungs a–c LANDED (bignum + exact engines; the u64>i64::MAX debt
   retired as a soundness fix); N4's FIRST SLICE landed 2026-07-11: the
   "core-injection" question DISSOLVED — the bundled `omega::` root +
   depend-mapping already reach omega/language/core, so `data Nat` lives
   in core/nat.omg as ordinary recursive (proof-only) data, pinned
   pass/proofs/runtime_core_nat_declared_exit + the consumption-refusal
   twin. Remaining: N2(d) Succ bridge, N3 routing, N4's Seq/Bag/Rat +
   extraction lemmas + view dissolution. Continue into N5–N7 when
   reached (they need the `<machine M>` plumbing and the `%` former).
2. **Measured recursion MR1 + MR3** — LANDED 2026-07-11 (MR1 whole; MR3
   direct leg — the mutual leg rides MR4). Next in the family: MR2 (the
   terminal-position tail rewrite onto loop-backs — the arm-target spelling
   needed NO lowering and already runs), then MR4/MR5.
3. **Dependent types R2** (section below) — where-clause + gating + windows;
   the big semantic build, explicitly ADDITIVE (the landed eager store-time
   checks stay sound as the conservative tier). One careful agent, multi-day.
4. **Windows platform-verification session** (section below) — checklist-
   shaped; one session on a Windows host closes the whole list.

The rendering-sample sweep landed 2026-07-11 (see Language ergonomics;
the match-subject computed-index gap closed with it).

## Cathedral M2 (owner priority 2026-07-15; RECAST = main lane, claimed)

**2026-07-11 full-tree measurement (owner side): Cathedral's TYPED M1 BOOTS.**
The whole Cathedral boot package (typed EfiStatus/EfiHandle wrappers,
`&TextOutputProtocol` reference field, `effects device_io`, field-model
provides, `use` modules, `Type::`-scoped const) was compiled via a staged
tree standing in for depend-mapping, and the image PRINTS THE GREETING under
QEMU/OVMF. Cathedral-side spelling drift is synced (Cathedral 5d8c6fe). The
measured remaining M2 blockers, NOW IN ORDER:

**OWNER 2026-07-11: every blocker below is MISSING IMPL, not missing
design — nothing here is owner-gated; claim and build.** Design cites per
item; the one design question found en route (ensures-as-vouch vs guard for
the firmware out-values) is RESOLVED against the vouch — see blocker 2.

1. **Provides-row catalog — LANDED 2026-07-11 (main lane):** names
   outside the built-in host catalog intern to stable `Custom(u32)` keys
   (process-wide interner in omega-calling-conventions; the key stays
   Copy, binding/call sites agree by construction), and the four
   authored-row consumer arms (selection operands, x86_64 encode +
   relocation-sites, emission-planning import blockers) accept Custom
   alongside the old Unknown sentinel. Any number of authored rows
   coexist; a genuinely repeated (trait, method) pair still collides
   loudly. Pinned pass/targets/efi_two_provides_rows (get_memory_map +
   exit_boot_services cross-compiled for uefi_x64) +
   fail/targets/duplicate_provides_row_rejected. M2's three-row image is
   expressible. [Original: the design is the field model, extern §12.]
2. **Gap-4 remainder, confirmed against the real walk**: the landed
   guard-route is single-predecessor + literal-K only; the walk state is
   the multi-predecessor self-re-entering loop its own comment names, so
   `interior_byte_region_source` returns None. Needs (a) per-edge meet,
   (b) the symbolic `offset + desc_size < map_size` route.
   DIAGNOSTIC BUG FIXED 2026-07-11 (main lane): the interior judgment
   answers three ways (NotInteriorShape / OffsetUnproven / Bounded);
   the unproven-offset case names the real failure and all three
   discharge routes (declared range, dominating guard, boundary-ensures
   witness) — pinned fail/recast/unbounded_offset_names_the_bound. Note
   the ensures WITNESS route also landed (see R4), and (a) the PER-EDGE
   MEET landed 2026-07-11: every incoming edge must prove (constant /
   guard / ensures routes per edge) and the max wins — pinned
   pass/recast/runtime_multi_edge_offset_meet_exit +
   fail/recast/multi_edge_offset_meet_rejected. (b) the symbolic route
   LANDED 2026-07-11 — the main-lane half of this blocker is COMPLETE,
   and VERIFIED against the owner-corrected HONEST spelling (post-call
   SANITY GUARD, no trait ensures — the ensures vouch is false under
   BUFFER_TOO_SMALL): guard bounds resolve symbolic right-hand NAMEs
   recursively through the per-edge meet (depth-capped; self-forwarding
   edges preserve entry bounds and are skipped), BOTH witness sides ride
   the guard route (`map_size <= K && desc_size >= F` on the post-call
   transition; guard_lower_bound_for is the lower twin), and the
   Add-composition discharges the compound loop argument
   (bound(offset + desc_size) = bound(RHS) - floor(desc_size)). Four lib
   tests pin the guard-route discharge, the ensures-machinery variant
   (still sound for boundaries whose contracts ARE unconditional),
   wide-witness refusal, and WEAK-GUARD refusal — the `< map_size`
   spelling refuses at exactly the tail overrun the coordination note
   predicts, mechanically enforcing the committed Cathedral spelling. ORIGINAL DESIGN
   (retained for the Cathedral-side spelling requirements): the loop edge's argument is the COMPOUND
   `offset + desc_size`, and guard_upper_bound_for already matches it by
   display — what's missing is (i) a symbolic right-hand side: `label <=
   NAME`/`< NAME` resolves NAME's inclusive bound recursively (depth-
   limited) through the same per-edge meet — for the walk, map_size's
   meet is {ensures witness 16384 on the entry edge; SELF-FORWARDING on
   the loop edge, which preserves the entry bound and must be SKIPPED in
   the meet (argument display == the param's own name, same state)};
   (ii) LOWER-bound witnesses (`desc_size >= 40`) via a
   guard_lower_bound_for twin; (iii) the subtraction composition:
   footprint offset' + sizeof <= N discharges from bound(offset' +
   desc_size) <= B ∧ desc_size >= sizeof ∧ B <= N. ✅ SPELLING
   COORDINATION DONE (Cathedral f0b7572, 2026-07-11): `more` now spells
   `offset + desc_size + desc_size <= map_size` (bounds the next
   descriptor's END). ⚠️ WITNESS CORRECTION (owner): the proposed
   `ensures map_size <= 16384` is a FALSE unconditional vouch — on
   EFI_BUFFER_TOO_SMALL firmware returns the NEEDED size (> capacity).
   Cathedral instead spells an honest POST-CALL GUARD:
   `let sane: bool = map_size <= 16384 && desc_size >= 40;` gating the
   walk entry — so both witnesses arrive through the ALREADY-LANDED
   guard route on the entry edge + self-forwarding on the loop edge; no
   new ensures machinery is needed for M2, and no trait ensures should
   be added. (Also: the stride floor is 40 — sizeof EfiMemoryDescriptor
   under natural alignment — not 48.) Remaining impl is exactly (i) the
   symbolic RHS resolution + (iii) the composition, discharging the
   Cathedral spelling as committed. [ENGINEERING — design above is
   complete; no owner input needed.]
3. **depend-mapping — LANDED 2026-07-11 (main lane):** the build
   vocabulary gained `Build::depend` + the `path` helper; each frontier
   collects `b.depend("alias", path("dir"))` rows (resolved against the
   declaring build.omg's directory) BEFORE resolving its uses, and a use
   whose first segment matches an alias resolves into the aliased
   directory. Dependency packages' build.omgs are read AS DATA for their
   transitive rows (their `build` machines never join the program — two
   would collide). Pinned pass/build/runtime_depend_mapping_exit
   (companion build.omg + aliased use + depended free const, both
   engines). Undeclared aliases fail loudly at the resolved path.
   [Original: design settled in build_and_package_model.md.]
4. **Free-floating const — LANDED 2026-07-11 (main lane):** the
   SHADOWING WALK guards it — a bare-name const must not collide with
   any name a bare reference could resolve to (data/machine/state
   names, params, locals, fields, cases) anywhere in the program;
   collisions refuse AT THE CONST naming both sites, so single-segment
   substitution can never silently win over a like-named local/field.
   Pinned pass/constants/runtime_free_const_exit +
   fail/constants/free_const_{local,field}_collision. Package/module
   NAMESPACING (`memory::PAGE_SIZE`) rides depend-mapping (blocker 3).
   [Original note — the design IS free-floating (owner,
   static_root_and_constants.md);
   the missing piece is the shadowing walk the v0 error message names.
   No owner input needed.]
5. **NEW (owner, 2026-07-11, measured on the REAL tree post-1–4): the
   x86_64 record-view read encoder — THE LAST M2 BLOCKER.** With 1–4
   landed, the full Cathedral tree (`../Cathedral/source/boot/uefi/
   main.omg`, `omega-run --target uefi_x64`, real build.omg + depend
   rows) now clears the ENTIRE semantic pipeline — depend-mapping
   resolves contracts/+core/, three provides rows coexist, free consts
   substitute, and the walk's recast footprint DISCHARGES (per-edge
   meet: entry edge constant 0 under the inline sanity guard; loop edge
   `offset + desc_size <= 16344` literal conjunct; `in Trapping` walk
   params — Cathedral 52e6a6e has the proof-carrying spelling). The one
   remaining refusal is BACKEND: "X86_64 MVP encoder cannot read
   40-byte machine indexed values yet" — the C2 record-view snapshot
   (40-byte read from a machine byte-array field at a runtime offset)
   has aarch64 lowering (descriptor_walk runs there) but no x86_64
   encoder rung. Land it and Cathedral M2 compiles end-to-end; the
   QEMU/OVMF done-check is ready on the Cathedral side. Spelling notes
   for the corpus, learned en route: guard conjuncts must be INLINE
   transition subjects (let-bound bools hide them from display
   matching), and the R1a declared-range edge-discharge cannot yet read
   compound guard displays (`offset + desc_size <= K`) — the guard-route
   meet can; a redundant literal conjunct bridges it. [ENGINEERING — an
   encoder rung; no owner input needed.]

Original gap list (compile-checked against the real
own_machine.omg 2026-07-11 via the new `omega-run --target uefi_x64`):
the recast MECHANICS are DONE for M2's shape (rungs A/B/C1/C2; the
all-scalar EfiMemoryDescriptor record view serves — see
samples/cli/systems/descriptor_walk for the in-tree twin), and the
remaining blockers are SURFACE + one proof rung: (1) `let mut` locals —
LANDED 2026-07-11: parser + tree threading + the mutability gate (BARE
reassignment of a plain let refuses — it used to compile and natively
fold reads to the STALE initializer, a silent divergence, pinned
fail/calls/plain_let_reassign_rejected; member/index fills and
`&mut`-view pointee writes stay ungated; mut locals slot-backed, never
bind-folded — pass/calls/runtime_let_mut_reassign_exit); (2) the `and`
boolean keyword — NOT guide vocabulary, own_machine.omg drift,
Cathedral-side fix; (3) tuple-subject transitions — LANDED 2026-07-11 (the
multi-subject desugar existed end-to-end; the missing piece was
bool-matrix EXHAUSTIVENESS — a covering matrix's last arm rewrites to
the fall-through at parse; uncovered matrices keep the refusal —
pass/control_flow/runtime_tuple_matrix_exhaustive_exit + the uncovered
fail canary; note the pre-existing `_`-armed tuple canary was nearly
clobbered by name collision — check for an existing canary before
minting one);
(4) the walk's
footprint is against RUNTIME `map_size` (`offset + desc_size <
map_size`), needing the guard/coupling footprint route rather than C1's
literal-N interval. FIRST RUNG LANDED 2026-07-11: an unranged offset
param discharges through the caller's dominating incoming-edge guard —
single-predecessor walk, guarded arm only, argument matched at the
param's non-self position, literal `<= K`/`< K` bound through `&&`;
the checker's element-index prover SKIPS a judged recast's direct
Indexed operand (the footprint `K + size <= N` subsumes `off < N`;
nested reads keep their obligations) —
pass/recast/runtime_guarded_offset_recast_exit +
fail/recast/guarded_offset_footprint_rejected. REMAINING for the real
walk shape: (a) MULTI-predecessor meet (the walk state re-enters
itself — every incoming edge must prove the bound, including the loop
arm), (b) the SYMBOLIC bound `offset + desc_size < map_size` where
`map_size` is itself runtime — needs a boundary `ensures` tying the
out-param to the buffer capacity (`map_size <= 16384`), then the
transitive chain; the DBM/coupling machinery from R1/R3 is the intended
carrier. NOTE: own_machine.omg currently fails AT PARSE
(`own_machine.omg:15:9`, `use uefi.EfiHandle` dot-paths + `package`
statements — guide ch15 settled `use pkg::Item;` with build.omg
packages and no `package` lines), Cathedral-side drift like (2); the
gap list above was measured against the header-stripped body, which
still stands. The old "two red efi
tests" proved stale 2026-07-17: field-model dispatch and `&mut` out-params
both serve, pinned (pass/targets/efi_vtable_field_call,
pass/targets/efi_out_param_call). Done-check: boot under QEMU/OVMF, greeting
prints, ExitBootServices succeeds against the fresh MapKey, no crash after
exit (the machine idles, owned).

## Probe rotation

Swept clean and pinned 2026-07-12..13 (operand positions, comparison
complements, staleness, ZII boundaries, deep-nesting writes + aggregate arg
marshaling, range endpoints, u64 high-bit wrapped ops; the fixes found en
route are in the git log). Marginal value on those axes is LOW — point
probes at NEW feature surfaces as they land, not at re-walking these.

## Dependent types — engineering track (design COMPLETE 2026-07-18)

Chapter 12 + design_briefs/dependent_types.md are decision-complete (gating,
windows, where-clause domains, stores bound, storage shapes — every owner
question settled or parked). LANDED 2026-07-09, detail in git log + canary
headers (pass/dependent/, pass/collections/, fail/dependent/): **R0** direct
`pixels[y*W+x]` (depth-2 hoist, compositional intervals, fence-before-fold);
**R1a** symbolic atoms (`i: u32 [0..=self.count]` — declaration gate, proof
atom, caller/callee discharge, forwarding, the subtraction rule, in-callee
ordering DBM mints, machine-`requires` intake on both sides, sibling-len
`[0..items.len]`); **R3 + R3b** bounded products (`requires rows*cols <= K`
discharges `y*self.cols+x`, direct spelling included) with the
bounded-escape store-containment keystone. Open rungs:

- **R1 remainder:** value-vs-value ENDPOINT mints LANDED 2026-07-11:
  a guard comparing two places transfers the bound-source's enforced
  declared endpoint onto the bounded place (`i < k` with `k: u32
  [0..=8]` proves `i < 8`; `<=` shifts; `>`/`>=` mirror), seeded on the
  co-located arm and the positive incoming edges
  (seed_value_vs_value_endpoints; two lib tests pin discharge and the
  one-past-the-region refusal). Still open in this bullet:
  value-vs-value EQUALITY mints across machines (`requires a.cols ==
  b.rows`, the matrix-multiply shape — the entailment engine's equality
  harvest covers same-machine; the cross-machine leg rides R4/R5
  framing). The bracket-as-sugar half LANDED
  2026-07-11: ENTRY-state params' literal `[a..=b]` ranges join the
  entailment engine's hypotheses on both the empty-body and inductive
  paths (collect_entry_range_hypotheses; `k: u64 [0..=8]` proves
  `ensures result <= 9` for `k + 1` with no spelled requires; two lib
  tests pin discharge and the no-over-prove rail). Cross-machine
  dependent params ride R4.
- **R3 residue (store-proof completion):** UNBOUNDED-store seeding stays
  permissive (conservative post-entry env seeding would flip sound corpus
  shapes) — revisit with a plan.
- **R2 (where-clause + gating + windows) — QUEUED (Next Tasks #3):** the
  big semantic build — where-clause parsing on data, the default-domain
  layer, gating (zero-excluding domains legal, construction mandatory
  fields), consumption-point windows (ADDITIVE relaxation of the landed
  store-time checks). Implementation notes: the clause is spelling over the
  DEFAULT DOMAIN model (re-skinnable); confirm at implementation whether
  the init-syntax reconstruction form is still needed now that windows
  admit piecemeal writes (likely dissolved). Record:
  design_briefs/dependent_types.md §6; ch11 (windows) is the spec.
- **R4 (boundary witness mints, proof side) — SLICE 1 LANDED 2026-07-11:**
  out-params as witnesses, S4 tier: a boundary callee's `ensures
  <param> <= K` re-seeds the `&mut` out-argument's PLACE in the value env
  right after the call clears it (calls::boundary_trait_signature +
  arithmetic_domains::seed_out_param_ensures; conjunctions split,
  literal-vs-param comparisons only, intersected with type+declared
  ranges; flow-scoped — a later write kills it; three lib tests pin
  mint/negative/rebind). CHECKER tier LANDED same day: the ranges walk's
  Call arm forgets every `&mut`-written place's upper bound, then seeds
  `ensures <param> <= K`/`< K` conjuncts as index-upper-bound facts on
  the matching argument places (statements.rs::
  seed_boundary_call_ensures_facts; three lib tests pin discharge /
  no-ensures refusal / too-wide-bound refusal — `buf[self.n]` after
  `ensures size <= 8` proves against length 12). CROSS-STATE transport
  LANDED same day (slice 3): ParameterFacts gained a MergedBound upper
  bound — max-over-edges meet, one unbounded edge poisons — collected
  from each incoming transition's argument (ensures-seeded or constant)
  and re-seeded onto the param name at state entry; the collection pass
  runs the ensures intake and mirrors the rebind invalidation. Three
  more lib tests pin transport / poisoned-sibling-edge / rebind-kill —
  the own_machine shape (`walk(self.n)` after `ensures size <= 8`
  proving `buf[off]`) discharges end to end. The recast judgment's ensures
  route LANDED same day: the incoming-edge walk's bound now falls back
  to an R4 witness — the LAST boundary call before the transition whose
  `ensures <param> <= K` bounds the `&mut` argument place spelled like
  the transition argument, invalidated by any intervening write or call
  — and Always/`_`-arm edges may carry it (the witness precedes the
  whole transition). The M2 mini-shape (`get_size(&mut self.n)` with
  `ensures size <= 8`, then `read(self.n)` recasting `&buf[off] as
  &u32`) discharges 8+4<=12 end to end; too-wide and intervening-call
  twins refuse (three lib tests). Bounded-target CONTAINMENT LANDED
  2026-07-11: BoundedAssignmentObligation carries the live
  ensures-witness set (a boundary call replaces the set with its own
  witnessed places; an assignment drops its target's bound), and the
  checker clamps the value directly plus a witness-only binary refold
  (no incoming guard needed — `self.m = self.n + 1` after `ensures
  size <= 8` refolds [0,8]+1 into the [0..=9] target; three lib tests
  pin discharge / wide-fold refusal / intervening-call kill). The
  SYMBOLIC half of the M2 stride LANDED with gap 4b (see the blocker-2
  entry); the older note below stands only for
  (`offset + desc_size < map_size` with `desc_size >= sizeof` as a
  second lower-bound witness — needs value-vs-value coupling, R1
  remainder territory) — the LITERAL half is now fully witnessed. Then decode-minted where-facts + recast bounds discharged
  from couplings + R1/R3. ⚠️ COORDINATE: recast MECHANICS are main-lane;
  this rung supplies only the proof side. Unblocks the UEFI memory-map
  stride discharge (`map_size <= 16384` as an ensures witness feeding
  `offset + desc_size < map_size`).
- **R5 (frames):** preserve-unless-written, `stores` clause, state arrival
  facts, Houdini inference. Needed when dependent facts cross
  sibling-machine calls.

## Measured recursion — engineering (settled 2026-07-18; AMENDS the NO RECURSION directive)

Owner-settled after the proofs exploration (record:
design_briefs/mathematical_proofs.md par-2; chapters 3/8/10/18 + appendix
updated). The rule: recursive CALL cycles are legal iff `decreases`-measured
(both strata, all positions); unmeasured cycles remain the hard error;
transition loop-backs unchanged (unmeasured, constant-stack, may diverge).
`decreases` is the SOLE termination gate; RUNTIME cycles are additionally
TAIL-ONLY (owner amendment — non-tail runtime CUT, the frame-budget/
cardinality rule deleted; depth lives in explicit storage the author sizes;
proof-stratum non-tail unaffected, it never lowers). Rungs:

- **MR1 — classifier + legality gate — LANDED 2026-07-11:** the
  transition-arm spelling `-> self.own_entry(..)` on a MEASURED machine
  resolves onto the SAME loop-back edge as the bare `-> own_entry(..)`
  (state-graph targets.rs; zero new lowering — the termination pass
  already proves the decrease across that edge by symbol, and the
  loop-carried arg staging rides unchanged); unmeasured refuses naming
  both fixes. Non-tail spellings classify in validation
  (validate_self_recursive_call_positions): embedded-in-expression
  (`3 * self.f(n-1)`) names why the frame outlives the call (MR3, cut);
  a state's bare TERMINAL self-call is named TAIL-awaiting-MR2.
  machine_self_call_recursion_rejected recast to unmeasured-rejected;
  pass/calls/runtime_measured_tail_recursion_exit runs both engines;
  fail/calls/nontail_value_self_call_rejected pins the non-tail message;
  terminal_self_call_recursion_rejected re-pinned on the MR2 pointer
  (recast it into a run twin when MR2 lands).
- **MR2 — tail lowering — COMPLETE 2026-07-11:** the fall-through
  complement landed in the decreases ranking
  (patterns::fall_through_self_loop + refuted_guard_proves_positive:
  an ALWAYS self-loop dominated by guarded EXIT transitions treats each
  refuted base case `n == 0`/`n < 1`/`n <= 0` as the positivity the
  countdown proof needs), so the rewritten terminal shape now RUNS both
  engines — pass/calls/runtime_terminal_tail_recursion_exit, with
  non-decreasing/no-base-case twins refusing
  (fail/calls/terminal_tail_nondecreasing_rejected). EN-ROUTE FIX: the
  default-order inference misclassified ANY constrained scalar measure
  (`u64 [0..=100]`, `u64 in Trapping`) as a SLICE — the Constrained
  shell renders brackets and the kind probe read them as an element
  type; kinds now classify the unwrapped base type. REMAINING follow-ons
  (recorded, not blocking): the PROOF-side transition-argument
  obligations now carry `refuted_exit_guards` (prior in-state exit
  guards, call-free-gated) and the checker applies their complements
  (apply_handle_condition_complement + complement_refined_binary_range,
  landed 2026-07-11) — and VALIDATION's S4 value-env now narrows later
  statements by the NEGATED guard of every exit-if-true transition
  (fall_through_narrowed_env; unwrapped single-arm guards handled;
  refuted equality = point exclusion against the type+declared range) —
  the exact-domain terminal shape (`u64 [0..=100]`, no Trapping) runs
  both engines and the run canary is spelled that way. LANDED
  2026-07-11; the exact-domain unlock is COMPLETE. Two-state IN-MACHINE cycles LANDED 2026-07-11 (MR4's
  first sub-rung): the ranking classifies every in-cycle edge STRICT or
  NON-INCREASING (measure forwarded unchanged) and requires the
  non-strict subgraph be acyclic — every traversal then crosses a
  strict decrease; strictness on an Always edge may come from the
  param's DECLARED `[1..=N]` floor (Exact domain only). Canaries:
  pass/calls/runtime_two_state_tail_cycle_exit (the accumulator shape,
  both spellings) + fail/calls/forwarding_cycle_no_decrease_rejected.
  MR4's REMAINING core is CROSS-MACHINE mutual cycles (Q6 relaxation:
  joint measures + tail classification along the cycle; the dungeon
  find_item_at/find_item_after pair is the live test). Original rung text:** a MEASURED machine's state whose TERMINAL
  expression is a self-entry call rewrites AT PARSE onto the bare
  loop-back transition (parse_machine::rewrite_terminal_tail_self_calls;
  verified faithful by bare-spelled twin — identical outcomes), so every
  downstream pass sees the same back-edge the arm spelling rides and the
  REAL obligations surface instead of a blanket refusal. FRONTIER
  (pinned fail/calls/terminal_tail_rewrite_obligation_frontier): the
  rewritten edge is the unguarded FALL-THROUGH, and neither the
  decision-17 argument fold nor the decreases ranking uses the
  dominating base-case COMPLEMENT (`transition n == 0 { true -> exit }`
  fall-through implies n >= 1 — probed: equality and ordering spellings
  both unproven; the `_`-arm complement is equally unread). The unlock
  is one checker rung: fall-through/underscore-arm complement facts for
  transition-argument obligations + the ranking's edge decrease. When it
  lands, recast the frontier canary into a run twin. Unmeasured terminal
  calls keep a clean validation refusal naming the measure
  (terminal_self_call_recursion_rejected, recast).
- **MR3 — non-tail runtime rejection — LANDED 2026-07-11 (direct; the
  MUTUAL leg rides MR4's cycle work):** a non-tail self-recursive call
  in runtime code refuses naming the offending call and why it is not
  tail, pointing at the tail arm + explicit-storage iteration
  (fail/calls/nontail_value_self_call_rejected; statement-position keeps
  its Q7 fence wording). Mutual cycles still refuse wholesale at the Q6
  walk (unchanged until MR4's joint measures). `decreases m -> View in a..=b` stays as spelled; the
  range is a termination fact only (floor = well-foundedness bound, any
  start; dependent endpoints legal; nothing sized from a range).
  Whole-program worst-case stack line = longest chain of the
  acyclic-after-lowering call graph. Record: mathematical_proofs par-2
  amendment.
- **MR4 — mutual cycles:** joint (lexicographic) measures across the
  cycle, every call along the cycle tail-classified;
  the dungeon's find_item_at/find_item_after pair is the live test case
  (currently absorbed by bounded clone specialization).
- **MR5 — proof-stratum evaluation — LANDED 2026-07-11 (pinned; the
  machinery already composed):** measured recursion evaluates at compile
  time under the const-eval ~100k-step fuel cap — the MR1/MR2 spellings
  interpret as loop-backs with no lowering and no space rule. Pinned by
  pass/comptime/runtime_const_measured_recursion_exit: `[u8;
  table_size()]` const-calls a zero-arg machine whose measured
  tail-recursive FREE-machine helper (MR2 bare terminal form) computes
  the length.

## Math roster & the Real arc — engineering track

Owner-settled through the proofs review (record: mathematical_proofs par-6
item 3 + par-7). Proof-only is COMPUTED, never spelled: recursive data is
legal and proof-only (fixpoint: recursive, or contains a proof-only field);
no `unbounded` property exists. Rungs:

- **N1 — proof-only classification — LANDED 2026-07-11:** recursive data
  (direct + mutual, incl. generic templates) legal; the recognizer is
  `omega_typed_trees::proof_only::classify` (inline-containment cycle
  seeds + contagion fixpoint; references/slices are indirection and stop
  containment); faces: machine-runs-on-data, owned data, `contains`
  (pre-wired — the keyword is reserved but not yet parsed, no canary
  possible), state params, returns, locals, data properties (ZII),
  wire fields, runtime-data-views-proof-only-through-indirection, and
  the layout walk skips proof-only like generic templates (visit-stack
  cycle error stays as the pipeline-bug backstop). Canaries:
  pass/data/runtime_proof_only_data_declared_exit + five
  fail/data/proof_only_* + fail/wire/proof_only_wire_field_rejected;
  fail/data/recursive_data_infinite_size recast onto the consumption
  refusal (declaration is now legal). Untyped lets don't parse, so
  construction always crosses a typed face; the ch14 Equatable
  recursive-reject note is about lowering conformance and stands.
- **N2 — engine bignum (IN FLIGHT, rungs a+b LANDED 2026-07-11):**
  (a) `omega_core::bignum::BigInt` — sign-magnitude limbs, exact ring ops
  + div_rem/gcd (Rat-ready), radix parsing, decimal display, oracle-
  tested against i128; (b) the polynomial ENTAILMENT engine
  (contract_entailment.rs) widened: BigInt coefficients, exact Interval
  ends, DBM edges — no more overflow-downgrades-to-unknown; literals
  enter exactly via `IntegerLiteral::value_bignum`, and D14 gained FIRE G
  (contract facts + decreases measures bless ANY-magnitude literals —
  contracts never lower to runtime bytes, the exact engine is their one
  consumer). Canaries: pass/proofs/proof_bignum_constant_fold
  (10^22 * 10^22 == 10^44 folds exactly), fail twin
  proof_bignum_constant_false. (c) LANDED 2026-07-11: the
  obligations/checker engine widened — ProofConstraint::IntegerRange +
  the checker's IntegerRange are BigInt; the u64 range fact is the TRUE
  (0, u64::MAX), retiring a probe-confirmed UNSOUNDNESS (a plain u64
  param stored into `u64 [0..=9223372036854775807]` and runtime held
  u64::MAX — a proven fact violated; pinned
  fail/arithmetic/u64_range_fact_cap_store_rejected + the guarded-copy
  positive twin pass/arithmetic/runtime_u64_guarded_cap_store_exit).
  Same-class sentinel fabrications retired with it: Named
  non_negative/positive facts now RAISE an existing floor instead of
  minting a standalone [0, i64::MAX] claim, and extrema (min/max call)
  one-sided folds return no range instead of an i64 sentinel bound.
  Literal facts and literal `[v,v]` ranges are exact at any magnitude
  (value_bignum); binary range folds are exact (no saturation; the
  `i64::MIN / -1` bail is gone); the checker's NEUTRAL guard-refinement
  start is documented as a non-claim. (d) GATEWAY LANDED
  2026-07-11: machine-stratum contagion — a FREE machine whose signature
  mentions proof-only data is a PROOF MACHINE
  (ProofOnlyClassification::is_proof_machine, computed never spelled;
  the brief's dissolved keyword). Faces exempt (params/returns/locals);
  tail-only exempt (no frames) but recursion MUST be measured and every
  self-call must STRUCTURALLY DESCEND — the measure-position argument is
  a case-payload subterm (arm-pattern `prev` lowers to the case-tagged
  member read `n.prev`; validate_proof_machine_recursion). Pinned
  pass/proofs/runtime_nat_structural_recursion_exit + unmeasured/
  non-descending fail twins; every runtime route into a proof machine
  refuses on existing rails (locals fence, discard + purity fences;
  attached machines keep all faces). REMAINING (d cont.): the arithmetic
  bridge itself (n > 0 => n == Succ(n - 1) for INTEGER-measured
  induction consuming Nat lemmas) + ensures-extraction from proof
  machines — rides N3/N4 (extraction lemmas). EN-ROUTE FENCE 2026-07-11:
  ensures conjuncts over proof-only data refuse loudly (a false
  `result == Nat::Zero` previously compiled clean — the polynomial
  engine stood down as out-of-language, a silent false certificate;
  fail/proofs/structural_ensures_unjudged_rejected). N3's tier replaces
  the fence with real judgment.
- **N3 — fact-position operator routing (RUNG 1 LANDED 2026-07-11):**
  the STRUCTURAL mini-judge in contract_entailment.rs judges equality
  conjuncts over proof-only data — requires-equality substitution
  (symmetry/transitivity fall out), reflexivity, nullary-case
  equality/disjointness; contradictory structural hypotheses accept
  vacuously (mirrors the polynomial vacuity rule); mixed contracts keep
  integer conjuncts on the polynomial engine. Pinned
  pass/proofs/proof_nat_structural_lemmas +
  fail/proofs/nat_structural_disproof_refuted; everything beyond the
  term language still fences (structural_ensures_unjudged_rejected).
  MEASURED GRAMMAR GAP: struct literals do not parse in fact position
  (`requires Nat::Succ { prev: a } == ...` is a parse error), so payload
  constructor terms + injectivity decomposition are the next rung.
  Result binding LANDED same day for
  SOLE-unguarded-value-arm bodies (the identity lemma
  `-> (b)` / `ensures result == b` proves; wider bodies judge without
  the binding — weaker, never unsound). REMAINING: proof-machine
  application unfolding (add/mul compute-mode), guarded/multi-arm result
  binding (needs per-arm reachability), Int/Rat routing, rearrange-mode
  for declared carriers. Int introduction rule: order has no floor,
  measures stay Nat-valued or range-floored.
- **N4 — roster library (Nat ops + Seq LANDED 2026-07-11):** core
  nat.omg carries add/mul as proof machines (mul composes add by an
  ordinary cross-machine call; only self-calls need the measure), and
  seq.omg carries generic recursive `Seq<T>` — pinned
  pass/proofs/runtime_core_roster_ops_exit (core ops + a program-side
  structural length lemma over Seq<u64> into core Nat). REMAINING:
  extraction lemmas (the ensures-extraction rung — proof machines'
  ensures consumed as facts), the proof views (Seq/Bag/Range) dissolving
  from parser-known atoms into these types (the L6 bag_view rung folds
  in). Rat/Bag ship via CANONICAL-REPRESENTATIVE domains (reduced
  fractions `where gcd==1`; sorted sequences) — plain `==`, no quotient
  dependency: N4 is decoupled from N6, but those two ride R2's
  where-clauses. The `%` former is reserved for carriers with no
  computable canonical form (Real: stream equality undecidable).
- **N5 — `boundary data` + the Real axiom package:** opaque carrier;
  ensures-less boundary machines = claim-free symbols (no grant); axioms =
  accepted-tier rows; schema axioms as `<machine P>` boundary machines (one
  grant covers the schema statement). LEM ruling SETTLED: excluded middle
  = ordinary core boundary machine; nothing granted by default (templates
  carry the grant line); trust report shows classical vs constructive.
- **N6 — quotients (spelling SETTLED: `data Real = CauchySeq %
  converges_together` — bodyless data decl, `%` = the one new type
  expression; bare RHS NEVER parses — `data Meters = u32;` rejected, the
  units/provenance job belongs to empty-body domains, owner-confirmed:
  `domain u32::Meters {}` + per-operator preservation):** `as` = mk,
  carrier-only; respect-ensures gates lift; congruence over the user
  equivalence; refl/symm/trans as ordinary lemma obligations. Buckets span
  the machine-param family (the equivalence is a nested-schema machine —
  N7 customer). Record: mathematical_proofs par-7.
- **N7 — nested schemas:** machine params on proof data
  (`data CauchySeq<machine S>`) + machine-parameter signatures that
  themselves take machine parameters.
- **N8 — the construction corpus:** Cauchy Real, well-definedness, order,
  completeness; axioms retire via the standard boundary upgrade.
  LLM-parallel, zero backend contact. Universe ladder PARKED (trigger:
  full-mathlib replay as a language goal).

## Owner-gated holds (see OWNER_QUESTIONS.md)

- **Q13 console convergence** — `platform` blocks vs boundary traits (the
  console byte-op arc's last rung; the purity checker calls read_byte pure
  because platform entries carry no effect rows -- refusal-guarded today).
  Guide ch18 already PRESCRIBES the boundary-trait shape; on a ruling the
  work is the std migration.
- **FLOAT-TO-INT half still open (no ruling)** — OWNER_QUESTIONS.md item 10.
  Parked cast divergence stays in the drift ledger until answered.

## Open bugs / gaps (ungated)

- **texteq arm-locals ZII for non-terminal consumers** — root-caused
  2026-07-17, analysis pre-paid in the pin headers
  (pending/calls/texteq_local_guard_read_divergence + the arg-forward
  twin): the dispatch GUARD evaluates before any expansion's write region,
  so the fix is a per-branch-state PRE-GUARD region of call-free LocalData
  initializers (with 82a9a92d3's two load-bearing exclusions).
  Dispatch-layout surgery in M2-active machinery — the OWNING LANE's; do
  not pick up. The pinned trailing_state_mut_param divergence likely
  shares the missing region.
- **Const-folder width-blindness (latent, unreachable via the live
  spelling):** the cast-retag spelling puts a Cast node in the tree, which
  the folder's literal window refuses; the runtime operand path's
  wrapping-truncation hole is FIXED and pinned. A width-carrying folder
  (`omega-state-values/simplify/folding.rs` is i64-window by design, D14)
  remains the deeper rung, gated with the type-carrying-constants design.
- **UnloweredCaseLiteralField poison:** every known texteq shape serves
  (pinned); the poison stays as negative space — give the NEXT unloweable
  payload-field shape a fail canary when authoring surfaces one.
- **Same-type receiver aliasing:** slice 1 landed (receivers serve on both
  routes for entry-machine callers); ambiguous multi-call states stay
  fenced (fs lane). Retire
  pending/time/value_machine_receiver_field_postentry when the fence lifts.
- **Float `is_float` nested-operand markers:** not silently reachable
  (probed 2026-07-12; pinned arithmetic/runtime_float_nested_operand_exit).
  Wire the canary legs on first real reproduction.

## Platform verification sessions (host-gated)

- **Windows session — QUEUED (Next Tasks #4); one session closes all of
  it:** natively verify the fs stat-row migration; migrate
  WINDOWS_IMPORT_ROWS into provides files; Win32 rows for the no-msvcrt fs
  ops (pread/*at/link/read_dir/flock/chown/futimens/realpath — loud "no
  native lowering" refusals today); file_journal sample recheck; WndProc
  entry stubs (title-bar close); the fs<->time mtime interop leg (time-side
  surface ready + canaried; rides the stat rows). Also re-baseline the two
  cfg(windows) efi byte-pin tests (proved stale via cross-target PE
  evidence, 2026-07-17).
- **Linux session:** fs + time binding tables are structural-only until a
  host exists. Time's monotonic/wall rows additionally need a timespec
  composite lowering (clock_gettime writes {tv_sec, tv_nsec}; result =
  sec * 1e9 + nsec) — buildable now with the byte-op composite pattern,
  deferred because it would ship unverifiable.
- Dormant residual: typed machines carry no source file (fine until a
  second consumer after is_build_machine needs one).

## Programmable-layouts remainder (ch21/21/22; chapters are the spec)

- **L4 full:** derived projections into a plan-laid BYTE VIEW + the no-op
  boundary theorem — needs the L5 carrier/domain rung.
- **L5 remainder:** target-directed `encode()` (spelling open, extern brief
  §10.2), the `Packed` grammar, the plan-walking deriver (blocked on
  case-vocabulary Plan element construction), the validate/materialize
  decode mint, refinement-as-obligation.
- **RECAST (main lane, claimed 2026-07-09; the last compiler-side M2
  blocker):** rungs A/B/C1/C2 ALL LANDED — static core, interior byte
  recast, runtime-offset recast, all-scalar record views, and the
  descriptor_walk stride sample (the Cathedral memory-map pattern
  end-to-end; detail in git log + pass/recast/ canary headers). REMAINING
  tail: non-scalar-field records, `&mut` views, plan-tiling beyond
  fact-free shapes (L5).
- **L6+:** Bits placements + access classes (MMIO deriver); durability plan
  grades; publish-time predecessor diff.

## Language ergonomics

- **numeric intrinsics remainder: sin/cos — LANDED 2026-07-11** as PURE
  OMEGA (omega/language/std/math.omg): ladder range reduction (no
  float→int cast — that ruling stays open), quadrant fold, degree-15
  Horner with `let mut` accumulators (plain lets BIND-FOLD and exhaust
  the MVP scratch pools). Both engines run the identical IEEE sequence —
  bit-equal by construction; sin(1)/sin(10)/cos(1) pinned to 1e-11
  windows in pass/calls/runtime_std_math_sin_cos_exit (canary_suite +
  differential). Both measured blockers closed: (a) FLOAT binary/literal
  terminal return-writes serve (the dispatch binary-terminal path
  classifies float operands, emits WriteRuntimeStorageBinary{is_float},
  f32-narrows literal bits; float ops gated to the Add/Sub/Mul/Div set
  both encoders carry); (b) the compile-thread stack overflow on
  value-call terminals expanding a CYCLIC callee is tamed by
  MAX_BINDING_SUBSTITUTION_DEPTH (selection/bindings.rs, all three
  binding-substitution twins): self-referential binding sets now refuse
  LOUDLY instead of crashing. The residue landed same-day:
  value-call TERMINALS (always-arm transition values + trailing
  returns) now AUTO-HOIST into the let-bound spelling at
  syntax->symbol-resolved lowering (hoist_terminal_value_machine_call;
  free user calls only — host calls and guarded arms keep the honest
  fence, fail/calls/guarded_value_call_terminal_rejected) — all three
  callee classes exercised by pass/calls/runtime_value_call_terminal_exit.
  Follow-ons recorded in math.omg: Cody-Waite constants for large args,
  sub-ulp accuracy.
- **Rendering-sample sweep (R0 follow-on) — SWEPT 2026-07-11:**
  bouncing_particles dropped its flat-field sidestep (plot + render paths
  now spell `grid[b*20+a]` / `grid[ry*20+cx]` under one dominating
  compound guard — the cross-state incoming-guard route serves) and
  histogram dropped the temp-field RMW idiom for the direct
  `histogram[v] = histogram[v] + 1` (served + canaried). dungeon_render
  followed once the match-subject gap closed: the membership-subject
  hoist emitted its shared temp with the raw computed index inside,
  which the #40 fence refused — the ONE position the R0 index hoist
  missed; the mint now hoists the subject's computed index into its own
  temp first (pinned
  pass/collections/runtime_computed_index_match_subject_exit), and
  dungeon_render spells `grid[r*6+c]` directly as the match subject.

## Backend perf (deferred, post-1.0)

MVP backend (fixed-register, mem-to-mem, no regalloc/SSA/SIMD) is slow for
real-time per-pixel work; fine for demos. The "serious backend" layer waits.
Today's bar is provably correct native output. Also queued: strengthening
assigned-target allocation toward real register/stack assignment; reducing
host/runtime special-case lowering; replacing the Windows GUI sample shortcut
with a real app-window story.

## Big arcs

- **Lifetimes (decision 15):** `'name` lifetime implementation arc.
- **Ranking-view spelling** (decision 2 follow-through).
- **Wire data stage 2 remainder:** items (1) decode-side domain validation
  (both ISAs + interp) and (2) wire-schemas-as-program-types CLOSED
  2026-07-16..17 (pinned; detail in git log). OPEN, ranked: (3) runtime
  layout of wire values; (4) encoding families beyond compact_binary v0 +
  version negotiation. Probe-author note: multi-call value machines
  writing self fields trip the pinned trailing-state phase bug — inline
  states are the reliable shape.
- **Versioned data stage 3:** the era tag itself (+ decision 10's wire-era
  ride), era-tagged containers, migration chains / `replaces` / quiescence.
- **Equatable synthesis:** a CALLABLE conformance surface is still open.
- **Trailing-state stale reads of threaded `&mut` param fields:** pinned
  (pending/calls/trailing_state_mut_param_phase_divergence, 71/Exit(70));
  cross-state phase allocation for the threaded &mut — the fs lane's
  claimed receiver-phase family, theirs to absorb with the aliasing arc.
- **Concurrency model:** chapter 18 is a sketch; per-target declarations.
- **Atomics remainder** beyond the landed stage-1 ops + memory model.
- **Separate compilation / component artifact model.**
- **Freestanding target + hardware vocabulary.**
- **Build-time evaluation:** comptime eval + trait generators (effect-free
  machines in value/refinement position).
- **Generics completion:** stage-1 data monomorphization landed; machines/
  traits remainder.
- **Allocator story:** `Vec` has no runtime; `alloc` is an effect name only.
- **Repr control** for hardware structures (packed, explicit).
- **Proof engine arcs** beyond L7 induction.
- **Hot-swap semantics:** quiescence proofs, borrows as swap barriers.
- **Wire encoding families + negotiation** (beyond stage-2 encoders).
- **Serialized capabilities:** attenuation + revocability across boundaries.
- **Text/string proof domains:** `String::Utf8`/`NoNul` as first-class
  domains.
- **KILL builtin `string`/`String` (Zach: "how is this not retired yet").**
  Text is `[u8] in <encoding domain>`. Blocked on the mint being real:
  comptime-eval in value/refinement position + the loop-invariant prover for
  the runtime case. Then sweep ~185 files + ~57 canaries + the dungeon,
  delete `PrimitiveType::String` + ~16 backend special-cases, retire the
  keyword. Recipe: wiki/architecture/string_retirement_execution.md. The
  capstone of the encoding-domains arc — NOT a background-tick item.
- **Trust system engineering (design settled through the proofs arc):**
  boundary machines + grants + the unified lockfile (trust receipts beside
  package pins), engine veto, trust report, oracle tripwires, `defer`
  tooling (site marker + root row from one command, hash-pinned,
  package-release-fatal), grant locality (own-package dev-active w/
  warning; package boundaries inert until root-granted). Record: ch10
  Evidence And Trust + mathematical_proofs par-4/par-6. No rungs cut yet —
  ladder it when a lane picks it up.

## Structural follow-ups (surface landed; semantics pending)

- **Inline asm:** only `asm { jmp state(...) }`; labels/back-edges rejected;
  mnemonics, register constraints, clobbers, `asm where` contracts pending.
- **Transition data-patterns:** guard-lowering only; real pattern binding,
  multi-subject validation, domain-pattern proofs, diagnostics pending.
- **Const data parameters:** symbolic lengths flow structurally;
  instantiation-time substitution, validation, layout diagnostics, const-fact
  proof integration pending.
- **Host providers:** rows parse + snapshot; registry validation, target
  whitelisting, syscall/import lowering, boundary report pending.
- **Trait defaults (`default machine`):** marker + body parse; conformance,
  reuse, override rules, dispatch pending.
- **Dynamic traits (`dyn Trait`):** structural + fat descriptor; construction,
  vtable emission, dispatch lowering, object-safety validation pending.
  NOTE from the proofs arc: dyn descriptors must carry satisfier identity
  (`as &dyn Card::PowerOrder` decays to `&dyn Trait`; ch14).
- **Relax surface removal (relax RETIRED 2026-07-17):** superseded by
  invariant windows (ch11). Remove the parsed `relax` surface
  (parser/statement.rs, type_reference.rs) + the relax canaries
  (canaries/pass/relax/*) + any corpus uses, replacing with plain writes —
  a deliberate compiler pass, coordinate with active lanes.

## Vertical slices

- **Vec[T]:** owned dynamic storage with length/capacity (surface declared;
  storage/lowering pending; allocator-story dependent).
- **as_slice/as_mut_slice:** back with real boundary-primitive storage.
- **Ownership events:** continue appending transfer/drop events from the
  remaining ownership forms; lower abstract summaries into explicit backend
  transfer ops.
