> OWNER_QUESTIONS.md (repo root) consolidates all lanes' pending owner decisions — batch-answerable.

> OWNER: Migrate questions from this into OWNER_QUESTIONS.md, reconciling duplicates.

# Tasks

Working backlog only. Finished work lives in the git log; canary headers carry
each fix's story. (Condensed 2026-07-12 per owner directive.)

## Current Strategic Focus

Omega's first real consumer is the Cathedral OS (`../Cathedral`). The gap
analysis lives in [wiki/cathedral_alignment.md](wiki/cathedral_alignment.md) —
Tier 1 items there (ZII guarantee, wire data semantics, versioned data,
separate-compilation awareness, concurrency/atomics decisions, freestanding
target, enum payloads) bias which vertical slices get picked next.

## Probe rotation (current state)

Swept clean and pinned where novel (2026-07-12..13): operand positions
(left/right asymmetries), comparison complement flavors (equal-operand `!=`
legs), staleness (sum reassignment vs equality; slice-capture is borrow-
fenced), ZII boundaries (strings, sums-as-first-case, arrays, nesting, host
marshal), deep-nesting writes + aggregate arg marshaling, range endpoints,
u64 high-bit wrapped ops. Found + fixed en route: wrapping operand
truncation, text `!=` inversion (both ISAs), TextEqualsLiteral x16 clobber
+ the x15 pool collision. Marginal probe value is now LOW -- next sweeps
should target NEW feature surfaces as they land, not re-walk these axes.

## Owner-gated holds (see OWNER_QUESTIONS.md)

- **Recursion scope -- RESOLVED by your OWNER_QUESTIONS answers ("machine
  call cycles = banned... 'decreases' stuff is for states. States are not
  recursion. They are transitions, jumps, goto... equal to a for loop").**
  You asked "Am I missing nuance?" -- no: that distinction is exactly the
  implementation reality. The bare `-> own_entry(args)` loop-back COMPILES
  AS A TRANSITION (a jump with re-bound args, constant stack, no call
  frame), so it is a for-loop under your ruling and STAYS, along with the
  states-scoped `decreases` proof surface and its canaries. The pre-scoped
  teardown dissolves into the two CALL-graph bans you confirmed, filed
  below as engineering: mutual value-call cycles (Q6 "yes fucking banned")
  and statement-position tail self-calls (Q7 "banned, go write this as
  states").
- **Float domain clauses -- ANSWERED (deferred).** Owner: deferred until a
  float domain pass; prerequisite is "a serious language document detailing
  all compiler-supported float domains." Until then `f32 in Saturating`
  keeps compiling as plain IEEE. Filed under Big arcs: the float-domains
  language document.
- **Range under non-Exact -- ANSWERED: "just a compile error."** Reject the
  `[range] in Wrapping/Saturating/Trapping` combination at declaration.
  Implementation task filed under Open bugs below. (The example's `usize`
  drew a second ruling -- see the usize retirement directive below.)
- **Q: what should out-of-range SHIFT COUNTS and FLOAT-TO-INT casts do?**
  Two concrete programs, currently platform-divergent:
  1. `let x: u32 = 1; let y: u32 = x << 40;` -- the shift COUNT (40) exceeds
     u32's 32 bits. Observed: aarch64 (LSLV masks the count mod 64) and the
     interpreter both give 0; x86_64 (SHL masks mod 32) gives `1 << 8` =
     256. Same program, two answers.
  2. `let f: f64 = 1e300; let n: i32 = f as i32;` -- the float exceeds i32's
     range. Observed: aarch64 FCVTZS and the interpreter saturate to
     i32::MAX; x86_64 CVTTSD2SI gives the 0x80000000 "integer indefinite".
  Both faces are parked as pending canaries (cross-arch divergence held in
  the drift ledger). OPTIONS: (a) extend decision-17 -- the count/value
  RANGE becomes a proof obligation, out-of-range is a COMPILE error unless
  proven in range (consistent with Exact arithmetic; our recommendation);
  (b) define one runtime semantic (e.g. mask counts mod width; saturate
  casts) and make both backends implement it; (c) per-target behavior
  (rejected by precedent: silent cross-arch divergence). Pick a/b/c.
  > Owner: Shift overflow is defined by the domain on which the operator is happening. If y is in wrapping, it has a domain where lhs should be wrapping, and rhs doesnt matter. If you mix domains here, the operator should not resolve to anything, its a compile error. If y is in saturating, domain should assume lhs is saturating, and rhs doesnt matter. Domain casts (x as Saturating) solve the case where we need to change domains, and its always explicit.

## Open bugs / gaps (ungated)

- **Implement the Q6 ban: MUTUAL value-call cycles are rejected.** "Yes
  fucking banned" (2026-07-13). The state-call cycle check does not see
  value calls; add the value-call cycle walk over the machine-call graph
  and reject any cycle (A calls B calls A; self value-call cycles are
  already rejected). Corpus: the dungeon's find_item_at/find_item_after
  pair must be rewritten as states. Fail canary pinning the diagnostic.

- **Implement the Q7 ban: statement-position tail self-calls are
  rejected.** A trailing `self.drip(n - 1);` statement still compiles via
  the Nested-transition route -- "banned, if it reads as recursion...
  go write this as states" (2026-07-13). Reject the spelling with a
  write-it-as-states diagnostic; fail canary.

- **Implement the shift-domain ruling.** "Shift overflow is defined by the
  domain on which the operator is happening... lhs domain governs, rhs
  doesn't matter; mixed domains = the operator does not resolve (compile
  error); domain casts solve explicit changes" (2026-07-13, the
  numeric-range thesis answer). Scope: define wrapped/saturating shift
  COUNT semantics per the lhs domain, keep Exact shifts as proof
  obligations, verify the mixed-domain operator rejection covers shifts,
  and retire/rewrite the parked shift-at-width divergence canaries per the
  ruling. The float-to-int half of the thesis remains open (no ruling
  yet; the parked cast divergence stays).

