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
- **aarch64 host-call ABI is INTEGER/POINTER only** (from the fs work): args in
  x0–x7, one stack scalar, int/deref returns. NO float/double args, NO
  struct-by-value, NO indirect struct returns. This is the one real gap.
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

1. **[ ] Float-arg + float-return host calls** — double args in v0–v7, double return
   from v0. **GROUNDED (fire 1, 2026-07-12):** the hard part (instruction encodings)
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
2. **[ ] HFA struct-by-value args (NEXT)** — pass `NSRect` (4 doubles) / `CGSize` (2) in
   v-regs. Canary: `NSMakeRect(...)` round-trip or `[NSWindow ... initWithContentRect:
   styleMask:backing:defer:]` produces a non-nil window.
3. **[ ] Objective-C runtime boundary** — `objc_getClass`, `sel_registerName`,
   `objc_msgSend` (int/ptr/string args → mostly the existing mechanism; strings are
   NUL-terminated rodata like fs paths). Canary: `[[NSString alloc] initWithUTF8String:]`
   → `length` returns the right count.
4. **[ ] Window without reverse callbacks** — `NSApplication sharedApplication` +
   `setActivationPolicy:Regular` + `activateIgnoringOtherApps:YES`; `NSWindow` with an
   `NSImageView` contentView (present via the image view, NOT a `drawRect:` subclass —
   Omega has no guest-callback ABI). Static empty window that stays up via a bounded
   non-blocking pump.
5. **[ ] CGImage blit** — build a `CGImage` from the `[i32;4096]` BGRA framebuffer
   (`CGColorSpaceCreateDeviceRGB` + `CGDataProviderCreateWithData` + `CGImageCreate`,
   bitmapInfo `kCGImageAlphaNoneSkipFirst | kCGBitmapByteOrder32Little`; watch row
   order vs the Win32 bottom-up DIB), wrap in `NSImage`, `setImage:` on the view.
6. **[ ] Event pump + quit** — `nextEventMatchingMask:NSEventMaskAny
   untilDate:distantPast inMode:default dequeue:YES` + `sendEvent:` (non-blocking, no
   `[NSApp run]`/delegate). Close-detect by POLLING `[window isVisible]` each frame (no
   callback). `key_state` from tracked `NSEvent` keyDown/keyUp (ESC keycode 53).
7. **[ ] `Input.key_state` + `Clock.sleep` darwin** — key state polled from the event
   stream; `sleep` → `usleep(ms*1000)` (plain int arg, existing mechanism).
8. **[ ] Wire behind the existing trait ops** (mapping below) so the samples are
   UNCHANGED. The `pixels:[i32;4096]` / `[u64;6]` message buffers pass as POINTERS
   (fs already materializes fixed-array-arg pointers).
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
