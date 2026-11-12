# per_target_satisfies

Per-target provider satisfaction: the same `Console` trait rows (`write_byte` `reaches Console terminates`, `exit_process`) are satisfied by `boundary machine`s scoped `macos_arm64`, `linux_x86_64`, and `windows_x86_64`. Windows resolves its byte write through a `ForeignBinding::DllImport { DllImport::PeByName { msvcrt.dll, _write } }` binding produced by a `windows_x86_64 machine`, with `WindowsConsoleImports::write` satisfying the `WindowsConsoleIo::write` boundary-trait requirement `via` that binding.

## Learnings

- `<target> boundary machine Provider::m satisfies Trait::m` scopes the provider row to one target.
- `via <binding>` cannot supply a machine by itself: the requirement row must be named first as `satisfies Trait::machine via Binding(...)`.
- `satisfies` must point at a DIFFERENT requirement owner — a machine `X::write` cannot satisfy a requirement it owns itself; the imports-owner pattern (`WindowsConsoleImports::write` satisfies `WindowsConsoleIo::write`) is the canonical shape.
- `exit_process` on Windows binds `kernel32.dll` `ExitProcess` directly `via` the import machine, no body needed.

Compile-checked only: run a check target, not native.
