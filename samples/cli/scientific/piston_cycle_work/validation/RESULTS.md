# NEW029 validation record

The complete application has executed through native and Terminal production on
Linux x86_64. Each route passed the 25 frozen input cases: 14 analytic executions,
six invalid-stage executions and five invalid-input executions. Actual captured
histories, work, balances, input bytes and exit statuses were checked against the
independently frozen oracle. The transfer bundle records subsequent integration
qualification, exact final candidate, command exits and all artifact hashes.

## Scientific inputs and oracle

The declared model is an imposed quasi-static pressure path, with kPa and litres
whose product is joules. Work is positive by the piston. Isochores interpolate
imposed pressure with zero work. No equation of state, heat or temperature is
inferred. Integer exponents are -1, 0, 2 and 3; exponent 1 is rejected because its
integral requires a logarithm outside this declared model.

`expected.json` was frozen before any Omega execution, SHA-256
`df99a86ce60e8566786f6ff1fb778226e18b90dbfba2d9f1dd2f5425266e372a`.
The independent reference uses Fraction arithmetic and
`W=(P1*V1-P0*V0)/(1-n)`, with zero work for holds. Internal absolute tolerance
is 1e-9; printed absolute tolerance is 2e-6 for six-place output. Neither the
oracle nor its tolerances was changed to match execution.

`input_cases.tsv` was separately frozen before execution, SHA-256
`c82ebabe11b7b3c15fff0876fa42a9845309d92b78a6fd2ca6ff190bd26ebe11`.
It exercises each selector using both digit-plus-EOF and digit-plus-LF-plus-EOF,
and includes empty input, non-digit input, extra digits, long input and an extra
byte after LF. The validator checks the captured input bytes themselves.

## Qualification and executor

Original clean checkout: `03e477fa424a9e6a2db7f49e17bf896ab3f76b48`.
Original development base: `33e34fd97e763f4725c6290f0a0d6ea1ec829224`.
Original reviewed candidate is preserved as branch
`new029-preserved-reviewed-9a51` at
`9a51dcb54acc4444c13fb27a6621f779887f4ccc`.
The first successful complete runtime qualification used main base
`343e19054f52b1b9939306e8ad27336f7890303c`, sample candidate
`49c40601931e6f84cb19a1cdb70cb466f7b423d0`.

The selected executor was verified READY after reconnection without replacement.
Linux 6.18.44 x86_64; five CPUs; approximately 17 GiB RAM; no swap.
Pinned toolchain: nightly-2026-09-04,
`rustc 1.100.0-nightly (a69a63265 2026-09-03)`.
One Rust build and one Omega execution at a time; Rust parallelism four.
Approved per-command execution retains UID 1000 and sees the real root UID 0.
The default sandbox's synthetic root fails existing cache custody checks.
No ownership, permissions or custody guard was changed.

The 343e1905 compiler built successfully in 7m41s, SHA-256
`2c0745c8ceaa5db8fa6ba2f16cdad090e557b3dc3e7b30916ab8b8a05d7a3e1a`.
Published executable SHA-256:
`9aa1844d9cec4d8132a3843c766027793d556b3f6a43e20d58703d35913ae321`.
Terminal capture executable SHA-256:
`033ae4ce052a78150c7a3d949bb11801518a4e11fcecdc11aa548b0a174d9998`.
The harness links against the exact CLI dependency fingerprints, not artifacts
selected by timestamps. It uses normal package preparation, native production
and the retained Terminal artifact; it calculates no reference physics and
manufactures no compiler IR. Fuel allowance is 10,000,000 per input with isolated
domain termination. All 200 input/stdout/stderr/exit capture files are hashed in
the transfer evidence. Other native operating systems were not available on this
executor and are not claimed.

## Package review and compiler history

The initial acceptance command was rejected by automatic approval review before
execution. Its transaction, 286 pending decisions and absent lock were preserved.
The user explicitly renewed one attempt for the same original action and target.
That single renewed `update --resume` completed with exit 0 and published
`build.omg` and `omega.lock`; no alternate origin or executor was used.
Original pending proposal SHA-256:
`d6f5b6d42a03abf9ab5df0380a89e214141807e85eaf184141d6dc9a1c189807`.
Original settled lock SHA-256:
`a15437ce9f1db9dee0a048db539646664478fec4a4dd86c3df8c0e12bf9c780a`.
All 286 choices settled as accepted: 140 callable and 140 external-supply rows,
two standard-library dangerous capability rows, three application Terminal
permissions and one application process capability row. Their complete meanings
and exact source capture are retained in the bundle. Standard-library source
rendering exceeded the renderer resource limit; the complete raw sources were
independently captured and checked instead.

The normal current-main identity refresh completed automatically with zero new
decisions. All 286 complete canonical policy meanings were byte-for-byte equal
to the approved original lock. Changed compiler/toolchain identity correctly
refused the old lock before that refresh. The only source integration adjustment
was `provider_proposal: None` in the capture harness for the current public API;
scientific sources, fixtures and oracles stayed byte-identical. Reporting this
record changes the source pin; normal preparation must refresh identity again,
without accepting additional policy meanings.

The original compiler required a bounded operator-family presence repair. Its
complete positive fixture reproduced the gap on the unmodified base compiler;
the repaired compiler checked it, and the wrong-signature sibling rejected for
the declared exact overload rule. The focused corpus retained the duplicate
result-domain rejection. That general repair is already present upstream; the
original regression fixtures and measured rows remain preserved in the original
reviewed branch and transfer evidence. The final candidate uses the existing
upstream coverage, with no duplicate presence repair or golden changes. Earlier
unrelated corpus rewrites were restored before the candidate was committed.

