> `OWNER_QUESTIONS.md` contains unresolved owner decisions only. Settled
> language rulings live in the guide/briefs; this file tracks engineering.

# Tasks

Engineering ledger and working backlog. Completion notes remain only where they
explain dependencies or migration state; detailed history belongs in git and
canary headers. Condensed 2026-07-12, 2026-07-18, and 2026-07-20.

## Current Strategic Focus

Omega's first real consumer is the Cathedral OS (`../Cathedral`). The gap
analysis lives in [wiki/cathedral_alignment.md](wiki/cathedral_alignment.md).
Current critical gaps are programmable layouts, freestanding entry/hardware
vocabulary, atomics/scheduling, and separately compiled component replacement.

## NEXT TASKS — design-unblocked, agent-ready

Claim an item, work its rungs in order, canaries per rung, push per rung.
(Detailed landed histories live in git log and the per-canary headers; this
section keeps rulings, current state, and open work only.)

**Termination surface TPR1→TPR6 (decision 23) — TPR1/2/3/5 LANDED.**
`terminates;` / `terminates by <subjects> [-> View] [in <range>];` is the
whole surface (block form + standalone `decreases` retired, loud
diagnostics); MachineTerminationPlan splits the published guarantee from the
private RankingWitness; the checker gates on the plan with canonical-default
elaboration; the full corpus was migrated atomically. TPR4 landed trait
requirements + conformance inheritance; REMAINING TPR4: published
omission/default rules for EXPORTS (needs artifact serialization); sealed
progress profiles are DESIGN-RULED (a profile is a sealed semantic domain
over a boundary-provider capability, admitted through the ch10 carrier —
never flow-inferred, never an entailment relation) with engineering pending:
profile-domain resolution, grant-backed admission, receipts, pinned premises
(rides GR6b below).

1. **Math roster N1→N4 — LANDED through the Rat carrier** (details in the
   Math roster section below). Open above the carrier: divisibility theory
   (demand-gated) and the N5–N7 rungs (gated on MP6).
2. **Measured recursion MR1–MR5 — LANDED**, including the MR4 cross-machine
   tail-cycle admission (call_cycles admission + v1 decrease prover +
   interpreter tail jumps; pass/calls/mutual_cycle_tail_admitted_exit).
   Richer guards / multi-subject lexicographic cycles are demand-gated.
3. **Dependent types R2 — LANDED** (section below).
4. **Windows platform-verification — HOST CHECKLIST COMPLETE**; remaining
   items are host-gated (see Platform verification sessions).
5. **Constant model (CM1/CM2/CM3) — LANDED**: two-phase law, landed-type
   folding at every face, metadata-carrying constant landings; the
   static-table carrier remains REPRO-GATED (no live repro; CR3 decision
   recorded).
6. **Place algebra — COMPLETE**: Copy* 18→1 (CopyPlaces), integer writes
   7→1, binary writes 6→1, text writes 7→2 (WritePlaceString/
   WritePlaceBoundedBuffer), address writes collapsed, guards/operands
   consume Places, op-set shrink landed — 38+ variants retired onto the
   place-shaped survivors; dead shells swept. Durable lessons: this
   aarch64-darwin host is the only runtime leg (x86-only mispatches are
   invisible — pin bytes structurally); when a shared offset constant
   moves, every consuming kind delegates in the SAME commit; battery
   results gate commits inside one script.
7. **Semantic taxonomy rework (STR1–STR7)** — STR1 pins, STR2 core enums,
   STR4 (kinded effect rows + declaration-free inference + checked plans:
   SemanticDomainTable, qualification facts, declared-domain casts, the
   mint v1) and contract-plan fingerprinting (surface + facts + positional
   normalization) are LANDED. PERMISSION PLANS RULED 2026-07-17: validate
   structural facts freely; admit semantic commitments through the ch10
   grant/receipt carrier; selection is a slot-owner capability (rides the
   PRV ladder). REMAINING: the boundary-facing calling plan; STR3
   termination-plan tail; STR5 validation/resolution; STR6 lower only from
   checked selections; STR7 retire the compatibility bools.

## Cathedral M2 (owner priority 2026-07-15; RECAST = main lane, claimed)

**SUPERSEDED 2026-07-13 (compiler lane): the 2026-07-11 "M2 BOOTS" below was
a FALSE POSITIVE — and M2 + M3's serial rung are now REALLY done (Omega
`7640a6f7a`, Cathedral `69051cc`).** The 07-11 boot idled without owning
anything: binding boot services as This-call vtable fields shifted every
argument one register right (EFI TABLE SERVICES take no This — protocols
do), so GetMemoryMap read the table pointer as *MemoryMapSize (the header
signature) and sprayed the map over whatever RDX aliased; the post-call
sanity guard then silently failed into idle — booting + halting was
indistinguishable from success. Four stacked fixes landed: (1) the
`TableFunction(field)` Binding case (extern brief SS12.1 addendum) — table
dispatch with the table OFF the wire, mechanism carries the declared arity;
(2) result-carrying field-model calls — the prepended result place had been
marshalled as the receiver (RCX=0 → fn ptr from phys 0x38 → the recurring
#UD at RIP 0xB0000); (3) wide-referee borrow-recast lets store the ELEMENT
ADDRESS (new WriteRuntimeMachineIndexedAddressToRuntimeFrame; the
referee-size rule now governs the WRITE side too; Named referees only —
slice {ptr,len} descriptors stay flat, caught by the utf8 canaries); (4)
reads THROUGH the recast pointer: guard clauses that cannot express the
deref re-select through the expression path's Pointee operands, and
transition arguments stop inline-folding recast locals (they were silently
dropped). Verified live under QEMU/OVMF with POSITIVE evidence: firmware
out-values echoed over debugcon (status 0, stride 48, version 1),
98-descriptor walk, ExitBootServices gated on its captured status, first
Region mint, "Owned 35 MiB" over Cathedral's own 16550 driver (LSR-polled
FIFO bursts, decimal by place subtraction), hlt idle; 842 canaries green,
the 7 known reds, zero regressions. Trust only positive-evidence
verification on boot claims. The gap list below stands as history.

## Cathedral M3 completion + M4 runway (queued 2026-07-20, compiler lane)

M3's serial rung is done (above). The remaining M3 rung is the TIMER TICK;
then M4 (scheduler/IPC) opens. Atomics already serve — `fetch_add`/
`compare_exchange` lower to real `lock xadd`/`lock cmpxchg` (width-
dispatched, canary-pinned); the cathedral_alignment RMW-blocker note was
stale and is corrected (`174ddbaf4`).

1. **InterruptFrame inbound plan** — the second stated convention
   (calling_plans.md: `boundary(InterruptFrame) machine on_timer(...)`;
   freestanding brief "Interrupt entry"). Entry stub: CPU-pushed frame
   (SS:RSP, RFLAGS, CS, RIP, ± error code), full register save/restore,
   `iretq` return. The declaration surface is settled; the concrete plan
   spec + acknowledgement-token types are the freestanding brief's named
   Still-open residue — the register-file layout half is architectural
   fact, agent-ready; the ack-token TYPE wants an owner glance when the
   LAPIC shape lands.
