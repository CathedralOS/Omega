# Windows raw filesystem breadth — 2026-11-02

[Matching-host run 41421092107](https://github.com/CathedralOS/Omega/actions/runs/41421092107)
qualified the original `tests/omega/pass/filesystem/windows_raw_breadth_exit`
program on Windows x86-64 at
`cdc089ff12b185545edc1c3036188c92d148663f`. Its optimized CLI SHA-256 is
`0f20a352672856ebbc5b4a475117a3f9d088132afa0582d42c7df1dce6f7c640`
(Rust nightly 2026-09-04, opt level 1).

Native, Terminal and wrapper execution each returned the unchanged expected
exit 70. Existing failure exits 71–87 still distinguish creation, six-byte write,
flush, reopen, seek to byte 2, readback of `c`/`d`, duplication with shared cursor
and `e` readback, writable attributes, non-replacing rename, the missing old
name, directory creation and cleanup. Eighteen existing nested blocking calls
now carry their explicit acknowledgement; neither operations nor oracle changed.

Normal update returned ReviewRequired after 222.969 seconds; 288 individual
pending decisions were accepted, and strict resume completed in 67.500 seconds.
`run --both --keep` completed in 63.421 seconds. The ordinary CLI helper guarded
all 179 tracked fixture/library inputs before and after the phases. Runtime
Terminal SHA-256 is `b5c5d769d08cd8ef5fac0f250a51dac214f282146ae3bd643bce4ab760d00622`;
native executable SHA-256 is
`e46ad9c84b813517ba4c0a2ea46ccbb3b5c33e0f7bd76c251578d136ed0f056c`.

The trusted adapter admits the exact `FlushFileBuffers`, `SetFileAttributesW`
and `MoveFileExW` contracts. Both captured path arrays retain their original
4096-element extents. The existing filesystem namespace and owned guest HANDLE
checks precede effects. Original scalar flags reach the OS; delayed reboot,
reserved and unknown move flags remain refused. Return values and last-error
state come from the actual Windows call, without simulated filesystem results.

The first breadth baseline exposed unchanged Std recursive directory traversal:
a structural `DirEntry` copy could not pass from a selected case below a
borrowed parameter into an owned formal. The ordinary projection operation now
joins readable source custody with the selected endpoint's reusable stored-copy
property. Exact case/member identities, target identity, active-case obligations
and recorded operand evaluation remain required. A source copy retains its own
completed whole home rather than aliasing the original payload.

The focused composition under
`samples/cli/facts/nested_machine_invocation/copied_payload/structural` passed
native and Terminal with exit 0 on the same Windows producer. It forwards a
two-field payload to a nested helper, clears its source case and checks both
captured values and surrounding canaries. Its twin removes only `copy` and
rejected with the authored borrowed non-copy transfer diagnostic. Source/Build
mirrors are retained under `tests/omega/{pass,fail}/facts/`.

The baseline cold optimized build took 1796.200 seconds. The same-branch
candidate restored its Cargo cache and took 594.110 seconds. The original miss
had two independent causes: Windows had no main cache and peer-branch caches
are inaccessible; Cargo/toolchain cache-key inputs had also changed. Retaining
a branch preserves locality only within its host and dependency epoch. The
existing pilot now exposes its internal checking-only lane for exact canonical
records without repeating qualified runtime programs; 33 focused Python
controls passed.

Original reports, producer/profile receipts, CRC/SHA-verified products and exact
source bundle are retained outside scratch under
`/Users/zcanann/Omega-build/filesystem-windows-breadth-20261102/` and
`/Users/zcanann/Omega-build/artifact-read/artifact-13131140963/`.
The source evidence ref is
`refs/corpus-observations/cdc089ff12b185545edc1c3036188c92d148663f`.
Portable filesystem programs, canonicalization and occurrence-bound close proofs
remain open under **FILESYSTEM-PLATFORM-SERVICES**.

[Checking-only run 41423781619](https://github.com/CathedralOS/Omega/actions/runs/41423781619)
recorded the two canonical mirror checks at `6485d52949547eda4dd664e1605c57eed115b16f`:
positive checked without diagnostics, non-copy twin rejected for its authored
reason. No native/Terminal or ordinary CLI project was selected in this follow-up.
The runner's original optimized-profile receipt and raw observations are retained
under `artifact-13130609728`; the exact source has its own retained bundle/ref.
