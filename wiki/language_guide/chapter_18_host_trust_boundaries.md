# Chapter 18: Host Libraries And Trust Boundaries

Omega should model the trusted root explicitly.

The outside world is not one thing. Linux may expose raw syscall numbers,
Darwin normally routes process IO through `libSystem`, Windows imports APIs
from DLLs such as `Kernel32.dll`, Wasm imports host functions, and embedded
targets may jump through firmware tables. The shared concept is not "Unix
syscall." The shared concept is an imported boundary whose implementation is
not Omega code.

## Boundary Traits

A boundary trait names callable behavior whose implementation crosses out of
proved Omega code. It is still a trait: callers see machine signatures,
requirements, guarantees, and effects. What makes it a boundary is that the
implementation is accepted through a host package, target binding, firmware
surface, dynamic loader, or other trusted edge.

```omega
boundary trait WindowsFile {
    machine read(
        handle: HANDLE<[read]>,
        buffer: &mut [u8, [writable, initialized]],
        bytes_to_read: usize,
        bytes_read: &mut usize
    ) -> bool
    effects
        filesystem_io;
}
```

Working interpretation:

- `boundary trait` means the machines describe behavior outside proved Omega
  code.
- Each `machine` is a callable boundary surface.
- `requires` clauses are obligations the caller must prove before crossing the
  boundary.
- `ensures` clauses are guarantees accepted from the boundary implementation.
- `effects` clauses are auditable capabilities such as filesystem, process,
  stdin, stdout, network, thread, clock, or device access.
- Build policy decides which boundary providers are allowed for a target.

Console capability should use the same shape:

```omega
boundary trait Console {
    machine write(text: String)
    effects
        stdout_io;

    machine write_line(text: String)
    effects
        stdout_io;

    machine read_line(out: &mut ConsoleLine)
    effects
        stdin_io;

    machine exit_process(code: i32)
    effects
        process_exit;
}
```

Domain requirements stay normal proof language. A filesystem boundary should
not invent special "initialized" words when a domain is what it means:

```omega
domain NonEmpty for String {
    length > 0
}

boundary trait Filesystem {
    machine open(path: String)
    requires
        path in String::NonEmpty
    effects
        filesystem_io;
}
```

Target metadata such as library artifact, symbol, syscall number, calling
convention, and trust root belongs in toolchain host packages or explicitly
whitelisted boundary providers. Ordinary application libraries should not be
able to self-declare new host boundaries silently. Pulling in a boundary with
`filesystem_io`, `stdout_io`, or `process_exit` is visible to the build, and a
restricted build can reject it.

The compiler should understand boundary traits, provider packages, libraries,
symbols, calling conventions, trust roots, and target image imports
generically. It should not special-case every Windows, Darwin, Linux, or SDK
API.

## Host Providers

Some targets do not need a named user-mode library for the lowest boundary.
Linux can expose a target syscall surface directly. That mapping is provider
metadata for a boundary trait, not a different user-facing callable concept.

```omega
host linux_aarch64 provides Console {
    write_line -> syscall 64;
    write -> syscall 64;
    read_line -> syscall 63;
    exit_process -> syscall 94;
}
```

This is the same proof shape as a library import:

- Omega proves caller-side type and state invariants.
- The imported boundary is trusted to satisfy its declared guarantees.
- The build artifact records which trust roots were used.

The exact provider syntax is provisional. The important design point is that
raw syscall tables, imported DLL functions, firmware jumps, and loader hooks
are provider details for boundary traits, not normal Omega machines.

## Invariant Parameters

Imported signatures should lean on invariant-parameterized types rather than
duplicating normal type facts in ad-hoc `requires` clauses.

```omega
&[u8, [non_empty, initialized]]
&mut [u8, [writable, initialized]]
HANDLE<[process, vm_read]>
RemoteBuffer<const u8, [readable_by<process>]>
```

The invariant names are resolved in the namespace of the type being
instantiated. `&[u8, [initialized]]` and `HANDLE<[initialized]>` do not
have to mean the same thing.

A type can define which invariant parameters it accepts:

```omega
builtin slice &[T, I]
    where I subset {non_empty, initialized}
    exposes len: usize
    invariant non_empty = len > 0
    invariant initialized = elements.initialized

builtin slice &mut [T, I]
    where I subset {non_empty, initialized, writable}
    exposes len: usize
    invariant non_empty = len > 0
    invariant initialized = elements.initialized
    invariant writable = elements.writable
```

The exact declaration syntax is provisional. The important point is that
`&[T]` and `&mut [T]` are built-in borrowed slice-view types, not user-defined
machines. They expose proof-visible facts such as `len`, and they define the
invariant names that callers may attach to that slice view.

This sketch needs more design work, but the direction is important:

- Callers name exported invariants.
- Invariant names are scoped to the type that defines them.
- The type implementation maps public invariant names to private facts.
- Callers should not need to know private field layout to state proof
  requirements.
- Safe Omega source does not expose raw pointer fields for ordinary slices or
  vectors. Address-level representation belongs to compiler/runtime lowering
  and explicit trusted boundary modeling, not the normal surface language.

Short forms such as `&[T]` and `&mut [T]` mean the same slice views with no
extra invariant parameters.

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

## Blocking Boundaries

Imported entries that can block must say what can unblock them, or they must be
reported as trusted opaque waits.

Examples:

- A pipe read may block until a matching write, close, timeout, or external
  event.
- A process wait may block until the target process exits.
- A socket receive may block on external network input.
- A driver call may block on hardware interrupt, timeout, cancellation, or a
  trusted opaque device contract.

The proof/invariant checker can reason about modeled waits. It can audit
trusted opaque waits. A proved-concurrency build may reject opaque blocking
boundaries.

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
