# file_journal

The settled
[provider-closure transport](../../../../wiki/spec/build/platform_services.md#provider-closure-transport)
keeps one portable `Filesystem` binding while its selected target provider
retains the exact POSIX or Windows raw-family closure.

Acceptance: `omega run samples/cli/systems/file_journal/main.omg` exits 7
after creating, writing, inspecting, reading, renaming and removing the file
and its directory. macOS ARM64 and both Linux targets select the POSIX portable
provider; matching-host execution remains separate evidence.

Windows selects the portable provider over its retained raw-family closure.
On windows_x86_64, `omega run` exits 7. macOS and Linux execution remain
separate evidence under **FILESYSTEM-PLATFORM-SERVICES**.
Terminal execution still declines the selected foreign bindings until trusted
host adapters supply them.

The authored `/tmp/omega_journal_dir` is rooted, not relative to the working
directory. A Windows runner must explicitly provision a writable `\tmp` on
its current drive. The library must not silently reinterpret that path.
