# Numerical qualification

Linux x86-64 execution, compiler source base `5c5bb7a1c2c`, with the required
sine-coefficient correction and validation workspace membership. Exact source,
CLI/harness/native binary hashes and both engines' captured stdout/status are
in `evidence.json`. Native and Terminal each passed all 15 frozen direct-DFT
fixtures and all 21 invalid controls. The reference and tolerances were frozen
before execution; validation never regenerates the reference.

| Fixture | Native integrated power | Terminal integrated power | Reference |
|---|---:|---:|---:|
| tone8 | 2 | 2 | 2 |
| gain_scaled | 8 | 8 | 8 |
| dc | 12.25 | 12.25 | 12.25 |
| maximum_calibration | 121000000.000032 | 121000000.000032 | 121000000 |
| hann_overlap | 1.325573160 | 1.325573160 | 1.325573205 |

Output truncation is at one nano-unit of density and one micro-Hz of frequency.
Density tolerance is `2e-9 + abs(reference)*1e-10`; the integrated-power bound
includes the sum of bin truncation allowances. This checks physical units as
well as bins. Each independently computed periodogram also satisfies Parseval.
The maximum-scale fixture checks i64 reporting without saturation.

The original library's `x^17` sine coefficient was ten times `1/17!`. A real
run produced `density_nano=7744000002539075584` in both engines, contradicting
the frozen `7744000000` density. The authored trigonometric endpoint regression
also returned native 19 / Terminal 19 before correction, then 0 / 0 after it.
The corrected spectral run produced `7744000000002028544` nano-units, a relative
error of approximately `2.62e-13`, within the unchanged oracle tolerance.
Agreement between engines alone would have missed the defect.

After integration on `d041496df10`, the compiler and capture harness were
rebuilt together. All 15 positive fixtures and 21 invalid controls again passed
separate native and Terminal numerical checks; the endpoint returned 0 / 0 and
complete std checking again compiled 76 files. Those second actual captures and
binary hashes are retained under `integrated` in `evidence.json`.

Other qualification: the scoped sample check and native gates each passed
1/1; complete std checking compiled 76 source files; the focused corpus checked
2/2 and executed both 2/2 (endpoint 0 / 0, existing interior canary 70 / 70).
Strict Clippy passes for the capture harness with `--no-deps`. Dependency-wide
Clippy hit an existing `useless_conversion` at
`omega-rust/psi/foundation/optimization-selections/src/optimization_selections.rs:152`;
that source is unchanged by this work.

Before landing, integration on `cf249b8954d` was qualified with compiler and
capture harness rebuilt from `251a03a39b4`. Native and Terminal again passed
all 15 positive fixtures and 21 invalid controls against the unchanged frozen
oracles. Both trigonometric corpus fixtures executed successfully (endpoint
0 / 0; interior 70 / 70), and complete std checking compiled all 76 files.
Actual stdout/status, producer identity, binary and source hashes are retained
under `resumed` in `evidence.json`. All 73 landing-tool tests also passed.

When main subsequently added float-contract/custody changes, integration on
`66ad153cb23` was rebuilt from `d0e1c0faa2b` and requalified. All 15 positive
fixtures and 21 invalid controls again passed each engine independently;
complete std checking compiled 76 files and both trigonometric corpus fixtures
retained their authored exits. These actual captures and input/binary hashes
are under `final_integrated` in `evidence.json`.

The package proposal was inspected before acceptance: root permissions were
exactly Console input, output and process termination; no build-host requests
or root filesystem permission were added. Source-risk acceptance was kept
separate from those permissions. The normal host approval route preserved
cache ancestry custody; its checks were not disabled.
