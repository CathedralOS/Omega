# BOOTSTRAP-CHAIN-NATIVE-EXECUTION — re-verification ledger

Re-verified on Linux x86-64 at `75650d2e94` (board tip at claim time) by
w10-04-bootstrap-chain-native (zergling Z3) under claim ticket `6197be6d`.
The item is a covered-alias of BOOTSTRAP-HOST-COVERAGE's recorded
native-execution evidence — every host-feasible seed-execution leg re-ran
green on this host class.

## Re-run on this host

```text
$ sh tools/bootstrap/check-chain-hygiene.sh
bootstrap chain topology and path hygiene OK        (exit 0)

$ sh tests/bootstrap/alpha-beta-edge.sh --edge
alpha conformance (alpha_x64_linux): 34 passed, 0 failed
Alpha bounds (native): 78 passed, 0 failed
reconstruction — Beta reconstructs its direct Alpha tape byte-identically
Beta root audit: 12,536-byte source -> 52 assertions + 273 emitting items
  -> 253 reachable Alpha instructions + 160 table bytes -> 1,773-byte tape
Beta word prefix: 736 exact status/stdout/stderr controls passed
Alpha-to-Beta edge VERIFIED                          (exit 0, 4m36s)

$ sh tests/alpha/parity.sh
seed parity (alpha_x64_linux agrees with alpha_ref.py): 33 ok, 0 failed

$ sh tests/beta/compiler/compiler-diamond.sh
compiler diamond (beta_ref.py assembles byte-identically): 6 ok, 0 failed
```

## Host-free gates (all exit 0)

- `tests/bootstrap/{alpha,beta,gamma,delta,epsilon,omega,proofs}-identity.sh`
  — each bound subject stamps exactly and refuses corrupted/truncated
  inputs.
- `tests/bootstrap/source-closure.sh` — 4/4.
- `tests/bootstrap/ocreq-tables.sh` — 224 contract/embedded records agree.
- `tests/bootstrap/chain-hygiene.sh` — 23 inventory cases pass.

## Residual legs are host-gated to owning items

- Literal Windows Git Bash route + macOS/Windows seed hosts →
  ALPHA-WINDOWS-CONFORMANCE, GAMMA-DERIVATION-CHECKER native acceptance.
  Windows, macOS, and QEMU acceptance is unavailable on this host.
- D→omega0/omega tapes → OMEGA-C (no omega0/omega tape exists under
  `bootstrap/`; the producing rung is still under construction).
- The omega-outcome/omega-request/omega-parser/omega-executable gates are
  explicit slow gates and were not re-run this cycle.

## Verdict

Nothing implementable remains on this host class — covered-alias of
BOOTSTRAP-HOST-COVERAGE; the residual legs are host-gated to their owning
items.
