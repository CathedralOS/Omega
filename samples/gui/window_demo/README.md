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

Windows x86_64 native execution validates the 60-frame scanline oracle and
successful device-context release and window destruction. A native observation
also measures the visible client area and changing pixel colors. The GUI
subsystem keeps Windows from attaching a separate console window.

The macOS route retains the existing Cocoa implementation and its resource
ownership. Native macOS execution and LaunchServices were not rerun for this
change. The CLI reports the retained build directory containing
`window_demo.app` on macOS; launch that bundle separately with
`open -n -W <reported-directory>/window_demo.app` to exercise LaunchServices.
