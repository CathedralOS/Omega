# Validation and provenance

Acceptance uses `validate.py`: source check; 13 domain cases with every numeric
metric checked inside Omega during native and Terminal execution; 20 independent
runtime input cases with numeric stdout and exact decisions/exits; and a separate
intended-invalid source. `build/validation/runs.json` records commands, exits and
stream hashes. The Library bundle retains the raw outputs and observed verdicts.

```sh
source /workspace/cloud-setup/activate.sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
./target/release/omega update --project samples/cli/independent/gravimetric_mixture --target linux_x86_64
python3 -B samples/cli/independent/gravimetric_mixture/validate.py --omega "$PWD/target/release/omega"
```

The expected JSON was frozen before the first Omega execution. SHA-256:
`1b05b4e97cb8a3ee5433e60d481a56241c806a269d8d4fef5888e71d49fd453a`.
The reference uses exact raw-observation sensitivities and 70-digit Decimal square
roots. Tolerances remain one displayed unit. Production `main.omg` and
`domain/mixture.omg` remain unchanged. `verify_domain.omg` is reproducibly rendered
from that checksum-verified oracle and checks unrounded scaled values within the
same tolerances. The production entry reads input and emits computed evidence.

Qualification compiler base: `547c909c6230b6491cbbe4a4c748042a0007896b`.
The final candidate uses current main's compiler with no additional Rust changes.
Build command: `mbx build -p omega --release`, with
`CARGO_UNSTABLE_CHECKSUM_FRESHNESS=true` and `CARGO_BUILD_FINGERPRINT=content`.
Binary SHA-256: `85908d0f5d78b238d8843f6af6c7a1ca1b918c1c190139bbd16a5bd4604907c0`.
Host: Linux x86_64, nightly-2026-09-04, release profile, four Rust build jobs,
18 GB executor. One Rust build and one Omega execution at a time.
Target resource:
`/workspace/cloud-setup/cache/mbx/targets/v1/c5dadb726e94fee1745b97dcc4112ab107aabd8c8d83603a4b81765c83cc33b0/release`.
The exact publication candidate, integration base, build receipt and validation
commands are recorded in the bundle manifest; this compiler base names the
measured inputs rather than a moving remote branch.

Unmodified main `6d07b516ba6ce31f77b0e0bbdf7b03717d2f5e54` initially reproduced
the complete application's `NoCommonCandidate { function: 8, register: 9 }`.
Diagnostic evidence showed ten RAX-fixed dividend uses interleaved with points
excluding RAX. The original qualified candidate `c8991ca34d7ff858e40fce43203c19926a86ec2b`
used a bounded stage-03 fixed-input copy repair and passed all application checks.
That checkpoint and its complete evidence remain in Library version 1.

Before landing, main supplied admitted value-local recovery in
`239252d789da6dbd4231a467dc44a95b6309b457` and
`41a34ff1d70887e8c92833bbcb6402c36cdff657`, followed by equivalent fixed-division
input copies in `bf84ee005dacce1724200d30784ffa45b3660122`. Recovery rewrites the failed value, supplies
point-local reloads for fixed uses and recomputes strict allocation facts. Existing
alias separation remains. Independent review found this sufficient in mechanism;
complete application and expanded regression execution establish its sufficiency
for this outcome. The final candidate therefore omits the duplicate stage-03
repair. No guard, proof, source origin, fuel or numerical oracle is weakened.

Main's ordinary repeated/aliased division fixture and its Arm bindings are
preserved. The distinct `runtime_fixed_input_policy_reuse_exit` regression adds
signed MIN/-1 policies, full-width unsigned inputs, trapping reuse and reused
unsigned products, with exact quotient/remainder checks. It targets x86_64.
Existing main corpus rows are preserved; only genuinely new observations are added. Their actual timing,
revision and resources are retained in the focused verdict JSON.

GPT-6.1 Sol with medium reasoning independently reviewed the original candidate,
frozen model/oracle, reference, renderer, private source, compiler repair and raw
validation records. Fresh reviewer-side Library materialization verified the
exact version-1 identity, bundle hash, 131 files, 41 git blobs, patch and 23 stream
hashes. Its existing-row timing policy finding was resolved during integration.
Final identity/diff review and integrated validation records accompany the bundle.

Only Linux x86_64 execution is validated. Application Windows x86_64 and macOS
arm64 bindings remain unexecuted. A supplemental AArch64 regression check exposed
`UnsupportedEarlyClobber`; its complete source and diagnostic are retained without
claiming a defect attribution or repair. Initial float-validation forms omitted
`float_operations`; the authoring error was fixed, with no Psi repair claimed.

Individually approved real-ownership compiler execution was needed because the
default sandbox presented synthetic root ownership rejected by cache custody.
No ownership, custody guard or host permission was changed. The bounded std
project admission was explicitly approved; compiler identity refreshes preserve
that consent. Actual denials were respected.

Claim: `5e007f8a70104299a6ab606faea78182`, IND20. No active-worker dependency,
remote worker branch or in-progress numerical package exists. Publication follows
the documented main-only landing queue after independent review and validation.
