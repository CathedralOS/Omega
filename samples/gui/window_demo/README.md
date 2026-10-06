# Native window demo

From the repository root with the development `omega` CLI:

```sh
omega update --project samples/gui/window_demo --target macos_arm64 --offline
omega run --keep samples/gui/window_demo/main.omg
```

Complete any pending local capability review using the review file reported by
`update`, then run `omega update --project samples/gui/window_demo --resume`.
Local acceptance does not claim that a package audit occurred.

The program creates a 256-by-256 client area, renders 60 animated color frames,
closes the window and prints `PASS: 60 frames blitted (exit 0)`. The CLI reports
the retained build directory containing `window_demo.app`. Launch that bundle
separately with `open -n -W <reported-directory>/window_demo.app` to exercise
LaunchServices. The window should visibly render; an exit code alone does not
verify that appearance. Closing it early should take the failure path.

Native execution and LaunchServices completion are demonstrated on macOS ARM64.
Pixel and input/close observation remain pending on a session with usable desktop
capture. Build declares Windows too, but this sample currently supplies macOS
implementations.
