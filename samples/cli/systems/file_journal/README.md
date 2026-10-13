# file_journal

Waiting on [OWNER_QUESTIONS.md Q1](../../../../OWNER_QUESTIONS.md#q1---what-carries-a-portable-services-per-family-raw-binding)
(`portable-service-per-family-binding-carrier`): the portable `Filesystem`
has no way to carry a per-target raw binding after `FilesystemHost` splits
into its POSIX and Windows families.

Current failure: package review rejects the entry with "selected ProgramEntry
Binding field `Main::fs` requires a selected Fused provider for boundary
`FilesystemHost`". Once Q1 is answered, the remaining work is the ordinary
FILESYSTEM-PLATFORM-SERVICES implementation in `TASKS.md`.
