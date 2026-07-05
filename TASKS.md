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

## North star: prove Omega non-toy via real software rendering (2026-06-28)

Settled direction (Zach): land *serious* sample projects that actually do
software rendering, to prove Omega is not a toy. A grounded multi-agent survey of
the compiler vs. that goal (2026-06-28) found the **computational core already
runs natively today** — 1D arrays as framebuffers, runtime-indexed read+write
(the index-load OOB segfault is fixed, `2ccb39da`), per-pixel integer and f64/f32
math, hand-written loops, the in-bounds proof model (now incl. the
multi-predecessor-join MEET, `62aba398`). A frame can be *computed* in memory now.
Two things gate getting it on screen and making it ergonomic.

### Tier 1 — animated CONSOLE renderer (no native-FFI gate; do this first)
> **STATIC console rendering DONE (2026-06-28).** A real per-pixel Mandelbrot set
> renders to the console in pure Omega with ZERO new compiler features —
> `samples/mandelbrot` (64x28) + canary `text/runtime_mandelbrot_render_exit`
> (self-checks 140 in-set pixels, in the differential oracle: native==interp). The
> working renderer pattern: `[u8;W] in Utf8` carrier framebuffer + nested col/row
> state-machine loops + per-pixel f64 escape-time + runtime-indexed carrier byte
> writes + `write_line` per row (the multi-pred-join meet `62aba398` is load-bearing
> at the convergence). The FIRST non-toy proof.
>
> **ANIMATION DONE (2026-06-28).** `Clock.sleep` (kernel32 `Sleep`) landed
> (`5e3072cc`) — the first new host op / FFI-ladder rung — plus `write` (no-newline)
> + `sleep` on the stdlib `Console`. `samples/bouncing_console` animates a ball in
> place on one line (clear/draw/`write(CR)`+`write(line)`/`sleep`, 50 frames,
> bounces off the wall), paced by real native sleeps (verified ~484ms for
> `sleep(400)`). So Tier-1 (static + animated console rendering) is COMPLETE. Next
> FFI rungs: Linux `clock_nanosleep` (timespec buffer), and a value-returning op
> (`GetTickCount64` -- the return-to-storage path) for elapsed-time animation.

(historical, now done) Render ASCII/ANSI frames to stdout in a timed loop. Needs only: (a) the **clock
capability** — the `clock` build flag is reserved but INERT (no host op / binding
/ encoder). Wire Windows `GetTickCount64` (0-arg, returns u64 ms in rax — the
simplest possible native call; exercises the full host-call return-value path),
Linux `clock_gettime` as a follow-up. (b) existing stdout. Then a real sample
(rotating cube / plasma / fire in the terminal). Proves "software rendering +
animation" cheaply and de-risks host extensibility.

### Software-rendering STATUS (2026-07-01): console tier COMPLETE

The functioning software-rendered canary/sample set exists and is output-verified
(not just exit-checked) + differential where applicable: `mandelbrot` /
`mandelbrot_zoom` (per-pixel escape-time), `dungeon_render` (enum tilemap),
`cellular_automaton` (animated Sierpinski Rule 90), `tick_marquee` (render loop
verified against REAL elapsed time via tick_count), `bouncing_console` /
`bouncing_ball_2d` / `bouncing_particles`, `ripple_field`, `dice_histogram`.
Extern ladder landed: rung 1 (general Win64 scalar-args call), rung 2 (value-
returning import + store-rax, `tick_count`), rung 3 (multi-DLL import table),
rung 6 (input, `key_state` via user32). This is the differential-testable tier;
it is DONE.

### WINDOWED TIER LANDED (2026-07-01, the dedicated session)

The "not differential-canary-able" concern above was WRONG -- two techniques
fixed it: (a) `CreateCompatibleDC(0)` gives a CI-safe memory-DC blit target
(nothing visible, real gdi32 pixels), and (b) StretchDIBits RETURNS the copied
source scanline count (probed: full height even into the memory DC's default
1x1 bitmap, and the SOURCE height when stretching), so a blit is exit-code
assertable and the interpreter's virtual GDI mirrors it. What landed:

- **General Win64 import-call encoder** (`encode_win64_import_call`): stack
  args at [rsp+32+8k] with 16-byte call alignment (pad 8 when the slot count is
  even), `RuntimeStorageAddress` operands (lea through the relocated r15 region
  base -- framebuffer as LPVOID, OS structs as inline arrays, byte-array C
  strings), width-honoring result store (4-byte results store eax). TickCount
  re-routed through it byte-identically.
- **`Gui` capability** (x86_64-gated lowerings; other arches get a clean
  UnsupportedHostCall): `dc_create` (CreateCompatibleDC), `get_dc` (GetDC),
  `window_create` (CreateWindowExA via the built-in "STATIC" class -- rung 8's
  WNDCLASS/DefWindowProc/message-pump bundle DODGED entirely), `blit`
  (StretchDIBits, 13 ABI args, separate dest/src sizes so a small framebuffer
  stretches). All value-returning, all REQUIRE the assignment form
  (statement-position lowers to no operands -> clean encoder error, #40).
- **BITMAPINFO as a plain [i32;11]** built in Omega code (biSize=40, negative
  biHeight for top-down, packed planes|bitcount word) -- no format-domain codec
  needed for the MVP; identity-layout arrays ARE the hot path the brief
  predicts.
- Canaries (both differential): `host/runtime_gui_memory_dc_blit_exit` (CI-safe
  memory DC) + `host/runtime_gui_window_blit_exit` (REAL invisible window ->
  GetDC -> blit). Sample `samples/gui/window_demo`: a visible WS_POPUP window, 60
  frames of animated 64x64 wash stretched 4x, ~1.5s, exit-asserted.

**MESSAGE PUMP + STANDALONE APP LANDED (same session, follow-up).** Five more
user32 ops -- msg_peek (PeekMessageW PM_REMOVE into a caller-owned [u64;6] MSG
buffer), msg_translate, msg_dispatch, is_window, window_destroy -- all pure
catalog+selection+interp additions (the general import encoder needed ZERO
changes: the machinery amortizes exactly as intended). The interp's virtual
window system mints live handle tokens (HashSet liveness; is_window/destroy
mirror native 1/0). Canary host/runtime_gui_window_lifecycle_exit
(differential): create invisible -> bounded pump drain (a fresh window has ~1
pending message natively, 0 virtually -- drain count deliberately unasserted)
-> IsWindow>0 -> DestroyWindow>0 -> IsWindow==0. `samples/gui/window_app` is the
GENUINE STANDALONE APP: stays open until closed, draggable (pump dispatches
through the STATIC class's DefWindowProc), X button works (verified end-to-end
by posting a real WM_CLOSE from another process -> pump -> destroy -> exit 0),
ESC quits too (key_state 27). It has NO "Expected exit" annotation ON PURPOSE
-- the harness compiles it but must never RUN it (it waits for a human);
window_demo remains the short-lived exit-asserted variant.

Remaining windowed-tier work: the PE GUI-subsystem toggle (headers.rs:57
write_u16(3) -> 2) so a double-click launch spawns no console -- NEEDS A
SPELLING DECISION (per-program/per-build; the settled design deleted the
`host:` flag dialect, so where "subsystem" lives is Zach's call -- do NOT
pre-decide); format-domain codecs when a NON-identity schema shows up; UTF-16
window text (CreateWindowExW) if titles matter. Note: launching from an
existing terminal inherits that console (no extra window) -- the toggle only
changes double-click/Start-Process launches.

### Tier 2 — windowed software renderer (the native-FFI ladder)
> **DESIGN SETTLED 2026-07-01** — see
> [wiki/design_briefs/extern_boundary_and_format_domains.md](wiki/design_briefs/extern_boundary_and_format_domains.md).
> No `extern` keyword: the foreign surface is `boundary trait`; the DLL is named
> only in a target's `provides` mapping (`present -> gdi32::StretchDIBits`, path
> form beside `-> syscall N`); OS structs are wire data serialized through FORMAT
> DOMAINS (encoding facts on byte carriers, chosen at the use site — `Utf8`
> generalized); NO c_layout/repr — Omega's layout stays sovereign; domain entry =
> mints only (no `is`/`as`; `when` dies); target blocks lose `host:` + the flag
> dialect (a target = the boundary packages it trusts; absence = denial).

The host layer is a CLOSED catalog of 5 hardcoded kernel32 ops (GetStdHandle,
ReadFile, WriteFile, ExitProcess, Sleep), each a bespoke x86_64 emitter. The
engineering ladder (sizes from the survey):
1. [LARGE — the gate] general Win64 native call — marshal an arbitrary arg list
   into rcx/rdx/r8/r9 then stack + shadow space, return in rax. De-risk by
   re-expressing the existing 5 kernel32 ops through it (no behavior change).
2. [MEDIUM] multi-DLL imports (user32/gdi32/winmm) — PE import table is hardcoded
   to a single KERNEL32.dll descriptor (`omega-image-pe/src/imports.rs`), plus the
   `provides` path-mapping parse (`machine -> dll::Symbol`).
3. [MEDIUM] wire-data format codecs (win32_x64 first): derived encode/validate
   mints for fixed-offset schemas (BITMAPINFO / WNDCLASSEXW / MSG); identity-encode
   borrowed view for structurally-identical carriers (the framebuffer hot path).
4. [MEDIUM] raw framebuffer pointer usable as an LPVOID arg (structural
   `Win32Dib` domain over `[u32; W*H]`).
5. [SMALL] value-returning import (`GetTickCount64` — the rax-return path).
6. [SMALL] input — `GetAsyncKeyState` (1 arg).
7. [SMALL] PE Subsystem GUI(2) toggle (hardcoded to console(3) in
   `omega-image-pe/src/headers.rs`).
8. [LARGE — AVOID] function-pointer type / machine-as-C-callback for a WndProc.
   Dodge it: register the class with `DefWindowProcW` (imported symbol, no Omega
   callback), poll with PeekMessageW/DispatchMessageW, blit with `StretchDIBits`
   of a top-down `.bss` DIB each frame.
The proof/effect system already RESERVES the authority (`clock_read`, `device_io`,
`memory_map`, `dynamic_link` in `omega-effects`) — it does not block this. #1 is
the gate; 2–7 are incremental after it.

Settled-design MIGRATIONS (mechanical, can precede the ladder):
- delete `when` (1 parser site `parser/domain.rs:83` + ~200 vestigial
  `domain ...::Utf8 when valid_utf8(self)` headers → invariants as domain members);
- delete the `host: { abi/stdout/filesystem=... }` blob + flag dialect from
  ~180 `build.omg` files + `parser/target.rs` (a target block = `boundary` package
  lines only);
- retire the stale May-era `omega/host/**` sketch packages
  (`capability X { entry ... }` / `String` / `Slice<u8>` spellings).
Open spellings tracked in the brief §10 (generic domains `Protobuf<Level>` is the
big one; encode/decode surface; field-peek accessors; streaming/append).

### "No silent-anything" wave (2026-07-02, Zach's direction) -- landed + the keystone gap

Direction (chat, 2026-07-02): nothing silent -- overflow-capable arithmetic must PROVE its
bounds or DECLARE a domain; if verifiably bounded, the operator shouldn't care which domain.
Toward abort-as-an-effect (#65): trapping should eventually be visible at main. Landed:

- **Nested-field Exact enforcement** (the decision-17 hole): 3+-member places
  (`self.p.x`, field-stored payload `self.m.dx`) now carry the overflow
  obligation instead of silently wrapping; nested RANGES also narrow.
- **ZII-range fence** (found during the migration, PRE-EXISTING): a range
  excluding 0 on zero-initialized state let the prover trust a bound the
  startup 0 violates -- probed to a runtime div-by-zero in a fully-"proven"
  program. Rejected across the ZII-reachable closure
  (validate_zero_reachable_field_ranges).
- **7 samples migrated FULLY EXACT** (no Wrapping added; modular_exponentiation
  dropped 2 Trapping fields). The MODULAR-COUNTER idiom is the one
  Exact-provable bounded counter today: `c: i32 [0..=N]` + `c = (c + 1) % (N+1)`.
- **Narrowing store obligation** (2026-07-02, decision-17 completion): a value
  whose proven range does not fit the DESTINATION integer type is a silent
  truncation and now a compile error at every value-binding boundary --
  assignment, `let x: T =`, terminal return, and transition-value return. Was a
  pervasive silent hole: `self.i32 = 3000000123` (exit 123, truncated), `let x:
  i8 = 300` (44), `self.i32 = someI64` all compiled + wrapped. The check reuses
  the S4 interval machinery: flag when the source interval, INTERSECTED WITH the
  source type's range (so a `u32 in Wrapping` sum stays a u32 -- no false
  positive -- while a flow-proven `[7,7]` still narrows into i8), is fully
  bounded and not contained in the target range. Widening + explicit `as` stay
  fine; unbounded unknowns (call results, `u64`/`usize` high) stay permissive,
  exactly as the exact-arith check leaves them. Blast radius across the whole
  corpus: ONE canary (a usize->u32 count round-trip; migrated to count in u32) +
  ZERO samples. Fail canaries arithmetic/narrowing_{literal_wider_than_target,
  wide_local_unproven}. REMAINING: transition-value returns still skip the
  ARITHMETIC overflow check (a pre-existing gap, distinct from narrowing --
  narrowing IS now enforced there via a throwaway-buffered analysis).

**THE DE-TRAPPING KEYSTONE -- LANDED (same day):** the bounded-place
obligations (omega-proof checker) now refine `<place> +/- K` values by the
DOMINATING GUARD: the value's folded range inverts to the place's bound, the
guard tightens it (apply_handle_condition: structural place match, `&&`,
either literal side), and the refold intersects back. Two faces:
- ASSIGNMENT (incoming-edge guard, all edges must agree): gated by a STABILITY
  check -- every earlier statement in the state must be a call-free local/
  assignment writing a provably DISJOINT place (member-path prefix aliasing;
  indexed writes alias their collection only -- so the render-loop shape
  `arr[i] = px; i = i + 1` stays provable). Enforcement canary:
  fail/arithmetic/guard_invalidated_by_prior_write_rejected (a prior write to
  the counter must still reject).
- CO-LOCATED transition args (the arm's own guard; no gap to gate -- but
  collection DOWNGRADES the guard when any sibling argument contains a call,
  which could mutate the guarded place between guard and argument evaluation).
Pass canaries (differential): runtime_guard_proven_counter_exit +
runtime_guard_narrowed_transition_arg_exit. The natural loop
`transition i < N { true -> body } ... i = i + 1` now proves EXACT with
`i: i32 [0..=N]`. samples/gui/window_demo + samples/gui/window_app are converted FULLY EXACT
(zero domains in a real interactive windowed renderer).

**De-Trapping sweep DONE (2026-07-02): 38/38 converted, 0 reverted.** 24 fully
Exact (incl. game_of_life_glider at zero domains with byte-identical output;
cellular_automaton via an identity-clamp + `% 8` fold); 14 partial (indices
Exact; array-element/unbounded values keep DECLARED Wrapping with comments --
elements carry no range facts yet). `in Trapping` survives in samples ONLY in
`samples/trapping_probe`, where trapping IS the feature under test; canary
arithmetic/runtime_trapping_overflow_traps asserts the ud2 trap actually fires
(exit != 70 and negative). Verified techniques beyond the recipe: identity
min/max clamps (bound an unbounded value inline), branch-per-direction for
`p + dir`, funnel states for multi-predecessor edges, decreasing counters off
a bare `>= 0` guard.

