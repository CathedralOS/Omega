# Tasks — GUI samples on macOS (`Gui`/`Input`/`Clock` native on aarch64)

> **Goal:** get the EXISTING gui samples running natively on macOS/aarch64 —
> `samples/gui/window_app`, `window_demo`, `windowed_calculator`. A real window
> opens, the animated framebuffer blits, events pump, ESC / close quits. **No image
> file decode** (BMP/PNG is explicitly out of scope). The Omega surface stays
> unchanged: the samples keep their current `Gui`/`Input`/`Clock`/`Console` boundary
> traits; we only add a macOS BACKEND behind them.

## Current state

- **`Gui` is Win32-only.** The `Gui` boundary (`window_create`, `get_dc`, `blit`,
  `msg_peek`/`msg_translate`/`msg_dispatch`, `is_window`, `window_destroy`) maps to
  user32/gdi32 symbols in `foundation/omega-calling-conventions/src/windows.rs`, and
  the code notes those encoders "exist only for x86_64 today" — on other targets a
  `Gui` call is a deliberate compile error.
- **darwin binds nothing GUI.** `foundation/omega-calling-conventions/src/darwin.rs`
  covers only `Filesystem`/`Process`/`Stdin`/`Stdout`/`Stderr`. So on macOS the
  samples' `Gui.*`, `Input.key_state`, AND `Clock.sleep` are all unbound.
- **The samples' model is a Win32 software blit + message pump** (see
  `samples/gui/window_app/main.omg`): create window → get DC → per frame fill a
  64×64 `pixels: [i32; 4096]` framebuffer and `blit(dc, dst_w, dst_h, src_w, src_h,
  pixels, info: [i32;11])` → pump up to 16 messages → `is_window`/`key_state(27=ESC)`
  quit paths. `blit` is `StretchDIBits`; the `[i32;11] info` is a Win32
  BITMAPINFOHEADER; pixels are 32-bit BGRA/BGRX.
- **aarch64 host-call ABI (from the fs work) is INTEGER/POINTER only** — args in
  x0–x7, one stack scalar, int/deref returns. NO float/double args (v-registers), NO
  struct-by-value, NO indirect struct returns (x8). This is the crux for any *native*
  Cocoa path (macOS windowing is unavoidably float/struct-heavy: `NSWindow
  initWithContentRect:` takes an `NSRect` = 4 doubles in v0–v3).

## Two strategies (pick one — this is the key decision)

### Strategy A — Objective-C shim (RECOMMENDED for "just get the samples on mac")

Write a small per-target Cocoa shim (`.m`) exposing **flat C functions with only
int/pointer args/returns**, compiled with `clang -framework Cocoa -framework
CoreGraphics` and linked into the mach-o. The `Gui`/`Input`/`Clock` darwin bindings
point at those shim symbols. This **sidesteps the entire arm64 float/struct ABI
project** — every host call stays integer/pointer, which the existing encoder already
does. Cost: ~150–250 lines of Objective-C + a link-step change + the darwin binding
rows. This is how most non-ObjC runtimes stand up a Cocoa window.

### Strategy B — native `objc_msgSend` from Omega (purist, no shim)

Bind `objc_getClass`/`sel_registerName`/`objc_msgSend` and drive Cocoa + Core
Graphics entirely from Omega. Requires the **arm64 float/HFA/struct-by-value +
indirect-return calling convention** in the host-call encoder first (a dedicated
multi-fire effort like the fs deep fixes), then ~30–40 `objc_msgSend` calls. Much
larger; keep as the long-term "no C in the tree" option, not the path to the samples.

> **Judgement call to record (D-gui-strategy):** default to **A** for the samples.
> Revisit **B** only if a no-shim policy is required. Everything below assumes A.

## Work items (Strategy A)

1. **[ ] Confirm the native link step can take an extra object + frameworks.** The
   aarch64 backend links via clang (fs work: clang + codesign). Verify we can add
   `gui_darwin.o` and `-framework Cocoa -framework CoreGraphics` to the final link
   (and that codesign still succeeds for a windowed binary — no special entitlements
   needed for a basic NSWindow). If the link is hard-coded, add a hook for
   target-specific extra objects/frameworks.
2. **[ ] Write the shim `gui_darwin.m`** (flat C ABI, all int/ptr):
   - `omega_gui_window_create(int32_t w, int32_t h) -> void*` — lazily
     `NSApplication sharedApplication`, `setActivationPolicy:Regular`,
     `activateIgnoringOtherApps:YES`; make an `NSWindow` (w×h, titled/resizable), set
     its `contentView` to an `NSImageView` (scale-to-fit), `makeKeyAndOrderFront:`.
     Return the NSWindow* (0 on failure).
   - `omega_gui_blit(void* win, const uint32_t* px, int32_t w, int32_t h) -> int32_t`
     — build a `CGImage` from the BGRA buffer (`CGColorSpaceCreateDeviceRGB` +
     `CGDataProviderCreateWithData` + `CGImageCreate`, bitmapInfo =
     `kCGImageAlphaNoneSkipFirst | kCGBitmapByteOrder32Little` for BGRX; watch
     row-order vs the Win32 bottom-up DIB), wrap in an `NSImage`, `setImage:` on the
     view. Returns 0/nonzero.
   - `omega_gui_pump(void* win) -> int32_t` — non-blocking
     `nextEventMatchingMask:NSEventMaskAny untilDate:distantPast inMode:default
     dequeue:YES` + `sendEvent:`; track ESC (keycode 53) and window-close (a shim-
     internal `NSWindowDelegate windowWillClose:` sets a flag — that callback is
     inside the shim, NOT a reverse call into Omega). Return a small status code
     (0=none, 1=event, 2=quit-requested).
   - `omega_gui_is_alive(void* win) -> int32_t` — `[win isVisible]` && not closed.
   - `omega_gui_destroy(void* win) -> int32_t` — `[win close]`, release.
   - `omega_input_key_state(int32_t vk) -> uint64_t` — report the tracked ESC state
     (map VK 27 → keycode 53). Extend to more keys as samples need.
   - `omega_clock_sleep(uint32_t ms)` — `usleep(ms*1000)`.
