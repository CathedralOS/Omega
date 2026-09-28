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

The receiver uses the settled
[`Service<R>` entry contract](../../../wiki/spec/build/entry_roots.md#entry-shape-and-arrival-bridge):
public boundary requirements, intrinsically established service fields, and no
authored `in Bound` qualification. The render loop and its storage are unchanged.
Provider selection is settled: the sample declares its own provider data and
`provider_defaults` selections, and `omega --check` accepts the receiver's
`Console`, `Clock`, `Input`, and `Gui` fields. The earlier "no selected Fused
providers" reading and the older "0 Terminal attachment identities" diagnostic
were downstream symptoms, not permission to invent an attachment. The check
gate's actual first failure was a missing direct `core::float_operations`
import — `std::macos_gui` imports those operations, but imports are
nontransitive, so each sample names its own selected float operators.

With the review above completed, `omega --target macos_arm64` now stops in
native production: `WindowGuiProvider::window_create` applies boundary
operator `F64::from_i32`, whose selected provider has no requirement-level
Terminal route for Omega to install. That producer/realization dependency is
TASKS.md's OPERATOR-BOUNDARY-CALLS, not provider selection.

Verified recipe (self-contained probe, `ddc66b61b5`, `--target macos_arm64`):
a `pub data` provider plus `machine P::m(&mut self, ..) satisfies T::m { }`
checked adapters, `builder.select_provider<Trait, Provider>()` in `build.omg`,
and one `Service<R>` field on the entry receiver compiles, passes package
review, and publishes native output. Every `Service` field in the program must
route to a selected plan holding at least one checked-adapter row; leaf-only
providers (`via Binding::..` rows alone) cannot join the Fused boundary.
Adapters may call ordinary machines but cannot reach satisfies-leaf callees or
foreign imports (both currently fail realization). The probe's one-field
carrier limit no longer binds this sample: `Main` holds four provider fields
and checked compilation now completes.

After provider wiring, complete the OPERATOR-BOUNDARY-CALLS route above, any
remaining lowering work, and the ordinary checkout-specific package review.
Test-owned acceptance does not complete that review. No app has launched
through this command yet. See
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
