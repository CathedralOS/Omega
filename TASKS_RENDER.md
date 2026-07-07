# Tasks — GUI samples on macOS (native, no C shim)

> **AUTONOMOUS LOOP (this file is the source of truth).** A `/loop` runs every ~5 min
> re-reading this file. Cron job id **`6bdd3b5e`** — `CronDelete 6bdd3b5e` to stop
> (when the samples run natively, or blocked only on a user-only decision). Keep
> Current state / work items / decisions current every fire. Push to `origin/main`
> each fire (fetch; rebase if behind; re-verify; push) — disjoint files from the
> other omega-rs work, conflict-free. Gates stay green: Console lowering;
> `omega-instruction-selection`/`omega-relocations`/`omega-calling-conventions` crate
> tests; interpreter coverage.

> **Goal:** run the existing gui samples natively on macOS/aarch64 —
> `samples/gui/{window_app,window_demo,windowed_calculator}`. A real window opens,
> the animated framebuffer blits, events pump, ESC / close quits. **No image file
> decode.** The Omega surface stays unchanged (the samples keep their current
> `Gui`/`Input`/`Clock`/`Console` traits); we add a macOS BACKEND behind them.

## Decision (D-gui-strategy): NO C shim — Omega calls Cocoa directly

Cocoa/Core Graphics are just C: `objc_msgSend`, `CGImageCreate`, etc. are plain C
functions, and the fs seam already proves Omega calls libc natively (aarch64
host-call sequences straight into libsystem). The ONLY missing piece is the arm64
**float / small-struct ABI**. Build that once and every native API — Cocoa, Core
Graphics, Metal, CoreAudio, libm, any double-taking syscall — becomes directly
callable, forever, with no shims. A `.m` shim would be throwaway work + a clang/objc
build dependency + it abandons the "no C in the tree" invariant, and it would be
ripped out the day the ABI lands anyway. **Rejected.**

