# Chapter 20: Memory layout and ABI

Most Omega code never thinks about bytes: a `data` declaration says what a
value means, and the compiler picks how to store it. This chapter is for the
places where bytes matter: foreign APIs, device registers, files and wire
formats.

## Zeroed storage and establishment

All-zero bytes are safe storage for every type:

```omega
data Inventory {
    gold: u32;          // zero: 0 gold
    items: [Item; 8];   // zero: 8 zeroed items
    label: [u8; 64];    // zero: 64 zero bytes
}
```

A zeroed sum is in its first case. Whether zero is a *valid value* is a
separate question: for a type whose invariant excludes zero, zeroed storage
cannot be used until something constructs a real value in it
([Chapter 12](chapter_12_dependent_types.md#between-fields)). Zero never means
"empty" by itself. If callers need to know a value is empty, publish a domain
that says so:

```omega
data Command {
    case None;
    case Say(text: [u8; 256]);
}

public domain Command::Inert
    requires self in Command::None;

public machine empty_command() -> Command in Command::Inert {
    Command::None as +Command::Inert
}
```

## Default layout

```omega
data Player {
    health: i32;
    gold: u32;
}
```

With no layout chosen, the compiler stores `Player` however it likes. It may
reorder fields, fold a sum's tag into unused bit patterns, or remove storage
entirely, so long as the program behaves the same. Do not assume field order,
size or offsets.

## Stable representation

When the bytes must match something outside the program, say so where the
value is stored or passed:

```omega
data WinHandle {
    value: u64;
}

machine consume(handle: @CLayout<WinHandle>) {
    inspect_handle(handle);
}
```

`WinHandle` stays layout-neutral. `CLayout<T>` is a library type that stores a
`T` the way a C compiler would. Layout families are libraries: the compiler
knows nothing special about C or protobuf, and a package can define another
family the same way.

Converting between two representations of the same type is an explicit,
fallible `TryFrom`, never a silent cast.

## Size, alignment and addresses

```omega
const player_bytes: u64 = size_of::<Player>();
const player_alignment: u64 = align_of::<Player>();

machine header_is_aligned(packet: @[u8]) -> bool
requires packet.length >= 16
{
    align_of(packet) >= 16          // alignment of this actual location
}

let code_address: addr = address_of::<identity<u32>>();
let storage_address: addr = address_of(@value);
```

An address is a number. Holding one grants no right to read or write it.

## Slices at a foreign boundary

A slice is a pointer and a length internally, but that pair is not an ABI. A
foreign function that wants a pointer and a count, or a NUL-terminated string,
is declared with that shape, and a checked adapter converts.

## Calling conventions

Omega uses its own calling convention by default, including at a boundary.
If the other side expects a different convention, declare it on the trait.
For example, std declares its Linux x86-64 kernel policy on the byte-view
requirements, including `read`.

```omega
boundary trait PosixFilesystemLinuxX86ByteViews: Calling<LinuxX86ByteViewCalling> {
    machine read(fd: i32, buffer: @mutable [u8]) -> i64 reaches FilesystemContentRead crashes none;
}
```

```omega
// In std's build machine
Build::bind_native::<PosixFilesystemLinuxX86ByteViews::read>(builder, linux_x86_64,
    ForeignBinding::Syscall { number: 0 });
```

The policy fixes argument placement, register preservation and kernel entry.
Build supplies the syscall number. Remove `: Calling<LinuxX86ByteViewCalling>`
from this trait and the compiler must reject the binding because an ordinary
Omega call cannot enter the kernel. Other foreign ABIs work the same way.
For example, std's `WindowsX64ScalarCalling` policy sets
`plan.call.shadow_bytes = 32;` to tell the caller how much stack space
to reserve for the callee. Your package can use the same registers,
stack slots and entry controls to describe its own convention entirely in Omega.
See the [Windows x64 policy](../../source/library/std/targets/windows_x86_64/calling.omg)
and [calling plans](../spec/build/calling_plans.md#authorship-and-evaluation).

## Functions supplied through a table

Some foreign APIs, including firmware, give you a table of function addresses.
Call an ordinary boundary requirement; its package's Build connects that
requirement to an exact field of the table argument with `FunctionFieldBinding`.
`reflect\parameter` names the table formal and `reflect\field` names the field.
The table need not be the first argument.

The binding says where to find the function. The requirement's calling plan
says whether the table is also passed to it. A firmware allocation operation
may use the table only for lookup; an object protocol may pass it as a receiver.
A separate receiver is an ordinary explicit argument, not an implicit `self`.

The caller still needs a valid readable table and the admitted contract that
justifies calling the function. A readable address or matching record layout
does not grant that trust. See
[function-table bindings](../spec/build/foreign_bindings.md#function-table-bindings).

For example, a UEFI entry adapter can lend a memory manager the authority to
call Boot Services. If firmware allocates four pages, the manager can pass the
application a grant for those four pages while keeping the Boot Services borrow
scoped to the allocation work. The application can use the pages without
acquiring permission to call firmware. [UEFI entry authority](../spec/build/uefi_entry.md#firmware-api-authority-and-storage-grants)

## Access policies and externally mutable memory

A device register can change underneath you, and reading or writing it has
side effects. Such memory is used through a view that allows only the
operations the hardware supports:

```omega
machine send(uart: @mutable (UartRegisters in DeviceRegisters), byte: u32) -> u32 {
    write(@mutable uart.transmit, byte);
    read(@uart.status)
}
```

Each `read` and `write` is exactly one device access. Reading `transmit` or
writing `status` is rejected, and so is borrowing either as ordinary memory.
The platform package supplies the view; application code uses it.

| Memory is | Meaning |
| --- | --- |
| Stable | Ordinary RAM. Unchanged while borrowed. |
| External | A device. Every access is an event performed once. |
| Atomic | Shared between threads through atomic operations. |

Provider authors describe a device with a layout (where fields are), an access
plan (who may do what) and a resource profile (what the backing supports).
See [access plans](../spec/resources/access.md) and
[device protocols](../spec/resources/device_access.md).

## Recast views

A recast borrows the same storage as another type:

```omega
let read: @u32 = @storage.word as @u32;
let header: @mutable Header = @mutable storage.bytes[offset] as @mutable Header;
let words: @mutable [u16] = @mutable storage.bytes as @mutable [u16];
```

The compiler checks size, alignment and that every value you could read is
valid for the target type. A mutable recast must also leave the storage valid
for its original type: viewing a `bool` as a writable byte would let you store
`2`, so that needs proof that you put a 0 or 1 back. See
[recasts](../spec/layouts/recasts.md), which also covers initializing a typed
header inside raw bytes.

## Relationship to serialized bytes

In-memory layout and serialized format are separate. A type has one memory
layout per target and may have several encodings. Members are numbered so
that formats survive renaming:

```omega
data CounterDiskV1 {
    #1 counter: i32;
}

data CounterDiskV2 {
    #1 counter: i64;
    #2 timestamp_seconds: u64;
}

data CounterDisk {
}

machine counter_v1_to_v2(
    old: CounterDiskV1,
    out: @mutable CounterDiskV2
) satisfies FormatMigration<
    CounterDisk,
    CounterDiskV1,
    CounterDiskV2
>::migrate {
    out.counter = old.counter as i64;
    out.timestamp_seconds = 0;
}
```

A published format never changes. A new version is a new type plus a migration
machine, and the value given to a new field is something you write, not a
default the format supplies. `retired #N;` reserves the number of a removed
member. Decoding bytes establishes only what the decoder checked; it grants no
authority. See [codecs](../spec/layouts/codecs.md).

## Where next

[Chapter 21](chapter_21_reflection.md) covers reflection, which layout
policies are built on. The complete rules are in
[layout plans](../spec/layouts/plans.md); how growable collections own their
elements is in [dense owned runs](../spec/resources/typed_runs.md).
