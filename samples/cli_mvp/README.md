# CLI MVP Sample

Smallest console-style Omega sketch: print one line, then exit.

This sample intentionally avoids input so the entry and platform boundary are easy to inspect.

## Build Output

The compiler writes phase artifacts and executable output to local `build/`.
That directory is ignored by this sample on purpose so the project can be copied
without bringing stale compiler output with it.

## Boundary Root Sketch

`build.omg` references compiler-provided `omega::host` packages from each `target` item. The host package bodies are still ahead of full validation, but the compiler now records their structure and emits a boundary report so we can design the boundary in Omega source instead of inventing a sidecar config format.

For cross-platform hello world, the boundary base is tiny:

- `Stdout.write_line`: host claims it can write initialized UTF-8 text to process stdout and report `IOError`.
- `Process.exit`: host claims it can terminate the process with a target-specific observable exit code.

Omega should prove the literal is initialized/UTF-8 and that errors are handled once `Result` exists. Omega should accept the OS wrapper contract only because `build.omg` explicitly names the host boundary.

## Standard Library vs Host Bindings

The standard library should be ordinary Omega code wherever possible: strings, slices, math, collections, parsing helpers, portable console helpers, and so on.

The host bindings are different. Files under the toolchain-provided `omega::host` package sketch the boundary provider that adapts those portable capabilities to a target ABI. Each platform target is folder-backed and split by domain, for example `omega::host::targets::windows` loads `targets/windows/mod.omg`, then pulls in `kernel32`, `stdout`, `process`, and local platform types.

- Windows uses documented Win32 imports like `Kernel32.dll!WriteFile` and `ExitProcess`.
- Linux can plausibly use raw syscalls for `write` and `exit_group`.
- Darwin/macOS should usually bind through `libSystem`.

So the goal is not "no DLLs exist" on Windows. The goal is no Omega runtime DLL, no C runtime dependency, and a tiny audited set of OS imports.
