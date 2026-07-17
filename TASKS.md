> `OWNER_QUESTIONS.md` contains unresolved owner decisions only. Settled
> language rulings live in the guide/briefs; this file tracks engineering.

# Tasks

Engineering ledger and working backlog. Completion notes remain only where they
explain dependencies or migration state; detailed history belongs in git and
canary headers. Condensed 2026-07-12 and 2026-07-18.

## Current Strategic Focus

Omega's first real consumer is the Cathedral OS (`../Cathedral`). The gap
analysis lives in [wiki/cathedral_alignment.md](wiki/cathedral_alignment.md).
Current critical gaps are programmable layouts, freestanding entry/hardware
vocabulary, atomics/scheduling, and separately compiled component replacement.

## NEXT TASKS — design-unblocked, agent-ready (loaded 2026-07-18)

Claim an item, work its rungs in order, canaries per rung, push per rung.
Collision map: the MAIN LANE owns RECAST/M2; the FS LANE owns the
dispatch-region + receiver-phase family. Everything queued here avoids both.

**FRONT-LOADED GATE — termination surface + semantic migration TPR1→TPR6
(decision 23):** the ruling deliberately invalidates the current parser,
typed representation, proof-cache shape, diagnostics, and essentially the
whole executable/proof sample corpus. At inventory time, 113 `.omg` files
contain `terminates` and 104 contain standalone `decreases`. Do not preserve
compatibility syntax at this age; migrate the compiler and corpus as one
explicit breaking pass. Rungs: TPR1 parser/AST — LANDED 2026-07-16
ATOMICALLY WITH ITS CORPUS SWEEP: the parser accepts bare `terminates;`
and `terminates by <subjects> [-> View] [in <range>];` (subjects parse
with the NO-MEMBERSHIP expression variant so the clause's own `in` is
never eaten; the range builds a Range node inline — index-position was
the only structural range parser); the block form and standalone
`decreases` are RETIRED with loud decision-23 diagnostics (pinned:
fail/termination/terminates_block_form_retired +
standalone_decreases_retired); the `in <range>` constraint parses into
the NEW syntax-Machine `decrease_range` field and the lowering refuses
it until TPR3 consumes ranges (pinned: rank_range_unconsumed — never a
silent drop). Corpus: 97 files regex-swept + 3 manual (a bare-
terminates body brace the sweep must not touch, a no-semicolon
standalone form, comment spellings) + 2 parser unit tests; every gate
green over the migrated corpus first try. TPR2 LANDED 2026-07-16: the
resolved+typed Machine records carry the normalized
`MachineTerminationPlan` (published guarantee vs private RankingWitness),
populated ONCE at syntax->resolved and COPIED downstream -- bare
`terminates;` (or the tolerated no-semicolon form) authors the PUBLIC
guarantee via the new syntax `terminates_guarantee` flag; `terminates by`
publishes NOTHING and supplies the witness (subjects rendered
source-like, explicit view). Canonical defaults elaborate IMMEDIATELY at
lowering (mirroring the checker's inference exactly): two subjects ->
Nat::BoundedDistance; single `.len` member or root-state parameter of
unsigned {u8,u16,u32,u64,nat} (constraint/reference shells stripped) ->
Nat::Descending; slice/fixed-array parameter -> Slice::Length; the rest
stay pending (empty view_path) for TPR3's in-checker elaboration.
omega-core gained the FIXED-ID canonical view catalog
(RankingViewId::{NAT_DESCENDING,NAT_BOUNDED_DISTANCE,SLICE_LENGTH} +
canonical()/canonical_path()); RankingWitness gained `view_path` (the
private witness's honest spelling carrier; declared measures carry NULL
id + spelled path until TPR3 measure identity). checked_summary stays
NoGuarantee until TPR3. LOSS-2 re-pinned (plan beside compat bools +
witness-swap-is-contract-invisible invariant test); 2 population tests in
typed-trees-to-checked-trees/tests/termination.rs. SIDE HARVEST: the TPR1
sweep missed EMBEDDED Rust-string omega sources -- 46 block forms + 1
brace-escaped + diagnostics/comments teaching the retired spelling
migrated across 6 crates, and 5 long-stale test fixtures (missing
supply_mode/target/decrease_range/TraitConformance fields) repaired; the
whole `cargo test --workspace` battery (158 targets) is green again and
joins the gate list. TPR3 SLICE 1 LANDED 2026-07-16 (the checker steps
onto the plan): checks/termination gates on the NORMALIZED plan
(published-or-witness) instead of the `terminates` bool; the
missing-witness diagnostic teaches the current spelling; the
entailment judge's hypothesis gate and the state-graph's
machine_is_measured read witness-presence from the plan; and a
DecreaseOutcome::PlanViewDivergence invariant makes any disagreement
between the RECORDED elaborated view and the checker's independently
resolved builtin order LOUD (pinned by a mutate-the-plan unit test;
declared measures agree by construction, pending views constrain
nothing). TPR3 SLICE 2 LANDED 2026-07-16 -- Nat::IncreasingTo(limit),
the ARGUMENTED ranking view (decision 23's acceptance test 5): the
by-clause grammar takes an optional parenthesized argument list after
the view path (new decrease_view_arguments span through
syntax/resolved/typed; witness gains view_arguments strings; core
catalog gains NAT_INCREASING_TO=4); the checker resolves it to
RankingOrder::IncreasingTo(limit) and runs the SAME bounded-distance
machinery with the view-fixed (subject, limit) orientation -- an
increasing cursor proves WITHOUT an authored subtraction; the
entailment judge's polynomial arm reads the measure `limit - subject`
from the view argument. DIRECTED rejections (new
OrderResolution/DecreaseOutcome::Rejected): unbounded `Nat::Increasing`
names the bounded spelling; argument-arity misuse and arguments on
plain views name the fix (pinned: pass
termination/increasing_cursor_bounded_view RUNS exit 4; fail
increasing_unbounded_rejected; 3 pipeline tests + 1 parser unit).
TPR3 SLICE 3 LANDED 2026-07-16 -- RANGE-ON-RANK v1: the TPR1 blanket
lowering refusal is LIFTED; the `in <range>` constraint lowers through
resolved->typed (new decrease_range handles), records into the witness
(RankRange {floor, ceiling, inclusive} strings), and the CHECKER
consumes it -- v1 accepts exactly the shape TRUE BY THE VIEW'S
DEFINITION (`in 0..=limit` on `Nat::IncreasingTo(limit)`: the rank IS
the distance up to that bound, so 0 <= rank <= limit structurally);
nonzero floors, foreign/exclusive ceilings, and ranges on plain views
get DIRECTED rejections naming what proof is missing -- consumed or
refused, never a silently recorded unproven fact (pinned: pass
increasing_cursor_rank_range; fail rank_range_unconsumed re-aimed at
the directed plain-view message + new rank_floor_unconsumed; 2
pipeline tests, 4 rejection shapes). ACCEPTANCE TEST 4 verified
ALREADY-PINNED: runtime non-tail measured recursion is rejected at
lowering (fail/calls/nontail_value_self_call_rejected) while the same
measured non-tail shape runs the proof stratum (the whole N4 Nat/Seq
lemma zoo). TPR3 SLICE 4 LANDED 2026-07-16 -- the CHECKED TERMINATION FACTS:
CheckFacts gains `termination: TerminationFacts` (one fact per
CLAIMING machine: checked_summary + resolved explicit view path),
built in build_check_facts from the SAME pure resolution/proof
functions the termination check uses (facts and diagnostics cannot
disagree; an unproven claimant records NoGuarantee AND fails
compilation). checked_summary's PRODUCER at last: an acyclic claimant
derives EventualTerminal without a witness (the brief's derivation
rule); a proven witness establishes it WITH the resolved view --
completing lowering-pending elaborations at the checked stage; a
machine claiming nothing gets NO fact. JOINT-SCC-ACROSS-MACHINES
verified SETTLED BY CONSTRUCTION: cross-machine call cycles are
source-banned with a directed fold-into-one-machine diagnostic
(run-probed: "machine call cycle ... banned (stack size must be
predictable)"), and within-machine multi-state SCCs already use ONE
joint witness (nonstrict-edge-acyclicity checker, pinned by the
mutually-recursive-states tests) -- the brief's deferred item is only
the cross-shape source SPELLING. Remaining TPR3: subject resolution
FROM the witness (retiring the compat spans -- TPR6-adjacent). TPR4
SLICE 1 LANDED 2026-07-16 -- trait requirements PARSE the guarantee:
the bodyless-signature clause parser's skip-any-token fallback was
SILENTLY EATING `terminates;` on trait requirements (decision 23's
PRIMARY bare-form use!); it now parses into the new syntax
StateSignature.terminates_guarantee flag, and `terminates by ...` on
a requirement is rejected loudly (the witness belongs to the
implementation that discharges the inherited claim -- pinned:
fail/termination/requirement_witness_rejected + 2 parser units).
TPR4 SLICE 2 LANDED 2026-07-16: the guarantee propagates into the
RESOLVED trait-signature record (StateSignatureStorage.
terminates_guarantee, populated at syntax->resolved through
lower_state_signature_parts, per-signature precision pinned by a
pipeline test). TPR4 SLICE 3 LANDED 2026-07-16 -- INHERITANCE AT
CONFORMANCE: the typed StateSignature carries the flag (copied
resolved->typed); the resolved->typed MACHINE lowering inherits the
requirement's published guarantee into the implementation's plan
(inherit_requirement_guarantee: explicit `satisfies Trait::req` or
simple-name matching mirroring the conformance carrier model; an
authored guarantee is never overwritten) -- and the TPR3 plan gate
then enforces the inherited claim FOR FREE: an acyclic inheritor
discharges silently, a CYCLIC inheritor without a witness fails with
the missing-witness diagnostic, `terminates by n;` discharges it
(pinned: 3 pipeline tests, first try). Remaining TPR4: published
omission/default rules for EXPORTS (needs artifact serialization).
SEALED PROGRESS PROFILES DESIGN-RULED 2026-07-17: a profile is a sealed
semantic domain over a boundary-provider capability; qualification is an
opaque commitment admitted through chapter 10's existing root grant,
lockfile, receipt, and trust-report path. It is never flow-inferred or used
as an entailment relation. Remaining engineering: profile-domain resolution,
grant-backed admission, receipts, and pinned premises. TPR5 LANDED 2026-07-16 (the
atomic corpus sweep is COMPLETE): omega core/std + samples + all
canary families swept in TPR1 (97 files + 3 manual); embedded
Rust-string sources in TPR2's side harvest (46 + 1 brace-escaped);
the last stragglers -- 4 compiler-lattice fixtures (7 block forms) --
swept this rung and RUN-VERIFIED natively (exit 70 each). Retired
spellings survive ONLY in the three deliberate fail canaries that pin
the retirement diagnostics and in clearly-labeled historical wiki
notes. The checked-in `.omg` corpus now demonstrates NORMATIVE
decision-23 syntax everywhere. TPR6 (artifact/cache/diagnostic
firewall + the ten acceptance tests) remains; of the ten, 2/3/4/5/6/7/
10 are already pinned by the TPR2-TPR4 test batteries -- outstanding:
1's export-omission half (needs artifact serialization), 8/9 (need
sealed progress profiles + grants, TPR4's remaining big half).

1. **Math roster ladder N1→N4** (section below) — zero backend/codegen
   contact; N1 LANDED 2026-07-11 (proof-only classification + all faces);
   N2 rungs a–c LANDED (bignum + exact engines; the u64>i64::MAX debt
   retired as a soundness fix); N4's FIRST SLICE landed 2026-07-11: the
   "core-injection" question DISSOLVED — the bundled `omega::` root +
   depend-mapping already reach omega/language/core, so `data Nat` lives
   in core/nat.omg as ordinary recursive (proof-only) data, pinned
   pass/proofs/runtime_core_nat_declared_exit + the consumption-refusal
   twin. N2(d) is deferred and blocks nothing. The next Rat rung is
   engineering-ready: Nat strict-ranking evidence for computed `sub`, then
   mod/gcd. Remaining: N3 routing, N4's Seq/Bag/Rat + extraction lemmas + view
   dissolution. Continue into N5–N7 when
   reached (they need the `<machine M>` plumbing and the `%` former).
2. **Legacy measured-recursion implementation MR1–MR5 — LANDED.**
   MR4's first cross-machine tail-cycle admission shape landed 2026-07-20
   (task #135; detail below). Richer guards and multi-subject lexicographic
   cycles are demand-gated rather than blockers. Decision 23's TPR migration
   supersedes this family's source spelling and must preserve its checker/lowering
   behavior rather than reimplementing it accidentally.
3. **Dependent types R2** (section below) — where-clause + gating + windows;
   the big semantic build, explicitly ADDITIVE (the landed eager store-time
   checks stay sound as the conservative tier). One careful agent, multi-day.
4. **Windows platform-verification session** (section below) — checklist-
   shaped; one session on a Windows host closes the whole list.
5. **Constant model — the two-phase law (FRONT-LOADED by owner
   2026-07-18; law: ch5 "Constants: Two Phases"; memory:
   anonymous-values-two-phase-constants):** file-disjoint from the RECAST
   selection files — START IMMEDIATELY. Rungs:
   ⚠️ CARRIER RE-SCOPED 2026-07-18 (owner ratified the COMBINED CM1+CM2
   carrier; the old "major cross-compiler change" scare is STALE): D14
   already did the hard migration — all four tree layers carry
   `omega_core::literals::IntegerLiteral` as the payload, so the landing
   rides INSIDE the literal with ZERO enum-variant or match-site churn,
   every splice/alias/`to_tree` CLONES it (the strip-on-substitution
   disease dies structurally), and the spelling already holds any
   magnitude (CM1's "165-site i128 refactor" dissolves into consumers
   consulting the landing). Rungs:
   CR1 — omega-core: `LandedIntegerType` (foundation-layer integer-type
   enum; ArithmeticDomain is already in omega-core) + an optional
   landing on IntegerLiteral (constructor `with_landing`, accessors;
   equality stays TEXT-ONLY by custom impl — spelling is identity,
   landing is metadata).
   CR2 — stamp at the sources: the state-values folder's land_result
   stamps fold results (it holds the landing already — retires the
   representative compromises: a landed u64 result rides spelling +
   landing losslessly); typed→checked lowering stamps declared-type
   landings (let annotations, field types).
   CR3 — LANDED 2026-07-18, ACCEPTANCE MET (the operand-position
   min/max divergence promoted, both engines 77). The TRACING QUESTION
   resolved the rung differently than scoped: the strip was NOT
   RuntimeStaticValues — empirical trace showed the `-1` already bare
   in the OWNED arg tree at the state-values level. The real leak: a
   `let`-captured local RE-FOLDS inside argument positions where the
   destination landing is None → the type-blind i64 window emitted a
   bare literal. Fix = two pieces of the ch5 law at the source:
   (1) BINDING-CAPTURE STAMP — simple_local_bindings stamps a plain
   literal initializer with the local's declared landing
   (folding::land_literal normalizes+stamps); (2) OPERAND-DERIVED
   LANDING — fold_binary_expression at an anonymous destination derives
   its landing from a LANDED operand (one witness, left first). Two
   anonymous literals keep the transitional window. The latent
   landing-aware signedness arm now fires end to end.
   The STATIC-TABLE sub-rung (RuntimeStaticValues → IntegerLiteral)
   stays BANKED, not built: acceptance is green without it; revive when
   a selection-recorded static (a TABLE round-trip, not a binding
   substitution) is caught stripping a landing. CR3 FACE CLOSED
   2026-07-16 — D14 Fire H: the literal-width gate now blesses a
   u64-magnitude literal in TRANSITION-ARGUMENT position when the
   target state's declared parameter is u64-classed (the same
   Named-target/non-self-param zip the F2c float stamping rides; the
   frame-slot arg writer already reads bits). Pinned:
   pass/arithmetic/u64_magnitude_transition_arg_exit (differential 70;
   struct-field + guard-equality fires exercised en route) + fail twin
   u64_magnitude_arg_non_u64_rejected (u32 param keeps the loud
   rejection). Remaining CR3 face: classify resolvers reading
   landings (consumer migration continues with CR4/CM1).
   CR4 — parse-site stamping. INTEGER HALF (CR4a) LANDED 2026-07-18:
   width suffixes stamp landings at parse (Exact domain — destination
   landings still govern their own folds by precedence);
   isize/usize/nat stay accepted-but-anonymous. The soundness edge that
   makes stamping safe: suffix-vs-declared-type DISAGREEMENT is a loud
   error (validate_suffix_landings — let/assignment/struct-field
   destinations, through Mutable). Payoff rides CR3's operand-derived
   folds unchanged: `z - 1u64` folds at u64 and the operand-position
   max compares unsigned (pass/arithmetic/
   suffix_landed_operand_position_exit, differential 77; fail twin
   suffix_type_disagrees_rejected). MAGNITUDE FIT LANDED 2026-07-18:
   validate_suffix_magnitudes (omega-validation/literals.rs) checks
   every landed literal's spelled VALUE against its suffix's range at
   validation (post-negation-fold, so `-128i8` is one literal valued
   -128 and fits while a bare `128i8` errors — the caveat resolves
   itself). Value semantics per ch5 (suffixes read VALUES, not bit
   patterns): `0xFFi8` is 255 and errors; spell `-1i8` or `0xFFu8`.
   Canaries: fail suffix_magnitude_overflow_rejected (200i8) +
   suffix_negative_unsigned_rejected (-1u8); pass
   suffix_boundary_magnitudes_exit (70; -128i8/127i8/255u8/i64::MIN
   boundaries pinned, differential). Remaining CR4 face: float
   literals → exact Rat (= the float ladder's F2 rung, tracked there;
   F2a/F2b landed).
   CM1 — anonymous carrier goes EXACT: unbounded integer + Rat constants
   (now = CR1's spelling payload + value_bignum, ALREADY LANDED via D14
   + N2; the remaining face is CR3's consumer migration).
   CM2 — landed constants carry their type/domain/format (the
   metadata-carrying constant). FIRST RUNG LANDED 2026-07-18: the
   state-values folder now folds AT THE LANDED TYPE. A DESTINATION
   LANDING — the let/assignment target's declared width/signedness/domain
   — threads through the simplifier's value spine (binary operands +
   Mutable wrappers only; indices/args/fields get None) into
   fold_binary_expression; fold_landed (folding.rs) normalizes operands
   to the landed type, computes exact in i128, and lands per domain:
   Exact/Wrapping wrap to width, Saturating CLAMPS (closes the
   domain-stripping hole's clamp face where a landing derives), Trapping
   overflow passes the EXACT value through so the existing store-trap arm
   fires (transitional; documented in land_result). Sign-sensitive folds
   (`>>` `/` `%` + comparisons) are now correct at the landed type; the
   local-initializer role mislabel (CallArgument → AssignmentValue,
   runtime_dispatch.rs) was the wiring fix that lets the landing apply.
   SECOND FACE ALSO CLOSED same day — selection's transition-arg
   materialization lowers a slot-backed local's substituted initializer
   as a TYPELESS nested runtime binary whose `>>`/`/`/`%` defaulted
   signed; select_runtime_storage_binary_write_in_table now takes the
   write TARGET's primitive as the signedness fallback of last resort
   (validation guarantees the value lands at the target's type; the
   frame-slot funnel derives it from the slot). Both former pending
   repros PROMOTED to pass canaries
   (arithmetic/const_fold_unsigned_{shift_right,divide}_arg_exit; the
   divide one grew the modulo third face). The whole const-fold SIGN
   class is closed end to end. The u64 min/max face closed the same way
   — the non-table mutation write's operator adjustment
   (signedness_adjusted_operator_for_tree_operands) now takes the write
   TARGET as its last-resort probe, mirroring the table variant's
   operand-then-target order; pending repro PROMOTED
   (arithmetic/unsigned_min_max_wrapping_local_exit, 77 both engines).
   Remaining CM2 scope: the metadata-carrying CARRIER itself
   (u64-at-width-64 representatives stay bit-faithful-but-signless on the
   i64 carrier — fine now that consumers fall back to targets, but the
   carrier change retires the whole fallback family), the guard-folder
   f32 residue. TASK #133 OPENED 2026-07-20 (the place-algebra arc
   #127-#132 is COMPLETE -- 55 variants onto 11 place survivors, zero
   shape crossings remain; this item is the next front-loaded vein).
   ENTRY PROBE RESOLVED 2026-07-20 (no code needed): the
   "guard-folder f32 residue" note is STALE -- the landed F2c guard
   face already absorbed it, pinned by pass/float/
   f32_guard_const_arith_landed_exit (EXACTLY the divergence probe:
   2^24 + 1.0 == 2^24 at f32 where an f64 window takes the wrong arm;
   dual-engine exit 70, in the differential legs at line 139). CM2's
   REAL remaining rung = the metadata-carrying CARRIER. RETIREMENT
   CHECKLIST SCOPED: the consumer-fallback family the carrier change
   retires = signedness_adjusted_operator_for_tree_operands (3
   consumers: writes/mutation.rs, binary_table_writes.rs,
   value_operands.rs) + the write-TARGET signedness fallbacks
   (mutation.rs:2026, binary_table_writes.rs:655/716,
   frame_slots.rs:524). The rung: the folded-constant carrier grows
   type/domain/format (rides IntegerLiteral's landing, per the CR1
   design) so operands are never typeless at selection -- then the
   four fallback sites retire. CM3 DIFFERENTIAL LEGS LANDED
   2026-07-20: const_fold_saturating_narrow_exit (fold_landed's
   Saturating CLAMP at i8 127/-128 + u8 255 + unsigned u8 division;
   composed guards discriminate the bare-i64 window) +
   const_fold_wrapping_narrow_exit (i8 100+100 -> -56, u16 65535+2 ->
   1), both dual-engine exit 70 in the differential rows. With the
   existing u32 landed-ops + wide-saturating + trapping-fold-abort
   canaries the width/signedness/domain fold matrix is covered.
   GRAMMAR NOTE: multi-subject transitions spell PARENTHESIZED tuples
   (`transition (a == x, b == y) {`). TASK #133 SCOPE SETTLED: (a)
   f32 residue = stale note (resolved); (b) static-table carrier =
   repro-gated by the recorded CR3 decision (stays banked); (c)
   differential legs = LANDED; (d) float->Rat = demand-gated in the
   float ladder (F2c note: per-op rounding at width == the exact-Rat
   spec for homogeneous ops). All non-gated faces done. EN-ROUTE FIX
   2026-07-20: the windows session's slice-2 canary
   (windows_positioned_io_exit) landed in ACTIVE_PASS_CANARIES and
   broke the darwin sweep (its computed-offset pwrite arg only the
   msvcrt composition accepts; darwin's host-call fence refuses) --
   moved to WINDOWS_HOST_PASS_CANARIES per the dc_blit precedent, its
   cfg(windows) run twin untouched. LESSON (cross-host manifest rule):
   a canary whose omg only lowers on one host belongs in that host's
   gated sweep list, even when its run twin is already cfg-gated.
   A possible cheaper slot-COPY design for arg delivery
   (skip re-derivation when the local is slot-backed) stays noted in the
   promoted canaries' history but is superseded for correctness
   purposes.
   CM3 — fold-at-landed-type everywhere: folder + guard folder + interp
   parity; differential legs per width/signedness/domain/format.
6. **Place algebra, Copy* pilot (FRONT-LOADED by owner 2026-07-18;
   record: codegen_representation_cleanup Phase 6):** introduce Place
   (base + ConstOffset/ScaledIndex path) + the per-target materializer;
   route the Copy* family through it behind the differential oracle.
   No lane conflict now (parallel thread ended). The queue's remaining
   FRESH-SESSION item — a foundational IR refactor, load-bearing for
   everything, deserving a focused run with a clean head (not a
   tail-of-turn start).
   RUNG-1 SCOPE MEASURED 2026-07-18 (recon): the enum
   (representations/omega-abstract-operations/src/instruction/
   operation_kind.rs, 979 lines) has 100 variants; 18 are Copy*; the
   LIVE dispatch spine is SelectedInstructionKind → machine-emission
   encoding.rs (96 arms) → per-ISA encoders + the omega-relocations
   per-kind byte-walkers. (MachineInstructionKind is a parallel
   representation used by branch distances/shapes, NOT the encoding
   spine.)
   RUNG 1a LANDED 2026-07-18 (the pilot's pilot — bottom-up,
   byte-for-byte): `Place`/`PlaceStep` (ConstOffset | Deref |
   ScaledIndex; inline [PlaceStep; 4], Copy, ZII-inert, adjacent
   const offsets merge, depth saturates loudly) lives in
   omega-abstract-operations (re-exported through target-ops for the
   ISA crates); `encode_place_copy` (omega-isa-x86_64/place_copy.rs) =
   the materializer (r14 source / r15 target / rax chunk scratch;
   trailing const offsets fold into chunk displacements; ScaledIndex
   REFUSES loudly until the indexed rung). Three variants delegate
   BYTE-FOR-BYTE (unit-tested identity): plain CopyRuntimeStorage +
   both pointee shapes. Relocation walker untouched (same bytes, same
   kinds; Place::region is documentation on this path until the walker
   consumes places).
   RUNG 1b LANDED 2026-07-18 (x86_64; the fixed-indexed family):
   `encode_place_copy_shared_base` = the materializer's second entry
   (same-region pair, source starts with a deref: ONE base mov into
   r15, the source's first deref hops to r14 BEFORE a target deref
   consumes r15; a direct source REFUSES). to_frame + to_pointee
   delegate BYTE-FOR-BYTE through it (identity unit tests);
   to_runtime_storage CANONICALIZED to the two-base shape — target
   reloc moved +24→+17 (FRAME_FIXED_INDEXED_COPY_TARGET_IMM_OFFSET +
   the walker's offsets arm + the arch-dispatched width fn all in
   lockstep) and the old 1|4|8 single-chunk restriction LIFTED (any
   byte_count chunks now). PREMISE CORRECTION vs the old recon: the
   walker offset fns are ARCH-PARAMETERIZED FUNCTIONS (not shared
   constants) — x86-only canonicalization forks cleanly, no aarch64
   coupling. (1d) aarch64 DEFERRED DELIBERATELY: its encoders are
   register-nonuniform per variant (x16/x17/x20 shifting roles),
   second-reloc offsets are VALUE-DEPENDENT (add-constant width), and
   this host has NO aarch64 runtime oracle — byte-layout changes there
   are unverifiable; do 1d when an emulation/CI oracle exists, or
   byte-identically per variant with golden tests.
   RUNG 1c-i LANDED 2026-07-18 (ScaledIndex): the materializer's index
   discipline — at most ONE runtime index per place, loaded into r11
   (32-bit ZX, the append_load_r11_from_r14/r15 helpers) and scaled
   IMMEDIATELY AFTER the base materializes and BEFORE any deref
   consumes the base, `add reg,r11` at the step's walk position; on
   the shared-base path an index is legal only on a DEREFFING side
   (a direct side's add would mutate the shared base), one side max.
   Three runtime-indexed variants delegate: from_frame_indexed +
   from_indexed_to_pointee (same instruction multiset REORDERED —
   index hoisted pre-hop — same width, start-anchored single reloc,
   walker untouched); from_indexed_to_runtime_storage CANONICALIZED
   (target reloc +41→+34, FRAME_INDEXED_COPY_TARGET_IMM_OFFSET + the
   walker arm + the arch-dispatched width fn in lockstep; 1|4|8
   restriction lifted). 9 materializer unit tests incl. both indexed
   layouts + refusal cases.
   RUNG 1c-ii LANDED 2026-07-18 — THE PILOT'S FIRST FREE CAPABILITY:
   the runtime-indexed whole-element slice WRITE (`exits[index] = e`)
   now lowers on x86_64 (it refused with the zero-width blocker; the
   producer already guarded frame-source only, so the shared-base
   indexed-target shape covers every producible instance and the
   start-only walker arm fits unchanged). Wired: the ISA encoder
   (encode_runtime_storage_copy_to_runtime_frame_indexed delegating to
   the materializer) + widths.rs x86 arm + the selection dispatch arm.
   Canary pass/slices/runtime_indexed_element_copy_write_exit (70,
   differential; runtime index AND runtime struct source).
   FOUND EN ROUTE + FIXED SAME DAY: a STRUCT-LITERAL transition ARG
   did not deliver natively — argument materialization had a
   field-wise arm ONLY for CASE-BEARING literals; a plain record arg
   fell through to the scalar writer (plans nothing for an aggregate)
   and the callee param stayed ZII silently. The record arm now
   mirrors the variant arm (no tag; DataShape::Record field layout;
   int fast path + general per-field writer). Canary pass/calls/
   struct_literal_transition_arg_exit (70, differential; constant AND
   runtime field legs).
   RUNG 2a LANDED 2026-07-14 — THE CopyPlaces SPINE IS LIVE: ONE
   `CopyPlaces { source: Place, target: Place, byte_count }` variant in
   BOTH kind enums (+ conversion, classification, machine-kind →
   RuntimeStorageCopy so branch distances see the same guarded-effect
   class), and the relocation walker's arm PATCHES BY PLACE REGION:
   the x86_64 materializer records its base-mov sites from the SAME
   walk that emits the bytes (PlaceCopySites, side-tagged; never a
   hand-maintained offset constant — that whole lockstep failure class
   retires), and width = the encoder's output length (one source of
   truth). `encode_copy_places` picks the shape from the pair itself
   (same region + a dereffing side → shared-base; else two-base).
   aarch64 transitional: direct pairs decompose to the retired plain
   copy (byte-identical, old offset fn); deref/indexed refuse loudly.
   TWO producers migrated (writes/storage_copy.rs runtime_storage_copy
   + its in-table twin — the mutation-statement copies); blocker
   matchers extended in lockstep (storage/text/call-result/descriptor
   — a CopyPlaces write counts everywhere CopyRuntimeStorage did;
   direct-place targets expose their flat range, deref places claim
   none). Canary pass/data/runtime_whole_struct_mutation_copy_exit
   (cross-region field writes + same-region 16-byte whole-struct copy,
   report spells `copy places`, dual-engine + differential).
   RUNG 2b LANDED 2026-07-14 — ZERO CopyRuntimeStorage PRODUCERS: all
   17 remaining construction sites migrated through ONE
   `copy_places_direct` constructor (selection/runtime_dispatch.rs) —
   argument_materialization x4, leaf, straight_line x2, edges x2,
   frame_slots x2, mutation x2, subslice_copy x2, runtime_dispatch x2
   — plus the two rung-2a sites converted to the same helper. The
   variant's consumer arms (encoding/layout/relocations/shapes/report/
   blockers/conversion) stay until the deliberate retirement rung.
   Every dispatch copy in the corpus now rides CopyPlaces (suite
   877/877 incl. EFI cross-targets + the aarch64 cross-emit,
   differential 14/14, samples green).
   RUNG 2c-i LANDED 2026-07-14 — CopyRuntimeStorage RETIRED, the
   pilot's FIRST variant deletion: the variant is gone from BOTH
   enums plus every echo (conversion arm, 2 classifications, encoding
   arm, layout arm + width dispatcher + both ISA width/encode fns,
   shapes arm, relocation walker arm, report arm, 5 blocker rows, 2
   unit tests) — the ~15-file echo product for one variant, deleted
   compiler-driven. KEPT deliberately: aarch64's ISA-level
   encode_runtime_storage_copy + runtime_storage_copy_width (the
   CopyPlaces direct-pair decompose rides them) and
   runtime_storage_copy_target_address_offset (the aarch64 walker
   arm). 17 variants remain.
   RUNG 2c-ii LANDED 2026-07-14 — THE POINTEE FAMILY MIGRATED: all 15
   producers (10 to-pointee + 5 from-pointee) build [Const, Deref,
   Const] places via copy_places_to_pointee/_from_pointee helpers.
   The aarch64 transitional path generalized into
   classify_copy_places_shape (instruction-selection encoding) —
   Direct | ToPointee | FromPointee decompose to the retired aarch64
   encoders; General refuses; the relocation walker's aarch64 arm
   routes by the SAME classifier so offsets always describe the
   emitted bytes (linux_arm64 cross-compile of the deref shape
   verified). x86 same-region pointee pairs now take the tighter
   shared-base form (behaviorally equal, differential-verified).
   GOTCHA HIT (own memory note): two EFI report pins asserted the OLD
   kind's report spelling — updated to the place spelling incl. the
   flat-fold anti-assert (`[ConstOffset(72)]`).
   RUNG 2c-iii LANDED 2026-07-14 — BOTH POINTEE VARIANTS RETIRED
   (variants 2 and 3 of 18): CopyRuntimeStorageToRuntimePointee +
   CopyRuntimePointeeToRuntimeFrame deleted from both enums with
   every echo (conversion, classifications, shapes, encoding/layout
   arms, both walker arms, report arms, blocker rows, the
   x86 ISA encode/width fns and all four cross-arch dispatchers).
   KEPT: aarch64 ISA pointee encode/width fns (the CopyPlaces
   decompose rides them) + both walker offset fns (the CopyPlaces
   aarch64 arm). 15 Copy variants remain.
   RUNG 2c-iv-a LANDED 2026-07-14 — THE FIXED-INDEXED FAMILY FOLDS TO
   DEREF PLACES: all 7 producers build [Const(desc), Deref,
   Const(idx*size + field)] sources (a compile-time index is just
   displacement); the retired ToFrame/ToStorage region split
   COLLAPSES at both storage_copy sites (the region rides the
   place). New PointeePair shape (both sides deref) in
   classify_copy_places_shape: aarch64 decomposes it to the retired
   fixed_indexed_to_pointee encoder (index 0 / size 1 fold, both
   pointer slots frame-guarded, refuses otherwise); its walker arm =
   the start reloc only (one frame base serves both derefs). All
   three fixed-indexed variants now have zero producers
   (retire-ready → 2c-iv-b).
   RUNG 2c-iv-b LANDED 2026-07-14 — THE FIXED-INDEXED TRIO RETIRED
   (variants 4-6 of 18): all three CopyRuntimeFrameFixedIndexed*
   variants deleted from both enums with every echo (conversions,
   classifications, shapes, encoding/layout arms, walker arms + the
   fixed-indexed offset fn, report arms, blocker rows, all six
   cross-arch dispatchers, all six x86 ISA encode/width fns). KEPT:
   aarch64's fixed_indexed_to_pointee encode/width (the PointeePair
   decompose rides it); the +17 layout pin lives on as a place_copy
   unit test. SIX variants retired total; 12 remain.
   RUNG 2c-v LANDED 2026-07-14 — THE RUNTIME-INDEXED FAMILY ON
   ScaledIndex PLACES: all producers of the four frame-indexed
   variants (FromIndexed ToFrame/ToStorage — both region splits
   COLLAPSED, incl. the edges.rs call-result copy — ToIndexed
   element writes, IndexedToPointee) build
   [Const(desc), Deref, ScaledIndex{frame}, Const(field)] places
   via copy_places_from_indexed/_to_indexed/_indexed_to_pointee.
   Classifier gains FromIndexed | ToIndexed | IndexedToPointee
   (single_indexed_path; frame index slots only, else General);
   aarch64 decomposes route to the four retired encoders with
   frame-region guards (FromIndexed picks ToFrame/ToStorage by the
   TARGET PLACE REGION); walker arms mirror the retired reloc
   shapes (machine targets reload their base at the indexed offset
   fn; all-frame shapes ride the one start reloc). x86 = the 1c
   materializer discipline (shared-base same-region, two-base
   cross-region; machine-source indexed writes now lower correctly
   by construction where the retired kind was frame-assumed).
   RUNG 2c-vi LANDED 2026-07-14 — THE RUNTIME-INDEXED FOUR RETIRED
   (variants 7-10 of 18): CopyRuntimeStorageToRuntimeFrameIndexed +
   the three CopyRuntimeFrameIndexedTo* variants deleted from both
   enums with every echo (conversions, classifications, shapes,
   encoding/layout arms, walker arms, report arms, blocker rows incl.
   the call-result write-target extractors — CopyPlaces' direct-target
   arm covers those ranges — all eight cross-arch dispatchers, all
   eight x86 ISA encode/width fns). KEPT: all four aarch64 indexed
   encoders (the decomposes ride them) +
   runtime_storage_copy_from_runtime_frame_indexed_target_address_offset
   (the CopyPlaces walker's machine-target FromIndexed arm). TEN of 18
   variants retired; 8 remain (machine-indexed group + double-indexed
   + FrameBaseIndexed + machine-to-machine).
   RUNG 2c-vii LANDED 2026-07-14 — THE CROSS-REGION INDEX + THE
   MACHINE-INDEXED PAIR: prepare_place_index serves an index slot in
   a DIFFERENT region than the place base — r11 first materializes
   the INDEX region's base (a recorded SourceIndex/TargetIndex
   relocation site; Place::scaled_index_region feeds the walker),
   then loads the index through itself (no new scratch register
   enters the discipline; unit-pinned byte layout + site list). New
   direct_indexed_path classifier (no deref — machine statics inline
   arrays) with FromMachineIndexed | ToMachineIndexed shapes; aarch64
   decomposes to the retired machine-indexed encoders (which take
   index_region themselves); the walker's aarch64 arm now picks the
   START symbol PER SHAPE (the machine-array WRITE opens with the
   machine base, not the source). All 5 machine-indexed producers
   migrated; both variants producer-free.
   RUNG 2c-viii LANDED 2026-07-14 — THE MACHINE-INDEXED PAIR RETIRED
   (variants 11-12 of 18): CopyRuntimeMachineIndexedToRuntimeStorage +
   CopyRuntimeStorageToRuntimeMachineIndexed deleted with every echo
   (incl. the x86 chunked-read internal helper + its retired-encoder
   test module — the materializer's own chunk pins in place_copy.rs
   keep that coverage; the two x86-only to-machine walker offset fns
   died with their arm). KEPT: both aarch64 encoders (the decomposes)
   + the machine-indexed offset fns the CopyPlaces walker arms use.
   RUNG 2c-ix LANDED 2026-07-14 — FrameBaseIndexed RETIRED (variant
   13): the frame inline-array read is a no-deref frame-rooted
   single-index place (no new capability); classifier
   FromFrameBaseIndexed shape, aarch64 decompose to the retired
   encoder (all-frame = the one start reloc), the single producer
   migrated, the variant + echoes deleted (incl. the dead x86 chunk
   helpers the machine-indexed retirement orphaned).
   THIRTEEN of 18 variants retired; 4 remain — ALL two-runtime-index
   exotics (machine-to-machine indexed, MachineDoubleIndexed pair,
   FrameBaseDoubleIndexed): each needs a SECOND index register in
   the materializer or stays hand-spelled — decide at their rung
   (r11 is deliberately the single index scratch).
   DECIDED + RUNG 2c-x SLICE A LANDED 2026-07-19 (the second index
   register): r10 joins as the SECOND index scratch — new x86
   primitives (load_index_r10_from_r14/r15/r10, imul_r10_imm32,
   add_r14/r15_r10; mov_r10_imm64 already existed); prepare_place_index
   loads up to TWO indices (r11 first — byte-identical to the
   single-index rung — r10 second) while the base register still
   equals the region base, walks consume by ordinal (first ScaledIndex
   adds r11, second r10, all three walkers); cross-region second
   indices record SourceIndex2/TargetIndex2 sites (PLACE_COPY_MAX_SITES
   4→6; the walker patches nth(1) of Place::scaled_index_regions —
   new ordered accessor). Refusal moves to THREE indices. Unit pins:
   double_index_same_region_layout, double_index_cross_region_records
   _both_sites, triple_index_refuses; the old two-index refusal
   flipped to is_ok. NO producers yet — zero behavior change (battery
   green). NOTE: machine→machine single-index-EACH-SIDE (the 4th
   exotic) needs NO r10 — the two-base path loads r11 sequentially per
   side; its rung is pure producer migration.
   SLICE B READ-HALF LANDED 2026-07-19: classifier
   `direct_double_indexed_path` (Const*, SI, Const*, SI, Const* no
   deref; mid-const folds into field -- pure adds commute) +
   CopyPlacesShape::{FromMachineDoubleIndexed, FromFrameBaseDoubleIndexed}
   (checked BEFORE the single arms; recognizers refuse each other's
   shapes); aarch64 decomposes to the retained
   from_runtime_{machine,frame_base}_double_indexed_to_runtime_storage
   encoders; the CopyPlaces walker arms mirror the old kind arms
   (machine: shared frame base when an index is frame-resident +
   target at the retained offset fn; frame: one start base + target
   at its offset fn); helpers copy_places_from_{machine,frame_base}
   _double_indexed build [Const, SI, SI, Const] places; ALL SIX read
   producers migrated (storage_copy.rs plain+in-table pairs,
   frame_slots.rs pair). GOTCHA: the frame-double classifier arm
   must NOT gate on target.region -- the retained encoder is
   ..._to_runtime_storage (any const target; the walker patches by
   region); the over-gate broke the frame double-indexed read canary
   on aarch64 first try. x86 rides the slice-A materializer.
   SLICE B WRITE-HALF LANDED 2026-07-19: CopyPlacesShape::
   {ToMachineDoubleIndexed, MachineIndexedPair} (the pair = one index
   EACH side, both machine, recognized inside the source-single arm);
   aarch64 decomposes to the retained
   to_runtime_machine_double_indexed_from_runtime_storage +
   machine_indexed_to_machine_indexed encoders; walker: the machine-
   targeted double write opens with the TARGET base (start_region
   match extended), shared frame base when the source or an index is
   frame-resident; the pair arm mirrors its old kind arm (frame index
   per side + the second machine base). Helpers copy_places_to_machine
   _double_indexed / _machine_indexed_pair; BOTH mutation.rs producers
   migrated. GOTCHA: the #40 dual-indexed soundness fence's planned-
   check (storage_blockers.rs dual_indexed_copy_is_planned) matched
   only the OLD pair kind -- extended to accept a CopyPlaces whose
   BOTH sides carry a ScaledIndex (the fence itself stays; this crate
   names places via omega_target_operations, not abstract-operations).
   ALL FOUR exotic variants are now PRODUCER-FREE (retire-ready).
   RUNG 2c-x RETIREMENT LANDED 2026-07-19 -- THE COPY FAMILY IS DONE:
   all four two-runtime-index exotics deleted from both enums with
   every echo (conversions, classifications, shapes arms, encoding/
   layout arms deleted BALANCE-AWARE -- the naive `\n        }`-anchored
   arm regex swallowed neighboring arms whose bodies end `),`, caught
   by exhaustiveness errors and redone with a brace/paren-depth
   walker; walker kind arms, report arms, blocker OR-rows, the
   planned-check's old-kind arm, the 4 machine-emission cross-arch
   dispatchers, the 4 instruction-selection cross-arch dispatchers +
   re-exports, the 4 zero-reference x86 ISA encode fns). KEPT: the
   aarch64 encoders + the whole offset-fn chain (the CopyPlaces
   decomposes/walker arms ride them) + x86 width fns the offset
   dispatchers reference; the Write* double-indexed family is a
   DIFFERENT family and stays. SEVENTEEN of 18 Copy variants retired
   -- CopyPlaces is the sole surviving copy operation. The Write/RMW
   ladder (the leaf-cascade duplication dies) is the pilot's next
   family; then Text, guards/operands, op-set shrink per the wiki
   ladder. WIKI CHECKBOX MARKED (codegen_representation_cleanup.md).
   WRITE RUNG 1a PLAN (banked 2026-07-19): the INTEGER-write
   sub-family pilots (7 variants, all "store an immediate at width to
   a place-shaped target": WriteRuntime{Machine,Storage,Pointee,
   FrameIndexed,FrameBaseIndexed,MachineIndexed,MachineDoubleIndexed}
   Integer). Introduce `WritePlaceInteger { target: Place, value,
   byte_count }` beside CopyPlaces + the x86 materializer entry
   `encode_place_integer_write` (materialize the target address via
   the EXISTING place walk -- r15/r11/r10 discipline unchanged --
   then store the immediate in chunks through rax); delegate the
   simplest variant byte-for-byte first (WriteRuntimeStorageInteger),
   then migrate producers shape by shape with aarch64 decomposes to
   the retained Write encoders, mirroring the Copy rungs exactly.
   WRITE RUNG 1a LANDED 2026-07-19 (the materializer entry, zero enum
   churn): `encode_place_integer_write(target, value, byte_size)` in
   place_copy.rs -- the target address materializes through the SAME
   walk as the copy entries (r15 base, r11/r10 index discipline
   unchanged), the value stages through rax, the residual const folds
   into the width store's displacement. Unit-pinned: a DIRECT place is
   byte-for-byte the retired integer-write layout; an INDEXED target
   rides the index discipline. Re-exported for rung 1b.
   WRITE RUNG 1b LANDED 2026-07-19 (the first delegations, in-ISA):
   encode_runtime_machine_integer_write (serves BOTH the machine and
   storage integer-write kinds) and encode_runtime_pointee_integer_write
   now DELEGATE byte-for-byte through encode_place_integer_write
   (direct place / [Const(ptr), Deref, Const(field)]; the pointee
   width debug_assert stays as the identity sentinel; the transitional
   place's region is documentation -- direct-place bytes never consult
   it, the walker patches from the kind's own region). The INDEXED
   integer-write encoders do NOT delegate yet: their retired layout
   stages the index through RAX (mov rax,[base+idx]; imul rax), not
   r11 -- delegation there is a CANONICALIZATION, queued as rung 1c.
   WRITE RUNG 1c FIRST HALF LANDED 2026-07-19:
   encode_runtime_machine_indexed_integer_write delegates -- a pure
   REGISTER RENAME (index staged in r11 not RAX; a frame-resident
   index base in r11 not r10): every instruction width matches
   position-for-position, so the walker's +10 frame-base offset and
   the width fn hold AS-IS (the width debug_assert is the sentinel);
   the differential legs oracle the byte change. FRAME-BASE-INDEXED
   LANDED same day -- the pilot's first CANONICALIZATION: 47 -> 44
   bytes (the retired layout's redundant `mov r15,r14` dies; the
   materializer opens the base in r15 directly); the width fn shrank
   in lockstep, the one frame-base start reloc unmoved, differential
   green. FRAME-INDEXED (descriptor-deref)
   LANDED same day: same multiset REORDERED (index pre-loads into r11
   while r15 still equals the frame base, before the deref consumes
   it; the retired layout used a separate r14 base) -- same width,
   one start reloc, differential green. FIVE of six integer-write
   x86 encoders now ride the materializer.
   MACHINE-DOUBLE-INDEXED LANDED
   same day -- WRITE RUNG 1c COMPLETE (all six integer-write x86
   encoders ride the materializer): the deepest canonicalization --
   each frame-resident index now materializes its OWN base (r11
   outer at offset 10, r10 inner at 10 + (17|7) + 7) where the
   retired layout shared one r10; the width fn became per-region sums
   (61 + 10 per frame index; both-frame grew 71->81); the
   WriteRuntimeMachineDoubleIndexedInteger walker arm split PER-ARCH
   (aarch64 keeps its retired shared-base layout + offset fn; x86
   patches per-index via the new
   runtime_machine_double_indexed_integer_write_{outer,inner}_frame_
   offset pass-throughs). Differential green.
   RUNG 2a ECHO PRODUCT LANDED 2026-07-19 (zero producers -- the
   variant is inert until 2b): `WritePlaceInteger { target: Place,
   value, byte_size }` in BOTH enums + conversion + classifications
   (RuntimeWrite) + report arm + encoding arm
   (encode_write_place_integer: x86 -> the materializer; aarch64
   REFUSES LOUDLY until its decompose rung -- producers stay on the
   shape-specific kinds there) + layout width = the encoder's output
   length (write_place_integer_width, one source of truth) + the
   walker arm patching BY PLACE REGION from the materializer's own
   sites (x86_64_encode_write_place_integer_with_sites; Source* sides
   unreachable -- a write materializes only the target) + the shapes
   machine-kind arm (keeps the plain integer write's machine shape,
   the CopyPlaces precedent).
   AARCH64 DECOMPOSE LANDED 2026-07-19: WritePlaceShape (the
   CopyPlacesShape twin for ONE place: Direct | Pointee | FrameIndexed
   | FrameBaseIndexed | MachineIndexed | MachineDoubleIndexed |
   Unsupported) + classify_write_place_shape -- encoder and walker
   classify with the SAME fn (consistent by construction); the aarch64
   encode arm decomposes to the six retained integer-write encoders;
   the walker's aarch64 branch anchors the target-region base at
   instruction start (every retained layout does) + the machine-
   indexed frame-index reloc + the double's shared frame base.
   WritePlaceInteger is now fully servable on BOTH architectures.
   RUNG 2b PRODUCERS PART 1 LANDED 2026-07-19: the constructor family
   (write_place_integer_{direct,pointee,frame_indexed,base_indexed,
   double_indexed} in selection/runtime_dispatch.rs), the blocker
   rows (runtime_text + storage OR-groups) and BOTH field-extracting
   matcher arms (call_result_blockers instruction_write_target +
   descriptor_argument_blockers instruction_frame_write_range --
   direct places expose their flat range, deref/indexed claim none),
   and the FIRST migration: both WriteRuntimeMachineInteger sites
   (branches/leaf.rs) build direct places. Battery green.
   RUNG 2b COMPLETE 2026-07-19 -- ALL 58 PRODUCER SITES MIGRATED (a
   field-aware transformer swept every construction across 13
   selection files; match PATTERNS skipped by rest-dots/arm
   detection); the prelude splice-covered-plain-write matcher now
   recognizes the DIRECT WritePlaceInteger shape (deref/indexed have
   no splice equivalent). Battery green incl. differential 14/14 --
   the seven integer-write variants are PRODUCER-FREE.
   RETIREMENT LANDED 2026-07-19 -- THE SEVEN INTEGER-WRITE VARIANTS
   ARE GONE: deleted from both enums with every echo (classifications,
   conversions, shapes arms, encoding/layout arms, walker arms
   incl. the per-arch double arm, report arms, blocker OR-rows, the
   two extracting matcher arms' old rows). KEPT: the six aarch64
   encoders + offset fns (the WritePlaceShape decompose rides them);
   runtime_storage_integer_write_kind (the WritePlaceInteger machine-
   kind arm uses it). SURGERY LESSONS (recurring): scripted arm
   deletion must anchor the arm END on the NEXT-ARM start (never a
   bare `}` alternative -- it matches the pattern's own close) and
   never swallow `_ =>` fallbacks; three files needed git-restore +
   redo. Dead delegating shells (x86 encode_*_integer_write fns +
   machine-emission dispatchers + the orphaned shapes *_kind helpers)
   remain as never-used warnings -- sweep at next touch.
   The INTEGER-WRITE COLLAPSE IS COMPLETE: 7 variants -> 1
   (WritePlaceInteger). NEXT FAMILY: the Binary writes -- RECON BANKED
   2026-07-19 (encode_runtime_storage_binary_write at x86 lib.rs:5868
   read): the binary write is REGISTER-ENTANGLED with the place
   discipline -- operand evaluation (append_runtime_value_operand)
   accumulates in r10, stages right in r11, and RELOADS r15 per
   operand source base, so the retired encoder holds the target in
   r14 ("mov r14,imm64"; same 10-byte width, reloc at +2). The
   COMPOSITION for encode_place_binary_write: (1) materialize the
   target place via the EXISTING walk (r15 + r11/r10 indices --
   consumed into the address BEFORE operands evaluate), (2) `mov
   r14, r15` (3 bytes) to protect the address across operand evals,
   (3) the existing operand/op/store appenders verbatim (floats,
   Saturating/Trapping, signedness untouched), storing to [r14 +
   residual displacement]. Direct places are the retired layout +3
   bytes with the start reloc moving from r14's imm to r15's imm
   (same +2 position) -- a canonicalization: width fns + the walker's
   OPERAND OFFSETS move in lockstep (left_offset currently = start +
   runtime_storage_binary_left_operand_offset(arch); the place form
   needs place_address_width(target) added -- deterministic from the
   place, ONE new width helper). Then: delegations per shape,
   WritePlaceBinary variant + echo + aarch64 WritePlaceShape reuse
   (the classifier is shape-only, operand-agnostic -- it transfers
   as-is), producers, retirement.
   BINARY RUNG 1a LANDED 2026-07-19: the ONE-SOURCE-OF-TRUTH
   extraction -- the target-address-agnostic half of every binary
   write (operand pair eval with the r10/push/r11 discipline, the
   full operator/domain dispatch: floats, Saturating/Trapping
   mul/shift/add-sub, Wrapping/Saturating signed div-mod, F8
   shift-count policies, the plain op) now lives in
   append_binary_operands_op_and_store; the retired
   encode_runtime_storage_binary_write delegates BYTE-IDENTICALLY
   (mov r14,imm64 + the helper); encode_place_binary_write = the
   place walk (r15 + indices, consumed into the address before
   operands evaluate) + `mov r14,r15` (new primitive 4d 89 fe) + the
   SAME helper. Battery green. Dead integer shells still pending
   sweep. POINTEE BINARY DELEGATED same day (the first shaped
   delegation): the place walk + r14 hop grows the operand-start
   prefix 17 -> 20; runtime_pointee_binary_operand_start_width moved
   in lockstep (it feeds BOTH the width fn and the walker's
   left-operand offset -- one constant, no drift); the Exact-domain
   tail is IDENTICAL through the shared helper (Exact never enters
   the domain arms -- semantic check recorded). Differential green.
   FRAME-BASE-INDEXED BINARY DELEGATED same day (27 -> 30, one
   lockstep constant) -- AND the index load canonicalized to the
   32-bit ZX discipline (the retired 64-bit index load could splice a
   neighboring slot's bytes into the high half; the materializer's ZX
   read is the correct one -- a latent-bug retirement).
   FRAME-INDEXED BINARY DELEGATED same day (34 -> 37; the descriptor
   deref hops r15 in place, index stays 32-bit ZX in r11, plus the
   r14 hop; the offsets test's own re-pin protocol followed -- the
   FRAME-BASE delegation commit had missed its pin, caught by the
   workspace battery POST-push and fixed forward 743ed1efe; batteries
   now GATE the commit inside one script).
   MACHINE-INDEXED BINARY DELEGATED same day -- AND THIS CLOSED A
   LIVE x86 MISPATCH WINDOW: its walker arm shares
   runtime_frame_base_indexed_binary_left_operand_offset (moved 27->30
   by the frame-base delegation) while its encoder still emitted the
   retired 27-byte prefix -- x86 operand relocations landed 3 bytes
   late for two commits (743ed1efe..c2a678145), INVISIBLE to the host
   battery (this Mac is the aarch64 oracle; x86 has no runtime leg).
   The delegation realigns encoder and walker (machine-index 27->30,
   frame-index 37->40; the frame-index base is a mov r11,imm64 at the
   SAME +10, so the walker's frame reloc and +10 operand shift hold).
   LESSON (recorded): when a SHARED offset constant moves, every kind
   consuming it must delegate IN THE SAME COMMIT -- grep the
   constant's consumers before moving it.
   DOUBLE-INDEXED BINARY DELEGATED same day -- BINARY RUNG 1b
   COMPLETE (all six binary x86 encoders ride the materializer):
   per-index frame bases replace the shared r10 (prefixes 44->47 /
   54->57 / 54->67 as per-region sums); the walker arm split per-arch
   IN THE SAME COMMIT as the offset move (the shared-constant lesson
   applied -- x86 per-index relocs reuse the integer double's
   outer/inner offset fns at the SAME positions, aarch64 keeps its
   retired shared-base layout).
   NEXT: WritePlaceBinary variant + echo product (the integer 2a
   pattern verbatim: variant in both enums + conversion +
   classification RuntimeWrite + report + encoding dispatcher x86->
   materializer / aarch64->WritePlaceShape decompose to the retained
   binary encoders + layout width from the encoder + walker via
   sites-and-shape + shapes machine-kind) + producers + retirement.
   BINARY RUNG 2a ECHO PRODUCT LANDED 2026-07-19 (zero producers):
   WritePlaceBinary { target: Place, byte_size, left, operator,
   right, is_float, domain, target_signed } in BOTH enums +
   conversion (remap_runtime_value_handle) + classifications
   (RuntimeWrite) + report arm + the machine-emission wrapper
   (validate_runtime_value_home x2, the retained idiom) + selection
   dispatcher (x86 -> encode_place_binary_write; aarch64 ->
   WritePlaceShape decompose to the six retained binary encoders,
   with shaped-non-Exact/float REFUSED -- matching today's producer
   split) + layout width = encoder length + the shapes machine-kind
   arm (plain binary shape, CopyPlaces precedent) + the x86 walker
   arm: base at start, each CROSS-REGION index base at its
   DETERMINISTIC prefix position (place_binary_index_base_positions,
   walk-summed twin of place_binary_operand_start_width -- no
   re-encode, no drift), operands at start+prefix (the existing
   left/right pattern); the aarch64 reloc arm is an explicit
   unreachable until producers land (refuse loudly, never
   under-patch).
   BINARY 2b LANDED 2026-07-19 -- ALL 24 PRODUCER SITES MIGRATED
   (storage x12, pointee x6, frame-indexed x4, frame-base x1,
   machine-indexed x1, double x1; the shaped constructors are
   Exact-only matching the retired field sets; the direct one carries
   the float/domain/signedness triple). The migration went LIVE
   mid-commit and immediately exercised the aarch64 walker branch --
   the placeholder unreachable was replaced with the per-shape mirror
   of the retained arms (start = target region; machine-indexed
   frame reloc + operand shift 8; double shared frame base; operand
   offsets via the retained per-shape fns; classified by the SAME
   classify_write_place_shape the encoder decomposes with).
   Matchers realigned in the same commit: blocker OR-rows (text +
   storage), the prelude splice direct row, call_result
   instruction_write_target + descriptor instruction_frame_write_range
   (direct places expose their range, deref/indexed claim none).
   Battery green incl. differential 14/14 -- the six Write*Binary
   variants are PRODUCER-FREE.
   RETIREMENT LANDED 2026-07-19 -- THE BINARY-WRITE FAMILY IS DONE
   (6 -> 1, WritePlaceBinary): all six variants deleted from both
   enums with every echo (classifications, conversions with the
   double-brace walk, shapes arms, the shapes/validation first/second
   operand-handle rows COLLAPSED onto WritePlaceBinary, walker arms,
   encoding/layout arms, report arms, blocker OR-rows, the
   call_result/descriptor old rows, the prelude splice old row, the
   instruction_bytes diagnostic-note rows). KEPT: the six aarch64
   binary encoders + per-shape operand-offset fns (the decompose and
   the WritePlaceBinary aarch64 walker branch ride them). Battery
   green incl. differential 14/14.
   PILOT SCOREBOARD: Copy* 18->1, integer writes 7->1, binary writes
   6->1 -- THIRTY-ONE variants retired onto THREE place-shaped
   survivors. DEAD-SHELL SWEEP (partial) same
   day: 21 zero-reference fns deleted (machine-emission dispatchers,
   shapes *_write_kind helpers); the x86 delegating shells remain
   PUB-CHAINED through uncalled selection dispatchers (dead-but-pub,
   invisible to intra-crate dead-code analysis) -- sweep the chain at
   next touch. TEXT RUNG 1a LANDED
   2026-07-19: encode_place_string_write -- the string DESCRIPTOR
   ({ptr -> rodata, len}) stores into a place-shaped target; the data
   pointer stages in r14 via the leading mov r14,imm64 (its
   data-object reloc ALWAYS at instruction start, the retired
   convention); the target address rides the standard walk (base site
   at +10 via PlaceCopySites). The machine + frame string encoders
   DELEGATE BYTE-FOR-BYTE (identical retired layouts, walker
   untouched). TEXT RUNG 1b LANDED 2026-07-19: the POINTEE string
   encoder delegates through the materializer -- width-identical
   position-for-position (the len stages in rax instead of a second
   r14 imm64, a pure register rename; data reloc stays at +2, frame
   base at +12, walker untouched). TEXT RUNG 1c LANDED 2026-07-19:
   the FRAME-INDEXED string encoder delegates -- total width holds
   (68) but the reloc ORDER flips to the materializer convention
   (data at start, frame base at +10), so the walker's
   WriteRuntimeFrameIndexedString arm arch-SPLIT in the same commit
   (x86 = new positions, aarch64 = retained layout untouched); the
   34-byte rax prefix helper stays live for the literal-APPEND
   encoder (its own walker arm + offset fn, unshared). TEXT RUNG 1d
   LANDED 2026-07-19: the MACHINE-INDEXED string encoder delegates --
   width holds (71), relocs move to data@0/machine@10/frame@20 (the
   cross-region index's own base), walker arm arch-split same commit
   (aarch64 retained; its frame offset fn stays live for the aarch64
   arm AND the WritePlaceBinary aarch64 walker branch -- the x86 arms
   of the two machine-indexed offset fns are now dead-but-pub, swept
   at retirement). ALL FIVE string encoders now ride the
   materializer. TEXT RUNG 1e LANDED 2026-07-19:
   encode_place_bounded_buffer_write -- the len word + IMMEDIATE
   content bytes (mov byte [r15+disp32],imm8) at a place-shaped
   target; base reloc(s) from the walk are the only sites. The
   machine + pointee bounded-buffer encoders delegate BYTE-FOR-BYTE
   (27+8n / 34+8n, walker untouched; target_in_frame stays a
   walker-side region choice). TEXT RUNG 2a LANDED 2026-07-19:
   WritePlaceString{target,data,byte_length} +
   WritePlaceBoundedBuffer{target,literal} variants + FULL echo --
   both enums + conversion (remap_data_handle / literal.clone),
   classifications, encode/width dispatchers (x86=materializer,
   aarch64=shape decompose to retained encoders; string serves
   direct/pointee/frame-indexed/frame-machine-indexed, buffer
   direct/pointee, else refuse), with_sites wrappers, layout arms,
   shapes plain kinds (RuntimeMachineStringWrite /
   RuntimeMachineBoundedBufferWrite), walker arms REAL on BOTH arches
   from day one (x86 = data@start + sites-by-region; aarch64 =
   per-shape retained positions), storage+runtime_text blocker rows.
   TEXT RUNG 2b LANDED 2026-07-19: constructor family
   (write_place_string_{direct,pointee,frame_indexed,machine_indexed}
   + write_place_bounded_buffer_{direct,pointee}) + ALL 27 producer
   sites migrated (string_values, writes/mutation+frame_slots,
   slice_descriptors, text_writes/builder+descriptor,
   branches/mutation); the retired target_in_frame flag became the
   place region at the two buffer sites. Zero retired-kind producers
   remain -- canary 864 exercises the new variants + walker arms live
   on this host. TEXT RUNG 2c LANDED 2026-07-19: RETIREMENT -- the seven
   variants (5 Write*String + 2 *BoundedBuffer writes; the 2 Append*
   RMW variants STAY) deleted from both enums + conversions +
   classifications + report + walker + emission encode/layout arms +
   blocker rows + shapes arms (brace-balanced/next-arm-anchor
   discipline, zero fallbacks swallowed). Canary 864 + differential
   14 green post-retirement. SCOREBOARD: Copy* 18->1, integer 7->1,
   binary 6->1, TEXT 7->2 -- 38 retired onto 5 place survivors.
   TEXT RUNG 2d LANDED 2026-07-19: DEAD-SHELL SWEEP -- 22 x86
   fns (7 delegating encoders + 7 width fns + 8 orphaned register
   primitives), 8 selection wrappers (7 arch-dispatch encoders +
   unsupported_x86_64_encoding), 7 selection width wrappers, 7
   emission wrappers, 9 pre-noted shapes/copies.rs orphans, 2
   residual relocation offset fns (string_descriptor_runtime_frame +
   the integer machine-indexed frame wrapper) = 55 dead fns deleted.
   KEPT: aarch64 retained encoders (the decompose path),
   MACHINE_INDEXED_STRING_{FRAME,DATA}_IMM_OFFSET constants (the
   widths.rs offset fns' x86 arms still reference them; those fns
   serve the WritePlaceString/Binary aarch64 walker arms), the
   FRAME_INDEXED rax prefix helper (the literal-append encoder).
   TASK #130 COMPLETE (7 -> 2 with echo + producers +
   retirement + sweep). TASK #131 OPENED (guards/operands ladder) --
   INVENTORY 2026-07-19: 68 variants remain in the abstract enum.
   NEXT COLLAPSE BANKED: the SIX address-to-frame writes
   (WriteRuntime{Storage,Pointee,FrameIndexed,FrameFixedIndexed,
   FrameBaseIndexed,MachineIndexed}AddressToRuntimeFrame -- all spell
   `frame[target_offset] = &place`) onto WritePlaceAddress{source:
   Place, target_offset} -- the proven arc, 5th run: (a) read the six
   x86 encoders + walker arms + width fns for prefix analysis, (b)
   materializer entry encode_place_address_write (materialize source
   address in r15, then mov r14,imm64(frame) + store r15->[r14+target]
   -- note the SOURCE address is the payload, a new wrinkle: the walk
   must NOT fold the trailing const offset into a store displacement
   but ADD it to r15 -- ALWAYS emit the add (retired encoders do,
   e.g. pointee width 31 fixed) for deterministic width. PREFIX
   ANALYSIS 2026-07-19 (partial): the retired encoders SHARE one
   frame base in r14 for source walk AND target store (one reloc for
   frame-rooted sources) -- the materializer form uses TWO bases
   (source walk in r15 + mov r14,imm64(frame) + store r15->[r14+
   target]), so delegations are NOT byte-for-byte; widths and reloc
   positions move -- every delegation needs its walker arm moved in
   the SAME commit. Note runtime_frame_indexed_deref_address_
   to_runtime_frame_write already uses a materializer-like prefix
   (FRAME_INDEXED_COPY_TARGET_IMM_OFFSET=34 comments) -- read it
   first; the six encoders live at x86 lib.rs 391 (pointee, 31),
   408 (fixed-indexed, 31), 534 (base-indexed), 566
   (machine-indexed), 5385 (storage direct), + the frame-indexed
   deref fn near 455), (c) delegations with lockstep walker updates,
   (d) variant + echo + producers + retirement + sweep. PLAN REVISED
   2026-07-20: 4/6 delegations would change widths anyway (the
   retired encoders SHARE one frame base; the materializer form uses
   two) so the family SKIPS the delegation phase -- variant + echo
   first (retired encoders stay byte-stable until unproduced), then
   producers, then retirement. ADDRESS RUNG A LANDED 2026-07-20:
   WritePlaceAddress{source: Place, target_offset} + full echo --
   encode_place_address_write (walk in r15 + ALWAYS-emitted residual
   add + mov r14,imm64(frame)@width-17 + store), aarch64 decompose to
   the six retained encoders incl. the machine-index deref shape via
   the NEW pub place_frame_deref_indexed_path helper (encoder+walker
   share it -- classify refuses that shape for value writes), walker
   arm REAL both arches (x86 sites-by-region + frame@len-17; aarch64
   per retained shape: direct source@0+frame@offset-fn, pointee/
   frame-indexed frame@0, base-indexed frame@0+optional-fn,
   machine-indexed copy-family offset fns, deref machine-idx
   frame@0+machine@32), emission encode/layout arms, shapes plain
   kind, storage+text+descriptor blocker rows, report arm. ADDRESS
   RUNG B LANDED 2026-07-20: constructor family (write_place_address_
   {direct,pointee,fixed_indexed->pointee-shape,frame_indexed_deref,
   base_indexed,machine_indexed}) + ALL 22 producer sites migrated
   (wire_encode x2, runtime_dispatch machine-indexed,
   argument_materialization x2, subslice_copy, slice_descriptors x7,
   frame_slots x9) -- every address write now rides the materializer
   natively on this host (canary 864 green). ADDRESS RUNG C
   LANDED 2026-07-20: RETIREMENT + SWEEP -- the six variants deleted
   from both enums + conversions + classifications + report + walker
   + emission encode/layout arms + blocker rows + shapes arms; then
   34 dead fns swept (6 emission wrappers, 6 selection encode
   wrappers, 8 selection width/offset fns incl.
   runtime_machine_indexed_address_relocation_offsets +
   runtime_frame_indexed_deref_address_target_frame_offset, 14 x86
   encoders/width fns/consts). KEPT: aarch64 retained encoders
   (decompose), runtime_storage_address_to_runtime_frame_target_
   frame_offset + runtime_frame_base_indexed_address_target_frame_
   offset + the copy-family machine-indexed offset fns (the
   WritePlaceAddress aarch64 walker arms ride them),
   FRAME_BASE_INDEXED_ADDRESS_TARGET_FRAME_IMM_OFFSET (widths.rs x86
   arm). SCOREBOARD: Copy* 18->1, integer 7->1, binary 6->1, text
   7->2, ADDRESS 6->1 -- 44 variants retired onto 6 place survivors.
   THE COMPARE family
   ANALYSIS 2026-07-20 (banked): the wiki step is guards CONSUME
   Places (representation upgrade -- indexed guard subjects without
   hoisting), NOT a variant shrink. Scope: CompareRuntimeStorage ->
   ComparePlaces{left: Place, right: Place, byte_size, operator,
   is_float}; CompareRuntimeStorageValue -> ComparePlaceValue{place,
   byte_size, expected_value, operator}. OUT of scope:
   CompareRuntimeValues (operand-based), text compares,
   EvaluateDispatchGuard (guard-lowering carrier). KEY MECHANICS:
   compares encode through instruction_bytes.rs (NOT encoding.rs)
   with failure_branch_distance from branch_distances::byte_distance_
   to_next_runtime_write_end; retired layout = mov r15,imm64(left
   base)+load r10 (7|8 for 2-byte)+mov r15,imm64(right)+load r11+cmp
   (3|4)/float ucomisd+jcc rel32 (+6 jp parity pre-branch for
   floats); value form = load + mov r11,imm64(expected)+cmp+jcc.
   MATERIALIZER ENTRY encode_place_compare: walk LEFT with
   AddressRegister::Source (r14, mirrors CopyPlaces -- sites
   unambiguous) + load r10<-[r14+residual]; walk RIGHT with Target
   (r15) + load r11; cmp; jcc. Direct places = position-identical to
   retired (r14 rename on the left leg). REGISTER FENCE: a
   TWO-INDEX right place would clobber r10 (the left operand) --
   refuse loudly (legalization principle). Walker arm mirrors
   WritePlaceInteger (sites-by-region; compares' retired walker =
   runtime_storage_compares.rs; left base @0, right base @17|18).
   Producers: 8 sites. COMPARE RUNG A LANDED
   2026-07-20: encode_place_compare (left walks in r14/Source + load
   r10 via the NEW append_load_reg_from_r14 primitive; right in
   r15/Target + load r11; cmp/ucomisd + failure branch; TWO-INDEX
   right places refuse -- the register fence) +
   encode_place_value_compare (walk + load r10 + mov r11,imm64 + cmp;
   any shape) + ComparePlaces/ComparePlaceValue variants + FULL echo:
   conversions, GuardEvaluation classifications, selection dispatch
   (aarch64 = Direct-only via classify, else refuse) + width fns
   (encode-at-distance-0) + with_sites, layout arms,
   instruction_bytes arms (branch_distances::byte_distance_to_next_
   runtime_write_end), encoding.rs handled-elsewhere rows, shapes
   plain kinds, report arms, walker arms REAL both arches (x86 =
   sites by side+region over TWO subjects; aarch64 = retained direct
   positions). COMPARE RUNG B LANDED 2026-07-20:
   compare_places_direct + compare_place_value_direct constructors +
   all 8 producer sites migrated (edges.rs x2 incl. the
   guard-conjunct arm, guards.rs x6) -- every storage guard compare
   rides the place form natively (canary 864 green). COMPARE RUNG C LANDED
   2026-07-20: RETIREMENT + SWEEP -- CompareRuntimeStorage +
   CompareRuntimeStorageValue deleted from both enums + conversions +
   classifications + shapes + report + walker + instruction_bytes +
   layout + encoding rows; 11 dead shells swept (2 selection encode
   wrappers, 2 selection width wrappers, 4 x86 encoder/width fns,
   append_add_r14_imm32, 2 shapes kind helpers). KEPT: aarch64
   retained compare encoders (decompose) +
   runtime_storage_compare_right_address_offset (the ComparePlaces
   aarch64 walker arm). GUARDS NOW CONSUME PLACES (the wiki step) --
   direct today, indexed/deref subjects unlock when producers pass
   walked places. SCOREBOARD: 46 variants retired onto 8 place
   survivors (CopyPlaces, WritePlace{Integer,Binary,String,
   BoundedBuffer,Address}, ComparePlaces, ComparePlaceValue).
   TASK #131 COMPLETE
   (guards consume Places). TASK #132 OPENED (op-set shrink, wiki
   Phase 6 tail): FIRST UNIT BANKED 2026-07-20 -- the 9-variant TEXT
   CROSSING (Materialize/AppendStored/AppendLiteral x direct/pointee/
   frame-indexed; the wiki names this product explicitly) collapses
   9 -> 3: MaterializeTextBufferToPlace{buffer, target: Place},
   AppendTextStoredToPlace{buffer, source_region, source_offset,
   target: Place}, AppendTextLiteralToPlace{buffer, target: Place,
   literal}. LIGHTER ARC than the write families: NO new materializer
   entries -- encode/layout/walker DECOMPOSE by
   classify_write_place_shape to the retained per-shape encoders on
   BOTH arches (the aarch64 transitional pattern applied to x86 too);
   the payoff is killing the 15-file echo duplication at the
   representation level. TEXT-CROSSING RUNG A LANDED 2026-07-20:
   MaterializeTextBufferToPlace + AppendTextStoredToPlace +
   AppendTextLiteralToPlace variants + FULL echo -- conversions,
   classifications (the retired group's domain), encoding + layout
   DECOMPOSE arms (classify_write_place_shape -> retained per-shape
   encoders/width fns on both arches, else refuse loudly at encode;
   layout _ => 0 is unreachable-in-practice since encode refuses
   first), walker arms per shape (direct/pointee share buffer@0 +
   target-at-offset; frame-indexed flips to frame@0 + buffer/source
   at indexed offset fns), text/storage/host_argument blocker rows,
   shapes plain kinds, report arms. TEXT-CROSSING RUNG B LANDED 2026-07-20: text_place_
   {direct,pointee,frame_indexed} place builders + ALL 11 producer
   sites migrated (text_writes/builder.rs: 2 materialize-direct + 6
   stored-append + 3 literal-append; the pointee/frame-indexed
   MATERIALIZE retired variants had ZERO producers -- already dead).
   Canary 864 green. TEXT-CROSSING RUNG C LANDED 2026-07-20:
   RETIREMENT -- the 9 crossing variants deleted from both enums +
   conversions + classifications (or-group arrow moved to the
   AppendTextLiteralToPlace row) + shapes + report + host/text/
   storage blocker rows + encoding/layout/walker arms; 9 dead shapes
   kind fns swept. The machine-emission runtime_text encode wrappers
   STAY LIVE (the decompose arms ride them -- the transitional
   pattern). SCOREBOARD: 55 variants retired onto 11 place survivors
   (the 8 prior + MaterializeTextBufferToPlace, AppendTextStoredToPlace,
   AppendTextLiteralToPlace); enum now 55 variants (from ~100 at the
   pilot's start). CLASSIFICATION PASS 2026-07-20 (the #132 inventory,
   wiki checklist updated): (a) place-shaped survivors 11; (c)
   genuinely distinct ~44 -- dispatch control 9, host/platform 5,
   port I/O 2, wire codec 10, atomics 2 (place-consumable later,
   deep), entry/return ABI 4 (CopyRuntimeStorageToReturnRegister a
   mini-collapse candidate), text builder/byte utils 6, bounded RMW
   appends 2, convert 1 (single-shape today), text compares 2,
   CompareRuntimeValues 1. ZERO per-shape opcode crossings remain --
   the Cartesian disease is ELIMINATED. RECORDED CALL: full
   value-category-on-operand unification deferred as diminishing
   returns (no shape crossing left to kill; payload-enum indirection
   would save ~zero arms) -- revisit when the aarch64 materializer
   lands. TASK #132 CORE COMPLETE. Then producers + retire 9 done -> classify
   the remaining ~53 variants (already-place-shaped / collapsible-via
   -value-category / genuinely-distinct dispatch-host-wire-atomic /
   straggler mini-collapses); walked-place guard subjects stay a
   banked selection-side feature. Then the
   bounded-buffer entry (immediate content bytes, single base reloc),
   then WritePlaceString/+Buffer variant + echo + producers +
   retirement. Then guards/operands consume Places, then the op-set
   shrink.
   Rung 2a after: the WritePlaceInteger variant + echo product +
   producer migration. Pre-existing dead helpers noted en route (shapes/copies.rs
   runtime_storage_copy_kind + _to_runtime_frame_indexed_kind +
   from/to_machine_indexed_kind -- orphaned by EARLIER retirements,
   swept opportunistically at the next touch).
   Then Write/RMW (the leaf-cascade duplication dies), Text,
   guards/operands, op-set shrink — the wiki ladder. Legalization
   refuses loudly at every rung.

7. **Semantic taxonomy representation rework (OWNER-LOADED 2026-07-18;
   serious prerequisite, record:
   wiki/architecture/semantic_taxonomy_representation.md):** the settled
   domain facets, machine taxonomy, core multiplicity, kinded effect rows, and
   termination guarantee/ranking-witness split
   are currently LOST
   in the compiler's old shapes (`DomainDefinition { facts, operators }`,
   `Machine { boundary: bool, terminates: bool, decreases, effects }`,
   `DataProperties { copy, zero_init, send }`, move/drop-only ownership
   summaries). Land explicit semantic representations before building the
   feature arcs on conditionals that later have to be excavated. Rungs:
   STR1 audit/snapshot invariants — LANDED 2026-07-16:
   representations/omega-typed-trees/tests/semantic_taxonomy_inventory.rs
   pins all FIVE loss shapes by destructure (DomainDefinition's
   undifferentiated fields; Machine's boundary/terminates booleans +
   flat effects span; DataProperties' three booleans; EffectSet's flat
   bit surface; the move/drop-only StateOwnershipSummary), each with
   the record's must-survive invariant in its doc — a pin break means
   RE-PIN the new shape and check the named distinction survived,
   never delete; STR2 core enums/IDs — LANDED 2026-07-16 (no behavior
   change, zero consumers yet): foundation/omega-core/src/semantics.rs
   carries Multiplicity (default Affine), MachineSupplyMode
   (CheckedBody | Requirement | Boundary | Accepted), decision 23's
   TerminationGuarantee / RankingWitness / MachineTerminationPlan (the
   witness lives BESIDE the published half — the firewall is the
   shape), decision 22's EffectMemberKind, the ZII-inert identity
   handles (SemanticDomainId, EffectMemberId, EffectRowId,
   ProgressProfileId, RankingViewId), and the DomainFacets PAIR
   skeleton (optional facets, never an enum — hybrids first-class);
   unit tests pin the defaults and the witness-blind published half;
   STR3 propagation through
   symbol-resolved/typed trees and snapshots — FIRST SLICE LANDED
   2026-07-16: DataProperties (both layers) carries `multiplicity`,
   POPULATED at the syntax->resolved lowering ([copy] -> Unrestricted,
   ordinary -> Affine; [linear] unspelled yet) and COPIED — never
   re-derived — through resolved->typed (all four construction sites);
   the STR1 LOSS-3 pin re-pinned per its own protocol (`copy` = the
   compatibility bool until STR7). SLICE 2 LANDED same day: both Machine
   records carry `supply_mode: MachineSupplyMode` (populated ONCE at
   syntax->resolved — Boundary | CheckedBody today; Requirement comes from
   trait requirements and Accepted is the existing bodyless `boundary machine`
   proof form — copied through
   resolved->typed; `boundary` = the compatibility bool; LOSS-2 pin
   re-pinned, noting the guarantee/witness conflation and flat effect
   span REMAIN). Remaining STR3: the termination-plan
   propagation is TPR2'S JOB (decision 23's front-loaded ladder owns
   the termination spelling — populating MachineTerminationPlan from
   the OLD terminates/decreases shape would be re-derivation the TPR
   pass immediately replaces; the core types from STR2 stand ready);
   the effect-row propagation rides STR4's normalizer (decision 22) --
   STR4 SLICE 1 LANDED 2026-07-16: omega-core gained the CANONICAL
   kinded member catalog (23 names, name-for-name consistency with
   omega-effects' legacy bit table PINNED by a cross-crate test that
   already caught one drift; OperationalMay = {thread_block,
   sync_wait}, ServiceReach otherwise) + the deterministic
   EffectRowTable interner (sorted/deduped member sets;
   EffectRowId(1) = the fixed EMPTY row; identity independent of
   spelling order and the legacy bits); SymbolResolvedTrees +
   TypedTrees carry the interner (copied verbatim); both Machine
   records carry `effect_row: EffectRowId` populated ONCE at
   syntax->resolved from the flat effects span, copied downstream;
   LOSS-2 + LOSS-4 pins re-pinned (the flat EffectSet remains an
   independently-built compatibility carrier until STR6/7 make it a
   derived projection). STR4 SLICE 2 LANDED 2026-07-16: CheckFacts
   gains `effect_rows: EffectRowFacts` -- per machine, the PUBLISHED
   ceiling (the authored clause's normalized row) beside the
   checker-INFERRED direct/transitive summary rows, interned into the
   typed table EXTENDED prefix-stably (bit->member hops through the
   CANONICAL NAME, never the bit value); pinned by a ceiling-vs-
   reality test (main has no clause -- EMPTY ceiling -- but its
   transitive row equals its callee's declared row). NOTE: today's
   EffectPlan counts the DECLARATION into its direct set, so
   declared==inferred on the declaring machine -- RESOLVED BY STR4
   SLICE 3 (LANDED 2026-07-16): MachineEffects gains `body_observed`
   (state + call direct sets, computed in the plan builder; the
   declared seed was ALREADY structurally separate -- `direct` is the
   authored clause, never mutated) and the checked inferred_direct
   row normalizes the declaration-free observation set; the slice-2
   test now pins ceiling != inferred_direct on the declaring machine.
   TRANSITIVE-SEED REWORK LANDED 2026-07-16: MachineEffects gains
   `body_transitive` -- the same call-graph fixpoint seeded WITHOUT
   the machine's own clause; a callee contributes its declared
   CEILING when it has one (ceiling enforcement guarantees it covers
   the body; the callee may change within it without recompiling the
   caller -- the modular bound), else its own honest reach.
   EffectRowFacts.inferred_transitive now carries it; the slice-2
   pin extended (the declaring machine's inferred_transitive is the
   EMPTY row while its ceiling names filesystem_io; the caller still
   sees the ceiling through the call edge). CEILING ENFORCEMENT verified
   ALREADY LIVE 2026-07-16 (run-probed: "declares effects `clock_read`
   but reaches undeclared effects `host_boundary`"; pinned:
   fail/capabilities/effect_ceiling_exceeded). CHECKED-PLANS SLICE 1
   LANDED 2026-07-16: omega-core gains SemanticDomainTable (the
   EffectRowTable twin) -- deterministic SemanticDomainId minting
   per DECLARED domain (declared-name identity, declaration order,
   NULL=0 reserved); both tree layers carry the table (copied
   verbatim, surviving the finish() rebuild) and DomainDefinition
   (both layers) carries `semantic_id` populated ONCE at
   syntax->resolved. Pinned: semantic_domain_ids_mint_and_propagate
   (two declared domains mint distinct valid ids; the typed table
   resolves them back; the resolved table agrees) + the omega-core
   interner determinism unit. Remaining STR4:
   pinned-slot refinement (provider-slot machinery -- profile-
   adjacent);
   snapshots gain the new fields as those slices land;
   CHECKED-PLANS SLICE 2 LANDED same day: the arithmetic policies
   (Wrapping/Saturating/Trapping -- the compiler-blessed closed
   semantic-facet subset) PRE-SEED SemanticDomainTable with FIXED
   ids 1-3 (proof-cache-safe; declared domains follow), and
   CheckFacts gains QualificationFacts -- per machine, the sorted/
   deduped SemanticDomainId set its body's `as`-casts COMMIT to
   (the body-observed half; a full statement+expression walk incl.
   transition guards/targets; cast-free machines carry no entry;
   the published AUTHORITY half is queued on the now-ruled shared
   grant/receipt carrier).
   Pinned: qualification_facts_record_policy_commitments (a
   Saturating cast commits the fixed id; a cast-free machine is
   absent). SLICE 3 LANDED same day -- the qualification SPELLING
   is recognized: the cast's `in` slot accepts any identifier (the
   parser carries non-policy names as `semantic_domain`, a
   HandleSpan through all three Cast representations + every
   copier/rebuilder); validation judges it -- a DECLARED domain gets
   the STAGED fence naming it and the missing mint rung
   (introduction authority + predicate discharge), an unmatched name
   gets the honest unknown error WITH the declaration check the
   parser could not perform (the old "unknown arithmetic domain" at
   parse time was misleading for declared domains). Pinned: fail
   domains/semantic_cast_mint_staged +
   domains/semantic_cast_unknown_domain. THE MINT v1 LANDED same
   day -- the FIRST live semantic-domain qualification:
   `5 as i64 in Km` mints a LITERAL into a declared domain
   (in-program authority is the owning package's -- sealed-vs-open
   bites at package boundaries, which do not exist in-program; the
   PREDICATE obligation folds every domain fact at the literal,
   `self := 5`). Three outcomes pinned: pass
   domains/semantic_cast_literal_mint (facts true, RUNS exit 70 --
   zero-cost, representation unchanged); fail
   semantic_cast_fact_false (self >= 10 at 5 -- the "predicate
   obligation not discharged" class); fail semantic_cast_mint_staged
   re-pinned to the flow-integration fence (runtime values route
   through validating calls/guards until that rung). QualificationFacts
   DECLARED-DOMAIN ROWS LANDED same day: an accepted mint's cast
   resolves its short name to the declaration (the judge's
   exact-or-::suffix rule) and commits the declaration's interned
   identity beside the policy ids (pin extended: Main::minted
   commits Km's id). FLOW-INTEGRATED MINT v1
   LANDED same day: a RUNTIME value qualifies when its DECLARED
   RANGE entails every domain fact (interval entailment, the R2
   slice-9 discipline -- and the same declared_place_type_RAW gotcha:
   the plain variant strips the Constrained shell the range lives
   in). The judge moved into the machine/state loop for context;
   the positional sweep keeps literal-only judging for strays.
   Pinned: pass domains/semantic_cast_range_mint
   (raw: i64 [0..=4096] entails self >= 0, RUNS exit 70); fail
   semantic_cast_range_insufficient ([-10..=4096] does not entail --
   undischarged, not false). THE GUARD CHAIN
   LANDED same day -- the mint's third discharge route: the
   machine's own REQUIRES facts about the cast value accumulate
   one-sided bounds (`requires raw >= 0` -> low = 0) that entail
   the domain facts; the CALLER proves those requires at a
   co-located guarded call site through the EXISTING R1 machinery
   -- so `transition sensed >= 0 { true -> take(qualify(sensed)) }`
   + `machine qualify(raw) requires raw >= 0 { -> (raw as i64 in
   Km) }` runs a RUNTIME value through a guard into a semantic
   domain END TO END. (The statement walker also gained transition
   TARGETS -- casts in arm values/arguments were unjudged.) Pinned:
   pass domains/semantic_cast_guard_chain_mint (RUNS exit 70); fail
   semantic_cast_requires_missing (requires dropped -- the caller's
   guard cannot reach inside the callee; the requires is the
   sanctioned carrier). CONTRACT PLANS SLICE 1 LANDED
   same day: CheckFacts gains MachineContractPlans -- per machine,
   the published halves already carried on the records (supply
   mode, effect-row ceiling, published termination guarantee) plus
   a deterministic FNV fingerprint over them, folding the row's
   catalog-fixed MEMBER ids (never program-local table indices).
   Prover-independence (acceptance 8) holds by construction: only
   declared material enters. Pinned:
   contract_plans_fingerprint_published_halves (same declared
   surface -> same fingerprint across different bodies; a changed
   effects clause -> different). FACT CANONICALIZATION
   LANDED same day (slice 2): the declared requires/ensures facts
   enter the fingerprint in a stable prefix-walk byte encoding
   (operator tags, name paths as text, exact literals; Membership
   folds the domain path), the fact SET sorted before folding so
   clause order never enters the identity. Pin extended: reordered
   requires -> same fingerprint; a changed bound -> different.
   POSITIONAL NORMALIZATION LANDED
   same day (slice 3): a contract fact naming the machine's Nth
   parameter encodes as P<N>, so renames never change the identity
   (pin: bounded_renamed with alpha/beta matches bounded_ab's x/y
   fingerprint). Remaining: the boundary-facing calling plan. PERMISSION
   PLANS DESIGN-RULED 2026-07-17: validate structural facts freely, then admit
   semantic commitments through the same chapter 10 grant/receipt carrier used
   by sealed domain introduction and progress profiles; selection remains a
   separately held slot-owner capability. Build the shared carrier once for all
   three consumers.
   Remaining: STR5
   validation/resolution; STR6 lower only from checked selections while
   preserving semantic contract IDs in artifacts; STR7 retire compatibility
   paths. Decision 22 now supplies the effect-row target: kinded
   `ServiceReach | OperationalMay` identities, deterministic normalized rows,
   explicit published ceilings, inferred internal summaries, and pinned-slot
   refinement. Authority, trust, resources, failure, and mutation remain
   separate fields; the flat `EffectSet` survives only as a compatibility/cache
   projection.

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
- **R2 (where-clause + gating + windows) — RUNG 1 LANDED 2026-07-16
  (the SPELLING, TPR1 staged-surface pattern):** `data M where
  count <= len, { ... }` parses into the new syntax
  DataDefinition.where_facts (proof-fact span, comma-separated,
  trailing comma tolerated, ends at the body brace; same clause
  position generics will use) and the syntax->resolved lowering
  REFUSES it loudly until rung 2 consumes the default-domain model
  (pinned: fail/dependent/data_where_clause_unconsumed + a parser
  unit). RUNG 2 SLICE 1 LANDED same day -- ZERO-GATING
  classification: the facts lower onto the RESOLVED
  DataDefinitionStorage.where_facts and classify AT ZERO via a v1
  constant folder (names read 0; literals; + - *; comparisons;
  &&/||): zero-SATISFYING clauses are ADMITTED (born established;
  facts stay INERT until rung 3 wires entailment hypotheses + write
  obligations ATOMICALLY -- an unenforced standing invariant must not
  be consumable); a GATED type (zero violates the domain) refuses
  until rung 2b lands construction-mandatory fields; an unfoldable
  fact refuses as outside the v1 fragment (pinned: pass
  dependent/data_where_zero_satisfying RUNS exit 70; fail
  data_where_gated_unsupported replaced the rung-1 unconsumed pin).
  SLICE 2 LANDED same day: the admitted facts COPY to the TYPED
  DataDefinition.where_facts (re-lowered like machine-contract facts;
  inert, propagation-pinned). RUNG 2b LANDED same day -- GATED
  CONSTRUCTION: gated types are ADMITTED (zero_gated stored on both
  records; the slice-1 refusal retired) and every LITERAL of a
  domain-carrying type must PROVE the default domain -- the where
  facts fold at the literal's field valuation (named integer values;
  omitted fields read the ZII zero; a where-mentioned runtime-valued
  field refuses as unverifiable until rung 3's prover). Ch12's Player
  example is the acceptance shape (pinned: pass
  data_where_gated_literal_proves RUNS exit 70; fail
  data_where_literal_violates -- omitted health reads 0, not a
  Player). RUNG 3 SLICE 1 LANDED same day -- the WRITE OBLIGATION
  (obligations BEFORE hypotheses: over-refusal is safe,
  over-assumption is not): a NEW default_domains validation pass
  walks each state linearly tracking `self`-rooted domain-carrying
  places (machine-owned = born zeroed, so untracked fields read 0);
  every store to a where-mentioned field re-folds the facts at the
  post-write valuation (integer-literal stores tracked; a
  whole-place struct literal reseeds from rung 2b's proven
  construction; runtime-valued constrained stores refuse directed;
  any call poisons the tracking -- conservative aliasing fence).
  Strict store-time semantics; ch11 windows are the sanctioned
  relaxation (pinned: fail data_where_write_violates -- count=9
  against zeroed len; the sequential len=8-then-count=3 pass canary
  discharges). RUNG 3 SLICE 2 LANDED same day -- the ACCESS GATE:
  TrackedPlace gains `established` (zero-satisfying places born
  established; a GATED place earns it through rung 2b's proven
  literal or an accepted constrained write, since every accepted
  write re-proves the whole domain); reads of an unestablished gated
  self-place refuse with direction (member chains in value positions,
  scanned before each statement's write effect; cross-state
  establishment is a later rung -- v1 same-state only, no corpus
  impact). Pinned: fail data_where_read_before_establish (zeroed
  Player.health read); the gated-literal pass canary now reads AFTER
  construction (the gate opens). RUNG 3 SLICE 3 LANDED same day --
  CROSS-STATE establishment: a MUST analysis over the state graph
  (established at entry of S = established at exit of EVERY
  predecessor; edges resolved by target SYMBOL, the termination
  graph's proven rule, with name fallback; SelfTarget = self-edge;
  bottom-start fixpoint = least/under-approximation -- loop-carried
  establishment stays conservative, over-refusal only). SOUND because
  establishment is globally monotone in the strict model: every
  accepted write anywhere re-proves the domain. Pinned: pass
  data_where_cross_state_establish (construct in entry, read in a
  successor state -- RUNS exit 70); the same-state fail pin keeps
  refusing with the updated construct-on-every-path direction.
  RUNG 3 SLICE 4 LANDED same day -- SOUNDNESS FIX: slice 1's
  untracked-reads-zero fold was valid ONLY in the never-re-entered
  boot state (machine-owned fields persist; in a later state an
  untracked field may hold any prior value, so a `!=`-shaped fact
  could wrongly ACCEPT a violating write -- an unsound accept, not
  over-refusal). walk_state now threads `born_zero` (state 0 with no
  incoming edges); elsewhere untracked fields POISON the fold and
  refuse with a directed message naming both causes (pinned: fail
  data_where_cross_state_unknown_refuses -- the len=3-then-count=3
  `count != len` shape; entry-state canaries unaffected). RUNG 3
  SLICE 5 LANDED same day -- cross-state VALUATION transport: the
  fixpoint is now COMBINED (establishment as before + Kildall must
  constant propagation: non-boot entries start TOP/unvisited; meet
  keeps a field only when every visited predecessor exits it with the
  SAME literal; a CALL poisons valuations at exit while establishment
  survives -- it is globally monotone). walk_state seeds
  freshly-tracked places from the transported entry valuation, so the
  len=8-in-entry-then-count=3 shape DISCHARGES again (pinned: pass
  data_where_cross_state_valuation RUNS exit 70), and the slice-4
  `!=` shape now refuses with the PRECISE violation (len=3
  transported -> 3 != 3 folds FALSE; its pin re-aimed from the poison
  message to `violates`). Bodyless machines guarded (the zero-state
  seed panicked -- caught by the canary gate, fixed same commit).
  RUNG 3 SLICE 6 LANDED same day -- the write net is TOTAL: writes
  through `&mut` data PARAMETERS carry the same obligation (probe
  confirmed the hole: `target.count = 99` through a param sailed
  through before). Any Name-rooted chain tracks now; born_zero is
  PER-PLACE (self-rooted x boot only -- param/local places arrive
  domain-VALID but value-UNKNOWN, so constrained writes poison unless
  the fact folds from written fields alone or a whole-place literal
  reseeds); param places count established for the access gate
  (caller's net enforced arrival validity); cross-state transport
  stays self-rooted (params are per-invocation). Pinned: fail
  data_where_param_write_unproven; pass data_where_param_write_proves
  (single-field fact folds without co-field knowledge, RUNS exit 70).
  RUNG 3 SLICE 7 LANDED same day -- READER HYPOTHESES, the payoff:
  a Member read of a domain-carrying place intersects its interval
  with the where-fact-implied bounds (where_fact_interval in
  default_domains, hooked into arithmetic_domains' operand fallback;
  symmetric comparison shapes; bounds from literals or the co-field's
  DECLARED range -- never flow values). Sound because the write net
  is total and gated reads are access-gated: the facts hold at every
  legal observation. The pin is a TRUE differential:
  pass data_where_hypothesis_discharges -- inside a callee with NO
  flow knowledge, `target.count * 16` proves Exact ONLY via the
  standing `count <= len` + len's [0..=4096] (negative probe without
  the clause refuses with the decision-17 obligation); RUNS exit 70.
  RUNG 3 SLICE 8 LANDED same day -- ch11 INVARIANT WINDOWS, the
  sanctioned ADDITIVE relaxation: a checkable-but-FALSE constrained
  write no longer refuses at the store -- it OPENS a window
  (TrackedPlace.window_open); a later write folding the facts TRUE
  closes it; CONSUMPTION POINTS refuse while open (reads of the place
  -- any domain-carrying type, the gate generalized past zero_gated
  with the establishment refusal correctly narrowed back to gated
  types; CALL statements -- the callee observes state; and STATE
  EXIT). Unfoldable writes stay refused (closure must be checkable).
  Pinned: pass data_where_window_closes (the shrink-len-then-count
  reorder that strict store-time refused, RUNS exit 70); the
  write-violates and cross-state-unknown fail pins re-aimed at the
  window-closure message (their violations now surface at the call
  consumption point). RUNG 3 SLICE 9 LANDED same day --
  PROVER-BACKED CONSTRUCTION: the literal check folds over INTERVALS
  (integer literals as points; Name/Member values by their DECLARED
  ranges via declared_place_type_RAW -- the unwrapping variant strips
  the Constrained shell, the first-try bug; omitted fields read 0);
  saturating interval +,-,* (4-product min/max); comparisons yield
  tri-state truth composing through &&/||; definitely-false refuses
  as violation, unknown refuses directed. Ch12's Player constructs
  from a RUNTIME `strength: i32 [1..=100]` parameter with no spelled
  requires (pinned: pass data_where_ranged_param_constructs RUNS exit
  70; point pins unchanged). RUNG 3 SLICE 10 LANDED same day --
  PRODUCT HYPOTHESES, ch12's MemoryMap VERBATIM: `count * stride <=
  len` with the sibling fact `stride >= 40` and len's [0..=4096]
  bounds count at floor(4096/40)=102 at every legal observation
  (sound iff the co-factor's lower bound >= 1 -- declared range or a
  single-level sibling literal fact -- and the field is UNSIGNED);
  param/local reads gained the arrival-validity rule mirroring the
  slice-6 write path (a gated-type param arrived valid). True
  differential pinned: pass data_where_product_hypothesis (the
  callee's `target.count * 32` proves u32 ONLY via the product
  chain, RUNS exit 70; dropping the stride fact refuses with the
  decision-17 obligation). RUNG 3 SLICE 11 LANDED same day --
  CROSS-MACHINE ESTABLISHMENT: per-machine SUMMARIES (v1:
  single-state machines, walked with born_zero=false and no nested
  summaries -- sound alone since internal calls clear tracking)
  record the self places a callee DEFINITELY establishes; call sites
  (statement + expression positions, target STATE symbol resolved to
  its owning machine -- the effects builder's proven rule, the
  first-try bug) join the summary into call_established, consulted by
  the read gate, exit-establishment, and fresh-place seeding.
  Establishment is globally monotone, so a call only ADDS. Pinned:
  pass data_where_callee_establishes (construct in recruit, read in
  main -- the slice-9 flow that had to be reworked, now RUNS exit 70);
  the no-call fail pin keeps refusing. MULTI-STATE callee summaries
  LANDED 2026-07-16: a multi-state callee runs the same must-fixpoint
  the main pass uses (intersection meet over predecessors,
  born_zero=false throughout -- a callee runs at arbitrary times) and
  the summary intersects the exit sets of the TERMINAL states only
  (no outgoing transition -- the only return points; a dispatch
  state's own exit is not one; cyclic graphs summarize as nothing).
  Pinned: pass data_where_multistate_callee (two-arm dispatch, both
  exits establish, RUNS exit 70) / fail
  data_where_multistate_partial_refuses (one arm does not establish
  -- the intersection empties and the read refuses). TRANSITIVE
  field-vs-field hypotheses LANDED same day: bound_source_interval
  falls back from the co-field's declared range to the co-field's
  OWN where-fact interval (depth-capped at 4; cycles resolve None --
  over-refusal only), so `count <= mid, mid <= capacity[0..=100]`
  chains. Pinned: pass data_where_chained_hypothesis (RUNS exit 70)
  / fail data_where_cyclic_hypothesis_refuses. (Direct
  field-vs-field with a RANGED co-field was already slice 7's
  MemoryMap shape.) WINDOW TRANSPORT LANDED same day -- THE R2
  REFINEMENT LIST IS CLEARED: an open ch11 window may cross a
  transition and close in a successor state. walk_state carries
  entry windows ((spelling, data) pairs) and a terminal flag; the
  fixpoint MAY-unions windows over predecessors (an obligation from
  ANY path in); calls and TERMINAL exits (no outgoing transition --
  where the machine returns) stay hard consumption points; reads of
  a transported-open place refuse; a write that re-proves the facts
  closes the inherited window. Pinned: pass
  data_where_window_transport (open in entry, restore in successor,
  RUNS exit 70) / fail data_where_window_unclosed_terminal (never
  restored -- the exit call and terminal exit both refuse, naming
  the predecessor-state window). The rest of the big semantic build:** the
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

## Ranked termination — implementation migration and landed substrate

Normative record: decision 23,
`design_briefs/termination_ranking_and_progress.md`; chapters 3/9/10/18.
The landed MR implementation below uses the retired
`terminates { decreases ...; }` representation. Preserve its proofs and
constant-stack lowering while TPR1–TPR6 migrate the source and semantic IR to
`terminates [by ...]`. Terminating state and call cycles require a joint
well-founded ranking; productive state loops may diverge without one. Runtime
recursive calls remain tail-only; proof-stratum non-tail recursion never
lowers. Historical rungs:

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
  complement landed in the legacy ranking checker
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
  decision-17 argument fold nor the legacy ranking checker uses the
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
  walk (unchanged until MR4's joint measures). Under decision 23 the range in
  `terminates by m -> View in a..=b` constrains the produced rank; it is a
  termination fact only (floor = well-foundedness bound, any
  start; dependent endpoints legal; nothing sized from a range).
  Whole-program worst-case stack line = longest chain of the
  acyclic-after-lowering call graph. Record: mathematical_proofs “Measured recursion”
  amendment.
- **MR4 — mutual cycles:** joint (lexicographic) measures across the
  cycle, every call along the cycle tail-classified;
  the dungeon's find_item_at/find_item_after pair is the live test case
  (currently absorbed by bounded clone specialization).
  SLICE 1 LANDED 2026-07-19 (qualification machinery; every cycle STILL
  refuses): call_cycles.rs now tags each cross-machine edge tail/non-tail
  (tail = the `self.X(..)` transition ARM TARGET spelling; statement and
  value-position calls are non-tail; multiple sites between one pair AND
  together) and the Q6 refusal appends an MR4 SHAPE CHECK verdict --
  either "every edge is a tail transition and every member is measured --
  the joint-measure admission is pending cross-machine tail-call
  lowering" or "NOT met" naming each non-tail edge and unmeasured
  machine (witness presence = termination_plan.implementation_witness or
  the compat decreases span). Canaries:
  fail/calls/mutual_cycle_{qualified,disqualified}_shape.
  ⚠️ ADMISSION GATE DISCOVERED (recorded, not a design question): the
  backend has NO cross-machine tail-call lowering -- arm-target calls
  grow the stack; the old dungeon pair only ran because BOUNDED clone
  specialization unrolled it. Admitting a measured-but-unbounded cycle
  would trade the Q6 static refusal for a runtime stack overflow.
  Slice 2 (the joint-measure decrease check across edges) is meaningful
  only WITH the TCO lowering; both ride together as the admission rung.
  ADMISSION LANDED 2026-07-20 (task #135): the exploration map
  DISSOLVED the TCO gate -- the whole program is ONE dispatch loop
  (every machine state a case; every transition arm target a
  SetDispatchState jump over ONE overlaid frame region), so
  cross-machine tail arm targets NEVER grew the stack; the old
  refusal was the only gate. Landed: (a) call_cycles.rs ADMITS a
  cycle whose every edge is a tail arm target, every member measured,
  and every edge PROVEN to strictly decrease the callee's measure --
  the v1 prover recognizes the `m == 0`-guarded base arm (incl. the
  bool-subject `(m == 0) == true` lowering, unwrapped) + `m - 1`
  tail-argument shape, single-subject witnesses, position-matched
  against the callee's entry params; anything else keeps the refusal
  with the verdict naming the unproven edges; (b) INTERPRETER PARITY:
  run_state_collect_inner's cross-machine Named branch was RECURSIVE
  (CALL_DEPTH_BUDGET 512 halted admitted cycles at depth ~512, exit
  0) -- it now REBINDS the loop (machine/instance/state/args, carried
  locals cleared) and continues: a tail jump at constant depth,
  matching the native lowering. Canaries:
  pass/calls/mutual_cycle_tail_admitted_exit (dual-engine, n=100000
  -- inside the interpreter's 10M-step budget; the native probe ran
  40M alternations on constant stack),
  fail/calls/mutual_cycle_decrease_unproven (qualified shape, arg `n`
  unchanged -- refused naming the edge),
  fail/calls/mutual_cycle_disqualified_shape (retained). The old
  fail/mutual_cycle_qualified_shape canary RETIRED (its shape now
  compiles -- it became the pass canary). MASKING NOTE: the
  differential row list masks members after a tick-stop in list
  order; the admitted canary's row passed spuriously at n=300000
  before the interpreter fix -- the canary_suite test fn was the
  honest oracle. NEXT (banked): richer decrease shapes (guards
  `n > k` / `n >= k`, args `n - k`), multi-subject lexicographic
  joint measures -- DEMAND-GATED (no live consumer; the v1 shape
  covers the canonical spelling). DUNGEON PAIR: NO ACTION -- it was
  already refactored to the blessed in-machine state loop when Q6
  landed (inventory.omg documents it); migrating it BACK would be
  regressive churn. The admitted pass canary is the live test.
  QUEUE SWEEP RECONCILED 2026-07-20 (post-#135): the texteq pair and
  trailing-state-mut repro remain claimed by the fs lane's
  dispatch-region/receiver-phase family. `<machine M>` generics remain a
  genuine owner question blocking only N5/N6 schema axioms and Seq
  map/filter. The wider queue is NOT exhausted: field-default retirement,
  Nat strict-ranking evidence plus mod/gcd, the shared grant/receipt carrier,
  ProviderPlan migration, and the task-runtime ladder are design-ruled,
  engineering-ready work recorded in their own entries below. MR4 richer
  shapes, the CM carrier, and value-category unification remain demand- or
  repro-gated.
- **MR5 — proof-stratum evaluation — LANDED 2026-07-11 (pinned; the
  machinery already composed):** measured recursion evaluates at compile
  time under the const-eval ~100k-step fuel cap — the MR1/MR2 spellings
  interpret as loop-backs with no lowering and no space rule. Pinned by
  pass/comptime/runtime_const_measured_recursion_exit: `[u8;
  table_size()]` const-calls a zero-arg machine whose measured
  tail-recursive FREE-machine helper (MR2 bare terminal form) computes
  the length.

## Math roster & the Real arc — engineering track

Design record: mathematical_proofs “Quantification and proof data” and
“Real-number direction”. Proof-only is COMPUTED, never spelled: recursive data is
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
  (contract facts + ranking witnesses bless ANY-magnitude literals —
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
  Payload constructor terms LANDED same day: the PARENTHESIZED case
  literal `(Nat::Succ { prev: a })` already parses (parens reset the
  contract grammar's no-struct-literal context), both equality fences
  (resolved pre-pass + typed synthesis) stand down for RECURSIVE data in
  fact position, and the judge decomposes constructor equations —
  INJECTIVITY (`requires (Succ{a}) == (Succ{b}) ensures a == b` proves)
  and payload DISJOINTNESS (refutes) are in
  proof_nat_structural_lemmas + nat_payload_disjointness_refuted.
  Result binding LANDED same day for
  SOLE-unguarded-value-arm bodies (the identity lemma
  `-> (b)` / `ensures result == b` proves; wider bodies judge without
  the binding — weaker, never unsound). APPLICATION UNFOLDING LANDED
  same day (compute-mode): StructuralTerm::Application for free calls;
  resolution unfolds single-state case-arm proof machines when the
  matched argument resolves to a constructor (the desugared arm guard IS
  `subject == Data::Case`, read directly); callee bodies convert under a
  strict param environment (payload member reads index the bound
  constructor's fields; any out-of-env name aborts — no capture);
  fuel-capped. The lemma recognizer folds leading `let` locals (the
  terminal auto-hoist's shape). PROVEN LIVE: add_zero_left (extraction
  shape) + one_plus_one (ground 1+1==2); FALSE ground compute REFUTES
  (nat_ground_compute_refuted). STRUCTURAL INDUCTION LANDED same
  day: case-arm machines judge per arm (subject substituted by a
  fresh-variable constructor; result bound to the arm value; every
  self-application intakes the machine's own ensures as the inductive
  hypothesis — sound because the proof-machine recursion check refuses
  non-descending self-calls in the same batch); add_zero_right (right
  identity by induction) PROVES, false inductive claims REFUTE
  (nat_inductive_claim_refuted). Rewrite orientation LANDED same
  day: hypothesis equations with an application side intake as REDUCING
  rewrites (App -> rhs, occurs-checked), applied in resolution before
  unfolding — the IH now rewrites the self-application away instead of
  expanding variables. FACT CONSUMPTION LANDED same day:
  a caller CITES a callee's proven functional ensures (`result == term`)
  instead of unfolding its body — the only route for an INDUCTIVE lemma
  whose body never finitely reduces for a symbolic argument
  (`cite_right` discharges `result == a` by consuming add_zero_right's
  ensures; a false-conclusion consumer still fences,
  nat_lemma_citation_false_rejected). Sound by the same batch-validation
  argument as the IH. core nat.omg also carries
  mul_zero_left (left annihilator) and add_succ_law (the successor-shift
  LAW `add(a, Succ b) == Succ(add(a, b))` — an ensures that is an
  EQUATION BETWEEN APPLICATIONS, proven by induction; the discovery that
  application-equation laws prove with the EXISTING per-arm IH machinery,
  no new engine). NEXT LEVER (record: mathematical_proofs “Explicit proof
  citation” + ch10 "Citing Proofs" — NO global rewrite engine):
  a law reaches a consumer proof as an EXPLICIT STATEMENT CALL
  (`add_zero_right(b);` — a fact-only machine invoked for its ensures,
  instantiated at the call's operands into the judge's hypotheses,
  erased at codegen; instantiation is machine application, not search).
  Citation soundness = the CALL-GRAPH discipline already landed (a
  lemma citing a lemma is a machine calling a machine; mutual cycles
  refuse as unmeasured call cycles). The rungs this opens, in order:
  (1) STATEMENT-CALL CITATION INTO THE STRUCTURAL JUDGE — LANDED
  2026-07-12: both citation spellings intake (the bare statement call
  `lemma(b);` and the let-bound `let fact = lemma(b);`, which is also
  what the trailing-return auto-hoist lowers the bare form into — NOTE
  their argument spans live in DIFFERENT arenas, statement table vs
  expression table); the callee's ensures conjuncts instantiate at the
  call's argument terms via substitute_term (with `result` mapped to
  the application at those operands) and feed the judge exactly like
  requires hypotheses; the sole-arm/case-arm recognizers step over
  citation statements (with the empty-arms guard — a citations-only
  body must NOT return Some([]) or every fact judges vacuously
  Proven); v1 boundary: citing a REQUIRES-bearing lemma errors loudly
  (site discharge is its own rung). Landed with it, the DECISION-12
  AMENDMENT (owner, 2026-07-12, chat): uniform compilation — the
  diagnostics system grew a WARNING severity (was error-only;
  warning-only batches print to stderr and pass — report integration
  is a recorded follow-up), `_ = pure();` demoted from error to
  warning, suppressed when the callee OR the enclosing machine is a
  proof machine, and bare `lemma(b);` exempted from the
  discarded-result error for proof-machine callees (a citation has no
  runtime result; the exemption keys on the callee's computed
  classification, never on site context). Canary rework:
  fail/calls/pure_discard_dead_code -> pass/calls/
  pure_discard_warns_compile. NOTE for the runtime-membrane rung: the
  exemptions key on the proof-MACHINE classification (signature
  mentions proof-only data), so ch10's integer-typed `mask_is_mod`
  shape rides only because fact-only integer lemmas are RETURN-LESS
  (no result to discard); if value-returning integer lemmas want bare
  citation, a broader "fact-only machine" marker is the recorded fix.
  Core nat.omg's add_zero_right now carries the LAW conjunct
  `(add(a, Nat::Zero)) == a` next to `result == a` (both by the same
  induction — multi-fact ensures, `;`-separated). Pinned: pass/proofs/
  proof_nat_structural_lemmas (cite_zero), fail/proofs/
  uncited_structural_fact_rejected (nothing ambient: same goal minus
  the citation fences), fail/proofs/citation_requires_bearing_rejected,
  + 5 lib tests (incl. wrong-operands non-leakage: citing at `b` says
  nothing about unrelated `c`, but DOES serve goals one compute step
  away — sound derivation). (2) PER-ARM CITATION — LANDED 2026-07-12:
  **COMMUTATIVITY IS PROVEN IN CORE.** nat.omg carries add_comm
  (`add(a,b) == add(b,a)`, induction on `a`) spelled with per-arm
  SUB-STATE proofs: each case arm transitions to its own state carrying
  exactly the citations that case needs — comm_base cites
  add_zero_right's law at `b`, comm_step cites add_succ_law AT THE CASE
  PAYLOAD (a frame only the sub-state sees) and takes the IH from its
  self-application. The three pieces as reconned: (a) descent THROUGH
  sub-state params (substate_parameter_descends in calls.rs: param
  descends iff EVERY Named transition into its state passes a
  strict-subterm Member read at that position; symbol-first matching,
  name-fallback refuses under any same-name local/assignment);
  (b) recognize_structural_case_arms follows Named arm targets into
  sub-states — sub-state params bind to the transition's argument terms
  converted under the arm environment (payload bindings become the
  fresh IH variables), sub-state citations instantiate under THAT
  environment onto StructuralCaseArm::citations, the sole Always value
  terminal is the arm's value; machine-level citation intake now reads
  the ENTRY state only (sub-state frames are per-arm; note mis-bound
  instances of requires-free ∀-lemmas were never UNSOUND — any
  instantiation of a proven lemma is true — only imprecise);
  (c) IH machinery unchanged. Pinned: cite_comm in
  pass/proofs/proof_nat_structural_lemmas (consumes core add_comm),
  fail/proofs/nat_substate_nondescending_rejected (param bound from `b`
  spins forever — rejects), + 4 lib tests (comm proves; step minus its
  citation fences — the citation is load-bearing; false comm-shaped
  claim refuses; non-descending sub-state recursion rejects).
  (3) the SHAPE-MATCH FAILURE DIAGNOSTIC — LANDED 2026-07-12: when a
  structural conjunct fences Unknown, requires-free free proof machines'
  law conjuncts (result-shaped ones excluded) first-order match against
  the failed goal (lemma params as pattern variables, either
  orientation) and the fence appends "note: `lemma` proves this shape --
  cite it: `lemma(operands);`" with the matched operands rendered.
  DIAGNOSTIC ONLY — the proving path never pattern-matches. Pinned in
  fail/proofs/uncited_structural_fact_rejected (expected.txt is the
  note itself) + the lib-test twin. (3b) LEMMA ZOO — LANDED 2026-07-12:
  core nat.omg now carries the COMMUTATIVE-SEMIRING surface minus
  distributivity, all proven: add_assoc (pure induction, no citations),
  mul_zero_right, mul_succ_right (`mul(a, Succ b) == add(a, mul(a,b))`
  — its step case is the citation CHOREOGRAPHY showcase: three reducing
  rewrites, comm/assoc/comm, hand-spelled Dafny-style), and mul_comm
  (base cites mul_zero_right; step cites mul_succ_right + IH). Pinned:
  cite_assoc/cite_mul_comm consumers in the lemmas canary + 5 lib tests
  (incl. choreography-stripped fences). Distributivity DEFERRED to
  rearrange-mode (its step case is exactly the choreography-heavy shape
  ring canonicalization dissolves). FOUND + FIXED en route (pre-existing
  backend bug, stratum-independent): a statement call whose ARGUMENT
  calls a self-recursive machine (`lemma(q, add(p, q));`) overflowed the
  compile thread — omega-state-values' simplifier expanded the recursive
  callee's model via the guarded-helper comparison, re-entered itself
  with the HelperStateStack freshly popped, and grew the argument one
  constructor-read per round. REENTRANT_SIMPLIFY_DEPTH_LIMIT (32) now
  budgets the growth edges (comparison expansion, helper-value inline,
  model build); past it the expression stays unsimplified — the
  canonicalizer's ordinary no-match behavior. Pinned
  pass/calls/statement_call_recursive_argument_compile. (4)
  rearrange-mode = ENGINE-INTERNAL ring canonicalization (L4
  sum-of-monomials generalized), not lemma rules. ⚠️ SETTLED 2026-07-18
  (owner, chat — full record in mathematical_proofs brief + ch14): a
  carrier EARNS canonicalization via EXPLICIT CONFORMANCE, never
  scope-sniffing (Lean CommRing-instance precedent; auto-enable
  REJECTED — proof behavior must not depend on imports; the one-pager
  debt is retired). Surface, all existing constructs completed: a core
  `CommutativeSemiring` trait whose op requirements are FREE-machine
  shaped (new: requirements without Self:: mirror free proof machines)
  and whose LAW requirements carry `ensures` (the layouts-settle
  conformance-theorem mechanism); lemma machines bind via the existing
  per-machine `satisfies` clause, checked proven-ensures ⊨ declared-law
  (∀-to-∀ first-order match — the N3 shape-match diagnostic promoted to
  load-bearing); clause order = signature → satisfies → terminates [by ...] →
  ensures → body; signature collisions name the requirement path
  (`satisfies CommutativeSemiring::mul as Tropical` — completes the
  named-satisfier draft's missing half); zero/one = trivial machines.
  Judge consumption: AMBIENT under the home-conformance rule, selected
  by OP SYMBOLS + operand DOMAINS (plural algebras = named satisfiers,
  same machine may fill different slots — Nat under (max,add) is the
  tropical semiring; same-ops duplicates are semantically vacuous for
  proofs; FOREIGN carriers own a `domain` on the carrier — no orphan
  surface, no newtype: encode precedent, conformances resolve through
  domains). ENGINEERING RUNGS: (1) trait + conformance checking; (2)
  the judge's rearrange mode (generalize the int canonicalizer to
  symbol-keyed ops). Acceptance = citation-free `mul_distributes`;
  regression = `mul_succ_right`'s msr_step choreography DELETES.
  RUNG A LANDED 2026-07-18: the single-requirement satisfies surface.
  Grammar: `satisfies Trait::req [as Alias]` (SatisfiesClause node
  replaces the flat identifier span; threaded syntax→resolved→typed
  TraitConformance as optional requirement/alias fields). Semantics:
  path form = single-requirement conformance for ANY machine; a FREE
  machine's bare `satisfies Trait` binds the requirement bearing its
  own name (free machines never had whole-trait candidates);
  data-attached bare stays whole-trait (unchanged). `Self` in
  requirement types BINDS the carrier on first use (synthetic
  invalid-symbol binding in the trait-bindings matcher) — the
  free-shaped requirement infers the carrier from the satisfier, data
  and primitive carriers both. The trait-side surface (free-shaped
  requirements + `ensures` LAWS) already parsed — probed + pinned.
  Canaries: pass/traits/ring_requirement_satisfies_exit (differential
  70; proof-only Peano + runtime i32 carriers) + three fail edges
  (unknown requirement / Self-bound signature mismatch / bare-satisfies
  no-name-match spells the fix).
  RUNG B LANDED 2026-07-18: the law-conformance check
  (contract_entailment::check_law_conformance, called from
  validate_machine_single_requirement). A requirement with `ensures` is
  a LAW; its satisfier must carry a machine-checked ensures matching it
  ∀-to-∀: op-slot applications rewrite to the CARRIER's bound machines
  (resolved through the carrier's own conformances; unconformed slot in
  a law = targeted error; alias-preferring slot resolution, ambiguity =
  the `as <Alias>` error), and every law param must bind a DISTINCT
  plain satisfier param (weaker instances rejected). result-mentioning
  conjuncts are functional specs, excluded both sides. CONVENTION
  PINNED: law requirements declare `-> Self` (lemma machines return the
  carrier). Canaries: pass/proofs/ring_law_conformance (op machine
  named `plus` — the rewrite is real) + three fails
  (unproven/weaker-instance/slot-unbound). Rung C (the judge) can now
  consume carrier_slot_bindings as its license + symbol table.
  RUNG C LANDED 2026-07-18 — THE ACCEPTANCE MET. The structural judge's
  REARRANGE tier (judge_equation's stuck-equation fallback): flatten
  both sides to addend MULTISETS over a licensed op; equal proves,
  unequal stays Unknown (never refutes — atoms may alias). LICENSE
  (compute_ring_licenses): a trait op slot with BOTH comm+assoc LAWS
  (detected by SHAPE over requirement names, never name-keyed), the op
  conformed, both law slots satisfied for the carrier. ⚠️ NO CIRCULAR
  LICENSING: a machine binding any comm/assoc law slot of a trait gets
  NO licenses from that trait (kills self-licensing AND multi-machine
  license cycles — the axiom base always proves ring-free).
  Core surface: core/algebra.omg = the settled CommutativeSemiring
  (zero/one/add/mul slots + 5 laws; identity laws deferred — their
  facts need the zero()-application vs Nat::Zero-constructor bridging
  rung, noted in the file). nat.omg conforms machine-by-machine (+ new
  trivial zero/one machines). ACCEPTANCE ARTIFACTS: mul_distributes in
  core nat.omg with ZERO citations; mul_succ_right's msr_step
  choreography DELETED. Load-bearing verified: pulling add_comm's
  conformance re-fences mul_distributes. Canaries:
  pass/proofs/ring_rearrange_core_nat + fails unlicensed (lemmas in
  scope, NO conformance → fence: the no-scope-sniffing pin) +
  false_shuffle (live license, differing multisets → fence).
  REMAINING (follow-up rungs, not blockers): tier-2 full polynomial
  (mul distributing through the canonical form — needs all five laws
  conformed; today mul applications are atoms, sufficient for the whole
  acceptance) -- TIER-2 FULL POLYNOMIAL LANDED 2026-07-16: a
  SemiringLicense pairs the add/mul ops when a conformed
  DISTRIBUTIVITY law connects them (shape-detected like comm/assoc;
  no-circularity extended over all FIVE law slots; the carrier match
  is STRUCTURAL type_references_match -- handle equality was the
  first-try bug); ring_rearranged_equal's polynomial tier normalizes
  both sides by distributing mul through add into sorted monomial
  multisets (64-monomial cap). Pinned: pass
  proofs/polynomial_expand_core_nat -- the binomial expansion
  (a+b)(c+d) = ac+ad+bc+bd discharges by INDUCTION with ZERO
  citations, RUNS exit 70; fail polynomial_false_expand (a wrong
  monomial multiset stands down to the fence). zero/one
  identity-law bridging LANDED 2026-07-16:
  check_law_conformance normalizes NULLARY applications of trivial
  CONSTANT machines (single state, one un-guarded transition to a
  CLOSED constructor -- the settled zero/one shape) to their
  constructor bodies on BOTH sides of the match, so
  `add(a, zero())` and the proof's `add(a, Nat::Zero)` are one term;
  CommutativeSemiring gained add_identity + mul_identity and nat.omg
  conforms them via add_zero_right/mul_one_right (run-verified: the
  misconformed add_zero_left refuses with the law rendered in its
  BRIDGED form). Int/Rat routing + the N2(d) arithmetic bridge
  (unchanged).
  INT RUNG STEP 1 LANDED 2026-07-16 -- IntPair, the difference-pair
  construction (core/int.omg): `data IntPair { neg; pos: Nat }` with
  componentwise add_int/neg_int, PROVEN add_int_comm + add_int_assoc
  (record-field congruence over two add_comm/add_assoc citations; the
  ring license also settles them citation-free) and add_int_neg
  (a + (-a) unfolds to cross_sum's zero-class shape, no citations).
  Two judge extensions made records provable: (1) plain-RECORD
  literals term as empty-case constructors in BOTH termifiers
  (congruence decomposes them; ""=="" never refutes-by-case), and
  (2) callee-body field reads off SYMBOLIC receivers (`a.neg` where
  a maps to a caller variable/opaque) term as the caller-vocabulary
  Opaque (`"a.neg"`), so citations over the same place align.
  Pinned: pass core/int_core_surface (citing consumer, RUNS exit
  70); fail proofs/record_false_comm_rejected (add_int(a,b) ==
  add_int(a,a) refuses). Probes run-verified: broken module comm /
  assoc / neg each refuse; a false INJECTED citation poisons toward
  refusal (over-refusal safe).
  BUILTIN-SHADOW FENCE LANDED with it (the rung's real finding): the
  first `data Int` attempt compiled while every reference silently
  bound to the BUILTIN `Int` (type lookup searches BuiltinType before
  Data) -- the definition was orphaned and every lemma stood down at
  the polynomial engine UNVALIDATED (false ensures certified). What
  looked like reference-lazy module validation was this shadowing.
  validate_data_field_types now refuses any data definition named
  like a builtin type (Int/UInt/Real/bool/i32/String/...); pinned
  fail data/builtin_type_name_shadow. Corpus scanned: no other
  colliding definitions.
  INT RUNG STEP 2 LANDED 2026-07-16 -- mul_int PROVEN: the
  difference-pair product ((p1p2+n1n2) - (p1n2+n1p2)) with
  mul_int_comm (four mul_comm citations rewrite the products; the
  neg field's swapped addend order settles under the add ring
  license), and mul_int_assoc + mul_int_distributes discharge
  CITATION-FREE -- each record field is a polynomial identity over
  the six Nat components and the semiring tier-2 normal form settles
  the monomial multisets (the deepest citation-free discharges yet:
  assoc is 8 monomials of degree 3 per field). Canary consumer
  int_mul_consumer cites mul_int_comm (RUNS exit 70). Negative
  probes run-verified: false comm / wrong assoc regrouping / wrong
  distributivity split each refuse.
  INT RUNG STEP 3 LANDED 2026-07-16 -- the QUOTIENT enters:
  hypothesis-aware RING EXCHANGE in ring_rearranged_equal (one
  bounded application): a requires/citation/IH equation whose sides
  flatten over the licensed op licenses swapping that SUB-MULTISET
  of the goal's addends (sound: sum(from) == sum(to) is the
  hypothesis and comm+assoc closure is what the license's
  conformance proved; whole-term matches were already rewritten
  during resolve -- this reaches what the rewriter cannot see).
  First consumer: add_int_respects_eq -- the cross-sum equivalence
  a ~ a2 (a.pos + a2.neg == a2.pos + a.neg, as requires) is
  RESPECTED by add_int, spelled in unfolded Nat component form (the
  fact grammar has no member-of-call). Probes run-verified: wrong
  goal refuses, requires REMOVED refuses (the hypothesis is doing
  the work); pinned fail proofs/ring_exchange_unhypothesized_rejected.
  SITE DISCHARGE LANDED 2026-07-16 (the N3 rung the old refusal
  named) + two LATENT-HOLE closes found auditing for it: (A)
  citations of REQUIRES-bearing lemmas now instantiate the callee's
  requires at the call's argument terms and judge them against the
  citing machine's hypothesis base -- Proven injects the ensures,
  anything else refuses NAMING the undischarged fact (v1: a
  citation cannot lean on another citation's fact; per-arm sites
  discharge against the same machine-wide base). int_eq_symmetric /
  add_int_respects_eq are now CITABLE. Pinned: pass
  proofs/citation_requires_discharged (citation LOAD-BEARING --
  probed: removing it fences; opaque `minus` links only through the
  injected ensures), fail citation_requires_bearing_rejected
  (re-pinned to the new naming message) + unit-test pair. (B)
  unfold_application's functional-ensures shortcut EXCLUDED
  requires-bearing callees -- it injected the conditional result
  term with no site to discharge the condition (latent, no exploit
  in-tree; bodies still unfold). (C) the INDUCTIVE HYPOTHESIS is
  now DENIED to requires-bearing machines -- the IH is conditional
  on the requires at the self-call's operands and injecting it
  unconditioned was a latent unsoundness; requires-bearing
  INDUCTION (the cancellation lemma add(c,a)==add(c,b) => a==b,
  which needs per-arm requires re-intake + IH premise discharge) is
  the recorded follow-up rung.
  REQUIRES-BEARING INDUCTION + CANCELLATION + TRANSITIVITY LANDED
  2026-07-16 (the whole chain in one rung): (1) per-arm requires
  RE-INTAKE -- the machine's requires re-intake under each arm's
  case hypothesis, so `add(c,a) == add(c,b)` under c := Succ(prev)
  unfolds and INJECTIVITY-decomposes to the prev-level equation;
  (2) IH PREMISE DISCHARGE -- a requires-bearing machine's IH
  intakes only after its requires, instantiated at the self-call's
  operands, judge Proven against the arm's hypotheses (the guard
  from the site-discharge rung upgraded from deny to discharge);
  (3) the checked-trees CALL-REQUIRES PROVER now EXEMPTS proof->
  proof calls -- a proof machine emits no runtime code, a call
  between them denotes mathematical application whose VALUE does
  not depend on the callee's requires, and every ensures-
  consumption face gates in the structural layer (kept for any
  call touching runtime machines; call targets resolve via
  entry-STATE symbols, the machine-symbol lookup alone misses).
  nat.omg gains add_cancel (left cancellation, induction on the
  pad; sub-state arm shape -- inline call-valued arms break the
  recognizer via terminal auto-hoist). int.omg gains
  int_eq_transitive: pad both goal sides with b.neg + b.pos, TWO
  hypothesis exchanges (the tier is now a depth-2 frontier-capped
  BFS) equalize the padded sums inside the CITED add_cancel's
  requires, site discharge strips the pad. The cross-sum quotient
  is now a PROVEN equivalence (symmetric + transitive; refl is
  ring-trivial) WITH add-congruence. Probes run-verified: false
  cancel ensures refuses, right-form requires (premise
  underivable) refuses, transitivity with a hypothesis DROPPED
  refuses naming the undischarged cancellation premise, corrupted
  goal refuses.
  MUL-CONGRUENCE LANDED 2026-07-16 -- the QUOTIENT IS COMPLETE:
  tier-2 gains the SCALED-HYPOTHESIS EXCHANGE (a hypothesis
  equation polynomial-normalizes to a monomial-multiset pair, and
  multiplying both sides by a monomial factor m -- drawn from the
  goal's own atoms, plus unscaled -- keeps it an equation under the
  conformed distributivity, so hl*m exchanges for hr*m; depth-2
  frontier-capped BFS, twin of the tier-1 addend exchange).
  int.omg gains mul_int_respects_eq: the cross-sum hypothesis
  scaled by b.pos and by b.neg equalizes the product components in
  two exchanges. The difference-pair construction now carries a
  PROVEN equivalence relation (symmetric, transitive; refl ring-
  trivial) with BOTH congruences (add + mul) -- the ZZ quotient
  story, machine-checked end to end. Probes run-verified: requires
  dropped refuses, corrupted goal refuses.
  INTPAIR CONFORMS CommutativeSemiring LANDED 2026-07-16 -- all 11
  slots (zero_int/one_int/add_int/mul_int + the 7 law lemmas bind
  their slots), the SECOND carrier to earn the licenses. Two
  enablers: (1) RECORD ETA in judge_equation -- a record literal
  rebuilding EVERY declared field of a variable from that same
  variable's field reads IS the variable (product extensionality;
  fields matched BY NAME, a permuted rebuild refuses) -- which is
  what the identity laws reduce to after add_zero_right /
  mul_one_right / mul_zero_right / add_zero_left citations fold the
  components; (2) the NO-CIRCULAR-LICENSING rule refined from
  trait-wide to PER-LICENSE (excluded only when the judged machine
  binds the license's own comm/assoc law slots FOR THE SAME
  CARRIER, by type_references_match) -- the trait-wide skip had
  wrongly stripped IntPair's mul lemmas of NAT's earned licenses
  the moment they bound their own slots; a law lemma's goal is the
  law shape over its own op, so per-carrier exclusion still breaks
  every cycle (the 5 ring_law_* fail canaries all still pin).
  Probes run-verified: eta PERMUTATION (neg_int(a) == a) refuses,
  false identity (one_int for zero_int) refuses. ZOO GROWTH same
  day: neg_int_involution (two swaps rebuild `a` -- pure unfold
  closed by record eta, ZERO citations) and neg_int_respects_eq
  (the third congruence: ~ is respected by add, mul, AND neg; one
  addend exchange). Negative twins probe-verified.
  N2(d) BRIDGE DEFERRED 2026-07-16 (nonblocking research item):
  the Nat<->integer surface is unruled -- extraction (Nat ensures ->
  polynomial facts) vs reflection (integer range facts -> constructor
  readings), which types participate, and where the bridge fact
  lives. Nothing in the queue is blocked on it. Int introduction
  rule stands: order has no floor, measures stay Nat-valued or
  range-floored.
- **N4 — roster library (Nat ops + Seq LANDED 2026-07-11):** core
  nat.omg carries add/mul as proof machines (mul composes add by an
  ordinary cross-machine call; only self-calls need the measure), and
  seq.omg carries generic recursive `Seq<T>` plus a
  structurally-recursive `length` proof machine + PROVEN lemmas
  length_empty and length_cons (the roster's first proven lemmas over a
  GENERIC type; the structural judge is parametric over constructor names) — pinned
  pass/proofs/runtime_core_roster_ops_exit (a two-element length lemma
  unfolding the core `length`). First PROVEN
  library lemmas LANDED 2026-07-11: core nat.omg carries add_zero_left
  (compute-mode) and add_zero_right (structural induction), each ensures
  machine-checked at compile time for every importer. SEQ ZOO GROWTH
  2026-07-12: seq.omg adds `append` + the COMPOSED law length_append
  (`length(append(s,t)) == add(length(s), length(t))`, induction on s —
  cross-machine unfolding through append/length/add plus the IH, no
  citations; Lean analog List.length_append), consumed by
  cite_length_append in the roster canary + a lib test — plus
  append_empty_right and append_assoc (same-day; pure inductions, Lean
  analogs List.append_nil / List.append_assoc). NAT SEMIRING COMPLETE
  2026-07-16: mul_distributes_right (citation-free — the rearrange
  tier flattens the step case's three-addend shuffle) + mul_assoc
  (satisfies CommutativeSemiring::mul_assoc — the LAST unbound law
  slot; step cites right-distributivity at (b, mul(prev,b), c) in a
  sub-state + the IH), both machine-checked on every import (the
  roster canary compiles them; judge discharged first try). All five
  CommutativeSemiring laws now bound for Nat. SEQ ZOO 2026-07-16:
  `reverse` + `length_reverse` BOTH LANDED (the latter's step cites
  length_append + add_succ_law + add_zero_right; Lean
  List.length_reverse). Landing it took THREE simplifier fixes found
  through its crash: (1) folding.rs's boolean family depth-budgeted
  (BOOLEAN_SIMPLIFY_DEPTH_BUDGET=256, raw-node fallback) closing the
  DNF-explosion CRASH; (2) distribute-over-Or size-gated
  (BOOLEAN_DISTRIBUTION_NODE_BUDGET=96, ITERATIVE counter); (3) the
  HELPER-EXPANSION FUEL BUDGET (HELPER_EXPANSION_FUEL_BUDGET=20_000,
  thread_local refilled per simplify entry, spent per
  helper_state_model build) closing the exponential-TIME hang — the
  depth cap bounds each path, fuel bounds the branching; exhaustion
  declines the fold (never unsound, merely unfolded). The pending
  overflow repro RETIRED (the landed lemma is the living regression
  witness — every seq.omg import re-verifies it). reverse_append
  LANDED 2026-07-16 (Lean List.reverse_append) via the recognizer's
  LET-BINDING RUNG: a sub-proof `let` — spelled (`let ih =
  reverse_append(tail, t);`) or the LOWERING'S OWN __hoist_N of a
  call-valued terminal — termifies its initializer under the arm
  environment and JOINS it, so call-wrapping-self-application
  terminals resolve. The root cause (found by instrumenting the
  arm-terminal conversion, trace line kept behind
  OMEGA_STRUCT_TRACE): the compiler hoists a call terminal into
  `__hoist_0`, and the recognizer saw an unresolvable Name — the IH
  machinery itself was ALWAYS shape-agnostic
  (StructuralJudge::self_applications walks the whole value term).
  The nested value-call fence is met by the let-bound spelling, per
  its own hint. reverse_reverse LANDED same day
  (first try — the involution, step citing reverse_append at the
  reversed tail + singleton, constructor-spine IH; Lean
  List.reverse_reverse). snoc + reverse_snoc LANDED same day: the
  refusal was the SAME hoist blindness in unfold_application (snoc's
  definitional wrapper body IS the lowering's `let __hoist =
  append(..)` shape) — the fact normalizer now binds LocalData
  statements exactly like the two recognizers (three sites total ride
  the let-binding rung). Lean List.reverse_concat. add_one_right +
  length_snoc + snoc_append LANDED 2026-07-16 (zoo at 13 Seq + 1 new
  Nat lemma): add_one_right is the add_zero_right induction shifted by
  one (Lean Nat.add_one); length_snoc chains TWO citations
  (length_append at (s,[x]) + add_one_right at length(s)) -- the
  first two-citation proof in the zoo; snoc_append is one
  append_assoc citation at (s,t,[x]) (Lean concat_append mirrored).
  All three discharged FIRST TRY; negative probe run-verified (a
  broken length_snoc claim rejects). mul_one_left + mul_one_right
  LANDED same day (both first try): the semiring's multiplicative
  IDENTITIES (Lean one_mul/mul_one) -- left is compute + one
  add_zero_right citation, right is structural induction with the
  head add computing through. snoc_reverse LANDED same day (first
  try, pure compute-mode: both sides normalize to the same append --
  Lean List.reverse_cons); zoo at 14 Seq + 8 Nat-lemma surface. Next
  zoo: map/filter once generics-over-machines land. REMAINING:
  more of the lemma zoo as the judge widens (commutativity needs
  double induction / rearrange-mode), extraction INTO consumer proofs
  (a caller citing a lemma's ensures — the fact-consumption face), the
  proof views (Seq/Bag/Range) dissolving
  from parser-known atoms into these types (the L6 bag_view rung folds
  in). Rat/Bag ship via CANONICAL-REPRESENTATIVE domains (reduced
  fractions `where gcd==1`; sorted sequences) — plain `==`, no quotient
  dependency: N4 is decoupled from N6, but those two ride R2's
  where-clauses. The `%` former is reserved for carriers with no
  computable canonical form (Real: stream equality undecidable).
  RAT-LADDER RUNG 1 LANDED 2026-07-20 (task #134): the MONUS
  development in core nat.omg -- pred + sub (Lean's Nat.sub spelling:
  recursion on b composing pred cross-machine; every machine
  single-transition so facts unfold) + FOUR proven lemmas:
  sub_zero_right (compute), succ_sub_succ (induction on b; Lean
  Nat.succ_sub_succ), sub_self (cites succ_sub_succ at (prev,prev)),
  add_sub_cancel (cites succ_sub_succ at (add(prev,a),prev) + IH; the
  Rat normalization workhorse, Lean Nat.add_sub_cancel_left). Zoo at
  12 Nat lemmas. Negative probe verified (broken add_sub_cancel
  claim refuses). TWO RECOGNIZER SHAPE RULES learned (recorded for
  the next zoo growth): (1) lemma facts only unfold SINGLE-TRANSITION
  machines -- a two-subject match spelled with a sub-state refuses
  (respell compositionally, the pred/sub route); (2) a recursive
  lemma's self-call arm must ride a SUB-STATE (the arm-terminal
  hoist shape sub_self/add_cancel use) -- a bare direct-value
  self-call arm fails recognize_structural_case_arms. NEXT Rat-ladder
  prerequisites: lt/le spelling (sub(a,b) == Zero), then mod + gcd.
  DESIGN-RULED / ENGINEERING-READY 2026-07-17: proof machines use the
  ordinary measured-termination family. A computed recursive subject such as
  `sub(a,b)` emits the selected ranking view's strict-edge obligation; ordinary
  entailment, contracts, or an explicit cited lemma such as `sub_lt` discharges
  it. No N2(d) integer bridge and no ranking-specific citation syntax are
  required. Next rung: add the Nat strict-ranking proposition/lemma and edge
  entailment, then implement mod + gcd. SLICE A1 LANDED 2026-07-20 --
  THE ORDER-AS-MONUS SURFACE + STRICT-RANKING EVIDENCE, machine-
  checked in core nat.omg: le_of_succ_le (case on the right arg; the
  Zero arm is VACUOUS by constructor clash), pred_le_left (Succ arm
  cites le_of_succ_le under the arm-refined premise), sub_le
  (induction on b following sub's own recursion; the Succ arm's
  pred_le_left citation discharges against the INDUCTIVE HYPOTHESIS),
  and sub_lt_succ -- the premise-free strict fact sub(Succ a, Succ b)
  < Succ a in the monus spelling (positivity by constructor shape),
  the exact lemma mod/gcd's computed-subject edges will cite. Zoo at
  17 Nat lemmas. THREE JUDGE EXTENSIONS carried it (all conservative):
  (1) CONSTRUCTOR-CLASH VACUITY -- an arm whose requires premise
  judges REFUTED under the bare case hypothesis closes vacuously
  (judged BEFORE intake; intaking first installs the premise's own
  rewrite and masks the clash); (2) ARM-REFINED CITATION DISCHARGE --
  sub-state citations discharge their requires under the arm's case
  hypothesis + re-intaken machine requires (the machine-wide base saw
  the unrefined spelling); vacuous arms skip instantiation; (3)
  IH-ENRICHED TWO-PASS CITATIONS -- citations collect in pass one and
  instantiate after the terminal is known, against a base enriched
  with the machine's own ensures at the terminal's self-applications
  (requires-free machines only; the well-foundedness stays the
  termination checker's separate job). FALSE TWINS pinned:
  vacuity_satisfiable_premise (satisfiable premise must NOT close
  vacuously) + ih_citation (sub_le's exact shape with a false claim
  refuses on both faces). REMAINING -- SLICE A2 DESIGN
  (banked 2026-07-20): prove the SYMBOLIC strict lemma first --
  sub_lt_of_le(pb, a) [requires Succ pb <= a; Zero arm vacuous by
  clash, Succ arm = sub_lt_succ's citation body, no induction] then
  sub_lt(a, b) [requires 1 <= b, b <= a; Zero arm vacuous, Succ arm
  cites sub_lt_of_le under the arm-refined premise] -- its ensures
  sub(Succ(sub(a, b)), a) == Zero is SYMBOLIC in (a, b), so the
  checker's obligation matches the instantiated ensures LITERALLY
  (no destructure equations needed). MOD SHAPE: terminates by a;
  requires 1 <= b; root transition cases the COMPUTED subject
  sub(b, a) -- the Zero arm (b <= a) hands to mod_rec which cites
  sub_lt(a, b) and recurses mod(sub(a, b), b); the Succ arm returns
  a. CHECKER RULE (nat.rs computed-subject arm): when the measure
  argument is an application, look for a source-state citation whose
  callee's instantiated ensures syntactically equals
  sub(Succ(ARG), MEASURE) == Zero; DISCHARGE the callee's requires
  at the site against (i) the citing machine's own requires +
  (ii) incoming-arm case equations (each transition targeting the
  source state whose guard cases subject S into constructor C
  contributes S == C -- mod_rec's incoming Zero arm contributes
  sub(b, a) == Zero, exactly sub_lt's premise). All syntactic,
  conservative. SLICE A2 LANDED 2026-07-20: the SYMBOLIC
  lemmas sub_lt_of_le + sub_lt prove first try on the a1 judge
  extensions (Zero arms vacuous by clash; sub_lt's ensures is
  symbolic in (a, b) so instantiation matches obligations literally).
  THE CHECKER RULE landed in calls.rs validate_proof_machine_recursion
  as cited_strict_decrease: a computed measure argument proves the
  strict edge when a SAME-STATE citation's instantiated ensures equals
  sub(Succ(ARG), MEASURE) == Zero, with the callee's requires
  discharged syntactically against machine requires + incoming-arm
  case equations; let-bound edge arguments resolve through their
  same-state initializer (the value-call face forces the hoist). MOD
  LANDED in core nat.omg (mod(a, b) for b >= 1; root cases the
  computed subject sub(b, a); zoo at 20 Nat machines). FALSE TWIN:
  computed_subject_requires_undischarged (the mod shape without
  1 <= b refuses -- with b == Zero the recursion truly diverges).
  REMAINING slice a3 -- GCD, analysis sharpened 2026-07-20: gcd
  needs mod to CARRY `ensures result < b`. KEY SIMPLIFICATION: flip
  mod's dispatch to case sub(Succ a, b) instead of sub(b, a) -- then
  the a<b arm's case hypothesis IS the ensures goal verbatim (free),
  and only the recursive arm needs a conversion (from
  sub(Succ a, b) == Succ w derive sub(b, a) == Zero -- order
  TOTALITY/dichotomy). BOTTLENECK IDENTIFIED: every remaining
  conversion lemma (dichotomy, lt_of_sub_pos, pred_eq_succ chains)
  needs DOUBLE DESTRUCTURE -- casing two subjects -- and
  recognize_structural_case_arms only walks ONE hop (root arms ->
  one flat sub-state ending in a Value terminal). THE NEXT JUDGE
  EXTENSION is nested-arm recognition: allow a sub-state to hold its
  own case dispatch (arms become a tree; each leaf carries the
  CONJUNCTION of case hypotheses + per-level IH environments). Also
  needed for mod's ensures at all: the recognizer requires a
  VARIABLE subject -- a computed subject (sub(Succ a, b)) must
  contribute its arm equation as an intaken HYPOTHESIS instead of a
  substitution (second recognizer extension, small). PROGRESS 2026-07-20 (nested arms
  PROVED UNNECESSARY): the conversion chain landed FLAT --
  pred_eq_succ (Zero arm vacuous), sub_eq_add (induction on a;
  pred_eq_succ + add_succ_law citations + conditional IH at
  (pa, b, Succ w)), le_add (flat strips), lt_of_sub_pos (cites
  sub_eq_add + le_add; the WITNESS spells as pred(sub(b, a)) so no
  payload binding is ever needed). RECOGNIZER EXTENSION (1) LANDED:
  computed transition subjects (`transition (sub(b, a)) { .. }`)
  carry their arm hypothesis as an intaken EQUATION
  (StructuralCaseArm.case_equation) instead of a substitution, in
  both the per-arm proving loop (requires vacuity/intake applies the
  same) and the arm-refined citation judge. MOD NOW CARRIES
  `ensures result < b` (sub(Succ result, b) == Zero): the a<b arm
  cites lt_of_sub_pos(a, b, pred(sub(b, a))); the recursive arm is
  the conditional IH. Zoo at 24 Nat machines; suite 866 green.
  REMAINING -- GCD (analysis 2026-07-20): the edge obligation
  matches mod's instantiated ensures via a LET-CITATION
  (`let next = mod(a, Succ pb);` -- extend cited_strict_decrease to
  treat call-initializers as citations with `result` mapped to the
  initializer expression). TWO GAPS: (i) mod's requires (1 <= b) at
  the site needs a premise-free helper (le_one_succ(pb), trivial) +
  site facts extended with premise-free citations' instantiated
  ensures; (ii) the MEASURE-SIDE mismatch -- the instantiated
  ensures speaks Succ{prev: pb} while the obligation speaks the
  measure name b, and the incoming payload-arm guard is TAG-ONLY
  (b == Nat::Succ, no fields), so the alias b == Succ{prev: pb}
  must be MATERIALIZED from guard tag + data declaration + the
  incoming transition's target-arg payload reads (pb = b.prev), or
  the whole discharge routed through the structural judge instead
  of syntactic matching (the judge already unfolds/rewrites; it
  lives in the same crate). Prefer the judge route -- the syntactic
  engine should not grow into a parallel judge. GCD LANDED 2026-07-20
  (the judge route, part 2): proof_edge_strict_decrease_judged in
  contract_entailment (pub(crate), called from calls.rs as the
  fallback after the syntactic match) builds a StructuralJudge from
  the machine requires, intakes the source state's INCOMING-ARM
  hypotheses -- guarded by EXACTLY-ONE-INCOMING-EDGE (a second path
  could reach the state without the arm's case; conservative skip)
  -- with the MATERIALIZED payload alias (subject == Case { field:
  param } recovered from the tag-only guard + data declaration +
  payload-read target args; intaken BEFORE any raw guard so the
  fieldless substitution cannot win first and mask the payload),
  then intakes citations in statement order (statement calls AND
  let-bound call initializers; requires judged Proven first, ensures
  instantiated with result -> the call term; the LET BINDER inserts
  a SUBSTITUTION, not an intake -- intake_equation orients
  application sides reducing, backwards for a binder), and judges
  the obligation. le_one_succ + gcd (Euclid) landed in core nat.omg
  -- zoo at 26 Nat machines. FALSE TWIN:
  computed_edge_positivity_missing (gcd without le_one_succ
  refuses). Traces behind OMEGA_EDGE_TRACE. RAT CARRIER LANDED
  2026-07-20 -- THE LADDER RUNG IS COMPLETE: div (mod's twin, same
  computed-subject strict edge) + omega/language/core/rat.omg -- the
  canonical-representative Rat (num/den Nats; mk_rat reduces by the
  cited gcd with the positivity premise; rat_eq is
  cross-multiplication, no division and no quotient former).
  Reducedness-by-type waits on the N6 quotient former; the
  DIVISIBILITY THEORY (gcd_pos, gcd_dvd_left/right, div_mul_cancel
  -- what would discharge mk_rat's gcd-positivity internally and
  prove mk_rat idempotent) is the next N4 extension above the
  carrier, demand-gated. Pinned:
  pass/proofs/runtime_core_rat_declared_exit (dual-engine). Zoo: 27
  Nat machines + the Rat carrier. TASK COMPLETE -- the full arc:
  monus development -> order-as-monus + strict evidence (3 judge
  extensions) -> computed-subject termination (syntactic + judged
  routes) -> mod with result < divisor -> Euclid's gcd -> div ->
  the Rat carrier. `sub_zero_left` landed as the monus
  surface's completion (5 lemmas, zoo at 13 Nat). Task #134 rung 1 COMPLETE;
  the rat.omg carrier waits only on this engineering rung.
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
- **F2 — exact literal/const pipeline (N2 bignum is landed):** float
  literals parse to exact Rat; compile-time float arithmetic = exact Rat
  + round once per op at operand width; specials per format (compile ==
  runtime bit-for-bit, NaN production included). RETIRES three pinned
  residues: FloatLiteral-as-f64-bits (f32 literal double-rounding), the
  guard folder's f64-window float folds, interp per-op f32 rounding.
  Differential canaries incl. the 2^24 plateau legs.
  F2a LANDED 2026-07-18: FloatLiteral = ONE shared omega-core TEXT
  carrier (spelling + Option<FloatFormat> landing; the bits-based
  resolved/typed twins retired into re-exports; text-only Eq). Width
  suffixes land the format at parse (CR4a's float twin) + the
  suffix-vs-destination float check. Per-format reads correctly
  rounded from the spelling (Rust std parses): an F32-landed literal
  NEVER routes through f64 — pinned by the double-rounding witness
  8388609.499999999999999f32 (pass/float/
  suffix_f32_single_rounding_exit, differential 77). ALL literal reads
  key the landing identically (5 selection/state-guard sites +
  interp landed_f64 — engines bit-for-bit). ⚠️ Two strip points fixed
  en route: the resolved→typed lowerer REBUILT from f64 (now clones —
  the carrier discipline), and the unconditional f64 value() reads.
  F2b LANDED 2026-07-18 (destination stamping): an UNSUFFIXED float
  literal at a declared f32/f64 destination lands its format on the
  text carrier via `land_float_literal_destinations`
  (omega-validation/literals.rs, called from the typed->checked
  lowerer on the STILL-MUTABLE tree pre-fork -- one stamp point, both
  engines bit-for-bit; NOT the state-values folder, which is
  backend-only and would have needed an interp twin). The walk
  enumerates exactly validate_suffix_landings' destinations (struct
  fields, assignments, lets) -- keep the two in LOCKSTEP; suffixed
  literals untouched (their landing is the spelling's; the CR4a check
  owns disagreement). Witness: pass/float/
  unsuffixed_f32_destination_single_rounding_exit (77; all THREE faces
  pinned, per-face failure exits 78/79/80). A second f64->f32 store
  rounding after the stamp is IDEMPOTENT (the landed value is exactly
  f32-representable), so interp store coercion stays untouched.
  F2c GUARD FACE LANDED (aarch64 lane): the comparison ADOPTS the place
  side's format (operand-derived landing, float flavor) — validation's
  land_float_literal_destinations grew a recursive guard walk
  (collect_guard_float_comparison_pairs; recurses the multi-arm
  `(subject) == true` desugar + And/Or legs) and STAMPS the literal
  side's whole tree (stamp_float_tree, Binary/Unary/Mutable deep;
  stamp-if-none, suffixed literals keep their landing); the interp's
  eval_float_binary now rounds PER-OP at the stamped width (Float
  literals witness their landing in expression_scalar_type); the native
  guard folder's const_fold_float folds per-op at the tree's landing
  (first-landed-literal witness). Pinned:
  pass/float/f32_guard_const_arith_landed_exit (70 both engines,
  differential; the 2^24 + 1.0 precision-cliff shape — interp was 71 /
  native was only coincidentally 70 via compare-time narrowing before).
  Suite 854/854, differential 14/14 after the stamps.
  F2c ARG FACE LANDED (aarch64 lane, 2026-07-16): transition-argument
  float trees adopt the TARGET STATE PARAMETER's declared type —
  land_float_literal_destinations walks [transition.target,
  transition.continuation], resolves Named targets to the same-machine
  state (path members live in the STATEMENT table's identifier arena,
  not the expression table's), zips spelled args against non-self
  params (`is_self` filtered; the receiver never pairs), and
  stamp_float_tree's the argument at the param's format. NO backend
  change needed: the stamped literal flows through the existing
  narrow_f32_literal_operands -> f32_bits() landed branch (text-parse
  at f32) and the runtime does per-op f32 (fadd s-regs). Pinned:
  pass/float/f32_arg_const_arith_landed_exit (70 both engines,
  differential; the 2^-24 + 2^-48 double-rounding tie — per-op f32
  ties DOWN to 1.0, the f64 window rounds UP to 1+2^-23). Debug
  lesson: an early "native 71" was a STALE CLI BINARY (validation
  crate rebuilt for the test harness but not omega-run); rebuild the
  CLI before diagnosing backend gaps.
  F2c NESTED-OPERAND WIDTH FACE LANDED (x86 lane, 2026-07-16): the
  scalar classifier (instruction selection storage_places.rs,
  classify_scalar_value_type_in_table) typed EVERY float literal F64,
  so an f32 chain's NESTED binary operand planned the 8-byte op
  (binary_value_operand_byte_width) — `addsd` over f32 bit patterns —
  while the outer write took width 4 from the target place: native
  diverged from the interp's per-op f32. The classifier now reads the
  literal's LANDING (the F2b/CR3 stamps), F32-landed classifies F32.
  Pinned: pass/float/f32_chain_per_op_rounding_exit (70 both engines,
  differential; the 2^24 precision cliff — per-op f32 collapses both
  +1.0s back to 16777216.0, a double-width intermediate keeps
  16777218.0, representable in f32, and exits 71).
  REMAINING (F2c): value-machine CALL-statement args (cross-machine
  param resolution, deferred until a shape demands it);
  exact-Rat multi-op const chains only if a shape demands more than
  per-op IEEE (per-op rounding at width == the exact-Rat spec for
  homogeneous ops).
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
- **F4 — float→int proof-or-policy cast:** build it; retire the drift-ledger
  entry; NaN differential legs
  become pinnable (runtime 0.0/0.0 constructs NaN portably).
  F4a LANDED (aarch64 lane, 2026-07-16): (1) `in Wrapping` float→int
  casts REJECTED at validation (no modular reading of a float, ch5;
  fail/arithmetic/float_cast_wrapping_rejected; corpus migrated — 4
  canaries + 2 samples respelled `in Saturating`, cast_operand takes
  the decision-17 chain-cast `(f as i32 in Saturating) as i32 in
  Wrapping` to meet its Wrapping operand); (2) interp eval_cast is
  DOMAIN-AWARE for float sources — Saturating = NaN→0 (cast-specific
  per the brief) + trunc + clamp to the target range, Trapping = trap
  on NaN/OOR (float_fits_integer with the exact power-of-two bound
  compares; i64's lower bound inclusive since MIN-1 isn't
  representable); (3) the Saturating pin
  pass/arithmetic/float_to_int_saturating_exit (1e20→MAX, -1e20→MIN,
  runtime 0.0/0.0 NaN→0, -3.7→-3) — ARCH-GATED (suite test +
  differential row cfg aarch64): aarch64 FCVTZS natively IS the
  Saturating semantics, x86's cvttsd2si integer-indefinite fixup is
  the F4 remainder for the x86 host's oracle.
  F4-TRAPPING-NATIVE LANDED (aarch64 lane, 2026-07-16): the cast
  carries a `trapping` flag end to end — WriteRuntimeStorageConvert +
  ValueOperand::Convert grew the field (the tuple accessor stays
  6-wide; convert_trapping() is the separate probe, the
  binary_is_float precedent), the four selection constructor sites
  derive it from cast.domain == Trapping && float source && integer
  target, and every dispatcher/width threads it. aarch64 emits
  append_float_to_int_trap_guard before FCVTZS: fcmp v0,v0 + b.vc +
  brk (NaN) then two padded-immediate float bound checks
  (FLOAT_TO_INT_TRAP_GUARD_WIDTH = 76; shapes f64→i32/i64 + f32→i32/
  i64 with exact power-of-two bounds — i64/f32 lower bounds INCLUSIVE
  since the -1 neighbours aren't representable; other shapes refuse
  loudly). x86 pass-through (`let _ = trapping` + comment): the
  cvttsd2si integer-indefinite fixup is its host session's rung — the
  cast lowers as today there (status-quo divergence, documented in
  the pending header). Pinned:
  pass/arithmetic/trapping_float_to_int_cast_traps (arch-gated
  abort-style suite test, both engine legs; in-range 7.9→7 first,
  then 1e20 traps; NaN probe-verified).
  F4-EXACT LANDED (2026-07-16) — F4 COMPLETE ON THIS HOST: a BARE
  float→int cast requires the value provably in the target's range;
  a float-LITERAL source (through Mutable, read at its landed format)
  proves via truncation-fits, everything else takes the policy error
  (the F8a shape: proof where visible, policy otherwise). Corpus
  migrated: 8 canaries + 3 samples respelled `in Saturating`
  (in-range values — identical runtime results on both hosts, x86's
  pass-through included). The pending
  float_to_int_overflow_divergence repro RETIRED EVERYWHERE (dir +
  both cfg-arch'd pending-gate rows + the PendingCanary entry): the
  bare out-of-range cast no longer compiles, so the pinned THREE-WAY
  native divergence is unreachable. Pinned: fail/arithmetic/
  float_cast_unproven_rejected + pass/arithmetic/
  float_literal_cast_proves_exit (differential 70). REMAINING (x86
  host only): the Saturating/Trapping cvttsd2si fixup sequences.
- **F5 — policy lowering: LANDED 2026-07-16 (aarch64 lane).** Float
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
  fail canary retired with it); x86_64 passes through until its host
  session (documented status-quo divergence, the F4-cast precedent).
  Pinned (arch-gated + differential):
  pass/arithmetic/float_saturating_overflow_exit (1e160² clamps,
  re-clamp idempotent, 5/0 keeps +Inf) +
  float_trapping_overflow_traps (abort-style, both engine legs).
  Remaining: the x86 guard sequences on its host.
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

- **F8 — shift-count ruling (integer):** Exact obligation (count < width;
  literal OOR = compile error), Wrapping = masked count, Trapping = trap;
  retire the shift divergence from the pending family; differential
  canaries per domain.
  F8a LANDED (aarch64 lane, 2026-07-16) — THE VALIDATION OBLIGATION:
  arithmetic_domains.rs's shift arm now requires the count PROVABLY in
  [0, width) when the lhs-governing domain is Exact OR Saturating
  (Saturating governs value overflow, not count validity — its count
  obligation is Exact's, per ch5). Width = the shifted operand's
  primitive (anonymous lhs falls back to the destination primitive;
  the lhs DOMAIN stays operand-driven per decision 17 — a target
  `in Wrapping` never re-domains the count). Two diagnostic verdicts:
  "provably out of range and can never execute" (literal OOR) vs "not
  provably below the operand width" (unproven). Corpus blast radius
  was exactly 3: shift_amount_over_width_compiles respelled
  (`(1 as i32 in Wrapping) << 100` — the folder no-crash pin
  survives), rule90's rule shift rides window's Trapping domain
  (`(90 as i32 in Trapping) >> window`), and
  runtime_shl_saturating_atwidth_exit RETIRED → split into
  fail/arithmetic/shift_count_saturating_oor_rejected (the count
  face) + pass/arithmetic/runtime_shl_saturating_value_overflow_exit
  (proven count 3<<31, the clamp machinery keeps 32-bit coverage;
  differential). New pins: fail shift_count_literal_oor_rejected +
  shift_count_unproven_rejected (branch-dependent count); pass
  runtime_shift_count_proven_range_exit (u32 [0..=7] count,
  differential 70). Pending-family note updated (the shift half of
  the unified design question is retired; float→int = F4 remains).
  PROOF-SURFACE GAP noted: a domain-cast LOCAL initializer
  (`let c: u32 in Saturating = (31 as ...)`) does not feed the count
  interval (fields fold via the tracker; locals with cast
  initializers do not) — a CR3-family refinement when a shape
  demands it.
  F8b LANDED (aarch64 lane, 2026-07-16) — WRAPPING MASKED COUNT on all
  three engines: interp Wrapping shifts mask `(r as u64) & (width-1)`
  (supersedes the 2026-07-13 modular-VALUE semantics); aarch64's
  with_domain drops the Wrapping zero-clamp/count-saturate and rides
  the register forms' native masking (NEW encode_lslv_w_register — the
  X form's mod-64 would recreate modular-value at u32 — plus
  encode_and_w_low_ones bitmask-immediate for the sub-word `& 7/15`);
  x86_64 mirrors structurally (the hardware shl/sar/shr masks mod
  32/64 = the ruling at widths 4/8; append_wrapping_shift_count_mask
  adds the sub-word `and r11d, 7/15`; the WrappingShift operand op is
  now DomainShift carrying the domain so Sat/Trap `>>` keep the floor
  fixes until F8c). Width twins in lockstep on both ISAs. Canaries
  MIGRATED to masked expectations: runtime_shift_count_domain_exit
  (1<<40 u32 = 256, 1<<70 u64 = 64), runtime_shift_right_atwidth_exit
  (the -(2^62)>>70 = -(2^56) leg is the discriminator),
  runtime_shift_atwidth_signed_modular_exit (the old exit-71 "masked"
  arm IS now the pass; the retired modular zero is the failure),
  runtime_shift_atwidth_indexed_targets_exit (u64<<70 legs = 64). NEW
  pin runtime_shift_subword_masked_count_exit (differential 70; counts
  chosen to uniquely witness mask width — 3u8<<13 = 96,
  -32768i16>>25 = -64). The linux_x64 byte pin migrated: width-correct
  shl/sar PRESENT, retired clamp/saturate ABSENT, sub-word AND pinned;
  x86 runtime verification pends the x86 host's next fire (byte pins +
  unit tests cover it structurally). The state-values folder is
  UNCHANGED (it defers OOR-count folds to the runtime — consistent
  either way; folding masked is a possible later nicety).
  F8c LANDED (aarch64 lane, 2026-07-16) — TRAPPING COUNT-TRAP,
  value-blind: an out-of-range count traps BEFORE the shift on all
  three engines (`0 << 40` Trapping aborts even though 0 fits; the
  COUNT is invalid, not the result). interp: one Trapping count check
  ahead of the Sat/Trap shift arms ("shift count out of range in
  Trapping domain"). aarch64: append_shift_count_trap_guard (cmp +
  b.lo + brk, SHIFT_COUNT_TRAP_GUARD_WIDTH=12) — with_domain's
  Trapping `>>`/`>>>` take guard+plain-op (the CSINV/CSEL floor fixes
  are now Saturating-only, unreachable post-F8a, kept for
  robustness); both Trapping `<<` value blocks (64-bit recovery
  witness + narrow wide-compute) prepend the guard. x86_64 mirrors
  (cmp r11 + jb + ud2, 8 bytes) in the binary-write arm, the
  DomainShift operand arm, and append_saturating_trapping_shift_left;
  all width fns in lockstep. Pinned:
  pass/arithmetic/trapping_shift_count_traps (abort-style suite test
  with BOTH engine legs — native != exit 7 + abnormal termination,
  interp error contains the trap reason; in-range Trapping legs
  compute exactly first; the `>>` count face probe-verified — one
  canary can only die once). F8 IS COMPLETE: Exact obligation (F8a) +
  Wrapping masked count (F8b) + Trapping count trap (F8c); the
  shift-count ruling is fully engineered per ch5. Known residual
  (pre-existing, out of F8 scope): the state-values folder folds a
  CONSTANT Trapping `<<` VALUE overflow with wrap semantics (its own
  comment marks the value face unruled) while the runtime traps —
  fold/runtime divergence only for constant Trapping shl overflow;
  ruled by the F5/value-face work when it lands.

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

- **ProviderPlan + Console boundary migration (DESIGN-RULED,
  ENGINEERING-READY 2026-07-17):** LANE PICKED UP 2026-07-20; MACHINERY
  MAP banked (explorer): TWO distinct provider systems exist --
  (i) `provides` rows (`<target> provides <Trait> { method -> Binding }`
  -> HostProviderDefinition item.rs:76-133 with the CLOSED Binding sum
  {Syscall, DllImport, VtableSlot, VtableField, TableFunction, Value};
  mirrored as calling-conventions ProvidesRow/ProvidesBindingKind
  lib.rs:1001-1041; consumed by provides_values.rs (Value substitution),
  extract_provides_rows compiler.rs:65-136, merge_provides_rows
  lib.rs:1071-1174 into HostAbiPlan's HostBindingMechanism lib.rs:855-903
  -- THE home for F7's future Instruction arm), and (ii) the Wave-0
  operator-provider registry (BoundaryProvider providers.rs:30-43,
  effect_set/contract_ref/target_applicability -- the FIELD-SHAPE
  precedent, different axis). CONSOLE TODAY = platform block
  (console.omg:14-27, parser platform.rs) + HARDCODED
  insert_platform_lowering in calling-conventions {windows,linux,
  darwin}.rs (PlatformCallData variants lib.rs:926-958) -- the exact
  pair ProviderPlan unifies. Target defaults: NativeTarget::
  from_omega_target_name target/lib.rs:34-46; rows filter inert by
  target (builder.rs:79-88; provides_values.rs:44-52);
  <subsystem>.provides.omg recognized frontend.rs:503-524. SERVICE
  SCHEMA source = typed TraitDefinition trait_definition.rs:7-15
  (is_boundary + machines signatures) -- today read AD-HOC
  (parameter_count walk compiler.rs:142-168, interpreter Console
  detection evaluator.rs:4174-4196). PRV1 SLICE CUT (construction
  only, per the ruling): the ProviderPlan + ServiceSchema types in
  omega-effects/capabilities (beside providers.rs) -- mechanism sum
  aligned with HostBindingMechanism + room for the Instruction arm;
  call-shaping policy (the PlatformCallData variants) representable
  so the populate tables can migrate; ServiceSchema::from the typed
  boundary TraitDefinition; construct the built-in Console lowering
  as ProviderPlan VALUES in tests. Console's platform->boundary-trait
  PROMOTION waits for PRV2/PRV4 (validation/selection must exist
  before the special-cases move). PRV1 LANDED 2026-07-20:
  omega-effects/capabilities/provider_plan.rs -- ServiceSchema
  {trait_name, methods: [name, parameter_count, has_result,
  effects]}, ProviderBinding (the Binding sum's union with the
  platform mechanisms: Import|Syscall|VtableSlot|VtableField|
  TableFunction|Value + the F7 Instruction arm reserved),
  ProviderPlanRow {method, binding, call_shape: rendered
  PlatformCallData policy -- keeps PRV1 free of a
  calling-conventions dep; PRV2 normalizes}, ProviderPlan {name,
  target, schema, rows, effect_set, origin_package -- NO trust
  field: classification is admission output}. covers_schema() as
  the PRV2 preview; the built-in windows Console lowering constructed
  as a plan VALUE in tests (the PRV4 relocation shape). PRV2 CORE LANDED
  2026-07-20: ServiceSchema::from_typed reifies a typed boundary
  TraitDefinition (names, receiver-excluded parameter counts, result
  presence, declared effects -- pinned by a pipeline round-trip test
  in omega-validation's harness) and
  ProviderPlan::identity_fingerprint gives the NORMALIZED identity
  (FNV-1a over the canonical rendering; rows/methods sorted --
  presentation-invariance pinned). provider_plan re-exported at the
  omega-effects root. PRV2 COMPLETE 2026-07-20:
  DEPENDENCY DIRECTION DECIDED -- neither crate deps the other; the
  call-shape RENDER/PARSE pair lives WITH the PlatformCallData sum in
  omega-calling-conventions (render_call_shape/parse_call_shape,
  exact-inverse round trip pinned; unknown spellings surface as
  errors) so the plan/table seam never drifts silently, while
  ProviderPlan::validate_against_schema stays schema-only in
  omega-effects (NAMED errors: unbound method, duplicate rows, stray
  rows, Value-with-arguments; all pinned). The ABI-plan consumer runs
  parse_call_shape at merge time (PRV4). PRV3 ADMISSION VERTICAL
  LANDED 2026-07-20: pipeline/provider_plans.rs DERIVES a
  ProviderPlan per authored `provides` block (the bridge from
  today's rows to the typed carrier -- schema from the typed
  boundary trait via from_typed, bindings mapped from the closed
  sum, unknown traits derive empty schemas whose validation names
  every row stray); each plan surfaces as a trust-report row
  (`provider plan: <target>::<Trait> [<fingerprint>]`, dev-active
  standing warning until build.omg grants the plan by name or trait
  leaf). Pinned: derived_provider_plans_surface_as_trust_rows.
  PLAN RECEIPTS LANDED 2026-07-20: a grant naming a derived
  plan (by name or trait leaf) pins the plan's NORMALIZED IDENTITY in
  the lockfile -- the fingerprint IS the receipt hash, so any policy
  change under the grant drifts (pinned:
  granted_plan_receipt_pins_the_fingerprint -- grant, edit the
  binding value, refusal). PRV3 IS COMPLETE except SLOT-OWNER
  SELECTION, which IS PRV4's surface (the target-default set +
  per-slot overrides in build.omg). PRV4 REMAINING -- SUB-RUNGS CUT 2026-07-20 (the corpus-scale
  breaking pass; machinery map banked above): P4a CONSOLE VERTICAL -- P4a-1 THE
  LOSSLESS ORACLE LANDED 2026-07-20: ProviderBinding gained the
  HostOperations arm (rendered Capability::operation refs -- the
  populate tables' lowering-sequence shape);
  pipeline/provider_plans.rs derives the built-in Console plan FROM
  a populated HostAbiPlan (builtin_console_plan; scoped by the
  lowering's PLATFORM field -- "*"/"Console" vs the fs raw seam's
  "FilesystemHost", which shares state names like `write`) and
  converts rows BACK (plan_row_to_lowering: from_names +
  parse_call_shape); the round trip is EXACT for Coff/Elf/MachO
  (console_plan_round_trips_the_populate_tables) -- the
  precondition for retiring insert_platform_lowering into authored
  plans. P4a-2 THE PROMOTION LANDED
  2026-07-20 -- FAR CHEAPER THAN MAPPED: std's platform Console
  became boundary trait Console (with declared effects rows --
  stdout_io/stdin_io per method, giving the purity checker truth,
  one of the migration's stated goals) and the FULL BATTERY IS
  GREEN UNCHANGED: both engines' dispatch keys on the field/type
  name and the native lowering matches by STATE NAME, so neither
  cared how Console was declared. The platform KEYWORD survives in
  exactly TWO canaries (fail/domains/call_requires_platform_unproven
  + pass/domains/call_requires_platform_satisfied_by_caller_requires
  -- the platform-requires feature pins) and the parser/consumer
  machinery; retiring the keyword = migrate those two pins' subject
  to boundary-trait requires + delete the platform parse/consumers
  (P4d's sweep). P4a REMAINING: consume the derived Console plans at
  merge time behind the byte-identical oracle, then retire the
  tables' Console rows (the plans-as-source flip). P4b FILESYSTEM: the per-target Value provides rows
  (targets/<t>/filesystem.provides.omg) re-authored as plan values
  in std target packages; slot-owner selection = build.omg target
  blocks choosing the default plan set + per-slot overrides. P4c
  FLOAT/F7: the Instruction binding arm formalizes the hardcoded
  IEEE lowering (needs the instruction-plan machinery -- may gate).
  P4d RETIREMENT: the provides keyword + platform blocks + the
  populate tables die; corpus sweep (215 tokens, 31 canary-suite
  sites); loud decision diagnostics per the TPR1 precedent. retire the `provides` keyword and platform
  blocks. PRV1 add the typed `ProviderPlan` policy carrier over boundary-trait
  service schemas and existing `Binding` cases; PRV2 validate coverage,
  signatures, calling/layout plans, and normalized identity; PRV3 admit
  semantic claims through the shared grant/receipt pipeline, then select by a
  slot-owner capability; PRV4 move target defaults into ordinary omega::core/std
  target packages, have `build.omg` select the target default set plus explicit
  per-slot overrides, and migrate Console/filesystem/float providers and
  canaries. Construction is free, validation proves structure, admission buys
  commitments, selection binds a slot. Trust classification is admission
  output, never author-selected plan data.
- **Float-to-int cast overflow — implement proof-or-policy.** Exact =
  unproven obligation (prove via guard/declared range, NaN excluded by
  `x == x`); `in Saturating` = clamp all targets (NaN -> 0; x86 grows the
  clamp); `in Trapping` = trap; `in Wrapping` on float source = compile
  error. Uniform with decision-17 arithmetic + the narrowing-store
  keystone. Build, then retire the drift-ledger entry.

## Open bugs / gaps (ungated)

- **⚠️⚠️ HOST-DIVERGENT COVERAGE — the seven "regressions" are standing
  x86_64-WINDOWS gaps, OURS to fix (owner directive 2026-07-18; bisected
  same day, correcting this file's earlier attributions):** none of the
  red canaries regressed in the recent window. Bisected on this host:
  `runtime_f32_field_guard_exit`, `runtime_scalar_pun_shared_let_exit`,
  and `runtime_newton_sqrt_exit` are RED AT THEIR OWN BIRTH commit
  (`9e3875802`, the "recast native READ lands / float conjuncts fixed
  program-wide" commit — green only on its author's aarch64-darwin
  host); `storage/requires_slice_indexed_alias_field_binary_compile`
  (zero-layout-width WriteRuntimeFrameIndexedBinary) and
  `filesystem/discarded_self_call_literal_errno_exit` (host-arg
  refusal) are red at `8d0b33b8e`, the previously-assumed-green floor.
  `7640a6f7a` is EXONERATED — the earlier "smoking gun" flag here was
  attribution-by-proximity, not bisection. The authoring lane gates on
  aarch64-darwin where all seven pass; this host exposes real standing
  gaps. STATUS:
  (1)–(5) THE FLOAT FIVE — FIXED 2026-07-18, ONE LINE: float conjuncts
  in dispatch-edge guards (`self.f > 3.14 && self.f < 3.15`) kept SIGNED
  jcc conditions after `ucomis*`, whose flags are unsigned-style with
  SF=OF zeroed — the Less-failure branch (`jge`) was ALWAYS taken, so
  every float range window failed its upper bound on x86_64. The
  single-clause path already swapped to the unsigned forms
  (edges.rs ~1421 documents exactly why); the CONJUNCTION loop was
  missing `clause.is_float ||` in its swap condition. All five canaries
  (f32_field_guard, newton_sqrt, scalar_pun_shared_let, std_math_sin_cos,
  value_call_terminal) flip green 70/70 from the one edit.
  (6) STORAGE ZERO-WIDTH — FIXED 2026-07-18: WriteRuntimeFrameIndexedBinary
  was aarch64-only from birth (`unsupported_x86_64_encoding()` + a
  hardcoded width 0). The x86_64 encoding landed: the frame-indexed COPY's
  34-byte descriptor-deref address prefix + the base-indexed binary
  write's operand/op/store tail; a proper arch dispatcher for the
  left-operand relocation offset replaced the integer-width derivation
  (which was constant-51 on x86_64 and would have misplaced operand
  relocations), and the record gained the x86 push-gap for the right
  operand (identity on aarch64). Compile canary compiles; NEW runtime
  twin storage/runtime_slice_indexed_binary_rmw_exit pins the encoding
  green 70/70 (its left operand is itself an indexed READ, so operand
  relocations are exercised).
  (7) FS HOST-ARG — FIXED 2026-07-18 (red from the fs canary's own BIRTH
  on this host): a path riding a runtime slot (the discarded-self-call
  shape) resolves to a RuntimeStringPointer operand, which the win64
  import-call marshaller had NO arm for — the encoder's rejection was
  swallowed into width 0 and surfaced as the misleading "argument must
  be a simple value" refusal. The marshaller now stages string
  descriptors exactly like the darwin SYSCALL encoder always has
  (pointer word at +0; LEA past the len word for bounded buffers);
  win64_import_arg_is_staged carries the width + relocation sites. The
  fs canary flips green 70/70 and note_vault's mkdir refusal clears.
  file_journal FIXED same day — its exit 3 was NOT missing rows: the
  sample drove the raw seam POSIX-naively (raw `create`/`open(flags 0)`
  = msvcrt TEXT MODE; `\n`→`\r\n` on write made the read-back short by
  one). The sample now composes `O_BINARY` exactly like the std wrapper
  (`open_create(path, composed_flags_field, 438)` + `open(path,
  O_BINARY)`; identical on posix where O_BINARY is 0) — exits 7 on
  windows.
  THE ONE REMAINING samples red = note_vault. ⚠️ RULED 2026-07-18
  (owner): PORTABLE CONTRACT + PER-TARGET IMPLEMENTATIONS — long-view
  correct over cheap. Diagnosis: the seam's `read_dir`/`open_at`/
  `unlink_at` rows were TRACED from darwin's syscall table
  (getdirentries64/openat shapes), not designed, and the portable
  wrapper's dir-walk recursion was written against that Unix paradigm —
  a paradigm leak in the middle layer (the user-facing path-shaped API
  is fine, and the rest of the seam is genuinely portable POSIX-ish
  that msvcrt implements). Rust avoids this by keeping the portable
  layer path-shaped and resolving the paradigm per-platform BELOW the
  contract (its Windows remove_dir_all uses NT handle-relative ops —
  Win32 lacks dirfd but the NT layer HAS it). REJECTED: seam-level
  dirfd emulation (a) and path-reconstruction (b) as endstates.
  ENGINEERING SHAPE: the paradigm-split ops move behind a portable
  contract with per-target implementation machines (darwin/linux keep
  the fd/dirfd recursion; windows implements over
  FindFirstFile/handle enumeration; the interp mirrors per the
  cfg-mirror principle). SPELLING SETTLED 2026-07-18 (owner:
  "push complexity to edges"): target-filtered sibling IMPL files —
  `std/targets/<target>/filesystem_impl.omg` holds that target's
  implementation machines beside its legacy provides rows (to migrate to
  `ProviderPlan` values), gated by the same
  target filter; the portable layer declares the contract signatures;
  a selected target with zero or two implementations = loud compile
  error; enforcement = name+signature match first, `satisfies` later.
  RUNGS: contract declaration → darwin/linux impls move
  (behavior-identical, gates prove) → windows impl over
  FindFirstFile/handle enumeration → note_vault green, samples gate
  fully green. LAW banked: seam rows get DESIGNED signatures, never
  traced ones.
  MECHANISM RUNG LANDED 2026-07-18: `<target> machine Path(..) {..}` —
  the provides-table item prefix extended to machines. Machine items
  carry `target: Option<Identifier>`; a pre-resolution filter
  (pipeline/target_machines.rs, called in BOTH engines' pipelines right
  after provides substitution) clears the SELECTED target's marker
  (ordinary machine from then on — const-v0 discipline) and leaves
  every other target's machine inert (resolution skips marked
  machines, the provides-row precedent; four same-name impls never
  collide). Loud edges live in the filter: implemented-twice for the
  selected target + no-implementation-for-selected naming who does
  provide one (unknown target names are silently never-selected,
  matching the legacy provides rows — that inertness is also what makes the fail
  canaries host-portable via `demo_target`/`local_unchecked`).
  ⚠️ Parser-order landmine: the identifier-led target-machine peek
  sits BELOW the contextual-led items so `boundary machine` never
  reads `boundary` as a target name (found by the fail sweep).
  Canaries: pass/targets/target_machine_gating_exit (differential 70;
  local_unchecked + four real-target same-name impls),
  fail/targets/target_machine_{missing,duplicate}_rejected.
  Cross-target SIGNATURE agreement deferred to the `satisfies`
  enforcement rung (callers still typecheck against the selected
  impl — the safety edge; agreement is a quality diagnostic).
  IMPL-FILE RUNG LANDED 2026-07-18: the dir-walk family — SEVEN
  machines (read_dir_count/stats/nth, read_dir_entry_fd,
  remove_dir_all, rda_drain, rda_step; read_dir_is_empty STAYS
  portable, it composes read_dir_nth) — moved verbatim from
  filesystem.omg into four `std/targets/<t>/filesystem_impl.omg`
  files as `<t> machine` items, imported by the WRAPPER
  (filesystem.omg) -- ⚠️ NOT by the target defs: the raw seam imports
  every target def, so target-def-imported impl machines LEAKED into
  raw-seam programs with no `data Filesystem` (file_journal + 8 suite
  canaries broke on ghost wrapper machines; caught by the gate
  battery, import edge moved same day). Provides ROWS are
  self-contained facts and suit target-def imports; impl MACHINES are
  wrapper parts and ride the wrapper's imports. The files still LIVE
  beside their legacy provider rows pending PRV4 migration. filesystem.omg keeps
  the CONTRACT block (the seven signatures + the loud-edge note) where
  the bodies were.
  The three posix copies are byte-identical today; the linux headers
  flag the getdents64 record-layout divergence (d_type@18/name@19 vs
  darwin's @20/@21) to diverge WHEN linux dir canaries arrive — that
  divergence is what the shape exists for. The windows copy is a
  PLACEHOLDER byte-copy: still fails host lowering with the SAME five
  diagnostics (note_vault the one samples red, verified byte-alike),
  nothing worse, nothing hidden.
  NEXT RUNG (flips note_vault green), scoped 2026-07-18 — TWO
  sub-rungs:
  (3a) SEAM — LANDED 2026-07-18, all layers: the trio + _name twins
  declared; HostOperation variants; kernel32 rows; windows (+darwin
  _name) lowerings; x86_64 operand shapes reuse THREE EXISTING arms
  (find_first==Stat, find_next==FStat, find_close==Close — zero new
  marshalling); hermetic find-cursor model + real-fs mirror.
  pass/filesystem/windows_find_enumeration_exit green interp+native
  (#[cfg(windows)] gated, outside cross-host sweeps BY DESIGN — posix
  has no trio lowering). ⚠️ Landmine class rediscovered: a new
  HostOperation with no operand-shape arm = empty operands = the
  misleading "argument must be a simple value" refusal (surfaced via
  OMEGA_DEBUG_RECEIVER "result storage place did not lower"). Original
  spec follows for the record:
  three DESIGNED find-enumeration ops on FilesystemHost
  (the law: designed signatures, never traced):
  `find_first(pattern: &[u8], data: &mut [u8]) -> i64` (handle, -1 on
  error; pattern is TRUSTED PLAIN bytes, the D-at trust class exactly
  like create_dir_name — the impl constructs `dir\*` no_nul BY
  CONSTRUCTION), `find_next(handle: i64, data: &mut [u8]) -> i32`
  (1 found / 0 end), `find_close(handle: i64) -> i32`; plus
  `remove_name(path: &[u8]) -> i32` + `remove_dir_name(path: &[u8])
  -> i32` (trusted-plain twins of remove/remove_dir, same native
  rows — the create_dir_name precedent). WIRING (layer map recon'd):
  HostOperation variants FindFirst/FindNext/FindClose in
  omega-calling-conventions/src/lib.rs (closed enum + Custom escape;
  first-class variants); WINDOWS_IMPORT_ROWS += ("Filesystem",
  "find_first", "Kernel32.dll", "FindFirstFileA") + FindNextFileA +
  FindClose (+ remove_name→_unlink, remove_dir_name→_rmdir);
  insert_platform_lowering("FilesystemHost", "find_first", ...) in
  windows.rs (the darwin.rs:383 read_dir block is the template).
  Marshalling already exists: string-descriptor + buffer args land
  (2026-07-18 win64 work); FindFirstFileA returns HANDLE as i64
  (INVALID_HANDLE_VALUE = -1). INTERP: model the find-cursor family
  on the virtual FS (cfg-mirror; note_vault interp runs on windows
  host need it) — pattern `dir\*` → snapshot the dir's entries into a
  cursor table keyed by handle; WIN32_FIND_DATAA layout for `data`:
  dwFileAttributes u32 @0 (FILE_ATTRIBUTE_DIRECTORY = 0x10), cFileName
  NUL-terminated @44 (record 320 bytes; buffer >= 320). Differential
  canary: windows-host find-enumeration exit canary (interp+native).
  Posix targets never call these ops (their impls don't), so no
  darwin/linux rows needed — inertness does the gating.
  (3b) BODIES: rewrite the SEVEN windows impl machines over the trio:
  enumeration = find_first/find_next skipping "." / ".." by name
  bytes (NOT record position — find order is not guaranteed);
  remove_dir_all's dirfd stack becomes a PATH-PREFIX stack (full-path
  byte buffer + a depth array of path LENGTHS; descend = append
  `\name`, ascend = truncate to stacked length; drain shape and fuel
  discipline stay exactly rda_drain's); removals via
  remove_name/remove_dir_name on the joined full path (path joining
  below the contract IS the windows paradigm — Rust's split). Scratch
  fields (pattern buf, full-path buf, 320-byte find-data buf, length
  stack) go on portable `data Filesystem` (unused on posix, harmless
  ZII). THEN note_vault green natively + interp, samples gate fully
  green.
  (3b) BODIES — LIVE 2026-07-18. The repeated-slice-arg blocker was
  ROOT-CAUSED (cdb runtime session; see the guard-slot record below)
  and FIXED; the shelf's reference bodies dropped into
  filesystem_impl.omg verbatim, the `w_*` scratch block restored into
  filesystem.omg, and the >= 2 single-target-internal relaxation
  landed in target_machines.rs (loud missing-row edge fires only for
  names TWO OR MORE targets implement; a single foreign implementer is
  that target's paradigm internal and filters silently). Canary pair:
  fail/targets/target_machine_missing_rejected grew a demo_target2 row
  (the loud edge's >= 2 evidence) + NEW
  pass/targets/single_target_internal_machine_skipped. The reference/
  shelf directory is RETIRED (bodies live now; git history keeps the
  txt). Two byte-level landmines preserved in the live bodies:
  (i) `self.w_path_len = path.len` (slice-length -> field) has NO
  runtime lowering — the copy recursion's terminal index param carries
  it; (ii) rda_step's rds_rootdone zeroes `rda_depth`, or the fuel
  loop re-opens a find enumeration on the drained root every iteration.
  ROOT CAUSE (the repeated-slice-arg miscompile, closed 2026-07-18):
  CROSS-CONTEXT GUARD-SLOT ALIASING. The guard-operand resolver
  (omega-state-guards operands/layout.rs runtime_frame_operand_layout)
  falls back from exact (dispatch_index, source_key) matching to
  same-(machine, state) matching — but a machine expanded at TWO call
  sites has two full frame-slot regions under the SAME (machine, state)
  symbols (contexts stacked disjoint by
  stack_runtime_storage_by_call_context), and the bare first-in-arena
  match is the FIRST expansion's region. The second walk's tail-segment
  guard (`i < path.len` at the recursion's continue-or-done decision)
  read the FIRST call's stale i=2/len=2 slots, 2 < 2 = false, exited
  the copy loop at iteration 0, w_path_len = 0, the find pattern sealed
  as "/*", find_first failed, Ok(0) silently. Everything upstream
  (descriptor writes, forwards, param slots) was byte-perfect — proven
  by cdb value probes after hardware watchpoints FALSE-NEGATIVED (ba
  w8 armed on the exact written address never fired on this box; trust
  bp + dq, not ba). FIX: the pipeline exposes
  BackendPlan::state_contexts (dispatch arena index -> call-context id,
  the same table the stacking pass uses), threaded into
  build_state_guard_plan / lower_guard_conjunction; each fallback tier
  now PREFERS a same-context slot and crosses contexts only when no
  same-context candidate exists (a caller's guard reading a
  straight-line callee's terminal slot has NO same-context candidate —
  the value-return keystone family; a strict same-context-only v1
  broke three straight-line canaries). Regression pin:
  pass/filesystem/repeated_dir_walk_scan_exit (exit 70 = 66+2+2,
  windows-gated dual test, RUN_CANARIES row). AUDIT NOTE (banked):
  storage_places.rs find_runtime_frame_slot_for_path has the same
  lenient name-only tail arms — context-blind by the same argument;
  no failing face known (its tiers 3/4 prefer nearest dispatch <=
  query, which usually lands same-context), but a future
  cross-context face there should get the same preference shape.

- **AARCH64-DARWIN HOST-DIVERGENT GAPS — ALL CLOSED (aarch64 lane):** the
  mirror image of the x86-windows sweep above: three canaries red at
  origin/main on aarch64-darwin (green on the authoring x86 host), all
  fixed on the aarch64 lane. (1) const_fold_unsigned_shift_right_arg —
  NOT signedness: the ShiftRightLogical arm ignored `narrow` (always the
  X form), so a 64-bit nested Wrapping op's untruncated high bits shifted
  down into bit 31; new encode_lsrv_w_register + sub-word zero-extension
  (the logical twin of ASR's sign-extension), width fn in lockstep.
  (2)+(3) runtime_record_view_exit + its pass_canaries_compile echo — the
  machine-indexed ADDRESS write (§5b wide-referee recast) had no aarch64
  lowering; implemented by reusing the machine-indexed copy family's
  address prefix byte-for-byte (the walker's aarch64 branch rides the
  copy family's offset fns; store via the offset-materializing 8-byte
  path). ALSO fixed en route: the local-slice-forward segfault (see the
  promoted-pin entry above). SUITE ON AARCH64-DARWIN NOW FULLY GREEN:
  canary_suite 853/853, differential 14/14, native fs+gui 88/88.
  The pending float_to_int_overflow_divergence ledger row is now
  ARCH-AWARE (cfg target_arch: x86 native 70 / aarch64 native 99), so the
  drift gate holds on both hosts.

- **array_field_default_silent — FIELD DEFAULTS RETIRED, RUNG 1 LANDED
  2026-07-20 (ruled 2026-07-17):** the parser REFUSES `field: T = value` at
  the spelling site (directed diagnostic: ZII zero-initializes; constructed
  defaults belong in a constructor machine) -- an initializer can never
  parse and then disappear. CORPUS MIGRATED in the same pass (47 files, 68
  initializers): ZII-identical/incidental initializers deleted (`= 0`,
  `= false`, zero-variant enums -- incl. the dungeon x2 whose six defaults
  were all ZII-identical); load-bearing seeds moved to entry-top field
  writes (borrow_unique, the two runtime_mutable indexed canaries,
  guarded_leaf_branch_expansion); the dynamic-indexed canary's `index: u64
  = 0` initializer had FED the index-bound proof -- replaced with the
  declared-range pattern (`[0..=1]`, ZII 0 in range). RETIRED canaries
  (subjects died with the feature): pass runtime_field_default_exit +
  machine_field_index_initializer_compile, fail mixed_common_field_default
  + property_zero_init_nonzero_default + field_default_class_rejected +
  field_default_narrowing_rejected, pending array_field_default_silent.
  NEW PIN: fail/data/field_default_retired (array-literal default refuses
  with the directed diagnostic). RUNG 2 LANDED 2026-07-20: the
  DataField.initial_value carrier is GONE from all three
  representations (syntax/resolved/typed) and every consumer swept
  compiler-driven -- parser construction, resolver/typed lowerings +
  wire, identity counters, snapshot variants, validation's
  default-specific checks (cross-class/narrowing, mixed-common,
  zero_init-nonzero), the checker's field-integer range seeding +
  initializer value facts (the machine-OWNED data carrier is a
  different feature and stays), the interpreter's construction arm
  (ZII always), the four backend emission fns
  (select_entry_machine_field_default_writes + recursion + constant
  fold + lookup), and structural `==`'s omitted-field default
  fallback (an omitted literal field now always refuses -- the
  consistent tightening). TASK COMPLETE: the feature has no carrier,
  no emission, no checks, and one parse-site refusal + pin.

- **`pending_runtime_divergences_hold` — GREENED 2026-07-18 (ledger
  host-corrected):** (a) `float_to_int_overflow_divergence` now documents
  the x86 host pair (native 70 / interp 71; the header keeps aarch64's
  99 as the cross-target face) — the entry retires entirely when float
  ladder F4 (proof-or-policy) is built. (b) RESOLVED
  2026-07-18 (owner: "obviously implement"): the immutable-lend-for-
  `&mut`-param hole is ENFORCED — semantic check in
  validate_call_arguments_handles (bare-name `&mut` forwards resolve at
  whole-machine scope; everything else errors with the fix spelled).
  Repro PROMOTED to fail/calls/immutable_arg_for_mut_param_rejected;
  legal forward pinned by pass/calls/runtime_mut_ref_forward_exit; two
  corpus canaries respelled to explicit `&mut self.<field>`. The pin
  found while authoring the forward canary — a frame-LOCAL-backed
  slice descriptor forwarded across a state boundary going wild
  natively — was FIXED 2026-07-18 (aarch64-darwin lane) and PROMOTED to
  pass/storage/runtime_local_slice_forward_exit (differential): the
  STATE-STORAGE demand analysis elided the struct-literal local's slot
  (its only reference rode a later `let` VALUE, invisible to the
  liveness scan), so the descriptor had no address and every downstream
  strategy silently planned nothing (ZII null deref). Fix = the
  slice-view carve-out in state-storage collection.rs: a struct-literal
  local whose field backs an `as_slice`/`as_mut_slice` view in a later
  statement keeps its slot. Deliberately view-gated, NOT the array arm's
  any-later-`let` gate: plain field reads (`m.body`) stay fold-served
  (borrow_carrying_data_field_exit pins that — a blanket arm regressed
  it and was narrowed same day).

  (The earlier "CONFIRMED ON MAIN / five float failures survived onto
  committed main / bisect from 8d0b33b8e" block that lived here was
  SUPERSEDED by the host-divergence finding above — the five were never
  green on this host, and 8d0b33b8e was not a green floor. Kept as one
  line so the correction has a paper trail.)

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

- **Windows session — RUNNING 2026-07-16 (Next Tasks #4); slice 1 landed:**
  the full `cargo test --workspace` battery now runs GREEN on a windows
  host — it had never run here (cfg(windows)-gated tests compile out on
  the aarch64-darwin authoring lane). What it surfaced + fixed:
  (1) TEST-HARNESS PATH ESCAPES — generated probe programs embedded
  `temp_dir().display()` verbatim, so `C:\Users\...` read as string
  escapes (`\U`) to the lexer; build_config_granted + real_fs (4 tests)
  now embed forward-slash paths (omg_path helper).
  (2) REAL-PROVIDER OP PARITY, WINDOWS WALK — the parity probe was the
  darwin walk (mtime @48, symlink/chmod asserted SERVED); windows now has
  its own 11-step served-set walk (mtime @40) + a separate pin that the
  unix-gated trio (symlink/set_permissions/change_owner) refuses with
  ENOTSUP(45) in real mode BY DESIGN (2026-07-10m ruling) while the
  host-independent virtual fs serves them — a documented divergence, not
  hidden. The provider also gained read-only DIRECTORY opens on windows
  (FILE_FLAG_BACKUP_SEMANTICS; the open_at/unlink_at dirfd mint) — write
  opens still fail like unix EISDIR.
  (3) read_dir_nth KIND-LATCH BUG (std windows impl, both engines
  faithfully wrong): the scan drain keeps classifying records after
  capturing the target, so reading the RUNNING w_scan_kind after the
  drain reported the LAST record's kind — every file child claimed
  is_dir whenever the dir's last record was a directory. Fixed by
  latching w_hit_kind at the hit; pinned by
  pass/filesystem/windows_read_dir_nth_exit (dual-engine, windows-gated)
  + the interp coverage test that caught it.
  (4) fs<->time mtime INTEROP, WINDOWS LEG — LANDED:
  pass/time/runtime_fs_mtime_interop_windows_exit (decode @40, bridged
  through SystemTime, both engines 70; the darwin twin's TASKS_TIME #9
  blocker note is retired).
  VERIFIED-BY-EVIDENCE: the fs stat-row migration serves natively (the
  windows metadata/wrapper canaries green on this host); file_journal
  was fixed 2026-07-18 (host-divergent section); the two "stale efi
  byte-pin" tests show NO failure in the battery — that item was stale.
  SLICE 2 (positioned io) LANDED 2026-07-16: `Filesystem::read_at`/
  `write_at` went PER-TARGET (the positioned-io contract block beside the
  dir-walk contract) — posix keeps the atomic pread/pwrite raw ops;
  windows COMPOSES save-cursor/seek/op/restore over the wired
  _lseeki64/_read/_write rows (msvcrt has no pread; the honest
  non-atomicity caveat is in the contract). Pinned:
  pass/filesystem/windows_positioned_io_exit (dual-engine, windows-gated;
  pins the CURSOR CONTRACT directly — a plain read after both positioned
  ops still starts at byte 0). ⚠️ COMPILER FACE surfaced en route: a
  MULTI-CONJUNCT transition guard inside a VALUE-CALLED machine breaks
  the CALLER's guard lowering with a mispointed "guard needs runtime
  guard lowering" error naming the caller's enum-match; workaround =
  chain single-conjunct PURE verdict states (side-effect-free arm states
  are legal in value-called machines). Untriaged: support the
  conjunction there or repoint the diagnostic at the callee.
  SLICE 3 (hard link) LANDED 2026-07-16: `Filesystem::hard_link` went
  per-target (the hard-link contract block); posix keeps link(2); windows
  binds the DESIGNED `create_hard_link` seam op — kernel32 CreateHardLinkA
  mirrored exactly ((NEW link, existing, NULL security-attrs), BOOL
  result; the wrapper impl swaps the portable arg order and reports
  failure kind Other BY DESIGN: kernel32 sets GetLastError, not msvcrt
  errno — kind mapping waits on a win32 last-error surface). New layers:
  HostOperation::CreateHardLink; the [result, path, path, scalar]
  operand-shape arm; interp virtual + real-provider create_hard_link arms
  (BOOL, errno-silent). Pinned: pass/filesystem/windows_hard_link_exit
  (dual-engine, windows-gated; engine-agnostic legs — the hermetic model
  copies bytes, so the pins are create+readback, link-survives-removal,
  taken-name refuses with kind unpinned).
  SLICE 4a (the handle bridge + canonicalize) LANDED 2026-07-16: two
  designed seam ops — `get_osfhandle(fd)` (msvcrt _get_osfhandle; the
  fd→HANDLE bridge; hermetic + real models use identity handles over
  their fd tables) and `final_path_name_by_handle(handle, buffer,
  capacity, flags)` (kernel32 GetFinalPathNameByHandleA; Win32 return
  contract modeled exactly: length sans NUL / required-with-NUL when too
  small / 0 bad handle, errno untouched). `Filesystem::canonicalize` went
  per-target: posix keeps realpath; windows composes
  open/bridge/resolve/close (entry-body, capacity = the contract's 1024
  floor; the OPEN leg's errno is CAPTURED before the trailing close(-1)
  can clobber ENOENT with EBADF — the errno mapping runs in the ENTRY
  because the value-call fence bans arm-state mutations; resolve-leg
  failures = Other by design, GetLastError territory). KNOWN LIMIT
  (recorded in the contract block): `_open` refuses directories, so
  windows canonicalize of a DIR reports Error until a designed
  directory-open op exists. Pinned:
  pass/filesystem/windows_canonicalize_exit (dual-engine, windows-gated;
  per-model first-byte discrimination `\\` native / `o` hermetic +
  the NotFound leg; in WINDOWS_HOST_PASS_CANARIES preemptively per the
  positioned-io precedent). GetFullPathNameA never left the ledger
  (lexical-only, = Rust path::absolute, not fs::canonicalize).
  MODEL-FIDELITY FIX en route: the hermetic OPEN never followed symlinks
  (native open resolves them on BOTH families) — surfaced when the
  composition made open the canonicalize entry point;
  virtual_open_flags now resolves one symlink level (the
  canonicalize/read_link model) and the descriptor stores the RESOLVED
  path, so handle-keyed consumers report the final target like Win32.
  SLICE 4b (set_file_time) LANDED RAW, WRAPPER BLOCKED 2026-07-16: the
  designed `set_file_time(handle, creation, &access_ft, &write_ft)` seam
  op (kernel32 SetFileTime; NULL-able creation scalar; BOOL) landed at
  every layer + hermetic/real arms (FILETIME → unix secs via the
  calibration constants) — pinned RAW by
  pass/filesystem/windows_set_file_time_exit (dual-engine,
  windows-gated; hand FILETIME + stat round-trip @40; ⚠️ canary lesson:
  stamp on a CLEAN rw reopen — a pending write's close updates mtime
  past the stamp on both engines). The WRAPPER `set_times` per-target
  migration is REVERTED + BLOCKED on a NEW value-call face: the windows
  impl (FILETIME compose = 18 entry field-stores + bridge + SetFileTime)
  returns rc==0 natively when the machine is VALUE-called but works
  STATEMENT-called (`_ = fs.set_times(...)` exits 70 with the mtime
  verified) — three dodges all fail in value position (params hopped
  through fields; args from caller fields instead of literals; the
  compose hoisted into a statement-called helper), interp correct
  throughout. Same family as the multi-conjunct-guard face above
  (value-call expansion of effect-heavy entries); whoever picks up the
  face gets a ready differential repro shape. lock_file (LockFileEx)
  waits behind the same face (its wrapper impl would be the same
  compose-heavy value-called shape). The *at family and fd-based
  read_dir stay
  paradigm-refused on windows BY DESIGN (the dirfd paradigm has no Win32
  twin; wrapper-level windows impls already serve the walk); chown/symlink
  stay refused (no windows semantics / privilege-gated). WndProc entry
  stubs (title-bar close). WINDOWS_IMPORT_ROWS consumption (build the
  ProviderPlan form per the 2026-07-16 provider verdict — design-adjacent,
  own kickoff).
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
  explicit checked/required/external/accepted supply mode. Consumption
  eligibility is derived. `boundary: bool` is not the semantic model.
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
- **Generics completion:** stage-1 data monomorphization landed; machines/
  traits remainder.
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
- **Trust system engineering (design settled through the proofs arc):**
  boundary machines + grants + the unified lockfile (trust receipts beside
  package pins), engine veto, trust report, oracle tripwires, `defer`
  tooling (site marker + root row from one command, hash-pinned,
  package-release-fatal), grant locality (own-package dev-active w/
  warning; package boundaries inert until root-granted). FRONT-LOAD one shared
  grant/receipt carrier for accepted proof rows, sealed semantic-domain
  introduction (`MintAuthority`), progress-profile qualification, and admitted
  provider plans; packages may claim but never self-grant. Record: ch10
  Evidence And Trust + mathematical_proofs “Trust and accepted facts” /
  “Explicit proof citation”. LANE PICKED UP 2026-07-20; RUNGS CUT
  after a full machinery map (explorer, banked here -- the ch10
  carrier is DESIGN-ONLY today; what exists: SemanticDomainTable
  interner semantics.rs:309-371, MachineSupplyMode::Accepted
  UNPOPULATED (machine.rs:67-73 emits only Boundary|CheckedBody),
  ProgressProfileId never minted, TerminationGuarantee premises
  hard-coded empty at 3 sites, the mint judge's staged fence
  recasts.rs:119-211 does predicate discharge with NO authority
  half, `accept_boundary` has ZERO parse occurrences, no
  receipt/lockfile types anywhere; the working ANALOG to model on is
  the effects boundary report boundary_report.rs+builder.rs and the
  host-authority gate host_authority.rs; artifact writers live in
  omega-artifacts/src/lib.rs). THE LADDER (sealed-domain
  introduction first -- identity, interner, judge, and a failing
  canary already exist):
  GR1 LANDED 2026-07-20: omega-core/src/trust.rs -- TrustCommitment
  (SemanticDomainIntroduction | ProgressProfile | AcceptedFact |
  ProviderPlan), TrustProvenance (OwnPackageDev dev-active-with-
  warning | RootGrant), TrustGrant, TrustReceipt {commitment,
  statement_hash, provenance}, TrustGrantTable (grant/authority --
  RootGrant outranks dev -- /admit-dedup/receipts); 2 unit tests.
  Admission is OPAQUE by construction (the carrier never interprets
  commitments; consumers validate, the carrier answers "granted
  here?").
  GR2 LANDED 2026-07-20: judge_qualification_cast consults the
  carrier FIRST (in_program_trust_table grants every declared
  domain's SemanticDomainIntroduction as OwnPackageDev -- grant
  locality v1: in-program mints all pass, ZERO behavior change; the
  consult is the SEAM where package inertness and GR3 root grants
  bite, with the refusal diagnostic already speaking accept_boundary).
  Standing-warning rows deferred to GR5 (a warning per mint would
  spam; the report is the right surface). The staged predicate fence
  is untouched (semantic_cast_mint_staged keeps pinning it).
  GR3 LANDED 2026-07-20: the postfix carve parses the RULED spelling
  b.accept_boundary<pkg::symbol>() (join/load/store carve precedent)
  and desugars to the MARKER-NAMED zero-arg call
  accept_boundary#<path> (the asm#hlt / __destructure__ convention
  -- ZERO representation changes); the evaluator serves the marker
  as a no-op (grants are declarations, not runtime effects); the
  build-config pass harvests grants STATICALLY from the build
  machine's statements (BuildConfig.grants, dedup, order-preserving)
  and the pipeline threads them into the trust report -- a granted
  domain's row flips to `root grant (build.omg)` and drops the
  standing warning; a grant naming no domain surfaces as an
  accepted-fact row. Pinned: root_grant_flips_domain_row test +
  fail/build/accept_boundary_outside_build (ordinary source refuses
  via marker resolution -- a package can never self-grant; directed
  diagnostic is a polish item).
  OPENER NOTES (2026-07-20): the build machine is identified by
  is_build_machine (named build/<C>::build AND declared at a
  build.omg root -- the FILE is the identity) and evaluates through
  the granted interpreter entry with ALLOWED_BUILD_EFFECTS
  {filesystem_io, stdout/stderr_io}; BuildConfig extraction reads
  the returned Build value (compute_build_config,
  pipeline/build_config.rs:75). accept_boundary would ride the SAME
  evaluation: v1 shape = recognize the accept_boundary CALL
  statements in the build machine's body (a symbol argument in
  angle brackets -- CHECK whether ch13's call surface parses
  explicit angle-bracket arguments on method calls today; if the
  spelling needs the `<machine M>` surface it may be gated on that
  owner question -- if so, a v1 STRING-path argument or a dedicated
  statement recognizer inside the build machine avoids the gate,
  judgment call to record), populate TrustGrantTable RootGrant rows,
  and thread the table from compute_build_config into validation
  (replacing GR2's per-run in_program_trust_table with
  in-program-grants + build grants).
  GR4 LANDED 2026-07-20: pipeline/trust_lockfile.rs -- omega.lock
  lives BESIDE build.omg (machine-written; must persist across
  builds to see drift), one receipt row per grant
  (`<fnv1a hex>  <commitment>`; inline FNV-1a so the hash never
  varies across Rust releases); a domain grant hashes the domain's
  name + rendered facts, an accepted-fact grant hashes its path
  until boundary statements carry bodies. Drift under a grant FAILS
  the build (directed error naming the file); re-approval v1 =
  delete the stale row/file (the defer-tooling item owns one-command
  UX). No grants -> no lockfile (canary corpus untouched). Pinned:
  lockfile_written_and_drift_fails_until_reapproved (write -> drift
  refusal -> re-approve round trip).
  GR5-LITE LANDED 2026-07-20 (REORDERED before GR3/GR4 -- a root
  grant is only OBSERVABLE through the report, so the report comes
  first): omega-artifacts gains TrustReport/TrustReportRow +
  ArtifactWriter::write_trust_report (trust_report.md; written even
  when empty -- the honest no-commitments statement);
  pipeline/trust_report.rs writes one row per declared domain
  (own-package dev-active + the STANDING WARNING naming
  accept_boundary), called at the typed stage beside the wire
  report. Pinned: tests/trust_report.rs (dev-active row + standing
  warning; empty report). GR5 REMAINDER (with GR3/GR4): RootGrant
  rows without the warning; receipts as the row source instead of
  re-derivation; accepted-fact/profile/provider rows (GR6).
  PARSE-SURFACE FINDING (GR3): the expression grammar has NO
  angle-bracket call arguments anywhere (no turbofish, no
  type_arguments on calls) -- accept_boundary<path>() needs new
  surface; a dedicated postfix carve for the accept_boundary member
  spelling is the recorded v1 (avoids inventing general machine
  generics ahead of the <machine M> owner question).
  GR6 the remaining consumers -- qualification AUTHORITY half
  (qualifications.rs:26-32), ProgressProfile minting + the three
  premises stubs, MachineContractPlan permission half, boundary
  machines populating Accepted. GR6(d) PROBED 2026-07-20 (the most
  bounded consumer; its rule is owner text -- "Accepted is the
  existing bodyless `boundary machine` proof form"): the BODYLESS
  form DOES NOT PARSE today (`boundary machine f(a: Nat) -> Nat
  ensures ...;` fails at the contract terminator -- the machine
  parser demands a body block). The arc: (1) parser accepts the
  bodyless form (`;` after contracts; body-less = zero states), (2)
  supply_mode populator (machine.rs:67-73) emits Accepted for
  boundary+bodyless, (3) JUDGE GATING -- instantiate_citation
  currently believes any callee's ensures because they were VERIFIED
  in-batch; an Accepted machine's ensures is UNVERIFIED and must be
  believed only under dev-activity/grant (the TrustCommitment::
  AcceptedFact consult -- the soundness half), (4) accepted rows in
  the trust report (dev-active standing warning / root-granted) +
  lockfile receipts hashing the axiom's rendered ensures, (5) the
  engine-veto rule (a refutable accepted statement is a compile
  error, grants notwithstanding) can ride the existing judge
  Refuted verdict. GR6(d) LANDED 2026-07-20 -- THE ACCEPTED TIER IS REAL:
  (1) the bodyless form PARSES (`boundary machine f(..) ensures
  <fact>;` -- the machine-final `;` peeks past fact separators and
  stays for parse_machine when a non-brace terminator follows, so
  `ensures F; {` keeps its body; hard item keywords terminate the
  fact list; the clause loop stands down at `;`; the implicit entry
  state still materializes EMPTY so citations bind parameters;
  syntax Machine gains `bodyless`, refused without `boundary` at the
  item parser); (2) the populator emits MachineSupplyMode::Accepted
  (its first population); (3) the JUDGE believes an Accepted
  machine's ensures (dev-active grant locality) instead of proving
  it, with the ENGINE VETO live -- a structurally REFUTED claim is a
  compile error, grants notwithstanding; the empty-body return-type
  check exempts Accepted; (4) instantiate_citation delivers axiom
  ensures to citing lemmas (probed: mul_comm_axiom closes a lemma
  the judge cannot prove); (5) trust-report rows per Accepted
  machine (dev-active standing warning / root-granted via
  accept_boundary name match). Pinned:
  pass/proofs/accepted_axiom_cited_exit (dual-engine + report-row
  asserts), fail/proofs/accepted_axiom_engine_veto. AXIOM RECEIPTS LANDED
  2026-07-20: a granted axiom's lockfile receipt hashes its rendered
  ensures (editing the CLAIM under a grant is drift -- pinned by
  granted_axiom_receipt_drifts_on_claim_edit). REMAINING GR6 (each
  opens a subsystem arc; entry points): (a) qualification AUTHORITY
  half -- qualifications.rs:26-32 body_committed gains the published
  half when the STR4 publication model lands (rides GR6c); (b)
  ProgressProfile minting + the three premises: Vec::new() stubs
  (machine.rs:121-125 resolved, machine.rs:292 typed,
  termination.rs:138 checked) -- needs the TPR4 profile-declaration
  spelling (a profile is a sealed semantic domain over a
  boundary-provider capability, ruled 2026-07-17); (c)
  MachineContractPlan permission half + PRV3 provider admission --
  the PRV1/PRV2 ProviderPlan carrier is the prerequisite (its own
  NEXT-TASKS entry). The CARRIER ITSELF (task #3's subject) is
  COMPLETE: types, consult, root grants, lockfile receipts + drift,
  trust report, and the Accepted tier end-to-end; the remaining
  consumers plug into it as their own subsystems land.

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
  non-copy fields surface unsoundly; field names containing `__` mis-split
  the LET marker encoding (spurious unknown-field error, never a masked
  missing-field).
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
  follow-up if they block something).

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
- **Core multiplicity slice (decision 21):** add the first `[linear]`
  acknowledgement token. Terminal consumption needs no annotation: an ordinary
  `move self` call consumes when no returned outcome carries the obligation,
  while a `try_*` incomplete outcome must return the live token. Pin create ->
  multi-binding transfer -> consume as one obligation; reject scope loss,
  copy, mixed branch treatment, and implicit zero-created obligation; add
  `Empty | Live(Token)` path-sensitive acceptance. Then make `Task<T>` a
  customer after the core checker, not the bootstrap implementation vehicle.
- **Task-runtime slice (settled task model):** TR1 retire the synchronous
  `spawn` desugar, parser-erased `Join<T>`, statement detach, and old canary
  spellings with directed diagnostics; TR2 add core `[linear] Task<T>`,
  transactional start and terminal task-outcome sums, and path-sensitive
  settlement/transfer tests; TR3 elaborate `runtime.start<M>(args)` from the
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
