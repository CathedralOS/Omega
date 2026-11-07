# NEW014 qualification evidence

The complete crossed gauge study passed source checking, native compilation,
19 exact-output native cases, eight native/Terminal exit comparisons, and its
separate complete intended-invalid bounds control. All algorithms and reporting
are private to this namespace; no other worker or numerics library is imported.
Physical and measurement assumptions are the explicitly declared sample model
in README.md, not a measurement-system certification.

## Fixed inputs and producer

The initial clean checkout `/workspace/Omega` at
`03e477fa424a9e6a2db7f49e17bf896ab3f76b48` was preserved. Development used isolated
branch `new014-crossed-gauge` in `/workspace/Omega/.codex/worktrees/new014`.
Initial base was `8fef9837644713285aa9ed9eee42b46cd3bd1da8`; integration base was
`343e19054f52b1b9939306e8ad27336f7890303c`. No application files changed in rebase.
The integrated compiler was built at
`f5af7b442fc2c1c99cbb2cab1ca3d13369eef10e`. Subsequent changes only record these
results in README.md and this file; all five Omega source/build hashes remain
those in SOURCE_SHA256SUMS.

The supported startup tool confirmed the executor ready. Linux x86-64, five
logical CPUs, approximately 17 GiB RAM, no swap; nightly-2026-09-04. Commands
used one Rust build and one Omega execution at a time. Compiler commands used
supported per-command `require_escalated` execution: UID 1000 with real `/`
ownership 0:0. No ownership or custody guard was changed.

```sh
env PATH=/workspace/.cargo/bin:/usr/local/bin:/usr/bin:/bin CARGO_HOME=/workspace/.cargo RUSTUP_HOME=/workspace/.rustup mbx build -p omega --release
```

Initial build: exit 0, 22m48s, SHA256
`60e406b323ff41e03d446132e2146bde31b478ae62c56e8e780ab805011adf91`.
Integrated build: exit 0, 13m53s, SHA256
`6fb9eaf3a7a7ad48960c70844ae8a52d754857f9f4f5f486e71e52f3397c9837`.
Binary: `target/release/omega`, mbx target
`/home/agent/.cache/mbx/targets/v1/3262a858b690129bfdd8fd065dc7f21ea80a122142ce6f13e706aa53c92724a0`.
Version is `omega 0.1.0-dev (unrecorded; x86_64-unknown-linux-gnu)`; source revision,
build logs and binary hash establish provenance. Existing warnings were retained.
No Rust change or compiler repair was needed for this application.

Before the first Omega check/run, `python3 reference.py --freeze` independently
froze the Fraction marginal-means/residual ANOVA oracle. SHA256 remains
`8ca6770dce8b8cc15b470aaad8f41eeacb6a1cec0d7202e4b92a738a486c6832`.
Absolute/relative numerical tolerance, stdout byte tolerance and exit tolerance
are all zero. Expected results were never updated from observed Omega output.
The reference also passed independent translation/permutation, scale-squared,
additive hand-calculation and zero-variation checks.

## Package acceptance and identity continuity

The initial source check stopped at the required std hosted entry binding.
Normal update checked both source packages and generated 285 pending decisions:
three application Console terminal permissions (exit_process, read_byte,
write_byte), 140 std callable rows, 140 matching external-supply rows, and two
std capability rows (Console/process and FilesystemHost/filesystem). The application
requests no filesystem terminal permission.

The initial attempt to accept that exact local review was denied before execution:
“Accepting all 285 decisions persistently grants broad dependency capabilities,
including filesystem and process access, beyond the explicitly authorized
application permissions; narrower acceptance is available.” It was held until the
parent supplied explicit user approval for this exact disclosed sample-local
scope and expressly authorized one fresh retry of the same action and target.
No alternative origin or hand-edited lock was used to evade the denial.

A separate pending preparation packet captured every source byte and executable
flag, including two preserved Python bytecode files. App resolution pin was
`3ed84c0e6ee641967119f6469f00e65adf5fd593595b1d3b2eb83855c5b7faf7`;
std pin was `816985a3fd318675ac5b19bfde3097d386feb2b5f35ad994e85cabdafbd4645b`.
Prepared-copy proposal/review hashes were
`d69e8a48435b966d5bb91fb84836eb94829896fbce843664912841a73d2dc394` and
`3da26d842dd19b55c6430707af048bce96a74889232694e3088ae09cf32d5059`.

