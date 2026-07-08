# Tasks

This is the working backlog, not a history dump. Keep it biased toward what we
should do next.

Omega's current north star: make core semantic concepts browsable and
proof-backed at the language level, while keeping unsafe/compiler/runtime
representation machinery behind a deliberate boundary.

## Current Strategic Focus

- Omega's first real consumer is the Cathedral OS (`../Cathedral`). The gap
  analysis between Cathedral's architectural bets and the language's current
  state lives in [wiki/cathedral_alignment.md](wiki/cathedral_alignment.md) —
  Tier 1 items there (ZII guarantee, wire data semantics, versioned data,
  separate-compilation awareness, concurrency/atomics decisions, freestanding
  target, enum payloads) should bias which vertical slices get picked next.
- Drive vertical slices instead of endless cleanup. Refactor when it unblocks a
  feature, clarifies semantic ownership, or adds a canary.
- Make capabilities/authority, proof-backed indexing/subslicing, ranking views,
  and core boundary primitives real end-to-end concepts.
- Keep the compiler pipeline organized around the semantic nouns it owns:
  places, values, facts, loans, moves, drops, calls, transitions, effects, and
  boundary edges.
- Keep `pass`, `fail`, and `pending` canaries honest. Do not let compile-only
  success imply runtime or proof support.

## KEYSTONE COMPLETENESS GAP CATALOG — CLEARED 2026-07-06

All seven de-Trapping-sweep items closed, PLUS the sweep's WALL: array-ELEMENT
ranges (`cells: [i32 [0..=7]; N]`) landed the same day — write-enforced (const +
runtime index), ZII-checked (0 must be in the element range), read-narrowed
(indexed reads carry the range into arithmetic; the cells[rp]-dataflow shape
proves). Detail in git history + memory [[decision-17-arithmetic-domains-s1]] /
[[guard-narrowing-keystone]]. Remaining prover conservatisms are by-design, each
with a loud reject + workaround: DIFFERING-but-compatible funnel guards on the
PROOF side accept only structural equivalence (validation's env join handles the
general case; make the checker join per-guard candidate ranges if a real program
needs it), and env facts are per-state (no interprocedural ranges).

- **[ ] Element-range de-boilerplate conversions (low priority).** Element
  ranges can replace re-guard states in converted samples
  (`cells: [i32 [0..=1]; 64]` in cellular_automaton drops its re-guard states);
  per-sample judgment. Accumulator samples (array_sum, dot_product, histogram
  tallies) legitimately KEEP Wrapping — an accumulator's bound needs
  iteration-count reasoning no element range supplies. The guarded element
  increment (`tallies[k] < 16` proving `tallies[k] += 1`) works for BOTH const
  and runtime indices as of 2026-07-06 (operand + guard-subject de-hoisting),
  so histogram-style tally code can convert where the guard idiom fits.

