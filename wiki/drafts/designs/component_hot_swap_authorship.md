# Who can replace the editor's Console?

An editor downloads a release while it is running. Its next call to `Console::write_line` should reach the new provider. A read already waiting for a key must finish on the provider that started it. The open design question is what an Omega author writes to make that happen.

The [first-bind ruling](../../spec/build/component_publication.md#first-binding-of-a-runtime-bound-slot) fixes the observable behavior. The source spellings below are proposals for review. Delete this note when a checked-in, runnable Omega example and its governing contract show the complete handoff.

## Start at the call site

**`editor/main.omg`** uses the existing Console interface. A controlled keyboard makes the example reproducible.

```omega
use {std}/console.omg as console;

public machine run() reaches StandardInput + StandardOutput blocks crashes none {
    |console|::Console::write_line("Press a key");
    let key = block |console|::Console::read_byte();
    |console|::Console::write_line("Read finished");
}
```

**`editor/build.omg`** selects the app entry and describes its product. For a build-selected first provider the established selection is:

```omega
Build::select_provider::<|console|::Console, |initial|::TerminalConsole>(
    plan, CompositionMode::Independent
);
```

Independent composition preserves a call boundary across which new calls can reach a different implementation. A caller cannot permanently inline V1 across that boundary. An explicitly runtime-bound slot can instead start empty. The spelling for declaring it is open. Build can describe the slot and compile product code and separately deployable artifacts. The editor owns when it downloads, accepts and activates an update.

## Where does startup get authority?

Two installations of the same editor can load the same bytes. They still have two distinct Console slots. Knowing the trait, component filename or provider type cannot let code change either slot. Each proposed design needs to identify the owner of the actual slot storage.

| Who created the installation? | Source of first-bind and update authority | What reviewers need to show |
| --- | --- | --- |
| Launcher or embedding host | The installer owns the fresh slot for this instance and can delegate its sole binding authority under a checked entry contract. | Name the entry parameter, its exact occurrence and the installed backing. Show that a second launch or a differently scoped slot cannot reuse it. |
| Editor's own bootstrap | Startup creates the instance that will run the editor and keeps authority over its own slot. | Show how the editor's compiled, statically named Console import resolves to that exact instance. Allocating an unrelated mutable cell proves nothing. |

**`editor/startup.omg`**, candidate for the first row. This is an excerpt of the supplied-slot design. The proposed component operations and types have no settled spelling. The caller in this example must actually own both the slot and the directory. An ordinary shell invocation supplies neither argument by virtue of this signature.

```omega
machine start(
    console_slot: |components|::UnboundSlot<|console|::Console>,
    plugins: @mutable |components|::ComponentDirectory
) {
    let loaded = |components|::load_for(@console_slot, plugins, "console.so");
    let provider = match loaded in |components|::LoadResult::Loaded {
        true -> loaded.provider,
        false -> |components|::linked_provider::<
            |console|::Console, |headless|::HeadlessConsole
        >()
    };
    let updater = |components|::bind_first(console_slot, provider);
    run_with_updates(updater);
}
```

The excerpt shows the successful bind path. If installation itself can still fail, its failure result must preserve the ability to bind an allowed fallback or exit. Holding a directory grants only its filesystem operations. The installer also needs independent authority to load and publish. The example assumes those authorized services are available. A complete candidate must spell out their supply.

An explicit `HeadlessConsole` can discard writes and report EOF on reads if that behavior fits the installed Console contract. An editor requiring an interactive terminal should take the startup error instead. An application that deliberately models an optional feature can branch once in startup. Its callers need no optional wrapper once the chosen path has a bound slot.

Earlier sketches used `Build::supply_component_updater` to fill a reflected field, or `Build::bind_provider_initializer` to nominate a startup callback. A field identifies a destination, but supplies no owner or backing. A callback selected as a product declaration could run after launch, but an ordinary call from the selected entry already expresses the fallback policy. Neither sketch settled who can issue the first handle. A source-visible `HotSwappable<Console>` on every call would duplicate the existing static requirement and provider-slot mechanism.

## What actually changes when V2 arrives?

In the two-build demonstration, `editor/build.omg` produces the running app and
V1. A separate release build binds V2 to the same shared Console declaration
and publishes that independent component. The editor consumes the candidate
component from that result without starting a second editor.

**`editor/updates.omg`**, candidate authoring shape for the success path. The
updater is an ordinary machine in the running editor. Its caller delegates an
actual release feed, version directory, loader and the retained update handle.

```omega
let manifest = |releases|::fetch_signed_manifest(feed);
let release = |releases|::verify_console_release(manifest);
let files = |releases|::download_verified_files(feed, versions, release);
let image = |components|::stage(loader, files);
let candidate = |components|::resolve_export::<|console|::Console>(image);
let old = |components|::publish(@mutable console_updater, candidate);
|components|::retire_when_idle(old);
```

Only `publish` switches future calls. Admission failure leaves the existing
provider serving. Passing `old` to retirement tracks the exact displaced
installation while calls drain. Reviewers should suggest the failure result
and custody for each proposed consuming operation.

The editor's updater obtains a candidate through supplied services. It can download a prebuilt image and description, or use an embedded compiler to build a candidate from source. The receiving installation checks the exact artifact, target, interface, authority and resource envelope under its own policy. Compiling locally or authenticating a release does not by itself discharge those obligations. A native loader capable of running foreign initializers must account for that execution before allowing the load.

For Linux, picture V1 and V2 in separate version directories. The loader makes V2 available in the process and resolves its exports. The installer checks the chosen export for this Console slot. Publication changes which installed provider new calls enter. A running V1 call retains the old image and state. Persisting the selected release for the next launch is a separate application action with its own failure behavior. Replacing an entire executable on restart can be sufficient for products with no live continuity requirement. [Executable installation](../../spec/build/executable_installation.md) · [Deployment ownership](../../spec/build/component_publication.md#deployment-ownership)

A standard library could offer a portable component loader over platform implementations. On a hosted target the first loader can be linked into the executable and use an authorized OS loader. Cathedral can supply its own installation service. A Psi host can admit an interpreted component without mapping new native code. Supplying a library by default grants no live installation authority. For the audit names and examples that distinguish loading from foreign execution, see [native images and foreign calls](../../spec/language/effects.md#native-images-and-foreign-calls).

The older core declaration [`NativeCodeExecution`](../../../source/library/core/native_execution_authority.omg) illustrates why the distinction matters. The [macOS GUI interface](../../../source/library/std/macos_gui.omg) uses it for both AppKit loading and Objective-C calls. `NativeCodeImport` would collide in meaning with source-level imports, while `InstallNativeCode` could be read as putting a file on disk. For migration, inspect which operations each binding actually permits before assigning either or both new identities. Merely compiling or running ordinary Omega as native code adds neither reach by itself.

## Quiescence has an observable meaning

1. Observe V1 actually enter `read_byte` with keyboard input withheld.
2. Have the editor's own authorized updater publish V2. A test harness can control timing and observe providers without acting as the updater.
3. Attempt V1 retirement while the read is active. The installer must retain V1.
4. Supply the key. Observe V1 return from the existing read and V2 handle the subsequent `write_line` in the same invocation of `run`.
5. Retire V1 after all its calls and handles release it. Equal stdout alone cannot distinguish a live update from running V1 the whole time.

The [Linux integration test](../../../source-rust/application/tests/independent_component.rs) starts an executable with a separate supplier and checks a fresh launch with that supplier withheld. It demonstrates startup linking. The sequence above exercises live cutover. The [embedding contract](../../spec/build/embedding.md#replacement-and-native-equivalence) requires the same lifecycle for Psi.

A recording session can retain the encoder that opened it for every frame. A driver may retain callbacks, interrupts or outstanding I/O. V2 may drain before V1. If the installation's live-version budget is full, defer or reject V3. A timeout cannot establish safe unload. Moving live sessions, transferring a device and preserving state across a cutover each need their own service contract.

## Questions for reviewers

- Show the first instruction that obtains the right to update this instance. Which existing authority, installed parameter or ownership rule establishes it?
- Show how the compiler accepts a call after successful binding and rejects the same transitive call before binding, including a branch that may fail to install its provider.
- Show what the owner gets back on failed first bind, failed replacement and delayed retirement. Can one blocked V1 session coexist with later versions without losing its custodian?
- For an app-managed slot, show the connection between its storage and the generated call to `Console::write_line`. For a launcher-managed slot, show the launch handoff using the [structured entry example](structured_program_entry.md).
