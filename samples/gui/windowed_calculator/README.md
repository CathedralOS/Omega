# Windowed calculator

A persistent software-rendered macOS calculator: a native window, per-frame
message pumping, keyboard input, a 64x64 software framebuffer stretched into
the client area, calculator state with memory recall, and explicit
state-machine dispatch for every interaction. Digits, `+ - * /`, Enter or
`=`, Backspace, `C`, `S` (sign), `M` (memory add), `R` (memory recall);
Escape or closing the window exits. The header documents no exit oracle:
the application waits for user input.

From the repository root with the development `omega` CLI:

```sh
omega update --project samples/gui/windowed_calculator --target macos_arm64 --offline
# If review is required, inspect the reported file and decide its pending rows.
omega update --resume --project samples/gui/windowed_calculator --offline
omega run --keep samples/gui/windowed_calculator/main.omg
```

The GUI subsystem keeps the console surface to exit plus a status line.
Build declares Windows too, but this sample currently supplies macOS
implementations through ordinary SDK machines selected by Build.

## Status (macOS ARM64, 2026-10-31)

Normal package review, source checking and native application publication pass
with the calculator's scalar predicates restored. The SDK uses whole-array
string contracts and Build-owned native bindings; its window helpers are
ordinary machines, with target selection retained in Build.

Terminal comparison currently refuses at runtime admission:
`native-provider-adapter: private native sponsor supplies no additional service capability`.
The AppKit and Objective-C requirements declare additional service reach that
the private native sponsor does not supply. This is an unavailable comparison,
not an observed program exit. Their declared ceilings remain intact.

The application has no exit oracle, so the sample gate builds it without
launching it (`no-oracle`). Rendering, keyboard input and normal close require
observation in a desktop session; a successful build does not establish them.