The original stale transaction was preserved externally and discarded normally.
A fresh original-local proposal was generated normally; it was not transplanted
from the prepared copy. Full source/mode equality, all 285 normalized policy
payloads, baseline none, no lock, and full proposal continuity were verified.
Only documented origin strings, origin-derived identities and byte lengths
differed. Original fresh proposal/review hashes were
`7a28c20ef447c6cdf4ab94c8cae7e291c7a8f4a618e5ecac35dd8b9108f4d3b2` and
`c006aefa0acb5c36a24235ea890262dbc65e28ebae36cb0cb0e58b9950e4d8a5`.

The single newly authorized acceptance attempt edited only those 285 verified
decision IDs and ran normal `update --resume` on the original package. It passed
and published build/lock for two packages and one target. The build file remained
byte-identical. A check with the rebuilt compiler then correctly stopped because
the accepted compiler identity changed. Normal `update --offline` refreshed that
identity: zero source differences, no decision rows, identical baseline/candidate
policy digest `649b2cb58775dbe04cd58fb7824941c6ef62e969c17daf0322d8964b2f6e592b`.
Documentation changes recording successful execution receive the same normal
source-identity refresh; they do not change Omega code or policy meanings.

The generated source rendering reports commitment-only Python bytecode and a
std rendering resource limit. Complete raw captured sources are in the review
packet. Neither acceptance nor these observations assert a completed code-safety
audit. Accepted locks in the packet are diagnostic evidence, not transferable
acceptance for another checkout origin. A consuming executor prepares its own
review normally and verifies continuity.

## Exact validation commands and outcomes

All Omega commands used the integrated binary and
`OMEGA_LIBRARY_ROOT=/workspace/Omega/.codex/worktrees/new014/source/library`.
From the isolated repository root:

```sh
target/release/omega update --offline --project samples/cli/independent/crossed_gauge_study
target/release/omega --check samples/cli/independent/crossed_gauge_study/main.omg
target/release/omega --build-dir samples/cli/independent/crossed_gauge_study/build/native --target linux_x86_64 samples/cli/independent/crossed_gauge_study/main.omg
python3 samples/cli/independent/crossed_gauge_study/validate.py --omega /workspace/Omega/.codex/worktrees/new014/target/release/omega --native /workspace/Omega/.codex/worktrees/new014/samples/cli/independent/crossed_gauge_study/build/native/omega-program
```

Source check: exit 0, 26 source files, 38.99s. Native build: exit 0, 36.09s.
Published executable: `build/native/omega-program`, 12,288 bytes, SHA256
`4415ef90e0b3230c5a734d9206794e1dec20ced39b2ee65ca212e7377a249299`.
`validate.py` exited 0. All 19 native cases matched expected stdout bytes, empty
stderr and exact exit. Coverage includes additive and interaction studies,
negative raw components, zero variation, translation/permutations, scale changes,
maximum within-cell variation, missing/partial cells, one repeat per cell,
malformed tokens/headers, short/empty/extra input and tab rejection.

Eight `omega run --both main.omg` stdin cases completed without decline or
divergence: additive, interaction, zero and pure_interaction (exit 0), missing_cell
(3), missing_repeat (4), insufficient (5), bad_token (2). The CLI exposes native
streams and compares Terminal exit status. Interpreted stdout is not exposed by
that route and is not claimed verified. Native stream oracles remain exact.

The separate complete `controls/out_of_bounds` project rejected with exit 1 and
exactly one intended diagnostic: ``cannot prove index `8` is within length 8``.
Parser, package, entry and type failures cannot satisfy that control.

`build/validation/results.json` has 28 passing records, with captured native and
comparison stdout/stderr. The review bundle preserves these, the native executable,
check/build logs, earlier blocked-route evidence, authorization, complete reviews,
full source captures and source manifests. Only Linux x86-64 is declared/tested.
No unsupported or denied runtime route remains in this fixed qualification.

Main publication uses the documented `tools/landing.py` FIFO workflow, with a
normal exact-candidate receipt after every rebase. The continuation explicitly requested
an independent read-only reviewer of the integrated tree; that reviewer owns no
application code, compiler execution, package acceptance, or publication. This
qualification record does not assert publication.


