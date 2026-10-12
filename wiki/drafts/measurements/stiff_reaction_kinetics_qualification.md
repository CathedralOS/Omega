# Stiff reaction kinetics qualification

This record accompanies `samples/cli/independent/stiff_reaction_kinetics`.
The sample accepts a runtime-supplied three-species, three-reaction equal-mass
mass-action network. A two-stage, second-order SDIRK step, analytic Jacobian,
damped Newton solves and step doubling advance a committed state; rejected
trials roll back. Each stage reuses the backward-Euler residual owner.
Invalid input and exhausted convergence produce separate nonzero exits.

## Frozen independent references

`data/FROZEN.sha256` preserves the exact inputs and references. SciPy 1.17.0
Radau and BDF use rtol 1e-11 and atol 1e-14. Their largest disagreement across
the 17 valid cases is 8.048145483385838e-11; reference mass drift is at most
2.22e-14. Distinct-rate and equal-rate linear chains also use analytic exponential
solutions. The reference generator never implements the sample integrator.
The remaining 21 cases exercise malformed/nonphysical input and convergence
exhaustion. Five model/time groups each refine rtol through 1e-3, 1e-5 and 1e-7.

## Recorded execution

The final SDIRK2 source at `d3f34d02fc55963cec355125541de37eabc3bff8` passed
all 38 frozen cases on native and Terminal execution, including five strict
refinement groups. Stdout, stderr and exit match exactly for every case.
Native product SHA256: `1bcf4476c1c844b4d63cf8e6c96f0d548d50c12d7f52b489cbf54b2e886607bc`.
Compiler SHA256: `6cb6be740b7cdf927d4b9f91e63182481629366bf112cb8a71d49973dd42e1d6`;
Rust source `49a0397cc29203b3bc1f57cad87c25e2f2143657`, embedded commit unrecorded.
Terminal sponsor SHA256: `99039ec360a380ab4d50485ba2b29af6bcf0abbac98c5300b7a9770759a74836`,
linked against the exact successful CLI Cargo artifact JSON. Its normal accepted
compilation retained one canonical Terminal artifact and original runtime provisions;
the same report published the native product. No fixture-specific numerical path exists.

Commands (from the repository root):

```sh
python3 samples/cli/independent/stiff_reaction_kinetics/validate.py --executable <published-native-product>
python3 samples/cli/independent/stiff_reaction_kinetics/validate.py --terminal-sponsor <exact-sponsor> --captures <completed-direct-sdirk-captures>
```

Both commands exited 0. Compilation took 532.105423245 seconds; all Terminal
cases took 434.097410607 seconds in total, with the longest 112.466947155 seconds.
The complete sponsor exited 0 in 16m06.639s. Every invocation had an explicit
100000000 fuel allowance and its own finite replay input and modeled clock.
Valid fixtures required between 1 and 480 accepted steps. Reported mass drift
was zero at 1e-12 resolution; reconstructed totals were checked independently.
Malformed/nonphysical cases exited 2, and trial/Newton exhaustion exited 3 with
the exact authored refusal text. A separate invalid-index source exited 1 with
`cannot prove index `3` is within length 3`.

| Model / final time | Error at rtol 1e-3 | Error at 1e-5 | Error at 1e-7 |
| --- | ---: | ---: | ---: |
| robertson / 0.4 | 5.1160878e-07 | 9.45077797e-08 | 5.31798883e-09 |
| robertson / 4.0 | 2.17088513e-05 | 1.52574834e-06 | 7.63583423e-08 |
| robertson / 40.0 | 9.07274437e-05 | 6.85859272e-06 | 3.40252719e-07 |
| chain / 1.0 | 0.000160040204 | 1.08433897e-05 | 6.13077669e-07 |
| equal_chain / 2.0 | 7.83474577e-05 | 4.81390467e-06 | 2.87225671e-07 |

The original accepted integrated producer a8f8a409b11a9ab2ce240d225375d5bdb04b4886
published a native product which passed all 38 cases and five strict refinement
groups. Two original Terminal captures completed and matched native output;
the higher-cost run was stopped to repair measured arithmetic and historical
scalar-storage costs. Original native compilation took 759.58905773 seconds.
The repaired first-order producer later compiled in 526.138954582 seconds;
two Terminal cases took 19.524385743 and 107.031192395 seconds. The scalar collector
initially deferred throughout the entry invocation because of a placed view;
independent review supported preserving its explicit backing identity instead.
These observations establish before measurements; they do not qualify SDIRK2.

## Necessary producer repairs

Structural subtree operations visit an opaque identity range instead of every
historical carrier. Settled block boundaries conservatively collect orphan
scalar records while preserving current/suspended roots, referents, explicit
installed-view backing identities and non-scalar storage; primitive loans,
byte/element views and nominal cleanups defer collection.
The public 4096-copy regression retains caller records and a shared referent.
It passed native and Terminal execution after the installed-view root repair.

