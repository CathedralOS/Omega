# Window application

This persistent macOS application renders an animated 64×64 framebuffer into
its 256×256 window. Close the window or press Escape while it has focus to
exit. Creation returns an application-owned `Window`; rendering and event
handling borrow it, and shutdown consumes it and releases the Cocoa resources.
The named `Gui`, `Input`, and `Clock` providers select shared standard-library
machines without hidden provider state.

From the repository root, with the current `omega` executable:

```sh
omega update --project samples/gui/window_app --target macos_arm64 --offline
# If review is required, inspect the reported file and decide its pending rows.
omega update --resume --project samples/gui/window_app --offline
omega run --keep samples/gui/window_app/main.omg
```

Run `--resume` only after an update requests review. Local source identities
belong to the checkout, so moving it requires fresh review. The run command
omits `--target` to execute on the host; `--keep` retains the reported build
folder and its `window_app.app`. Launch that bundle separately with:

```sh
open -n /path/reported/by/omega/window_app.app
```

The implemented sample providers target macOS ARM64. The Build declaration also
names a Windows entry, but this sample does not yet supply Windows providers.
GUI intent selects a macOS application package with the authored identifier
`com.omega.window-app`; see the [publication contract](../../../wiki/spec/build/macos_application.md).

Rendering, focus-gated input, and normal close behavior require observation in
an interactive desktop session. A process that merely stays alive does not
establish that acceptance. `window_demo` provides the bounded 60-frame check
of the shared window API.