3. **[ ] darwin binding rows + operand arms** for `Gui`/`Input`/`Clock` → the shim
   symbols (same mechanism as the fs ops: capability/op → symbol, an operand arm per
   signature; all int/ptr, so no new encoder work). **Map the Win32-shaped ops** onto
   the shim (semantic mapping below). The `pixels: [i32;4096]` and `[u64;6]` message
   buffer are passed as POINTERS — the fs work already materializes fixed-array-arg
   pointers.
4. **[ ] Interpreter backend** for `Gui`/`Input`/`Clock` — a HEADLESS stub so the
   samples still run in the interpreter (and differential/coverage don't break): open
   no real window, succeed all calls, report "no event / alive", and quit after N
   frames (or on a synthetic ESC). Keeps the samples runnable on both engines.
5. **[ ] Run the samples natively** — `window_app`/`window_demo`/`windowed_calculator`
   open a window and animate on real macOS; ESC and window-close quit.
6. **[ ] CI canary** `native_gui_window` — create a window, blit one frame, pump a
   few (non-blocking) frames, destroy, exit 0. Runs headless-safe without a human
   (bounded frame count; no blocking `[NSApp run]`).

## Semantic mapping (Win32 op → macOS shim behavior; keeps the trait unchanged)

| Gui op (existing) | macOS shim behavior |
|---|---|
| `window_create(cls,title,style,x,y,w,h) -> u64` | `omega_gui_window_create(w,h)`; ignore cls/title/style/x/y (or apply). Returns NSWindow* |
| `get_dc(window) -> u64` | return the window handle again (sample only checks `> 0`) |
| `blit(dc,dw,dh,sw,sh,pixels,info) -> u32` | `omega_gui_blit(win, &pixels, sw, sh)`; ignore `info`/`dw`/`dh` (or use dw/dh to size the window) |
| `msg_peek(msg) -> u32` | `omega_gui_pump(win)`; stash the status in `msg[0]`; return have/quit |
| `msg_translate` / `msg_dispatch` | no-ops (pump already dispatched) — return 0 |
| `is_window(window) -> u32` | `omega_gui_is_alive(win)` |
| `window_destroy(window) -> u32` | `omega_gui_destroy(win)` |
| `Input.key_state(vk) -> u64` | `omega_input_key_state(vk)` (ESC first) |
| `Clock.sleep(ms)` | `omega_clock_sleep(ms)` |

## Gotchas / decisions to record

- **D-gui-shim:** a compiled `.m` lives in the tree + link step (breaks the "no C in
  the tree" purity the fs work held). Accepted for the samples; the native-objc path
  (Strategy B) remains the purist alternative.
- **Activation/focus:** a bare CLI mach-o CAN show an `NSWindow`, but needs
  `setActivationPolicy:Regular` + `activateIgnoringOtherApps:YES` to be visible +
  focused. No `.app` bundle required for a basic window.
- **No reverse callbacks into Omega:** presentation via `NSImageView`+`NSImage`
  (not a `drawRect:` subclass); event handling via the non-blocking pump (not
  `[NSApp run]` + an app delegate). Any objc callbacks (window-close) stay INSIDE the
  shim. Omega has no guest-callback ABI, so keep it that way.
- **Pixel format:** the framebuffer is 32-bit BGRA/BGRX, 64×64; Win32 DIBs are often
  bottom-up. Match the `CGBitmapInfo` byte order + row order or the image is
  swizzled/flipped. Small but real.
- **Interpreter differential:** the headless stub must produce a deterministic,
  bounded run (quit after N frames) so `samples_compile`/differential stay green.
- **key_state coverage:** start with ESC (27→53); add keys only as
  `window_demo`/`windowed_calculator` require them (check each sample's `key_state`
  calls).
- **Verification is partly interactive** (a human sees the window); the CI canary
  covers the non-interactive path (open→blit→pump→destroy→exit).

## If Strategy B is ever chosen (the gating item)

The single prerequisite is **arm64 float/HFA/struct-by-value + indirect-return
support** in the host-call encoder (`omega-isa-aarch64` + `omega-calling-conventions`
+ the width/relocation lockstep sites the fs work established). Prove it with a canary
that `objc_msgSend`s a float-arg method (e.g. `NSMakeRect`/`initWithContentRect:`)
before building any Cocoa on top. Everything else (objc runtime boundary, the ~30–40
Cocoa/CoreGraphics calls) is then plumbing over that.
