# Orbital dynamics qualification

The sample uses velocity Verlet for a planar test body around a fixed central mass.
Linux x86-64 source, native and captured Terminal results are qualified against
independent references. The explicit entry is `Main::main`; stdin is limited to
512 read operations and seven fixed-decimal fields, with at most 4096 steps.

The three frozen manifests pin 42 reference and input files for 36 cases.
Analytic circular/eccentric Kepler trajectories and SciPy 1.17.0 DOP853
(`rtol=2.3e-14`, `atol=2e-15`) agreed within 1.106e-12 before sample execution.
The original freeze was preserved; separately frozen additions cover a drift
segment crossing the collision threshold with both endpoints outside, decimal
fraction limits, and exact stdin read limits. No Verlet implementation is used
as an oracle. `check_output.py` checks every checkpoint, independently recomputes
energy and angular momentum, and checks exits, diagnostics and periapsis times.

| Measurement | Observed |
| --- | --- |
| Unit circular final position error, dt=2π/1280 < 0.005 | 5.04654814684e-5 |
| Circular coarse-to-fine error ratios | 3.99988978654, 3.99997244332 |
| Eccentric timestep-halving error ratio | 3.99978401128 |
| Maximum independently recomputed angular momentum drift | 5.88418203051e-15 |
| Eccentric periapsis time (expected π) | 3.141587797782398 |

The 36 cases include circular, eccentric, reversed, axis-rotated and obliquely
rotated trajectories, nonunit gravitational parameter, 4096 steps, initial and
segment-interior collision, and invalid syntax/nonfinite/range/step controls.
Collision reports exit 2 before evaluating a singular force. Source-invalid
`coordinate(1000001.0)` rejects as structurally false against the declared bound.

The sample exposed an actual compiler failure: a guarded operator call lost its
declared result type. The general repair resolves the selected declaration and
instantiates its type parameters from explicit or inferred arguments, retaining
constant/lifetime origins. Admission still checks arguments and proofs. Positive
fixtures cover machine conversion, generic explicit/inferred calls and parameter,
field, local and nested expressions; a mismatched explicit generic argument
continues to reject for the intended reason.

Package review retains the exact user-approved inventory: 141 callable rows,
141 external-supply rows, two std capability source disclosures, and three actual
Console permissions (input, output, process termination). The FilesystemHost
source disclosure grants no filesystem runtime permission. An initial broad
acceptance was refused by automatic approval review; no acceptance occurred
until the user explicitly approved this inventory. Validation uses the approved
host execution route so existing cache custody and filesystem/runtime authority
checks remain active; no check or compiler identity is bypassed.

The ordinary CLI default Terminal fuel declines the full-period workload.
`terminal_sdk.rs` instead executes the actual retained Terminal artifact through
the public runtime API with bounded fuel 200,000,000 and captured stdin/stdout,
using normal package acceptance and exact executable identity. Exit parity alone
is not accepted: all numerical output is checked, and native/Terminal streams
must match byte for byte. This does not claim execution on other target hosts.

Recoverable source patches, retained products, stream captures, frozen hashes
and validation receipts are preserved through the supported recovery artifact
and Library workflow. The default Rust build cache was kept warm, with one Rust
build active at a time. Existing unrelated Clippy warnings are not new failures.

The final release producer was rebuilt over main
`b7aaf37278ea0c23a273db7eccae9bacdee1e221`. Its native matrix passed all 36 cases;
source controls returned 0/1/1, and the affected source corpus was clean (two
checked, one intended rejection). Four runtime-regression fixtures produced one
source-only check and three native/Terminal executions with the expected exits
0, 70, 70, no lost successes and no wrong rejection reasons. The product closure
check passed; Clippy completed with the same two existing stage-03 warnings.
Later main changes add independent samples, a validation workspace member,
a CLI example and local coordination probes; they do not change the executed
product compiler, std, or the sample's computational inputs. An exact-commit
landing receipt rechecks the integrated candidate after reconciliation.

The final producer captured and numerically verified all 36 Terminal cases. All
36 stdout streams and the recorded exits match native byte for byte; all stderr
streams are empty. The retained Terminal artifact SHA-256 is
`39d2a68a58624a7f2bb34fb366e6be02e3b08a0456f5976ca8aad8f046474684`.