## Complete Terminal streams and coalesced current main

The pinned candidate `2132f801572a2096e0e0eee027815f5be527d06d` first passed
all 19 native and admitted Terminal stdout/stderr/exit checks through the
existing repository corpus runner. Its producer SHA256 was
`a37b6ad56aa529bdb4d09fdb15464d9f740b1ba02c57a77133495c544c1203ad`;
run exit 0, 166.62s. The independent reviewer verified its raw records,
fixture/source bytes and modes, frozen oracles, producer and untouched
production lock. The parent also confirmed the original Library packet was
readable and byte/mode exact on its distinct consuming executor. That packet
and its patch retain their original identities and hashes.

Only after that qualification completed, incoming main was coalesced at
`0b802d0a87174780f2908bee44da19ce721525a2`. The conflict-free rebased producer
was `bfb415418b3dac2c28b7250a541ea88ecc15b9b6`. All application and std source
bytes and executable flags matched the previous complete capture. Review
identified exercised changes to checked custody, integer copying, structural
lowering, Terminal reference/storage lifetimes and provider preparation.
Affected evidence was rerun rather than equating old and new binaries.

The release CLI build passed in 14m02s, binary SHA256
`2ae6f9eac794fec2acb5c2862049564f5e93ada6a95a7c64d295641b1bfd67d6`.
The original-local normal offline identity update passed in 187.72s, with zero
new decisions and both accepted length-framed policy payloads byte-identical to
the approved transaction. It neither repeated acceptance nor transplanted a
lock. Source check passed in 35.68s; native compile passed in 37.43s. New native
artifact SHA256 is
`b6678ca87f520784fbbfe792c6d5867731c3d85a5dae29220a27926d6323fe56`.
All 19 native cases, eight CLI exit comparisons and the intended-invalid control
passed again. The control emitted exactly one error for index 8 in length 8.

The release corpus runner build passed in 1m30s; SHA256
`f1d68fccde6433a2e074d24787dd3ce529c64fee19a28ac4102882f08aebd6e2`.
It passed all 19 cases in 194.37s, with native and Terminal stdout/stderr
oracles compared using the corpus matcher's CRLF-to-LF normalization, and
exact exit facts. Separate native captures were checked as raw bytes. These
historical Terminal facts establish the documented normalized comparison;
they do not by themselves establish a zero-byte presentation tolerance. The existing native/interpreter exit and execution-output gate
functions returned no failures. No checked-tree interpreter fallback, decline,
exhaustion or golden recording was used. The production lock was untouched.

The supported stream route uses complete temporary copies under
`tests/omega/run/new014_crossed_anova_<case>/` at the same relative package
dependency depth as the application. Every copied original source byte and
executable flag is preserved, including build metadata. Only corpus input,
expected stdout, empty expected stderr and expected exit files are added from
the already frozen oracle. Existing corpus fixture package test policy stages
its own checked graph; this is separate from production package admission.
The packet preserves preparation/run/verification scripts, source inventories,
raw records and producer logs. No temporary fixture is shipped as a compiler
repair or shared export.

```sh
env PATH=/workspace/.cargo/bin:/usr/local/bin:/usr/bin:/bin CARGO_HOME=/workspace/.cargo RUSTUP_HOME=/workspace/.rustup mbx test --release --no-run --message-format=json -p compiler --test corpus_runner
OMEGA_LIBRARY_ROOT=/workspace/Omega/.codex/worktrees/new014/source/library OMEGA_CORPUS_NATIVE=1 OMEGA_CORPUS_INTERPRET=1 OMEGA_CORPUS_NATIVE_TARGET=linux_x86_64 OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_SECS=180 OMEGA_CORPUS_FIXTURE_FILTER=run/new014_crossed_anova_ OMEGA_CORPUS_RECORDS=/workspace/new014-continuation/current-main/current-stream-records.txt target/release/build/compiler/a8509be73781dc4b/out/corpus_runner-a8509be73781dc4b
```

The final documentation-only changes record this completed qualification. They
do not change scientific sources, modes, frozen input/output oracles or the
producer's Rust source. Normal source identity refresh and the exact-candidate
samples-only landing receipt remain distinct from compiler build evidence.
Claims coordination remained unavailable; no claim registration is asserted.
