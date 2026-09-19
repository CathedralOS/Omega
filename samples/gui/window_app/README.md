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

Complete that review once per checkout with the package manager:

```sh
target/debug/omega update --project samples/gui/window_app --target macos_arm64
# edit the pending decision rows in build/package-manager/review-macos_arm64.txt
target/debug/omega update --resume --project samples/gui/window_app
```

This publishes `omega.lock` beside `main.omg`. The lock records
checkout-specific local source identities, so a relocated checkout reviews
again rather than reusing another machine's acceptance.

The latest fresh-checkout probe stops at ordinary package review after compiling
the dependencies. It does not establish a current application-lowering failure.
The sample's bare service fields also need migration to the settled
[`Service<R>` entry contract](../../../wiki/spec/build/entry_roots.md#entry-shape-and-arrival-bridge).
After review and that migration, rerun the unchanged render loop before treating
the historical `InvalidUnitMachinePlan` diagnosis as current. See
[MACOS-APPLICATION-PUBLICATION](../../../TASKS.md#immediate-product-closure)
for resume evidence.

The downstream publication machinery is implemented: the authored
`builder.identifier` supplies the CodeDirectory
signing identity and `CFBundleIdentifier`, and publication installs one whole
validated `window-app.app` (`Contents/Info.plist` and
`Contents/MacOS/window-app`) reported through the checked package-root and
inner-executable accessors — exercised today through the test-owned routes,
not this command. This sample does not yet establish Finder launch behavior.
See the
[publication contract](../../../wiki/spec/build/macos_application.md).
