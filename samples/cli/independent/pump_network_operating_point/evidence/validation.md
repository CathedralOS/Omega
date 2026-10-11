# Validation receipt

The oracle was frozen at 2026-10-11 00:41:12 UTC, before Omega execution.
SHA256: `19a20e8e3332ea6e5f7e1b99b767b0adf8e3882f918fd24f6b6401fc72ffa928`.
Flow tolerance is 1/1024, exact report-fraction tolerance is zero, and the
frozen energy/head residual tolerance is 800/1024. No expectations were tuned
from observed output.

Original main base: `b4008bb3c7894aebad47eaccf8af12bb4a8006b1`.
Preserved starting checkout: `/workspace/Omega`, branch `work`,
`03e477fa424a9e6a2db7f49e17bf896ab3f76b48`; it was never reset or switched.
Candidate worktree: `/workspace/Omega/.codex/worktrees/new079`.
Host: Linux x86-64, UID1000, 5 CPUs, 18 GiB RAM; Rust1.100.0-nightly
(a69a63265, 2026-09-03), mbx1.21.0, CARGO_BUILD_JOBS=4.
Private target cache: `/workspace/cloud-setup/cache/mbx/targets/v1/473aea33a3ac1eed7a482b9f841c666345bb3d95781ad9b35d189304f280ec98`.
One Rust build and one Omega execution at a time were used; corpus jobs=1.

The default sandbox rejected cache custody because it represented `/` with
UID65534. Supported per-command escalated execution preserved UID1000 and real
root custody; every such request was approved. No ownership/guard mutation or
actual package/approval denial occurred. No foreign-target execution was claimed.

## Original-base and bounded-repair evidence

- Unmodified-base application check passed (28 source files).
- Ordinary inspected package review accepted 286 exact rows in two packages
  and published the accepted lock. Std source was unchanged.
- Complete intended-invalid source rejected exactly the fixed-array extent rule.
- Base native production failed with NoCommonCandidate(function10,register6).
- Repaired native/Terminal rational run agreed at exit0 and produced the frozen
  branch/head/energy report. Retained native artifact: `/tmp/omega-probe-21543-0/omega-program`.
- All76 native cases matched stdout, stderr and exit (`native-validation.json`).
- Focused native+Terminal corpus: new call-live remainder customer and existing
  trapping-add/borrowed-array pressure customers all built, executed and agreed
  at exit70; authored streams matched. Gate exit1 means one new row and two
  improvements; no golden was rewritten.
- All14 allocator unit/route tests passed. Scoped crate check and Clippy passed.
  Dependency warnings and unchanged existing lint warnings remain in the logs.

Exact log names and the final integration receipt are supplied in the Library
review archive with full source hashes, commands, patch and git bundle.

- Complete original-base+repair corpus matrix: 76/76 cases built and matched
  native and Terminal exits, stdout and stderr. Gate exit1 reports exactly
  76 new scratch rows, with zero oracle failures; no golden was rewritten.
