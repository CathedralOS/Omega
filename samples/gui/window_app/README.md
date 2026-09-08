# Window application

`build.omg` declares `window-app`, selects GUI presentation, and binds the
Windows x86-64 and macOS ARM64 entrypoints. GUI intent is distinct from a raw
PE subsystem value: `Subsystem::Unspecified { value: 2 }` does not request a
GUI application package.

From the repository root, on macOS:

```sh
cargo run -p omega -- --target macos_arm64 --build-dir build/window-app-intent samples/gui/window_app/main.omg
```

On Windows PowerShell:

```powershell
cargo run -p omega -- --target windows_x86_64 --build-dir build/window-app-intent samples/gui/window_app/main.omg
```

Use `mbx run` instead of `cargo run` when the repository build wrapper is
installed. These commands require ordinary accepted package review; absent or
changed policy stops compilation and reports the review to complete. They do
not authorize new admissions automatically.

The macOS command currently stops at that review boundary without an accepted
policy. Build evaluation and the checked compilation retain authored GUI intent;
retaining it through native realization, supplying signing identity, and
publishing a whole validated `.app` remain unfinished. This sample does not yet
establish Finder launch behavior. See the [publication contract](../../../wiki/spec/build/macos_application.md).
