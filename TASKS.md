> OWNER_QUESTIONS.md (repo root) consolidates all lanes' pending owner decisions — batch-answerable.

# Tasks

Working backlog only. Finished work lives in the git log; canary headers carry
each fix's story. (Condensed 2026-07-12 per owner directive.)

## Current Strategic Focus

Omega's first real consumer is the Cathedral OS (`../Cathedral`). The gap
analysis lives in [wiki/cathedral_alignment.md](wiki/cathedral_alignment.md) —
Tier 1 items there (ZII guarantee, wire data semantics, versioned data,
separate-compilation awareness, concurrency/atomics decisions, freestanding
target, enum payloads) bias which vertical slices get picked next.

## Owner-gated holds (see OWNER_QUESTIONS.md)

- **NO-RECURSION scope (Q5, with Q6/Q7).** Zach's countdown note reads as
  banning the bare `-> own_entry(..)` spelling too; holding the one-way
  teardown for the batch answer only. Pre-scoped blast radius on a "banned"
  answer: reject bare entry re-entry + the statement-position
  `self.drip(n-1);` route (Q7) + (per Q6) mutual value-call cycles; convert or
  delete the ~12 pass canaries pinning the spelling (termination/measure
  family, recursive-walk pins, loop_{accumulator,rotation}, bind-first serve,
  lattice dual_accumulator); remove the orphaned machinery (entry-reentry
  `decreases` proof surface, recursive-clone specialization, the
  unserved-recursive-result sweep in runtime-storage planning.rs); rewrite
  corpus users (proofs canaries, std fs mkall, dungeon find_item pair) into
  explicit sub-state self-transition loops.
- **Float domain clauses (Q8):** `f: f32 in Saturating` compiles but means
  nothing (both engines run plain IEEE). Reject or define.
- **Range under non-Exact (Q9):** `i: usize [0..=4] in Wrapping` accepts
  `self.i = 100` — the range only enforces under Exact. Ill-formed, or
  wrap/clamp into range at stores?
- **Underspecified numeric-range ops (design thesis, flagged for Zach):**
  shift-amount >= width and float-to-int out-of-range are the same shape —
  behavior undefined outside a range, native and interp diverge, neither
  canonically correct. Proposed: extend decision-17 — make the RANGE a proof
  obligation (compile error otherwise), one ruling covering both + future
  corners. Related parked divergences + the const-fold family live in the
  pending-canary ledger (self-watching on compile + runtime axes).

## Open bugs / gaps (ungated)

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
- **Native-emission gap (clean refusal): state-calls into a BRANCHING callee
  entry with arguments.** Interp supports it; native refuses ("needs guarded
  state-call expansion"). Scope spike says NOT small: needs a new planned
  expansion threaded through planner + emitter
  (omega-emission-planning/state_call_blockers taxonomy). Workaround: inline
  the second dispatch or enter through a non-branching state.
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
- **[RESEARCH, sidesteppable]** nonlinear index `pixels[y*W+x]` isn't provable
  (no product-bound fact); route around with a linear counter until an axiom
  or octagon domain is added.

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
