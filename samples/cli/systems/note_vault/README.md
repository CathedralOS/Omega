# note_vault

The settled
[provider-closure transport](../../../../wiki/spec/build/platform_services.md#provider-closure-transport)
keeps one portable `Filesystem` binding while its selected target provider
retains the exact POSIX or Windows raw-family closure.

Acceptance: `omega run samples/cli/systems/note_vault/main.omg` exits 14
after the complete filesystem/time workflow.

The POSIX portable provider supplies macOS ARM64 and both Linux targets.
Windows selects its portable provider over the retained Windows raw-family
closure and cross-produces an image. Its exit-14 filesystem/time oracle still
requires native Windows execution under **FILESYSTEM-PLATFORM-SERVICES**;
there is no Windows runner on the current macOS host.
