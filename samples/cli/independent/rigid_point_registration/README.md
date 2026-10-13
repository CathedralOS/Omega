# Weighted rigid point registration

A bounded stdin application fitting a proper rigid transform to paired 3D
points. All computation is private Omega source using f64 operations and the
existing std Console provider. The independent Python oracle is a qualification
tool, not the application. Build metadata explicitly binds Linux x86_64,
Windows x86_64 and macOS arm64 program entries. Qualification here covers Linux.

## Input and refusals

Supply a count followed by that many records, whitespace separated, ending at
EOF. Each record has seven decimal fields:

```text
N sx sy sz tx ty tz weight
  sx sy sz tx ty tz weight ...
```

N must be an integer from 3 through 16. Coordinates must lie in [-1000,1000]
and weights in (0,1000]. Space, tab, CR and LF separate fields. Decimal tokens
allow an optional minus sign and one dot, at least one digit, at most 12 total
digits and 8 fractional digits. `1.`, `.5` and `-.5` are accepted. Plus signs,
exponents, NaN and infinity spellings are rejected. The grammar and finite
bounds ensure all admitted coordinates and weights are finite. No extra tokens
are accepted. The complete stream must contain fewer than 2048 bytes; the
reader refuses at the cap without waiting for EOF. Empty stdin, missing fields,
invalid count, nonpositive weights, malformed/nonfinite coordinates and any
limit violation print `error invalid input` and exit 64.

Coincident or collinear source points have no unique rotation. The program also
refuses numerically unresolved near-degeneracy or a tied optimum (for example
coincident target points). It prints `status non-identifiable` and exits 2.
The classification is numerical: for normalized weighted source scatter G,
require trace(G)>0 and the sum of its three principal 2x2 minors greater than
1e-12 trace(G)^2. Require the largest Davenport eigenvalue to exceed each other
one by more than 1e-12 times its magnitude. These declared thresholds can refuse
an exactly unique but severely ill-conditioned fit; the sample does not claim
1e-7 transform accuracy for such inputs. A noncollinear plane with adequate
spread is accepted. Frozen boundary cases bracket the rank threshold at heights
1e-6 (refused) and 2e-6 (identity fit accepted).

## Computation and output

For positive weights w_i, minimize sum_i w_i ||R s_i + t - target_i||² with
R transpose R=I and det(R)=+1. Weighted centroids give t=target_center-R source_center.
The centered cross covariance forms a symmetric 4x4 Davenport matrix; 24 cyclic
Jacobi sweeps find its largest eigenvector. Normalizing that unit quaternion
constructs a proper rotation directly. Reflected data retain the best-fit
positive residual; the solver never emits a reflection or reports false exact
alignment. Jacobi skips off-diagonal terms below 1e-15 times the two diagonal
magnitudes. Storage and iteration counts are fixed; no external numerical
package, random initialization, variable iteration cutoff or point matching is
used. Pair order is supplied by the caller.

Exit 0 emits `status fitted; numbers in 1e-12 units`, then:

- `rotation`: nine row-major elements of R.
- `translation`: three elements of t.
- One `point` row per supplied pair, in input order: transformed x,y,z,
  sqrt(weight) times the three signed residual components, and their norm.
- `rms`: sqrt(sum_i w_i ||R s_i+t-target_i||² / sum_i w_i).

Every numeric field is a signed decimal integer scaled by 1e12, truncated toward
zero. Divide by 1e12 to recover physical values. Integer quantization is below 1e-12, with additional f64 scaling rounding
(up to about 1e-10 at the admitted magnitude limits);
input/model bounds keep the scaled magnitudes within signed 64-bit range.
Coordinates and translations share caller units; weighted residuals use those
units times sqrt(weight). RMS retains coordinate units. Weights need no
normalization. There is no correspondence search, scale fit, uncertainty model,
outlier trimming or real-world safety inference.

## Frozen independent qualification

`validation/reference.py` freezes weighted Kabsch/SVD results, including the
proper-rotation correction, separately from the quaternion solver. Exact quarter
turn and translation fixtures also assert their constructed rotation and
translation before freezing. `validation/reference_boundary.py` freezes rank
boundary, large-coordinate, tiny-weight and weight-scale cases. Both authoring
tools refuse to overwrite their oracles. Validators pin both oracle hashes and
every input hash before execution.

The 38 cases cover exact and noisy fits, minimum three-point planar data, a
larger noncollinear plane, reflection, permutation, six seeded 16-pair noisy
cases, coincident/collinear source and coincident target refusals, numerical
boundaries and 18 malformed runtime inputs. Numerical comparisons check every
transform element, every transformed coordinate, weighted residual component
and norm, and RMS to 1e-7 absolute. Orthogonality and determinant errors must be
at most 1e-8. Geometry is also recomputed from the emitted R and t. All outputs,
stderr and exit statuses are captured. Exit parity alone is insufficient.

`intended_invalid/` is a separate complete source-invalid control. Its coordinate
helper accesses a 48-element source array through an unconstrained index and
must fail for `cannot prove index` and `within length 48`, not an admission,
typing or cast error.

## Reproduction

From the repository root, activate the documented toolchain. Set
`OMEGA_LIBRARY_ROOT` to this checkout's absolute `source/library`. Use a compiler
built from the recorded producer revision. Ordinary package review accepts the
local sample/std declarations and the sample's three Console runtime permissions:
input, output and process exit. std's filesystem source-risk declaration alone
is not a host filesystem grant. Inspect the pending review before resuming;
retain all genuine authority and custody checks.

```sh
sample=samples/cli/independent/rigid_point_registration
omega update --project "$sample" --target linux_x86_64
omega update --resume --project "$sample"
omega --target linux_x86_64 "$sample/main.omg"
python3 -B "$sample/validation/validate.py" "$sample/build/omega-program" --output build/w211-native
omega --check --target linux_x86_64 "$sample/intended_invalid/main.omg"
```

The last command must refuse the exact coordinate bounds obligation. For actual
Terminal stdout, `validation/terminal_validate.py` links the read-only observer
against the verified producer's build artifacts. Supply `--producer` (directory
containing omega and deps), `--receipt` (exact compiler-artifact hashes), and
`--output`. The receipt is produced from the successful Cargo/MBX JSON artifact
manifest using the repository's existing
`samples/cli/independent/dose_response_fit/validation/producer_receipt.py`.
The observer compiles this actual admitted complete project once, then executes
its retained canonical Terminal artifact with the exact runtime provisions and
finite fixture stdin. Each invocation delegates only isolated-domain process
exit, gets 100 million fuel units and a 60-second watchdog. Decline, timeout,
missing capture or exhausted fuel fails qualification.

Qualification commands and captured results are retained in the outcome's
ignored evidence directory and preserved through the supported artifact route.
EOF, whitespace, source bounds and proper-rotation assumptions are covered by
frozen cases; supported host bindings do not imply untested host qualification.
