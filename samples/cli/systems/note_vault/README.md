# note_vault

The settled
[provider-closure transport](../../../../wiki/spec/build/platform_services.md#provider-closure-transport)
keeps one portable `Filesystem` binding while its selected target provider
retains the exact POSIX or Windows raw-family closure.

Acceptance: `omega run samples/cli/systems/note_vault/main.omg` exits 14
after the complete filesystem/time workflow.

The POSIX portable provider supplies macOS ARM64 and Linux x86-64. Other
authored targets still need their portable provider under
**FILESYSTEM-PLATFORM-SERVICES**; raw-family support alone does not supply it.
