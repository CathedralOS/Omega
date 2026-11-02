# Windows canonicalization — 2026-11-02

[Matching-host run 41433859703](https://github.com/CathedralOS/Omega/actions/runs/41433859703)
qualified `tests/omega/pass/filesystem/windows_canonicalize_exit` at
`6d32fc3ea8b6e86d3f0adfcbb5666970aaf51de2` on Windows x86-64.
The optimized CLI SHA-256 is
`692e6c8d3955084a8f4349cedfdeefc782832e8c5ebd30f07a49d4e51d933c71`
(Rust nightly 2026-09-04, opt level 1).

Native, Terminal and wrapper each returned the unchanged expected exit 70.
The original file and directory path-prefix checks, missing-path refusal,
NotFound classification and cleanup remain intact. Nine recorded nested
blocking calls now carry their required acknowledgement. Both engines use
the authored program; the runtime supplies the exact Windows query rather
than manufacturing a canonical path.

The baseline [41427987430](https://github.com/CathedralOS/Omega/actions/runs/41427987430)
at `45aa5f12e8e2ec897182418c50713b80b5456f6f` returned native 70, while
Terminal declined `final_path`: toolchain `FilesystemMetadataQuery` needs
its owner's modeled supply. The native executable and Terminal artifact
are byte-for-byte identical before and after the adapter repair:

- Native executable: `5fe15d874dca8da6e0f07dc29c03961d5cf8a052708e0bd661520cfd75f81116`.
- Terminal artifact: `0ae72813f6e644f154368d779d58fd21dcaeb275cc477b0addbb33999ccc9f08`.

The adapter validates the exact DLL/export, complete semantic signature and
selected calling/storage plans. Its existing instance-owned guest HANDLE
supplies the queried object. The full mutable 4096-element UTF-16 array,
capacity 4096 and flags 0 are required before effects; the filesystem namespace
still authorizes metadata access. Native return counts and last-error state
remain intact. Source performs the strict UTF-8 conversion and caller-buffer
checks. No service delegation, query result or pathname is fabricated.
The ABI and count convention follow the
[Windows API contract](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfinalpathnamebyhandlew).

Normal update returned ReviewRequired after 360.375 seconds; 288 individual
pending rows were accepted, and strict resume completed in 168.797 seconds.
`run --both --keep` completed in 156.078 seconds. All 179 tracked fixture/library
inputs were guarded before and after the phases. Only this original program
was selected; completed ABI, metadata and breadth witnesses were not rerun.

The baseline optimized compiler took 1723.672 seconds under a new dependency
cache epoch (`16ff27…`); the same-branch candidate reused its cache and took
155.390 seconds. Keeping a branch preserves cache locality, while changed
Cargo/toolchain key inputs still require a new epoch. Phase timings are actual
observations, not a whole-compiler performance guarantee.

Raw reports and original optimized producer/profile receipts are retained under
`/Users/zcanann/Omega-build/artifact-read/artifact-13134674651/` and
`artifact-13136715877/`. CRC/SHA-verified products, raw baseline job log and exact
source bundle are retained outside scratch under
`/Users/zcanann/Omega-build/filesystem-windows-canonicalization-20261102/`.
The source ref is
`refs/corpus-observations/6d32fc3ea8b6e86d3f0adfcbb5666970aaf51de2`.
Portable filesystem programs and occurrence-bound release proofs remain open.
