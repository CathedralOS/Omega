# note_vault

Current failure: package review rejects the entry with "selected ProgramEntry
Binding field `Main::fs::host` requires a selected Fused provider for boundary
`FilesystemHost`". The portable `Filesystem` still holds the mixed
`Binding<FilesystemHost>`, which no target's provider covers.

Next: **FILESYSTEM-PLATFORM-SERVICES** in `TASKS.md`. Per
[platform services](../../../../wiki/spec/build/platform_services.md#three-layers),
the application holds a binding to the portable service only; each target's
implementation of it reaches that target's raw family
(`PosixFilesystemHost` or `WindowsFilesystemHost`).