- **Q9 IMPLEMENTED as a declaration-level rejection** (`[range] in
  Wrapping/Saturating/Trapping` is ill-formed on every declaration site;
  fail/types/range_under_non_exact_domain_rejected pins it).
  > Edit: I misunderstood this one at first -- I offered clarity in OWNER_QUESTIONS.md
  [Read your clarity. Your message sketch ("Exact assignments must be
  within invariant range...") reads as STORE-level enforcement with the
  declaration staying legal. We shipped the stricter DECLARATION-level
  rejection (the combination cannot exist, so no store question arises).
  If you want the declaration legal + range enforced at stores instead,
  say so and we swap the check.]
- **OWNER DIRECTIVE: `usize` is not an Omega type -- retire it.** "We do not
  have usize. We have addr, we have primitives. Conflating addresses & size
  is a semantic disaster" (2026-07-13). Current reality: the compiler
  ACCEPTS `usize`, ~366 canaries and several guide chapters use it, and
  prover diagnostics print it. Scope (String-retirement-scale): pin the
  addr/primitive split semantics from the owner's existing notes, add the
  compile-time rejection (or alias-with-warning migration step), sweep the
  corpus + chapters, purge the diagnostics. NOT a background-tick item;
  needs its own execution recipe first.

- **Multi-arm TEXTEQ-valued locals drop silently on the leaf route (found
  2026-07-13 probing the fresh scoped-leaf-key surface; parked at
  pending/calls/multiarm_texteq_local_divergence, both drift lists).** The
  scalar flavor works (the account_ledger fix's canary); a texteq
  initializer emits NO text-compare and NO local write in the leaf
  expansion -- arms deliver ZII 0 (native 71 / interp 70). Fix in
  branches/leaf.rs: reach the frame-slot text-comparison writer from the
  leaf route, or poison unserved arm-local initializers loudly. NOTE: the
  fs lane owns the just-landed scoped-key machinery -- coordinate before
  fixing (work-stealing protocol).

- **Const-folder width-blindness: latent, currently unreachable via the
  live spelling.** The 2026-07-04 miscompile class (`(0u32 - 2) >> 1` folding
  through bare i64) no longer reproduces as a FOLD: the mandatory cast-retag
  spelling (`0 as u32 in Wrapping`) puts a Cast node in the tree, which the
  folder's literal window refuses -- the expression reaches the RUNTIME
  operand path instead (whose wrapping-truncation hole is now FIXED and
  pinned by arithmetic/runtime_wrapping_operand_truncation_exit). The folder
  (`omega-state-values/simplify/folding.rs`) is still i64-window/type-blind
  by design (D14 comment); a width-carrying folder remains the deeper rung,
  gated with the type-carrying-constants design.
- **UnloweredCaseLiteralField poison is now UNPINNED by a fail canary.**
  Every previously-poisoned texteq shape serves (terminal position landed:
  the write rides the binary write's own target arms, and the
  TextEqualsLiteral operand encoder moved off x16 -- it was clobbering the
  write's target base; pass/text/case_literal_texteq_terminal_exit pins it,
  with the x15 precedent note). The poison stays as negative space for the
  NEXT unloweable payload-field shape; when one surfaces in authoring, give
  it the fail canary.
- **Same-type receiver aliasing** — CLAIMED by the fs lane (TASKS_FS.md
  "Stolen work #2"); per-instance receiver phases have been landing. Retire
  pending/time/value_machine_receiver_field_postentry when their arc closes.
- **Float `is_float` on nested operand paths: not silently reachable
  (probed 2026-07-12).** Nested float binaries serve in write-value,
  transition-arg, and spliced-mutation positions (pinned:
  arithmetic/runtime_float_nested_operand_exit); guard-position nested
  arithmetic fences on the conjunction rule; case-literal terminals are
  poisoned. The `is_float: false` notes in the tree/branch resolvers stay as
  latent markers -- if a route change makes one reachable, the canary legs go
  loud. Wire on first real reproduction.

## Programmable-layouts remainder (ch19/20/21; chapters are the spec)

- **L4 full:** derived projections into a plan-laid BYTE VIEW + the no-op
  boundary theorem — needs the L5 carrier/domain rung.
- **L5 remainder:** target-directed `encode()` (spelling open, extern brief
  §10.2), the `Packed` grammar, the plan-walking deriver (blocked on
  case-vocabulary Plan element construction), the validate/materialize decode
  mint, refinement-as-obligation.
- **RECAST (settled §5b):** borrows under a second stated shape spelled `as` —
  checker borrow-recast form + plan-tiling/fact-implication validator. Queued
  behind the validate-mint rung.
- **L6+:** Bits placements + access classes (MMIO deriver); durability plan
  grades; publish-time predecessor diff.

## Language ergonomics

- **[ENGINEERING]** numeric intrinsics remainder: sin/cos need range reduction
  + a polynomial matching interp precision — a numerical mini-project.
- **Nonlinear index `pixels[y*W+x]` -- ANSWERED: enabled by dependent types
  eventually** (planned, huge, not in language docs yet). Until then the
  linear-counter workaround stands; no axiom/octagon stopgap.

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
- **Wire data stage 2 remainder:** String decode (borrow-facts), nested/
  repeated fields, wire-schemas-as-program-types, runtime layout of wire
  values, encoding families beyond compact_binary v0, version negotiation.
- **Versioned data stage 3:** the era tag itself (+ decision 10's wire-era
  ride), era-tagged containers, migration chains / `replaces` / quiescence.
- **Equatable synthesis:** a CALLABLE conformance surface is still open.
- **Signed/unsigned residue:** sibling shape (2) only.
- **Concurrency model:** chapter 17 is a sketch; per-target declarations.
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
- **Default-domain invariants (relax follow-up):** pin the declaration
  surface + init-syntax for cross-field-related `self` reconstruction at
  implementation time.

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
- **Relax semantics:** scopes flatten structurally; the checked-tree/proof
  pass (mark relaxed place, exclusivity, restore obligations at exit) pending.

## Vertical slices

- **Vec[T]:** owned dynamic storage with length/capacity (surface declared;
  storage/lowering pending; allocator-story dependent).
- **as_slice/as_mut_slice:** back with real boundary-primitive storage.
- **Ownership events:** continue appending transfer/drop events from the
  remaining ownership forms; lower abstract summaries into explicit backend
  transfer ops.
