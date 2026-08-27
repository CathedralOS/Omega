# CLI MVP Sample

Smallest console-style Omega sketch: print one line, then exit.

This sample intentionally avoids input so the entry and platform boundary are easy to inspect.

## Build Output

The compiler writes phase artifacts and executable output to local `build/`.
That directory is ignored by this sample on purpose so the project can be copied
without bringing stale compiler output with it.

## Boundary Providers

The sample imports the portable Console requirement. Ordinary targets select
their standard provider defaults, so `build.omg` does not enumerate every host
leaf. The settled build shape still binds the target's program-entry slot to the
exact source machine; an application adds provider bindings only when it
intentionally substitutes a default. This sample's source and transitional
target-only build file still exercise temporary entry discovery pending the
corpus migration tracked by `ENTRY-CONTENT-ROOTS`; that discovery is not
supported language behavior.

For cross-platform hello world, the boundary base is tiny:

- `Stdout.write_line`: host claims it can write initialized UTF-8 text to process stdout and report `IOError`.
- `Process.exit`: host claims it can terminate the process with a target-specific observable exit code.

Omega proves the literal satisfies the borrowed byte contract. The target's
selected provider is accepted through the ordinary provider-plan admission
pipeline and remains visible in the boundary report.

## Standard Library vs Host Bindings

The standard library is ordinary Omega code wherever possible: byte/text
domains, slices, math, collections, parsing helpers, and portable console
adapters. Target provider packages under `source/library/std/targets/` adapt
those requirements to the selected ABI; they are ordinary source inputs to a
derived, validated, admitted provider plan rather than floating compiler magic.

- Windows uses documented Win32 imports like `Kernel32.dll!WriteFile` and `ExitProcess`.
- Linux can plausibly use raw syscalls for `write` and `exit_group`.
- Darwin/macOS should usually bind through `libSystem`.

So the goal is not "no DLLs exist" on Windows. The goal is no Omega runtime DLL, no C runtime dependency, and a tiny audited set of OS imports.
