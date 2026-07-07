# Tasks — GUI samples on macOS (native, no C shim)

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

1. **[ ] Float-arg support** — double host-call args in v0–v7. Canary: call a libm
   function with a double arg/return (e.g. `pow(2.0, 10.0) == 1024`) or an
   `objc_msgSend` to a float-arg method. Disassemble + run.
2. **[ ] HFA struct-by-value args** — pass `NSRect` (4 doubles) / `CGSize` (2) in
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
