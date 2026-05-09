# Chapter 11: Foreign Boundaries And Trust

Omega should not pretend the trusted root does not exist.

The core concept is not "host syscall." That framing is too Unix-shaped. Different targets cross the boundary in different ways:

- Linux may call a raw syscall table.
- Darwin usually reaches OS services through `libSystem`.
- Windows normally imports user-mode APIs from system DLLs such as `Kernel32.dll` or `KernelBase.dll`.
- Wasm may call imported host functions.
- Embedded targets may call firmware vectors.
- Console or custom platforms may call SDK-provided entry points.

The shared language concept is a foreign boundary.

## Foreign Machines

A foreign machine declares an external contract surface. Its implementation is not Omega code.

```omega
foreign machine windows::memoryapi {
    state ReadProcessMemory(
        process: HANDLE<[process, vm_read]>,
        remote: RemotePtr<const u8, [readable_by<process>]>,
        out: Slice<u8, [writable, initialized]>
    ) -> Result<usize[range<0, out.length>], IOError>
        trust result.Ok(n) => n <= out.length
}
```

Working interpretation:

- `foreign machine` means the implementation is outside Omega.
- `windows::memoryapi` is a namespace path. Omega should use `::` for namespaces and reserve `.` for member/state access.
- Parameter types carry caller obligations through invariant parameters.
- Return types carry as much of the postcondition as possible.
- `trust` states a guarantee accepted from the foreign implementation.

This avoids duplicating normal type facts with `requires` clauses. If a parameter must be non-empty, writable, initialized, aligned, or rights-bearing, that should usually be expressed in the parameter type.

## Invariant Parameters

Foreign signatures should lean on invariant-parameterized types.

```omega
Slice<u8, [non_empty, initialized]>
HANDLE<[process, vm_read]>
RemotePtr<const u8, [readable_by<process>]>
```

The invariant names are resolved in the namespace of the type or machine being instantiated. `Slice<u8, [initialized]>` and `HANDLE<[initialized]>` do not have to mean the same thing.

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

This sketch needs design work, but the direction is important:

- Callers name exported invariants.
- Invariant names are scoped to the type that defines them.
- The type implementation maps public invariant names to private facts.
- Callers should not need to know the private field layout to state proof requirements.

Short forms can be provided by overloads or aliases rather than magical defaults:

```omega
machine Slice<T> = Slice<T, []>;
machine InitializedSlice<T> = Slice<T, [initialized]>;
```

## Trust

Trust is the authority for accepting a guarantee that Omega cannot prove from Omega code.

For a foreign call:

- Omega proves the caller satisfies the parameter and state invariants.
- The foreign implementation is trusted to satisfy the declared `trust` guarantees.
- Omega may use those trusted guarantees as facts after the call.

In proof vocabulary:

- Input refinements and invariant parameters are caller requirements.
- Return refinements and trusted clauses are callee guarantees.
- The call creates obligations for the caller.
- `trust` explains why unproved foreign guarantees are accepted.

For ordinary Omega code, the compiler should know the contracts for assignments, arithmetic, borrows, transitions, and field access. For foreign machines, the contract must be declared or imported from an audited package.

## Target Bindings

A foreign machine is not the same thing as its target binding.

The foreign machine describes the API and proof contract. The target binding says how that API maps to a platform mechanism.

Sketch:

```omega
target windows_x64 {
    bind windows::memoryapi::ReadProcessMemory
        to dll "Kernel32.dll" symbol "ReadProcessMemory"

    trust omega::windows_contracts
}
```

Linux may bind the same kind of foreign surface to syscall numbers:

```omega
target linux_x64 {
    bind linux::syscalls::write
        to syscall 1

    trust omega::linux_contracts
}
```

Darwin may bind through `libSystem`:

```omega
target macos_arm64 {
    bind darwin::libsystem::write
        to dylib "libSystem.B.dylib" symbol "_write"

    trust omega::darwin_contracts
}
```

The compiler should not special-case every OS API. It should generically understand foreign machines, target bindings, calling conventions, imported symbols, syscalls, and trust policies.

## Standard Library vs Foreign Bindings

The standard library should mostly be portable Omega code. It can provide data structures, algorithms, string helpers, slices, formatting, numeric helpers, and high-level APIs. Those pieces should be proven like any other Omega code.

Foreign bindings are the bottom edge where Omega touches the outside world. They are target-specific and trusted.

Most users should not author raw Windows, Linux, Darwin, or console SDK contracts for ordinary applications. They should import audited toolchain-provided surfaces:

```omega
use omega::foreign::windows::memoryapi;
use omega::std::process;
```

Advanced users can author foreign machines for custom OSes, firmware, game consoles, or unusual hardware, but doing so explicitly expands the trusted computing base.

## Build Artifacts

Compiler artifacts should list trusted foreign machines, target bindings, and unchecked policies.

Example shape:

```text
trusted foreign machines:
  omega::foreign::windows::memoryapi
  omega::foreign::windows::processthreadsapi

target bindings:
  windows::memoryapi::ReadProcessMemory -> Kernel32.dll!ReadProcessMemory

trusted guarantees used:
  ReadProcessMemory result.Ok(n) => n <= out.length
```

A build with proofs or contracts disabled should be stamped loudly rather than silently behaving like a normal safe build.

The goal is not to eliminate trust. The goal is to make trust explicit, scoped, and auditable.