Dynamic linking is orthogonal and already done: the frameworks are dylibs resolved
by dyld via the SAME lazy-bind stub/relocation path the fs work uses to call
`_open`/`_stat`. A shim adds nothing there — it only moves where the float/struct
marshalling happens (clang's output vs our own encoder).

## Current state

- **`Gui` is Win32-only.** `window_create`/`get_dc`/`blit`/`msg_peek`/`msg_translate`/
  `msg_dispatch`/`is_window`/`window_destroy` → user32/gdi32 in
  `foundation/omega-calling-conventions/src/windows.rs`; those encoders are x86_64
  only (a `Gui` call on other targets is a deliberate compile error).
- **darwin binds no GUI.** `darwin.rs` covers only `Filesystem`/`Process`/std
  streams — so `Gui.*`, `Input.key_state`, AND `Clock.sleep` are all unbound on macOS.
- **aarch64 host-call ABI: integer/pointer + SCALAR FLOAT (both directions) DONE.**
  Args in x0–x7 + one stack scalar + double args in v0–v7 (fires 5–6); int/deref
  returns + double return from d0 (fire 6). STILL MISSING: HFA struct-by-value
  (double args in v0–v3, item #2) and indirect struct return via x8 (item #3, avoid
  if possible). HFA is the remaining gating gap for `NSWindow initWithContentRect:`.
- **Sample model** (`samples/gui/window_app/main.omg`): create window → get DC → per
  frame fill a 64×64 `pixels: [i32;4096]` framebuffer (32-bit BGRA/BGRX) and `blit` →
  pump ≤16 messages → `is_window` / `key_state(27=ESC)` quit paths.

## The one gating capability: arm64 float + small-struct ABI

The host-call encoder (`omega-isa-aarch64` + the width/relocation lockstep sites in
`omega-calling-conventions`) needs, in dependency order:
1. **Float/double args in v0–v7** — materialize double constants, move into v-regs
   (`fmov`/`ldr`). (Also f32 for `CGFloat` on 32-bit, but macOS aarch64 `CGFloat` is
   a double.)
2. **HFA struct-by-value in v0–v3** — arm64 passes a homogeneous-float aggregate in
   consecutive v-registers: `NSRect` = 4 doubles, `CGSize`/`CGPoint` = 2. This is
   what `NSWindow initWithContentRect:` and `NSImage initWithCGImage:size:` require.
3. **(Only if unavoidable) indirect struct return via x8** — favor
   pointer/scalar-returning methods so the window path never needs this; revisit only
   if a required method returns a struct by value.

Same discipline as the fs deep-work: each capability lands with a RUN-VERIFIED canary
(disassemble + run) BEFORE anything is built on it.

## Work items (native, ABI-first)

1. **[x] Float-arg + float-return host calls DONE (fires 5–6)** — double args in v0–v7,
   double return from d0; run-verified `round_nearest`/`square_root`/`hypotenuse`
   canaries. **GROUNDED (fire 1, 2026-07-12):** the hard part (instruction encodings)
   already EXISTS — `aarch64/primitives/float.rs` has `encode_float_move_from_gpr`
   (GPR→v `fmov`, for loading an arg's bits into a v-register) and
   `encode_float_move_to_gpr` (v→GPR, for spilling a v0 return to store). Omega
   already has float values + float dispatch guards (`aarch64/dispatch.rs`
   `encode_float_compare`). So this is WIRING, not new encoding. The gap:
   - `representations/omega-abstract-operations/src/instruction/operand.rs`
     (`InstructionOperandKind`) has `RuntimeScalarInteger` but no float variant — add
     `RuntimeScalarFloat { byte_offset, byte_count }` + accessor.
   - `omega-isa-aarch64` `append_call_operands`: for a float operand, load its bits
     into a scratch GPR then `encode_float_move_from_gpr` into the next V-register —
     track `next_vreg` SEPARATELY from `next_register` (arm64 has independent x/v arg
     sequences). Its `operand_width` = load(≤12) + fmov(4), so arg-offset relocation
     accounting stays automatic (summed from `operand_width`, like the fs stack arg).
   - Float RETURN: the result lives in v0; spill via `encode_float_move_to_gpr` then
     the normal result store — gate it like the deref/stack-restore cases
     (`widths.rs` + `data_addresses.rs` lockstep), keyed on a `returns_float()`
     predicate on `HostOperationKey`.
   - `darwin.rs` binding row for a libm symbol + the host-op operand arm + checker
     routing an `f64` boundary param to the float operand kind.
   - **VERIFY FIRST next fire:** does the checker/interpreter even accept an `f64`
     param + return on a `boundary trait machine`? Write `boundary trait Libm {
     machine sqrt(x: f64) -> f64; }` and see if it type-checks before touching the
     backend (may be a small checker gap to close). SMALLEST CANARY: `sqrt(16.0) ==
     4.0` (or `pow`) — disassemble (`otool -tv`: `ldr d0,…; bl _sqrt; fmov x,d0; str`)
     + RUN.
   - **FIRE 2 (2026-07-12) — NO CHECKER GAP; pure backend.** Compiled a probe
     `boundary trait Libm { machine square_root(value: f64) -> f64; }` +
     `self.root = self.lib.square_root(16.0); transition self.root == 4.0 {..}`
     (probe committed at `canaries/run/float/sqrt_probe/main.omg`). It gets PAST the
     checker (the `root == 4.0` f64 proof obligation even passes) — the only errors
     are lowering: (a) "host lowering: `Libm.square_root`: no native lowering for
     target Aarch64/MachO", (b) "`self.root = square_root(16)` needs runtime storage
     write lowering", (c) "AssignmentValue ... needs runtime value lowering". So f64
     boundary params/returns are ACCEPTED; the whole job is the backend vertical.
   - **TWO operand enums to extend** (both currently int/ptr only, NO float; neither
     has a stack/float variant): shared `representations/omega-abstract-operations/
     src/instruction/operand.rs::InstructionOperandKind` and per-arch
     `backend/instruction_set_architectures/omega-isa-aarch64/src/operand.rs::
     Aarch64CallOperand`. Add `RuntimeScalarFloat { region?, byte_offset, byte_count }`
     to both + the map between them. THEN: darwin binding row for the libm symbol; the
     host-op operand builder (route an f64 param/return to the float operand);
     `append_call_operands` float-arg arm (load bits → GPR → `encode_float_move_from_
     gpr` into next V-reg, `next_vreg` tracked separately); float-return store (v0 →
     `encode_float_move_to_gpr` → store) gated on a `returns_float()` predicate;
     `widths.rs` operand_width for the float operand. Adding an enum variant breaks all
     exhaustive matches (x86_64 encoder/width/data_addresses) — handle each (x86_64
     untested → a "not yet" arm is fine). Land it as ONE focused vertical, build green.
   - **FIRE 3 (2026-07-12) — full blast radius CONFIRMED by compiler; do the whole
     vertical in ONE pass (no clean half-green checkpoint — an unconstructed variant
     dead-code-fails, so the builder must CONSTRUCT it in the same change).** The
     operand flows through a 3-layer pipeline, each its own enum needing a
     `RuntimeScalarFloat { region, byte_offset, byte_count }` variant + pass-through:
     (1) `omega-abstract-operations/.../instruction/operand.rs::InstructionOperandKind`;
     (2) `omega-target-operations/.../instruction/operand.rs` (target enum ~L166; its
     accessor methods use `_ => None`, no arm needed); (3)
     `omega-isa-aarch64/src/operand.rs::Aarch64CallOperand`. Mapping arms:
     `pipeline/omega-abstract-operations-to-target-operations/src/operands.rs`
     (abstract→target) + the target→Aarch64CallOperand mapper; plus a formatter arm in
     `omega-backend-report/src/codegen/operands.rs`, the x86_64 "not yet" arm, and
     `widths.rs`. REAL work: (a) aarch64 `append_call_operands` float-arg arm (load bits
     → scratch GPR → `encode_float_move_from_gpr` into next V-reg, `next_vreg` tracked
     separately; width = load+fmov); (b) the builder
     `host_operations/operands.rs::scalar_argument_operand_at` detects a float arg via
     `resolve_runtime_storage_primitive_type_in_table` and emits `RuntimeScalarFloat`
     (constructs it → no dead code); (c) darwin binding + `HostOperation` + op arm.
   - **DO `lround` FIRST (float ARG only), not `sqrt`.** `machine round_nearest(x: f64)
     -> i64` → darwin `lround`: double arg in d0, LONG return in x0 (existing int-return
     path). Proves float-arg passing WITHOUT the v0 float-return store — a strictly
     smaller first vertical. Canary `round_nearest(3.7) == 4` (`ldr d0,…; bl _lround;
     str x0,…`), disassemble + run. A follow fire adds the float-RETURN store (v0 →
     `encode_float_move_to_gpr`, gated on a `returns_float()` predicate), proven by
     `sqrt(16.0) == 4.0`. (Reverted this fire's partial edits to keep the tree green.)
   - **FIRE 4 (2026-07-12) — operand-layer plumbing + aarch64 float-ARG encoder LANDED,
     GREEN.** Added `RuntimeScalarFloat { region, byte_offset, byte_count }` to all three
     operand enums (`omega-abstract-operations`, `omega-target-operations`,
     `omega-isa-aarch64::Aarch64CallOperand`) + the abstract→target mapping arm + a
     backend-report formatter arm. Implemented the REAL aarch64 float-arg marshalling in
     `append_call_operands` (mod.rs): a `next_vreg` counter (separate from
     `next_register`); for a float operand `adrp/add` to the region base into scratch
     x16, load the bits (`ldr x`/`ldr w` by byte_count), then
     `encode_float_move_from_gpr(byte_count, next_vreg, 16)` → value lands in the next
     v-reg. `widths.rs operand_width` reports 16 (adrp+add+load+fmov) so BL/result-store
     relocation offsets stay automatic — no manual lockstep. VERIFIED: workspace builds;
     isa-aarch64 5/5; native fs harness 55/55 (zero regression). The variant is
     UNCONSTRUCTED so far (harmless dead-code warning — no `deny(warnings)`).
   - **✅ FIRE 5 (2026-07-12) — FLOAT-ARG CALLING CONVENTION LANDED + RUN-VERIFIED.**
     Constructed the variant + proved it end-to-end: `HostCapability::Math` +
     `HostOperation::RoundNearest` (+ `from_name`/`name`/`from_str`/`to_str` +
     `returns_value` extended to Math) in `lib.rs`; `darwin_import("Math",
     "round_nearest","_lround")` + `insert_platform_lowering` in `darwin.rs`; the
     `(Math, RoundNearest)` op arm + a `float_argument_operand_at` builder helper (emits
     `RuntimeScalarFloat`). ONE extra site the compiler didn't flag (a runtime
     `unreachable!`): the target→aarch64 operand mapper `omega-instruction-selection/
     src/operands.rs::aarch64_call_operand` needed a `runtime_scalar_float()` branch,
     which meant adding that accessor to the `InstructionOperandLike` trait + both impls
     (target + x86_64→None). AND the region relocation: `omega-relocations/src/
     data_addresses.rs` collects an operand's region via an accessor or-chain —
     `runtime_scalar_float()` added there so the arg's `adrp` is relocated to the region
     base (without it the `adrp` stayed at page 0 → `lround` read the Mach-O header →
     wrong result). PROVEN: `canaries/pass/float/native_float_arg` (`round_nearest(3.7)
     == 4`) compiles + RUNS natively, exit 4, `otool` shows `ldr; fmov d0,x16; bl
     _lround`; regression `native_float_arg_exits_4` (native fs harness 56/56);
     canary_suite **zero new failures** (A/B diff; baseline is 86 post-fs-thread, not
     mine). **Scalar float ARGS work.** Float RETURN (a method returning `f64`) is a
     small follow-up: after `BL`, `encode_float_move_to_gpr(v0→GPR)` then the normal
     store, gated on a `returns_float()` predicate — needed only if a Cocoa/CG call we
     use returns a double (most return objects/ints); defer until one does.
   - **✅ FIRE 6 (2026-07-06) — FLOAT-RETURN + MULTI-FLOAT-ARG LANDED + RUN-VERIFIED.**
     Did the follow-up now (not deferred): float RETURN is the same +4-lockstep shape as
     `dereferences_result`, so mirroring it was cheap AND it closes the whole *scalar*
     float ABI (both directions) in one fire — the natural next canaries (HFA `cabs`, any
     objc double getter) need it anyway. Added `HostOperationKey::returns_float()`
     (`lib.rs`, true for `Math::SquareRoot`/`Hypotenuse` only — `round_nearest` returns
     i64 so it is excluded); a new encoder
     `encode_host_call_sequence_value_returning_float_from_operands` (`isa-aarch64/aarch64/
     mod.rs`) = the plain value-returning encoder + `fmov x0,d0` after `BL` (the result
     place stores bit-identically to an i64); dispatch arm in `encoding/host.rs` BEFORE
     the plain `returns_value()` arm; +4 lockstep at `widths.rs` and `offsets/
     data_addresses.rs` (result store sits 4 bytes later — same discipline as deref).
     Ops `SquareRoot`(`_sqrt`)/`Hypotenuse`(`_hypot`) + darwin imports/lowerings + op arms
     reusing `float_argument_operand_at`. GOTCHA (already known, re-hit): a bare float
     LITERAL arg (`square_root(16.0)`) has no storage slot → `float_argument_operand_at`
     returns None → empty operands → encoder hard-errors "no result storage operand"; the
     value must flow through a FIELD (`self.input = 16.0; square_root(self.input)`).
     PROVEN: `canaries/pass/float/native_float_return` (`sqrt(16.0)`→4.0, round-tripped
     through the proven `round_nearest` to dodge float-`==`-in-guard) exits 4;
     `native_float_two_args` (`hypot(3,4)`→5.0, args in v0+v1) exits 5. `otool` on the
     sqrt binary shows the exact new instr: `fmov d0,x16; bl _sqrt; fmov x0,d0; …; fmov
     d0,x16; bl _lround`. Regressions `native_float_return_exits_4` +
     `native_float_two_args_exits_5` (native fs harness now 58/58); crate tests green
     (isa-aarch64 31, instruction-selection 10, relocations 5, calling-conventions 5);
     canary_suite **zero new failures** (A/B failure-SET diff — identical 87-failure
     baseline w/ and w/o my changes; the remembered "86" had drifted after rebases).
     **Scalar float ABI (args v0–v7 + double return d0) is now COMPLETE both directions.**
     Next is HFA (struct-of-N-doubles in v0–vN-1) — item #2.
2. **[x] HFA struct-by-value args — DONE AT THE ABI LEVEL (fire 7).** ⚑ **KEY FINDING:**
   the aarch64 host-call encoder's integer (`next_register`→x0..) and float
   (`next_vreg`→v0..) counters are **already fully independent** (verified by reading
   `append_call_operands` in `isa-aarch64/aarch64/mod.rs`): a float arg consumes ONLY a
   v-register, an int/ptr arg ONLY an x-register. So an AArch64 homogeneous-float
   aggregate — `NSRect` = 4 doubles → v0–v3, `CGSize`/`CGPoint` = 2 → v0–v1 — occupies
   the SAME v-registers that N separate `f64` args do, and a mixed `objc_msgSend(x0=self,
   x1=sel, NSRect→v0–v3, x2=style, x3=backing, x4=defer)` call already encodes correctly
   (the two counters advance independently). **DECISION (D-hfa): model NSRect/CGRect as N
   consecutive `f64` boundary params in the Gui backend lowering — NO generic Omega
   struct-by-value language feature.** This is ABI-identical to HFA-by-value for
   pure-double aggregates and needs zero new encoder work. PROVEN THIS FIRE:
   `canaries/pass/float/native_float_three_args` — `fma(2,3,4)=10.0` (three f64 args in
   v0/v1/v2 via `Math::fused_multiply_add`→`_fma`), round-tripped through `round_nearest`,
   exits 10; `otool` shows `fmov d0,x16; fmov d1,x16; fmov d2,x16; bl _fma`. v3 follows by
   the identical `next_vreg += 1` mechanism (no special-casing) and will be run-verified
   the moment the window-creation `objc_msgSend` first relies on it. Regression
   `native_float_three_args_exits_10` (native fs harness 59/59).
   ✅ **fire 10: v0–v3 RUN-VERIFIED for real (a 4-double struct-by-value).** Wired
   CoreGraphics as a bindable framework (`darwin_import_library` routes `_CG*` →
   CoreGraphics; `DARWIN_COREGRAPHICS_PATH`; `HostCapability::CoreGraphics` +
   `RectMaxX`/`RectMaxY` ops → `_CGRectGetMaxX`/`_CGRectGetMaxY`, `returns_float` +
   `returns_value` extended). A `CGRect` = 4 doubles marshals as an HFA into v0–v3; op
   arm `[result, x, y, w, h]` (4 `RuntimeScalarFloat`). PROVEN: `canaries/pass/objc/
   cgrect_hfa` — `CGRectGetMaxX({10,20,30,40}) = v0+v2 = 40`, `CGRectGetMaxY = v1+v3 = 60`,
   both round-tripped through `round_nearest` → exit 6; `otool` shows `fmov d0,x16; fmov
   d1,x16; fmov d2,x16; fmov d3,x16; bl _CGRectGetMaxX`. All four v-registers proven placed.
   ⚑ **BONUS FIX (D-import-pruning): reference-driven imports.** DISCOVERED that since fire
   8 (objc bindings added), EVERY program imported ALL ~55 darwin bindings — so even a
   pure-fs program loaded libobjc + Foundation + AppKit (a latent landmine: a headless env
   where AppKit can't load would break ALL programs). Root cause: the object-plan inserts an
   Import symbol for every binding (`omega-object-file-planning/src/symbols.rs`).
   FIX at the mach-o layer: `install_import_thunks` now creates thunks only for imports
   REFERENCED by a relocation (a host-call `bl` targets the thunk via one) — dead thunks
   forced needless dylib loads. Result: native_stat → **1 dylib** (libSystem), cgrect →
   libSystem+CoreGraphics, framework_classes → all 5. Native harness 65/65; canary_suite
   zero new failures. **HFA is now fully proven (isolated v0–v3 + the independent-counter
   mix); the window `initWithContentRect:` is HFA v0–v3 + trailing x2–x4 scalars — next.**
3. **[~] Objective-C runtime boundary — STARTED (fire 7b): `objc_getClass` DONE; multi-dylib
   linking SHIPPED.** ✅ **The real blocker — MULTI-DYLIB LINKING — is solved.** The mach-o
   now emits an `LC_LOAD_DYLIB` per linked dylib (libSystem ordinal 1, libobjc ordinal 2)
   and binds each import to its library's ordinal. What landed:
   - `omega_calling_conventions::darwin_import_library(symbol)` (mirrors
     `windows_import_library`): routes `_objc_*`/`_sel_*`/`_class_*`/… → libobjc, else
     libSystem. Exported consts `DARWIN_LIBSYSTEM_PATH`/`DARWIN_LIBOBJC_PATH`.
   - `omega-image-macho`: `MachoDylib` {path, versions} with `LIBSYSTEM`/`LIBOBJC` specs +
     `command_size()` = `align_to(24 + path + 1, 8)` (libSystem = 56, historical value);
     generic `write_macho_load_dylib_command`. `MachoImportThunk` carries `library`
     (from `darwin_import_library(symbol)`); `macho_dylib_list` = ordered de-duped dylibs
     (libSystem always first); `macho_bind_info` emits `0x10 | ordinal` per thunk;
     `plan_macho_image` takes the dylib list and computes `command_count = 10 + dylibs.len()
     + …` and per-dylib `sizeofcmds`. macho now deps calling-conventions (as PE does).
   - SAFETY: images with NO objc symbols get exactly 1 dylib (56 bytes) → **byte-identical**
     output; the whole 59-canary fs corpus is unaffected (verified).
   - `HostCapability::ObjectiveC` + `HostOperation::GetClass` (→ `_objc_getClass`,
     `returns_value`); darwin import+lowering; operand arm `[result u64, name path-pointer]`
     reusing `path_pointer_operand`. Canary declares its own NUL-free byte-domain
     (`domain [u8]::ClassName when no_nul`).
   - PROVEN: `canaries/pass/objc/objc_get_class` — `objc_getClass("NSObject") != 0` exits 7;
     `otool -l` shows BOTH `LC_LOAD_DYLIB` (libSystem + libobjc). Regression
     `objc_get_class_exits_7` (native harness 60/60); macho unit 2/2; canary_suite **zero
     new failures** (A/B, identical 87 baseline).
   **fire 8: `sel_registerName` + `objc_msgSend` DONE (2-arg, 3-arg-scalar).** Added
   `HostOperation::RegisterSelector`(→`_sel_registerName`), `MsgSend`(2-arg) and
   `MsgSendScalar`(3-arg, scalar 3rd) — `send`/`send_scalar` share the `_objc_msgSend`
   symbol; op arms `[result, recv→x0, sel→x1, (arg→x2)]` reuse `scalar_argument_operand_at`
   / `path_pointer_operand`. PROVEN: `canaries/pass/objc/objc_alloc` — `[[NSObject class]
   alloc] != 0` exits 7 (sel_registerName + 2-arg send + non-null id); `objc_msgsend_scalar`
   — `[NSObject respondsToSelector:@selector(alloc)] == 1` exits 8 (3-arg send, SEL arg in
   x2, BOOL return read cleanly from x0 — `disasm: ldr x2,[…]; bl _objc_msgSend`). Native
   harness 62/62; crate gates green; canary_suite zero new failures (identical 87).
   ⚑ **DISCOVERY / D-objc: `objc_getClass` only sees classes from LOADED dylibs.** libobjc
   provides the runtime + `NSObject` (root class), so NSObject works — but `objc_getClass
   ("NSString")` returns **nil** (Foundation not loaded), which stalled the first
   NSString-length canary (it silently took the nil path). **Framework classes
   (Foundation `NSString`, AppKit `NSApplication`/`NSWindow`) require their frameworks
   LOADED.** So the window path needs a THIRD/FOURTH dylib load for
   `/System/Library/Frameworks/{Foundation,AppKit}.framework/…`. My multi-dylib machinery
   already supports N dylibs, BUT the dylib list is built from IMPORTED SYMBOLS — a
   framework loaded only for its class-registration side effects has no imported function.
   Two routes (decide next fire): (a) bind the class DATA symbol `_OBJC_CLASS_$_NSWindow`
   (forces the framework load AND yields the Class pointer directly — no `objc_getClass`),
   or (b) force-load a fixed framework set when `Gui` is used. Route (a) is cleaner + more
   correct; it needs a new operand path (import a DATA symbol, use its address as a value).
   `send_string` (char* arg) was implemented then removed — it can't be run-verified with
   libobjc alone (no NSObject method takes a char*); it returns with Foundation loaded.
   ✅ **fire 9: FRAMEWORK LOADING DONE + objc boundary COMPLETE.** Chose route (b) —
   **D-objc-load: auto-load Foundation + AppKit + CoreGraphics whenever the objc runtime
   is used** (simpler than route (a) and sufficient — `objc_getClass` resolves ANY class
   in a loaded framework, so no class-data-symbol binding is needed). In `macho_dylib_list`,
   if any thunk binds libobjc, append `MachoDylib::{FOUNDATION,APPKIT,COREGRAPHICS}` AFTER
   libobjc (keeps ordinal 2 stable; they carry NO imported symbols, loaded purely for class
   registration). Install-name paths + compat versions confirmed from a system app; compat
   set to 1.0.0 so the dyld check always passes. Re-added `send_string` (char* arg → x2,
   provable now). PROVEN: `framework_classes` — `NSString`+`NSApplication`+`NSWindow` all
   non-null → exit 9 (`otool -l` shows all 5 LC_LOAD_DYLIB; **AppKit loads cleanly from a
   bare CLI mach-o, no .app bundle, no window-server hang at load**); `nsstring_length` —
   `[[NSString alloc] initWithUTF8String:"hello"] length] == 5` → exit 5 (full char*-arg
   msgSend round-trip). Native harness 64/64; crate gates green; canary_suite zero new
   failures (A/B vs FRESH baseline — the +1 `runtime_indexed_struct_field_rmw` is
   pre-existing drift from an unrelated rebase, present with AND without my changes).
   **The full objc→Cocoa surface (class lookup, selectors, msgSend 2/3-arg scalar+string,
   framework classes) is now proven — the window path is unblocked.**
4. **[~] Window without reverse callbacks — NSWindow INSTANTIATED (fire 11).** ✅ A real
   `NSWindow` is now created natively from Omega. Added `ObjectiveC::send_rect(recv, sel,
   x, y, w, h, a, b, c) -> u64` → `_objc_msgSend` — the MIXED HFA-plus-scalar send: the
   `NSRect` (4 doubles) goes in v0–v3 and the three trailing scalars in x2–x4 (op arm
   `[result, recv, sel, x_f, y_f, w_f, h_f, a, b, c]`; the independent x/v counters place
   them without interference). PROVEN: `canaries/pass/objc/nswindow_init` —
   `[NSApplication sharedApplication]` then `[[NSWindow alloc] initWithContentRect:
   {0,0,200,150} styleMask:15 backing:2 defer:0]` builds a non-null window AND
   `[win styleMask] == 15` → exit 3. `otool` shows the textbook AAPCS layout: `x0=recv,
   x1=sel, fmov d0..d3=rect, x2=#0xf, x3=#0x2, x4=#0x0, bl _objc_msgSend`. Headless-safe
   (never ordered on-screen). Native harness 66/66; canary_suite zero new failures (A/B vs
   FRESH baseline — 5 pre-existing `runtime_frame_indexed_*` drift, present w/ and w/o my
   changes). ✅ **fire 13: FULL VISIBLE WINDOW + FRAME PRESENTED.** The complete Cocoa
   object graph now works natively: `[NSApplication sharedApplication]` +
   `setActivationPolicy:0`; a CGImage-backed `NSImage` (`initWithCGImage:size:`) attached
   via `[imageView setImage:]` to an `NSImageView` (`initWithFrame:`) set as the window's
   `setContentView:` and `makeKeyAndOrderFront:`. The one NEW ABI piece: `ObjectiveC::
   send_image_size(recv, sel, image, w, h)` — a scalar arg (x2) + an `NSSize` (2 doubles →
   v0,v1); everything else reuses `send`/`send_scalar`/`send_rect`. PROVEN:
   `canaries/pass/objc/present_frame` — builds the whole graph and asserts `[imageView
   image] != nil` → exit 5 (headless-safe: the check is the object graph, not on-screen
   visibility; the window DOES show on a session box). Native harness 68/68; canary_suite
   zero new failures. **The static-window + blit path is DONE end-to-end.** REMAINING:
   the animated pump + input (items #6/#7) and wiring behind the samples' traits (#8).
5. **[~] CGImage blit — framebuffer → CGImage DONE (fire 12); NSImage/view wrap next.**
   ✅ The pixels-to-image half works: a `[i32;N]` BGRA framebuffer becomes a `CGImage`.
   **JUDGEMENT CALL: use `CGBitmapContextCreate` (7 args, all registers) + snapshot,
   NOT `CGImageCreate` (11 args, 3 on the STACK).** Same result, avoids building a
   stack-arg ABI capability. Added `HostCapability::CoreGraphics` ops (all int/ptr args,
   results in x0): `color_space_rgb()`→`CGColorSpaceCreateDeviceRGB` (0 args),
   `bitmap_context(data, w, h, bpc, stride, space, info)`→`CGBitmapContextCreate` (7 args
   x0–x6, `data` = framebuffer POINTER like an fs buffer), `bitmap_context_image(ctx)`→
   `CGBitmapContextCreateImage`, `image_width(img)`→`CGImageGetWidth`. PROVEN:
   `canaries/pass/objc/cgimage_blit` — a 4×4 BGRA buffer (bitmapInfo `0x2006` =
   `kCGImageAlphaNoneSkipFirst|kCGBitmapByteOrder32Little`) → `CGImageGetWidth == 4` →
   exit 4; `otool` shows `x0=&pixels, x1=4, x2=4, x3=8, x4=0x10(stride), x5=space,
   x6=0x2006, bl _CGBitmapContextCreate`. Native harness 67/67; canary_suite zero new
   failures. REMAINING: `NSImage initWithCGImage:size:` (needs a send with a CGImage
   scalar + an `NSSize` = 2 doubles in v0,v1 — a small new mixed variant) + `NSImageView
   setImage:` (`send_scalar`), then the window presents the frame. NB the sample is
   top-down 32bpp; CGBitmapContext is also top-down, so NO row flip (unlike the Win32
   bottom-up DIB) — good.
6. **[~] Event pump — NON-BLOCKING PUMP PROVEN (fire 14).** The drain-the-queue call the
   animated samples run every frame works natively. Added `ObjectiveC::send_scalar4(recv,
   sel, a, b, c, d)` → `_objc_msgSend` (4 scalar args → x2–x5) for
   `nextEventMatchingMask:untilDate:inMode:dequeue:`. PROVEN: `canaries/pass/objc/
   event_pump` — a bounded 3× loop of `[NSApp nextEventMatchingMask:0xffffffff
   untilDate:[NSDate distantPast] inMode:"kCFRunLoopDefaultMode" dequeue:1]` completes
   without hanging → exit 6 (`untilDate:distantPast` = non-blocking; `otool` shows `ldr
   x2=mask, x3=date, x4=mode, mov x5=1, bl _objc_msgSend`). The regression test SPAWNS with
   a 20s deadline and fails loudly rather than hanging the suite if the pump ever blocks.
   Native harness 69/69; canary_suite zero new failures. REMAINING for full quit-handling:
   `[NSApp sendEvent:evt]` (guarded on `evt != nil`, `send_scalar`) + `[window isVisible]`
   poll for the close path (`send`, BOOL) — both reuse existing sends; wire in item #8.
7. **[ ] `Input.key_state` + `Clock.sleep` darwin** — key state polled from the event
   stream; `sleep` → `usleep(ms*1000)` (plain int arg, existing mechanism).
8. **[ ] Wire behind the existing trait ops** (mapping below) so the samples are
   UNCHANGED. The `pixels:[i32;4096]` / `[u64;6]` message buffers pass as POINTERS
   (fs already materializes fixed-array-arg pointers). — **⚑ THE INTEGRATION; ALL ABI
   PRIMITIVES ARE PROVEN, this is composition + provider wiring (fire 15 investigation).**
   HOW MULTI-OP LOWERING WORKS: a boundary call → ONE `HostCall` carrying a LIST of host
   operations (`insert_platform_lowering(plan, trait, method, [ops], data)`); the ENCODER
   emits the sequence, threading intermediates in registers (e.g. win32 `write_line` →
   `[get_std_handle, write_file]`). But a ~8-call `window_create` with class-lookup +
   runtime selector interning + intermediate-ptr threading is HEAVY bespoke encoder code
   per op. **DECISION D-gui-backend: the macOS Gui/Input/Clock backend is an OMEGA module**
   — one trait-op-sized machine per op, built from the PROVEN ObjectiveC/CoreGraphics/libc
   boundary primitives (the fs-wrapper pattern: a machine making host calls, proven
   native). PROVEN THIS FIRE: `canaries/pass/objc/gui_backend_valuecall` — a value-called
   `open_window` machine composes getClass/alloc/`initWithContentRect:` into a non-null
   NSWindow → exit 7. The one remaining COMPILER GAP is PROVIDER WIRING — dispatching the
   sample's `boundary trait Gui` call to that macOS implementation machine. Two routes:
   **(W1)** per-target boundary-trait provider (compiler recognizes on darwin that Gui/
   Input/Clock are satisfied by the `macos_gui` module's machines and lowers
   `self.gui.window_create(..)` to a value-call into it) — clean architecture, NEW compiler
   feature, and if the provider is a SEPARATE `data` type it needs the through-FIELD
   value-call fix (task #45, deferred); the same-data-type value-call already works (this
   fire). **(W2)** bespoke composite host-op lowering per Gui op in the aarch64 encoder —
   no new feature but heavy/fragile. **Recommend W1.** Prereq: decide whether the macos_gui
   provider is same-data (avoids #45) or a separate data type (needs #45). Next fires:
   build the `macos_gui` Omega module + the W1 dispatch, then #9/#10.
9. **[ ] Interpreter headless stub** for `Gui`/`Input`/`Clock` — open no real window,
   succeed all calls, report "no event / alive", quit after N frames — so the samples
   stay runnable on both engines and differential/coverage stay green.
10. **[ ] Run the samples natively** + a CI canary `native_gui_window`
    (open → blit one frame → pump a few non-blocking frames → destroy → exit 0),
    headless-safe (bounded frame count, no blocking pump).

## Semantic mapping (existing Win32-shaped op → native macOS behavior)

| Gui op (existing, unchanged) | native macOS behavior |
|---|---|
| `window_create(cls,title,style,x,y,w,h) -> u64` | NSApp setup + `NSWindow` (w×h) + `NSImageView` contentView; return the NSWindow* |
| `get_dc(window) -> u64` | return the window handle again (sample only checks `> 0`) |
| `blit(dc,dw,dh,sw,sh,pixels,info) -> u32` | CGImage from `&pixels` (sw×sh) → `setImage:`; ignore `info`/`dw`/`dh` (or size the window from dw/dh) |
| `msg_peek(msg) -> u32` | non-blocking `nextEvent`; stash status in `msg[0]`; return have/quit |
| `msg_translate` / `msg_dispatch` | `sendEvent:` (or fold into `msg_peek`); no-op otherwise |
| `is_window(window) -> u32` | `[window isVisible]` (poll, no delegate) |
| `window_destroy(window) -> u32` | `[window close]` |
| `Input.key_state(vk) -> u64` | tracked NSEvent state (map VK 27 → keycode 53; ESC first) |
| `Clock.sleep(ms)` | `usleep(ms*1000)` |

## Gotchas / decisions to record

- **Activation/focus:** a bare CLI mach-o CAN show an `NSWindow`, but needs
  `setActivationPolicy:Regular` + `activateIgnoringOtherApps:YES` to be visible +
  focused. No `.app` bundle required.
- **No guest callbacks:** present via `NSImageView` (not `drawRect:`); detect close by
  polling `isVisible` (not a `NSWindowDelegate`). Omega has no OS→guest callback ABI;
  keep every objc call one-way (Omega → Cocoa).
- **Pixel format:** framebuffer is 32-bit BGRA/BGRX, 64×64; Win32 DIBs are often
  bottom-up. Match `CGBitmapInfo` byte order + row order or the image swizzles/flips.
- **Interpreter differential:** the headless stub must be deterministic + bounded
  (quit after N frames) so `samples_compile`/differential stay green.
- **key_state coverage:** start with ESC (27→53); add keys only as
  `window_demo`/`windowed_calculator` need them.
- **Verification is partly interactive** (a human sees the window); the CI canary
  covers the non-interactive open→blit→pump→destroy→exit path.
