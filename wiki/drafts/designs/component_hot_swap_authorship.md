# Replacing the editor's preview Console

An editor writes its own messages to the terminal. Its preview pane can use a
provider built after the editor shipped. Changing the preview should leave
the editor's messages on the terminal.

## What each Build produces

| Build | Selection | Product |
| --- | --- | --- |
| Editor | `Build::select_provider::<Console, TerminalConsole>(b)` | Editor with a fixed Console for its own messages. |
| Preview V1 | `Build::export_provider::<Console, TerminalConsole>(b)` | Independently built terminal provider image. |
| Preview V2 | `Build::export_provider::<Console, QuietConsole>(b)` | Quiet provider image for the same target and requirement. |

A supplier may use a target-specific implementation. The host authorizes
loading and applies its receiver policy to the supplied code, contracts,
target, imports and resources. The host can accept V2 without knowing its
implementation name at build time. [Build selection and exports](../../spec/build/provider_selection.md#independent-library-exports-and-owned-providers)

## What changes in one launch

The editor reads each image through its selected filesystem provider. It
handles short reads, enforces the receiver's size limits and closes the file.
This example assumes the release tool also supplies the corresponding
descriptions from the two provider builds. The loader checks each description
against its image and receiver policy before returning a provider. `?`
propagates failure.

```omega
let mutable preview: Provider<Console> = load_provider::<Console>(v1_image, v1_description)?;
let before: @Console = Provider::get(@preview);
before.write_line("one");                         // V1 prints this

let next: Provider<Console> = load_provider::<Console>(v2_image, v2_description)?;
preview = next;                                   // exclusive write to this local
let after: @Console = Provider::get(@preview);
after.write_line("two");                          // V2 discards this
Console::write_line("editor is still running");   // still reaches the fixed terminal
```

The editor stores the loaded value in its local `preview`. Each provider keeps
the code and operation record for its own admitted installation alive. V1's last
borrow ends after the first write. If loading V2 fails, assignment never runs
and a caller handling the error can keep using V1. In this stateless example V1
has no outstanding registrations or sessions when the assignment disposes of it.

Changing only the lifetime of one callable makes the replacement illegal:

```omega
let ops: @Console = Provider::get(@preview);
let write = ops.write_line;
preview = next;          // rejected: write still borrows the current provider
write("too late");
```

For a stateful provider, a session or callback that needs old code must keep
its loan or hold independently established custody of that installation.
Transferring device ownership, draining buffered output and preserving state
require the service's own handoff contract. [Replacement and coexistence](../../spec/build/component_publication.md#replacement-and-coexistence)

The [owned provider contract](../../spec/build/component_publication.md#owned-runtime-providers)
governs these examples. The [implementation board](../../../TASKS.md#embedding-and-interpreted-components)
tracks loading, projection and the lifetime checks.
