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
   contact; N1 LANDED 2026-07-11 (proof-only classification + all faces),
   N2 next: N2 retires the long-standing u64>i64::MAX i128 debt. Continue
   into N5–N7 when reached (all design-settled; they need the `<machine M>`
   plumbing and the `%` former).
2. **Measured recursion MR1 + MR3** — LANDED 2026-07-11 (MR1 whole; MR3
   direct leg — the mutual leg rides MR4). Next in the family: MR2 (the
   terminal-position tail rewrite onto loop-backs — the arm-target spelling
   needed NO lowering and already runs), then MR4/MR5.
3. **Dependent types R2** (section below) — where-clause + gating + windows;
   the big semantic build, explicitly ADDITIVE (the landed eager store-time
   checks stay sound as the conservative tier). One careful agent, multi-day.
4. **Windows platform-verification session** (section below) — checklist-
   shaped; one session on a Windows host closes the whole list.

Also standing: the rendering-sample sweep onto the direct `pixels[y*W+x]`
spelling (R0 follow-on, under Language ergonomics).

## Cathedral M2 (owner priority 2026-07-15; RECAST = main lane, claimed)

Cathedral is fully written and waiting; M2 (`GetMemoryMap → ExitBootServices
→ first Region mint`, `../Cathedral/source/boot/uefi/own_machine.omg`) is
down to a MEASURED gap list (compile-checked against the real
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

- **R1 remainder:** value-vs-value guard mints at range endpoints generally
  (`requires a.cols == b.rows`); the bracket-as-sugar desugar for
  machine-signature requires. Cross-machine dependent params ride R4.
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
- **R4 (boundary witness mints, proof side):** out-params as witnesses,
  decode-minted where-facts, recast bounds discharged from couplings +
  R1/R3. ⚠️ COORDINATE: recast MECHANICS are main-lane; this rung supplies
  only the proof side. Unblocks the UEFI memory-map stride discharge.
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
  landed 2026-07-11) — but VALIDATION's arithmetic_domains S4 value-env
  fold checks the same `n - 1` first and does NOT read fall-through
  complements yet, so the exact-domain terminal shape
  (`u64 [0..=100]`) still refuses there and the run canary rides a
  Trapping domain; the validation value-env complement is the remaining
  unlock. Two-state cycles (entry→step→entry) still need per-edge
  strict decrease, so a same-value forwarding edge refuses — MR4's
  joint-measure work is the natural home. Original rung text:** a MEASURED machine's state whose TERMINAL
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
- **MR5 — proof-stratum evaluation:** measured recursion under interpreter
  fuel for compile-time proof machines (no lowering, no space rule).

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
  start is documented as a non-claim. REMAINING: (d) the structural
  bridge for induction (n > 0 => n == Succ(n - 1)) — needs Nat-as-data
  engine semantics, design with N4's roster library in view.
- **N3 — fact-position operator routing:** glyphs route by operand type —
  Nat/Int/Rat compute-mode, declared proof carriers rearrange-mode
  (ring-generic polynomial normalization). Int introduction rule: order
  has no floor, measures stay Nat-valued or range-floored.
- **N4 — roster library:** Nat/Seq/Bag/Rat in core as ordinary recursive
  data + extraction lemmas; the proof views (Seq/Bag/Range) dissolve from
  parser-known atoms into these types (the L6 bag_view rung folds in).
  Rat/Bag ship via CANONICAL-REPRESENTATIVE domains (reduced fractions
  `where gcd==1`; sorted sequences) — plain `==`, no quotient dependency:
  N4 is decoupled from N6. The `%` former is reserved for carriers with
  no computable canonical form (Real: stream equality undecidable).
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

- **[ENGINEERING]** numeric intrinsics remainder: sin/cos need range
  reduction + a polynomial matching interp precision — a numerical
  mini-project.
- **Rendering-sample sweep (R0 follow-on, standing):** the direct
  `pixels[y*W+x]` spelling serves since 2026-07-09 — sweep the rendering
  samples' linear-counter workarounds + re-guard states onto it.

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
