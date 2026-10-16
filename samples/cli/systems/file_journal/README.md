# file_journal

The settled
[provider-closure transport](../../../../wiki/spec/build/platform_services.md#provider-closure-transport)
keeps one portable `Filesystem` binding while its selected target provider
retains the exact POSIX or Windows raw-family closure.

Acceptance: `omega run samples/cli/systems/file_journal/main.omg` exits 7
after creating, writing, inspecting, reading, renaming and removing the file
and its directory. macOS ARM64 and Linux x86-64 have the portable provider;
the other authored targets remain under **FILESYSTEM-PLATFORM-SERVICES**.