Exact big-number division and GCD recognize dyadic operands. Integer quotient
and remainder signs, normalization and software floating-point rounding remain
unchanged; 63 numerics unit tests passed, including independent old-algorithm
comparisons at limb boundaries and binary64 subnormal exponents.

Runtime spill admission retains constraint and transport validation while
computing only unit liveness. Its CFG fixed point and reverse transfer share the
full liveness implementation. Full virtual-register liveness and evidence remain
in their normal analysis owner.

## Integration method

The final tableau is `A=[[gamma,0],[1-gamma,gamma]], b=[1-gamma,gamma]`,
with gamma represented as 0.2928932188134524 (1-1/sqrt(2)). The second stage's
base is `y0+((1-gamma)/gamma)*(stage1-y0)`. Negative/nonfinite stage bases reject
the trial; no concentration is clipped or renormalized. Six Newton stage solves
form one full and two half steps. The full/fine normalized difference divided
by three estimates the accepted fine result's local error. Growth occurs below
0.1, allowing the order-two factor-eight error increase when doubling h.
Independent mathematical review checked order, stiff accuracy, L-stability,
conservation and conditional positivity. The completed frozen scientific checks establish the implemented method; the
first-order measurements were not substituted for them.

## Environment and evidence discipline

Linux x86_64; four CPU quota, 16 GiB memory; pinned Rust nightly 2026-09-04,
mbx 1.21.0. Requested model is the repository default gpt-6.1-sol/medium;
actual runtime model, effort and generation are not exposed and remain unknown.
Native and Terminal use the normal accepted project graph and original runtime
provisions. The standard-library FilesystemHost row is a source-risk disclosure;
actual application authority is Console input, output and process exit.
Generated captures remain under excluded `build/`; an accepted host-local lock
is not shipped. Source, commands, original witness and observed reports are
preserved in the recovery archive and journal.

The sample did not have a before-witness when assigned. Its initial authored
source actually failed on `initial.*` with `expected identifier, found punctuation *`;
the real source hashes and diagnostic are preserved. No pre-existing witness
was invented.

## Scoped checks and integration

The 4096-copy/reference fixture passed direct `omega run --both --keep` after
rooted collection and the normal filtered both-engine corpus gate. Only its new
rows were retained in the source and Linux both-engine goldens; unrelated rows
and context were preserved. The source-invalid bounds control still rejects.

Scoped Clippy is baseline-red: Terminal reports 36 errors versus the recorded
b2d763407d baseline's 37, with no new normalized diagnostics; incoming main
removed one clone-on-copy error. Selection reports 45 diagnostics, each on a
file unchanged from main. The unchanged capabilities test suite previously
failed 15 tests on both original main and candidate because portable entries
lacked retained machine/target-family heads. These are recorded baseline failures,
not passing gates or suppressions. Numerics passed all 63 tests.

The public command `omega run --both --keep` on frozen input
`robertson_2_5_r1000.stdin` completed with native exit 0 and Terminal exit 0,
reporting 29 accepted and 15 rejected steps. Its numerical report matched the
SDK capture. Final independent read-only reviewer `/root/bio03_review` at
`2d15559f65325f1fa4f045220ed29fed26d0bb1f` independently recomputed frozen
hashes and all actual numerical, conservation, refinement and cross-engine checks;
no blocking defect was found. Sample, fixture and repaired Rust bodies remain
unchanged from their qualified revisions.

Incoming main `04dde76a66` changes computed/runtime structural-copy index-selector
scheduling. These programs copy whole records or named fields; the parser's
u64-array operations are scalar reads/stores. The new selector-capture branches
are unexercised here, as independently confirmed during final integrated review.
Recorded qualification therefore remains applicable under actual-changed-input
validation. The normal workspace-aware exact-SHA build receipt is still required
for publication; no checkpoint or archive substitutes for verified main landing.

Incoming main `5e5835ccaddcc1be1b2e5e160c4a1f79c4a14eef` changes enum case
payload observation used by the input parser's `ByteRead`. The rebuilt compiler
SHA-256 `69f8fd05bf193236075b023d5228f63b1cc8632595f64db92c33b2954f1a2ddd`
completed the same public frozen Robertson command with native exit 0 and
Terminal exit 0. Its stdout and retained Terminal stdout matched the qualified
capture exactly; retained Terminal stderr was empty. Compiler identity was
refreshed through normal `omega update`, with no source or capability changes.
The 4096-copy/reference source check also passed on this producer.

Incoming main `9a404fe988651d6e1df2db1cbe551d28c3c12e64` expands indexed
reference-return origin families. Independent reviewer `/root/bio03_review`
confirmed these programs return plain records or scalars and exercise no indexed
reference-return expansion. Non-indexed paths preserve previous behavior, and
interpreter, numerics and liveness repairs are unchanged. The completed matrix
and public command remain applicable; no additional execution check is required
for this incoming change. Final publication uses a fresh exact-SHA receipt.
