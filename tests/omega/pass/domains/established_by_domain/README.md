# established_by_domain

`pub domain Folder::Writable established by Desktop::choose_folder,
DesktopProvider::choose_folder;` — a domain whose establishment is
granted by named machines; the capability-typed return hands the minted
`Folder` up the call graph (`Backup::stage acquires via Vault::pick`),
and `reaches Desktop` rides each hop.

Compile-checked only: `build.omg` declares no `Build::depend` row, so the root package
loads with no dependency closure and the program names only compiler-known declarations;
`builder.freestanding = true` selects the freestanding target profile.