The original repaired binary passed `--check`, but native staging failed with
`RegisterAllocation(RuntimeSpill(Homes(UnresolvedEntryTransitions { function: 9,
register: 9, count: 2 })))`. This is preserved as an actual compiler gap, not
weakened or hidden. Current main produced the unchanged application and both
runtime routes passed. Incoming changes to exercised compiler/runtime/source
readers require affected checks again; the transfer manifest distinguishes each
recorded producer and its actual evidence.

A subsequent main producer exposed a source-selection custody gap at
`core/algebra.omg:44:27`: a copied resultless trait contract reconstructed its
proposition equality as `Float::equal`, conflicting with the authored builtin
meaning. The complete unchanged application reproduced the failure. The bounded
repair binds equality on resultless trait requirement contracts at their source
before applied callable copies are inspected. It preserves the existing
resultless-law semantics, consistency guard, type checks and proof obligations.
The transfer evidence includes the exact failing producer, complete diagnostic,
repaired compiler identity and focused valid/invalid controls; no scientific
source, fixture, oracle or tolerance was changed.

The complete intended-invalid control independently rejects with exit 1 and the
frozen nominal-type diagnostic:
`expects the \`Volume\` data type but got \`Pressure\``.
Wrong-reason rejection does not pass. Independent review approved the integrated
application source, conditional on actual execution; the final transfer includes
review of the integrated candidate and its completed evidence.

Claims coordination timed out with uncertain ticket
`7343c39c10f34082aca77076a8bdc572`; no successful registration is asserted.
The read-only status inspection and unavailable mutation receipts are retained.
There are no worker or unfinished numerics dependencies.

## Published-main integration

The next pinned integration base is
`4146666641c68fb3642dfd3e182f357720750184`. Its published rooted-operator custody
repair `783a1cde856bf35b023f8c1f0a34f8ee0497098e` supplies the general fix for detached
contract copies. The integration withdraws this candidate's redundant 41-line
pre-specialization repair and preserves the upstream implementation unchanged.
All scientific sources, fixtures, build declarations, reference oracles, input
inventory and tolerances remain byte-identical to the qualified `3e06` producer.
Fresh integrated-producer qualification and final publication receipts are
recorded separately in the supported transfer evidence; historical executions
are never relabeled as executions of this newer compiler.

The original completed review was executed by distinct canonical task
`/root/independent_review`, separate from author task `/root`. It independently
materialized the exact Library packet and completed its receipt at
`2026-10-11T11:31:45.364981+00:00`, then returned its completed signoff. The same
reviewer continues the affected integrated-tree review. The original changed-tree
SHA-256 audit digest is not a Git tree object ID.

Supplementary contextual IEEE and Ring diagnostic failures and nested Float
historical timing failure are retained as historical `c778` evidence. They are
not claimed as passed or as a baseline certification for this newer compiler.
This candidate owns only application files and reuses published compiler repairs;
its required affected gates are complete application checking, native and Terminal
numeric/input matrices, exact nominal invalid rejection, true/false theorem
controls, zero-new-meaning preparation, distinct final-tree review and the normal
exact-SHA landing receipt. This follows scoped validation and main-change
validation in `tools/testing.md`; no frozen golden or threshold is changed and no
full compiler-release or performance acceptance is asserted.

## Reproduction

From this worktree, use the supported executor context and run sequentially:

```sh
source /workspace/cloud-setup/activate.sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
export CARGO_TARGET_DIR="$PWD/build/rust-target"
mbx build --locked -p omega --release -j4
build/rust-target/release/omega update --project samples/cli/independent/piston_cycle_work --offline
# Review any new policy meanings; only already authorized exact meanings may be accepted.
build/rust-target/release/omega --check --target linux_x86_64 --build-dir samples/cli/independent/piston_cycle_work/build/check samples/cli/independent/piston_cycle_work/main.omg
build/rust-target/release/omega --target linux_x86_64 --build-dir samples/cli/independent/piston_cycle_work/build/native-current samples/cli/independent/piston_cycle_work/main.omg
python3 samples/cli/independent/piston_cycle_work/validation/run_native.py samples/cli/independent/piston_cycle_work/build/native-current/omega-program build/new029-output
python3 samples/cli/independent/piston_cycle_work/validation/verify.py build/new029-output native
python3 samples/cli/independent/piston_cycle_work/validation/build_terminal.py build/rust-target/release build/terminal_capture_current METADATA_MAP_JSON
build/terminal_capture_current samples/cli/independent/piston_cycle_work/main.omg build/new029-output
python3 samples/cli/independent/piston_cycle_work/validation/verify.py build/new029-output terminal
build/rust-target/release/omega --check --build-dir build/new029-intended-invalid samples/cli/independent/piston_cycle_work/controls/intended_invalid/main.omg
```

The final command must reject for the frozen nominal-type reason.
`METADATA_MAP_JSON` is the exact dependency map in the transfer bundle; a fresh
build with unique metadata may omit it. Successful compilation reports the actual
published executable. The native runner executes only that product.
For the two overload controls use the documented focused corpus gate, with one
job and the runner built from the qualified producer. No `--record` is necessary.

Landing follows the normal main-only FIFO workflow: checkpoint, exact-SHA verify,
enqueue/claim, integrate the returned base, verify again and publish through
`tools/landing.py`. The transfer manifest records the actual outcome and remaining
blockers, rather than treating this reproduction text as proof of publication.