**Abort-as-effect (#65) design sketch (chat, NOT settled):** every trap-capable
site (`in Trapping`, future assert/panic) carries an `abort` effect threaded to
main / the target block (absence = denial = total program); Wrapping/Saturating
stay effect-free (total, visible in types). Needs a settle before building.

## Programmable-layouts remainder (ch19/20/21 rewrite)

The chapters are the spec (design_briefs/programmable_layouts.md, SETTLED).

- **L4 full — STILL OPEN:** derived projections into a plan-laid BYTE VIEW + the
  no-op boundary theorem (`&CLayout<T>` IS `&[u8] in CLayout<T>`) -- needs the
  carrier/domain rung (L5) to express the byte side.
- **L5-full REMAINDER:** target-directed `encode()` into a refined carrier
  (spelling OPEN -- extern brief section 10.2 builtins-vs-boundary-operators),
  the `Packed` grammar, the plan-walking deriver (blocked on the case-vocabulary
  Plan = array-of-struct element construction), the validate/materialize decode
  mint, and refinement-as-obligation (unrefined buffers still work today).
- **RECAST (settled, programmable_layouts §5b): borrows under a second stated
  shape, spelled `as`** (`&x as &f32`, `&mut gdt as &mut GdtRaw`, `&mut gdt as
  &mut [u8; N]`). Engineering: the borrow-recast form in the checker (borrows are
  same-type today) + the plan-tiling / fact-implication validator (same
  footprint; `&` = src⟹tgt facts, `&mut` = both directions; weaken-never-
  strengthen -- as-into-domain stays dead; untyped sources annotate first).
  Queued behind the validate-mint rung.
- **L6+: Bits placements + access classes (MMIO deriver); durability plan
  grades consumed by Store<T>-class APIs; publish-time predecessor diff.**

## Language ergonomics surfaced (mostly engineering; one research)

- **[ENGINEERING]** numeric intrinsics remainder: sin / cos are NOT one opcode
  (no single SSE instruction): they need range reduction (mod 2pi) + a
  minimax/Taylor polynomial whose precision matches the interpreter; a genuine
  numerical mini-project, not a quick lane.
- **[RESEARCH — sidesteppable]** nonlinear index `pixels[y*W+x]` is not provable
  in-bounds (the interval/ordering prover has no product-bound fact). Route around
  with a single linear `0..W*H` counter until/unless a `y<H && x<W => y*W+x<W*H`
  axiom or an octagon domain is added.

## Backend perf (deferred, post-1.0)

The MVP backend (fixed-register, memory-to-memory per op, no regalloc/SSA/
optimizer/SIMD) makes a *real-time* per-pixel renderer slow. Fine for small/simple
demos; a fast renderer waits on the deferred "serious backend" layer (virtual-reg
IR + a linear-scan allocator + a few passes + SIMD selection). Today's bar is
"provably correct native output," which it meets.

## Open latent bugs / fenced gaps

- **[ ] Straight-line LETS in a POST-ENTRY state of an inlined value callee misdeliver
  natively (2026-07-07, std::time `divide`; interp correct).** A guarded post-entry
  state computing lets (`seconds_quotient`, chained div/mod/mul) then threading them to
  an emit state produced a wrong subsecond natively; restructuring to ENTRY-ONLY lets
  (safe-divisor bump trick for the eager-eval zero case) fixed it. Same #2B
  splice-machinery family as the deferred-entry-locals fix — post-entry-state locals
  need the same deferral/contiguity treatment. The std authoring rule (entry-only
  lets, params-only states) dodges it; a minimal repro can be distilled from
  time.omg's divide at commit HEAD~1 if the fix lands here.
- **[x] NESTED-value-call transition guard read the nested result's PRE-STORE ZII
  TAG natively — FIXED 2026-07-08.** NOT a splice-ordering bug: a bare-call binding
  (`let since = self.checked_subtract(..)`) whose local ALSO has a LocalStorage
  slot minted a call-result slot carrying the SAME name, and the guarded arms'
  terminal writes (which rebuild the slot as a NAME expression and re-resolve it)
  landed in whichever slot the name matched FIRST — the arms wrote the LOCAL while
  the guard read the CALL-RESULT slot's ZII. Fix: the call-result slot takes the
  binding's name ONLY when the binding has no storage of its own
  (`call_result_slot_symbol_and_name` + `local_slot_exists` filter,
  omega-runtime-storage/src/body.rs); otherwise it gets the unique anonymous name.
  std time's `saturating_add`/`saturating_subtract`/`is_less_than`/
  `is_greater_than` non-ZII arms now deliver natively; `Instant::duration_since`'s
  single-level dodge reverted to the house nested idiom. Pinned by
  calls/runtime_nested_value_call_guard_exit (differential; every leg asserts a
  NON-ZII arm incl. an is_less_than designed-FALSE — the ZII-coinciding arms of
  runtime_duration_core_exit pass under both correct and buggy emission, the
  owner's "passing canary IS the bug" lesson).
- **[x] LOCAL-receiver value calls FENCED 2026-07-09 (was: silent ZII zero).**
  `let p: Pair = ..; p.total()` inside an INLINED value callee silently zeroed
  (receiver resolution reaches machine FIELDS and state PARAMETERS only);
  Main-state spellings hit the emission backstop. Now a clean validation error
  (omega-validation/src/calls.rs `report_local_receiver_value_call`, wired at
  both bound-value sinks; builtin view methods exempt -- same list as the
  nested-arg fence). PROBED SUPPORTED: `self`, field receivers, and by-value
  STATE-PARAMETER receivers (param probe green natively). Fail canary
  calls/local_receiver_value_call_rejected. The deep fix (resolve receivers to
  local slots) stays open with the receiver-offset work; time.omg
  elapsed_since keeps its single-level body until then.
- **[x] The `build_machine_wrong_arity` missing-files red is RESOLVED
  2026-07-09.** The suite's FAIL_CANARIES list referenced a canary whose files
  were never committed (2026-07-06, another thread) -- it panicked the fail
  suite BEFORE later entries, masking validation of every new fail canary
  since. Reconstructed from its name: a build machine with an extra parameter
  must produce the build-time arity error ("takes 2 argument(s); the
  build-time position supplied 1"). With it, canary_suite is FULLY GREEN
  (694/0) for the first time.
- **[ ] Folded-binary left operand loses the UNSIGNED marker: `(a / b) % c` on u64
  runs SIGNED idiv/modulo natively (2026-07-07, seen in Time::now lowering; the
  inner divide selected DivideUnsigned but the outer modulo stayed signed
  `Modulo`).** Semantically wrong only for msb-set u64 values (unreachable for real
  clock frequencies; interp honors the unsigned witness) — [[signedness-codegen-gap]]
  family. Fix = signedness resolution must consult the folded BINARY operand's
  declared type, not just direct storage operands.
- **[x] Cross-callee LET-NAME collision — BOTH flavors FIXED (2026-07-09).** Two
  different callees with same-named lets value-called from ONE caller state:
  - Mutation-fallback flavor: the substituted callee terminal resolved with the
    CALLER's key -> cross-source-key name ladder -> other callee's ZII local
    clobbered the first call's result. FIXED: the substitution returns the
    expansion's branch_key and the terminal resolves in the CALLEE's context
    (mutation.rs). Pinned: calls/runtime_cross_callee_let_names_exit.
  - Internal-op flavor: the NON-GUARD branch prelude re-emitted each callee's
    scalar local-initializer writes (the stale "the splice does not cover local
    initializers" rule) — WRONG-TIMED (before the splice's host calls) and with
    cross-callee resolution; a duplicated `x / freq` executed div-by-ZII-0 ->
    #DE. FIXED: `RuntimeBranchPreludeExpansion` carries the spawning call's
    ROLE, and the prelude's local-init writer skips plain scalar write kinds
    (integer/binary/convert) for non-guard roles — the splice is their executor;
    descriptor/indexed/copy prelude writes remain (no splice equivalent:
    fixed_vec's `as_mut_slice` construction + `cells[index]` reads live there).
    Pinned: calls/runtime_cross_callee_division_exit (the #DE shape) +
    time/runtime_time_elapsed_since_exit (std tripwire — elapsed_since shares
    let names with now() again; the stopwatch_* dodge is REMOVED).
- **[ ] Owner: REJECTED. NO-RECURSION directive ENFORCED 2026-07-10 (was: runtime_recursive_accumulator_exit).**
  Caused by THIS TASKS.md thread (the 2026-07-09 "recursive value machine" work -- my error:
  I read the pre-existing termination canaries' `self.countdown(..)` spelling as sanctioning
  recursion). NOW: `-> self.X(..)` targeting the machine's OWN ENTRY is a COMPILE ERROR
  ("call-spelled recursion ... write the state transition bare") -- fail canary
  calls/machine_self_call_recursion_rejected. The bare arm `-> X(..)` to the own entry
  remains: it is mechanically a self-transition LOOP (a jump with re-bound args, constant
  stack -- the dispatch back-edge; NO call frames). The offending pass canaries were
  REWRITTEN to the bare loop spelling (runtime_loop_{accumulator,rotation}_exit -- they lock
  the loop-carried-argument staging fix, which is loop machinery, not recursion) and the
  `self.` spelling was swept out of the WHOLE corpus (6 termination canaries, proofs
  canaries, std filesystem's mkall, the dungeon parser -- the spelling predated this thread
  and appears in several workstreams' code; no TASKS_{X}.md identified as the source).
  ⚠️ REVIEW ITEMS FOR ZACH: (1) is the BARE `-> own_entry(..)` loop-back acceptable, or must
  loops be spelled through explicit sub-states only? (2) MUTUAL value-call cycles (A calls B
  calls A - the dungeon's find_item_at/find_item_after pair) still compile; the state-call
  cycle check does not see value calls. If mutual cycles must also die, that needs the
  value-call cycle walk (bounded clone specialization currently absorbs them).
  (Owner amendment upstream, same commit window: "machine calls are stack based,
  but state transitions are not" -- exactly the enforced distinction.)
  machine Main::countdown(&mut self, remaining: usize) -> usize
terminates
decreases remaining
{
    transition remaining > 0 {
        true -> countdown(remaining)
        false -> 0
    }
}
^ Fix fucking failed, fuck you. this is a machine creating a cycle. No fucking cycles. Retard. Removing the `self` keyword doesn't change fuck-all, read what I wrote you sack of shit.

- **[ ] Interpreter unsigned-u64 arithmetic remainder (2026-07-07).** Comparisons now
  take an UNSIGNED witness from declared types (evaluator: Frame.unsigned64_locals +
  cast/self-field classification; found via std::time's wrapped-compare idiom breaking
  at u64::MAX interp-only). STILL SIGNED for msb-set u64: divide/modulo/shift-right and
  min/max (`eval_int_binary`/`eval_min_max` — need the same witness threaded).

- **[ ] Same-type receiver aliasing: VALUE-CALL flavor confirmed (2026-07-07, std::time
  authoring; repro canaries/pending/time/value_machine_receiver_field_postentry).** A
  pure-value method receiver (`self.sum.checked_subtract(...)` with several
  Duration-typed fields) resolves to the FIRST field of the type — the SAME root as the
  contained-machine aliasing entry above (machine_storage_offset by type). Std value
  types make same-type fields ubiquitous, so the deep fix (thread the receiver field
  offset through dispatch) is now HIGH-LEVERAGE for all std authoring. Workaround
  (canaried): route receivers through the first field of the type.
  ALSO CONFIRMED same session: the parallel write cascade silently DROPS a case-payload
  field whose value is `(x as T) % literal` (Binary with a Cast operand) — tag+siblings
  land, that field never writes (the known missing-arm landmine; bare `x % literal`
  works). NOTE for report readers: backend_report renders convert widths in BYTES
  (`as i8->i8` = an 8-byte u64 identity convert, NOT i8).

- **[ ] Range constraint + non-Exact domain = the range is a LIE (found 2026-07-06).**
  `i: usize [0..=4] in Wrapping` accepts `self.i = 100` -- the range enforces only under the
  EXACT domain ("Wrapping stays permissive" was scoped to source-type narrowing, but it also
  bypasses the DECLARED-range store check entirely). Consumers are correctly defensive (the
  index prover's declared-range feed is gated on Exact; canary
  collections/wrapping_range_index_unproven), but the DECLARATION itself is misleading.
  PROBLEM: what does a range constraint MEAN under Wrapping/Saturating -- wrap/clamp INTO the
  declared range at stores, or is the combination ill-formed (reject at declaration)?
  Same underspecified-numeric family as the shift/cast divergences
  ([[shift-amount-out-of-range-divergence]]).

- Contained-machine METHOD-CALL storage resolution is a SILENT miscompile with TWO faces
  (see memory `contained-machine-same-type-aliasing`), both from the backend resolving a
  method-call receiver's `self`-base by machine TYPE rather than the receiver field:
  (1) SAME-TYPE ALIASING — `a: Counter; b: Counter` + `self.b.increment()` mutates `self.a`
  (reconfirmed 2026-07-05: no-init `self.b.increment()×3` -> a==3, b==0); root =
  `machine_storage_offset` returns the FIRST field of the type. (2) DIRECT-WRITE-THEN-METHOD
  (found 2026-07-05, SINGLE instance, no aliasing) — a direct field write before a method
  call makes the method's mutation VANISH: `self.a.value = 5; self.a.increment();` reads
  back 5 (root unconfirmed; the method's self-base diverges from the direct-write location).
  ⚠️ (2) MASKS (1): inits like `self.a.value=0` in a repro hide the aliasing. DIRECT field
  access (no method call) works; sound workaround (distinct types / direct-field ops) locked
  by `calls/runtime_same_type_contained_direct_fields_exit`. Real fix = thread the receiver
  field offset through dispatch (deep). A precise frontend fence for (1) rejects
  currently-compiling code -> needs a Zach decision, not landed unilaterally.
- u64 literals above i64::MAX rejected at parse (`literals.rs`) — **CLAIMED by the
  std::time workstream** (TASKS_TIME.md D14: ANONYMOUS literals — payload-until-use,
  no numeric carrier at all; also absorbs the type-blind const-fold sign-miscompile
  class, since a context-less folder defers instead of folding); const float arith
  in a guard refused (clean error); a tail of value-call corner cases.
- **`::` for static name paths, `.` for value access** (ch14 updated). `use
  a::b::C` and `module a::b` (not `.`); `::` is already used for type-scoped
  machines (`Main::run`), now uniform for all compile-time name resolution;
  `.` is reserved for runtime field/method access. Migrate `use x.Y` /
  `mod a.b` / `pkg.Type` → `::` across canaries, samples, and stdlib.
- **Drop the per-file `package X` line** (ch14 updated). Package identity lives
  in `build.omg` / the directory (one dir = one package = one build.omg); source
  files are members by location and don't re-declare it. Remove the `package X`
  header from source files; the parser stops requiring/accepting it.

## TASK — invariants are the default domain; NO default values (settled 2026-07-05, Zach)

FULL detail in memory `default-domain-invariants`. Design-only; nothing built. Extends
ch7's existing "no invariant syntax on types -> contracts/domains" direction. THESIS:
`data` = layout only; ALL invariants live in a data type's always-travelling DEFAULT
DOMAIN; there are NO default VALUES on data.
- **Default domain = the domain always in scope for a data type.** Not a special construct
  — just *the* domain that travels with the data everywhere, so nothing to shed/track. A
  field constraint (`health: i32 [0..=100]`) is SUGAR for a per-field invariant of it.
  "Top-level" domains (`Player::New`, `Quantity::Additive`) are SUBDOMAINS refining it.
- **Single-field invariants = STANDING**, maintained by the existing store-check machinery
  (decision-17 / narrowing / cross-class — REUSED as the default-domain enforcer, not
  deleted). **Cross-field invariants** (`start <= end`) live in the default domain but are
  reachable ONLY via INIT-SYNTAX (construct-a-valid-whole, move in atomically) or a `relax`
  scope (suspend, re-prove at exit). ENFORCEMENT: a bare single-field store that would break
  a cross-field invariant is REJECTED, forcing init-syntax or relax.
- **No default VALUES.** KILL `field: T = default`. ZII is the substrate; construction forces
  you through the default domain -> override exactly the fields whose ZERO is invalid
  (Odin/Go partial-literal, invariant-gated: `Player{health = 50}` -> age ZII, health
  mandatory). The old "ZII default must conform" rule BECOMES the construction semantics; the
  half-broken array/aggregate defaults (silently dropped — see the "ARRAY field DEFAULTS"
  latent bug) DISAPPEAR. Non-zero convenience defaults -> explicit constructor machines only.
- **Constructors are not new** — a machine `-> T in <domain>` that discharges the invariants.
- **SOUNDNESS HINGE = `relax` EXCLUSIVITY**: while relaxed, nothing may observe the value as
  still-in-domain (no alias/call/read assuming the invariant) until scope-exit re-proves it.
  This is the unbuilt part of relax (the "Relax semantics follow-up" TASK). Watch hardest.
- Migration: every `field: T [range]` / `field: T = default` -> default-domain constraint +
  construction. Shares the invariant-prover dependency with the encoding-domains work.
- [ ] To pin at implementation: the surface for declaring the default domain's invariants;
  init-syntax for reconstructing `self`'s own cross-field-related fields.

## TASK — explicit case discriminants (settled 2026-07-04, Zach; ch1 updated)

A payload-less `case` may pin its tag to a specific integer (`case
ConventionalMemory = 7`) — required for foreign-ABI enums whose tag values are
fixed by a spec (UEFI EFI_MEMORY_TYPE, device/protocol enums). Unspecified cases
number sequentially from the previous (0-based default), C-style; mixing
specified/unspecified is allowed; duplicate discriminants are a compile error.
The discriminant is the on-wire/in-memory tag under a layout policy, so a foreign
enum reads back into the right case. Internal sums leave them off (tag identity
stays the compiler's). Milestone-2 driver: `EfiMemoryType` in the memory-map
walk.

## TASK — const + the static root (settled 2026-07-04, Zach; brief: static_root_and_constants.md, ch1 updated)

> **const-v0 CLAIMED by the std::time workstream (2026-07-06)** — the `const`
> declaration with LITERAL-ONLY initializers (scalars + struct-literals-of-
> literals, free or `Type::`-scoped, pure-value check); see TASKS_TIME.md D15 /
> rung 2. The full build-time-evaluation arc (machines in const position) stays
> here.

The three tangled holes (where const lives, where static lives, why main's &self
looked like a hack) resolve into two:

- **`const`** — a named compile-time PURE VALUE. Free-floating (package/module-
  namespaced) by default, or `Type::`-scoped like a machine when it belongs to a
  type; NEVER a `data` member, so excluded from `sizeof` by construction (Rust's
  impl-const separation via the `::` rule). Build-time-evaluated. **Pure-value
  restriction:** no cleanup obligation, no shared ownership, no interior
  mutability (checked from the ch16 cleanup facts) — copied freely, trivially
  borrowable, thread-safe; forbids Rust's interior-mut-in-const footgun. Not
  authority → no capability concern. **Implement:** the `const` declaration +
  the pure-value check.
- **static** — NO `static` keyword, NO free-floating mutable static. Persistent
  mutable state is `main`'s `&self` subtree, reached only by borrowing DOWN
  (threaded as params) — the capability model at the storage layer. This makes
  borrow-check over static LOCAL (no global name to grab) and thread-safety
  ORDINARY (Send/Share over the subtree, not a bespoke static analysis). `main`'s
  `&self` is the single static root the entry establishes before `main` runs —
  document it as the bootstrap allocation, not magic. **Nothing to add** (it is
  the absence of a feature); the entry-model doc names the root allocation.

Cathedral's free-floating constants become `const`; EFI_MEMORY_TYPE tags stay
named `const` u32s (robust to unknown firmware kinds; a full EfiMemoryType sum
via case discriminants is the typed alternative if wanted).

## TASK — foreign vtable dispatch: the FIELD MODEL (decided 2026-07-04, Zach)

A `provides` binding names *which* function pointer to call in a foreign table
(UEFI BootServices/protocols, COM vtable) by binding the trait method to a
**named fn-ptr FIELD of a declared table `data` struct** — not a magic slot
index. `VtableSlot(index)` is retired. Rationale: no magic numbers, header
handling is free (a header is just leading fields), and the FFI audit surface
reads by NAME instead of by count. (Security unchanged — still the same
boundary-gated `provides` dispatch; this was always spelling only.)

Proposed spelling (refine while implementing):
```
data TextOutputProtocol { reset: addr; output_string: addr; }   // fn-ptr fields
boundary trait SimpleTextOutput {
    machine output_string(console: &TextOutputProtocol, string: &[u16]) -> EfiStatus;
}
uefi_x64 provides SimpleTextOutput over TextOutputProtocol {
    output_string -> output_string      // bind the method to the fn-ptr FIELD
}
```
The table is a plain `data` struct (header + fn-ptr `addr` fields, in spec
order); the layout policy computes each field's offset; dispatch lowers to
"deref the object as `&Table`, read the fn-ptr field at its plan offset, call it
at the edge's calling plan." This **subsumes the header-offset case** (M2 ask #2)
with no special variant — `BootServices`' fields simply start after the
`EFI_TABLE_HEADER` fields.

Scope: implement `provides Trait over Struct { method -> field }`; retire
`VtableSlot(index)`. Milestone-1 (con_out) and milestone-2 (BootServices) both
move to it. The Cathedral source is being updated to the field model in
parallel (declares the table structs + field bindings), so it is the target.
Cost is a one-time transcription of each table struct's fields in spec order
(BootServices ≈ 27 to reach ExitBootServices) — the auditability the model buys.

## TASK — encodings are library code, not compiler intrinsics (settled 2026-07-05, Zach)

Foundational reframe of #66/#21/#22. FULL detail in memory `encoding-domains-no-intrinsics`.
Design-only; nothing built. THESIS: the compiler has ZERO encoding intrinsics — Utf8 is
no more special than Shift-JIS/Ascii/UTF-16; encodings are ordinary library domains
(plausibly `core`). Utf8 is one domain among peers. Litmus: delete every encoding from
`core` → the compiler must still lex/parse.
- **Compiler's ONLY string privilege = quoted-text → bytes.** COPY source bytes; never
  synthesize/interpret. A string literal is raw `&[u8]`, NO domain. Byte-escapes only
  (`\n \r \t \0 \\ \" \xNN`; `\\`/`\"` are irreducibly parse-time). NO `\u{}` in the front
  end (codepoint→bytes is the leak; make it a `core` comptime helper). Source must be
  ASCII-transparent (UTF-8's real, minimal "special relationship" = an input-format
  precondition, not value semantics). FORBID raw newlines in `"..."` (determinism);
  join lines with `"a" + "b"` (comptime-folded concat); multiline later = explicit LF-spec'd.
- **Domains = recursive predicates.** `domain <carrier>::<Name> { <invariants> }`, invariants
  ONLY. KILL the `when` keyword (currently `... when valid_utf8(self)`, ~dozens of sites).
  Operators = ordinary machines `machine <carrier>::<Name>::<method>`. Two tiers: Tier-1
  local per-element (Ascii/NoNul, prover-native) vs Tier-2 recursive/fold (Utf8 =
  `valid(b)=empty ∨ (well-formed 1-4B codepoint ∧ valid(rest))`). The invariant language
  MUST admit recursive/fold predicates (+ `decreases`); the `valid_utf8` intrinsic exists
  ONLY to paper over that missing expressiveness.
- **`as` is the whole mint.** `bytes as [u8] in Utf8` compiles iff invariants provably hold.
  LITERAL → CHECK (decide the predicate over known bytes at comptime, Lean `decide` role).
  RUNTIME → inductive proof (a validator's byte-walk LOOP is the induction; loop-invariant
  discharge, Dafny model; no stored `is_utf8` bool = that would be RTTI + forgeable).
- **#22 RECAST:** the "derived core `Schema::validate -> Valid|Invalid`" is REJECTED. Runtime
  cased validation is USER code (user cased data + checking loop + `as`). Only compiler
  mint = `as`. **#21:** `as` stays the sole mint construct.
- **Forgery guard:** an empty invariant is VACUOUSLY satisfied → free `as` → forgery. A mint
  target may NOT have a vacuous invariant. Empty domains stay legit as BRAND tokens guarded
  by constructor VISIBILITY (not for Utf8). Trusted base = the recursive predicate's `core`
  definition (predicate IS the spec; accept = definitional membership) — tiny/audited, like
  Rust `from_utf8_unchecked` forced into ONE place. Any UNCHECKED mint = a conspicuous
  distinct construct, NEVER `as`.
- **THE REAL COST = a Dafny-style prover engine** (loop-invariant / inductive-predicate
  discharge for the runtime case) — parallel-thread-sized, beyond current interval +
  guard-narrowing. Copy Dafny (recursive `predicate`+`decreases`; `while`+loop-invariant;
  Boogie→Z3), Lean the other precedent (`decide`/`native_decide` for literals). Prerequisite
  under all of it: comptime-eval-in-value-position (the literal `decide` path).
- **Demolition (after the engine):** rip out `ByteSequencePredicate` + the blessed-predicate
  grant path; move encodings to `core` as recursive-predicate domains; rewrite every
  `domain ... when valid_utf8(self)` site (~dozens + dungeon); re-green corpus.
- [ ] **TASK — KILL builtin `string` / `String` (Zach, 2026-07-05: "how is this not
  retired yet").** The `PrimitiveType::String` builtin + the `string` unsized view must go;
  text is `[u8] in <encoding domain>`, never a nominal type. This is #66's endgame. Blocked
  on the mint being real: `<literal|bytes> as [u8] in Utf8` needs (a) comptime-eval in
  value/refinement position and (b) the loop-invariant/inductive prover for the runtime case
  (both above). Once minting works: sweep ~185 files + ~57 canaries + the dungeon off
  `string`/`String` onto `[u8] in Utf8`, delete `PrimitiveType::String` and its ~16 backend
  special-cases, retire the `string` keyword. Recipe: wiki/architecture/string_retirement_execution.md.
  NOT a background-tick item — it's the capstone of the whole encoding-domains arc.
- **Invariant spelling (settled 2026-07-05):** NO `predicate` or `invariant` keyword —
  the domain block body IS the boolean expression (`domain P::Vulnerable { self.health >= 1
  && !self.in_cutscene }`). A "predicate" is just a comptime-eligible (pure+total) machine
  returning bool (reuse the machine substrate; array accesses ride the existing bounds
  prover via `&&` short-circuit). RECURSION IS BANNED, so a sequence walk is a STATE MACHINE
  transitioning to itself with a NARROWED SLICE (slicing over indexing; no `usize`). See the
  canonical `utf8_ok` recognizer in memory encoding-domains-no-intrinsics. Also: `when`
  CLASSIFIER KEYWORD KILLED → SUB-DOMAINS (`Type::A::B` auto-includes A's facts; matching
  tests parent-first, so cheap-parent = tag-switch for free); horizontal fact reuse = a named
  bool machine (no `predicate` binder). ch8 "Sub-Domains" section rewritten.
- **State param scope (settled 2026-07-05):** THREADED — pass what each state needs, NO
  whole-machine/global param access (keeps the borrow checker transition-local; `&mut`
  splitting is a linearity issue). Global access = a possible follow-up, compatible via
  SHADOWING. CONSEQUENCE: flips the bare-name scoping — the whole-machine-scope allowance +
  `bare_name_scopes` canary shipped this session become WRONG; the reverted per-state check
  was correct. Undo when threaded scope lands.
- **Still open (design):** `as` referencing the invariant machine + the mint-obligation
  spelling at the call site.

## Outstanding (pick up next)

> The Cathedral OS work (calling-plan lowering as stated policies, hardware facts,
> milestone 2 = GetMemoryMap/ExitBootServices/first Region mint) is a SEPARATE
> agent's track over this same machinery — coordinate before touching the
> ABI/boundary layer. The compiler-language backlog, by leverage:
>
> **Soundness (correctness-first):**
> - **#37 guard-subject deref through entry ref-params — CLOSED (verified
>   2026-07-07).** The ref-param-member HOIST (statement.rs
>   is_reference_struct_parameter_member, boundary machines only) already
>   materializes direct guard subjects / machine-target assignments /
>   transition args through the boot-verified pointee path — pinned by
>   targets/efi_ref_param_direct_faces (the dedicated suite test asserts
>   pointee derefs in the report, no flat slot+field read). The NON-boundary
>   flavor is correct by construction (alias slots share the caller's
>   storage, flat fold reads the right value) — live-verified native==interp
>   and pinned by references/runtime_shared_ref_param_guard_exit. No
>   StateGuardOperandStorage::Pointee slice needed.
> - **Nested/indexed access — CLOSED on x86_64 (2026-07-05..07; detail in
>   memory nested-runtime-indexed-write-gap + computed-index-value-operand-gap
>   + array-of-structs-indexing).** Everything lowers value-validated
>   native==interp: const/runtime index mixes at any level, both-runtime
>   `grid[i][j]` reads/writes/operands (the double-indexed op family),
>   member-between (`rows[i].data[j]`) and member-suffix (`boards[i][j].x`)
>   faces, computed indices (`arr[k+1]` auto-hoist + guard-fact bridging),
>   array-of-structs operand matrix, transition-arg operands (+ the RUN-SPLICE:
>   hoisted lets inside a dispatch run must precede the run head -- arm
>   adjacency is load-bearing). Canaries pin every face; matrix_multiply
>   sample (exit 189) exercises the family natively. STILL OPEN, all loud:
>   - direct RMW: LANDED 2026-07-07 (WriteRuntimeMachineDoubleIndexedBinary
>     per the filed recipe) + TWO bugs it surfaced: (a) STALE-FOLD
>     invalidation -- the prefix key embedded the runtime index
>     ("grid[self.i]" never prefixes "grid[1][2]"), so const folds survived
>     nested runtime writes; nested_place_key now stops BELOW the outermost
>     runtime level (static_values.rs, both tree/table flavors); (b) the
>     hoist-temp typing walks interleaved Indexed/Member chains (the
>     rows[i].data[j] RMW temp was Unit). Canary
>     runtime_double_indexed_rmw_exit.
>   - local/param 2D arrays: READS LANDED 2026-07-07 for the let-consumer
>     faces (CopyRuntimeFrameBaseDoubleIndexedToRuntimeStorage + the
>     COLLECTION NO-FOLD: aggregate-literal bindings must not fold into
>     indexed collection positions -- simplify_collection_expression; canary
>     runtime_frame_double_indexed_read_exit). STILL LOUD: the
>     machine-field-target face: CLOSED 2026-07-08 (both coupled causes per
>     the FB3 diagnosis: the aggregate carve-out now keeps slots for
>     assignment-value consumers, and the selection-layer alias substitution
>     refuses aggregate literals in indexed COLLECTION positions -- the
>     third fold layer gets the same guard as simplify). Canary extended.
>     STILL LOUD: frame-2D writes + member faces (same recipe when needed).
>   - 3+ runtime index levels; aarch64 double-indexed encoders (clean
>     rejections).
>   - u64 literals > i64::MAX (i128 refactor; fenced clean).
>   One-fence-per-fire.
>
> **Mint arc remainder (library-grade; the boot path used the boundary-vouch
> shortcut):**
> - **#22 validate-mint — RECAST 2026-07-05 (Zach; see "encodings are library
>   code" section above + memory encoding-domains-no-intrinsics).** The
>   "derived core `Schema::validate -> Valid|Invalid`" is REJECTED: runtime cased
>   validation is USER code (user cased data + a checking loop + `as`), NOT a core
>   or compiler construct. The only compiler mint is `as`.
> - **#21 recast** — `as` is the sole mint construct; literal→check (comptime
>   decide), runtime→loop-invariant/inductive discharge. Gated on the Dafny-style
>   prover engine + comptime-eval-in-value-position.
> - **Rung-2 finish** — std-source the `CompactBinary` policy; retire the Rust
>   agreement walk once the policy is the sole author.
>
> **Big structural unlocks (multi-fire, design settled):**
> - **Generics runtime boundary** — per-instance monomorphization. Highest
>   single leverage (unblocks containers, Store<T>, Grammar conformances). Zach
>   settled 2026-07-02 (per-instance mono; NO unification; instances always
>   spelled). Recon map 2026-07-04 (agent a87aee3d) -- PHASED PLAN below.
> - **String/encoding #66** — retire builtin `string`/`String` (~185-file
>   migration, ~57 canaries; recipe in string_retirement_execution.md; worktree
>   big-bang).
> - **`usize` retirement** — design-dead (count/addr model settled); impl queued.
>
> **Ergonomics / completions:** sin/cos (numerical mini-project, must match
> interp); layouts-ladder remainder (mint rung, Packed grammar, layout
> plan-walking deriver).
>
> **value-CALL-in-guard (#40) — CLOSED for the integer-comparison face
> (2026-07-07; re-verified live 2026-07-07 with fresh probes: `transition
> self.dbl(5) == 10` and `== 11` both DISCRIMINATE, native==interp).** The
> frontend hoist (hoist_scalar_value_call_comparison) materializes the call
> into a shared let temp; non-exhaustive dispatch is a compile error. Remaining
> open flavors (clean/loud, per memory runtime-conditional-value-primitive):
> enum method matching bare `self`; nested value-call ARG (bind to a local).
> Memory value-call-in-guard-always-true has the full position matrix.
>
> **GENERICS / MONOMORPHIZATION -- phased plan (recon a87aee3d, 2026-07-04).**
> Today: type-check-only. Stage-1 monomorphization (typed-trees-to-checked-trees/
> monomorphization.rs) infers args at return/param position + substitutes IN
> PLACE; the LAYOUT builder (omega-layout/builder.rs ~760) keys per-DEFINITION
> and POISONS on a 2nd distinct instantiation. Sizes are computed correctly
> per-use; only per-instance IDENTITY is missing. The fence:
> fence_generic_value_callee (validation/calls.rs:801). Discovery PRECEDENT to
> mirror: plan_laid.rs. No new arena types needed -- follow plan_laid's slug-named
> synthetic instances.
>   - **Phase 1 -- generic DATA, scalar T -- DONE (75c445a49).** REMAINING Phase-1:
>     composite ARGS (`Box<[i32;4]>`, `Box<&T>`, range-bounded) still fall through.
>   - **Phase 3 -- NESTED generic data -- DONE 2026-07-03.**
>   - **Phase 2 -- generic MACHINES (EXECUTION PLAN, recon 2026-07-08):** two
>     slices, both at the PRE-RESOLUTION syntax layer following
>     generic_instances.rs (Phase 1/3's home -- synthesize + substitute +
>     slug-rewrite; copy_machine/copy_state/copy_type_reference exist in
>     syntax_trees.rs).
>     SLICE 1 -- CONTAINER METHODS: LANDED 2026-07-08. The desugar clones
>     each BODIED attached machine per data instance (snapshot copy_item_from
>     + type-reference WATERMARK substitution of Named(T); the clone's
>     full-path NAME + attached_data rewrite to the synthetic record --
>     machine identity keys on the composed name). Declaration-only container
>     surfaces (stdlib Vec<T>, empty bodies) stay type-check-only (cloning
>     them tripped the returns-but-empty check); generic TEMPLATE machines
>     are skipped by the LAYOUT builder (they have no layout; their calls
>     stay behind the validation fence). Canary
>     generics/runtime_container_method_instances_exit (Box<i32> + Box<bool>
>     coexisting, stored() value-validated both engines -- the runtime
>     silent-0 is dead). HARDENED 2026-07-08 (canary
>     runtime_container_setter_matrix_exit): T-typed setters (&mut self),
>     non-generic methods, cross-call state; fixes: the mentions-parameter
>     walker recurses Constrained/FixedArray/Slice/Reference shells precisely
>     (a parameter-free `in Wrapping` field refused the whole container --
>     also WIDENS method-less Phase 1 to constrained fields), and the layout
>     skip covers machines attached to template data that declare no type
>     params of their own (Cell::touch_count).
>     PROBED LIMITS (2026-07-08, both LOUD -- the existence check catches
>     them; ergonomics features, not soundness holes):
>     (a) NESTED member receivers (`self.p.second.stored()`) do not resolve
>     at ANY genericity -- concrete `PairI/BoxI` fails identically. RECON
>     2026-07-08: the SYMBOL resolution already recurses Member chains
>     (receivers.rs); the real limit is the CALL-PLAN layer -- the state-call
>     plan's receiver is BARE-NAME-keyed (`machine_symbols.contained_type(
>     receiver_name)` in validation mirrors it; cf. the contained-machine
>     receiver_name threading). The feature = thread a receiver PLACE (member
>     chain -> storage offset) through the plan to the callee's self storage,
>     THEN extend receiver_declared_type_name to walk field types. Do NOT
>     relax the validation check first -- it guards exactly the lowering
>     limit (un-gating = silent 0). Workaround: a forwarding method on the
>     outer type.
>     (b) a generic TEMPLATE method calling a method on a Generic-typed
>     field (`Pair::first_stored<T>` calling `self.first.stored()` with
>     `first: Box<T>`) -- the template's receiver type is a Generic node the
>     resolution does not see through; the CLONES would resolve fine if the
>     template validated only as a template. Both faces block the
>     nested-container-with-methods composition (probe gc3).
>     SLICE 2 -- FREE generic machines at MULTIPLE instantiations
>     (`Main::id<T>` at i32 AND bool): needs typed-layer DISCOVERY (extend
>     stage-1's return/param inference to collect per-call-site signatures
>     instead of conflict-flagging) feeding a synthesis pass -- either
>     re-run-the-frontend with syntax clones (phase-ordering problem) or
>     typed-layer deep clone. DEFER until slice 1 proves the substitution
>     copy; single-instantiation free machines already work via stage-1
>     in-place substitution, and multi-instantiation stays cleanly fenced.
>   - **CONTAINERS (generic data + attached method) -- valid-but-unimplemented;
>     silent-0 at RUNTIME.** The CORRECT method syntax is T-on-METHOD:
>     `machine Box::stored<T>(&self)->T`. With it, `Box::stored<T>` used as
>     `Box<i32>` + `self.b.stored()` compiles and RUNS but returns ZERO -- the DATA
>     monomorphizes but the METHOD stays a generic machine whose T-typed value-call
>     result is never materialized (the #40 class). A desugar-level "reject all
>     container instantiations" is TOO BROAD (containers are used TYPE-CHECK-ONLY
>     today; a fence keyed on data_with_machines was built + reverted TWICE). The
>     narrow #40 fence (if wanted) is at the VALUE-CALL/codegen. REAL FIX = Phase 2:
>     clone the attached machine with T substituted when synthesizing the data
>     instance -- pre-resolution-tractable via a substitution-aware extension of
>     syntax_trees.rs copy_machine/copy_state/copy_type_reference.
>   - **Phase 4 -- generic trait conformances / containers**: `Store<T> satisfies
>     Container<T>`. DESIGN QUESTION (Zach, when reached, far off): generic-trait
>     dispatch = static specialization (one stamped impl per instance) vs a
>     vtable. Phases 1-3 need no such decision.
>   - **Phase 5 -- const type params** (`Vec<T, N: u32>`): extend the
>     FixedArrayLength::ConstParameter machinery to data/machine params.
>   Implementation choices (mine, not design): synthetic-name slug scheme
>   (follow plan_laid); instance identity = synthetic SymbolHandle (no new key
>   struct).

<details><summary>Historical snapshot (2026-06-19/22 wave — kept for provenance; open items only)</summary>

Long-view sign-offs still open (only the maintainer): S1-S6 (separate
compilation -- the big backend revamp, untouched), M1-M6 beyond build-time evaluation
stage 1, A1-A5 beyond allocator stage 1. The next major VERTICAL SLICE is
CONCURRENCY (decisions C1-C5 frozen, briefs/concurrency_atomics.md; the atomics
foundation is now done) — gated on the ch15 error model for cancellation.
The long-range proof-engine direction (obsolete SPARK near-term, Lean long-term;
automation-front-line + trusted-kernel backstop + quantifiers) is its own brief:
[wiki/design_briefs/proof_engine_north_star.md](wiki/design_briefs/proof_engine_north_star.md).

**Open remaining work:**

- **`abort` effect (ch15 stage 3, #65) — ch16-gated.** The contagious capability
  already exists as `process_exit`. The only-new-part — a nuclear no-cleanup abort
  distinct from graceful exit — is meaningless until drops (ch16) exist. Revisit
  with ch16.
- **S4 arithmetic-domain narrowing (refinement, not a correctness gap).** ~30
  corpus ops are pinned to `Wrapping` ONLY because the prover can't yet narrow
  their operand ranges; flow-sensitive narrowing (dominating guards, loop bounds,
  contracts, range types) would return them to Exact. SOUNDNESS-CRITICAL: every
  narrowing fact must be enforced at its source, never trusted. aarch64 Sat/Trap is
  already at x86 parity. This is automation-engine work — see the proof-engine
  north star.
- **Recoverable-error / failure model (ch15) — DESIGN SETTLED (decision 18);
  implementation pending.** Implementation arc, by leverage: (1) **facts on sum
  cases** — `ensures Case.field in <range>` parsed + carried into the handling arm
  by the existing decision-17 narrowing engine; (2) **modular contract inference**
  (infer `ensures` for non-exported machines; require at boundaries); (3) **`abort`
  effect** — declare + propagate through callers/boundaries, lower to
  `exit`/`abort` syscall. Unblocks clean concurrency cancellation.
- **Versioned decision 14 maintainer reconciliation:** update decision 14's
  frozen text + versioning.rs provenance to the wire-data role once ch21
  settles (chapter is the authority; it is being actively edited).

**Implementation, design already frozen:**

- [ ] **Lifetimes (decision 15).** New implementation arc: `'name` lifetime
  parameters in the `<>` generic list (lexer tick token, parser, all three
  tree representations), elision rules (one ref input → output borrows it;
  `&self` → self), borrow-checker linkage (returned view extends the named
  input's loan), borrow-carrying `data` declarations. Staging suggestion:
  elision-only first (no user-visible ticks; fixes the conservative
  all-args aliasing), then explicit parameters, then struct borrows.
  Unlocks zero-copy String decode + view-returning machines.
- [ ] **Ranking-view spelling (decision 2 above).** Build
  `decreases (index, limit) -> Nat::BoundedDistance`; retire the use-site
  subtraction form once landed. Grammar scope in the Measures bullet.
- [ ] **Wire stage 2: encoders + decoders.**
  Remaining: historical-era decode via `Versioned<T>` (after the stage 3
  sign-off), String decode (borrow-facts follow-up, mechanical once lifetimes are
  implemented: read len varint, bounds-check against the remaining buffer, store
  `{buffer_base + cursor, len}`), arbitrary-depth nesting (needs per-level staging
  regions), repeated fields, wire-schemas-as-program-types, runtime layout of wire
  values, encoding families beyond compact_binary v0, version negotiation. Encode
  also has no runtime overflow signal (content past capacity is dropped; callers
  size buffers for their longest text) -- an encode ok/overflow out-parameter is
  candidate follow-up work.
- [ ] **Versioned data stage 3.** Era tag + the wire integration decision 10
  assumes; era-tagged containers that make version MATCH arms selectable
  (stage 2 ruled them unreachable — no value can hold a historical era yet);
  migration chains, `replaces`, quiescence obligations. DESIGN SIGNED OFF
  2026-06-12 (frozen decision 14). Stage 3b (no new surface, dispatchable
  independently): migration-chain completeness validation along the declared
  version chain. `replaces`/quiescence stay deferred behind the concurrency model.
- [ ] **Equatable synthesis / conformance defaults.** STILL OPEN: a CALLABLE
  synthesized `Type::equals` machine (build-time evaluation/trait-generator arc), trait
  `default machine` instantiation for other traits, recursive Equatable
  support, String-vs-literal structural compares, equality in
  contracts/domain facts (no typing scope there), and written-equals
  signature matching against `&Self` (validation accepts `Self` in trait
  signatures; substitution per conformance is unchecked).

**Backend residue (small, known):**

- [ ] Signed/unsigned residue, sibling shape (2) only -- shape (1) is DONE
  (checkbox scope corrected 2026-07-04).
  (2) Trailing-state STALE READS of threaded `&mut` param fields:
  a transition-guard SUBJECT read of `random.calls` in a state appended
  after build_main_hall_1 saw the post-seed snapshot (0), and a `let hi =
  (random.seed >> 32) as u32` in a state appended after build_main_hall_4
  read a seed stale by the last TWO build_segment calls — instrumentation-
  only so far, but the same one-shrink-away family; needs its own minimal
  skeleton hunt.

**Long view (deliberately deferred — big designs or revamps; listed so they
stay visible, not because they're next):**

- [ ] **Concurrency model.** Chapter 17 is a sketch; every target declares
  `threads = disabled`, zero canaries. Needs the hard answers first:
  scheduler suspension across ticks, cancellation/deadline propagation,
  ownership-vs-scheduler interaction. Gates Cathedral's scheduler chapter.
- [ ] **Atomics + memory model.** Absent entirely. Shape decision (intrinsics
  vs boundary operators vs core library) + which orderings. Gates IPC rings,
  `spawn`, SMP anything.
- [ ] **Separate compilation / component artifact model.** Whole-program
  compiler, one image, absolute frame offsets, fused dispatch loop —
  Cathedral wants independently compiled/signed/hot-swapped components.
  Full backend revamp; meanwhile, codegen decisions keep deepening the
  whole-program assumption (see wiki/architecture/whole_program_assumptions.md
  for which layers are ALLOWED to assume it).
- [ ] **Freestanding target + hardware vocabulary.** No-host-bindings target,
  custom entry, linker/section/physical-address control, volatile/MMIO
  semantics, inline asm beyond `asm { jmp state(...) }` (CR3/MSR/port-IO
  contracts). **Concrete near-term driver: the Cathedral first-boot ladder** —
  see the "Cathedral first-boot ladder" and "MILESTONE-2 ladder" sections above.
- [ ] **Build-time evaluation (const eval + trait generators).** Effect-free machines in
  constant positions; `default machine` bodies with `Self::fields` member
  reflection expanded per conformance. Direction frozen (no macros, no #run);
  implementation is a large interpreter+expansion arc. Equatable/Hashable
  synthesis becomes ordinary once this lands.
- [ ] **Generics completion.** STAGE-1 DATA MONOMORPHIZATION LANDED (2026-07-01).
  STAGE-1 BOUNDARIES: (a) a SECOND different instantiation of the same generic
  data poisons the recorded offsets -- both instances still SIZE correctly and the
  program compiles, but native field access through the colliding type rejects
  cleanly (fail canary `generic_second_instantiation_access_rejected`); real
  per-instance identity needs instance keys threaded through type descriptors.
  (b) A generic ENUM payload (`Maybe<T>::Some(value: T)`) still rejects: the
  DESTRUCTURED BINDING's frame slot is sized from the unsubstituted variant
  field type in compute_machine_layout (the data-side layout is ready; the
  machine/frame side needs dispatch-site bindings). (c) A VALUE-position call
  to a generic machine is FENCED with a clean error in omega-validation/calls.rs
  `fence_generic_value_callee`. Statement calls to generic machines still work.
  REMAINING: (b)'s frame-side bindings, real machine-call monomorphization (unfence
  value calls by materializing the result slot), per-instance identity for (a),
  const-parameter substitution, layout for symbolic lengths. Decision-13 bounds are
  checked on type-reference instantiations; extend the check to machine-call
  monomorphization arguments when those land.
- [ ] **Allocator story.** `Vec` has no runtime; `alloc` is an effect name
  only. Decide explicit allocator/arena capabilities vs ambient heap BEFORE
  implementing Vec lowering.
- [ ] **Repr control for hardware structures.** packed, explicit
  offsets/alignment, untagged unions (page tables, descriptor tables, device
  registers). Chapter 19 has `repr native` only.
- [ ] **Proof engine arcs.** L7 LANDED 2026-06-12: induction via recursive
  contracts + decreases for single-state machines whose body is a chain of
  guarded value/tail-self-call transitions. Still open: exit-ensures
  anchoring for general bodies (statement-position recursion gets no
  hypothesis — the termination graph does not see those calls), non-tail
  value recursion (compound arm expressions do not parse), quantifiers,
  Bag/Seq lowering, growing the Lean ladder past L7.
- [ ] **Hot-swap semantics.** Quiescence proofs, borrows as swap
  back-pressure, multi-version concurrency mode, replacement declarations
  (`replaces`/`migrates`) — versioned data stage 3+, depends on the
  concurrency model.
- [ ] **Wire encoding families + negotiation.** Beyond stage-2 encoders:
  fixed-width/text families, canonicalization, unknown-field preservation
  policy surface, version negotiation.
- [ ] **Serialized capabilities.** Attenuation + revocability across
  IPC/reboot/network (Cathedral's #1 flagged gap). Depends on wire + the
  capability runtime story.
- [ ] **Text/string proof domains.** `String::Utf8`/`NoNul` as
  boundary-established carried facts without a byte-level proof tax (frozen
  direction in decision 5; the domains themselves unbuilt).

</details>

## Resolved Design Decisions (frozen)

Reference, not tasks. Implementation slices build against these. Full multi-paragraph
bodies live in the wiki + memory files; only the titles are kept here.

1. Measure declarations (termination).
2. Range forms.
3. Operator spellings.
4. Boundary primitive registry.
5. Text types.
6. Fat descriptor model + owner.
7. Case members, not `enum`.
8. Properties, traits, conformance, and ZII opt-in.
9. Strict result use.
10. Wire eras.
11. Equality vs membership.
12. Discard admits effects; pure discards are dead code.
13. Property bounds: brackets attach to what they follow, everywhere.
14. `Versioned<T>` container.
15. Lifetimes: the Rust model, adopted wholesale.
16. Suspension: the `await` marker; waiting is a boundary primitive.
17. Arithmetic is EXACT by default; overflow is a proof obligation; weaker behavior is an explicit DOMAIN.

## Next Up (highest leverage)

**Inline asm control-flow follow-up.** Current inline asm support is deliberately
narrow: `asm { jmp state(...) }` parses and lowers to an ordinary Omega
transition target. Arbitrary labels/back-edges are actively rejected by fail
canary, while structured load/store mnemonics, register constraints,
clobber/effect declarations, and `asm where` contracts remain unsupported and
should not be faked as generic statements.

**Transition data-pattern follow-up.** Current data-pattern support is a narrow
transition-guard lowering path: `Type { field, .. } if guard` rewrites bare
captured field names inside `guard` to member reads on the single match subject.
Need real pattern binding semantics, multi-field/multi-subject validation,
domain-pattern lowering that proves membership rather than just compiling the
surface, and clearer diagnostics for unsupported destructuring forms.

**Const data parameter follow-up.** Current `const` data parameter support is a
structural compile path: syntax/resolved/typed trees preserve const parameters,
and `[T; N]` carries a symbolic length instead of collapsing to a fake literal.
Uninstantiated symbolic lengths deliberately do not produce concrete layout or
runtime-storage descriptors yet. Need instantiation-time substitution,
duplicate/value-kind validation, layout diagnostics for unresolved symbolic
lengths in non-generic contexts, and operator/range proof integration for
const-length facts.

**Data version semantics follow-up.** STAGE 1 + STAGE 2 DONE. STAGE 3 frontier:
the era tag itself (and decision 10's wire-era ride), era-tagged containers that
make version matching selectable, migration chains / `replaces` / quiescence
obligations.

**Wire data semantics follow-up.** Stages 1, 2a, 2b done. Still needed: String
decode (borrow-facts follow-up), nested/repeated fields,
wire-schemas-as-program-types, runtime layout of wire values, encoding-family
semantics beyond compact_binary v0, and version negotiation.

**Host-provider semantics follow-up.** Current host-provider support is
syntax-preserving metadata: it parses and snapshots syscall mapping rows, but
semantic lowering still ignores the item. Boundary-provider registry validation,
target-package whitelisting, syscall/import lowering, and boundary report
integration still need the real implementation.

**Trait default semantics follow-up.** Current `default machine` support is
structural: the marker flows through syntax/resolved/typed signatures and the
default body is parsed. Trait conformance, implementation reuse, override rules,
and dispatch behavior still need a real semantic pass before default methods are
more than surface syntax.

**Dynamic trait follow-up.** Current `dyn Trait` support is structural and
compile-path oriented: syntax/resolved/typed/checked trees preserve dynamic trait
types, receiver lookup can target trait machines, and layout/runtime-storage use
an explicit dynamic-trait fat descriptor. Need true trait-object construction,
vtable/interface table emission, dynamic dispatch lowering, and validation that
only trait object-safe machines are callable through `dyn Trait`.

**Relax semantics follow-up.** Current relax support is intentionally structural:
syntax is preserved, relaxed reference metadata flows through typed trees, and
relax scopes flatten during syntax-to-resolved lowering after resolving the target.
The invariant-weakening semantics still need a checked-tree/proof pass that marks
which place is relaxed, verifies exclusivity, and restores obligations at scope
exit.

## Vertical Slices

### Array, Vec, String, And Views

- [ ] Design `Vec[T]` as owned dynamic storage with length and capacity (surface
  declared; real storage/lowering pending).
- [ ] Back `Array::as_slice`/`as_mut_slice` with real boundary-primitive
  lowering (declared as contracts today).

### Ownership, Borrowing, And Views

- [ ] Continue appending ownership transfer/drop events from the remaining
  value-expression sites. (Now covered: operator-result + let-init seams,
  assignment-target owned production, statement-level operator/boundary calls,
  terminal/bare expression statements, and exit-drop obligations for owned
  by-value state parameters. Operator argument/receiver policies resolve by
  spelled path — call sites carry no operator symbols today — and a static
  type-name receiver like `String::with_capacity` no longer records a bogus
  type-symbol move. `self.field` event roots re-root at the machine symbol so
  downstream stages, which filter `self` parameters, can still resolve them.
  Remaining: move-subtraction/liveness so exit drops become per-edge truths
  instead of conservative obligations, and events for owned operator results
  produced directly in argument/transition-value positions, which have no
  place to root at yet.)
- [ ] Lower abstract ownership summaries into explicit backend transfer and
  cleanup operations. (First landing: the encoded ownership summary now
  renders per event in the backend report's Artifact Semantic Spine — place,
  machine/state, and source point — proving the events survive checked trees
  through the encoded machine. Real transfer/cleanup operations are
  deliberately NOT emitted yet: no type carries a cleanup machine, so every
  drop is semantically empty and emitting no-op cleanup code would be dead
  weight. Revisit when drop-bearing types land — Vec/String real storage and
  the allocator story.)
  - CORRECTION 2026-07-04 (probe): the "no type carries a cleanup machine, so
    every drop is semantically empty" premise is FALSE for user code. A user CAN
    write `Guard::drop(&mut self)` with an OBSERVABLE body today; it compiles
    clean and the drop is TRACKED (report: `drop <unnamed> ... at state exit`),
    but the body is NOT lowered to execution — probe-verified: a drop whose body
    exits 42 never fired; the program reached its normal path and exited 70. So a
    NON-EMPTY drop is a SILENT NO-OP (unlock/close/flush all do nothing, no error,
    no warning), and ch16's "lowered drop edge runs the cleanup" is aspirational.
    Soundness-adjacent, not mere dead-weight avoidance. SETTLED (Zach, chat
    2026-07-07): option (a) — FENCE. LANDED 2026-07-08 (omega-validation
    machine walk; fail canaries drops/nonempty_drop_body_rejected +
    drop_ensures_nonempty_body_rejected). NOTE a premise correction: ONE of
    the five drops canaries (drop_ensures_domain_membership) had a NON-EMPTY
    body backing an `ensures` — the sharpest case of the hazard (a proof fact
    from cleanup that never runs), so it CONVERTED to the second fail canary
    rather than staying green; the other four (empty bodies, incl.
    ensures-with-empty-body) stay green as expected. Full enforcement (drop
    lowering, use-after-move, borrow mutability) stays DEFERRED as one
    subsystem, "directionally correct" per Zach. Real fix = emit the drop-machine call at
    the tracked state-exit point (reverse declaration order; skip on move-out per
    ch16's move-guard edges). PREREQ for the real fix: a DROP-TIMING design
    brief — Omega has no lexical scopes, so candidate drop points are
    state-transition boundaries vs machine completion; Rust's scope-exit model
    doesn't transfer directly, though the explicit finite state graph makes
    use-after-move dataflow cleaner than Rust's inferred CFG. See memory
    drop-bodies-not-executed.
  - SIBLING (probe 2026-07-04): use-after-move is NOT rejected either. ch2 line 20
    states "After a move, the old binding is no longer usable," but
    `let g = Guard{..}; self.sink(move g); let x = g.handle;` COMPILES clean --
    verified for both an all-scalar (Copy-eligible) type AND a LINEAR type (one
    with a `drop` machine, which cannot be Copy). No fail canary covers it (only
    ownership/assign_immutable_parameter). Same subsystem as drops: ownership is
    frontend-MODELED (move mechanics work, drop obligations tracked) but
    ENFORCEMENT is unimplemented. Memory-safe TODAY (value semantics + ZII +
    drop-is-no-op ⇒ no double-free/dangling); both become real bugs under true
    linear semantics. Real fix for this half = the move/borrow checker tracks
    moved-out bindings and rejects subsequent reads. Fence-vs-implement is Zach's
    call, same as drops. Treat "ownership enforcement" as ONE in-progress
    subsystem; don't re-probe it expecting rejection.
  - SIBLING (probe 2026-07-04): borrow MUTABILITY compat is NOT enforced either.
    Passing an IMMUTABLE reference where a `&mut` parameter is expected --
    `handle(&self.c)` for a `c: &mut Counter` param -- COMPILES clean (the callee
    may then mutate through a borrow the caller only lent immutably). Precisely
    located: `validate_call_arguments_handles` (calls.rs) `CONTINUE`s on a
    mutability mismatch (`parameter.is_mutable && !is_mutable`) instead of erroring
    -- it SKIPS the arg entirely rather than rejecting. (The reverse, `&mut x` for a
    `&` param, is SAFE and correctly accepted.) Repro parked at
    `canaries/pending/arithmetic/immutable_arg_for_mut_param_not_checked`. NOT a
    one-liner (analysis 2026-07-04): `is_mutable` there is SYNTACTIC
    (`matches!(arg, Mutable(_))`), so a naive `continue`->error FALSE-POSITIVES on a
    valid `&mut` FORWARD (`a(x: &mut Counter)` passing `x` to a `&mut` param --
    `x` = Name, not a Mutable node, is_mutable=false; verified it compiles today).
    The real check needs SEMANTIC mutability (resolve the arg's declared reference
    mutability), i.e. borrow-checker work -- confirming the ownership-enforcement
    subsystem deferral. Memory-safe today (value semantics + drop-is-no-op).

### Runtime And Backend Confidence

- [ ] MISCOMPILE CLASS (probe 2026-07-04): the CONST-FOLDER miscompiles every
  SIGN-SENSITIVE op — `>>`, `/`, `%` — on a WRAPPING-produced high-bit value.
  All three verified native-vs-interp: `(0u32 - 2) >> 1` → native 0xFFFFFFFF vs
  interp 0x7FFFFFFF; `(0u32 - 2) / 3` → native 0 vs interp 1431655764;
  `(0u32 - 2) % 3` → native 0xFFFFFFFE vs interp 2. ROOT (single):
  `omega-state-values/src/simplify/folding.rs` is TYPE-BLIND (bare i64), so
  `0u32 - 2` folds to i64 `-2` (losing u32 width), and each sign-sensitive op
  then diverges from the typed value. Non-sign-sensitive ops (`+ - * & | ^ <<`)
  agree mod 2^width under i64 + truncation, so they are unaffected; comparisons
  are NOT reachable (guards keep the runtime storage ref + pick the unsigned
  compare — parser also rejects an inline arithmetic guard subject).
  SCOPE: only COMPILE-TIME-CONST-FOLDED high-bit-from-wrapping values. RUNTIME
  (field-held) unsigned `>>` / `/` / `%` are CORRECT (selection resolves the
  field's signedness) and are LOCKED: `arithmetic/runtime_shift_right_signedness`
  (new), `arithmetic/runtime_{signed,unsigned}_division_exit`. A DIRECT
  `0xFFFFFFFE …` literal folds to a POSITIVE i64 and is fine.
  FIX (real task, NOT a tick): the fold needs the operand's integer TYPE, which
  is erased here — `simplify_binary_expression` (simplify.rs) has `program` and
  the pre-fold `binary.left/right`, but the only type helper
  (`reflexive_operand_provably_not_nan`/`member_field_primitive`) types ONLY
  literals + data fields, not locals/params/sub-exprs. Need a general
  `expression_primitive_type(program, machine, expr)` there, then fold `>>` `/`
  `%` with UNSIGNED semantics (width-masked) for unsigned operands. Verified a
  "just defer to selection" band-aid does NOT work (`TableBinaryExpression`
  carries no type; selection defaults to signed on the type-less literal). Parked
  repros: `canaries/pending/arithmetic/const_fold_{unsigned_shift_right,unsigned_divide}_miscompile`.
  Memory: `shift-right-signedness-const-fold`.
  SPIKE 2026-07-04 (rules OUT the tempting narrow fix): canonicalizing an unsigned
  binding's folded value at the STATE-VALUES layer (enrich `Binding` with the
  let's type, mask `-2`→`4294967294`) is INSUFFICIENT. `simple_local_binding_value_from_table`
  stores binding values UNFOLDED (it preserves `Name(a)`; the substitution point
  does not re-simplify), so the fold-to-constant does NOT happen there — and
  per the decision-17 memory's DBG trace, instruction-selection's alias/static-
  value resolution independently RE-FOLDS via `fold_binary_expression`. Fixing
  one fold layer is whack-a-mole; the type must ride ON the constant so it
  survives every layer (the metadata-on-`Expression::Integer` representation).
  ⚠️ SCOPE CORRECTED 2026-07-05 (attempted the Box-only scaffold, reverted clean): the "41
  backend sites, Box `Expression::Integer`" estimate is TOO SMALL. Box `Expression::Integer`
  (state-values folder) and TABLE `ExpressionNode::Integer` (arena form, what validation +
  everyone else matches) are SEPARATE. instruction-selection re-materializes its Box exprs via
  the CONTEXT-FREE `expressions.to_tree(handle)` straight from the TYPE-LESS TABLE (grep to_tree
  in instruction-selection), so a Box-only stamp NEVER reaches its fold. The type must live on
  the TABLE `ExpressionNode::Integer(i64)` → `Integer(i64, Option<PrimitiveType>)`, rippling to
  the WHOLE typed-tree consumer base (validation's dozens of matches, lowering, interp, backend)
  -- a major cross-compiler representation change, NOT backend-localized. `to_tree` is mechanical
  so there is no single POPULATE site; stamp at typed→checked lowering from declared/context types.
  NEXT-SESSION PLAN (scoped 2026-07-04, RE-SCOPE per above): change TABLE `ExpressionNode::Integer`
  → carry `Option<PrimitiveType>`, mechanical (`Integer(v)` →
  `Integer(v, _)` / `Integer(v, None)`) across the whole consumer base. Then (a) POPULATE at substitution
  (`simplify/bindings.rs` stamps the binding's declared `PrimitiveType` onto the
  folded value; checking stamps context type where a literal lands in a typed
  slot), and (b) READ in `fold_integer_math` — mask the result to the operand
  width and pick unsigned `>>`/`/`/`%` for unsigned operands (so `0u32-2` folds
  to `4294967294`, after which every downstream sign-op is already correct). The
  scaffold (variant + all-`None`) is green-and-behavior-neutral on its own but
  delivers no fix alone, so land scaffold+populate+read together in one session.
  This representation ALSO subsumes the decision-17 domain half (the folded
  constant could carry its domain too) and is the unified root fix flagged in the
  `decision-17-const-fold-domain-hole` / `shift-right-signedness-const-fold`
  memories. Because it changes a checked-tree data-shape between phases (ZII
  concern), surface the design to Zach before landing.
  UNIFIED ROOT with the domain hole: `Expression::Integer(i64)` is
  metadata-free, so every const-substitution/fold strips BOTH the operand's
  signedness/width AND its arithmetic domain. A single metadata-carrying-constant
  (or metadata-aware fold) fix closes both.
- [ ] DESIGN Q + divergence (probe 2026-07-04): a shift by an amount >= the
  operand WIDTH diverges native-vs-interp, and the semantics are UNDECIDED.
  `i32 1 << 40`: native masks the count to the register width (x86 `shl`: 40 & 31
  = 8 → 256); the interpreter does `(l as i64).wrapping_shl(40)` (masks to 64 →
  1 << 40, truncated to i32 = 0). In-range shifts (amount < width) agree + are
  correct — only out-of-range amounts diverge, so INTEGER shift differentials are
  unreliable for out-of-range amounts. QUESTION for Zach (per
  design-discussion-protocol): what are shift-by->=width semantics? Most
  proof-carrying-consistent = a PROOF OBLIGATION that the amount < width (like an
  index bound) → compile error for unproven `a << n`. Alternatives: define as
  mask-to-operand-width (match native; then fix the interpreter to mask to the
  operand width, not i64) or shift-out-to-zero. Parked repro
  `canaries/pending/arithmetic/shift_amount_at_or_above_width_divergence`; memory
  `shift-amount-out-of-range-divergence`.
  ROLLOUT BLAST RADIUS (measured 2026-07-04): the proof-obligation direction is
  Zach-endorsed, but introducing an Exact-shift compile error is NOT autonomous
  tick work — it breaks existing corpus shifts with RUNTIME amounts that aren't
  yet proven < width: `samples/cli/collections/bitset` (`mask << vals[i]`),
  `samples/cli/collections/bitset_sieve` (`bits >> i`, `m << j`), and
  `canaries/pass/arithmetic/runtime_signed_modulo_shift_edges_exit`
  (`base << self.n`), plus any others among the 126 corpus shift occurrences with
  a non-constant amount. Each needs per-site migration — a dominating guard
  (`n < width`, via the guard-narrowing keystone) OR moving the operand into a
  Wrapping/Saturating domain. That per-site choice is a real design surface;
  bring the migration plan to Zach rather than rolling the error out blind.
- [ ] SAME-CLASS divergence (probe 2026-07-04): a float-to-int cast of an
  OUT-OF-RANGE value diverges native-vs-interp. `1e20 as i32`: native = 0 (x86
  `cvttsd2si` yields the i64 "integer indefinite" 0x8000…, truncated to i32 = 0);
  interp = -1 (`f.trunc() as i64` SATURATES to i64::MAX, truncated to i32 = -1);
  both garbage; in-range casts agree. Parked repro
  `canaries/pending/arithmetic/float_to_int_overflow_divergence`.
- [ ] ** SYNTHESIS — UNDERSPECIFIED NUMERIC-RANGE OPS (design thesis for Zach) **:
  the two entries above (shift amount >= width; float-to-int cast out of range)
  are the SAME shape — an operation whose behavior is UNDEFINED outside a range,
  where native (hardware) and interp (Rust `as`/i64) diverge because neither is
  canonically correct. The proof-carrying-consistent resolution, extending
  decision-17 (Exact arithmetic = a proof obligation), is to make the RANGE a
  PROOF OBLIGATION: the shift amount provably < operand width, the float provably
  in the target integer's range — else a COMPILE ERROR (like an array index
  bound). Alternatives per-op: define saturating (Rust-style) or match-hardware.
  ONE ruling covers both (and likely future corners like `usize`/`Addr` casts).
  DESIGN CALL for Zach — flagged, not decided.
- [ ] Native-emission gap (surfaced 2026-07-04, CLEAN error — interp supports it):
  a state that CALLS another machine whose ENTRY is a branching (dispatching)
  state, passing arguments, is refused: "state calls: `A.s` … calls branching
  state `B.entry` with N argument(s); native emission needs guarded state-call
  expansion". So chaining `state next { self.check_c(Rec::C{…}); }` into a
  dispatch-entry machine works in the interpreter but not natively. Safe (clean
  refusal, no miscompile); the fix is guarded state-call expansion at the call
  site. Low priority — the workaround is to inline the second dispatch or make
  the callee entry a non-branching state that transitions inward.
  SCOPE SPIKE 2026-07-04: NOT a small fix. Lives in
  `omega-emission-planning/src/state_call_blockers/` over a developed
  `RuntimeBranchCallExpansion` taxonomy (GuardedLeaf → NeedsBranchPrelude →
  NeedsStraightLineTarget → NeedsNestedBranchTarget → UnknownTarget → Unplanned,
  ranked). My case doesn't even MATCH a planned branching call (reasons.rs:34
  `matching_calls.peek().is_none()` path) — it needs a new planned expansion
  threaded through the planner AND the emitter, not just filling `Unplanned`.
- [ ] Strengthen assigned-target allocation toward a real register/stack
  allocation story with register classes, spills, and post-assignment cleanup.
- [ ] Reduce host/runtime special-case lowering around stdin/stdout/process
  calls; build richer multi-step text flows and real console interaction.
- [ ] Replace the current Windows GUI sample shortcut with a real app-window
  host surface. `samples/gui/windowed_calculator` showed on 2026-07-04 that using
  predefined classes is only partial: the older `samples/gui/window_app` /
  `samples/gui/window_demo` `STATIC`-class path did not get real caption
  interaction right. On 2026-07-04, all three GUI samples were moved to the
  calculator's `#32770` + `WS_OVERLAPPEDWINDOW` workaround, so dragging and the
  non-close caption buttons behave like the calculator. The caption X button
  still does not close. This points at needing a registered Omega window class
  plus a real WndProc/close path instead of relying on borrowed
  predefined-class DefWindowProc behavior.
- [ ] UNKNOWN-FIELD validation -- direct `self.<field>` READ + WRITE both LANDED
  2026-07-04. A nonexistent field (a typo `self.cont` for `self.count`) is now caught
  at type-check in BOTH positions: a WRITE via places.rs
  `validate_assignment_target_handle` ("data `Main` has no field `cont`"), a READ via
  a check at the top of calls.rs `scan_expression_calls` (the full read-position
  expression walk: values, args, guards, let inits) ("reads `self.cont`, but data
  `Main` has no field `cont`"). Shared helpers `direct_self_field_member` +
  `machine_attached_data` (places.rs, pub(crate)) + struct_literals `data_declares_field`.
  Scoped to DIRECT `self.<field>` vs top-level data fields (exactly the accessible set);
  VERSIONED data excluded (`is_version_selector`). Locked by fail canaries
  unknown_field_{write,read}_rejected; full suite 579 + samples-compile clean (no
  false-positive across 150+ samples' field reads).
  STILL OPEN: NESTED `self.a.b` and non-self member accesses (`local.field`) are
  unchecked -- the direct-self scope leaves `b`/receiver-typed members to a general
  member-symbol-validity walk (an unknown field leaves an invalid symbol; name_paths.rs),
  which must handle the valid member forms (case payloads, era fields, domain members).
  A "did you mean `count`?" edit-distance suggestion would further help.
- [ ] CROSS-CLASS scalar assignment -- SILENT MISCOMPILE CLOSED 2026-07-04
  (literal + place, two waves same day). `self.i32 = true` (a `bool` literal) AND
  `self.i32 = self.bool_field` (a bool PLACE) BOTH used to pass `--check` and
  `--build-dir` with NO error at any phase; the backend stored the bool as `1` --
  a silent soundness hole (sibling of the #27 narrowing hole). NB the non-literal
  place case was WRONGLY assumed non-silent at first (I expected "needs mutation
  lowering"); dogfooding showed `i32 = self.bool_field` compiles+runs silently.
  Fix: `assignment_class_conflict` gate (expression_types.rs) folds every scalar
  into three DISJOINT value classes -- boolean / text / numeric -- and rejects an
  RHS whose class differs from the target primitive's, in the Assignment path
  (lib.rs) BEFORE value-range analysis. Resolves the RHS class two ways: a literal
  node's class, OR a resolvable PLACE (`self.field`/local) via
  `declared_place_type` -> primitive. Deliberately narrow: computed exprs
  (binary/call/cast/indexed) resolve to None and are left to the blanket general
  gate (ZERO false positive on computed values); int and float are the SAME
  (numeric) class, so numeric copies/coercions (`f64 = 5`, `i8 = 300`,
  `i32 = self.i8_field`) are untouched -- those stay the province of the
  narrowing/mutation-lowering checks (verified: they still error via THOSE checks,
  not this gate). Locked by fail canaries literal_class_mismatch_rejected +
  member_class_mismatch_rejected; full suite + samples-compile clean.
- [ ] CROSS-CLASS call/transition ARGUMENT -- SILENT MISCOMPILE CLOSED 2026-07-04
  (3rd wave, same family). A `bool`/`String` field passed where an `i32` parameter
  is expected -- `take_int(self.b)` (transition), `self.contained.f(self.b)`
  (machine call), `self.console.exit_process(self.b)` (host/boundary) -- ALL passed
  `--check` + `--build-dir` with NO error; the arg reached the backend/host encoder
  as a raw byte and was read as garbage (exit 0/1). NB I again WRONGLY assumed this
  was non-silent (the `validate_call_arguments_handles` "documented frontier"); the
  existing shape gate `argument_matches_type_reference_handle` BLANKET-ACCEPTS
  place/name args (Member/Name) against ANY primitive param, so the class conflict
  slipped. Fix: reuse `cross_class_conflict` (the assignment gate, renamed from
  `assignment_class_conflict`) inside `validate_call_arguments_handles` -- for each
  arg that PASSES the shape gate, resolve its scalar class and reject a cross-class
  store. Threaded `current_machine` + `current_state` through the fn + all 6
  callsites (calls.rs x4, transitions.rs) so place args resolve via
  `declared_place_type`. ALSO added arg validation to the boundary/trait-call
  branch (calls.rs ~249) which previously skipped it entirely ("validation lives
  elsewhere" -- only the backend host-encoder caught anything, and only literals);
  full validate_call_arguments_handles there is safe (582 canaries + samples-compile
  clean, no arity/shape regression). Only fires on args that pass the shape gate, so
  no double-report with cross-class LITERAL args (the shape gate already rejects
  those). Locked by fail canary arg_class_mismatch_rejected.
- [ ] CROSS-CLASS value-position call ARGUMENT -- SILENT MISCOMPILE CLOSED
  2026-07-04 (4th & final wave, cross-class family COMPLETE). `let v: i32 =
  self.take(self.b)` (bool field into an i32 value-position param) compiled+ran
  silently (exit 0, arg read as garbage) -- confirmed by building a repro from the
  `let v = self.next(&mut ...)` canary syntax. Value-position calls route through
  `validate_value_position_calls` -> `scan_expression_calls` ->
  `validate_expression_call_bounds` (decision-13 residue), which validated only
  type-parameter BOUNDS, never argument classes. Fix: extracted the per-arg check
  into a shared `report_cross_class_argument` helper + a value-position wrapper
  `validate_value_call_argument_classes`, called it at ALL 5 callee-resolution
  branches (self-state, attached-sibling, free-machine, external-machine,
  attached-data). Deliberately NOT placed inside
  `validate_machine_call_type_parameter_bounds` (shared with the statement path,
  which already class-checks via `validate_call_arguments_handles` -- would
  double-report). No shape gate ahead of the value-position path, so it also
  covers literal args there. Locked by fail canary
  value_call_arg_class_mismatch_rejected.
  Whole cross-class-store family (assignment literal/place + call/transition/host
  arg + value-position arg) now CLOSED -- see [[literal-class-assignment-miscompile]].
- [ ] Broaden persistent machine/state mutation coverage beyond isolated
  micro-shapes toward dungeon-sample blockers.
- [ ] Link final-image imports/fixups back to source and lowered boundary-edge
  summaries for reporting and target-policy validation.

## Standing Rules

### Cleanup

- Only split modules when a file owns multiple semantic nouns, blocks a vertical
  slice, or hides a query/canary boundary.
- Keep representation roots explicit when a stage carries both executable shape
  and preserved semantic evidence; keep root constructors and canaries for any
  durable root shape.
- Keep `lib.rs`/`mod.rs` as boundary declarations, not junk drawers.
- Prefer arena/handle/handlespan storage over nested tiny allocations for durable
  IR.

### Canaries

- Three honest categories: `pass` = supported, `fail` = intentionally rejected
  (focused on intended diagnostics), `pending` = desired behavior known but
  implementation behind. Promote pending quickly when fixed; don't let
  compile-only pass canaries imply runtime support.
- Current local suite status (2026-06-11, macOS ARM64 host): `cargo test -p
  omega-compiler --test canary_suite` is 184/184 and the differential oracle
  is 5/5, dungeon included — FULLY GREEN. The aarch64 encoder convergence
  wave closed the 30-failure arm64 gap, and the dungeon "hot-potato" root
  cause was the encoder using x18 (the Darwin reserved platform register,
  zeroed by XNU on kernel→user returns) as copy scratch — fixed by register
  substitution, pinned by the interrupt-soak canary under `pass/dungeon/`.
  Full `cargo test --workspace` is also green. No registered pending
  canaries (the proofs false twins were promoted to `fail/proofs/` by the
  entailment engine; see `wiki/proof_engine_roadmap.md`). Keep this line
  current when backend/runtime work moves canaries between `pass`, `fail`,
  and `pending`.

### Docs

- Add a dedicated guide section/chapter for core semantic types once syntax
  stabilizes; add navigable core docs alongside `omega/language/core`.
- Keep traits/modules/host-boundaries sequencing coherent; keep speculative
  topics clearly labeled as working direction.
