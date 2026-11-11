# `invokes` as an entry ceiling

Ratified 2026-11-10. `invokes` was specified as synchronous entry into a
callback, but one sentence let a row name "an internally selected binding",
and the library used it to list the host slots a provider calls. A portable
requirement then named its implementations, and so did every public wrapper
above it (the first two are excerpts from `std/filesystem.omg`):

```omega
// before: the portable contract names two platforms
machine read(file: File, buffer: &mut [u8], count: u64) -> IoResult
    blocks
    reaches FilesystemContentRead
    invokes PosixFilesystemHost
    invokes WindowsFilesystemHost;

// after: it says what a caller can observe
machine read(file: File, buffer: &mut [u8], count: u64) -> IoResult
    blocks
    reaches FilesystemContentRead;

// unchanged: a requirement that can run the caller's code keeps its row
machine execute<const Callback: Handler::read>() -> u64
    reaches none
    crashes none
    invokes Handler;
```

Adding a backend that calls a different host slot edited the portable trait
and revalidated every dependent. The host calls are implementation
dependencies; they already bottom out in the reach the requirement publishes.

The normative owner is
[direct synchronous invocation](../../spec/language/effects.md#direct-synchronous-invocation).
Track implementation under `INVOKES-ENTRY-CEILING` in [TASKS.md](../../../TASKS.md).
Delete this record when the implementation and acceptance are complete.

## What changes

1. `invokes` bounds synchronous arrival at a callback or inbound entry,
   through any number of intermediate calls: a callable argument, a statically
   bound or stored callback, or a binding to an `EnvironmentToProgram` slot.
2. A call to a `ProgramToProvider` slot is not itself an entry and takes no
   row, whoever authored the provider. Entry demands reached through it still
   propagate through the callee's published `invokes`.
3. A provider's possible calls to provider slots are its realization
   inventory: derived for a checked body, kept with the component's outgoing
   requirements, and outside the portable requirement's interface identity.
4. Composition checks that every reachable entry fits the calling
   requirement's `invokes`, and separately that the complete graph of possible
   edges, inventory included, is acyclic. Both rerun on provider replacement.

## What stays

- Omission on a bodyless requirement or a callable type is closed: no entry.
  Omission on a checked body infers and publishes; a written clause there is
  an optional ceiling.
- A row may name a parameter path or a requirement identity. A requirement
  identity is still how a registered handler with no parameter path is named.
- Storing a callable is storage and registering one creates a future root.
  Neither is a synchronous entry. Calling a stored callable counts on the
  operation that calls it.
- A native binding's clause, or its omission, is its admitted claim. Unknown
  native behavior is not an empty claim. No new receipt.
- An invoked entry contributes its reach and operational envelope to the
  calling invocation. No `invokes` clause, present or absent, makes an open
  invariant window safe.
- No parent closure, no rename, no new keyword. A row that no selected
  provider uses is a broader ceiling; it gets no diagnostic and no automatic
  rewrite.

## Acceptance cases

| Case | Required result |
| --- | --- |
| A callable makes ordinary downstream service calls and enters no callback | Fits a callable contract that omits `invokes`. |
| A callable calls a supplied callback | Does not fit an otherwise identical contract that omits `invokes`. |
| A downstream service enters a previously registered handler | The outer operation must cover that entry. |
| An application-authored provider of an ordinary slot enters no callback | Adds no `invokes` demand. |
| A provider is replaced by one whose inventory closes a cycle | Rejects at composition with no portable contract edited. |

The first case fails by the letter of the earlier rule: a callable that calls
`Console::write_line` inherited `invokes WindowsConsoleIo`.

## Migration

The order is fixed, because a checker on the earlier rule rejects a provider
that exceeds its requirement's ceiling:

1. Both checkers derive the inventory and apply the entry rule.
2. The library's plumbing rows are removed by review. Privacy of the named
   trait does not authorize removal: a private implementation identity can
   travel through a public interface and be registered by code that cannot
   name it.

`source/library` has about 530 authored rows. The most common targets are
host slots: `WindowsFilesystemHost` 133, `PosixFilesystemHost` 97,
`PosixFilesystemDarwin` 40, `WindowsFilesystemIo` 31, `PosixFilesystemLinux`
31. Forty-six are `invokes FilesystemService` on checked public wrappers.
The rows that stay name what the same signature receives or enters, for
example `invokes StaticInterruptWork` on the static interrupt installer and
`invokes ProgramImage` on `UefiPhysicalEntry::enter`. No row names a
parameter path.