**KEYSTONE COMPLETENESS GAP CATALOG (the next round's worklist, from the sweep):**
1. The proof-side range fold has NO Divide (obligations.rs integer_binary_range
   returns None for Divide + BitwiseAnd) while validation's Interval::divide
   folds -- the two folds disagree; `y = c/26` into a ranged y rejects even
   with c ranged, and appending `% K` doesn't rescue it.
2. guard_refined_binary_range is literal-only + top-level-only (`p + dir` and
   compound values unrefinable); declared ranges of a non-self OPERAND don't
   feed either fold.
3. A guarded COPY (`y = self.yv` with yv fully edge-guarded) doesn't narrow.
4. Multi-VARIABLE compound guards don't decompose (single-variable conjuncts work).
5. Multi-predecessor edge agreement fails for the write keystone (equivalent
   `sp < 16` guards on 3 edges don't prove; funnel states are the workaround).
6. Declared ranges don't feed the INDEX lower-bound prover (an explicit
   `>= 0` conjunct is still needed).
7. EXPRESSION range bounds (`[0 - 1..=40]`) parse but behave UNBOUNDED (no
   const-eval); literal negative forms (`[-1..=10]`) work. Const-eval or reject.

**Abort-as-effect (#65) design sketch (chat, NOT settled):** every trap-capable
site (`in Trapping`, future assert/panic) carries an `abort` effect threaded to
main / the target block (absence = denial = total program); Wrapping/Saturating
stay effect-free (total, visible in types). Needs a settle before building.

### Programmable-layouts compliance refactors (ch19/20/21 rewrite, 2026-07-02)

The chapters are now the spec (design_briefs/programmable_layouts.md, SETTLED).
The compliance work splits into a MECHANICAL near-term series and a
build-time evaluation-GATED program:

**R1 — the surface migration: DONE (266035104).** Identity numbers on plain
`data` + `retired N;`; the `wire data` form, `reserved` spelling, and the
declaration `encoding` clause retired with guided errors + fail canaries;
compact_binary v0 byte-identical (the wire-schema trees are unchanged
underneath -- an implementation detail the diagnostics no longer name). 42
corpus files migrated. Landmine recorded: a numbered schema whose FIRST member
is a version block is ambiguous with ch21's plain-data version blocks -- the
parser peeks inside the leading block.

**R2 — LAYOUTS AS A REAL CONCEPT (greenlit 2026-07-02; unblocked by the
build-time-evaluation settle: NO keyword, `comptime` retired as a foreign
term -- evaluation is position-driven, the effect system is the gate, trait
signatures carry the contract; design_briefs/build_time_evaluation.md).**
The ladder, smallest-landable-first; layouts need NO general const positions
(the compiler itself invokes plan() -- a blessed-trait call site):

- **L0 -- DONE (dc3acbd43).** `evaluate_build_time_machine`: structured
  arguments in (BuildTimeValue), positional binding (receiver excluded,
  count-checked), structured terminal value out; fuel cap; pilot = a
  plan-shaped machine. **L1-L3 -- DONE (same session).** layout.omg v0
  vocabulary (FLAT index-keyed Plan until array-of-struct element
  construction lands); compute_layout_plan (schema materialization for
  primitive fields, purity gate, L2 validation: overlap/bounds/alignment/
  power-of-two); pilot = CLayout IN OMEGA planning a UEFI-ish schema to
  correct C offsets/size/align; effectful + overlapping policies rejected.
  **Effect-surface gap CLOSED (48f600151):** boundary-trait methods with no
  declared effect row carry the implicit `host_boundary` effect in the
  decision-12 transitive surface -- both build-time gates reject statically;
  the evaluator's dynamic backstop stays as defense-in-depth.
- **L1: the closed vocabulary as library data.** `Schema`/`SchemaField`,
  `Plan`/`FieldEntry`/`FieldPlan` (At/Bits/Varint/LengthPrefixed)/`SizePlan`
  + the `Layout` trait declared in omega/ .omg. The compiler materializes
  `Schema` from a data definition's fields (name/size/align/kind/number --
  target-resolved).
- **L2: plan validation pass** (compiler-owned): names resolve, no overlap,
  in-bounds, no Bits straddle, ranges fit slots, tag/retired collisions.
  A bad plan is a compile error, never unsafety.
- **L3: first consumer, zero codegen -- the plan REPORT.** Evaluate + validate
  a policy's plan for a schema and emit it into the wire-report artifact
  (offsets/size/align). Proves L0-L2 end-to-end; canary-able by artifact
  content. CLayout (the ~15-line C ABI policy, written in OMEGA) is the pilot.
- **L4: derived VALUE TYPES for fully-static plans -- v0 LANDED 2026-07-02**
  (`gdt: Spread16<Gdtish>` in FIELD type position works end to end, both
  backends). Mechanism: a pre-resolution desugar synthesizes
  `data Policy<Schema> { <schema fields> }` and rewrites the spelling, so
  typing/validation/proof/interp see an ordinary record (the interpreter is
  name-keyed -- zero interp changes); the post-typing pass evaluates +
  validates the plan (L2/L3 pipeline), REQUIRES a fixed size ("a dynamic plan
  cannot be a value type -- values need offsets, bytes need mints"), and
  records it on `TypedTrees::plan_laid_layouts`; the native layout builder
  places those records at the plan's offsets (`place_fields_by_plan`) instead
  of packing. Placement is deliberately unobservable in-language, so the
  proof is two-sided: run canary `layouts/runtime_plan_laid_value_field_exit`
  (spread 16-byte placement, write/copy/read-back, differential-matched) +
  the `plan_laid_value_types_are_placed_by_their_plan` unit test asserting
  the baked FieldLayout offsets [0,16,32,48]/64/16 (native would be
  [0,4,8,16]/24). Fail canaries: dynamic plan; `X<...>` on a non-generic
  data with no `plan` machine. v0 boundaries (all clean errors): field type
  position only (params/lets keep existing generic errors); schema = plain
  record of primitives; construction = ZII + field writes + whole-value copy
  (no literal spelling); policy gate is structural (attached `plan` machine)
  until Layout-trait conformance is declared. STILL OPEN for L4 full: derived
  projections into a plan-laid BYTE VIEW + the no-op boundary theorem
  (`&CLayout<T>` IS `&[u8] in CLayout<T>`) -- needs the carrier/domain rung
  (L5) to express the byte side.
- **L5: OmegaLayout carriers -- v0 LANDED 2026-07-02** (834570a11): the
  carrier spelling `[u8; N] in OmegaLayout<Save>` parses (parameterized
  domain names flatten to instance names), validates (compiler-known family;
  byte-array carrier; schema must be numbered -- the packed grammar of an
  unnumbered schema honestly rejects; explicit grammar argument rejects,
  `Derived` is the default and only grammar), and is ENFORCED at every
  encode/decode call site (schema agreement). Byte-identical by canary (the
  refined round-trip pins the same hand-computed framing as the unrefined
  twin, native + differential); the {len,bytes} text-carrier reclassification
  excludes the family at both layout-builder gates; samples/wire_protocol
  states its buffer's format. REMAINDER for L5-full: target-directed
  `encode()` into a refined carrier (spelling OPEN -- extern brief section
  10.2 builtins-vs-boundary-operators), the `Packed` grammar, the
  plan-walking deriver (blocked on the case-vocabulary Plan = array-of-struct
  element construction), the validate/materialize decode mint, and
  refinement-as-obligation (unrefined buffers still work today).
- **RECAST (settled 2026-07-02, programmable_layouts §5b): borrows under a
  second stated shape, spelled `as`** (`&x as &f32`, `&mut gdt as &mut
  GdtRaw`, `&mut gdt as &mut [u8; N]`). Engineering: the borrow-recast form
  in the checker (borrows are same-type today) + the plan-tiling /
  fact-implication validator (same footprint; `&` = src⟹tgt facts, `&mut` =
  both directions; weaken-never-strengthen -- as-into-domain stays dead;
  untyped sources annotate first). Queued behind the validate-mint rung.
- **L6+: Bits placements + access classes (MMIO deriver); durability plan
  grades consumed by Store<T>-class APIs; publish-time predecessor diff.**

### Language ergonomics surfaced (mostly engineering; one research)
- **[NEEDS DESIGN — Zach]** loop syntax (`for`/`while`) that desugars to the
  existing proven state-machine loop pattern. Today every loop is a hand-written
  multi-state machine (head + body + advance); `game_of_life` is fully manually
  unrolled into 16 machines and does not iterate. The proof machinery exists —
  this is front-end sugar, but the surface is a language-design call.
- **[RESOLVED 2026-07-03]** `arr[i] = <binary>` (a computed value into a
  runtime-indexed target) now selects: `arr[i] = arr[i]+22`, `arr[i] =
  arr[j]+arr[k]`, `(arr[i]+22)*1`, even `arr[j]*1` all compile+run (the
  assignment-value operand hoist materializes the read operands; the computed
  store then emits). Canaried: collections/runtime_computed_indexed_write_exit.
  The remaining fenced sources into an indexed target are BARE places only:
  `arr[i] = <bare local>` and `arr[i] = arr[j]` (bare indexed, #38) both hit
  NeedsMachineOwnedWrite -- a BINARY/literal/field source works, a bare local or
  bare indexed read does not. Backend mutation-selection gap (not a frontend
  hoist: hoisting `arr[i]=arr[j]` to a LOCAL doesn't help -- `arr[i]=<local>`
  fails too; a FIELD temp is the working workaround).
- **[ENGINEERING]** numeric intrinsics: min / max / abs / sqrt / clamp DONE.
  abs = `max(x, 0 - x)`, clamp = `min(max(x, lo), hi)` (frontend desugars);
  sqrt = unary float builtin riding the BINARY SSE value-write path with both
  operands = x (x86_64 sqrtsd/sqrtss; aarch64 = clean fence until fsqrt).
  sin / cos remain -- NOT one opcode (no single SSE instruction): they need
  range reduction (mod 2pi) + a minimax/Taylor polynomial whose precision
  matches the interpreter; a genuine numerical mini-project, not a quick lane.
- **[RESEARCH — sidesteppable]** nonlinear index `pixels[y*W+x]` is not provable
  in-bounds (the interval/ordering prover has no product-bound fact). Route around
  with a single linear `0..W*H` counter until/unless a `y<H && x<W => y*W+x<W*H`
  axiom or an octagon domain is added.

### Backend perf (deferred, post-1.0)
The MVP backend (fixed-register, memory-to-memory per op, no regalloc/SSA/
optimizer/SIMD) makes a *real-time* per-pixel renderer slow. Fine for small/simple
demos; a fast renderer waits on the deferred "serious backend" layer (virtual-reg
IR + a linear-scan allocator + a few passes + SIMD selection). Today's bar is
"provably correct native output," which it meets.

### Smaller open items / latent bugs surfaced
- DONE 2026-07-03 (#38, the dual-indexed copy): `arr[i] = arr[j]` (both runtime
  indices) now LOWERS FOR REAL -- new
  `CopyRuntimeMachineIndexedToRuntimeMachineIndexed` instruction composing the
  two proven halves (read the source element off the machine base by j exactly
  as CopyRuntimeMachineIndexedToRuntimeStorage does, store by i exactly as
  CopyRuntimeStorageToRuntimeMachineIndexed does; 68-byte x86_64 encoding, two
  machine-base relocations at +2/+34, distinct index registers rax/r10;
  aarch64 = clean error like sqrt). Selection tries the dual arm BEFORE the
  storage-place source (which mis-resolves an indexed read to the array base --
  the historic silent bug). The #40 stopgap blocker now stands down ONLY when
  the dual copy is actually planned (any uncaught dual shape stays fenced).
  Verified: top-level (canary runtime_dual_indexed_copy_exit, exits 50 =
  element j), IN-LOOP element-wise a[i]=b[i] (runtime_dual_indexed_copy_in_loop_
  exit, sum 70 -- the old classifier fence routes it to the write path where the
  dual arm picks it up), CROSS-ARRAY a[i]=b[j], and an in-place swap; both fail
  canaries converted to run canaries; 550 canaries + samples + differential 11/11
  (native == interp). SIBLING DONE same day: `arr[i] = <bare local>` -- see the capture entry below.
- DONE 2026-07-03 (the CAPTURE-SEMANTICS stack + frame-source write, the
  iter-33 wall bottomed out): `let t = self.v; self.v = 0; nums[i] = t` was a
  SILENT miscompile (wrote the post-overwrite v -- the stale-fold family; same
  root as the OPEN euclid-gcd swap bug). Fixed at the SINGLE deepest layer plus
  its enablers: (1) `simple_local_bindings` (omega-state-values simplify) now
  SKIPS the local->initializer binding when a member field the initializer
  reads is REASSIGNED between the declaration and the use (capture semantics;
  mutation-BEFORE-decl still folds -- the spawn_interleaved don't-over-block
  rule holds); (2) `local_data_requires_storage` allocates the slot for exactly
  that shape (new assignment-VALUE visibility -- the liveness scan never
  counted assignment RHS uses); (3) the write half
  `CopyRuntimeStorageToRuntimeMachineIndexed` accepts a RuntimeFrame SOURCE
  (x86_64: load rax off the frame base, second machine-base reloc at +17,
  width 51; aarch64 clean error). The GCD swap (`let r = a % b; a = b; b = r`)
  now computes gcd(48,36)=12 natively -- the samples/euclid_gcd field-temp
  workaround is no longer required. Canaries
  collections/runtime_indexed_write_frame_local_source_exit +
  control_flow/runtime_captured_local_swap_exit; 552 canaries + samples +
  differential 11/11. SAME-DAY FOLLOW-UP: the TRANSITIVE copy chain
  (`let t = arr[i]; let c = t; let d = c; let b = d > 5` silently read false)
  is closed -- the slot scan follows bare-Name copies transitively
  (local_or_bare_copy_used_as_arithmetic_operand). Canary
  collections/runtime_indexed_local_copy_chain_exit; 553 green. Remaining
  fold-family face: the VALUE-CALL arg paths (deep, separate thread).
- DONE 2026-07-04 (#37, the DIRECT entry-ref-param faces): `transition
  table.firmware_revision == 0` and `self.c = table.con_out` (no manual let)
  used to fold FLAT (slot+field frame read -- silent garbage; the guard face
  compared frame@72). Fixed via the house guard-hoist pattern, NOT type-blind:
  a param's `&Named` type is DECLARED on the state signature, so the
  syntax->resolved lowering records `reference_struct_parameters` per state and
  hoists any member read through one into a `let` (guard subjects, guard
  operands, and bare assignment-RHS roots), which lowers through the
  boot-verified pointee path. Three coordinated rules keep the temp sound:
  (1) hoist_temp_type types it from the referee data's field; (2) the storage
  layer SLOTS ref-param-member locals used afterwards (any position);
  (3) state-values' MATERIALIZATION rule (twin of the capture rule) never
  substitutes one away. `&mut` params excluded (alias slots fold flat
  correctly); `&self` unaffected (SelfType referee). Canary
  targets/efi_ref_param_direct_faces asserts the report shows both pointee
  derefs and no flat @72. 555 canaries + samples + differential green.
  SAME-DAY FOLLOW-UP: the host-call-arg-direct face
  (`out.output_string(table.con_out, ..)`, the last #37 sub-face) is CLOSED --
  it silently flat-folded (frame@72 fed firmware poison into the vtable
  dispatch); the CALL-ARGUMENT lowering now hoists ref-param-member args
  through the same deref-let path (indexed-read args untouched -- their
  call-arg substitution is correct). Canary targets/efi_ref_param_call_arg
  asserts deref + no flat @72 + the dispatch bytes. #37 is fully closed;
  556 canaries + samples + differential green.
- FIXED 2026-07-04 (non-boundary shared `&Struct` param SEGFAULT, latent):
  probing the first-ever runtime use of a shared `&Struct` param in a
  NON-boundary machine (`read(&self.inner)`; the corpus never passed one)
  found an ACCESS VIOLATION: the arg pass spills the pointee's CONTENT into
  the param slot (flat member reads correct), but the m1-era pointee-source
  arms treated the `&Named`-typed slot as a POINTER and dereferenced a field
  value as an address. CONVENTION now enforced in the pointee resolver
  (storage_places.rs): shared `&Named` slots deref ONLY in boundary machines
  (the vouched entry hand-off); `&mut` slots deref everywhere (writes must
  land in caller storage -- the alias-write canaries pinned this during the
  fix); slices are {ptr,len} everywhere. The new frontend/storage/state-values
  ref-param rules are likewise scoped to boundary machines. Run canary
  calls/runtime_shared_ref_param_member_exit (exit 42, differential-covered --
  the first &Struct-param run coverage). 557 canaries + samples + differential
  green.
- u64 literals above i64::MAX rejected at parse (`literals.rs`); const float arith
  in a guard refused (clean error); a tail of value-call corner cases.
- FIXED 2026-07-03 (sum-type FIELD store payload offset): `self.tx =
  Tx::Transfer { to: 3, amount: 40 }` (a variant STORED INTO A MACHINE FIELD)
  then matched read its payload fields shifted (`to`->40, `amount`->0) when a
  payload field name was shared with another variant. ROOT: the case-construction
  field-write loop (writes/mod.rs) tagged payload members `case_variant: None`, so
  a shared-named field resolved to the FIRST variant's field, clobbering a
  sibling; the read/destructure path already tags the variant, so write and read
  disagreed. FIX: tag each payload write with the constructed variant
  (`case_payload_field_variant_tag`; common fields stay untagged). Canary
  control_flow/runtime_sum_field_store_payload_exit; full suite + differential
  11/11 green.
- Same-type contained-machine METHOD-CALL aliasing is a SILENT miscompile,
  re-confirmed on current code 2026-07-03: `a: Counter; b: Counter` +
  `self.b.increment()` mutates `self.a` (dispatch resolves the receiver region by
  TYPE via `machine_storage_offset`, losing which field). Single instance, first-
  instance calls, and DIRECT field access all work; the sound direct-field
  workaround is now locked by `calls/runtime_same_type_contained_direct_fields_exit`.
  Real fix = thread the receiver field offset through dispatch (deep). A precise
  frontend fence (error a method call on a non-first same-type field) is possible
  but CHANGES LANGUAGE SURFACE (rejects currently-compiling code) -- needs a Zach
  decision, not landed unilaterally. See memory `contained-machine-same-type-aliasing`.

## Cathedral first-boot ladder — the concrete freestanding consumer (2026-07-02)

Cathedral has landed **real milestone-1 source** as the driving target for the
freestanding backlog item: `../Cathedral/source/boot/uefi/main.omg` (a UEFI app
that prints "Hello from Omega" via `con_out->OutputString` and returns) +
`../Cathedral/source/contracts/uefi/uefi.omg` (the hand-off ABI as plain `data`
+ boundary traits). It does not compile yet — it is the pressure-test. **The
QEMU/OVMF harness is already verified** on real hardware
(`../Cathedral/tools/boot-harness`, QEMU 11.0 + split OVMF booted to the boot
manager against an empty ESP), so **"done" = that `.EFI` boots under QEMU/OVMF
and the greeting appears on the serial console.** This is the smallest possible
end-to-end freestanding proof, and most of the machinery already exists.

> **STATUS (2026-07-03, reworked to the LONG VIEW):** the boot MECHANICS are
> done and boot-verified; what remains is LANGUAGE READINESS, driven by the
> MINT ARC below (Zach: "the point of the UEFI test is to test our language
> readiness; if we're not there we should get there correctly").

**DONE (each boot-verified under QEMU/OVMF unless noted):**
- **Freestanding emission**: `subsystem efi_application` = empty host ABI plan
  (absence = denial) → import-free PE32+ subsystem-10 with `.reloc`;
  arbitrary-base load verified live ("Image Return Status = Success").
- **Return-to-firmware**: the entry's terminal value lands in RAX (returning 5
  printed "Warning Stale Data"). Known gap: COMPUTED terminals miscompile at
  the entry (place/literal fine; field-bind workaround canaried).
- **THE CANONICAL ENTRY**: `machine Main::run(&self, args: &[u8])` — Main's
  members are the program's statics; `args` is the platform handoff as RAW
  BYTES ({ptr → 32-byte register spill, len 32}); `args.len == 32` returned 5
  under OVMF. `Main::main` accepted + preferred-while-present (7 corpus
  `Main::run` helpers; the retire-main sweep flips precedence later). Typed
  entry params (`main(handle: addr, ...)`) also work (differential boot 4→5).
- **`addr`** (naive pointer-width) + **`utf16"..."`** (parser desugar to the
  integer array literal; the greeting's code units are a differential canary).
- **`call r/m64` encoder** (`append_call_register`, byte-oracle-tested) —
  awaiting the VtableSlot wiring at the top of the mint arc.
- **provides-table PARSE** (Binding sum RHS + identifier-led form) — awaiting
  consumption.

**SETTLED 2026-07-04 (chat, Zach) — THE BOOT PATH RE-CUT: the boundary vouches.**
The entry is an EXPORTED CALLABLE — `boundary machine Main::run(&self,
handoff: EfiHandoff) -> EfiStatus` — whose parameter list is the
boundary-trusted shape over the arrival bytes (recast AT the boundary, never
in code; raw `&[u8]` stays first-class). Vouching is TRANSITIVE through
declared shapes: `handoff.table` is a foreign-backed reference laid by the
Uefi policy, so `table.con_out` reads at a plan offset with NO per-read mint —
the validate-mint is NOT the boot door (it remains the trusted-side promotion
tool, tasks #21/#22, de-scoped from m1). Calling plans are PROGRAMMABLE
(closed placement vocabulary + policy-authored plans, `calling_plans.md` §7);
inference from the image subsystem is the norm, `boundary(<Plan>)` the
override spelling (first customer: interrupt frames, m3). Image facts live in
**build.omg** — an interpreted `machine build(b: &mut Build)` over a ZII
`Build { subsystem: Subsystem (Console|Gui|EfiApplication|Unspecified(u16),
zero=Console); freestanding: bool }` — and the in-source `target { subsystem }`
block DIES (`build_and_package_model.md` addendum).

**THE ENGINEERING LADDER to the greeting (all settled mechanism, in order):**
1. **build.omg v1**: Build/Subsystem types; interpret `build(b: &mut Build)`
   (purity-gated; needs the &mut out-param read-back in build-time eval);
   consumption replaces `resolved_target_subsystem`; target block parse dies;
   ~6 canaries/samples migrate; boot re-verified.
2. **`boundary machine` parse** + exported-callable registration; the entry is
   the boundary machine; shape-fits-arrival check against the subsystem's
   compiler-known handoff (EFI: 16 bytes / 2 registers).
3. **Typed-handoff entry**: `handoff: EfiHandoff` materialized from the spill
   (the landed unmarshal, struct-shaped); `handoff.table` a foreign-backed
   reference param (frame-slot pointer — the existing pointee machinery).
4. **Plan-offset projection**: `table.con_out` lowers to the existing pointee
   read at the Uefi plan's baked offset (zero new backend per the rung-3
   recon; the plan comes from the L4 machinery).
5. **provides CONSUMPTION + VtableSlot call**: parsed Binding table →
   HostAbiPlan for the selected image; `output_string -> VtableSlot(1)` lowers
   onto `append_call_register` (deref this, read slot, marshal MS-x64).
6. **The greeting**: `table.con_out.output_string(&utf16 greeting)` under
   QEMU/OVMF — milestone 1 closes.

**SETTLED 2026-07-04 (Zach) — DOMAIN MINTING = `as`, ALL FACTS PROVEN, NO OUTS.**
The language has NO unsafe / no assert-escape. `as` is the ONLY minter and is
allowed ONLY when the invariant-prover discharges EVERY invariant of the target
domain AT THAT SPOT (constant source = trivial; utf8 = prove the byte sequence
valid; range off a socket = prove it). Can't prove it -> COMPILE ERROR; you
restructure with guards until the proof exists -- there is NO out. Required for
ALL domain minting -- layout, range, behaviour, encoding -- NOT layout-only
(resolves the earlier open sub-q; the `let s: i32 in Saturating = e` and
`let r: i32 [0..=100] = seed` that silently compile today are bugs to fence).
SOLE exemption = a DECLARATION's ZII default: no `as`, but the compiler checks the
ZERO value conforms (a declaration IS a mint of the ZII constant; `[1..=100]` on a
ZII field is an error -- generalizes the ZII-range-excluding-0 ruling). CONSEQUENCE
(load-bearing, not a hole): a domain is UN-MINTABLE at runtime until the prover can
prove its invariant -- literals mint now (`"hi"` = compile-time utf8), runtime bytes
-> `&[u8] in Utf8` is BLOCKED until the prover lifts "every byte guarded valid across
a loop" to "the slice is utf8" (inductive/loop-invariant reasoning). Soundness-first,
accepted even where the proof is hard. NO compiler-generated validate/codec, no
builtin verdict type -- "#22 derived validate" DISSOLVES like encode-as-builtin. The
entire remaining compiler surface = #21: `as` + the invariant-prover's REACH
(single-scalar range = the landed guard-narrowing keystone; whole-buffer/loop = the
hard edge). If you want a Valid|Invalid result, you DECLARE the enum and WRITE the
machine yourself.

**#22 RECON (2026-07-04, probe-verified; checklist in session memory):** the
settled surface ALREADY compiles (`case Valid(view: &[u8] in
OmegaLayout<Schema>)` parses, types, constructs, dispatches). Today's decode is
validate+materialize FUSED (verdict out-param); the split = factor the
plan-walk's CHECKS from its WRITES (wire_decode.rs has both). OPEN DESIGN
QUESTION (needs Zach, not fenced unilaterally): `Checked::Valid { view:
<plain &[u8]> }` COMPILES — user construction trivially claims the refinement,
the same spirit L5 closed for declared storage; either construction-from-
unrefined errors (derived validate = the only minter, needs a privileged
synthesis path) or it stays open until the deriver lands.

**THE MINT ARC (re-scoped: the language ladder, no longer boot-blocking):**
1. **Case-vocabulary Plan — DONE (task #34).** 2. **Plan-walking deriver —
DONE (rungs 2a/2b/2c, task #35):** the wire codec is plan-driven end to end;
an Omega `CompactBinary::plan` policy authors the plan, agreement-gated;
nested child tags plan-driven. Remaining: std-source the policy; retire the
Rust agreement walk. 3. **Foreign-backed views** (recon done): now the
LIBRARY-GRADE sibling of ladder step 3 — `validate`-minted views for bytes
code chooses to check (network/disk), riding the same pointee machinery.
4. **Fact establishment** (#22's second half): `Valid` MEANS every declared
fact holds (CheckWireScalarRange through both encoders, differential).
**Calling plans' maturation** joins the arc: hardcoded MS-x64 → `MsX64` as a
STATED plan with the agreement oracle (the wire-codec playbook; its §6
start-moment arrived with the boot-verified entry stubs) → policy-authored.

**Adjacent, not blocking:** recast (#21) as the args-extraction cleanup;
top-level consts; the free `machine main(sys)` resolution gap; entry
computed-terminal returns; >4-arg platforms.

Milestone 2 (GetMemoryMap → ExitBootServices → first Region mint) needs **no new
language features** beyond these — it is Cathedral-side code over the same
machinery. Milestone 3 (interrupts, serial, staying alive) adds inline asm
beyond `asm { jmp state(...) }` (cli/hlt/port-IO instruction contracts) + the
real interrupt-entry stub (the re-entrant sibling of feature 1). A
`canaries/pass/uefi/` entry pinning "emits a PE32+ subsystem-10 image with entry
`main` that calls through a projected pointer" is the natural done-check once
feature 1 exists.

## Cathedral MILESTONE-2 ladder — own the machine (2026-07-04)

Milestone 1 booted; milestone 2 is the memory-map dance → `ExitBootServices` →
the first `Region` mint (the origin of Cathedral's authority graph). The target
is landed Cathedral-side: `../Cathedral/source/boot/uefi/own_machine.omg` +
`../Cathedral/source/core/region.omg` (the first `Region` + `mint_region`).
**No fundamentally new language features** — it exercises the M1 machinery
(boundary calls, transitive vouching, VtableSlot dispatch, state-machine loops)
in new shapes. The specific asks, smallest-first:

1. **`&mut` params through a vtable call.** `get_memory_map` has five out-params;
   the MS-x64 lowering passes their *addresses* (like `output_string`'s value
   args, but by-reference). A small delta on the VtableSlot arg encoder.
2. **Header-offset vtable dispatch.** BootServices' fn pointers begin after a
   24-byte `EFI_TABLE_HEADER`, so dispatch must reach `*(bs + 24 + N*8)`, not
   `*(bs + N*8)` (con_out was a header-less protocol struct). **Exact spelling is
   the OPEN "foreign vtable dispatch model" decision** (below): `VtableSlot(index)`
   + a base offset, or bind a named fn-ptr *field* of a declared table struct
   (offset computed, header-agnostic). Security identical either way (same
   boundary-gated `provides` dispatch) — this is spelling, low-stakes because
   vtable dispatch is rare (boot tables + FFI shims only; Cathedral steady state
   has none).
3. **Recast at a runtime offset.** `&self.map_buf[offset] as &EfiMemoryDescriptor`
   is the §5b borrow-recast, indexed by a runtime `offset` strided by the runtime
   `desc_size`. Needs the bounds fact `offset + sizeof <= map_size`; recast
   itself is the RECAST arc (already queued).
4. **A larger static buffer field** (`[u8; 16384]`) inside a `data` value.
5. **A machine that never returns** — `own_machine` idles (busy self-re-entering
   state) after exit; `hlt` is milestone 3.

Done-check: boot under QEMU/OVMF, print the greeting, ExitBootServices succeeds,
no crash after exit (the machine is ours and idling). A serial "Owned N MiB"
report is milestone 3 (survives the console teardown).

## Surface spelling cleanups (settled 2026-07-04, Zach)

Vestiges from early spelling, now that `build.omg` and the `::` convention have
settled. Both are mechanical migrations across the corpus + Cathedral source:

- **`::` for static name paths, `.` for value access** (ch14 updated). `use
  a::b::C` and `module a::b` (not `.`); `::` is already used for type-scoped
  machines (`Main::run`), now uniform for all compile-time name resolution;
  `.` is reserved for runtime field/method access. Migrate `use x.Y` /
  `mod a.b` / `pkg.Type` → `::` across canaries, samples, and stdlib.
- **Drop the per-file `package X` line** (ch14 updated). Package identity lives
  in `build.omg` / the directory (one dir = one package = one build.omg); source
  files are members by location and don't re-declare it. Remove the `package X`
  header from source files; the parser stops requiring/accepting it.

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

## Outstanding (pick up next)

> **CURRENT OPEN WORK (2026-07-04, post-MILESTONE-1).** "Hello from Omega" boots
> under QEMU/OVMF (ea51376cd); the whole first-boot ladder is done + sampled
> (`samples/uefi/uefi_hello`). The Cathedral OS work (calling-plan lowering as stated
> policies, hardware facts, milestone 2 = GetMemoryMap/ExitBootServices/first
> Region mint) is a SEPARATE agent's track over this same machinery — coordinate
> before touching the ABI/boundary layer. The compiler-language backlog, by
> leverage:
>
> **Soundness (correctness-first):**
> - **#37 guard-subject deref through entry ref-params** — CONFIRMED silent
>   wrong-read (`transition r.c == 0`, `r: &Struct` flat-folds `slot+field`, no
>   deref). Value/let/mutation routings deref correctly now; only guards miss it.
>   Needs a `StateGuardOperandStorage::Pointee` vertical slice (state-guards +
>   selection + a both-arch deref-compare encoder) OR a routed hard error.
>   Workaround proven (let-bind then guard). Dedicated fire, not loop-sized.
> - **Backend miscompile fences** — a cluster of `clean error` gaps to complete:
>   nested-runtime-indexed write (`grid[*][i]`), array-of-structs as a binary
>   operand (NARROWED 2026-07-03: the `let t = arr[i].field; use t` idiom now
>   works -- local_data_requires_storage recognizes a Member-off-a-runtime-index
>   initializer so the local keeps its slot instead of alias-folding; canary
>   collections/runtime_indexed_field_local_operand_exit. The DIRECT
>   `self.r = arr[i].field + 22` still errors. A frontend operand-hoist of
>   `arr[i].field` was ATTEMPTED + REVERTED 2026-07-03: it is TOO BROAD -- at the
>   pre-resolution hoist layer the field's TYPE is unknown, so hoisting every
>   `arr[i].field` operand breaks STRING/SLICE fields (`arr[i].str_field` concat,
>   indexed writes, slice reads -- ~10 canaries), which have their own operand
>   paths and must NOT be hoisted. Only a SCALAR `arr[i].field` should hoist, which
>   needs a TYPE-AWARE hoist (typed layer) or a backend operand resolver, not a
>   frontend predicate. The `let t = arr[i].field; use t` idiom is the workaround.
>   RE-SCOPED 2026-07-04 (2 ticks of instrumented pinning): the backend fix is PATH B
>   -- a NEW machine-indexed struct-field VALUE OPERAND (`base + i*elem + field_off`)
>   + x86_64 emission, NOT the earlier "contained" hope. Pinned: struct
>   `cells[i].x+5` fails at resolve_runtime_value_operand_in_table's storage fallback
>   (Member unresolvable); plain `nums[i]+5` does NOT use the generic resolver (a
>   third, unpinned path), so machine arrays aren't FrameIndexed operands and the fix
>   can't reuse FrameIndexed -- add the new branch using
>   resolve_runtime_machine_indexed_target_in_table (which already computes the
>   machine Member(Indexed)+suffix for WRITES). The entity/particle pattern is
>   demonstrated working today via the field-temp idiom in
>   samples/cli/collections/entity_list. See array-of-structs-indexing memory),
>   `arr[i]=arr[j]` both-runtime
>   (#38 -- NARROWED 2026-07-03: this is the last BARE-source case into an indexed
>   target; a binary/literal/field source already selects, only a bare local or
>   bare indexed read fails, both NeedsMachineOwnedWrite in the mutation emitter),
>   computed-index `arr[k+1]` double-gate, u64 literals > i64::MAX (i128
>   refactor). Fenced (safe) but block real programs. One-fence-per-fire.
> - **SHIFT in a guard subject** (`transition self.x >> 1 == c`) -- LANDED
>   2026-07-04. The guard value-operand path now threads shifts: `runtime_arithmetic_operator`
>   maps `<<`/`>>`, the value-operand site swaps `>>`->`ShiftRightLogical` when the
>   shifted VALUE (left operand) is unsigned, and `guard_expression_support` allows
>   both in the emit gate. The predicted sound fix landed too -- shifts run at the
>   OPERAND width (not a hardcoded 64-bit) in `runtime_binary_operation_byte_size`
>   on BOTH arches, so a narrow signed `sar` honors the i32 sign bit (`-320>>2==-80`)
>   and `<<` drops i32 overflow. x86_64 verified (canary+differential
>   `arithmetic/runtime_shift_in_guard_exit`); aarch64 correct-by-construction (ASR-W
>   for narrow `>>`), and the same width fix also corrects aarch64 narrow div/mod in
>   guards (previously ran 64-bit, unverified). fail canary shift_in_guard_rejected
>   removed. See bitwise-operators.md.
> - **CAST in a guard subject** (`transition self.x as u8 == c`) -- LANDED
>   2026-07-04, same pattern as shifts. The guard value-operand resolver
>   (guards.rs `resolve_runtime_value_operand_in_table`) gained a Cast arm that wraps
>   the source in a `RuntimeValueOperand::Convert` (mirroring the write-path resolver
>   writes/mutation/value_operands.rs), and `guard_expression_support.rs` allows
>   `Expression::Cast` in the emit gate. The Convert carries its target width, so the
>   guard compare sizes to the cast target automatically -- no byte_size change was
>   needed (unlike shifts). The feared "bare-field cast store miscompiles" was an
>   ARTIFACT of the reverted 2026-07-03 hoist, NOT a HEAD bug (re-probed: bare +
>   domain-annotated cast stores both give 44 for `300 as u8`). Verified x86_64:
>   canary+differential `arithmetic/runtime_cast_in_guard_exit` (narrowing 300->u8,
>   widening signed -4->i64, widening unsigned 200->i32), interp==native. aarch64
>   reuses the existing Convert emit. fail canary cast_in_guard_rejected removed.
>   ZII cleanup: added `PrimitiveType::scalar_byte_size()` (single source of truth;
>   convert_scalar_byte_size now delegates). Parenthesized guard subjects (incl.
>   `(cast) OP x`) parse as of 2026-07-04, and boolean guard nesting is now FULLY
>   done -- And-of-Or (`a&&(b||c)`) lowers via a distribute-to-DNF pass. See
>   boolean-guard-nesting-gap memory.
>
> **Mint arc remainder (library-grade; the boot path used the boundary-vouch
> shortcut):**
> - **#22 validate-mint** — the GENERAL `Schema::validate(&bytes) -> Valid(view)
>   | Invalid` deriver (copy-out cut settled; the borrowed-view half re-opens
>   view-lifetime questions — surface, don't settle, if hit).
> - **#21 recast** — `as` weaken-only re-view + the plan-implication validator.
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
> **Ergonomics / completions:** #26 auto-hoist pure-builtin guard subjects
> -- CORE LANDED 2026-07-03. `transition min(self.a, self.b) == 7 { .. }` now
> hoists the builtin subject into a temp automatically (statement.rs
> `hoist_child` gains a guard-scoped `hoist_builtin_calls` branch; the temp is
> typed by a `Call` branch in hoist_temp_type.rs from the first arg's field
> type). Scoped to min/max/sqrt (abs desugars in; clamp's nested first arg is
> left) whose first arg is `self.<field>`. Effect-free by construction, so the
> effectful-single-eval tripwire stays green -- the constraint that reverted the
> general value-call hoist twice. Canary calls/runtime_min_max_guard_subject_
> hoist_exit. The NATURAL `{ true -> false -> }` PAIR form now works too (#41
> below): hoist_comparison_match_subject hoists the SHARED bool subject once
> (keyed on the syntax subject handle the parser reuses across arms) into
> `let __b: bool = min(..) == 7`, so both arms test one local and the pair pairs.
> Canary calls/runtime_min_guard_true_false_pair_exit (534 suite + differential).
> #41 COMPLETE 2026-07-03: the INDEXED true/false pair (`arr[i] > 5 { true/false }`)
> now works too -- subject_contains_hoistable re-widened to indexed reads once the
> underlying miscompile was fixed (the nested `let __t=arr[i]; let __b=__t>5`
> failed because a comparison operand did not force the local's slot -- fixed in
> the storage layer, 94d306a7e). The subject-hoist hoists the read inside the
> shared temp; `__t` keeps its slot so `__t > 5` reads correctly. Canary
> collections/runtime_indexed_guard_true_false_pair_exit (535 suite + differential).
> sin/cos (numerical mini-project, must match interp); layouts-ladder remainder
> (mint rung, Packed grammar, layout plan-walking deriver).
>
> **value-CALL-in-guard SILENT MISCOMPILE (#40) — still open; 3rd stopgap shape
> ruled out 2026-07-04.** A scalar/bool USER value-call directly in a guard subject
> (`transition self.dbl(5)==11`) silently takes the true arm (not materialized).
> The full fix is deep (gated on effectful-subject evaluation semantics — a Zach
> design call). A validator STOPGAP (silent->clean-error) was tried this tick in
> omega-validation/calls.rs, resolving the callee via arithmetic_domains::
> call_return_type (the PROVEN value-machine resolver; machine_symbols.state does
> NOT resolve a value-MACHINE) + rejecting an integer/Bool return. It correctly
> rejected the bug + allowed the workaround/builtins, but OVER-REJECTED 7 canaries:
> KEY FINDING — the bug is only OBSERVABLE when the transition's arms DISCRIMINATE.
> `runtime_{transition_subject,effectful_subject}_single_evaluation` put an effectful
> bool value-call in a guard with BOTH arms -> the SAME target (testing single-
> evaluation, not discrimination) and legitimately PASS. REVERTED (clean). Next
> shape (do THIS): gate the rejection on true-arm target != false-arm target; also
> explain why the VIEW canary runtime_nested_guarded_reference_returned_slice_
> element_exit was rejected (views should be None-primitive/allowed). Memory
> value-call-in-guard-always-true. Focused session, NOT a tick.
> CLOSED 2026-07-04b: the view canary is a DISCRIMINATING bool value-call guard
> (`self.should_enter()==true { true->enter _->skip }`) that passes COINCIDENTALLY
> (should_enter returns true, so always-true agrees with intended-true). A value-call
> guard designed to be TRUE passes under both correct + buggy emission -- so the
> corpus is full of coincidentally-passing discriminating value-call guards (BOTH
> dungeon binaries included). Therefore ANY sound stopgap forces rewriting all of
> them to bind-to-local, incl. the flagship dungeon -- a POLICY decision (is a
> coincidentally-correct value-call guard an error?) + an invasive migration, not an
> autonomous tick. SURFACE TO ZACH: "should a scalar/bool value-call directly in a
> guard subject be a hard error until the backend materializes it?" If yes = a
> mechanical validator gate + corpus sweep; if no = the silent miscompile waits for
> the deep effectful-semantics fix. Stop re-attempting the stopgap solo.
>
> **GENERICS / MONOMORPHIZATION -- phased plan (recon a87aee3d, 2026-07-04).**
> Today: type-check-only. Stage-1 monomorphization (typed-trees-to-checked-trees/
> monomorphization.rs) infers args at return/param position + substitutes IN
> PLACE; the LAYOUT builder (omega-layout/builder.rs ~760) keys per-DEFINITION
> and POISONS on a 2nd distinct instantiation (`Box<i32>` + `Box<bool>` in one
> program = the poison invalidates the record -> clean "needs lowering" error).
> Sizes are computed correctly per-use; only per-instance IDENTITY is missing.
> The fence: fence_generic_value_callee (validation/calls.rs:801). Discovery
> PRECEDENT to mirror: plan_laid.rs (pre-resolution synthesizes `Policy<Schema>`
> instance records + rewrites field spellings). No new arena types needed --
> follow plan_laid's slug-named synthetic instances (ZII: synthetic symbol +
> per-instance DataLayout keyed by (definition, canonical-args)).
>   - **Phase 1 -- generic DATA, scalar T -- DONE (75c445a49).** A pre-resolution
>     desugar (pipeline/generic_instances.rs, plan_laid shape + parameter
>     substitution): synthesize a distinct concrete record per `Base<Args>`
>     field spelling, rewrite the field ref to its plain name -> downstream sees
>     ordinary records, poison gone. PURELY ADDITIVE: skips generic enums,
>     non-plain-Named args, param-nesting fields, and method-bearing generics
>     (containers = Phase 2). Canary runtime_generic_two_instantiations_exit
>     (Box<i32>+Box<bool>). DOMAIN-ARG EXTENSION DONE: a `Named` carrying only
>     nameable domain constraints (`Box<i32 in Wrapping>`, `Store<u8 in Utf8>`)
>     now slugs distinctly (type_reference_slug/constraint_slug) so two
>     domain-differing instances coexist; the substitution rides the argument's
>     own type reference so the domain follows the field for free. Canary
>     runtime_generic_domain_instantiations_exit (Box<i32 in Wrapping>+Box<u8 in
>     Wrapping>, exit 42). TYPE-POSITION POLISH DONE 2026-07-03: the desugar now
>     scans machine-body `let`-local, state PARAMETER, and RETURN type positions
>     too (not just data FIELDS) via consider_generic_spelling, so two
>     instantiations as let-locals no longer poison. Canary
>     runtime_generic_let_local_instantiations_exit (Box<i32>+Box<bool> let-locals,
>     exit 30). REMAINING Phase-1: composite ARGS (`Box<[i32;4]>`, `Box<&T>`,
>     range-bounded) still fall through.
>   - **Phase 3 -- NESTED generic data -- DONE 2026-07-03.** `Pair<T> { a: Box<T> }`
>     used as `Pair<i32>` synthesizes `Box<i32>` too. The desugar now runs to a
>     FIXPOINT (collect_type_reference_positions each round; a synthesized
>     `Pair<i32>` has a fresh `Box<i32>` field the next round monomorphizes);
>     base_is_fully_monomorphizable accepts a nested-generic field of a KNOWN base
>     with parameter-or-concrete args; substitute_member builds the concrete
>     `Box<i32>` spelling; and generic TEMPLATE bodies (defs/machines with type
>     params) are SKIPPED so the param-arg `Box<T>` inside `Pair<T>` is not
>     mistaken for a concrete instance. Was an outright error before ("references
>     unknown data type T"). Canary runtime_nested_generic_instantiations_exit
>     (Pair<i32>+Pair<bool>, exit 30).
>   - **Phase 2 -- generic MACHINES**: synthesize one Machine per instantiation
>     (copy states, substitute T); value-call targets rewrite to the synthetic
>     symbol; StateKey.machine carries it automatically; the fence becomes a
>     no-op. Canary: `machine id<T>(x:T)->T` called at i32 + bool. STATE VERIFIED
>     2026-07-03: `id<T>` called at BOTH i32 and bool is cleanly FENCED today
>     ("a value call to the generic machine `id` is not supported natively yet"),
>     NOT a silent poison -- monomorphization.rs substitutes IN PLACE + clears the
>     param list, which only covers the single/agreeing-instantiation case. So
>     Phase 2 = machine deep-clone-per-instantiation at the typed layer (fresh
>     symbol, substitute, retarget each value-call site) -- a real multi-file
>     slice, not a fence-flip.
>   - **Phase 3 -- nested generics** (`Store<T>` containing `Item<T>`) -- DONE
>     2026-07-03 (fixpoint desugar; see the Phase-3 note above).
>   - **CONTAINERS (generic data + attached method) -- valid-but-unimplemented;
>     silent-0 at RUNTIME (corrected 2026-07-03; my prior "parser gap" note was
>     from the WRONG syntax).** The CORRECT method syntax is T-on-METHOD:
>     `machine Box::stored<T>(&self)->T` (like the corpus's `Main::id<T>`), which
>     ATTACHES to Box fine (attached_data=Box). `Box<T>::stored` (T-on-SCOPE) is a
>     separate/unsupported spelling that parses attached_data=null. With the
>     correct syntax, `Box::stored<T>` used as `Box<i32>` + `self.b.stored()`
>     compiles and RUNS but the method returns ZERO -- a silent miscompile at
>     runtime (the DATA monomorphizes but the METHOD stays a generic machine whose
>     T-typed value-call result is never materialized, the #40 class). SCOPE OF THE
>     FENCE: a desugar-level "reject all container instantiations" is TOO BROAD --
>     containers are used TYPE-CHECK-ONLY today (stdlib `Vec<T>` in the borrow
>     canaries, e.g. vec_view_invalidated_by_push, which reach borrow-check not
>     runtime), so it pre-empts those and masks their real check; a fence keyed on
>     data_with_machines was built + reverted TWICE this arc for exactly this. The
>     narrow #40 fence (if wanted) is at the VALUE-CALL/codegen: a T-returning
>     value-call to a generic container method (statement calls / slice-returning
>     `as_slice` are fine). REAL FIX = Phase 2 (implement the container runtime):
>     clone the attached machine with T substituted when synthesizing the data
>     instance -- pre-resolution-tractable (the `Box<i32>` spelling determines T,
>     unlike free `id<T>` which needs call-site inference) via a substitution-aware
>     extension of syntax_trees.rs copy_machine/copy_state/copy_type_reference.
>   - **Phase 4 -- generic trait conformances / containers**: `Store<T> satisfies
>     Container<T>`. DESIGN QUESTION (Zach, when reached, far off): generic-trait
>     dispatch = static specialization (one stamped impl per instance) vs a
>     vtable. Phases 1-3 need no such decision.
>   - **Phase 5 -- const type params** (`Vec<T, N: u32>`): extend the
>     FixedArrayLength::ConstParameter machinery to data/machine params.
>   Implementation choices (mine, not design): synthetic-name slug scheme
>   (follow plan_laid); instance identity = synthetic SymbolHandle (no new key
>   struct).

<details><summary>Historical snapshot (2026-06-19/22 wave — kept for provenance)</summary>

Snapshot refreshed 2026-06-19. Decisions 8-17 implemented (stage 1+); harness
canary suite 303, differential oracle fully matched (11), `cargo test
--workspace` green, tree clean. Ordered roughly by leverage.

> **Audit reconciliation (2026-06-22).** Verified against git + a full green
> `cargo test --workspace`: canary suite is now **321** (the 621-pass/271-fail
> corpus + differential oracle pass). Since the 06-19 snapshot the Rust compiler
> **moved to `compiler/omega-rs/`** (`f3ee813b`, 06-21) — the bare crate names
> below are unaffected, but `wiki/architecture/repository_layout.md`'s tree is now
> stale. Status drift to fold into the bullets below:
> - **encoding domains #66** — Phase C (carrier+name resolution, `&[u8] in Utf8`
>   operator set) and **Phase B1a** (string literal → `[u8]` coercion) **landed**;
>   remaining is **Phase B2–B4** (the ~185-file corpus migration + retiring
>   builtin `string`/`String` + 15 sites — the big-bang, best in a worktree).
>   Verified, mechanical **execution recipe** (keystone + the 15 sites + order +
>   allocator-cleared rationale):
>   [wiki/architecture/string_retirement_execution.md](wiki/architecture/string_retirement_execution.md).
> - **ch15 / decision-18 fact catalog** (#60–#64: facts on sum-case payloads,
>   plain struct fields, construction values, field-assignment enforcement,
>   modular return-range inference) **landed**; remaining is the `abort` effect
>   (#65, ch16-gated) and the recoverable-error propagation arms.

Also closed this wave (later commits): S4 result-interval narrowing for
modulo/div + min/max clamp (14978462/5c7d308e); a native miscompile where a
min/max value-call result fed into arithmetic dropped its write (0e4a88d3);
and **exact-arith enforcement on transition-arm arguments + dominating-guard
narrowing** (ce4cb71b) -- exact-by-default is now UNIFORM (no transition-arg
hole). The S4 narrowing item below is therefore CLOSED for within-state +
co-located-guard sources; only true cross-state param narrowing remains (no
consumer). Known low-pri gap: `(elem as T in Wrapping)` domain-cast of a slice
element miscompiles natively (#59; workaround = element-type Wrapping).

**Closed since the 2026-06-14 snapshot (2026-06-19 wave):**

- **Atomic RMW → real atomic instructions — DONE (#27, both ops, both arches).**
  `fetch_add` → `lock xadd` (x86_64) / `LDADDAL` (aarch64); `compare_exchange` →
  `LOCK CMPXCHG` / `CASAL`. Detected by TREE-SHAPE at the binary-write selection
  site on an atomic-typed target — no frontend churn, parser desugar unchanged.
  `samples/cli/systems/atomics_cross` + a canary byte-verify the aarch64 LSE ops and run the
  host (exit 70). Cross-thread observability still waits on the scheduler (values
  oracle-matched). Commits af2cf360 / 598e5f38 / cf7ab02f / 1a146b1c.
- **Exact-arithmetic overflow proof-check — DONE (decision 17 S1-S3).** Unprovable
  integer `+ - *` is a compile error by default
  (omega-validation/src/arithmetic_domains.rs Interval engine); `T in
  Wrapping/Saturating/Trapping` are the opt-ins (spelling settled as DOMAINS, not
  policies). Native + interpreter agree. S4 narrowing is the remaining refinement
  (below).
- **Zero-copy borrowed wire decode — DONE as `&[u8]` (#43/#46/#47/#49).** Borrowed
  wire text/bytes unified onto `&[u8]` (the `&string` wire type was RETIRED, per
  ch8 "text is not a type"); encode + decode + index + `.len` work natively on both
  arches, oracle-matched. Owned-bytes decode (copy out of the buffer) stays
  allocator-gated.
- **Pending-canary backlog purged.** All 5 `canaries/pending` entries were stale
  duplicates of already-passing canaries; deleted (f6ec39e7). ACTIVE_PENDING_CANARIES
  empty.

**Closed in the 2026-06-14 wave:**

- **Scalar-width re-derivation remediation — DONE.** `RuntimeValueOperand::Binary`
  now threads a resolved `byte_width` (set once at build, read by the x86_64
  float emission); `classify_scalar_value_type_in_table` now types a `Cast` as
  its target so nested casts resolve. Closed the whole f32 miscompile family
  (3 pending canaries promoted to pass RUN canaries) + the sequential self-field
  RMW stale-fold (verified already fixed, promoted). `ACTIVE_PENDING_CANARIES`
  is now EMPTY. Remaining named re-derivation sites are unbitten; fix the same
  way if a canary surfaces one.
  [wiki/architecture/scalar_width_rederivation_smell.md](wiki/architecture/scalar_width_rederivation_smell.md).
- **Decision 11 residue — DONE** (was already fixed @5d6464cf; canary promoted
  to `fail/generics/machine_bound_value_call_unchecked` + a satisfied pass
  companion).
- **Versioned<T> / ch21 reconciliation — DONE** (analysis only):
  [wiki/architecture/versioned_data_stage3_reconciliation.md](wiki/architecture/versioned_data_stage3_reconciliation.md).
  The Decided Model repositions `Versioned<T>` to the WIRE-DATA era matcher;
  live-state hot-swap is net-new `Upgradable<Old,New>` + a `replace` plan (do
  NOT extend the era container into it). decision 14's text below still wants a
  maintainer reconciliation pass.

**Open remaining work:**

- **Encoding domains / string retirement (ch8; design settled 2026-06-20) — CORE
  DONE + PROVEN, migration in progress (executing C → B).** Settled model: NO
  `string`/`String` keyword, transparent `{container} in Utf8`; domains stay
  STORAGE-BOUND (re-declared per carrier, e.g. `domain [u8]::Utf8`, sharing
  machinery — no storage-less form); `&[u8] in Utf8` is the borrowed-view common
  case (OS-canonical `ptr+len`); owned `Vec<u8> in Utf8` deferred to the allocator;
  string literals → static `&[u8] in Utf8`. See memory `string-encoding-domain-model`.
  DONE + canaried + pushed: the `Domain` constraint (parse + validate), parameter
  enforcement via an implicit `requires <param> in Domain` desugar, fact-forwarding,
  the `Slice<u8>`/`[u8]` alias, slice-carrier domain targets (`domain [u8]::Utf8`),
  and the `from_utf8` grant validator. REMAINING:
    - **(C) Mechanism completion:** tighten domain resolution to match CARRIER+name
      (not name alone — latent unsoundness once two same-named domains coexist); a
      fuller `&[u8] in Utf8` operator set (len/byte/range reuse slice ops; a FALLIBLE
      Utf8 char-boundary slice); the `NoNul` domain. Returns/fields enforcement needs
      the entailment to PROVE membership at non-call sites (the `ensures`-desugar is
      vacuous; #66 commit d51273c6) — separate from the param path.
    - **(B1) Keystone:** string LITERALS lower to static `&[u8] in Utf8` views (today
      typed owned `String`, expression_types.rs:160) — a backend/lowering change.
    - **(B2-B4) Corpus sweep + retirement:** migrate the ~185 literal-using `.omg`
      files, then retire the builtin `string`/`String` PrimitiveType + its ~16
      special-case compiler sites. ~3-6 files use owned-growable ops
      (`push_str`/`with_capacity`) — those stay ALLOCATOR-GATED until `Vec<u8>`.
- **`abort` effect (ch15 stage 3, #65) — ch16-gated.** The contagious capability
  already exists as `process_exit` (infers bodies-up, forced at boundaries, in the
  manifest). The only-new-part — a nuclear no-cleanup abort distinct from graceful
  exit — is meaningless until drops (ch16) exist. Revisit with ch16.
- **S4 arithmetic-domain narrowing (refinement, not a correctness gap).** ~30
  corpus ops are pinned to `Wrapping` ONLY because the prover can't yet narrow
  their operand ranges; flow-sensitive narrowing (dominating guards, loop bounds,
  contracts, range types) would return them to Exact. SOUNDNESS-CRITICAL: every
  narrowing fact must be enforced at its source, never trusted — the #40
  return-range scar was trusting a *declared* range without enforcing the callee
  respects it (an unsound miscompile; reverted, then re-landed with enforcement).
  The flow-sensitive ValueEnv exists (got the count 44->30). First safe increment
  = **dominating-guard narrowing** (the guard IS the runtime enforcement, so it's
  sound by construction). aarch64 Sat/Trap is already at x86 parity. This is
  automation-engine work — see the proof-engine north star (long-view below).
- **Recoverable-error / failure model (ch15) — DESIGN SETTLED (decision 18,
  2026-06-19); implementation pending.** All three formerly-open questions
  resolved: (a) propagation is an explicit arm targeting a state that returns the
  caller's own failure — verbose by design, NO `?`/`fails`/sugar; (b) cross-call
  propagation is modular/contract-mediated (prove `requires`, assume `ensures`),
  contracts inferred intra-unit + written at boundaries; (c) the "fallible fact"
  is subsumed by the UNIFIED FACT CATALOG (success case's `ensures` inherited by
  the handling arm). Also frozen: no trap category for logic (compile errors, not
  runtime traps); `Trapping` arith is the only opt-in runtime trap (done); no
  `expect`/`unwrap` (prove the failure dead); deliberate death is the contagious
  nuclear **`abort` effect**; host failure returns a sum (no out-param results);
  failure cleanup = ordinary per-edge drop set (ch16). The verbose model already
  has a canary (`errors/fallible_result_data_shape`). Implementation arc, by
  leverage: (1) **facts on sum cases** — `ensures Case.field in <range>` parsed +
  carried into the handling arm by the existing decision-17 narrowing engine (v1
  fact-kinds: which-case + interval + slice-length); (2) **modular contract
  inference** (infer `ensures` for non-exported machines; require at boundaries);
  (3) **`abort` effect** — declare + propagate through callers/boundaries, lower
  to `exit`/`abort` syscall. Unblocks clean concurrency cancellation (rides this
  channel — decision 16).
- **Versioned decision 14 maintainer reconciliation:** update decision 14's
  frozen text + versioning.rs provenance to the wire-data role once ch21
  settles (chapter is the authority; it is being actively edited).

Long-view sign-offs still open (only the maintainer): S1-S6 (separate
compilation -- the big backend revamp, untouched), M1-M6 beyond build-time evaluation
stage 1, A1-A5 beyond allocator stage 1. The next major VERTICAL SLICE is
CONCURRENCY (decisions C1-C5 frozen, briefs/concurrency_atomics.md; the atomics
foundation is now done) — gated on the ch15 error model above for cancellation.
The long-range proof-engine direction (obsolete SPARK near-term, Lean long-term;
automation-front-line + trusted-kernel backstop + quantifiers) is its own brief:
[wiki/design_briefs/proof_engine_north_star.md](wiki/design_briefs/proof_engine_north_star.md).

---

Earlier snapshot (2026-06-10 wave, decisions 8/9/10; suite 179).

**Decisions needed (sign-off register, 2026-06-12).** Every vertical slice
below is complete; what remains is gated on these maintainer calls. Each
points at the bullet carrying the full proposal:

1. **`Versioned<T>` container** — DECIDED 2026-06-12 (frozen decision 14):
   permanent builtin template type; u32 era; union-of-eras payload storage;
   `era` read-only source-queryable; incomplete-chain = report verdict, not
   error; paren arm form binds the whole historical value
   (`Counter::v1(old) ->`). Stage 3a/3b unblocked.
2. **Argumented ranking-view spelling** — DECIDED 2026-06-12: the use-site
   subtraction (`decreases limit - index`) is rejected as permanent surface;
   BUILD the argumented view `decreases (index, limit) ->
   Nat::BoundedDistance` (tuple form: the arrow's left side is uniformly
   the ranked subjects) and RETIRE the subtraction spelling once it lands.
   See the Measures bullet for the grammar-surgery scope.
3. **Call-output borrows** — DECIDED 2026-06-12 (frozen decision 15): adopt
   the RUST MODEL wholesale — lifetime parameters with the tick spelling
   (`machine header<'buf>(buffer: &'buf [u8], ...) -> &'buf string`),
   aggressive elision (one ref input → output borrows it; `&self` → self),
   borrow-carrying data IN-MODEL (`data ChatMessage<'buf>`), descriptive
   lifetime names as house style. Unblocks zero-copy wire decode +
   view-returning machines.
4. **Long-view arc priority** — RESOLVED 2026-06-12: all four arcs were
   scouted in parallel; the briefs live in wiki/design_briefs/ and their
   maintainer decisions are the register below.

**Decisions needed (scout round 2, 2026-06-12).** Four design briefs in
wiki/design_briefs/ (concurrency_atomics, separate_compilation, build-time evaluation,
allocator_story). Each question one line + the scout's recommendation;
sign-off freezes them.

CONCURRENCY (briefs/concurrency_atomics.md):
- C1 DECIDED 2026-06-12 (supersedes the scout's `yields` proposal): NO
  suspension keyword and NO await. Waiting originates ONLY at boundary
  wait primitives (a `Scheduler` boundary trait: host targets bind
  futex/WaitOnAddress syscalls; Cathedral userland binds the scheduler
  capability; the Cathedral kernel implements it over hlt/interrupts).
  `suspend` is an INFERRED transitive effect (decision-12 machinery),
  declarable on signatures and checked like any effect; awaiting = calling
  (the task parks inside the callee; frames are planned storage, so a
  parked task is just data — no Future reification needed). Enforcement,
  not vigilance: borrows may not live across a suspend-effect call site;
  effect ceilings forbid `suspend` where parking is illegal (ISR
  contexts); atomicity is DERIVED (a state calling no suspending machine
  runs uninterrupted). Artifacts surface all suspension points.
  Follow-on decisions from the same discussion (2026-06-12):
  - C1a SCOPED SPAWNS, no keyword: the lexical block IS the scope. A spawn
    borrowing parent locals holds ordinary loans, so the join must occur
    before the block ends; dropping a `Join<T>` JOINS (blocks), so an
    unconsumed handle joins implicitly at scope end. Free-floating spawns
    stay move/copy-only. DECIDED.
  - C1b TASK STORAGE: no stack sizes exist — no general recursion + planned
    frames mean the compiler computes each spawned machine's EXACT
    worst-case storage M; pools are per-machine-type M x N slots (declared
    N; overflow is a proof obligation or boundary failure). Region-backed
    dynamic N later (allocator arc). Overflow-impossible by construction.
    DECIDED.
  - C1c ATOMIC-STATE GUARANTEE is derived and documented precisely: a state
    body that calls no suspending machine cannot have ITS TASK parked
    mid-body. It is NOT mutual exclusion (other tasks run on other cores;
    cross-task safety = ownership/[send]/atomics). The language stays
    scheduler-agnostic (Cathedral may preempt; guarantees come from
    ownership, not non-preemption). DECIDED.
  - C1d CANCELLATION IS A VALUE AT THE WAIT (proposed, pending ch15
    alignment): no unwinding exists, so a cancelled scope makes each
    child's current/next wait return the zero case (`Cancelled`) instead
    of ready; the machine transitions to its own cleanup path and drops
    run as frames retire. Never interrupts mid-state. A never-suspending
    task is joinable but not cancellable (its effect surface says which).
    Cancellation rides the SAME propagation channel as ch15 recoverable
    errors, whatever that lands as.
  - C1e WAITABLE SURFACE IS FUTEX-SHAPED AND SINGULAR: one primitive
    (wait on word / wake N) — mutex, condvar, channel, join, timer are
    library above it; interrupts and IO completions POST TO WORDS. The
    anti-Linux-sprawl rule: no second wait mechanism, ever. DECIDED.
  - C1f SELECT DISSOLVES: no select construct. Multiplexing is data-level —
    producers post into ONE mailbox carrying a case-bearing sum
    (`Event { case Packet(...); case Tick; ... }`), the consumer does one
    wait and one ORDINARY transition over the sum (Erlang's one-mailbox
    model; already Cathedral's IPC-ring shape). Deferred work shrinks to a
    core MPSC event-queue library on the wait primitive. DECIDED.
- C2 Unit of concurrency: spawned machine = one task, per-task frame
  discipline now (separate-compilation-ready). ACCEPTED 2026-06-12.
- C3 Cancellation: structured Join SCOPES (scope drop cancels children,
  deadlines attach to scopes). ACCEPTED 2026-06-12.
- C4 Sharing: atomics-only at language level; `Mutex<T>` is a core-library
  type over atomic spin-locks, never a primitive. ACCEPTED 2026-06-12.
- C5 Atomics + model: compiler intrinsics, five C11 orderings, C11 memory
  model wholesale. ACCEPTED 2026-06-12. Concurrency is the chosen next arc
  (unblocks the most: wait primitive, Mutex, scheduler, IPC ring all build
  on atomics). First slice = atomics foundation (see Next Up).

SEPARATE COMPILATION (briefs/separate_compilation.md):
- S1 Component = PACKAGE; artifact = sealed IR + boundary manifest +
  layout/wire reports (.o format follow-up). REC: yes.
- S2 Linking: hermetic static composition phase first; loader-time
  relocation deferred to Cathedral's loader. REC: yes.
- S3 Cross-package monomorphization: REJECT in stage 1, resolve at
  composition time in stage 2. REC: yes.
- S4 Cross-component ABI: compiler-ENFORCED public layout reports +
  wire-data contracts for evolution edges; host ABI reused for calls.
  REC: yes.
- S5 Dispatch: keep ONE fused loop with per-component entries + import
  tables (never split per component). REC: yes.
- S6 The composition/linker tool is OMEGA's (Cathedral consumes it).
  REC: yes.

BUILD-TIME EVALUATION (briefs/build_time_evaluation.md):
- M1 Purity gate: reuse decision 12's inferred transitive effect surface
  (empty effects + no &mut/out = const-evaluable). REC: yes.
- M2 Reflection access spelling: bracket form `self.[field]`;
  `Self::fields` exposes names + types only in stage 1. REC: yes.
- M3 Termination: NO new rule — const-evaluable machines inherit the
  language's existing termination discipline (general recursion does not
  exist; self-calls are tail self-loops; loops carry decreases/measures).
  Fuel at most as a defense-in-depth backstop against checker gaps.
  (Maintainer-corrected 2026-06-12; the scout's self-recursion framing was
  Rust-shaped.) REC: yes.
- M4 First const position: fixed-array lengths; TARGET-width emulation in
  the const evaluator is mandatory from day one. REC: yes.
- M5 Generator bodies must expand to effect-free machines (build-time code
  is declarative only). REC: yes.
- M6 equatable.rs is TEMPORARY: stage 2 rewrites Equatable as a core trait
  generator and retires the hand-rolled path. REC: yes.

ALLOCATOR (briefs/allocator_story.md):
- A1 No ambient heap ever; allocation is an explicit capability. REC: yes.
- A2 The allocator surface is named `Region<'r>` (over `Arena`), bound
  through the frozen `Allocation` provider category. REC: yes.
- A3 Failure semantics: proof-obligated capacity (`requires len <
  capacity`); `try_push -> Result` optional later; no silent traps.
  REC: yes.
- A4 Vec ladder: stage 1 fixed-capacity (no allocator at all); stage 2
  `Vec<'r, T>` borrows a Region, capacity fixed at construction, NO
  growth; pluggable allocators only if demand appears. REC: yes.
- A5 Drops: elements drop immediately; the Region frees memory in bulk
  (cleanup and memory release are separate concerns). REC: yes.

Smaller wire remainders (repeated fields, arbitrary-depth nesting,
encoding families, negotiation) are derivable from decision 10 + the
landed framing without sign-off. Language-design open questions with no
implementation pressure stay in the guide's appendix "Still Open".

**Wave landed 2026-06-12 (round 1 of the execution sweep; suite 234/234,
oracle fully matched, `cargo test --workspace` green):**
(a) RANKING VIEWS: `decreases (index, limit) -> Nat::BoundedDistance` built;
the use-site subtraction is retired with a guided error; 7 corpus files
migrated. (b) LIFETIMES STAGE 1: elision-only output-borrow linkage — a
free-machine view return now links to its single ref input (closing what
was an actual silent soundness hole, not a conservative rejection);
`&self` rule unchanged; two-plus ref inputs + view output errors
("explicit lifetime parameters are not implemented yet"). (c) VERSIONED
STAGE 3: `Versioned<Counter>` synthesizes an ordinary data definition
(era u32 + per-era payload fields), version match arms desugar to era tag
compares (paren whole-value binding), `era` reads, writes rejected, plain
subjects suggest the container; chain-completeness verdicts in
04_wire_protocols.txt. KNOWN DEVIATION: payload layout is a STRUCT (sum of
era sizes), not the frozen union-of-eras max layout — unobservable until a
boundary decoder mints non-zero eras; true union layout lands with the
decoder (stage 4). (d) WIRE REPEATED FIELDS: `N: name: [scalar; max];`
packed LEN-delimited, max-unrolled self-guarded ops both ISAs, count
companion `name_count`, hostile counts rejected by the Open/Close bound
discipline. (e) BUILD-TIME EVALUATION STAGE 1: `[T; table_size()]` const-evaluates
zero-arg effect-free machines via the reference interpreter
(orchestration-layer pass pre-checking; decision-12 purity gate; 100k fuel
backstop; target-width audited). (f) TEST DEBT: cargo test --workspace
compiles + passes everywhere; architecture_boundaries 6/6.

**Implementation, design already frozen:**

All three frozen decisions (11, 12, 13) landed 2026-06-11 — see the wave
notes under Next Up. Decision 11's formerly-accepted hole (place==place on
a payload-bearing sum slipping through as a tag/width compare) is now
CLOSED for typable operands by Equatable synthesis: conforming types expand
structurally, non-conforming structural types error with the conformance
suggestion (operands the state typing scope cannot type — e.g. inside
contracts — still slip through). Decision 13's residue (machine-call
monomorphization arguments not bound-checked; generics-completion arc)
remains tracked in its bullet below.

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
- [ ] **Wire stage 2: encoders + decoders.** STAGE 2a LANDED (2026-06-11):
  era assignment along the version chain (decision 10; queryable on the
  typed `WireSchema`, surfaced in `04_wire_protocols.txt`), the synthesized
  `Schema::encode_wire(&value, &mut out, &mut written)` encoder for
  primitive integer fields (i32/i64/u32/u64/bool; other types reject), and
  compact_binary v0 framing (era varint, then per field a tag varint +
  value varint; LEB128, zigzag for signed, bool 0/1) -- lowered as two
  dedicated wire-append operations on BOTH aarch64 and x86_64 (cursor lives
  in the `written` slot; widths/relocations in pinned lockstep), with
  byte-identical native interpreter support and byte-exact run canaries in
  the differential oracle. STAGE 2b LANDED (2026-06-11): the current-era
  decoder `Schema::decode_wire(&mut value, &buffer, &mut read, &mut ok)` --
  expected-byte reads for the era discriminator and field tags plus a
  bounds-checked LEB128 value read per field (un-zigzag for signed), as two
  dedicated wire-read operations on BOTH ISAs (cursor in the `read` slot,
  STICKY failure flag in the `ok` slot: wrong era / unexpected tag /
  truncated / overlong varint fail cleanly, every read bounds-checked
  against the buffer's compile-time length; widths/relocations pinned),
  interpreter parity including the failure path, and round-trip +
  wrong-era-rejection run canaries in the differential oracle. STRING
  FIELDS, ENCODE-ONLY, LANDED (2026-06-11): a String field rides as tag
  varint + LENGTH varint (byte count) + raw UTF-8 bytes (no NUL, no
  padding), lowered as one new `AppendWireTextBytes` operation on BOTH ISAs
  (loads the `{ptr, len}` text descriptor, reuses the scalar LEB128 emit
  loop for the length, then a byte-copy loop that bounds EVERY store
  against the out buffer's compile-time capacity and drops overflow --
  widths/relocations pinned, byte-exact run canary in the differential
  oracle). Validation allows at most ONE String field and requires it to
  carry the highest field number (it encodes last) so every earlier append
  keeps the compile-time worst-case capacity guarantee; the worst case
  budgets the String's tag + ten-byte max length varint. String DECODE
  stays rejected -- the honest options were (a) zero-copy (descriptor
  pointing into the decode buffer) or (b) reject, and we took (b) because
  today's borrow facts only track view loans from explicit borrow
  expressions: the checker cannot see `decode_wire(&mut value, &buffer,
  ..)` leaving `value`'s String field aliasing `buffer`, so buffer
  mutation after a zero-copy decode would silently invalidate the decoded
  string -- a KNOWN HOLE to close before (a) lands (borrow-facts follow-up:
  model a call output retaining a borrow of another argument; RULED
  2026-06-12, frozen decision 15: the Rust lifetime model is adopted, so
  zero-copy String decode is mechanical once lifetimes are implemented:
  read len varint, bounds-check against the remaining buffer, store
  `{buffer_base + cursor, len}`).
  Encode also has no runtime overflow signal (content past capacity is
  dropped; callers size buffers for their longest text) -- an encode
  ok/overflow out-parameter is candidate follow-up work. NESTED MESSAGE
  FIELDS LANDED (2026-06-12), one level deep, scalar-only child bodies: a
  field whose type is a sibling wire schema rides as tag + byte-LENGTH
  varint + the child's tag/value pairs with NO era discriminator (decision
  10: one era varint per top-level message, never per struct). The actual
  length is runtime-sized (varints), so the encoder two-pass STAGES the
  sub-message through a planner-reserved frame scratch region shaped as a
  `{ptr, len}` text descriptor + worst-case staging buffer, then replays it
  through the existing `AppendWireTextBytes` (length varint + bounded copy)
  -- ZERO new encode operations; capacity math composes (parent worst case
  counts tag + length varint + child worst case), and the one-String-LAST
  rule is per message scope (child bodies have no String today). The
  decoder reads the length into the scratch slot, then two new loop-free
  operations on BOTH ISAs (widths/relocations pinned):
  `ReadWireNestedOpen` (absolute end bound = cursor + length, checked both
  as raw length and as bound against the buffer so a huge length cannot
  wrap the 64-bit sum) and `ReadWireNestedClose` (sticky ok fails unless
  the cursor lands EXACTLY on the bound). Schema cycles (no finite worst
  case) are hard errors at the declaration
  (wire/nested_schema_cycle); String-in-child and nested-in-nested reject
  at the call (wire/encode_nested_in_nested); round-trip + corrupted-length
  run canaries with hand-computed bytes in the differential oracle
  (wire/runtime_wire_roundtrip_nested_exit,
  wire/runtime_wire_decode_rejects_bad_nested_length_exit), interpreter
  parity included.
  Remaining: historical-era decode via `Versioned<T>` (after the stage 3
  sign-off), String decode (above), arbitrary-depth nesting (needs
  per-level staging regions), repeated fields,
  wire-schemas-as-program-types, runtime layout of wire values, encoding
  families beyond compact_binary v0, version negotiation. (Found while
  landing, FIXED 2026-06-11: struct-literal String field initialization did
  not lower to a native descriptor write -- data planning never collected
  string literals from `let` local initializers, so the descriptor-write
  selection found no data object and silently skipped; pinned by
  data/runtime_struct_literal_string_field_exit, which covers the record-
  and case-literal forms.)
- [ ] **Versioned data stage 3.** Era tag + the wire integration decision 10
  assumes; era-tagged containers that make version MATCH arms selectable
  (stage 2 ruled them unreachable — no value can hold a historical era yet);
  migration chains, `replaces`, quiescence obligations. (Stage 2 landed
  2026-06-11: historical-shape construction, the type-name migration call,
  the first runtime migration canary, struct-literal field validation.)
  DESIGN SIGNED OFF 2026-06-12 (frozen decision 14): builtin `Versioned<T>`
  — `{ era: u32, payload: UNION-OF-ERAS }` — constructed at boundaries only
  (chapter 21: ordinary values never carry era tags); version match arms
  legal ONLY on `Versioned<T>` subjects (tag compare + shape
  reinterpretation per arm; paren form binds the whole historical value);
  `era` read-only source-queryable; incomplete-chain = report verdict; the
  wire decoder is NOT a prerequisite. Stage 3b (no new surface,
  dispatchable independently): migration-chain completeness validation
  along the declared version chain. `replaces`/quiescence stay deferred
  behind the concurrency model.
- [ ] **Equatable synthesis / conformance defaults.** EQUATABLE SYNTHESIS
  LANDED (2026-06-11): `Type satisfies Equatable;` on a record or
  payload-bearing sum makes `==`/`!=` legal -- expanded INLINE at
  resolved->typed lowering into field compares (sums: OR over cases, tag
  compares first, then payload fields), riding existing backend/interpreter
  comparison machinery; the interim `==` error is retired for conforming
  types and extended with a declare-the-conformance suggestion for
  non-conforming ones; a written `Type::equals` wins (`==` lowers to a
  call); prerequisites error at the conformance item (every field scalar /
  `String` / payload-less sum / conforming; recursive types rejected). The
  interpreter short-circuits `&&`/`||` and ZII-defaults enum fields to the
  zero case; the native value-operand resolver reads oversize enum places
  as their tag prefix in tag compares (was a silent statement drop for
  two-field payloads). STRING FIELDS LANDED (2026-06-11): a `String` field
  compares by CONTENT through a new `TextEquals` value-operand LEAF
  (`{left, right}` descriptor places -> bool) lowered in both ISAs as a
  length compare plus a bounded byte loop (fixed-width encodings, pinned
  left/right descriptor-base relocation offsets, debug_asserts against the
  width functions); selection routes `String == String` place compares to
  it in nested-operand AND top-level binary-write positions; comparing a
  String field against a CONSTRUCTED LITERAL stays rejected (no stored
  descriptor at the compare site -- bind it to a value first). Canaries:
  pass+RUN `traits/equatable_record_equality_exit` +
  `traits/equatable_sum_payload_equality_exit` +
  `traits/equatable_string_field_equality_exit` (equal contents / same
  length different bytes / different lengths / scalar sibling), fail
  `traits/equatable_missing_conformance_suggested` /
  `equatable_field_not_equatable` / `equatable_recursive_type` /
  `equatable_string_field_literal_compare`. STILL OPEN: a CALLABLE
  synthesized `Type::equals` machine (build-time evaluation/trait-generator arc), trait
  `default machine` instantiation for other traits, recursive Equatable
  support, String-vs-literal structural compares, equality in
  contracts/domain facts (no typing scope there), and written-equals
  signature matching against `&Self` (validation accepts `Self` in trait
  signatures; substitution per conformance is unchecked).
- [x] **Case members: remaining halves.** (Both halves closed 2026-06-11 -- see
  the closing note at the end of this entry; checkbox synced 2026-07-04.)
  EXHAUSTIVENESS COUNTING LANDED
  (2026-06-11), over implicit case-domains AND case-subset domains: a
  dispatch run (consecutive transitions, the shape every block desugars to)
  whose arms classify a case-bearing subject must cover every case or close
  with `_`. Decidable arms: case arms (one tag) and PURE case-union domain
  arms; predicate-domain arms, `if`-guarded patterns, and value compares are
  uncountable, so uncovered+uncountable errors suggest `_`, while fully
  counted gaps name the missing cases ("match over `Command` does not cover
  `Command::Move`; add an arm or `_`"). RULING (chapter-1 footnote): pure
  case-union recognition is SYNTACTIC -- the domain's `when` classifier must
  be literally `self in Type::A | Type::B` over its own target type's cases
  with NO other facts; classifier analysis stays a possible later widening.
  The check runs on RESOLVED trees (omega-symbol-resolved-trees-to-typed-
  trees/src/exhaustiveness.rs, the `crate::equality` pattern) because typed
  lowering erases membership into tag compares/classifier expansions. With
  it landed: `when` classifiers now admit membership unions, `domain T::D
  when ...;` (semicolon, body-less) parses, and executable declared-domain
  membership now ANDs the classifier into the test (a union-subset domain
  works as a guard/arm at runtime; native+interpreter agree, see
  pass/data/match_exhaustive_by_case_union_domain). Probe record: before the
  check, a 2-of-3-case dispatch compiled and FELL THROUGH divergently at
  runtime (native exit 1, interpreter exit 0) -- the error is the fix.
  Corpus fallout: ZERO (suite was already covered-or-defaulted). Canaries:
  fail data/match_nonexhaustive_cases +
  data/match_predicate_domain_needs_default; pass+RUN
  data/match_exhaustive_by_cases + data/match_exhaustive_by_case_union_domain;
  pass data/match_default_satisfies_exhaustiveness. Payload sums are done;
  `self in Type::Case` and unions at use sites landed with decision 11.
  MIXED SHAPES LANDED (2026-06-11) -- the final half of decision 7; both
  halves of this item are now closed (see the next entry).
- [x] **Mixed data shapes (common fields + case part) LANDED (2026-06-11).**
  Decision 7's final half; the trees already modeled fields+cases together
  (only validation rejected). Decisions recorded here:
  - LAYOUT (owned in omega-layout, `DataShape::Enum` now carries
    `common_fields`): TAG-FIRST -- tag at offset 0, common fields packed
    after the tag, payload overlay after the common fields. Deliberate
    deviation from the suggested common-fields-first order: the backend's
    tag-only compares/writes (state-guard clamps, runtime value operands,
    static folds) treat "first ENUM_TAG_BYTES of the value" as the tag
    WITHOUT layout context, so the tag offset must stay the universal
    constant 0. Common-field offsets are case-independent constants in
    either order; ZII holds (zeroed value = first case + zeroed common
    fields); pure sums degenerate to the historical layout (empty common
    span), so every existing offset is unchanged.
  - CONSTRUCTION: case-literal form only (`Type::Case { ... }`; record-form
    literals over case-bearing types are rejected). Common fields may be
    named alongside payload fields; every common field NOT named
    ZERO-INITIALIZES (explicit zero writes ride the ordinary member-write
    path natively; the interpreter zeroes the cells), because construction
    replaces the whole value. Consequences, both hard errors: common-field
    defaults (would silently never apply) and non-scalar common fields
    (first cut: zeroing nested aggregates/text at construction is deferred).
    Payload-field names may not collide with common-field names (member
    access searches both).
  - ACCESS: common fields read/write WITHOUT case knowledge
    (`event.consumed` / `event.bonus = 5`); payload fields stay case-bound.
  - EQUALITY: Equatable over mixed = common fields AND tag AND matching
    payload (equatable.rs Mixed -> Structural; structural_equality.rs
    conjoins common compares with the sum expansion). FOUND+FIXED a latent
    compiler hang: omega-state-values folding's `factor_common_conjuncts`
    re-entered `boolean_and`, whose distribute-over-Or rewrite re-created
    the factored shape -- non-terminating mutual recursion, first reachable
    via mixed equality (its arms share the common-field compares). Factoring
    now re-attaches conjuncts with a non-distributing combinator.
  - REJECTED LOUDLY (scope kept honest): wire `encode_wire` over ANY
    case-bearing value type (sum or mixed) -- the schema field set has no
    spelling for the tag/payload, so encoding would silently drop the case
    part (this also closed a pre-existing silent hole for pure sums).
    Unnamed common fields in equality-compared case literals keep the
    existing "literal omits field" diagnostic (name the field).
  - Exhaustiveness, tag dispatch, payload binding, and `in` membership work
    over mixed unchanged (tag@0 preserved every existing path). Canaries:
    pass+RUN data/runtime_mixed_shape_exit (construct with named common
    field, case change zeroes unnamed common field, common write, 3-case
    dispatch with payload binding, exit 70) +
    traits/equatable_mixed_shape_equality_exit (common-field-only
    difference compares unequal), both differential; fail
    data/mixed_common_field_nonscalar, data/mixed_common_field_default,
    data/mixed_payload_field_shadows_common, data/mixed_record_literal,
    wire/encode_case_bearing_value. Retired:
    fail data/mixed_data_shape_unimplemented.

**Backend residue (small, known):**

- [x] Eager-guard divergence (effectful transition SUBJECTS) FIXED: a guard
  subject like `transition self.should_carve(random, 2) { true/false }` now
  evaluates exactly ONCE natively, matching the interpreter, even with
  diverging arm targets and a nested callee chain. Three compounding causes,
  all repaired: (1) every arm's guard holds a parser COPY of the subject call
  and each arm allocated its OWN `__call_result` slot — the runtime-storage
  plan now shares ONE slot across arms with structurally equal subjects
  (`shared_transition_guard_slot_offset`, omega-runtime-storage/body.rs);
  (2) later arms appended their own nested-callee/leaf/straight-line
  expansions, re-running the callee's side effects per arm — the
  runtime-branching plan now suppresses ALL execution machinery for repeated
  subjects (omega-runtime-branching/branching/mod.rs + expansions.rs);
  (3) `let x = self.f(...)` inside an expansion emitted the call TWICE (once
  for its StateCall operation, once via the LocalData operation's
  initializer-call path) — one doubling per nesting level, the dungeon's
  32-draws-for-1 amplification (instruction-selection straight_line.rs).
  Regression net: canaries/pass/control_flow/
  runtime_effectful_subject_single_evaluation_exit (diverging-arm,
  3-deep chain; pre-fix native exits 77, post-fix 70 = interpreter) plus the
  measured dungeon shape (1 draw per should_carve decision in BOTH backends).
- [x] Non-guard call chains over-draw / read stale values natively — BOTH
  named symptoms FIXED (2026-06-11); the splice is now the single executor of
  record for non-guard chains. (1) OVER-EXECUTION: the
  `carve_room -> roll_event -> rng.range -> next_u32` STATEMENT chain ran
  `next_u32` 3x natively (interpreter 1x). The three executors, mapped in the
  backend report: the splice's flattened Mutation op (the keeper), the
  non-guard branch PRELUDE's StateCall arm (a `let x = self.f(...)` statement
  classifies as StateCall in prelude_operations, and its arm re-emitted the
  callee's nested expansions), and the nested-walk straight-line expansion
  (created by append_branch_prelude_expansion's callee walk, then matched
  AGAIN at the flattened nested call's own body op). Plan-level suppression
  mirroring the eager-guard fix: non-guard (LocalDataOnly) preludes now carry
  ONLY call-free local initializers (omega-runtime-branching operations.rs)
  and never walk nested callees (expansions.rs — only guard-role `All`
  preludes walk, since the splice flattens every nested call into the body
  where each gets directly-matched machinery). (2) STALE READ: depth-1
  `let v = self.next(&mut state)` returned the PRE-mutation value because the
  call-result value selection (leaf expansion) emitted at the StateCall body
  op, before the splice's mutation ops. The dispatch loop now DEFERS the
  selection to the statement's own LocalStorage operation (after the callee's
  spliced effects, before the local copy) when the statement's only leaf role
  is AssignmentValue (instruction-selection runtime_dispatch.rs + leaf.rs
  `leaf_expansions_defer_to_local_initializer`). Canaries: pass+RUN
  control_flow/runtime_statement_call_single_execution_exit (pre-fix native 3
  = three executors, post-fix 70) and
  calls/runtime_assignment_call_post_mutation_value_exit (pre-fix native 2 =
  stale read, post-fix 70), both in the differential oracle. Dungeon: seed-7
  generation went from 34 native draws to 14 (interpreter 15).
- [x] Dungeon residual, ONE draw — FIXED (2026-06-12). The misfire was NOT an
  arm-selection/flow bug and NOT a stale depth: delta-debugging the dungeon
  down to a 130-line skeleton (copy sample to /tmp, delete rooms/events/
  systems while native!=interp held) plus an lldb breakpoint trace of the
  emitted guard loads showed the second `roll_event`'s parameter slot
  receiving 0xFFFFFFA6 = -90: the inline CALL ARGUMENT `raw % 100` (raw: u32,
  a prior call result) was emitted with SIGNED division (sdiv 0x1ada0e33),
  and for raw >= 2^31 the negative remainder reads as a huge value under the
  ladder's UNSIGNED guards (`roll < 20`/`roll < 60` both fail), falling into
  the enemy arm whose bat draw (depth 1 — legitimately <= 1, hence the
  "stale depth" misread) advanced the stream once. The first call survives by
  luck (its raw < 2^31), which is why the bug needed two call contexts; small
  probes passed because their raw values never crossed 2^31. Root cause:
  `select_runtime_storage_binary_write_in_table` (the pre-resolved-place
  entry the frame-slot ARGUMENT write funnels through) never ran the
  signedness adjustment its sibling targeted-mutation path has — fixed by an
  operand-only `signedness_adjusted_operator_for_operands` (binary_table_
  writes.rs); the branch-expansion binary write (branches/mutation.rs), a
  third drifted copy, now adjusts too. Selection-level only; aarch64/x86
  widths for Modulo vs ModuloUnsigned are identical. Canary: pass+RUN
  arithmetic/runtime_unsigned_modulo_call_argument_exit (pre-fix native 71 =
  4 draws, post-fix 70 = 3 draws = interpreter), in the differential oracle.
  Dungeon seed-7: full-tour event/path lines now byte-match the interpreter
  across all eight rooms (draw streams agree; the bullet's "14 vs 15" is
  retired). R05's description stayed un-asserted for a DIFFERENT reason — the
  side-room carve guard, since resolved (next bullet); both side-room
  description lines are now asserted in the scripted suite test.
- [x] Side-room DESCRIPTIONS lost natively — RESOLVED (2026-06-12). The
  suspect shape was wrong: the description WRITE machinery (carve through
  `room_mut`'s `&mut Room` in a guard-branch target) was sound and its
  selected/encoded code byte-correct per dispatch. The side rooms were never
  CARVED natively at all: `transition self.should_carve(random, N)` always
  took the FALSE arm because the guard byte was never computed. should_carve
  returns `self.rng.chance(random, chance, 100)`, and chance's inline leaf
  value `roll < numerator` binds `numerator` to should_carve's local `chance`
  (`max(15, 80 - depth*6)`), a fold-only local with NO frame slot — the leaf
  context could not resolve the name as a place, so
  `select_runtime_leaf_branch_terminal_value_write` silently emitted nothing
  and the chance call-result slot stayed 0. Every other side-room render line
  (label/event/paths) is HARDCODED per cell in the view, which is why only
  the data-driven description line exposed it (and why the "RNG streams
  match" observation held: the draws ran via the straight-line expansion;
  only the decision byte was lost). Fix: leaf terminal-value resolution now
  substitutes caller-local initializer names (bindings re-applied) for
  slot-less locals (`resolve_leaf_caller_local_initializer_names`,
  branches/leaf.rs) — selection-level only. Canary: pass+RUN
  dungeon/runtime_nested_value_call_caller_local_guard_exit (pre-fix exit 71,
  post-fix 70 = interpreter), in the differential oracle. The dungeon
  scripted suite test now detours through R06 and asserts BOTH side-room
  description lines; the full tour is byte-identical to the interpreter.
  Residue spotted while hunting: a `transition rooms[i].description ==
  "literal"` String-equality guard evaluated TRUE natively while the field
  was empty (two false-negative probes) — RESOLVED, next bullet.
- [x] Slice-indexed String guard compares lied — RESOLVED (2026-06-12).
  Failure class: SILENTLY DROPPED COMPARE, guard defaults truthy. A
  `String place == "literal"` guard (slice-indexed `items[i].name` AND plain
  fields `self.name` alike, `!=` too) had NO selection: the buffer-literal
  guard needs a runtime text buffer (stdin machinery), the storage guard
  needs places on BOTH sides, and the value guard can neither resolve a
  literal operand nor compare 16-byte descriptors — every path returned None,
  the dispatch edge emitted no compare (`EvaluateDispatchGuard
  NeedsRuntimeExpression` encodes nothing), and the first arm was taken
  unconditionally. Both probe regimes lied (empty AND non-empty-differing);
  the matching case "passed" for the same reason. Fix: a new
  `TextEqualsLiteral` value operand (place handle + inline literal bytes,
  bool 0/1; guards lower it as `CompareRuntimeValues == 1`), selected by
  `runtime_text_equals_literal_guard(_in_table)` for String-typed Storage and
  FrameIndexed descriptor places (frame-indexed tried FIRST so a slice index
  never falls back to the descriptor-as-value trap); emitters in both ISAs
  with width fns in lockstep (length mismatch short-circuits unequal, so a
  zeroed descriptor's null pointer is never dereferenced; the TextEquals
  half-empty behavior was audited and is correct). Honest guards then
  UNMASKED three double-masked write bugs, all fixed: (1) skewed relocation —
  aarch64 `runtime_machine_indexed_string_runtime_frame_address_offset` said
  20 but the encoder puts the frame adrp at 12, so machine-indexed string
  writes read a garbage index and landed nowhere; (2) concat-built String
  LOCALS (`let line = "== " + name + " =="`) were never materialized — local
  initializers are not mutations, so the runtime-text planner never planned
  their builder (StateLocalStorage now carries `initial_value`;
  `collect_runtime_text_local_initializer_writes`); (3) ALL-LITERAL concats
  (`"prefix " + "omega"`) per-segment "appends" to machine-indexed targets
  are full descriptor writes, leaving only the LAST segment — now folded to
  one StaticText write at planning/data/selection in lockstep. Canaries:
  pass+RUN text/runtime_slice_indexed_string_guard_exit (empty field takes
  the false arm, matching takes true, same-length-differing takes false;
  exit 70 only when all three behave) and
  text/runtime_string_field_literal_guard_exit (the storage-place sibling),
  both in the differential oracle. The remaining place-kind gap is RESOLVED
  (2026-06-12): TextEqualsLiteral selection + both ISA emitters now cover
  FrameBaseIndexed (local inline fixed array, frame base + runtime index
  scale), FrameFixedIndexed (slice descriptor + folded constant offset), and
  Pointee (pointer slot deref + field offset) places, widths in lockstep
  (x86_64 setup 30/17/17 bytes; aarch64 reuses the storage-read setup width
  fns). Probes pre-fix: base- and fixed-indexed selected NOTHING (silent
  truthy — empty field "equalled" the literal); pointee places lied
  DIFFERENTLY — the storage resolver saw through the reference and selected
  the POINTER SLOT's raw bytes as the descriptor, an always-false compare
  (match regime took the false arm), in both the `&mut Room` local-alias and
  called-machine-parameter shapes. Fix: descriptor-place resolution tries
  frame-indexed, base-indexed, fixed-indexed, then pointee, with static
  storage LAST (pointee-before-storage kills the pointer-slot-as-descriptor
  trap; direct base-indexed String WRITES still hard-error
  `needs runtime storage write lowering` — honest, writes go through slice
  aliases). Canaries (pass+RUN, three regimes each — empty≠literal, match,
  same-length-differ — all in the differential oracle):
  text/runtime_local_array_indexed_string_guard_exit,
  text/runtime_slice_fixed_indexed_string_guard_exit,
  text/runtime_pointee_string_guard_exit (alias + parameter shapes; also
  linux_x64 cross-emission smoke-checked for the width debug_asserts).
  Still open follow-up: the guard fallback itself still emits silence
  rather than a hard error (guard-must-select-or-error tightening). The
  array-literal initializer residue is RESOLVED (2026-06-12): `[Room {
  label: "x" }, ..]` into a local fixed array emitted rodata but wrote NO
  frame slots — and probing showed the gap was wider than the String guards
  suggested: the local-initializer mutation path had a StructLiteral arm but
  no ArrayLiteral arm, so the WHOLE initializer (scalar elements of
  `[1, 2, 3]` included) fell through to the scalar path and selected
  nothing. Fix (selection-level, writes/mod.rs): an ArrayLiteral arm in
  select_runtime_storage_resolved_mutation_write_in_mutable_table recurses
  per element through a literal-indexed target (`target[i]`), so
  struct-literal elements expand into their per-field member writes (String
  descriptors ride the landed fixed-indexed WriteRuntimeFrameString
  machinery) and scalar elements ride the static-write path. Canary
  (pass+RUN, differential oracle):
  data/runtime_array_literal_string_field_exit (two elements, distinct
  literals, runtime-indexed guards on each element's scalar sibling and
  String field plus an element-0-vs-element-1-literal cross check; exit 70).
- [ ] Signed/unsigned residue, sibling shape (2) only -- shape (1) is DONE (see
  the `[x]` entry immediately below; checkbox scope corrected 2026-07-04).
  (2) Trailing-state STALE READS of threaded `&mut` param fields:
  a transition-guard SUBJECT read of `random.calls` in a state appended
  after build_main_hall_1 saw the post-seed snapshot (0), and a `let hi =
  (random.seed >> 32) as u32` in a state appended after build_main_hall_4
  read a seed stale by the last TWO build_segment calls — instrumentation-
  only so far, but the same one-shrink-away family; needs its own minimal
  skeleton hunt.
- [x] Signed/unsigned residue, shape (1) — CAST OPERANDS — FIXED (2026-06-12).
  `((random.seed >> 32) as u32) % 199` lowered SIGNED because
  `resolve_runtime_storage_is_signed_in_table` could not see through Cast
  nodes (None -> signed fallback). The resolver now classifies a Cast by its
  TARGET type name (storage_places.rs) — `(x as u32)` is unsigned no matter
  what `x` is — which fixes every funnel at once (guards, edges, all binary
  writes route through this one resolver). Sibling sweep in the same change:
  the NESTED value-operand Binary/min-max builders never adjusted signedness
  at all (only top-level write operators did) — the dungeon probe's inner
  `seed >> 32` emitted the arithmetic shift, masked only by the following
  4-byte truncation. All seven remaining operator-choosing sites now run the
  shared decision: value_operands.rs in-table Binary + builtin-call,
  value_operands.rs non-table Binary + builtin-call (via a new
  `signedness_adjusted_operator_for_tree_operands` insert_tree adapter),
  branches/mutation.rs nested Binary + builtin-call, and the non-table
  `select_runtime_binary_mutation_write` (writes/mutation.rs — the cleanup
  doc's [!] alias path; instrumented across the full suite + dungeon + a
  purpose-built alias-fed guarded-transition probe, it is reached 0 times,
  so the adjustment there is defense-in-depth and a canary for it cannot be
  written from surface syntax today). Canary: pass+RUN
  arithmetic/runtime_unsigned_modulo_cast_operand_exit (pre-fix native 71 =
  signed remainder -87 in the u32 slot, post-fix 70 = interpreter), in the
  differential oracle.
- [x] Stale assignment-call result when the local's slot is ELIDED — FIXED
  (2026-06-12), option (b): the storage plan no longer elides the LocalStorage
  slot when the local's initializer contains a MUTATING call (a call passing a
  `&mut` argument). One condition in `local_data_requires_storage`
  (omega-state-storage/collection.rs, new `expression_contains_mutating_call`
  walk) — the elision is an optimization and correctness gates it: with the
  slot kept, the executor-of-record deferral
  (`leaf_expansions_defer_to_local_initializer`) has its landing op and the
  call-result copy emits AFTER the splice's mutation writes (backend report
  now shows `write binary ... Add 1` then `copy @0 -> frame@0`). Canary:
  pass+RUN calls/runtime_call_result_after_splice_mutation_exit (pre-fix
  native 71 / interpreter 70, post-fix both 70), in the differential oracle;
  guard-only sibling runtime_assignment_call_post_mutation_value_exit
  re-verified green. Original report follows. The
  "trailing-state stale-&mut-field reads" instrumentation observations
  shrink to this: `let seed: u64 = self.rng.next_seed(&mut random)` (callee:
  `state.seed = state.seed + 1; transition { _ -> state.seed }` — a PLAIN
  `&mut`-param field terminal) followed by ANY consumer statement
  (`let doubled: u64 = seed * 2`) makes `seed` deliver the PRE-call value
  natively (probe guards: doubled==84 -> 70 post-mutation, ==82 -> 71 stale;
  native 71, interpreter 70). Mechanism, read from the backend report +
  lldb slot dumps: when the assignment local feeds a LATER STATEMENT's
  initializer, the storage plan elides its LocalStorage op (slots.txt shows
  only the call-result slot, no `local seed`); the deferral fix
  (`leaf_expansions_defer_to_local_initializer`) has no LocalStorage op to
  defer to, so the call-result copy (param field -> call-result slot) emits
  at the StateCall body op, BEFORE the splice's mutation ops — emission
  order is literally `copy @0 -> @8` then `write binary @0 = @0 Add 1`.
  Guard-only consumption keeps the local slot and the copy emits AFTER
  the mutation (correct), which is why
  calls/runtime_assignment_call_post_mutation_value_exit stays green — its
  local keeps a slot. Fix direction: defer the call-result selection to the
  statement's position in splice order even when the local slot is elided
  (or stop eliding the slot for &mut-param-field call results). The
  trailing-state SUBJECT-read observation (`random.calls` after
  build_main_hall_1) is consistent with this shape feeding a guard, but was
  not separately reproduced.
- [x] 3 pre-existing `_compile` canaries hang at runtime — STALE (probed
  2026-06-11): the slice-write `_compile` canaries run now (the hang was the
  x18 zeroing below) and their dispatch shape already has a runtime `_exit`
  sibling in the suite; `calls/runtime_mutable_local_parameter_write_compile`
  "hangs" by its own unconditional `true -> main()` self-loop (source
  structure, not a backend bug; its `_exit` sibling verifies the behavior).
- [x] Straight-line `main` terminal LOCALS/EXPRESSIONS don't deliver as the
  exit code — FIXED (2026-06-11). Interpreter parity confirmed first (it
  already returned 70 for all three probe shapes; pinned in
  omega-interpreter/tests/coverage.rs). Root cause: the dispatch terminal's
  return-value selection (`select_runtime_dispatch_return_value`,
  runtime_dispatch/edges.rs) only handled a CONSTANT terminal
  (`static_terminal_target_value`) and silently fell through otherwise. Now:
  (1) constants write the immediate as before; (2) runtime places (field
  read-backs, locals with frame slots — reassigned locals always have
  storage) load via the new `CopyRuntimeStorageToReturnRegister` instruction
  (both ISAs, widths in lockstep, region-symbol relocation at instruction
  start); (3) storage-less locals/constant arithmetic constant-fold through
  `simplify_state_expression` to a small fixpoint. Residue: a runtime
  ARITHMETIC terminal (`self.n + 1`) still has no return-value write — fold
  it into a local or field first. Canaries:
  control_flow/runtime_straight_line_terminal_local_exit,
  control_flow/runtime_straight_line_terminal_field_readback_exit, and the
  promoted slices/runtime_mutable_slice_element_write_straight_line_exit
  (formerly _compile; now writes through the slice view and exits on the
  read-back), all RUN at 70 + registered in the differential oracle.
- [x] aarch64 runtime convergence (dungeon hot-potato). ROOT CAUSE FOUND AND
  FIXED: the aarch64 encoder used x18 as a general scratch for frame-slot
  copies (`ldr x18, [src]; str x18, [dst]`), but x18 is the reserved platform
  register on Darwin arm64 and XNU ZEROES it on every kernel->user return — any
  timer interrupt landing between the load and the store silently replaced the
  copied value with 0. In the dungeon this zeroed a threaded `&mut Level` arg
  (build_segment's level param), so `room_mut` computed `0 + element_offset`
  (the segfault on `str w17, [x16]` with x16 = 0x1d0 = rooms[2]'s byte offset:
  an offset-LIKE value because the BASE was the zeroed pointer). Looked
  nondeterministic/hot-potato because the first timer tick lands at a roughly
  fixed point in the deterministic instruction stream, and any debugger
  perturbation moved it. Fix: x26 (verified unused) replaces x18 everywhere in
  omega-isa-aarch64; register-only substitution, instruction widths unchanged.
  Regression net: canaries/pass/dungeon/runtime_threaded_mut_arg_interrupt_soak_exit
  (50M pointer-threaded increments across many timer ticks; pre-fix encoder
  fails it 4/5 runs, post-fix deterministic exit 70).
- [ ] Borrow layer records free-machine value-call targets as `invalid` in
  checked trees (cosmetic today).
- [x] Borrow layer records free-machine value-call targets as `invalid` in
  checked trees: fixed by the call-requires soundness wave (receiverless
  free-machine targets now resolve to the entry state in symbol resolution,
  and the checked-trees resolver accepts them).
- [x] Platform state-signature `requires` (calls through platform-typed
  contained objects) are never collected as call obligations -- the same
  vacuity the free-machine/boundary-trait wave fixed, third shape. FIXED:
  platform entries now parse the shared bodyless-signature clause grammar
  (`effects`/`requires`/`ensures`, previously a parse error), the
  checked-trees call-target resolver accepts platform state-signature
  symbols, `contract_target_from_state_symbol` maps them to the owning
  platform, and `call_target_parameters` reads the signature's parameter
  list -- so the existing instantiation path, caller-requires discharge,
  and mutation invalidation work identically to the trait shape (probe
  verified all three). Corpus fallout: none (suite stayed 187/187 before
  the new canaries; all canary `platform/console.omg` shims are boundary
  traits and were already enforced). New canaries: fail
  domains/call_requires_platform_unproven, pass
  domains/call_requires_platform_satisfied_by_caller_requires.
- [x] Stale test fixtures repaired: lib-test fixtures of omega-graph/types/
  names/proof/syntax-trees/abstract-operations/target-operations/facts gained
  the missing `abi`/`type_parameters`/`kind`/`properties`/`is_float` fields;
  omega-state-calls fixtures moved off the retired bare-`->` explicit-state
  syntax (omega-machine-emission already passed); architecture_boundaries
  brought in line with the omega-architecture-test layering policy + the
  facts/effects relocation (dev-deps exempt, pipeline->backend-helper edges
  tolerated, final machinery still forbidden, stale `lowering/` path fixed).
  `cargo test --workspace` is green apart from aarch64 MVP encoder gaps.

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
  a landed milestone-1 UEFI hello-world with a verified QEMU/OVMF harness is
  waiting; see the "Cathedral first-boot ladder" section above for the four
  smallest-landable features (of which #3 rides the layouts arc and #4 is
  mostly the existing win64 encoder).
- [ ] **Build-time evaluation (const eval + trait generators).** Effect-free machines in
  constant positions; `default machine` bodies with `Self::fields` member
  reflection expanded per conformance. Direction frozen (no macros, no #run);
  implementation is a large interpreter+expansion arc. Equatable/Hashable
  synthesis becomes ordinary once this lands.
- [ ] **Generics completion.** STAGE-1 DATA MONOMORPHIZATION LANDED (2026-07-01):
  a generic data instance's fields now lower NATIVELY -- the layout builder
  records one monomorphized instance per generic definition (memoized, keyed by
  the definition symbol; descriptors substitute bound parameters), so
  `Box<i32 in Wrapping>` field reads/writes run end-to-end (canary
  `generics/runtime_generic_record_instance_exit`, differential). STAGE-1
  BOUNDARIES: (a) a SECOND different instantiation of the same generic data
  poisons the recorded offsets -- both instances still SIZE correctly and the
  program compiles, but native field access through the colliding type rejects
  cleanly (fail canary `generic_second_instantiation_access_rejected`); real
  per-instance identity needs instance keys threaded through type descriptors.
  (b) A generic ENUM payload (`Maybe<T>::Some(value: T)`) still rejects: the
  DESTRUCTURED BINDING's frame slot is sized from the unsubstituted variant
  field type in compute_machine_layout (the data-side layout is ready; the
  machine/frame side needs dispatch-site bindings). (c) A VALUE-position call
  to a generic machine used to SILENTLY yield 0 natively (interp correct -- a
  #40 violation, worse than the 06-29 audit recorded); now FENCED with a clean
  error in omega-validation/calls.rs `fence_generic_value_callee` (fail
  canaries `generic_value_call_rejected` +
  `machine_bound_satisfied_value_call_fenced`, the latter re-purposed from the
  pass set where it codified the miscompile as must-compile). Statement calls
  to generic machines still work. REMAINING: (b)'s frame-side bindings, real
  machine-call monomorphization (unfence value calls by materializing the
  result slot), per-instance identity for (a), const-parameter substitution,
  layout for symbolic lengths. Decision-13 bounds are checked on
  type-reference instantiations; extend
  the check to machine-call monomorphization arguments when those land.
  HISTORICAL (superseded by the above): generics were TYPE-CHECK-ONLY; the
  unbuilt piece was the backend monomorphization path: threading substituted
  (T -> concrete) layouts through layout planning -> storage places ->
  instruction selection, so a monomorphized instance gets real storage. The
  front-end type system already substitutes correctly; the lowering does not
  consume it.
- [ ] **Allocator story.** `Vec` has no runtime; `alloc` is an effect name
  only. Decide explicit allocator/arena capabilities vs ambient heap BEFORE
  implementing Vec lowering.
- [ ] **Repr control for hardware structures.** packed, explicit
  offsets/alignment, untagged unions (page tables, descriptor tables, device
  registers). Chapter 19 has `repr native` only.
- [ ] **Proof engine arcs.** L7 LANDED 2026-06-12: induction via recursive
  contracts + decreases for single-state machines whose body is a chain of
  guarded value/tail-self-call transitions (`proofs/proof_inductive_gauss_sum`
  proves; `inductive_gauss_sum_false_twin` and `..._step_false_twin` reject).
  The recursive arm assumes the machine's own ensures for the call's
  arguments only after the engine discharges a strict decrease of the
  declared measure at that exact call site. Still open: exit-ensures
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
- [x] **aarch64 runtime convergence.** Resolved: the dungeon hot-potato was
  the encoder using interrupt-clobbered x18 as a scratch register (see the
  backend-residue entry above for the full diagnosis). The scripted dungeon
  loop and the dungeon differential oracle are green on the arm64 host; the
  last interpreter/native divergence (R05/R06 descriptions) was the
  side-room carve guard's lost call-result write, since resolved (see the
  backend-residue list) — the scripted tour is now byte-identical.
- [ ] **Text/string proof domains.** `String::Utf8`/`NoNul` as
  boundary-established carried facts without a byte-level proof tax (frozen
  direction in decision 5; the domains themselves unbuilt).

</details>

## Resolved Design Decisions (frozen)

Implementation slices below build against these. Minor/easily-reversible details
(exact namespace casing, builtin view surfacing) are left to the owning slice.

1. **Measure declarations (termination).** Custom well-founded orderings use a
   dedicated `measure` keyword as a standalone item:
   `measure Card::PowerOrder(card: Card) -> usize { card.power }` and
   `measure Quest::Difficulty lexicographic { tier, remaining_steps }`. Use site
   `decreases value -> Type::Name` is unchanged. Multiple measures per type and
   lexicographic tuples are supported.
2. **Range forms.** `a..b` exclusive, `a..=b` inclusive (plus open `a..`, `..b`,
   `..`). Inclusive normalizes to `a..(b+1)`. Exclusive end requires `b <= len`
   (range-bound facts); inclusive end requires `b < len` (index facts) — this is
   how range validity connects to index validity; inclusive non-empty ranges
   also establish a `non_empty` fact. The `..=MAX` overflow edge is a proof
   error (`checked_add`), not a panic.
3. **Operator spellings.** Fixed spellings are declared with an optional
   `spelling` clause on a named `operator`
   (`... -> T spelling [] requires index < items.len;`). Overload key stays path
   + parameter types. `items[index]`/`items[1..]` resolve to the spelled core
   operator and its `requires` IS the bounds obligation. The spelling sits above
   the `boundary` modifier, so it never hides signature or proof obligations.
4. **Boundary primitive registry.** One `BoundaryProvider { name, category,
   contract_ref, effect_set, target_applicability, origin_package }` record.
   Categories: `SliceIndexing | PointerOffset | PointerAccess |
   DescriptorConstruction | Allocation | HostAbiCall`. Core primitives bind a
   named provider; host providers are target-package metadata (generalizing the
   existing `HostAbiPlan`/`HostBoundaryPolicy` whitelist). Only whitelisted
   (core/host/toolchain) packages may declare providers; every boundary binding
   must resolve to a registered provider; unregistered names are rejected. The
   emitted boundary report is the audit artifact.
5. **Text types.** Owned text stays `String` (capacity/`push_str`); the borrowed
   text window is its own type spelled `&string`/`&mut string` (lowercase
   `string`, casing distinguishes owner from window). `StrView`/`&str` naming is
   retired. The window shares the slice `{ptr,len}` descriptor carrier. Expose
   `length`/`non_empty` measures first (cheap, O(1)); `no_nul`/`utf8` are domains
   established at validating boundary constructors and carried as facts, never
   re-proved per use.
6. **Fat descriptor model + owner.** One `FatDescriptor { ptr@0, len@pointer_size
   }` (size `2*pointer_size`, pointer-aligned) covers slices and text windows;
   slice `len` is an element count, text `len` a byte count (kind tag). Owned vs
   borrowed share layout, differing only by an ownership tag in the semantic
   spine. `omega-runtime-abi` owns the shape (field-offset + subslice accessors);
   `omega-layout` and instruction-selection are consumers.
7. **Case members, not `enum`.** Alternatives are a member class of `data`:
   `case` members with named payload fields, shape derived from members
   (record / sum / MIXED; sum-only shipped first, mixed landed 2026-06-11
   -- see the mixed-shapes entry under Outstanding for the recorded
   layout/construction/access rules). First
   case is the zero case (ZII); no niche layout. A case implicitly declares
   the same-named DOMAIN (free tag-compare classifier), so `case` never
   appears at use sites: match arms are classifications -- case arms and
   domain arms mix with identical `Type::Name` spelling, first satisfied arm
   wins, payload binding only on case arms, exhaustiveness counts only
   decidable arms (cases + case-union domains). Case subsets are domain
   unions (`when self in A | B`), replacing shadow enums.
   Cases/domains/machines share the `Type::member` namespace; collisions are
   hard errors, never priority. Foreign-type domains are allowed
   (extension-trait analog), import-gated, same loud-collision rule. The
   `enum` keyword is retired once `case` parsing lands (today it remains the
   transitional spelling for payload-less sums). See chapters 1 + 8 +
   appendix.
8. **Properties, traits, conformance, and ZII opt-in.** Type PROPERTIES are
   lowercase facts in brackets on the data declaration
   (`data Point [copy, zero_init]`, reusing invariant-parameter syntax);
   acquisition is computed (`sized`) / declared+verified / boundary-asserted —
   no inference, no negative form, not declarable on foreign types. TRAITS
   stay behavior: implemented by ordinary machines (structural satisfaction),
   claimed whole by a standalone conformance item `Point satisfies Equatable;`
   (checks written members, instantiates trait `default machine` bodies,
   synthesizes the CLOSED core derivable set — Slice::index pattern; nothing
   trait-shaped on data declarations). Equality is trait-resolved core
   `Equatable` with synthesized structural `equals`; interim: `==` on
   payload-bearing case values is a compile error (payload-less sums keep the
   tag compare). ZII splits: zero-validity is the unconditional compiler
   guarantee; zero-means-empty is the opt-in `[zero_init]` property which
   owns the zero-case-payload-free rule (the current hard error demotes into
   its verification when properties land). NO macro system ever; user
   structural synthesis, if needed, goes through compile-time execution +
   member reflection (direction only). Case construction stays the brace
   form. See chapters 1, 7, 13, 19 + appendix.
9. **Strict result use.** Discarding a non-unit return value is a compile
   error; intentional discards are spelled `_ = call();`. No per-type
   must_use marker. (Landed 2026-06-10.)
10. **Wire eras.** Generated wire encodings carry one era discriminator
    varint per top-level message/record (era 0 = the pre-versioning body);
    cross-era field-number recycling is legal; cross-era type changes are
    "requires migration" report verdicts, not errors (within-era violations
    and declared-history contradictions stay hard errors); unknown-case-tag
    handling is a wire decode policy (reject / preserve / decode as zero
    case). In-language exhaustiveness is never weakened; `[open]` is
    permanently dropped. See chapter 20 + appendix.
11. **Equality vs membership.** `==` is always value equality, resolved
    through core `Equatable`; `in` is always domain membership (the tag
    test for case domains, value-position legal: `let b: bool = cmd in
    Command::Quit | Command::None;`). A bare PAYLOAD-BEARING case name
    denotes no value — only its domain — so `x == Command::Move` is an
    error suggesting `in`; the brace form `x == Command::Move { dx: 1,
    dy: 2 }` is a constructed value and compares structurally. Equatable is
    IMPLICIT for primitives and payload-less sums (tag identity is
    unambiguous; match desugaring depends on it) and DECLARED
    (`Type satisfies Equatable;`, synthesizing structural `equals` from
    members) for records and payload-bearing sums — deliberately looser
    than Rust's universal derive, since whole-program compilation removes
    the accidental-API pressure. Boundary consequence: adding a payload
    case to a payload-less sum flips it implicit -> declared, erroring
    every `==` site until the one-line conformance is written —
    re-affirming equality after its meaning changed. Tag-clamped guard
    equality is retired as user-visible semantics (it survives only as the
    internal lowering of `in`).
12. **Discard admits effects; pure discards are dead code.** `_ =` accepts
    any CALL today and, by rule, any effectful evaluation later (effectful
    boundary operators, volatile/MMIO reads) — the gate is "evaluation has
    effects", not "is a call". Discarding a provably pure call (resolved
    callee has an empty effect set AND no `&mut`/out parameters — both
    signature-level facts) is a hard error, not a warning. Discarding a
    pure non-call expression stays a parse error. (Landed 2026-06-11:
    purity is judged against the callee's INFERRED transitive effect
    surface, not the declared list alone, so an undeclared-effects machine
    that transitively reaches `console.write` never counts as pure.)
13. **Property bounds: brackets attach to what they follow, everywhere.**
    Type parameters take bracket facts inline: `data Box<T [copy]> [copy]`.
    The Rust-style colon bound (`<T: copy>`) and the attribute-prefix form
    (`[copy]` on its own line) are rejected — colon would split the
    spelling system, and a floating prefix line is positional metadata (the
    attribute magic properties deliberately avoid). Leaves
    `T [copy] satisfies Equatable` room for trait bounds without
    collision.
14. **`Versioned<T>` container.** A permanent builtin template type
    `{ era: u32, payload: union-of-eras }` — the only thing version match
    arms are legal on (matching `Counter::v1(...)` on a PLAIN value stays
    an error; ordinary values never carry era tags). Constructed only at
    boundaries (wire decode, storage read, hot-swap edges); consumption is
    ordinary tag dispatch where the paren arm form binds the WHOLE
    historical value (`Counter::v1(old) -> ...`; braces stay field
    binding). `era` is read-only source-queryable. Migration-chain
    completeness is a report verdict, not an error (an arm may handle an
    old era manually). See chapter 21.
15. **Lifetimes: the Rust model, adopted wholesale.** A call's output may
    borrow an input; lifetime parameters (tick spelling, declared in the
    same `<>` list as types and `const`) express which:
    `machine header<'buf>(buffer: &'buf [u8], scratch: &mut [u8]) ->
    &'buf string`. ELISION covers the common cases (one ref input → output
    borrows it; `&self` → self) so most signatures stay annotation-free.
    Borrow-carrying data is IN-MODEL from day one
    (`data ChatMessage<'buf> { body: &'buf string; }`). House style:
    descriptive lifetime names (`'buf`, `'arena`), not `'a`. Rejected
    spellings: `from <arg>` clauses, `borrows` clauses, keyword
    region/origin parameters, Mojo-style bracket origins (collide with
    slice/property/invariant brackets). Unblocks zero-copy wire decode and
    view-returning machines. See chapter 2 + appendix.
16. **Suspension: the `await` marker; waiting is a boundary primitive.**
    AMENDED 2026-06-13 in chapter 17 (chapter is authority): the original
    no-keyword form below is SUPERSEDED. A wait is still an ordinary CALL
    (no `async`/`Future`, no signature coloring) but is MARKED `await` at
    the call site; the compiler REQUIRES `await` on any call carrying the
    `suspend` effect (call-site marker for visibility, never infects the
    caller's type). NEW HARD RULE: SUSPEND-IN-CALL IS FORBIDDEN -- a
    `suspend` machine can be SPAWNED but not CALLED, so suspension never
    nests through a call chain and a parked task's carry-set is SINGLE-LEVEL
    (one machine's live locals at its own `await`; M = MAX over its await
    points, not sum). N is DERIVED from the finite resource parked on
    (mailbox->1, permit pool->capacity), so `M x N` is a model-checked
    bound. Multi-await continuations thread as a `self` sum field
    (optionally `[max_size=N]`-pinned), not a paused stack. Everything else
    below still holds (boundary wait primitive, no-select one-mailbox,
    cancellation-as-value, scopeless scoped spawns). C2-C5 ACCEPTED; atomics
    stage 1 LANDED (load/store are real -- a plain aligned mov is atomic on
    x86). BOTH RMW ops are interim NON-ATOMIC parser desugars: fetch_add AND
    compare_exchange -- each must become a real LOCK-prefixed instruction
    (LOCK XADD / LOCK CMPXCHG) before any real parallelism. IPC-critical: the
    Cathedral many_to_one mailbox is unsound on the desugars. Tracked task
    chip task_b176af85.
    --- ORIGINAL (superseded) ---
    Typed state clusters CAN suspend across ticks. Waiting originates only
    at a futex-shaped `Scheduler` boundary trait (wait-on-word / wake-N --
    the ONLY wait mechanism, ever; ISRs/IO completions post to words);
    `suspend` is an inferred transitive effect (decision-12 machinery),
    declarable and checked; awaiting = calling (a parked task is state +
    planned frame storage -- no Future reification); borrows may not span
    suspend-effect call sites; effect ceilings forbid suspension where
    parking is illegal (ISR contexts); atomic-state is DERIVED ("your task
    cannot park mid-body unless the body calls a suspending machine" --
    not mutual exclusion; scheduler-agnostic). Scoped spawns borrow with
    no scope keyword (loans force the join; drop of `Join<T>` joins;
    free spawns stay move/copy-only). Task storage: per-machine-type pools
    of EXACT compiler-computed worst-case frames (no recursion = no stack
    sizes, overflow impossible); declared N, Region-backed later.
    Cancellation is a VALUE at the wait (zero case, no unwinding; rides
    chapter 15's recoverable channel; never-suspending tasks are joinable,
    not cancellable). There is NO select: producers post into one mailbox
    carrying a case-bearing sum, the consumer waits once and transitions
    ordinarily (Erlang one-mailbox model). See chapter 17 +
    wiki/design_briefs/concurrency_atomics.md. (C2-C5 of the scout
    register -- task unit, Join scopes, atomics-only sharing, C11
    intrinsics -- remain open for sign-off.)
17. **Arithmetic is EXACT by default; overflow is a proof obligation; weaker
    behavior is an explicit DOMAIN.** DECIDED 2026-06-14. SPELLING DECIDED:
    `<primitive> in <Domain>` (e.g. `count: u32 in Wrapping`), reusing the `in`
    domain keyword. Default integer
    arithmetic must be PROVEN not to overflow/underflow (and no div-by-zero /
    invalid shift); if the compiler cannot prove safety, it is a COMPILE ERROR
    (the strict model -- both provable-overflow AND the unprovable middle must
    be resolved; "no unexpected arithmetic"). To do arithmetic that can
    overflow, the value/type lives in an explicit primitive DOMAIN:
    `Wrapping` (wrap mod 2^width), `Saturating` (clamp to min/max), or
    `Trapping` (runtime check, trap on overflow -- the escape hatch when you
    cannot prove it and do not want wrap/saturate). Reuses the ch8 domain
    concept (these are arithmetic-behaviour domains on primitives).
    NO MAGIC WIDENING: `u8 + u8` is always `u8` (must be proven to fit, else
    error); widen by an explicit `as` cast (`a as u16 + b as u16`). NO MIXED
    DOMAIN: `(x in Wrapping) + (y exact)` is illegal; cross domains with an
    explicit `as` cast. Explicit always wins. Lineage: Ada/SPARK (range types +
    prove-no-overflow), cleaner than Rust's invisible debug/release mode or
    C's UB; fits Omega's proof-first, Cathedral-assurance identity. Breaking
    existing samples/canaries is ACCEPTED (that is what they are for) and is a
    forcing function for the range prover. Supersedes ch5's exploratory
    "Possible policies / likely default" text.
    IMPLEMENTATION PLAN (incremental, canary-driven):
    - S1: DONE (2026-06-14). The three domains are OPT-IN behaviour (additive,
      non-breaking): `T in Wrapping/Saturating/Trapping` parses, threads as a
      Constrained constraint -> descriptor domain -> the binary-write op, and
      x86_64 emits wrapping (default op + truncating store), saturating (width-
      correct op + cmov clamp to min/max), trapping (overflow-flag check + ud2)
      per width + signedness. Interpreter models all three; differential oracle
      agrees. Canaries: 200+100 in u8 -> 44 (wrap) / 255 (sat) / trap; i8
      100+100 -> 127 (signed sat); in-range trapping runs. Gaps (deferred):
      aarch64 (errors), `*`/`/` (errors), interpreter field-targets + u64/usize.
      See wiki/architecture/arithmetic_domains_implementation_map.md "S1b DONE".
    - S2: DONE (2026-06-15). Domains are OPERAND-driven (the domain lives on each
      value's type, combined per binary; literals are neutral). A binary mixing
      two different explicit domains is rejected (omega-validation/
      arithmetic_domains.rs; FAIL canary arithmetic_domain_mixed). `as` DOMAIN
      CASTS (`x as u8 in Saturating`) are the escape hatch (RUN canary
      arithmetic_domain_cast_exit). Codegen + interpreter re-keyed to operands.
    - S3: DONE (2026-06-15). EXACT-by-default ENFORCEMENT: an exact (undomained)
      integer `+`/`-`/`*` not provably in range is a compile error (omega-validation/
      arithmetic_domains.rs interval prover over type bounds + literals); the fix is
      `as`-widen, a range, or a domain. Atomic types resolve as Wrapping.
      nested_i32_mul_overflow is now a FAIL canary. The corpus (5 differential
      samples + ~37 canaries + 1 flow-test fixture) was migrated to `Wrapping`
      (behaviour-identical). Full workspace green.
    - S4: MOSTLY DONE (2026-06-15). DONE: flow-sensitive value tracking (blast
      44->30); range-constraint narrowing (`x: i32 [range<0,100>]`); literal-target
      folding (`let c: u8 = 200+100` rejected; comparison operands are [0,1]);
      contract `requires` narrowing (`requires amount<=100` keeps `amount+amount`
      exact). STILL TODO: loop/`decreases` bounds; return-range / inter-state
      inference (remaining corpus `Wrapping` operands are call-results/cross-state);
      range-respecting assignment check.

## Next Up (highest leverage)

**Landed 2026-06-11 (proof soundness: call-requires collection for free
machines + boundary traits).** Two call shapes silently never produced
call-site `requires` obligations, so callee contracts passed vacuously
(negative-control probes compiled clean): (a) FREE top-level machine calls --
the receiverless target stayed an invalid symbol through the frontend (the
backend dispatched by name), so borrow/contract collection saw nothing; in
STATEMENT position the call did not even resolve (`machine X has no local
state` from validation). (b) BOUNDARY-TRAIT machine calls
(`self.console.show(item)`) -- the trait machine signature was invisible to
`resolve_state_call_target`, and signature-owned contract facts were
explicitly excluded from call-fact matching. Fixes: symbol resolution now
points receiverless free-machine calls at the machine's entry state
(builtins still win); validation accepts the free-machine statement call
(strict result use applies, named as spelled); the checked-trees call-target
resolver accepts cross-machine state symbols and trait machine signature
symbols; `contract_target_from_state_symbol` maps trait signatures to their
owning trait; `append_contract_fact_refs` matches StateSignature owners; the
instantiation path (`call_target_parameters`) reads parameters from machine
states OR trait signatures, so callee-parameter -> caller-argument place
mapping and caller-requires discharge work for both shapes, and mutation
invalidation strikes the instantiated facts (probe verified: interleaved
`item.value = 0` before the call rejects with the invalidation detail).
Chapter 18 authority flow is now load-bearing: `Filesystem::write_bytes
requires folder in Folder::Writable` is enforced at the caller. Corpus
fallout: NONE (suite stayed at baseline; host `capability` blocks are
dropped at lowering and never reach contract collection). New canaries: fail
`domains/call_requires_free_machine_value_unproven`,
`domains/call_requires_free_machine_statement_unproven`,
`domains/call_requires_boundary_trait_unproven`; pass
`domains/call_requires_free_machine_satisfied_by_caller_requires`,
`domains/call_requires_boundary_trait_satisfied_by_caller_requires`.
Residue: PLATFORM state-signature `requires` (calls through platform-typed
contained objects) are still never collected -- same vacuity, third shape,
needs the same treatment when platforms matter; `capability { entry ...
requires }` blocks are dropped at symbol-resolution lowering, so host
capability contracts (omega/host/contracts) remain unenforced until
capabilities lower at all.

**Landed 2026-06-11 (decision 12 implementation).** Pure discards are now dead
code: `_ = call();` rejects when the resolved callee's inferred TRANSITIVE
effect set is empty AND its signature takes no `&mut` out-parameters
(`validate_effect_plan` owns the check; the transitive surface — not the
declared list — is the purity source, so a no-declaration machine that
transitively reaches `console.write` stays discardable). New canaries:
`fail/calls/pure_discard_dead_code` and
`pass/calls/effectless_mut_out_param_discard_compile` (&mut out-param, no
effects — must stay legal); `runtime_explicit_discard_executes_exit` is
unaffected (its callee writes through `&mut Tally`).

**Wave landed 2026-06-10 (decisions 8/9/10 implementation + backend gaps).**
Six lanes merged, suite 179/179, differential oracle fully matched:
(a) type properties `data Point [copy, zero_init, send]` parse + verify
(copy/send structural, zero_init owns zero-means-empty incl. the DEMOTED
zero-case rule); (b) standalone conformance items `Point satisfies
Equatable;` validate against written attached machines (default
instantiation/core synthesis still pending -- the build-time evaluation direction);
(c) interim `==` error on payload-bearing cases in statement position;
(d) strict result use: discarding a non-unit call result errors, `_ =
call();` is the explicit discard (only ONE corpus file needed the sweep);
(e) wire era chain checks + migration verdicts + legal recycling;
(f) versioned data stage 1 (historical-shape symbols, `Counter::v1` types,
migration-machine spelling compiles natively); (g) case PAYLOADS lower
natively (tag-prefix writes, payload member reads, tag-only guard compares;
pending canary promoted, ACTIVE_PENDING_CANARIES empty); (h) value-position
calls to FREE stateful machines dispatch and deliver values (incl. looping/
recursive shapes). Known interim semantics flagged for design review:
`_ =` accepts only calls. (Tag-only case equality in guards was RESOLVED by
the decision-11 landing below: the tag clamp is no longer user-visible
equality semantics, only the internal lowering of `in`.)

**Decision 11 landed 2026-06-11 (equality vs membership).** `in` now accepts
implicit case domains at use sites: `cmd in Command::Move` (payload-bearing
included) and unions `cmd in Command::Quit | Command::Move` work in value
position and as transition guard subjects, lowering to tag-equality compares
in the resolved->typed stage. Transition case arms desugar to MEMBERSHIP at
parse time (not `==`), so the bare-payload-case `==` check runs on the
RESOLVED trees and covers every position -- statements, guard
subjects/conditions, transition target arguments, domain `when` classifiers
and proof facts, machine contracts -- with a message suggesting `in`; the
brace form keeps the structural-equality interim error, payload-less `==`
stays legal everywhere. The guard tag clamp survives only as the internal
lowering of `in` (and payload-less `==`); the runtime-value expression paths
gained the same tag clamp for case compares inside boolean trees. New
canaries: pass+RUN `data/case_membership_value_exit`,
`data/case_membership_union_guard_exit` (both in the differential oracle);
fail `data/bare_payload_case_equality_suggests_in`,
`data/bare_payload_case_equality_guard`.

**Decision 13 landed 2026-06-11 (property bounds on type parameters).**
`data Box<T [copy]> [copy] { value: T; }` parses everywhere
`parse_type_parameters` runs (data, machine, trait, operator); the bracket
fact list is the SAME parse as the data-declaration property list (closed
set, duplicates/`sized`/unknown rejected). `zero_init` is accepted as a
bound: its structural rule reads fields, so it is checkable at
instantiation exactly like copy/send. The Rust-style colon bound
(`<T: copy>`) and the attribute-prefix form (`<[copy] T>`) are rejected
with the bracket spelling suggested. The structural copy/send/zero_init
verifier now accepts a field whose type parameter declares the matching
bound (and suggests `T [copy]` when it does not), and every VALIDATED
type-reference surface (data fields, domain targets, machine owned data,
state locals, state parameters/returns) checks instantiation arguments
against the base data's parameter bounds — in-scope bounded parameters
count as carrying their bound. An instantiated generic whose base declares
a property now also satisfies the structural walk (`Box<i32>` is copy
inside another `[copy]` data). NOT yet checked: machine-call
monomorphization arguments (generics completion arc). Canaries:
`pass/generics/property_bound_type_parameter`,
`fail/generics/{property_bound_missing_on_field,
property_bound_violated_at_instantiation, colon_bound_rejected}`.

**Recent canary promotions.** Numeric literal suffixes (`3i32`, `3.0real`,
`3nat`), newline-separated proof facts, field `+=` assignment, relax scope syntax
(`relax target { ... }`), relaxed borrow parameter spelling (`&mut relaxed T`),
trait `default machine` syntax, `data FixedBuffer<T, const N: usize>` const
parameters usable as symbolic fixed-array lengths, and top-level
`host <target> provides <Trait> { machine -> syscall N; }` provider metadata,
plus `wire data` schemas with encoding, numbered fields, reserved tags, and
version blocks, plus `data` historical `version` blocks, plus `&mut dyn Trait`
parameters and trait-method calls on dyn receivers now compile in the active
pass suite. Trailing machine version selectors like `Counter::increment::v1`
now split structurally as an attached-data method instead of treating `v1` as
the entry state. Single-subject transition match arms can now parse data
destructure guards such as `Player { health, .. } if health > 5` by rewriting
the destructured guard name to the matched subject field. Vec slice-view
invalidation now rejects through source-visible `Vec<T>::push`, and the last
physical pending canaries were promoted to active fail coverage for expression
`match` and version migration matching. Full canary suite is green locally
(`cargo test -p omega-compiler --test canary_suite`, 163 Rust tests); pass/fail
canary counts can change without changing the Rust harness test count because
many canaries are batched. The proofs false twins were promoted to
`canaries/fail/proofs/` when the contract entailment engine landed (empty-body
proof machines now PROVE or REJECT in-language contracts); see
`wiki/proof_engine_roadmap.md`.

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

**Data version semantics follow-up.** STAGE 1 DONE (2026-06-10): each
`version vN { ... }` block now lowers to a real historical-shape data
definition `Data::vN` with root symbols and member resolution, so
`Counter::v1` is a nameable type usable in machine signatures and generic
arguments; the chapter-21 migration spelling
`machine Counter::from_v1(old: Counter::v1, out: &mut Counter)` compiles
end-to-end including native lowering, and version-scoped machine paths
(`Counter::increment::v1`) type-check `self` against the v1 field set.
Declared-history contradictions (duplicate/non-canonical/nested version
names, version-scoped machines targeting undeclared versions) are compile
errors. STAGE 2 DONE (2026-06-11): historical-shape VALUES construct —
`Counter::v1 { counter: 3 }` resolves the brace literal to the version
block's shape definition (NOT a case of `Counter`; constructing an
undeclared version is a compile error), struct-literal field names now
validate against the constructed shape's declared members (current shape,
historical shape, and case-payload literals alike), and a call through the
data TYPE name (`Counter::from_v1(old, &mut current)`) resolves to the
attached machine, so the chapter-21 migration runs end-to-end — the first
runtime migration canary (`versioning/runtime_version_migration_exit`,
exit 70) passes natively AND in the differential oracle. Version MATCH arms
(`Counter::v1(old) ->`) got their stage-2 ruling: values carry no era tag,
so every value has the current shape and a version arm can never be
selected — the arm is rejected as UNREACHABLE (fail canary
`versioning/match_on_version` pins the diagnostic) rather than lowered with
fake runtime semantics. STAGE 3 frontier: the era tag itself (and decision
10's wire-era ride), era-tagged containers that make version matching
selectable, migration chains / `replaces` / quiescence obligations.

**Wire data semantics follow-up.** Stage 1 (validation + compatibility) is
done: wire schemas now lower through symbol-resolved and typed trees as their
own root family (`WireSchema` with arena-stored members and a `WireSchema`
symbol kind), `omega-validation` rejects duplicate/reserved tag misuse,
duplicate versions, unresolved field types, and version-vs-current type
changes or unreserved retirements (fail canaries under `canaries/fail/wire/`),
and every compile emits a `04_wire_protocols.txt` compatibility report with
per-version verdicts. DECISION 10 LANDED (2026-06-10): the checker and the
report now walk the version chain `[v1, v2, ..., current]` comparing only
ADJACENT eras; cross-era type changes are "requires migration" report
verdicts (compile clean); retiring a documented number without reserving it
is era-scoped to the successor and stays a hard error; cross-era
field-number recycling is legal (per-scope `reserved`); pass canaries cover
recycling + type-change migration verdicts. STAGE 2a LANDED (2026-06-11):
era assignment (era 0 = the pre-versioning body; version blocks count up in
declaration order; the current body is the highest era, reported per schema),
the compiler-recognized `Schema::encode_wire(&value, &mut out, &mut written)`
call (validated front-end: stage 2a scalar set, field coverage by name+type,
worst-case out-buffer capacity so the emitted code needs no runtime bounds
checks), and compact_binary v0 framing emitted through two new wire-append
operations (literal framing byte + runtime scalar varint) implemented on both
ISAs with widths/relocation-offset functions asserted against the encoders.
STAGE 2b (current-era decoder) and STRING-FIELD ENCODE landed 2026-06-11
(see the wire stage 2 bullet above for the String storage decision and its
known holes). Still needed: String decode (borrow-facts follow-up),
nested/repeated fields, wire-schemas-as-program-types, runtime layout of
wire values, encoding-family semantics beyond compact_binary v0, and version
negotiation.

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

### Capabilities And Authority

- [x] Capability facts flow through returns/derives/acquires across nested calls,
  not just direct boundary calls: `build_capability_facts` runs a call-graph
  fixpoint that folds a callee's verb into its caller when the authority value
  reaches the caller (capability-typed return for `acquires`/`returns`,
  capability return or parameter for `derives`). Propagated facts carry the
  helper state as provenance (`CapabilityFlowFact.via_state_symbol`) and the
  boundary blast radius renders it (`Backup::stage acquires via Vault::pick`).
  Canaries: `capabilities/acquires_through_helper_return` (two-level acquire
  chain), `capabilities/derives_through_helper`.

### Core Boundary Primitive Registry

- [x] Populate `BoundaryProvider.contract_ref`/`effect_set`/`target_applicability`
  from the bound operator instead of empty defaults. The populated registry is
  surfaced in the boundary report artifact (`10_boundary.html`, "Boundary
  Providers" section): per provider, the governing contract, authority effects,
  target applicability, and origin package.

### Proof-Backed Indexing And Subslicing

- [x] Thread refined subslice diagnostics through operator-contract errors once
  `Slice::from/to/range` contracts drive checking directly. RESOLVED
  (2026-06-12): a failed operator-sourced bound now names the spelled operator
  and its contract for browsability — e.g. ``cannot prove `start <= end && end
  <= items.len` — the `requires` of `Slice::range` (spelled `[..]`)`` appended
  to the fact-level failure. Attribution only fires when the core slice surface
  is imported (the obligation is operator-sourced); literal-shape diagnostics
  stand alone otherwise. Pinned by
  `fail/slices/subslice_range_operator_contract_unproven` and
  `fail/slices/index_operator_contract_unproven`.
- [x] Represent length facts and window-shrinking facts as first-class slice
  proof vocabulary (non-empty already exists). LANDED (2026-06-12): the
  vocabulary is `minimum_lengths` (floor), `exact_lengths` (pinned), and
  `window_parents` (carve relation) in `RangeFacts`. New derivations: a
  start-only tail `items[a..]` with constant `a` shrinks the parent's floor and
  exact length by `a` (`prove_shrunk_window_length`), and a constant-bounded
  range over a symbolic-length base discharges its `start <= end` ordering by
  folding both bounds. Consumers: index proofs over the derived window length
  (`pass/slices/window_shrink_min_length_tail_index_compile`,
  `pass/slices/window_literal_bounds_min_length_parent_index_compile`; one-past
  rejections pinned in the matching `fail/slices/*_unproven` canaries).
  Soundness companion: reassigning a local collection now forgets its
  label-keyed facts (floors, exact lengths, window relation, position proofs)
  via `forget_collection_facts` — a stale floor from the old value must not
  prove indexes into the new one
  (`fail/slices/window_reassigned_shrunk_floor_unproven`).
  Honest scope: symbolic (non-folded) bounds — e.g. `tail[parent_len - 3]` —
  still need a symbolic length algebra; only constant-folded offsets shrink.
- [x] Ensure alias and borrow facts understand subslice overlap conservatively.
  VERIFIED CONSERVATIVE (2026-06-12): two `&mut` windows of the same base are
  rejected unless their literal bounds prove disjointness
  (`windows_may_overlap` defaults to overlap on any unknown bound; the borrow
  pass reuses it via the loan-overlap engine). Probes: `items[0..2]`+`items[2..4]`
  accepted, `items[0..2]`+`items[1..3]` rejected, `items[0..2]`+`items[..]`
  rejected. Pinned by `pass/slices/disjoint_mut_subslice_windows_compile`,
  `fail/slices/overlapping_mut_subslice_windows_rejected`, and
  `fail/slices/unknown_bounds_mut_subslice_windows_rejected`.

### Slice Runtime Descriptor Semantics

- [x] Blank-room rendering RESOLVED (verified 2026-06-11): native dungeon
  room lookup/render now produces labels/descriptions byte-identical to the
  interpreter on the canonical scripted loop (the x18 reserved-register fix
  closed the remaining corruption). The final dungeon divergence (R05/R06
  data-driven descriptions) was the side-room carve guard's lost call-result
  write, since resolved in the backend-residue list — descriptor
  initialization itself was already fixed.
- [x] Generalize subslice descriptor pointer offsets beyond fixed-array alias
  copy special cases. DONE (2026-06-12): every slice-descriptor write consumer
  (locals, transition arguments, branch preludes, mutations) routes through one
  seam — `emit_runtime_frame_slot_slice_descriptor_write_in_table` now tries the
  generalized runtime-descriptor subslice after the literal fixed-array path.
  Newly lowering shapes (all interpreter-differential-verified):
  subslice-of-param into a LOCAL (`let tail = sub[1..]`, previously a silent
  whole-descriptor copy), nested subslice in one expression (`sub[1..][1..]`,
  literal layers fold into a window bias; previously a silent un-offset
  descriptor natively AND an interpreter reject — `eval_subslice` now evaluates
  nested range-indexed bases as views), and runtime-start over a subslice local
  (`tail[start..]`, bias rides the indexed-address op's field offset).
- [x] Generalize start-only/end-only/bounded descriptors beyond literal
  fixed-array-backed views. DONE (2026-06-12): bounded (`sub[1..4]`) and
  end-only (`sub[..2]`) literal ranges over runtime descriptors already lowered
  (now pinned by canaries); RUNTIME bounds are new — `sub[start..]` computes
  ptr via `WriteRuntimeFrameIndexedAddressToRuntimeFrame` (its aarch64 width
  table was stale by 40 bytes — fixed to use `runtime_frame_index_setup_width`)
  and len as a storage-storage subtraction; `sub[..end]` reads the runtime
  length from `end`'s slot; literal inclusive ends (`sub[1..=3]`) fold to
  `end + 1` at selection time. STILL UNSUPPORTED (loud, see below): computed
  bounds (`sub[offset + 1..]`), RUNTIME inclusive ends (`sub[..=n]`, needs a
  +1 at runtime), and runtime bounds in a NESTED inner layer. Slice-typed
  `data` fields (`items: &[T]`) do not parse, so the "machine-field slice"
  base shape is not expressible at the language level today.
- [x] Add focused pass/fail canaries for each newly supported subslice descriptor
  lowering shape. DONE (2026-06-12): eight new runtime canaries (suite + RUN +
  differential): `runtime_subslice_param_bounded_range_exit`,
  `runtime_subslice_param_end_only_exit`, `runtime_subslice_param_local_exit`,
  `runtime_subslice_runtime_start_exit`, `runtime_subslice_runtime_end_exit`,
  `runtime_subslice_nested_of_param_exit`,
  `runtime_subslice_runtime_start_over_local_exit`,
  `runtime_subslice_param_inclusive_end_exit`.
- [x] Unsupported subslice shapes now fail LOUDLY instead of silently keeping a
  stale/garbage descriptor: the `descriptor_argument_blockers` emission pass
  verifies every range-indexed transition argument writes its callee parameter
  slot and every subslice-initialized slice local writes its descriptor slot,
  and blocks emission naming the state, statement, and expression otherwise
  (probed with `sub[offset + 1..]` in both argument and local position — both
  previously compiled and exited wrong; both now block).
- [x] Keep backend reports explicit about descriptor construction and mutation.
  Verified 2026-06-12 by probe: each construction renders one line per half —
  `write runtime-frame pointer @T = &(runtime_frame@desc[runtime_frame@idx * elem]) +bias`
  for the pointer and a `write runtime storage binary … Subtract …` /
  `write runtime storage integer …` for the length — base, start source, and
  length source are all readable. No gaps found; nothing changed.

### Measures, Orderings, And Rankings

- [x] Support builtin/default inference for plain `decreases value` only when
  unambiguous. DONE (2026-06-11; core inference had landed earlier as
  "Infer default decreases order"). The rule: plain `decreases value` infers a
  builtin ranking only when the value's type makes it unambiguous — unsigned
  integer kinds (`usize`, `u8`-`u64`, `nat`, and `slice.len` members) get
  descending naturals; slice-typed values get `Slice::Length`; `upper - lower`
  is the named bounded distance. Everything else (signed integers, floats,
  structs) errors with a type-aware diagnostic naming the value and the reason
  (e.g. "cannot infer a ranking for `decreases remaining` ...: signed values
  have no default well-founded order -- select one with
  `decreases remaining -> View`"). RULING: a declared `measure` is NEVER
  selected implicitly, even when it is the only one declared for the value's
  type — only true builtins infer, so declaring a second measure later cannot
  silently change or break distant `decreases` clauses at a distance. Matching
  declared measures are suggested by name in the diagnostic instead
  (fail canary `termination/default_order_declared_measure_not_inferred`
  locks the ruling; pass canary
  `termination/default_order_unsigned_width_countdown_compile` covers
  non-`usize` unsigned widths).
- [x] Replace arithmetic-facing proof UX such as `limit - index` with named
  bounded-distance rankings. DONE (2026-06-12). The named view is
  `Nat::BoundedDistance` ("rank by the natural-number distance from the lower
  value up to the upper bound"), following the existing `Nat::Descending` /
  `Slice::Length` Type::Name pattern, which the view position already parses
  with no grammar change. What landed: (a) plain `decreases upper - lower`
  resolves to the distinct `RankingOrder::BoundedDistance` (no longer folded
  into NatDescending), so diagnostics and the checker name the ranking;
  (b) explicit selection `decreases limit - index -> Nat::BoundedDistance`
  (pass canary `termination/bounded_distance_named_view`); (c) the inverted
  spelling `decreases index - limit` is recognized — the checker probes the
  swapped operands, and when they prove, rejects with a diagnostic that names
  the right shape ("... inverts the named bounded distance --
  `Nat::BoundedDistance` ranks `upper - lower` ... write
  `decreases limit - index`"; fail canary
  `termination/bounded_distance_inverted`); (d) the L7 induction gate also
  accepts the named view — the distance polynomial goes through the identical
  strict-decrease + non-negativity check (pass canary
  `proofs/proof_inductive_climbing_sum`, step-false twin
  `proofs/inductive_climbing_sum_step_false_twin` pins that the hypothesis
  actually enters through this gate); (e) the ambiguity diagnostic's browsable
  builtin-view list now includes `Nat::BoundedDistance`. DECIDED 2026-06-12
  (maintainer): the use-site subtraction is NOT acceptable permanent
  surface — build the argumented view spelling
  `decreases (index, limit) -> Nat::BoundedDistance` (tuple form; the
  arrow's left side stays uniformly the ranked subjects) and retire
  `decreases limit - index` once it lands. Grammar-surgery scope: the
  ranking-view position is a plain identifier path
  (`parse_path_handle_span` in
  `omega-tokens-to-syntax-trees/src/parser/machine/clauses.rs`) and
  `decrease_order` is `HandleSpan<Identifier>` through all three tree
  representations, so view arguments need new syntax, storage, and symbol
  resolution. NOTE (pre-existing bug,
  RESOLVED 2026-06-12 below): a `requires` clause on a recursive machine used
  to overflow the compile-time contract evaluator's stack
  (`ContractExpressionEvaluator::integer_value` followed the self-call site's
  arguments in a loop), which is why `proof_inductive_climbing_sum` states its
  theorem as `result >= acc + limit - index` (true without a precondition)
  instead of the equality that would need `requires index <= limit` (the
  climbing canary's weaker theorem statement is kept as-is).
- Resolved 2026-06-12: `requires` on a recursive machine no longer crashes
  the compiler. Root cause: the contract evaluator's constant walk
  (`checks/contracts/evaluator/` in omega-typed-trees-to-checked-trees)
  resolves a callee parameter to the call-site argument expression to
  discharge `requires` by constant propagation; at a SELF call site the
  argument mentions the same parameter (`n` resolves to `n - 1`, whose `n`
  resolves to `n - 1` again), so `integer_value`/`resolved_expression`
  alternated forever. The pre-existing same-handle check in the Name arm only
  caught cycles of length 1. Fix: two active-expression stacks
  (`active_evaluations`, `active_resolutions` on
  `ContractExpressionEvaluator`, threaded through `guarding_cycles`) detect
  re-entry into an expression still being evaluated/resolved and STAND DOWN
  with None -- unknown never proves and never falsely rejects, so discharge
  falls through to the semantic provers (arm facts, caller requires).
  Legitimate constant following is untouched (pass
  constraints/scalar_requires_satisfied_by_literal and the rest of the suite
  are unchanged). Regression pin: pass canary
  proofs/recursive_machine_with_requires_compiles -- a recursive gauss_sum
  threading an untouched `limit` parameter with `requires limit > 0`,
  discharged by the literal at the outer call (constant walk) and by the
  caller's own requires at the recursive call; its value is that it compiles
  AT ALL. Unprovable shapes on recursive machines (e.g. `requires n >= 0`)
  now produce the normal cannot-prove diagnostic instead of a stack overflow.
- Resolved 2026-06-11: shrinking-slice recursion runtime exit canary added as
  `termination/runtime_shrinking_slice_recursion_exit` (suite ACTIVE list +
  dedicated run test + differential RUN_CANARIES; the parked
  `canaries/run/shrinking_slice_recursion_total_probe` is deleted). Root cause
  of the wrong native total: `resolve_runtime_storage_place_in_table`'s
  path-based fall-through DROPPED a root element index over a slice-descriptor
  frame slot, so a threaded `items[0].value` transition argument resolved to a
  plain place over the descriptor slot itself — `take` received the data
  pointer's low bytes (observed exit 152 = (4*ptr + 4+8+12) & 0xff; 152 is not
  a multiple of 5 while every element is, the fingerprint that ruled out any
  element-sum). Fixed in instruction selection: the resolver now refuses an
  unhonorable root index (descriptor slots always; inline fixed arrays for
  index != 0), transition-argument materialization gained a descriptor-aware
  `CopyRuntimeFrameFixedIndexedToRuntimeFrame` strategy, and
  `argument_source_frame_range` reports the descriptor slot as the read range
  so the same-context overlap staging (source -> scratch -> target) still
  triggers — without it the in-place `items[1..]` update would shrink the
  window BEFORE the head read. The statement-position shape of the same
  accumulation still over-executes natively — that is the separate non-guard
  executor-of-record residue, not this argument-lowering bug.

### Operators And Domains

- Consolidated 2026-06-11: the two parallel operator-resolution surfaces are
  now one authority. `omega_typed_trees::operator::resolve_spelling` (spelling
  -> root + domain-owned candidates, receiver-type narrowing) is the single
  use-site resolution implementation — resolution is a typing-stage decision
  per the pipeline Ownership Rule — and the checked stage
  (`omega-typed-trees-to-checked-trees/src/operators.rs`) only records its
  outcome as durable evidence (`CheckedOperatorFacts`, candidate contract
  spans, `ProofFacts.contract_operator_uses`) instead of re-resolving. The old
  operand-key `resolve_spelling`/`SpellingDispatch` had no callers and was
  deleted, and `omega-validation` dropped its private copy of the operand
  signature normalizer in favor of the typed-trees one. Declaration-conflict
  diagnostics (duplicate spellings, competing domain meanings in
  `omega-validation`) and use-site resolution evidence (checked facts) answer
  different questions and intentionally remain separate consumers of the one
  authority; the bounds-from-`requires` seam keeps consuming the typed-trees
  helpers unchanged.
- Resolved 2026-06-12: positive proof-context operator selection landed — only
  facts in the CURRENT context can select a domain-operator meaning. Spelled
  binary uses are now recorded as operator evidence (`build_operator_facts`
  gains a `Binary` arm; builtin-only arithmetic with no spelled candidates
  stays unrecorded and untouched), and a post-flow pass
  (`operators/selection.rs`, run from `build_check_facts` after flow facts
  exist) admits a domain-owned candidate only when the LEFT operand's domain
  membership is PROVEN by the semantic contexts entering the statement — the
  same invalidation-adjusted contexts the call-`requires` discharge reads, so
  caller `requires`, call `ensures`, and interleaved-mutation invalidation all
  participate. Selection ruling from chapter 8's "participates only if it
  exposes a unique operator meaning" text: exactly ONE admissible (proven)
  domain meaning wins the expression over the builtin — the `requires`
  deliberately narrowed the context; ZERO admissible domain meanings leave the
  ordinary meaning in place when one exists (unique root spelled candidate, or
  the builtin scalar operation for primitive operands → evidence status
  `BuiltinFallback`) and reject otherwise (`Inadmissible`, the positive-proof
  error); TWO or more admissible domain meanings are ambiguous (largely
  precluded by the declaration-level competing-meanings rejection). What
  selection PRODUCES: evidence (`CheckedOperatorFacts` records the winning
  meaning; `selected_candidate` exposes it) — domain operators have no bodies,
  so a selected meaning never changes lowering (no hidden runtime tag, per the
  chapter), and as an honesty guard a selected binary-spelling meaning that
  carries `requires` contracts is rejected loudly because contract discharge at
  spelled binary use sites is not wired yet (slice `[]`/`[..]` discharge
  through the ranges seam is unaffected). Canaries:
  pass `domains/domain_operator_proven_fact_selects_meaning` (+ suite test
  asserting the domain meaning is the recorded selection),
  pass `domains/domain_operator_unproven_keeps_builtin_meaning` (+ suite test
  asserting `BuiltinFallback`), fail `domains/domain_operator_meaning_unproven`,
  fail `domains/domain_operator_meaning_invalidated_by_mutation`, and the
  previously-unregistered `domains/domain_operator_spelling_selected` (pass)
  and `domains/domain_operator_competing_spelling_meanings` (fail) now run in
  the sweeps.
- Resolved 2026-06-12: `requires` contracts of selected spelled BINARY operator
  meanings now discharge at the use site (`checks/operators/requires.rs`). The
  selected candidate's contract span — preserved in the operator evidence
  precisely for this — yields the `requires` proof facts, each instantiated
  over the actual operands (parameter -> operand positional mapping at `Name`
  nodes, the call-`requires` label-instantiation precedent from
  `checks/contracts/labels/calls.rs`; operators have no `self` and no `result`
  binder) and proven against the semantic contexts entering the use's
  statement — the same invalidation-adjusted contexts the selection pass and
  the call-`requires` discharge read. Membership clauses prove via
  `domain_implies` + place/value label match; boolean clauses decompose
  And/Or like the call prover and accept direct boolean facts or
  domain-membership-derived facts (`domain_proves_expression_label`).
  Unproven clauses report the indexed seam's contract-naming attribution
  shape: ``cannot prove `b in Quantity::Additive` — the `requires` of
  `Quantity::Additive::add` (spelled `+`)``. The honesty guard that rejected
  contract-carrying binary selections outright is retired — selections are
  now checked, not refused. Slice `[]`/`[..]` uses keep discharging through
  the ranges seam, unchanged. Canaries:
  pass `domains/domain_operator_requires_discharged` (caller facts prove both
  the selecting membership and the operator's `requires`),
  fail `domains/domain_operator_requires_unproven` (same shape minus the
  `right`-operand fact, asserting the attribution diagnostic).

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
    Soundness-adjacent, not mere dead-weight avoidance. DECISION for Zach (not
    fenced unilaterally): until drop lowering lands, either (a) FENCE — a type
    with a non-empty `drop` body is a compile error/warn (the 5 existing drops
    canaries have EMPTY bodies and stay green), or (b) accept + mark ch16
    aspirational. Real fix = emit the drop-machine call at the tracked state-exit
    point (reverse declaration order; skip on move-out per ch16's move-guard
    edges). See memory drop-bodies-not-executed.
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

### Array, Vec, String, And Views

- [ ] Design `Vec[T]` as owned dynamic storage with length and capacity (surface
  declared; real storage/lowering pending).
- [ ] Back `Array::as_slice`/`as_mut_slice` with real boundary-primitive
  lowering (declared as contracts today).

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
  NEXT-SESSION PLAN (scoped 2026-07-04, NOT started — a focused session, not a
  loop tick): change checked `Expression::Integer(i64)` →
  `Integer(i64, Option<PrimitiveType>)` (signedness+width; `None` = the current
  domain-neutral literal). Blast radius = 41 construct/match sites across
  `omega-state-values` (bindings/folding/simplify), `omega-instruction-selection`
  (guards, writes/static_values, writes/subslice_copy, storage_places/{expressions,
  static_values}), and the checked-trees def — mostly mechanical (`Integer(v)` →
  `Integer(v, _)` / `Integer(v, None)`). Then (a) POPULATE at substitution
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
  Confirms this is a real representation change, not a one-site patch.
  UNIFIED ROOT with the domain hole below: `Expression::Integer(i64)` is
  metadata-free, so every const-substitution/fold strips BOTH the operand's
  signedness/width (this entry) AND its arithmetic domain (next entry). A single
  metadata-carrying-constant (or metadata-aware fold) fix closes both.
- [x] const-fold DOMAIN hole — FIXED 2026-07-04. The domain half is now fully
  sound: Saturating (field+local) clamps, Trapping FIELD traps, and the last
  broken case — **Trapping LOCAL** `let b: i32 in Trapping = a + a` — now traps
  natively too. FIX (frame_slots.rs `trapping_frame_slot_constant_overflow_write`):
  the frame-slot static-integer store arm now checks the slot's domain and, for a
  Trapping out-of-range folded constant, re-emits a `bound±1` Trapping binary write
  so the ud2 fires (mirroring the field path's `trapping_constant_overflow_write`).
  Saturating stays pre-clamped at the fold (never reaches here out of range).
  Locked by `expressions/arithmetic_domain_trapping_let_overflow` (aborts). Full
  suite (571) + interp differential green. Memory `decision-17-const-fold-domain-hole`.
  NOTE: the SIGN half (const-folded `>>`/`/`/`%` misfold) is a SEPARATE, still-open
  bug that DOES need metadata-on-constant — see `shift-right-signedness-const-fold`.
- [~] ORACLE f32 rounding — PARTLY FIXED 2026-07-04. The interpreter now rounds a
  `Value::Float` to f32 at THREE type-aware seams (evaluator.rs): the Assignment
  store, the LocalData store, and the transition-arg param binding (`bind_frame`)
  — each mirroring the integer `apply_arithmetic_domain`/`wrap_to_width` wrap.
  This kills the FIELD-ACCUMULATION divergence (`a:f32; a=a+1; a=a+1` past 2^24
  plateaus at 16777216 in native AND interp, was interp 16777218) AND the
  transition-arg-accumulation divergence. Locked by canaries
  `arithmetic/f32_field_store_rounding` + `arithmetic/f32_transition_arg_rounding`
  (run + differential, in canary_suite AND interpreter RUN_CANARIES). Full suite +
  interp differential green. REMAINING (still open): a fully-inline
  `let x:f32 = 16777216.0+1.0+1.0`
  yields 16777218 — the host const-FOLDER folds the whole expression in f64 before
  the store rounds, losing intermediate f32 rounding. Native + interp now AGREE
  (no divergence — oracle reliable again) but both are imprecise vs true f32.
  Fully fixing needs per-OP f32 rounding, which needs the binary op's result type
  (same metadata-on-values gap as the integer const-fold class). Memory
  `float-f32-computed-in-f64`.
- [ ] DESIGN Q + divergence (probe 2026-07-04): a shift by an amount >= the
  operand WIDTH diverges native-vs-interp, and the semantics are UNDECIDED.
  `i32 1 << 40`: native masks the count to the register width (x86 `shl`: 40 & 31
  = 8 → 256); the interpreter does `(l as i64).wrapping_shl(40)` (masks to 64 →
  1 << 40, truncated to i32 = 0). In-range shifts (amount < width) agree + are
  correct — only out-of-range amounts diverge, so f32… no, INTEGER shift
  differentials are unreliable for out-of-range amounts. QUESTION for Zach (per
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
  interp = -1 (`f.trunc() as i64` SATURATES to i64::MAX, truncated to i32 = -1).
  Both garbage; in-range casts agree. Parked repro
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
- [ ] Reduce duplicate descriptor assumptions remaining across backend crates.
  PARTIAL 2026-07-04: added `PrimitiveType::scalar_byte_size()` as the single
  source of truth for scalar byte widths and collapsed the two exact byte-size
  duplicates onto it (`binary_table_writes.rs scalar_primitive_byte_size`,
  `wire_plans.rs primitive_wire_size`). STILL DUPLICATED: three near-identical
  `primitive -> TypeLayout` matches (`omega-layout/sizing.rs primitive_type_layout`,
  `omega-instruction-selection/.../storage_places.rs primitive_layout`,
  `omega-runtime-storage/layout.rs primitive_layout`). They agree on the scalar
  1/2/4/8 cases but differ in ABI-context source (`target` vs `input.runtime_abi`
  vs `context.target`) and in how the String/fat-descriptor layout is spelled
  (slice_descriptor vs text_descriptor vs hardcoded `2*pointer`). Consolidating
  needs a shared helper parameterized by (pointer_size, pointer_alignment, string
  TypeLayout) -- NOT a route through `omega-layout` directly, which would add a
  crate dependency the architecture-layer DAG likely forbids (probably why the
  duplication exists). Focused refactor; verify the String spellings are truly
  equivalent (they appear to be 2*pointer either way) while collapsing.
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
  STILL OPEN (NON-silent today, so lower priority): value-position call ARG
  type-compat (`f(true)` for an i32 param) is the documented
  `validate_call_arguments_handles` frontier -- the arg's own analysis errors, not
  a miscompile. And an RHS whose class we don't resolve (a value-call returning
  bool into an i32 field) would slip -- but value-position calls are separately
  fenced. Revisit if a silent case surfaces.
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