2. **Handler-address registration** — the IDT is a stated-layout data
   struct whose entries need a `boundary(InterruptFrame)` machine's
   ENTRY-STUB ADDRESS as a runtime value ("interrupt entry registrations"
   in the freestanding brief's required build report). The registration
   grammar rides the brief's "exact build/entry declaration grammar"
   Still-open item — flagged here so the timer tick does not discover it
   late.
3. **`cli`/`sti`/`lidt` known-contract asm** — the hlt pattern
   (machine_control; encoders 0xFA / 0xFB / 0F 01 /3 with a memory
   operand for the descriptor pointer).
4. **Cathedral timer-tick acceptance** — IDT + PIT (or LAPIC timer)
   programming via the landed port I/O; the handler posts a bounded tick
   event; done-check: tick count reported over the owned serial line under
   QEMU, hlt between ticks.
5. **Compiler gaps filed en route (none M4-blocking):** (a) a
   binary-initialized local consumed as a STATE-CALL argument inside a
   proof-obligation-carrying guarded state refuses ("CallArgument binary
   expression needs runtime value lowering"; ANY operator, division was a
   misdiagnosis — same fold-vs-place family as the recast-arg fix; make
   the state-values fold copy the local's slot); (b) assignment-RHS
   `self.x = ref.field` and `&self.field` address-of call arguments
   resolve INLINE for large referees; (c) the red value_call_terminal
   canary will block the first machine-call on a boot path — triage
   before M4's scheduler shapes lean on machine calls.
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
   expressible. Design record: the `Binding`/provider model in
   `design_briefs/extern_boundary_and_format_domains.md`.
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
5. **x86_64 record-view read encoder — LANDED 2026-07-11 (9ea55a5ca);
   ⇒ Cathedral M2 BOOTS.** CopyRuntimeMachineIndexedToRuntimeStorage now
   handles byte_count outside {1,4,8} (the C2 record-view snapshot): the
   single-value path is byte-identical, the chunked path keeps the source
   in r15, loads the target base into r10, and copies in 8/4/1-byte pairs
   via the same alignment-aware decomposition as aarch64's
   for_each_runtime_copy_chunk; the width fn + relocation target-offset
   helpers thread byte_count. Proven: descriptor_walk (16-byte all-8s,
   the in-tree twin) runs native==interp==50 on x86_64; four unit tests
   pin width/emitter lockstep, the 8/4/1 decomposition, the exact
   4-byte-tail opcodes, and single-value invariance. **The full Cathedral
   M2 tree cross-compiles for uefi_x64 AND BOOTS under QEMU/OVMF
   (2026-07-11): greeting prints, firmware never regains control (no
   UiApp fall-through, unlike M1), no triple-fault — the app entered
   own_machine, walked the map, ExitBootServices, and idles. The machine
   is ours.** (40-byte EfiMemoryDescriptor is 5x8, no tail; the tail path
   is unit-tested since the frontend specializes small-trip .omg loops to
   fixed copies before the op — see the found-bug below.) Spelling notes
   that stand for the corpus: guard conjuncts must be INLINE transition
   subjects (let-bound bools hide them from display matching), and the
   R1a declared-range edge-discharge cannot yet read compound guard
   displays (`offset + desc_size <= K`) — the guard-route meet can; a
   redundant literal conjunct bridges it.

**FOUND BUG (owner lane, 2026-07-11, not M2-blocking): a recast at a
COMPILE-TIME-CONSTANT offset stale-reads the zero-init image.** `let r:
&Rec = &self.buf[K] as &Rec` with `K` a constant (e.g. entering a state
with a literal offset) const-folds the field reads against the STATIC
(ZII) buffer, ignoring runtime writes to `self.buf` — native returns 0,
interp returns the written value (a silent divergence). Reproduced
minimizing an M2-shape canary; does NOT affect M2 (the walk's offset is
genuinely runtime, so it uses the real runtime-indexed op — descriptor_walk
and the booting Cathedral image both confirm). Adjacent: a runtime-offset
recast loop with a statically-small trip count (2 records) SPECIALIZES to
fixed-offset copies at the WRONG displacements (read [+0] and [+16] for
records at 0 and 12), also diverging native 0. Both are frontend
recast-lowering / const-fold issues, not backend. No fail canary filed yet
(the frontend keeps folding the repro away — needs a genuinely
runtime-but-small offset shape to pin).

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
- **R2 (where-clause + gating + windows) — LANDED end-to-end** (rungs 1–3,
  slices 1–11, plus the refinements: multi-state callee summaries,
  transitive field-vs-field hypotheses, window transport across states).
  The model: `data ... where <facts>` declares the default domain;
  construction is gated (literals prove it, prover-backed interval folding
  included); constrained-field writes re-prove; unestablished reads refuse;
  establishment runs as a fixpoint over the state graph with cross-state
  valuation/window transport and cross-machine callee summaries; readers'
  hypotheses refine intervals (incl. product hypotheses count*stride <=
  len). SOUNDNESS anchor: untracked fields read 0 only in the boot state.
  Pins live under pass/dependent/ + fail/dependent/ (see canary headers).

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

## Ranked termination — landed substrate

Normative record: decision 23,
`design_briefs/termination_ranking_and_progress.md`; chapters 3/9/10/18.
Terminating state and call cycles require a joint well-founded ranking;
productive state loops may diverge without one. Runtime recursive calls
remain tail-only; proof-stratum non-tail recursion never lowers.

**MR1–MR5 LANDED** (detail in git log): MR1 classifier + legality gate
(`-> self.own_entry(..)` on a measured machine = the sanctioned tail
spelling); MR2 terminal tail rewrite (parse-time loop-back, incl. the
complement-route frontier; pin runtime_terminal_tail_recursion_exit); MR3
constant-stack lowering; MR4 in-machine two-state cycles AND the
cross-machine tail-cycle admission (the program is ONE dispatch loop —
every transition arm target is a SetDispatchState jump over one overlaid
frame region, so admitted cycles run on constant stack; call_cycles.rs
admits all-tail + all-measured + per-edge proven decrease; interpreter
mirrors with constant-depth tail rebinds; pins
mutual_cycle_tail_admitted_exit / mutual_cycle_decrease_unproven /
mutual_cycle_disqualified_shape); MR5 proof-stratum const-eval under the
~100k-step fuel cap (pin runtime_const_measured_recursion_exit).
TPR1–TPR5 migrated the surface (see NEXT TASKS).


## Math roster & the Real arc — engineering track

Design record: mathematical_proofs “Quantification and proof data” and
“Real-number direction”. Proof-only is COMPUTED, never spelled: recursive data is
legal and proof-only (fixpoint: recursive, or contains a proof-only field);
no `unbounded` property exists. Rungs:

- **N1–N4 — LANDED (the full ladder to the Rat carrier).** N1 proof-only
  classification (recursive data legal, proof-only, layout skips it); N2
  engine bignum rungs a+b; N2(d) integer bridge DEFERRED (nonblocking
  research); N3 fact-position operator routing rung 1; N4: the Nat zoo (27
  machines) — add/mul laws, cancellation, requires-bearing induction, the
  monus development (pred/sub + 5 lemmas), ORDER-AS-MONUS (`a <= b` :=
  sub(a,b)==Zero; `a < b` := sub(Succ a, b)==Zero) with the strict-ranking
  evidence (sub_le, sub_lt_of_le, sub_lt, sub_lt_succ), computed-subject
  termination (cited syntactic route + the judged route
  proof_edge_strict_decrease_judged), mod carrying `ensures result < b`,
  Euclid's gcd, div, Int (difference pairs, CommutativeSemiring), Seq/Bag
  lemmas, and omega/language/core/rat.omg (canonical-representative Rat:
  mk_rat reduces by cited gcd; rat_eq = cross-multiplication). Key pins:
  runtime_core_nat_declared_exit, runtime_core_rat_declared_exit
  (dual-engine), the proofs false-twin corpus, and
  computed_edge_positivity_missing.
  JUDGE EXTENSIONS that carried it (all conservative, each with a false
  twin): constructor-clash vacuity (premise judged REFUTED under the bare
  case hypothesis closes the arm; judged BEFORE intake so the premise's own
  rewrite cannot mask the clash); arm-refined citation discharge; IH-enriched
  two-pass citations; computed transition subjects as intaken arm EQUATIONS
  (StructuralCaseArm.case_equation) instead of substitutions.
  PROOF-SHAPE DISCIPLINE (durable authoring rules): every compute machine
  stays single-transition so lemma facts unfold; a lemma's self-call arm
  rides a SUB-STATE (one hop; the recognizer walks root arms → one flat
  sub-state ending in a Value terminal); double destructure is avoided by
  flat lemma chains (the nested-arm recognizer extension was proved
  unnecessary); spell witnesses as expressions (pred(sub(b, a))) rather
  than payload bindings.
  OPEN above the carrier: DIVISIBILITY THEORY (gcd_pos, gcd_dvd_left/right,
  div_mul_cancel — would discharge mk_rat's positivity internally and prove
  idempotence) — demand-gated; reducedness-by-type waits on the N6 quotient
  former.
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
  N7 customer). Record: mathematical_proofs “Real-number direction”.
- **N7 — nested schemas:** machine params on proof data
  (`data CauchySeq<machine S>`) + machine-parameter signatures that
  themselves take machine parameters.
- **N8 — the construction corpus:** Cauchy Real, well-definedness, order,
  completeness; axioms retire via the standard boundary upgrade.
  LLM-parallel, zero backend contact. Universe ladder PARKED (trigger:
  full-mathlib replay as a language goal).

## Float semantics — engineering track (design settled 2026-07-18)

Record: design_briefs/float_semantics.md; UX: ch5 Float Facts. Zero new
keywords — value/policy domains + Rat const-eval + satisfiers + provider-plan
rows. Rungs:

- **F1 — policy-domain validation: LANDED 2026-07-18.** `Wrapping` on a
  float = hard compile error ("no modular reading of a float");
  `Saturating`/`Trapping` on floats = recognized policies that refuse
  loudly (not lowered until F5) instead of silently no-opping; integers
  unchanged. Replaced the type_references.rs no-op arm
  (`ArithmeticDomain(_) => {}`). Fail canaries
  arithmetic/float_{wrapping,saturating}_domain_rejected. Cleanup: the
  orphaned bounded_float family (pass/arithmetic/bounded_float +
  fail/arithmetic/bounded_float_call_unproven; 0 rust refs, dead `0.0f`
  syntax) RETIRED. STILL STALE (different family, constraint machinery,
  not F1 -- flagged for that lane): fail/constraints/invariant_{unknown_
  constraint,recursive,mutual_recursion} carry the same dead `0.0f` and
  are unregistered.
- **F2 — exact literal/const pipeline — a/b LANDED, one leftover.** Float
  literals parse to ONE shared exact text carrier with per-format landing;
  compile-time float arithmetic = exact Rat + round once per op at the
  LANDED width (guard trees, arg faces, nested operands — the f32 chain
  per-op rounding pins the 2^24 plateau, dual-engine differential).
  REMAINING (F2c): value-machine CALL-statement args (cross-machine
  argument faces still land at the old width in one spot — see the pin
  headers under pass/float/).
- **F3 — `Finite` core domain:** promote ch5's `finite`; window
  enforcement; ranges-imply-Finite in the prover. std `is_finite`
  LANDED 2026-07-14 (omega/language/std/math.omg, the hoisted-let
  spelling `let d = x - x; transition d == 0.0`; `x != x` stays the
  IEEE-binding idiom underneath, never in the grammar) --
  pass/float/runtime_std_is_finite_exit pins all three legs
  (finite/inf/NaN) dual-engine. The whole value-call float arc that
  blocked it is CLOSED: bool + float returns deliver
  (pass/calls/bool_value_call_return_exit,
  float_value_call_return_exit), inlined float-local guards lower
  (pass/float/expansion_float_local_guard_exit), and the last face --
  RUNTIME float-local ARGS -- fell 2026-07-14 to the ARM-GUARD FAILURE
  DISTANCE fix: the pin's every emitted op was statically correct
  (three digs: args deliver real bits, the substituted
  `d = (a/b) - (a/b)` chain computes NaN, the guard has the jp
  NaN-parity branch), but an inlined multi-arm transition's compare
  took the STATE-guard failure convention (next dispatch action = the
  caller's failure trailer) and sailed past its emitted-but-orphaned
  no() arm. byte_distance_to_next_dispatch_action_end
  (machine-emission branch_distances/dispatch.rs) now stops early at
  the arm's ForwardBranchSkip (leaf-arm-only marker), landing failure
  on the sibling arm's first byte; state guards never meet a skip
  before their dispatch action and keep the old target. Promoted pin:
  pass/calls/float_value_call_runtime_arg_exit. Integer twins never
  showed it because constant args let the guard fold statically.
  KEPT from the arc: the float-arithmetic-initializer vanish-guard
  (storage_blockers, both paths, census-calibrated: calls/places and
  integer arith exempt) -- zero corpus fires; planner refusals of the
  float shape stay LOUD. DEAD ENDS (don't re-chase): three-door
  "refuse nested floats" (broke 8 green canaries; nested float chains
  are fully supported), bindings.rs "keep computed float locals
  slotted" (the inliner's own capture drives the path). Inline
  guard-position float arith stays loudly fenced (unchanged). Window
  enforcement additionally waits on the invariant-window machinery
  (unbuilt).
- **F4 — float→int proof-or-policy cast — COMPLETE (all targets).**
  `in Wrapping` float→int REJECTED (no modular reading of a float);
  Exact = proven-range obligation (NaN excluded via `x == x`);
  Saturating = clamp with NaN→0; Trapping = pre-conversion guard on the
  ACTUAL target range; `target_signed` travels through conversions and
  nested operands on both ISAs; interp preserves u64 above i64::MAX.
  Pins: float_to_int_unsigned_narrow_saturating_exit,
  trapping_float_to_narrow_int_cast_traps, float_cast_wrapping_rejected.
- **F5 — policy lowering: COMPLETE 2026-07-17 (both native lanes).** Float
  `Saturating` clamps MAGNITUDE OVERFLOW to ±MAX_FINITE at the landed
  width and nothing else (div-by-zero/invalid keep their non-finites,
  per the brief — 0/0 has no defensible clamp); `Trapping` traps on
  invalid (NaN-from-non-NaN), overflow, and div-by-zero. Interp:
  eval_float_binary's domain arms. aarch64:
  float_policy_guard_bytes — ALL-INTEGER classification (sign-cleared
  bits vs the format's Inf pattern: LO/EQ/HI = finite/Inf/NaN in ONE
  compare; the raw operand bits stay live in the GPRs), patched-branch
  assembly, the Saturating MAX_FINITE|sign clamp vs the Trapping brk,
  the Divide zero-divisor carve-out; NEW encoders
  encode_and_x_low_ones + encode_and_{w,x}_top_bit; the WIDTH is the
  emitter's own length (fixed-register call + .len(), the rung-2a
  one-source-of-truth discipline — no lockstep constant). The
  validation fence LIFTED (the old float_saturating_domain_rejected
  fail canary retired with it). X86 F5 LANDED 2026-07-17: the same
  integer-bit classification rides r8/r9/rax around the SSE result in
  r10, emitted branch targets define their own width, and nested Binary
  operands now retain their arithmetic domain instead of silently taking
  Exact's unguarded path. Pinned on both hosts + interpreter:
  pass/arithmetic/float_saturating_overflow_exit (positive/negative f64,
  nested f64/f32, re-clamp idempotence, 5/0 keeps +Inf) plus abort-style
  float_trapping_{overflow,divzero,invalid}_traps; both ISA suites pin
  encoder/width lockstep.
- **F6 — TotalOrder named satisfiers** for f32/f64 (sign-magnitude
  integer compare) once satisfier machinery lands.
- **F7 — format records in omega::core + Float ProviderPlan bindings:** needs the
  `Instruction` arm of the Binding sum (new machinery). Today's hardcoded
  IEEE lowering IS the built-in binding — formalization, not a blocker.
- **Cleanup — DONE 2026-07-16:** the bounded_float pass/fail canaries were
  already retired (directories gone); the three fail/constraints/
  invariant_* carriers still spelled the stale `0.0f` suffix — respelled
  `0.0`; each still rejects with its intended invariant diagnostic
  (fragment-matched by the fail gate).

Micro-decisions ALL SETTLED (owner, 2026-07-18; record: float brief §8 +
ch5): min/max = hardware contract, documented (order-dependent under NaN;
Finite makes it unobservable in proven code); Saturating = overflow-only
(div-by-zero/invalid ops stay Finite obligations; `Finite & Saturating`
composes; at most one policy per `&` chain); shift-count = proof-or-policy
(Exact proves count < width, Wrapping masks, Trapping traps, literal OOR
rejects — an INTEGER ruling, engineering rides this ladder as F8).

- **F8 — shift-count ruling — COMPLETE** (Exact obligation count < width;
  Wrapping masks the count at width on all three engines; Trapping traps
  the COUNT value-blind before the shift). Pins:
  runtime_shift_subword_masked_count_exit, trapping_shift_count_traps,
  shift_count_* fail family. Known residual (out of scope, value face):
  the state-values folder folds a constant Trapping `<<` VALUE overflow
  with wrap semantics while the runtime traps — ruled by the F5
  value-face work when it lands.

## Settled rulings with remaining engineering

- **Dead trapping computation — RULED + LANDED: a trap is OBSERVABLE,
  never dead** (the durable content of abort-as-effect #65; failure/control is
  separate from decision 22's reach/operational row). A Trapping op "actually
  traps on paper — it's not dead code anymore"; the backend may lower to
  any trap-EQUIVALENT effect but never to silence, and NO contract
  auto-narrowing ever (boundary signatures are the truth as written).
  Landed: the storage layer keeps trap-carrying initializers' slots
  (initializer_carries_trapping_arithmetic, both the destination-declared
  and operand-declared faces); pinned
  pass/expressions/dead_trapping_let_traps (both engines abort).
  The always-traps WARNING landed same day (S3's interval machinery:
  a Trapping op whose result interval is provably DISJOINT from its
  type's range warns "traps unconditionally at runtime"; warning, not
  error, per owner — future unused-var warnings live in the same
  family).

- **ProviderPlan + Console boundary migration (PRV1–PRV4) — RULED; PRV1–3
  LANDED; the P4 flip RULED 2026-07-20 with a strict order.**
  LANDED: PRV1 the typed carrier (omega-effects provider_plan.rs:
  ServiceSchema/ProviderBinding incl. the HostOperations sequence arm +
  reserved Instruction arm/ProviderPlanRow with rendered call_shape/
  ProviderPlan — no trust field, classification is admission output);
  PRV2 ServiceSchema::from_typed + identity_fingerprint (FNV over the
  canonical rendering, presentation-invariant) + validate_against_schema
  (named errors) + the render/parse call-shape pair co-located with
  PlatformCallData (exact-inverse round trip pinned); PRV3 provides
  blocks derive plans -> trust-report rows with fingerprints; a grant
  naming a plan pins the fingerprint as its lockfile receipt (drift
  refuses; pinned). P4a: the LOSSLESS ORACLE (builtin_console_plan
  round-trips all three populate tables' Console rows exactly —
  discriminated by the lowering's platform field) + platform Console
  PROMOTED to boundary trait Console with per-method effects (battery
  green unchanged); platform blocks RETIRED (directed diagnostic +
  pin; syntax + dead-carrier machinery fully swept; the keyword token
  survives only for the diagnostic).
  THE FLIP RULING (do not reorder): ProviderPlans are DERIVED artifacts,
  never authored rows. Composite lowerings become checked machines with
  explicit `satisfies`; irreducible leaves use
  `satisfies Requirement via <Binding>` (compile-time evaluable,
  normalized); the satisfied requirement supplies the contract/effect
  ceiling; validation/admission checks refinement and assigns trust;
  target packages declare default provider types; slot owners select
  defaults or override a slot by type. No plan-builder API, call-shape
  DSL, authored effect_set, or structurally inferred conformance.
  IMPLEMENTATION ORDER: (1) `via` parser/tree + ExternalRealization
  supply variant feeding the existing lowering — SURFACE LANDED
  2026-07-20: `satisfies Requirement via <Binding>` parses (the
  provides RHS grammar reused verbatim), threads syntax→resolved→
  typed (TraitConformance.via = the NORMALIZED rendering;
  HostProviderMappingKind::normalized_rendering is the one spelling
  per value), and a bodyless non-boundary machine with a via clause
  populates MachineSupplyMode::ExternalRealization { binding } (the
  ExternalBindingTable interner in omega-core; contract-plan
  fingerprints fold the binding id). The item parser admits exactly
  this bodyless shape (pin: pass/providers/external_leaf_via_compile).
  REFUSAL RUNGS LANDED 2026-07-20: a via clause that is not exactly
  the external-leaf shape refuses (via+body, via+axiom, multi-via --
  never a silent drop; pins providers/via_with_body_rejected +
  via_on_axiom_rejected). STEP (1) COMPLETE 2026-07-20 -- MERGE FEED
  LANDED: external leaves derive ProvidesRow-equivalents in
  extract_provides_rows (requirement-named satisfies + the structured
  via binding; a <target>-scoped leaf rides its marker, unscoped
  rides the portable name; VtableField/TableFunction leaves wait for
  the leaf over-struct surface); from_normalized_rendering is
  normalized_rendering's exact inverse; the empty-body check exempts
  leaves (the binding IS the body); leaf rows hit the SAME collision
  rule as authored rows (extend, never override -- the via_compile
  canary decollided onto a custom capability and RUNS exit 70).
  PARITY PROVEN: a leaf DllImport row lowers identically to an
  authored provides row (external_leaf_dllimport_compile).
  SURFACED PRE-EXISTING GAP: the hosted general-import call path
  loses its ARGUMENT (exit(70) reaches libSystem as 0) for authored
  rows and leaves alike -- filed as
  pending/providers/import_call_argument_lost; runtime exit pins
  land with that fix. STEP (2) CORE LANDED
  2026-07-20: derive_satisfies_plans assembles one plan per
  (boundary trait, target) from external leaves -- rows from the
  structured via bindings, schema from the typed trait, the effect
  surface = the union of the SATISFIED requirements' declared
  effects (the requirement is the ceiling, never the leaf), and the
  trust rows now show COVERAGE n/m against the schema (pinned:
  satisfies_leaves_derive_a_covered_plan -- one of two requirements
  covered reads 1/2). Signature refinement was already enforced
  per-edge by the conformance checker. SELECTION V1 LANDED
  2026-07-20: validate_slot_selection refuses two FULLY COVERING
  plans on the SELECTED target (implicit selection is only ever
  unique; both plans named with fingerprints; partial coverage never
  collides). Target INERTNESS preserved: non-selected targets' plans
  never participate (the fail-canary host-portability convention).
  Pinned: fail/providers/slot_plan_ambiguous (an authored block +
  a portable leaf covering one slot). ADAPTERS JOINED 2026-07-20 -- STEP (2)
  COMPLETE: a checked machine with a requirement-named satisfies
  edge (no via) derives a CheckedAdapter plan row over a BOUNDARY
  trait (plain traits stay the existing conformance machinery's
  business -- the decision-20 fixtures pin that split), and
  ADMISSION-AS-REFINEMENT enforces the effect ceiling: the adapter's
  TRANSITIVE effects must fit inside the satisfied requirement's
  declared effects, else a loud hidden-effect refusal (pins:
  pass/providers/adapter_satisfies_compile,
  fail/providers/adapter_hidden_effect). The build.omg per-slot
  override spelling rides the target-package surface (step 4). IMPORT-ARGUMENT GAP FIXED
  2026-07-20 (was pending/providers/import_call_argument_lost, the
  step-3 blocker): authored/leaf imports on hosted aarch64 routed by
  the capability-keyed returns_value() catalog to the NON-returning
  sequence -- the prepended result place marshaled into x0 and every
  argument shifted. Authored imports (Custom/Unknown capability +
  Import mechanism) now route to the VALUE-RETURNING sequence at the
  mechanism-aware emission dispatch, and BOTH relocation walkers (the
  BL offset in external_calls.rs and the operand data-address fixups
  in data_addresses.rs) carry the same authored_import flag so the
  layout and its fixups can never diverge. exit(70) reaches libSystem
  as 70 -- promoted to pass/providers/runtime_import_call_argument_
  exit (native-exit suite assert; interpreter serving of custom
  imports is its own rung). STEP (3) UNBLOCKED. Still surfaced:
  authored/leaf Syscall rows refuse at merge ("not wired yet" --
  pre-existing, documented refusal); (2) derive plan
  coverage/signatures/effects/dependency closure from satisfies edges +
  target-default and type-per-slot selection; (3) replace Console
  HostOperations/call-shape rows with checked Omega adapters under the
  lossless oracle -- SHAPE ANALYSIS (2026-07-20, load-bearing): the
  INTERPRETER can never serve ARBITRARY authored imports (dlsym-ing
  user symbols from the evaluator is unsound; authored-binding calls
  refuse there today, differential-excluded), so Console adapters
  must bottom out in ops BOTH engines serve -- a RAW-STDIO std
  boundary trait whose methods are SEMANTIC operations (the existing
  byte-io composites natively; the existing console serving in the
  interpreter), NOT authored import leaves. ADAPTER DISPATCH LANDED
  2026-07-20 (the consumption rung 3a rides on): a call through a
  field whose declared type is a BOUNDARY trait rewrites -- in BOTH
  engine pipelines, before checking (pipeline/adapter_dispatch.rs,
  the const-lengths/MP4 rewrite family) -- to a direct call to the
  UNIQUE checked adapter satisfying that requirement (statement AND
  value calls; two adapters for one requirement refuse; no adapter =
  the host route untouched; v1 adapters are FREE machines, the field
  is dispatch-only). Pinned DUAL-ENGINE:
  runtime_adapter_dispatch_exit (interpreter + native both exit 70
  through the adapter). Rung order forward: (3a) Console's byte ops
  ARE the raw-stdio surface (write_byte/read_byte already serve
  dual-engine -- no new trait needed); (3b) Console's write_line/
  write as std checked adapters over write_byte -- CONSTRAINT
  (2026-07-20): a Console adapter must CALL write_byte, which is
  only reachable through a Console-typed PLACE -- so the adapter
  cannot be a v1 FREE machine (it needs a console capability field),
  and adapter dispatch must extend to ATTACHED adapters with
  receiver threading (construct/borrow the adapter's data around the
  dispatch-only slot -- the composed-provider model). EXTEND
  DISPATCH FIRST: attached adapters whose data holds exactly the
  capability fields the body needs, the call rewriting to
  attached-machine form with the caller's own capability fields
  forwarded (Main's console forwards into the adapter's console) --
  spec the forwarding rule before building; (3c) oracle-compare +
  retire the built-in write_line/write rows from tables +
  interpreter serving. The import-argument fix
  unblocked authored-import adapter leaves generally (fs-style
  seams), but Console's own path rides semantics, not imports; (4) move foreign offsets/bit constants out of
  Binding::Value into programmable layout/format declarations, migrate
  filesystem leaves; (5) delete call_shape, HostOperations, Value, the
  populate tables, and provides syntax/consumers only after their last
  users disappear. The provides retirement is ENGINEERING-UNBLOCKED but
  still carries the fs Value rows until step (4).

- **Machine body/supply spelling (decision-20 follow-through, RULED
  2026-07-20):** delete the unimplemented expression-bodied
  `machine f(...) = expr;` sketch. `{ ... }` is the only executable machine
  body, including one-expression predicates. Preserve four distinct semantic
  supply variants and spellings: checked `{ ... }`; bodyless trait
  requirement; external `satisfies ... via <Binding>;`; accepted bodyless
  `boundary machine ... ensures ...;`. Parser work: add `via` as mutually
  exclusive with a body, require an explicit `satisfies` target, const-evaluate
  and normalize the Binding, and carry its ID through resolved/typed/checked
  trees. Add pass/fail canaries for qualified `Binding::DllImport`, runtime-
  dependent bindings, `via` plus body, missing `satisfies`, repeated `effects`,
  signature mismatch, and admission/refinement failure. The current corpus has
  no implemented `= expr` dependency; Chapter 8's stale example was prose only.
- **Float-to-int cast overflow — CLOSED 2026-07-17 (F4 above).** Exact is
  proof-or-reject; Saturating clamps with NaN -> 0; Trapping traps; Wrapping
  rejects. Interpreter, aarch64, and x86_64 cover signed/unsigned 8/16/32/64-bit
  targets, including nested operands and the u64 high half.

## Open bugs / gaps (ungated)

- **Host-divergent coverage — CLOSED on both hosts.** The seven standing
  x86_64-windows gaps (the float five: signed jcc after ucomis* in
  conjunction guards; the frame-indexed-binary zero width; the fs host-arg
  refusal) and the note_vault chain are fixed; the samples gate is fully
  green on both hosts. DURABLE LAWS from that arc: seam rows get DESIGNED
  signatures, never traced ones; authoring-host green is not cross-host
  green — byte-pin x86 sequences structurally because this repo's only
  runtime leg is aarch64-darwin.
  TARGET-MACHINE GATING (the mechanism that carried it):
  `<target> machine Path(..) {..}` — machine items carry
  `target: Option<Identifier>`; a pre-resolution filter in BOTH engines
  clears the selected target's marker and leaves other targets' machines
  inert; loud edges = implemented-twice + no-implementation-for-selected.
  ⚠️ Parser-order landmine: the identifier-led target-machine peek sits
  BELOW contextual-led items so `boundary machine` never reads `boundary`
  as a target name. Pin: pass/targets/target_machine_gating_exit.
- **aarch64-darwin host-divergent gaps — ALL CLOSED.**
- **Field defaults — RETIRED end-to-end** (parse-site refusal +
  47-file corpus migration + carrier/emission/validation sweep; pin
  fail/data/field_default_retired). An initializer can never parse and
  then silently disappear.
- **pending_runtime_divergences_hold — GREENED** (ledger host-corrected).

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
  wrapping-truncation hole is FIXED and pinned. The deeper fix is now
  QUEUED as NEXT TASKS #5 (Constant model CM2 — landed constants carry
  their type; the two-phase law in ch5).
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

- **Windows session — COMPLETE 2026-07-17.** The full workspace battery
  runs green on a windows host (it had never run there); the session
  landed the win32 fs seam end-to-end (designed *W-signature rows, handle
  bridge, FindFirstFile enumeration, set_file_time, locking), the PE
  emitter fixes it surfaced, and the four-target wrapper-selection checks.
  Deliberate scope: the blocking form is the synchronous CRT-handle
  provider; the *at family / fd-based read_dir / chown / symlink stay
  paradigm-refused on windows BY DESIGN. Residual feature arcs (not
  verification blockers): WndProc callback entry stubs for title-bar
  close; WINDOWS_IMPORT_ROWS consumption into the settled ProviderPlan
  form (rides the PRV flip order).

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
- **L5 remainder (four of five items RULED 2026-07-18; record:
  programmable_layouts “Policy selection”):** encode-call spelling SETTLED — bare
  `encode(x)` reads the destination's declared domain, home conformances
  only, loud on plural/undeclared; third-party conformances named-only
  (general rule in ch14). Plan-walking deriver REPLACED: policies
  HAND-WRITE encode/decode/validate as ordinary proven library machines;
  the conformance theorem (`ensures decode(encode(x)) == x` + validate
  agreement) lives ON the trait, proven per conformance — anti-serde kept
  as a proof obligation, not a code generator; the hardcoded
  compact_binary codec serves until inductive prover reach lands (same
  bridge the deriver needed). Validate/materialize MINT EXCLUSIVITY
  RULED: `Checked::Valid { view: plain }` from unrefined bytes = compile
  error (case-payload construction is a fact-checked position, the
  2026-07-04 all-facts-proven implicit-add bug class; validate's proven
  contract licenses its Valid construction). Packed grammar REMOVED
  (dead since 2026-07-02: plan-laid value + recast IS the packed
  encoding). Refinement-as-obligation stays queued.
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
  GUARDED-ARM DEEP FIX DESIGN (settled 2026-07-16, implementation
  queued — needs a fresh-context session; the surface spans two
  files of the symbol-resolved lowering): a guarded arm
  `cond -> (call(args))` cannot hoist ABOVE the transition (the
  callee would run when the arm is not taken — the fence's reason),
  but the language ALREADY serves the target shape: named-target
  arms with arguments + a sub-state whose Always terminal
  auto-hoists (mul_comm's mc_step). AUTOMATE that rewrite:
  `cond -> __arm_k_N(a, b)` + synthesize
  `state __arm_k_N(p: T, ..) -> R { transition { _ -> (call(p, ..)) } }`.
  V1 gate: every call argument must be a NAME of an enclosing state
  parameter (types copy over; general expressions keep the fence —
  the temp would be untypeable at this layer). R = the enclosing
  state's declared return. State synthesis needs fresh names
  (__arm_k_N) + appending to the machine's state list in
  machine/state lowering (statement.rs does not own it — the
  cross-file part). LANDED NOTES: the target's argument NAMES are
  minted FRESH (the original handles stay inside the synthesized
  body — sharing one node across two scopes let the second symbol
  resolution overwrite the first's, the probe-found bug); duplicate
  arguments dedup to one parameter. Pinned: pass
  calls/guarded_value_call_arm_exit (BOTH arms observed — pick(60)
  takes the call arm -> 120, pick(10) takes the else arm -> raw 10;
  RUNS exit 70); fail guarded_value_call_terminal_rejected re-pinned
  to the V1 gate (a computed argument `dbl(x + 1)` keeps the honest
  fence).
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

## Semantic taxonomy representations (front-loaded correctness architecture)

This is not backend performance work. It prevents semantic information from
being erased before validation, interface hashing, proof artifacts, or
lowering can use it. Authoritative audit and target shapes:
[semantic_taxonomy_representation.md](wiki/architecture/semantic_taxonomy_representation.md).

- **Domains (decision 19):** pair of optional predicate/semantic facets,
  normalized semantic-domain identity, introduction/denotation/weakening
  metadata, fact membership separate from semantic qualification. Hybrid
  domains must not be duplicated or guessed from body contents.
- **Machines (decision 20):** normalized complete machine contract plus
  explicit checked/required/external/accepted supply mode; external supply
  carries the normalized `Binding` ID produced by `via`. Consumption
  eligibility is derived. `boundary: bool` is not the semantic model, and
  ProviderPlan rows are derived from explicit conformance closure rather than
  stored as authored machine metadata.
- **Task activation/lifecycle:** deterministically derived
  `TaskActivationPlan` (machine-contract/entry IDs, argument/outcome layouts,
  continuation requirement, cancellation/effect contract) plus permission
  state for provider provenance, activation identity, optional storage lease,
  and live/settled claim. Never recover this from `spawn` syntax, `Join<T>`
  erasure, or a backend frame pointer.
- **Multiplicity (decision 21):** `Unrestricted | Affine | Linear`, with
  establishment and create/transfer/consume/affine-drop events in a
  place-keyed permission context carrying access and provenance. `zero_init`
  and `send` stay orthogonal.
- **Effects/observation (decision 22):** replace the flat `u64` source of truth
  with normalized, symbol-resolved members kinded as `ServiceReach` or
  `OperationalMay`; retain authored interface ceilings separately from checked
  internal inference and pinned slot rows. Authority/capabilities, trust
  receipts, resources, failure, and mutation remain separate. Preserve a
  one-way compatibility/cache projection to the legacy bitset; never recover
  semantics from it.
- **Termination/progress (decision 23):** represent authored/inherited eventual
  terminal guarantees and pinned premises separately from provider-local
  ranking witnesses. Witness subjects/views/ranges/SCC certificates drive
  checking and proof-cache identity, never public contract identity. Sealed
  progress profiles carry grant/receipt identity and stay outside proof facts.

Ordering gates: no general domain mint/operator-family build on the old
undifferentiated domain record; no linear Task/transaction/buffer build on
move/drop-only summaries; no component import-slot/hot-swap manifest pins a
body hash or flat effect row in place of normalized machine contract identity.

## Backend perf (deferred, post-1.0)

MVP backend (fixed-register, mem-to-mem, no regalloc/SSA/SIMD) is slow for
real-time per-pixel work; fine for demos. The "serious backend" layer waits.
Today's bar is provably correct native output.

- **Place algebra (owner-accepted diagnosis 2026-07-18; record:
  wiki/architecture/codegen_representation_cleanup.md Phase 6):** the
  ~100-variant op enum is a Cartesian product of operation x addressing x
  value-category; the factoring is first-class Place (base + composable
  ConstOffset/ScaledIndex path) + one per-target materializer + a ~15-op
  set with category on the operand + legalization (the hoists, promoted
  to principle). Retire the Copy* family first behind the differential
  oracle. RE-SEQUENCED: FRONT-LOADED by owner 2026-07-18 (NEXT TASKS
  #6) — Copy* pilot starts on main-lane coordination or M2-tail
  completion, whichever first; the constant-model track (#5) is
  file-disjoint and leads immediately. Also queued: strengthening
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
- **Historical formats and live replacement (magic retired):** no builtin
  `Versioned<T>`, era-path type syntax, migration chain, or `replace` block.
  Historical eras are immutable ordinary data plus sum envelopes, layout
  metadata, codecs, provenance domains, and checked conversion machines.
  Live replacement is a Cathedral/component package over artifact identities,
  pinned slots, liveness pins, admitted runtime operations, and ordinary phase
  machines. Implement those substrate pieces; add no Omega versioning syntax
  unless the package exposes a semantic impossibility.
- **Equatable synthesis:** a CALLABLE conformance surface is still open.
- **Trailing-state stale reads of threaded `&mut` param fields:** pinned
  (pending/calls/trailing_state_mut_param_phase_divergence, 71/Exit(70));
  cross-state phase allocation for the threaded &mut — the fs lane's
  claimed receiver-phase family, theirs to absorb with the aliasing arc.
- **Task runtime and lifecycle (settled architecture; record:
  wiki/design_briefs/task_runtime_and_lifecycle.md):** ordinary named machines
  are started through an admitted `TaskRuntime` provider using compile-time
  machine-symbol parameters; the compiler derives a normalized activation
  plan. Runtime custody, physical storage ownership, and the linear `Task<T>`
  lifecycle claim are separate. `start` is proof-obligated, `try_start` is an
  ownership-transactional outcome that returns moved arguments/leases on
  rejection; `request_cancel` retains the claim; `finish` terminally settles
  it; `adopt` is ordinary transfer. Pools, mailboxes, supervisors, and task
  groups are packages. `RegionTaskPool` is the standard bounded reference
  implementation/Cathedral profile, not universal semantics. RETIRE the
  synchronous `spawn` parser desugar, erased `Join<T>`, mandatory/vestigial
  `await`, statement fire-and-forget, detach, and privileged scope/group
  concepts. Engineering ladder TR1→TR8 below.
- **Atomics remainder** beyond the landed stage-1 ops + memory model.
- **Separate compilation / component artifact model.**
- **Freestanding target + hardware vocabulary.**
- **Build-time evaluation:** comptime eval + trait generators (effect-free
  machines in value/refinement position).
- **Generics completion — machine parameters RULED; MP1–MP4a LANDED, MP4b
  in flight.** `<machine M>` with a MANDATORY authored
  `where machine M(args) -> Result` contract (never inferred); explicit
  call-site selection `map<Card::power>(items)` as static-symbol metadata
  (never a value argument); selections validated as refinements
  (shape/substitutions/effects/termination/contracts) before lowering;
  machine params are their own callable symbol kind checked modularly;
  MP4a specializes a complete tuple, rewrites uses to the selected entry,
  records a reproducible cache fingerprint (single-tuple fence pinned).
  MP4b rungs 1–3 landed (frozen-SymbolTable generated-child batches; typed
  deep-copy primitive; one lexical-symbol remap across all graphs).
  REMAINING MP4b: compose deep copy + remap + fresh symbols to clone a
  full template, group/rewrite calls per tuple, drop the fence. Then MP5
  (accepted-template grants once + per-instance argument contract IDs)
  and MP6 (Seq map/filter, N5/N6 schema-axiom, task-runtime, and
  build-surface canaries). No runtime callable values, dictionaries, or
  capture inference; dynamic dispatch remains `dyn Trait`.

- **Allocator story:** `Vec` has no runtime. Retire ambient legacy `alloc` in
  favor of explicit allocator/region capabilities and dependent resource
  contracts. Quantitative `Alloc<Peak, Retained>`-style rows wait for the
  resource-algebra brief; do not canonize a one-number allocation effect.
- **Repr control** — FOLDED into the layouts ladder (2026-07-18): L4
  plan-laid types + policies ARE repr control (CLayout pilot landed;
  packed = a no-padding policy); remaining work is surface polish, no
  arc.
- **Proof engine arcs** beyond L7 induction.
- **Domain facets & qualification (frozen decision 19, settled 2026-07-18;
  record: wiki/design_briefs/domain_facets_and_qualification.md):**
  engineering ladder, roughly in dependency order — (1) facet kinds in the
  checker (predicate vs semantic, mechanically enforced at merges, joins,
  casts, generic substitution; per-axis composition algebra); (2)
  binding-site operator resolution (declaration/mint/`requires` select;
  flow facts never consulted; tuple-resolved; collisions hard errors);
  (3) introduction authority (sealed default, `introduction open`,
  `MintAuthority<D>`; split diagnostics: missing proof vs missing
  authority); (4) the deterministic domain-expression normalizer (owns
  type/monomorphization identity; entailment engine may never redefine it);
  (5) `weakens_to` certificates + sealed-theory enforcement (hash of the
  normalized operator theory detects staleness; the sealing law is the
  soundness rule); (6) the units family (see Vertical slices). The unbuilt
  mint arc (encoding/layout recast surface) builds to these rules —
  arithmetic-domain mints (decision 17) already conform.
- **Machine taxonomy (frozen decision 20; record:
  wiki/design_briefs/machine_taxonomy.md):** one contracted transition system,
  explicit supply modes, derived consumption eligibility, full behavioral
  refinement, and contract-defined observability above the caller's floor.
  Engineering begins with STR2-STR4's normalized machine contract; effects,
  suspension, and component linking consume it later.
- **Core multiplicity (frozen decision 21; record:
  wiki/design_briefs/core_multiplicity_and_linearity.md):** unrestricted /
  affine / linear usage, establishment-created obligations, conservative
  transfer/consume accounting, path-sensitive sums, and proposition facts
  separate from the permission/resource context. The core checker precedes
  linear Task lifecycle claims and dependent-linear buffers.
- **Effects/authority/observation (frozen decision 22; record:
  wiki/design_briefs/effects_authority_and_observation.md):** one kinded
  `effects` row, with service reach supplied by boundary-trait identities and
  a small core operational set (`Suspend`, `Block`). Rows are possible-behavior
  ceilings; absence is the negative guarantee. Public rows are authored and
  normalizer-owned; private omission infers a finite fixed point; provider rows
  refine pinned slot rows by subset. Capabilities carry authority, admission
  receipts carry trust, and resources/failure/mutation retain separate homes.
  No masking, subtraction, handlers, `may`, `budget`, `fails`, or `uses`
  surface. Engineering ladder: EFX1 kind/ID/normalizer IR plus legacy
  projection; EFX2 boundary-trait and core-member resolution; EFX3 transitive
  inference, recursive fixed points, and explicit-interface checks; EFX4
  pinned slot/provider subset admission; EFX5 artifact/diagnostic/trust-ledger
  split; EFX6 migrate standard library/canaries and retire the lowercase global
  table as semantic canon. Then complete the decision-16/ch18 amendment:
  suspension composes
  through ordinary calls with inferred/declared `Suspend`, no call-site marker,
  and no `async` machine species or `Future` return transformation. Specify
  continuation/frame layout and capacity, cancellation timing, and the proof
  rules for loans that cross suspension. Task start is a provider call, not a
  new suspension marker; `finish` may suspend through its ordinary contract.
- **Termination/ranking/progress (frozen decision 23; record:
  wiki/design_briefs/termination_ranking_and_progress.md):** one
  `terminates [by ...]` family; public guarantee and premises separated from
  private ranking witness; direction-neutral ranking views; runtime tail-only
  recursion and proof-stratum non-tail use one machine taxonomy; sealed opaque
  progress profiles ride boundary grants. TPR1–TPR6 at the top of this file is
  the compatibility-breaking implementation and corpus migration.
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
- **Trust system engineering — THE CH10 CARRIER IS COMPLETE (task #3);
  remaining consumers ride their host subsystems.** LANDED end-to-end:
  omega-core trust.rs (TrustCommitment: SemanticDomainIntroduction |
  ProgressProfile | AcceptedFact | ProviderPlan; TrustProvenance
  OwnPackageDev-with-standing-warning | RootGrant; TrustGrantTable —
  root outranks dev, receipts dedup); the MintAuthority consult in
  judge_qualification_cast (own-package domains dev-active; the seam
  where package inertness bites); root grants via
  `b.accept_boundary<name>()` in build.omg through the granted
  build-machine evaluation; the unified lockfile (omega.lock receipts
  hashing rendered claims/plan fingerprints — DRIFT under a grant
  refuses until re-approved, pinned for axioms and provider plans); the
  trust report (accepted-tier rows, dev-active standing warnings,
  fingerprints); the Accepted tier end-to-end (bodyless boundary
  machines cite-able with engine veto; pins
  accepted_axiom_cited_exit / accepted_axiom_engine_veto /
  granted_axiom_receipt_drifts_on_claim_edit). Grant locality: packages
  may claim but never self-grant.
  REMAINING GR6 (each rides another arc): (a) qualification AUTHORITY
  half (rides STR4 publication); (b) ProgressProfile minting + the three
  premises stubs (rides the TPR4 profile spelling); (c)
  MachineContractPlan permission half + provider admission (rides the
  PRV ladder). Oracle tripwires + `defer` tooling remain future work.

## Structural follow-ups (surface landed; semantics pending)

- **Inline asm:** only `asm { jmp state(...) }`; labels/back-edges rejected;
  mnemonics, register constraints, clobbers, `asm where` contracts pending.
- **Transition data-patterns:** guard-lowering only; real pattern binding,
  multi-subject validation, domain-pattern proofs, diagnostics pending.
  SPEC DIRECTION SETTLED (owner, 2026-07-18; record: build_time_evaluation
  brief): grow record patterns into LET position — exhaustive by law
  (every field bound or waived), `as` renames, `as _` waives, colon and
  arrow rejected; arm-position record binding shares the grammar; v1 may
  restrict bindings to [copy]-eligible fields. The canonical hand-written
  equals opens with the exhaustive destructure (convention, unenforced).
  LET POSITION SHIPPED (2026-07-19): `let { x, y as v, z as _ } = place;`
  parses via try_parse_destructure_let (parser/statement.rs) — desugars at
  parse time to a `__destructure__<fields>` MARKER let (the exhaustiveness
  carrier; initializer = the place) + per-BOUND-field Unit-sentinel lets
  reading the place's members (types via hoist inference); v1 place gate
  (Name/self/member chains — calls would double-evaluate); colon/arrow fall
  through to the plain parser's error. The LAW half:
  omega-validation/src/destructure.rs resolves the marker's place type
  (declared_place_type_raw) to its data definition and refuses missing
  fields (named, with the bind/rename/waive menu) and unknown fields;
  sum types are redirected to `case`. Canaries:
  pass/data/record_pattern_{let,bind_all}_exit (run-verified 70),
  fail/data/record_pattern_{missing_field,unknown_field}.
  ARM POSITION SHARES THE GRAMMAR (2026-07-19): the arm destructure field
  parser (transition/guards.rs parse_data_destructure_pattern_fields) now
  accepts `field as name` (rename -- the binding rewrites to the same
  subject.member read; DestructureBinding already separated binding from
  member) and `field as _` (waive -- spelled, no binding; a guard using
  the waived name refuses at symbol resolution). `..` stays the arm-only
  rest escape (pre-spec surface; LET has no `..`). Canaries:
  pass/control_flow/{case_pattern_rename_waive,record_pattern_arm_rename_guard}_exit
  (run-verified 70), fail/control_flow/arm_pattern_waived_field_use.
  ARM EXHAUSTIVENESS LAW SHIPPED (2026-07-19): a `..`-free destructure arm
  must spell every field; `..` is the arm-only explicit opt-out. Carrier:
  the transition parser collects arms first, then appends variant-aware
  marker lets (`__arm_destructure#V=<variant>#<f1>...` -- `#`/`=` cannot
  appear in identifiers, so the split is unambiguous; initializer = the
  subject place, same is_place gate as LET) before the arm Transition
  statements; destructure.rs resolves the marker against the record's
  fields or the named case's payload and refuses missing (full menu named)
  and unknown fields. Proof-side statement-shape walks in
  contract_entailment.rs step over markers (is_arm_pattern_marker, the
  citation-skip precedent) -- without this, markers inside nat.omg's
  induction lemmas broke shape recognition and the lemmas fell through to
  the proof-only fence. Canaries: fail/control_flow/arm_pattern_missing_field,
  pass/control_flow/arm_pattern_rest_optout_exit (run-verified 70).
  REST-PATTERN EXISTENCE CHECK CLOSED (2026-07-19): `..` patterns now
  ALSO mint markers, suffixed `#~rest` -- validation skips only the
  missing-field half; a spelled field that is not a field of the case
  still refuses (fail/control_flow/arm_pattern_rest_unknown_field). The
  parser unit test indexing arm statements by position was re-anchored
  to filter Transitions (markers precede them).
  REMAINING: non-place subjects skip the law (no declared type to
  resolve); [copy]-eligibility restriction if
  non-copy fields surface unsoundly. DOUBLE-UNDERSCORE FIELD FIX LANDED
  2026-07-17: LET markers use the arm family's identifier-impossible `#`
  delimiter, so a field such as `left__value` remains one component instead
  of spuriously becoming two unknown fields; parser + compile canaries pin it.
- **Const data parameters:** symbolic lengths flow structurally;
  instantiation-time substitution, validation, layout diagnostics, const-fact
  proof integration pending.
- **Host providers:** rows parse + snapshot; registry validation, target
  whitelisting, syscall/import lowering, boundary report pending.
- **Trait defaults — `default` KEYWORD KILLED (owner, 2026-07-18):** a
  trait machine with a body IS the default (body presence = the marker;
  record: build_time_evaluation brief). KEYWORD DROPPED 2026-07-16:
  the parser marks defaults by BODY PRESENCE (a `{` after the
  signature clauses); the retired spelling refuses with direction
  (pinned fail traits/default_keyword_retired); the one corpus use
  (pass/traits/default_machine_in_trait) dropped the keyword and
  still runs; ch14 carries no stale spellings. Remaining:
  conformance/reuse/override rules, dispatch pending as before.
- **Dynamic traits (`dyn Trait`):** structural + fat descriptor; construction,
  vtable emission, dispatch lowering, object-safety validation pending.
  NOTE from the proofs arc: dyn descriptors must carry satisfier identity
  (`as &dyn Card::PowerOrder` decays to `&dyn Trait`; ch14).
- **Relax surface removal (relax RETIRED 2026-07-17):** superseded by
  invariant windows (ch11). REMOVED 2026-07-16: both spellings refuse
  with direction to ch11 windows (the `relax` statement and the
  `&relaxed` reference marker); the two pass/relax canaries were
  UNREGISTERED dead files (their own headers said the forms never
  parsed) and are deleted; no real corpus uses existed (the collections
  hit was a comment + state name). Pinned: fail parse/relax_retired.
  Representation `is_relaxed` fields stay inert-false (the sweep is a
  follow-up if they block something). PARSER DEAD PATH REMOVED 2026-07-17:
  the unreachable old `relax { ... }` builder and its `TableRelax` import are
  gone; the syntax-tree `Relax` variant, snapshot/identity/visualization arms,
  and resolved-lowering flatten path are gone too. The directed retirement
  diagnostic remains pinned. REFERENCE-BIT SWEEP LANDED 2026-07-17: the inert
  `is_relaxed` field is gone from syntax/resolved/typed reference carriers,
  snapshots, displays, identity/canonicalization, lowerers, and operator/trait/
  machine-parameter compatibility. Reference mutability is once again the only
  qualifier bit; no permanently-false compatibility state remains.

## Vertical slices

- **Machine-contract representation slice (decision 20):** one ordinary
  checked machine used by runtime-call and compile-eval consumers shares one
  normalized contract ID; add requirement/provider/accepted supply fixtures;
  reject a provider with one hidden extra effect; snapshot that the checked
  artifact preserves supply mode and contract identity. Start before component
  manifests or hot-swap admission. ADMISSION FIXTURES PINNED 2026-07-16
  (the reject halves were already live, now witnessed): fail
  capabilities/provider_widens_requirement_ceiling (a provider declaring
  beyond the requirement's effects -- acceptance 7's never-widen) + fail
  provider_hidden_extra_effect (declares exactly the ceiling but REACHES
  filesystem_io through a callee -- the conformance ceiling bounds the
  declaration, ceiling enforcement bounds the reach; nothing hides
  between) + pass provider_within_ceiling (RUNS exit 70). The contract-ID
  and snapshot halves ride MachineContractPlans (landed same day).
- **Termination firewall slice (decision 23):** one bodyless requirement
  authors `terminates;`; an acyclic implementation inherits and derives it
  without repetition; cyclic implementations prove it once with descending
  and once with bounded-increasing views. Swapping those witnesses changes the
  provider proof hash only, not caller/import-slot contract identity. Reject
  runtime non-tail lowering while accepting the same ranked proof-stratum
  shape, and reject an ungranted self-asserted progress profile.
- **Kinded effect-row slice (decision 22):** declare `Readable` and a scheduler
  service with separate wake and wait operations; derive `Readable + Suspend`
  for a caller of the read/wait paths and only the scheduler reach member for
  wake. Reject an undeclared public member, a caller ceiling missing a callee
  member, and a `Block` provider for a `Suspend`-only slot. Snapshot stable
  normalized row/contract IDs across a prover-strength change; demonstrate the
  legacy bitset is derived output, not input to admission.
- **Core multiplicity slice (decision 21):** **CML1 LANDED 2026-07-17:**
  `[linear]` is now a real closed-set type property on data declarations and
  type-parameter bounds. The syntax tree carries first-class `Multiplicity`
  directly (Affine default, `[copy]` -> Unrestricted, `[linear]` -> Linear),
  lowering copies it through resolved/typed trees, and the legacy `copy` bool
  is derived only as a compatibility projection. `[copy, linear]` rejects
  loudly; parser + end-to-end typed-lowering tests and pass/fail ownership
  canaries pin the representation. **CML2 SLICE 1 LANDED 2026-07-17:** the
  checker now carries a whole-place linear permission state for parameters and
  explicitly established locals: construction/assignment creates, moves and
  calls transfer, by-value `self` calls terminally consume, implicit zero-fill
  creates nothing, and overwrite/second-transfer/live-scope-exit reject. The
  existing machine-call ownership lane now records an explicitly supplied
  static by-value `self` argument (previously filtered out with borrowed self).
  Affine records may not erase a linear field/payload, and `[linear]` generic
  bounds participate in instantiation checks. Pinned by a fixed-size Receipt
  token: create -> two bindings -> consume; zero storage without use; and fail
  twins for unestablished use, duplicate transfer, scope loss, and affine
  containment. **CML2 SLICE 2 LANDED 2026-07-17:** permission states fork at
  transition arms and reconcile exactly; both-arm transfer passes while mixed
  transfer/retention rejects. Explicit assignment establishes previously bare
  storage and cannot overwrite a live obligation. Affine sums now carry
  conditional obligations in only those cases whose payload is linear:
  `Empty` drops normally, `Live(Receipt)` must transfer/consume, and moving the
  whole live sum conserves the payload claim. Pass/fail canaries pin all three
  faces. **CML3 SLICE 1 LANDED 2026-07-17:** checked flow now retains
  first-class `Establish | Transfer | Consume | AffineDrop` permission events
  with machine/state/source/place identity and the conditional payload-debt
  bit. The compiler-recording check path writes the artifact once; unit tests
  pin create -> transfer -> create -> consume plus affine cleanup, and `Empty`
  establishment with no debt. Legacy move/drop arenas remain temporarily for
  downstream compatibility, not as the semantic event taxonomy. **CML3 SLICE 2
  LANDED 2026-07-17:** one core `PermissionEventKind/Source` vocabulary and the
  typed events now survive state graph -> control flow -> abstract/target/
  assigned operations -> machine program/instructions/bytes; exact capacities,
  machine-graph merge/remap, tests, the audit pin, and backend report all carry
  them. **CML3 SLICE 3 LANDED 2026-07-17:** permission events now carry the
  full entry axes -- multiplicity, `Owned | Shared | Exclusive` access, and
  transfer-stable establishment provenance. The straight-line linear checker
  preserves one origin through multiple bindings; legacy-derived affine drops
  stay explicitly `Unknown` rather than fabricating evidence, and backend
  reports expose all three axes. **CML3 SLICE 4 LANDED 2026-07-17:** existing
  borrow activations and weakenings now emit permission-context entries over
  the borrowed place: shared loans are `Unrestricted + Shared`, exclusive
  loans are `Affine + Exclusive`, and both retain one establishment origin
  through release. Borrow legality remains in the established checker; this
  rung removes the representational split without changing it. **CML3 SLICE 5
  LANDED 2026-07-17:** permission production and linear judgment are separate;
  the validator reads only qualified permission events (a test deletes both
  legacy arenas before re-running it), while the transitional producer may
  still project legacy move/drop discovery. This also closes the conditional
  zero-storage hole: payload-debt=false is not establishment evidence. **CML3
  SLICE 6 LANDED 2026-07-17:** a by-value `self` call whose result carries a
  linear/conditional obligation is a transfer, not a terminal consume; a
  single unambiguous moved input preserves its origin into the result binding.
  The direct `Receipt -> Receipt` customer and canary pin this before task
  outcomes depend on it. **CML3 SLICE 7 LANDED 2026-07-20:** moving the live
  payload out of an affine `Empty | Live(Token)` sum now records the nested
  transfer, settles the sum's one conditional debt, and preserves the original
  provenance through both sum construction and payload extraction. Ordinary
  linear aggregates stay conservative: nested extraction does not pretend to
  consume sibling obligations before the per-field resource algebra exists.
  Pinned by the pipeline event sequence/origin test and
  pass/ownership/conditional_linear_payload_extraction (native exit 70).
  **CML3 SLICE 8 LANDED 2026-07-20:** affine cleanup events are now
  discovered directly from typed state ownership (locals in reverse
  declaration order, then owned by-value parameters), independent of the
  compatibility `drops` arena; a producer-level test clears that arena before
  rebuilding permissions and pins identical cleanup roots/order. **CML3 SLICE
  9 LANDED 2026-07-20:** semantic transfers and consumes now run the canonical
  typed move-discovery traversal through their own event sink instead of
  projecting the compatibility `moves` arena. A producer-level test clears
  that arena and rebuilds the identical permission sequence. CML3's producer
  migration is complete: legacy move/drop arenas remain compatibility output
  for downstream migration only, never semantic producer input.
  Terminal consumption needs no annotation: an ordinary
  `move self` call consumes when no returned outcome carries the obligation,
  while a `try_*` incomplete outcome must return the live token. Pin create ->
  multi-binding transfer -> consume as one obligation; reject scope loss,
  copy, mixed branch treatment, and implicit zero-created obligation; add
  `Empty | Live(Token)` path-sensitive acceptance. Then make `Task<T>` a
  customer after the core checker, not the bootstrap implementation vehicle.
- **Task-runtime slice (settled task model):** **TR1 LANDED 2026-07-17:**
  the synchronous `spawn` desugar, parser-erased `Join<T>`, `.join()` identity,
  statement detach, and their reserved-name hacks are gone. Former block/type
  spellings reject at parse with migration diagnostics naming
  `runtime.start<Worker::run>(...)`, linear `Task<T>`, and `finish()`; `spawn`
  remains an ordinary contextual identifier and `Join`/`join` are ordinary
  declaration names. The five fake pass canaries and four staging-specific
  fail canaries retired into spawn_retired + join_type_retired; the real direct
  call/result pins remain elsewhere. **TR2A LANDED 2026-07-17:** core now owns
  the source-visible `[linear] Task<T>` claim carrier. Task-specific pass/fail
  canaries pin multi-binding transfer, terminal by-value-self settlement,
  `Idle | Running(Task<T>)` payload extraction, and scope-loss rejection.
  TR2B adds transactional start and terminal task-outcome sums with
  qualifier-aware generic payload propagation: `Returned(LinearT)` and
  `Rejected(LinearArguments)` must retain their substituted debts rather than
  laundering them through an unconstrained generic field. TR3 elaborate
  `runtime.start<M>(args)` from the
  existing compile-time machine-symbol parameter into a normalized activation
  plan (contract/entry IDs, argument/result layout, continuation requirement,
  alignment/pinning, cancellation/effect metadata); TR4 add the `TaskRuntime`
  boundary requirement, provider validation/admission, and return of every
  moved argument/lease on rejected start; TR5 represent provider provenance
  and child storage leases so close/reclaim rejects while dependent claims
  live; TR6 lower continuations and land a first provider, with inline
  completion admitted only where the pinned contract permits it; TR7 enforce
  the conservative suspension-safe-loan subset (moved ownership, shared
  immutable, explicit synchronization first); TR8 build `RegionTaskPool`, a
  bounded mailbox, and a supervisor reference package, then migrate the sample
  corpus. Acceptance register lives in
  wiki/design_briefs/task_runtime_and_lifecycle.md. Pool/supervisor convenience
  never earns core syntax unless the packages expose a semantic impossibility.
- **Units slice (decision-19 stress test; run EARLY, before the generics
  arc):** two units, one dimension, no generics — pin the model with the
  brief's seven acceptance tests: (1) `Km + Metre` rejects without explicit
  conversion; (2) `Km / Metre` = scaled dimensionless preserving the 1000 —
  explicit forgetting yields raw 1, conversion yields canonical 1000,
  implicit erasure forbidden; (3) Energy vs Torque: same dimension, distinct
  kinds; (4) `Vec<f64 in Km>` survives a generic identity machine
  unweakened; (5) `f64 in Km` to a plain-`f64` param fails without explicit
  forgetting or conversion; (6) Energy cannot SILENTLY launder into Torque;
  (7) sibling packages cannot independently claim one cross-domain operator
  tuple. Depends on facet kinds + binding-site resolution (Big arcs entry);
  tests 4/7 additionally gate on domains-over-carriers generics and package
  coherence machinery — stage them last within the slice.
- **Vec[T]:** owned dynamic storage with length/capacity (surface declared;
  storage/lowering pending; allocator-story dependent).
- **as_slice/as_mut_slice:** back with real boundary-primitive storage.
- **Ownership events:** continue appending transfer/drop events from the
  remaining ownership forms; lower abstract summaries into explicit backend
  transfer ops.
