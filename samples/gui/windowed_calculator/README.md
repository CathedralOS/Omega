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
implementations through the shared `omega_language_std::macos_gui` families.

## Status (macOS ARM64, 2026-10-13)

Package review, the whole-package check and the native build succeed with the
library's `macos_arm64` window families selected through the local `Gui`,
`Input` and `Clock` maps. At the base this was measured on, the physical
pipeline first refused the build in the fixed-occurrences spill rewrite
(`WorkBudgetExceeded`, 2,071,390 of 2,000,000 validation steps on a
16,319-register function); `447f9be019d` recalibrated that ceiling and the
build now completes. The sample documents no exit oracle, so the sample gate
builds it without launching it (`no-oracle`); rendering, keyboard input and
normal close still require a desktop session that permits observation.
