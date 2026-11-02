# Windows raw metadata acceptance — 2026-11-02

[Matching-host run 41409681344](https://github.com/CathedralOS/Omega/actions/runs/41409681344)
qualified `tests/omega/pass/filesystem/windows_raw_metadata_exit` on Windows x86-64
at `2467efdd6a51cabb719986f9ffb97f3e9da580eb`. The optimized CLI SHA-256 is
`4edf168d1743035cdd89d77fab821c61de91b4eafc3a09c9733638811595c1cd` (Rust nightly 2026-09-04, opt level 1).
This qualifies this raw metadata witness; portable filesystem, breadth,
canonicalization and close-release acceptance remain separate.

The unchanged exit-70 oracle passed native, Terminal and wrapper execution.
The program retains invalid HANDLE zero/error 6, a real anonymous pipe/error 50,
missing path/error 2, same-object metadata, preserved cursor and caller handle,
byte readback, directory classification and cleanup. Its 25 existing nested
blocking calls now carry the required explicit acknowledgement.

Normal update returned ReviewRequired after 153.562 seconds;
290 individual pending decisions were accepted, and strict resume completed in
50.125 seconds. Both phases retained the same root source,
whole-policy and build-observation commitments. Source inspection of `Main::main`
completed in 41.188 seconds, then canonical
artifact reload completed in 4.938 seconds
with the same fingerprint `0baa12c1f27ec65a0dddf71884d40f48a7de6e08d4052733bb90d9be8555df8b`.
`run --both --keep` completed in 40.234 seconds with native 70
and Terminal 70. All 181 tracked fixture/library inputs were verified before and
after each phase; no corpus runner or additional runtime program was selected.

The adapter joins exact DLL/symbol, semantic signature, calling plan and selected
storage before effect execution. Native HANDLEs are represented by this
instance's owned guest identities. Pipe outputs are rewritten in their original
borrowed homes. Metadata classes 0/1/9 retain their complete 40/24/8-byte records,
including the standard-info two-byte array field; scalar/reference width and
full codec paths cannot be replaced by a flattened byte count.

Initial review/resume exposed six private semantic-dependency paths changing
from source declarations `DllImport`/`ForeignBinding` to materialized
`<12, 10, 0>` carrier names after Std cache reuse. The corrected inert projection
uses retained checked applications, keeps nominal argument dependencies, and
checks source custody. Runtime carriers and exact cleanup callables remain
unchanged. No cache disabling, comparison substitution or hash omission is used.

Raw reports, original producer/profile receipts and CRC-verified products are
retained outside scratch under
`/Users/zcanann/Omega-build/filesystem-windows-metadata-20261102/` and
`/Users/zcanann/Omega-build/artifact-read/artifact-13124998729/`.
Runtime Terminal SHA-256: `7d264dcdbd70fe780b73fc0e33f90ab75afa75849bc1ce973db8e422406ebba0`.
Named-inspection artifact SHA-256: `e1b210be3c0e9792de7cb0d25125a49ea58d20f7da76c8a861167a7f0e5dafc9`.
The exact producer source is also retained at
`refs/corpus-observations/2467efdd6a51cabb719986f9ffb97f3e9da580eb` with a durable source bundle.
The existing pilot's 31 focused Python refusal controls passed.
