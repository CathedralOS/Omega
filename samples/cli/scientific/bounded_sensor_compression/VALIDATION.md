# Validation evidence

The complete Omega application passed source checking, ordinary native production,
44 exact native stdin/stdout/exit cases, and 44 exact admitted Terminal cases.
The separate intended-invalid source rejected for its documented bounds rule.
The scientific model, executable Omega statements and frozen fixture bytes match
approved Git candidate `d0f93df4687d3af33ab9898d88f9fe5f1fe050aa`.
Only two non-executable EOF oracle header comments were added for the sample gate.

## Recorded qualification

Compiler producer: `90552855a28050d9b7e81ba7b6f325854ce8a929`.
Main base: `0e7af1e5722c8ff56db5a5c1824610ce99c444b2`.
Compiler SHA-256: `5bf1bec88f8d5739ed5ff7725b522048833c107fc183d18c016c73b4745193f2`.
Build: `source /workspace/cloud-setup/activate.sh; mbx build -p omega --release`,
exit 0, 8m19s, four Rust jobs. Artifact JSON receipt was collected using the same
command with `--message-format=json` after the build completed; no dependency
recompilation was needed. Version reports `omega 0.1.0-dev (unrecorded;
x86_64-unknown-linux-gnu)`; producer attribution comes from the recorded build.
Linux x86_64, UID 1000, kernel 6.18.44, five CPUs, 17 GiB RAM, no swap;
Rust nightly-2026-09-04; target `linux_x86_64`.
One Rust build and one Omega operation at a time. Compiler commands used the
approved per-command real-custody context; no ownership or custody guard changed.

## Frozen oracle

Frozen before any Omega check or execution; all expectations are exact integers,
bytes and process exits, with no floating epsilon or adaptive tolerance.

- cases.json: `4c49491c494e53104cc4ec5de87a33596aac53037f34ed30e00af3510e7ffe54`
- oracle.json: `68e84f2f3b9f49a113ae3af438847ab649214402ead2323afeb7414a9aaa5595`
- FIXTURE_SHA256.json: `32e493dd4920b1d52f5bc65be5fe7c1b30f090375bcbc71058550c758ccca2d1`

11 compression and 11 corresponding decode cases,12 malformed streams and 10
invalid runtime inputs. Independent reference checked 32768 singleton combinations.
The reference is supplemental; the complete Omega entrypoint produced runtime
bytes, reconstructions, sizes and error certificates for all 44 cases.

## Exact policy authority

The earlier automatic approval denial is preserved in the Library history packet;
it was not bypassed. The user subsequently approved the exact NEW092 scope in
Library packet `libfile_05d5bb7955e48191b4b758ec22dc22c3` (archive SHA-256
`bbcaf31270c2a50474a3db88d31a0678b20e3e606a64b03b2a8156823a568b29`).
The single fresh retry on the same original target succeeded:286 decisions,
140 callable declarations, 140 external supplies, 3 terminal permissions and 3
dangerous capability declarations. Actual accepted lock SHA-256:
`4080df55237a210196e5a37604f630ba7aae760bfbe2d9fd9ef0c53344a650ad`.
Subsequent normal compiler/header identity refreshes published successfully with
zero new decision rows. Accepted locks are executor-local evidence, not portable
policy authorization or a transplantable review for a different package origin.

## Commands and results

From the recorded worktree `/workspace/Omega/.codex/worktrees/new092`:

```sh
source /workspace/cloud-setup/activate.sh
./target/release/omega update --project samples/cli/independent/bounded_sensor_compression --target linux_x86_64
./target/release/omega --check --target linux_x86_64 samples/cli/independent/bounded_sensor_compression/main.omg
./target/release/omega --target linux_x86_64 --build-dir samples/cli/independent/bounded_sensor_compression/build/qualified-cli --report-file /workspace/review-artifacts/new092-approved-completion/native-report.txt samples/cli/independent/bounded_sensor_compression/main.omg
python3 -B samples/cli/independent/bounded_sensor_compression/validate.py --binary samples/cli/independent/bounded_sensor_compression/build/qualified-shared-product/omega-program --output /workspace/review-artifacts/new092-approved-completion/native-results.json
python3 -B samples/cli/independent/bounded_sensor_compression/validate.py --omega target/release/omega --both --case smooth --case delta_overflow --output /workspace/review-artifacts/new092-approved-completion/cli-both-results.json
python3 -B samples/cli/independent/bounded_sensor_compression/validate_invalid.py --omega target/release/omega --output /workspace/review-artifacts/new092-approved-completion/invalid-results.json
```

All commands exit 0 (invalid compiler exits1; its correct-reason validator exits0).
Public `--both` agreed on smooth exit 0 and malformed delta_overflow exit 12;
its comparison covers termination, with native stdout separately checked.

The evidence packet also contains an independently reviewed Rust validation
consumer using existing public preparation/native-production/runtime facilities.
It borrows the exact retained Terminal artifact, runs all 44 finite inputs with
fresh instances, captures stdout/stderr/exit, then publishes the same report's
native image. It rejects existing output directories and writes completion only
after all 44 executions and native publication succeed. Its Python checker
requires all frozen stdout/exit oracles and empty stderr. Native validation uses
that exact recorded image. This is a validation consumer, never a replacement
implementation of the Omega application. Build/source/dependency hashes and
successful capture receipts accompany the output files.

Native image SHA-256: `105b3a626886068bb84774ab90aa753c815727e78a72e776098772926bad1b15`.
Terminal artifact SHA-256: `b49d7229e0d29c66c394661fd2d719f5b163dfcd5c314a9ef93ab7a8bc56a3a1`.
Terminal: 44/44 exact; native: 44/44 exact.
Invalid rejection: cannot prove index 16 is within length 16; the actual
diagnostic backtick-quotes the index.
No compiler implementation change or compiler defect is claimed. A validation
consumer authoring error (publication consumed the retained artifact too early)
was corrected and its failed run retained, never counted as acceptance.

## Review and integration

The explicitly authorized GPT-6.1 Sol Medium reviewer independently checked all 91
fixture hashes, regenerated 90 fixture/manifests and verified 32768 reference cases.
It found and verified fixes for `--both` aggregation and stale Terminal captures.
No application algorithm finding remains. Main integration follows the recorded
change-scoped policy: changed exercised compiler inputs receive focused reruns;
unaffected evidence remains tied to its actual producer. Exact final integration,
independent review, landing receipt, remote proof and Library round-trip identities
are recorded in the final evidence packet, rather than claimed before publication.
Claims coordination timed out; no successful assignment registration is asserted.
