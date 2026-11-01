# Native window demo

From the repository root with the development `omega` CLI, prepare the matching
host target and complete the generated local capability review:

```sh
omega update --project samples/gui/window_demo --target windows_x86_64 --offline
omega update --project samples/gui/window_demo --resume --offline
omega run --keep samples/gui/window_demo/main.omg
```

Edit the pending review decisions before `--resume`. Local acceptance records
project acceptance; it does not claim a package audit occurred. On macOS ARM64,
replace the update target with `macos_arm64`.

The program creates a 256-by-256 client area, renders 60 animated color frames,
releases the drawing context and window, and prints
`PASS: 60 frames blitted (exit 0)`. Closing the window early takes the failure
path and releases the remaining resources. The Windows provider uses the native
frame metrics instead of assuming a fixed titlebar and border size.

Build selects `WindowsGuiProvider` for Windows x86_64 and `MacosGuiProvider`
for macOS ARM64. The implementations own ordinary inline machine bodies;
selection does not create application state. Both supplied implementations are
checked regardless of which host runs the program.

`Main` declares its counter predicates with `requires self`: pumped messages
remain between 0 and 16, frames between 0 and 60, and the pixel index between
0 and 4096. The original dominating guards establish each increment. Width
conversions use `integer::max<i32>()`; packed header fields and Win32 flags use
hex values. Windows uses its native frame metrics for the client area, and
the GUI subsystem keeps it from attaching a separate console window.

On macOS, `--keep` reports the retained build directory containing
`window_demo.app`. Launch the same bundle with
`open -n -W <reported-directory>/window_demo.app` to exercise LaunchServices.
The PASS line establishes the program's frame and cleanup-result checks;
observing the native window's dimensions, changing pixels and disappearance
also checks visible presentation and closure.
