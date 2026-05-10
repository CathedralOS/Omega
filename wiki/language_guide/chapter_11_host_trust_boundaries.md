# Chapter 11: Host Libraries And Trust Boundaries

Omega should not pretend the trusted root does not exist.

The outside world is not one thing. Linux may expose raw syscall numbers,
Darwin normally routes process IO through `libSystem`, Windows imports APIs
from DLLs such as `Kernel32.dll`, Wasm imports host functions, and embedded
targets may jump through firmware tables. The shared concept is not "Unix
syscall." The shared concept is an imported boundary whose implementation is
not Omega code.

## Imported Libraries

A dynamic or platform library declaration names the binary interface directly.

```omega
library Kernel32 = "Kernel32.dll" calling_convention winapi {
    fn ReadFile(
        handle: HANDLE<[read]>,
        buffer: &mut [u8, [writable, initialized]],
        bytes_to_read: usize,
        bytes_read: &mut usize
    ) -> bool
        trust omega_windows_kernel32_read_file
}
```

Working interpretation:

- `library` means the implementation lives outside Omega code.
- The optional name, `Kernel32`, is the Omega namespace for references and
  reports.
- The string, `"Kernel32.dll"`, is the platform library artifact.
- `calling_convention winapi` is the default calling convention for functions
  in the block.
- `fn` is used because imported library entries are stack calls, not
  state-machine transitions.
- `trust` is per function and names the trusted contract root that allows
  Omega to use the imported function's guarantees.

Anonymous library declarations are allowed when no namespace is needed:

```omega
library "libSystem.B.dylib" calling_convention c {
    fn write(fd: i32, buffer: &[u8, [initialized]], count: usize) -> SyscallResult<usize>
        symbol "_write"
        trust omega_darwin_libsystem_write
}
```

Function-level metadata can override library defaults where the platform needs
it:

```omega
library User32 = "User32.dll" calling_convention winapi {
    fn MessageBoxW(...) -> i32
        symbol "MessageBoxW"
        trust omega_windows_user32_message_box
}
```

The compiler should understand libraries, symbols, calling conventions, trust
roots, and target image imports generically. It should not special-case every
Windows, Darwin, Linux, or SDK API.

## Syscall Tables

Some targets do not need a named user-mode library for the lowest boundary.
Linux can expose a target syscall surface directly.

```omega
syscalls linux_aarch64 {
    fn write(fd: i32, buffer: &[u8, [initialized]], count: usize) -> SyscallResult<usize>
        number 64
        trust omega_linux_write
}
```

This is the same proof shape as a library import:

- Omega proves caller-side type and state invariants.
- The imported boundary is trusted to satisfy its declared guarantees.
- The build artifact records which trust roots were used.

The spelling of `syscalls` is still less settled than `library`; the important
design point is that raw syscall tables are target surfaces, not normal Omega
states.

## Invariant Parameters

Imported signatures should lean on invariant-parameterized types rather than
duplicating normal type facts in ad-hoc `requires` clauses.

```omega
&[u8, [non_empty, initialized]]
&mut [u8, [writable, initialized]]
HANDLE<[process, vm_read]>
RemotePtr<const u8, [readable_by<process>]>
```

The invariant names are resolved in the namespace of the type being
instantiated. `&[u8, [initialized]]` and `HANDLE<[initialized]>` do not
have to mean the same thing.

A type can define which invariant parameters it accepts:

```omega
machine Slice<T, I>
    where I subset {non_empty, initialized, writable}
{
    ptr: Ptr<T>;
    len: usize;

    invariant non_empty = len > 0;
    invariant initialized = ptr.initialized<len>;
    invariant writable = ptr.writable<len>;
}
```

Surface syntax such as `&[T]` and `&mut [T]` is shorthand for borrowed
proof-bearing slice views. The underlying core type can still be modeled as
`Slice<T, I>`; users should not need to spell that internal form for ordinary
borrowed contiguous data.

This sketch needs more design work, but the direction is important:

- Callers name exported invariants.
- Invariant names are scoped to the type that defines them.
- The type implementation maps public invariant names to private facts.
- Callers should not need to know private field layout to state proof
  requirements.

Short forms can come from overloads or aliases rather than magical defaults:

```omega
machine Slice<T> = Slice<T, []>;
machine InitializedSlice<T> = Slice<T, [initialized]>;
```

## Trust

Trust is the authority for accepting a guarantee that Omega cannot prove from
Omega code.

For an imported function:

- Omega proves the caller satisfies the parameter and state invariants.
- The imported implementation is trusted to satisfy the declared guarantees.
- Omega may use those trusted guarantees as facts after the call.

In proof vocabulary:

- Input refinements and invariant parameters are caller requirements.
- Return refinements and trusted clauses are callee guarantees.
- The call creates obligations for the caller.
- `trust` explains why unproved imported guarantees are accepted.

For ordinary Omega code, the compiler should know the contracts for
assignments, arithmetic, borrows, transitions, calls, and field access. For
imported libraries and syscall surfaces, the contract must be declared or
imported from an audited package.

## Host vs Standard Library

The standard library is the portable API most application code should use. It
can provide `Console.read_line`, formatting, strings, slices, data structures,
and higher-level process or filesystem helpers.

Host packages are the audited bottom edge. They contain imported libraries,
syscall surfaces, startup bindings, and trust roots.

Most users should not author raw Windows, Darwin, Linux, firmware, or console
SDK contracts for ordinary applications. They should import toolchain-provided
surfaces:

```omega
use omega::std::console;
use omega::host::windows::kernel32;
```

Advanced users can author libraries for custom OSes, firmware, game consoles,
or unusual hardware. Doing so explicitly expands the trusted computing base.

## Build Artifacts

Compiler artifacts should list imported libraries, syscall surfaces, trusted
functions, and unchecked policies.

Example shape:

```text
imported libraries:
  Kernel32 -> Kernel32.dll calling_convention winapi
  DarwinLibSystem -> libSystem.B.dylib calling_convention c

trusted imported functions:
  Kernel32.ReadFile trust omega_windows_kernel32_read_file
  DarwinLibSystem.write trust omega_darwin_libsystem_write

target image imports:
  Kernel32.dll!ReadFile
  libSystem.B.dylib!_write
```

A build with proofs or contracts disabled should be stamped loudly rather than
silently behaving like a normal safe build.

The goal is not to eliminate trust. The goal is to make trust explicit, scoped,
and auditable.
