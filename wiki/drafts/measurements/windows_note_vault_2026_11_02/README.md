# Portable note vault on Windows — 2026-11-02

[Matching-host run 41484097742](https://github.com/CathedralOS/Omega/actions/runs/41484097742)
qualified the original `samples/cli/systems/note_vault` at
`a93031a2b7f8b0bf0fbcea94edc05858f8151da0` on Windows x86-64.
The optimized CLI SHA-256 is
`131eb464ad7c0cd768db781df7c293f66a57c806cd8cc23ab05f1e4ec6236f3b`
(Rust nightly 2026-09-04, opt level 1).

Native, Terminal and wrapper each returned the authored exit 14. The program
created nested directories, exclusively created a file, appended notes, checked
metadata/time and readback, truncated and copied content, counted directory
entries, removed a file and recursively removed the tree. All 180 tracked
project/library inputs were guarded before and after phases. Source, its relative
Std dependency and all original checks remain unchanged.

The compiler had refused the fixed `NtQueryDirectoryFile` capacity argument:
an admitted exact integer rematerialization appeared between its outgoing
address and store. Custody now rejoins that literal, type, source identity,
fresh result and complete contiguous emitted interval. Different values,
effectful/charged instructions, wrong origins or a different stored register do
not earn that interval. The ordinary native artifact rejoin uses the same query
after independent selected/physical replay.

Terminal supplies the exact NT directory classes and complete selected records;
basic/end-of-file/disposition file-information classes; and synchronous relative
opens. It retains read/write/delete sharing, no-follow flags, null optional
callbacks, actual NT status, errors and returned byte counts. Complete typed
reference contracts and actual pointer equality join object attributes to the
captured Unicode name and WCHAR array. A private fixed ABI header translates
only the owned guest root HANDLE. The readonly Source header and both original
pointees remain unchanged; this creates no Source write or reference permission.
Successful output handles enter the existing guest namespace before completion.
Final alignment padding cannot stand in for semantic record fields.

These adapters follow the documented [NT query](https://learn.microsoft.com/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntquerydirectoryfile),
[relative open](https://learn.microsoft.com/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntopenfile),
[object attributes](https://learn.microsoft.com/windows/win32/api/ntdef/ns-ntdef-_object_attributes),
[Unicode string](https://learn.microsoft.com/windows/win32/api/ntdef/ns-ntdef-_unicode_string)
and [file-information update](https://learn.microsoft.com/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle)
contracts, admitted against each selected target calling/storage plan.

Normal update returned ReviewRequired in 389.640 seconds; 294 individual
decisions were accepted, and strict resume completed in 190.734 seconds.
`run --both --keep` completed in 168.235 seconds. Compiler preparation took
161.000 seconds. No corpus, completed application or additional producer was
attached after this acceptance.

Native executable SHA-256:
`ff542e873679d809b75f34db8481b0dcbc321514e3a21ce83fd3788821bd1436`.
Terminal artifact SHA-256:
`8cedbe099d32ba4262bd70ee88601b572bc1ecf46a6deff3f2d3a57ac2ecc531`.
Both are identical to the preceding native-only `8477a472d1e8` products;
the final receiving runtime observed Terminal exit 14 on that same artifact.
Raw reports/profiles and CRC-verified products are retained outside scratch in
`/Users/zcanann/Omega-build/filesystem-windows-vault-20261102/` and
`/Users/zcanann/Omega-build/artifact-read/artifact-13163240437/`.
The producer source bundle/ref is
`refs/corpus-observations/a93031a2b7f8b0bf0fbcea94edc05858f8151da0`.

The iteration also exposed two avoidable costs. Named inspection and native
products have different numbering; native refusal diagnostics must retain their
own compact slot/instruction/origin coordinates within the message budget.
Routine unrelated main merges invalidated qualified compiler inputs despite a
successful same-epoch cache restore. Fetch and verify publication read-only,
then retain the warm branch until an actual conflict or prerequisite requires
integration, as recorded in `tools/docs/testing.md`.

Remaining platform controls and occurrence-specific ordinary-release proofs
stay open under **FILESYSTEM-PLATFORM-SERVICES**.
