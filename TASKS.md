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

## NEXT PICK (owner priority 2026-07-15): Cathedral M2 unblock — two red efi tests

Cathedral is fully written and waiting; its milestone 2 (`GetMemoryMap →
ExitBootServices → first Region mint`, `../Cathedral/source/boot/uefi/
own_machine.omg`) is blocked on exactly the two currently-red tests in the
known-failure baseline — no lane is driving them:

1. **`targets/efi_vtable_call`** — REGRESSION: the boot-verified M1 dispatch
   (`provides TextOutput { output_string -> VtableSlot(1) }`) went width-0 at
   the vtable encoder (2026-07-11 note). Fix = restore existing machinery.
   ⚠️ Green here does NOT compile Cathedral: its source is authored in the
   FIELD MODEL (`provides Trait over Struct { method -> field }`, extern
   brief §12, decided 2026-07-04 — offsets from the declared struct, header
   handled free), which has ZERO compiler implementation today (no
   VtableField anywhere). That's the real dispatch work; the slot fix just
   un-reds the baseline and de-risks the encoder underneath it.
2. **`targets/efi_ref_param_call_arg`** — `&mut` out-params through that
   boundary call, MS-x64 (addresses passed for `get_memory_map`'s five
   out-params). M2-ladder item #1.

Then smoke the third mechanism behind them: the runtime-offset borrow-recast
`&self.map_buf[offset] as &EfiMemoryDescriptor` strided by runtime
`descriptor_size` (M2-ladder #3; the indexing + bounds-proof substrate landed
in the 2026-07-09..13 arcs, unverified against the recast spelling).

Done-check = the M2 ladder's: boot under QEMU/OVMF, greeting prints,
ExitBootServices succeeds against the fresh MapKey, no crash after exit (the
machine idles, owned). Substrate already in place: `uefi_x64` registered +
`uefi_hello` cross-compiles (3915d1cec/631fa6e28), `const` v0, both-runtime
indexing, declared-range bounds discharge. This is the highest-leverage pick
on the board: it turns the booting toy into an OS that owns its RAM, and it
is the first end-to-end exercise of the boot/FFI stack (dispatch + out-params
+ recast under one roof).

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
- **FLOAT-TO-INT half still open (no ruling).** `1e300 as i32`: aarch64
  FCVTZS + interp saturate to i32::MAX; x86 CVTTSD2SI gives the 0x80000000
  "integer indefinite". Parked cast divergence stays in the drift ledger.

## Open bugs / gaps (ungated)

- **OWNER DIRECTIVE: `usize` is not an Omega type -- retire it** ("we have
  addr, we have primitives", 2026-07-13). Recipe:
  wiki/architecture/usize_retirement_execution.md (inventory: 380 .omg
  files, ~15 compiler files, ~8 chapters; usize -> u64, addresses -> addr,
  isize -> i64). Stages 1+2 DONE (2026-07-15): termination
  accepts u64 naturals; the corpus is swept (380 files usize -> u64, the
  isize canary -> i64/renamed; zero usize left in Omega code). The sweep
  surfaced + fixed: wire byte-count/count-companion contracts (now u64,
  usize tolerated until the type dies) and a REAL proof gap -- only
  u32/usize carried type-level range facts, so u64/u8/u16 fields had no
  `>= 0` fact and proved weaker (primitive_constraints now covers the
  unsigned family). Stage 3 (wiki) + stage 4 (compiler
  rejection: variants + builtins + AtomicUsize deleted, tolerances
  collapsed, fail canaries types/usize_rejected + isize_rejected) landed
  2026-07-15 -- the std/core/host/lattice-corpus .omg trees were swept too
  (the recipe's inventory had missed them). FOLLOW-UP: region.omg's
  allocate return + deallocate `address` parameter are ADDRESSES and should
  be `addr` per the index/count/address brief -- move the trait, target
  providers, and callers together (annotated at the site).

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
