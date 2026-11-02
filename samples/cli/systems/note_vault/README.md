# note_vault

The settled
[provider-closure transport](../../../../wiki/spec/build/platform_services.md#provider-closure-transport)
keeps one portable `Filesystem` binding while its selected target provider
retains the exact POSIX or Windows raw-family closure.

Acceptance: `omega run samples/cli/systems/note_vault/main.omg` exits 14
after the complete filesystem/time workflow.

The POSIX portable provider supplies macOS ARM64 and both Linux targets.
Windows selects its portable provider over the retained Windows raw-family
closure. [Windows matching-host evidence](../../../../wiki/drafts/measurements/windows_note_vault_2026_11_02/README.md)
qualifies the unchanged application at native exit 14 and Terminal exit 14,
including ordinary package update/review/resume. Native executable and Terminal
artifact identities remained unchanged while the receiving adapters were repaired.
Remaining Linux and filesystem controls stay under **FILESYSTEM-PLATFORM-SERVICES**.

On macOS ARM64, the CLI explicitly supplies the host filesystem namespace and
live host clock to Terminal execution. `omega run --both` must observe exit 14
from both engines, including readback, bounded copying, directory enumeration
and recursive removal. The clock reads the executing host; an embedding may
instead explicitly supply a private snapshot. Filesystem supply is unprovided
by default in other embeddings. This witness does not close the parent task's
remaining platform acceptance or its occurrence-bound release proofs.
