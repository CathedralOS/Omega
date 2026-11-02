# Delegated Darwin metadata mutation — 2026-11-02 UTC

The unchanged `tests/omega/pass/filesystem/runtime_portable_metadata_exit`
completed the normal CLI path on macOS 15.5 ARM64: **native exit 0 and Terminal
exit 0**. It creates and links a file, compares descriptor/path metadata, sets
both timestamps and checks their exact new values before closing and removing
its files. The original program and exit oracle were preserved.

This subtracts only Darwin `_futimens` from `FILESYSTEM-PLATFORM-SERVICES`.
Windows/Linux breadth, release classification and the rest of that task remain.

## Exact source and producer

- Clean program/compiler source: `3cfea8f4f431ac2aed98dfcda23ddb9bb62f9211`.
- CLI: opt-level 1, SHA-256
  `9eebcd0155f1019ae9de41ca84d794ca71532b4a0d6d767c4a57f0ee1c0caa9f`.
- Original producer/profile and ZIP CRC `0xd1304887` were retained with the
  executable. All 110 copied fixture/Std inputs were hashed against the source
  before and after every ordinary admission/runtime phase.
- Source evidence is retained in
  `refs/corpus-observations/3cfea8f4f431ac2aed98dfcda23ddb9bb62f9211`.

## Observations and authority counterfactual

[Mac run 41366045245](https://github.com/CathedralOS/Omega/actions/runs/41366045245)
checked the exact fixture without diagnostics (84,785 ms), then observed native
exit 0. Its Terminal run refused **before execution** because no explicit host
filesystem namespace was supplied. That corpus harness intentionally supplies
`Unprovided`; its outer runtime gate is red, and is not claimed as agreement.
The refusal diagnostic was `runtime-filesystem: no explicit host filesystem
namespace was supplied`.

The same retained CLI then used its ordinary `HostNamespace` supply, copied
project topology and normal package review, with no source, lock or policy
bypass. Update returned ordinary ReviewRequired in 558.695 seconds; its 262
pending decisions were accepted normally. Resume exited 0 in 574.919 seconds.
`run --both --keep` completed in 180.222 seconds with explicit `native exit 0`
and `interp exit: 0` markers. Phase allowances were 600 seconds for each review
phase and 240 seconds for runtime. A separate two-job Cargo build was active;
these wall times do not establish a performance change.

## Preserved contracts

The matching-host libSystem adapter checks the complete semantic timespec
array and retained native recipes: two records, four signed 64-bit words,
32-byte extent, at least 8-byte alignment and offsets 0/8/16/24. It preserves
nanosecond sentinels, exact toolchain service admission, explicit namespace
supply and guest descriptor translation. No type/policy spelling or default
packing supplies geometry, and the private native sponsor gains no capability.

Direct external satisfiers keep the public declaration ABI. Their synthetic
Source/Terminal route retains the whole original schema and selected calling
receipts separately. Production, recovery and native use validate the complete
semantic contract, application roster and exact Source-to-Terminal association;
ABI commitments are unchanged. Strict ordinary schema validation remains.

Raw reports, original producer/profile, source bundle, review files, input and
log hashes, retained products and the final acceptance receipt remain outside
scratch at `/Users/zcanann/Omega-build/darwin-stat-abi-20261101/`. In particular,
`filesystem-3cfe-normal-cli/acceptance-receipt.json` records the actual ordinary
chain; `pilot-41366045245/artifact/` records the separate namespace control.
