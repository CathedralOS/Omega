# Replacing the editor's Console

Build the editor before either console provider exists. Launch it with a terminal provider, then install a quiet provider while the editor is still running. Its next write reaches the quiet provider. A terminal read that started before the swap can still finish on the terminal.

## Three products, built independently

| Build file | What the author selects | Output |
| --- | --- | --- |
| `editor/build.omg` | Bind ordinary zero-argument `main` as the product entry. Leave the shared Console slot runtime-bound. Designate the editor's install operation, select its backing and filesystem and placement providers for the target, and supply the authority they require. | An executable containing neither console implementation. |
| `terminal/build.omg` | Select the target's terminal implementation satisfying the shared Console contract as a library output. | A target-native component image and its sibling `.component` description. |
| `quiet/build.omg` | Select a console implementation that discards writes and returns end-of-input on reads, under the same contract. | Another independently built image and description for the editor's target. |

The libraries decide what they export. The editor decides who may replace its Console. All three use the same nominal requirement declaration and closed arguments. The receiver matches the actual export, calling and state contracts, target, permissions and resource demands against the editor's frozen policy. It checks that the description belongs to the received image. The host can admit a provider built next year. A provider that also demands an unapproved filesystem import fails admission even if its exported methods have the right signatures. [Verified component descriptions](../../spec/build/component_publication.md#verified-component-descriptions)

## One launch and two publications

The quiet supplier uses `Build::export_provider::<Console, QuietConsole>(b)`.
The editor can name `EditorConsoleInstaller::install` directly as its publisher.
The [Build declarations](../../spec/build/provider_selection.md#independent-library-exports-and-runtime-publishers)
also show how a host selects the installer through its requirement. The trace
below follows one occurrence. How authored code obtains its [update authority](../../../OWNER_QUESTIONS.md#component-publication-authority)
and establishes [first binding for callers](../../../OWNER_QUESTIONS.md#component-first-binding-proof)
remain open.

1. The editor enters `main()` with its Console slot empty. From its ordinary startup code it asks its selected filesystem provider to open `plugins/terminal` and `plugins/terminal.component`, relative to the process working directory. It reads each into an owned buffer up to the receiver's limit, loops over short reads, and closes each opened file on every exit. A missing file ends startup before any Console call.
2. The designated installer checks the exact export against this occurrence's envelope and the native image against the receiver's admission policy. Its target placement provider materializes the admitted sections and relocations in writable, non-executable storage, freezes and validates the final bytes, and completes executable placement and instruction visibility. The result backs the admitted export. The [placement contract](../../spec/build/executable_installation.md#placement-lifecycle) governs that transition on each platform.
3. The installer uses its sole update authority for **this editor's slot** to publish that export. Successful publication establishes the bound fact on the path that enters the editor. An installer that returns success without publishing on an initially empty path fails the contract. Calling Console to log an earlier failure would also be rejected. The editor may instead retry or exit.
4. An editor task enters the ordinary Console code below. Withhold keyboard input until the terminal provider is inside the read. Meanwhile the editor's update task reads `plugins/quiet` and its description through the same filesystem path. It admits and places the replacement while the terminal remains callable. If admission fails, the editor keeps its terminal provider and can retry.

```omega
use {std}/console.omg as console;

public machine run() reaches StandardInput + StandardOutput blocks crashes none {
    |console|::Console::write_line("Press a key");
    let key = block |console|::Console::read_byte();
    |console|::Console::write_line("Read finished");
}
```

5. The installer's publication atomically changes where new calls enter. Every caller wired to this editor's Console now enters the quiet provider. A second editor process keeps its own binding. Attempting to unload the old terminal now must retain it. Supply the key and observe the terminal finish the existing read and the quiet provider receive the subsequent write. Once all old calls, handles and callbacks release it, the installation can retire and unload the terminal.

If both providers need exclusive control of the same device, the installer must quiesce the old owner and transfer that claim before cutover under the service's waiting and recovery contract. Keeping device ownership in a longer-lived service is another valid product design. A blocked read or an outstanding callback still pins the code it can enter. [Replacement and coexistence](../../spec/build/component_publication.md#replacement-and-coexistence)

## Platform and reach

The editor can use the same installer on Windows and Linux with target-selected placement providers. Reading bytes or allocating ordinary storage has its usual reach. Making the image executable reports `NativeImageLoad`; an operation that enters a foreign SDK reports `ForeignCodeExecution`. A loader that can invoke foreign initializers reports both even if this particular image contains none. Switching between two already installed, admitted exports adds neither reach by itself. The candidate never acquires the installer's authority by exporting Console. [Native images and foreign calls](../../spec/language/effects.md#native-images-and-foreign-calls)

A native-only supplier must meet the receiver's native evidence or explicitly accepted opaque-code policy. An interpreted Psi supplier can instead run under an admitted interpreter. Compiling a supplier locally is a separate product choice. [Candidate acceptance](../../spec/build/component_publication.md#candidate-acceptance-and-resources)

The [Linux integration test](../../../source-rust/application/tests/independent_component.rs) covers separate supplier startup. The live test above requires observing two providers during a single launch. [Remaining implementation work](../../../TASKS.md)
