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
> fs lane 2026-07-17: rungs 1+2 DONE (the FIELD MODEL serves --
> pass/targets/efi_vtable_field_call -- and `&mut` out-params marshal as
> addresses with six-arg stack spill -- pass/targets/efi_out_param_call;
> the "two red efi tests" proved stale, dispatch bytes verified
> cross-target). Rung 3 SMOKED: the recast refuses at the PARSER (`as`
> takes no reference type) -- M2 now blocks on exactly the queued RECAST
> rung below (SS5b). It is the last compiler-side blocker.

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

- **FLOAT-TO-INT half still open (no ruling).** `1e300 as i32`: aarch64
  FCVTZS + interp saturate to i32::MAX; x86 CVTTSD2SI gives the 0x80000000
  "integer indefinite". Parked cast divergence stays in the drift ledger.

## Open bugs / gaps (ungated)

- **FS-LANE FOLLOW-THROUGH: texteq arm-locals still ZII for non-terminal
  consumers (found 2026-07-15 probing the just-closed leaf fix).** The
  fix's initializer collection rides Terminal-value arm expansions only; a
  sub-state whose transition targets ANOTHER sub-state emits no initializer
  write, so a texteq local read by the arm's own GUARD or forwarded as a
  transition ARG delivers ZII 0 natively (interp 70 / native 71 both
  shapes). Pinned: pending/calls/texteq_local_guard_read_divergence +
  texteq_local_arg_forward_divergence. ROOT-CAUSED 2026-07-17 (analysis
  in the guard-read pin's header): the dispatch GUARD evaluates before any
  expansion's write region, so per-expansion initializer collection can
  never feed its own guard (or an edge-taking argument) -- the fix is a
  per-branch-state PRE-GUARD region emitting call-free LocalData
  initializers, with 82a9a92d3's two load-bearing exclusions. Dispatch-
  layout surgery in the M2-active machinery; left for the owning lane with
  the analysis pre-paid. The pinned trailing-state &mut-param stale read
  (trailing_state_mut_param_phase_divergence) likely shares this region's
  absence.

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
  eventually.** NOW IN THE LANGUAGE DOCS (2026-07-15, owner-requested, NOT
  settled): chapter_23_dependent_types.md (UX surface, static + dynamic
  lowerings) + design_briefs/dependent_types.md (deep dive; systems fragment;
  lifetimes interplay; Lean path; implementation lab agenda §8 -- rung R3 =
  the ONE bounded-product entailment rule that discharges `y*W+x` and
  `i*stride`). Until it lands the linear-counter workaround stands; no
  axiom/octagon stopgap.

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
- **Wire data stage 2 remainder (list refreshed 2026-07-16 by survey):**
  nested/repeated fields and utf8-slice decode are DONE + pinned (the old
  "String decode" line was stale; runtime_wire_roundtrip_utf8_exit).
  OPEN, ranked: (1) **decode-side DOMAIN VALIDATION** -- INTERP HALF DONE
  (2026-07-16): ByteSequencePredicate lifted to
  omega-typed-trees::byte_predicates (one vocabulary for compile-time proof
  + runtime validation); the interp decode evaluates the slice's declared
  byte predicates over untrusted bytes, verdict Invalid on failure,
  unrecognized classifiers refuse loudly. AARCH64 DONE (2026-07-16):
  validation blocks emitted over the decoded content (mask-selected;
  non_empty 2 / no_nul 7 / ascii_only 8 instructions; utf8 = a
  77-instruction compare/branch walk assembled with a local label
  resolver), width twins + the target-page relocation offset in lockstep;
  edge classes pinned (runtime_wire_utf8_edge_verdicts_exit: overlong /
  surrogate / beyond-max / truncated INVALID, honest multi-byte SOUND).
  The decode-boundary domain-validation slice
  is CLOSED (2026-07-17): x86_64 emits the twin blocks (single-scratch
  lead-first dispatch, rel32 label resolver; widths measured from the pure
  emitter -- one source of truth); the refusal canary PROMOTED to
  pass/wire/runtime_wire_utf8_invalid_refused_exit with a linux_x64 ELF
  pin. NOTE for probe authors: multi-call VALUE MACHINES writing self
  fields trip the PINNED trailing-state phase bug -- inline states are the
  reliable shape. (2) wire-schemas-as-program-types --
  LANDED 2026-07-17 (a numbered data never entered the type namespace; the
  wire lowering now dual-registers a regular DataDefinition from the
  schema's current-era fields, so numbered datas are plain program types
  and the Message/Sample twin pattern is optional; pinned:
  runtime_wire_schema_as_value_type_exit) The wire-wide
  decode-then-let-compare fold is FIXED (2026-07-09): the wire encode/
  decode selection branches now clear RuntimeStaticValues like every other
  call shape (both walk sites); the pin promoted to
  pass/wire/runtime_wire_decode_let_compare_exit and the schema-as-value
  canary regained its decode-into-self roundtrip leg. Item (2) closes
  outright. (3) runtime layout of wire values. (4) encoding families
  beyond compact_binary v0 + version negotiation.
- **Versioned data stage 3:** the era tag itself (+ decision 10's wire-era
  ride), era-tagged containers, migration chains / `replaces` / quiescence.
- **Equatable synthesis:** a CALLABLE conformance surface is still open.
- **Trailing-state stale reads of threaded `&mut` param fields -- SKELETON
  FOUND (2026-07-16), pinned:**
  pending/calls/trailing_state_mut_param_phase_divergence (71, Exit(70)).
  Appending a sub-state that bumps-and-reads the param makes the FIRST
  state's guard read go stale natively; every single-state shape is green.
  Cross-state phase allocation for the threaded &mut -- the fs lane's
  claimed receiver-phase family; theirs to absorb with the aliasing arc.
  (This closes the old "signed/unsigned residue shape (2)" mystery line.)
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
