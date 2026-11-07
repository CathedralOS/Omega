# NEW085 validation record

The complete synthetic voice-period application is qualified on Linux x86_64:
application check passes, 21 distinct inputs match exact native stdout and exits,
and those same 21 inputs independently match exact Terminal stdout, empty stderr
and explicit process exits without declines.
The separate positive type control passes; the intended-invalid control rejects
with exactly one Boolean-to-`i32` assignment diagnostic. Tolerance remains zero.

## Frozen science and provenance

The originally approved candidate is
`28039cf89ea21ef0206446781cec7014a4e180d5`, based on
`c24b141cb29b311e92de0a75b9609ec37e9bf28a`. Its full application resolution pin was
`6daa30b02e31a12dca5d51d9800f95d5619c2fde57e41d9e1b6741c6bd0ac5aa`; std remains
`816985a3fd318675ac5b19bfde3097d386feb2b5f35ad994e85cabdafbd4645b`.
All executable Omega sources, build metadata, controls, reference code and original
fixtures/oracles are unchanged. Only this validation document, its hash manifest and README
were updated after qualification. README clarifies that the initial sentinel
comparison can reach 300000; selected-score comparisons remain bounded by72900.
Both fit i32, and the scientific model and algorithm are unchanged. Complete captures, modes and content pins are
in the accompanying review packet; the original candidate remains preserved.

The independent Fraction oracle was frozen before execution. Its manifest remains
`91a5c1f72a4a241e26a7ab3524df2d3db6a43279bfa4a35293000cca544c1a30`.
Nine original cases cover 27 tracks, all five statuses, 70 exhaustive lag rows
(ten rows for every lag 2..8), three changes, two uncertainty spans and six input
errors. Constructed periods, harmonics, silence, within-window and between-window
changes, short/minimum/maximum windows and EOF boundaries are checked as bytes.

Twelve additional cases were frozen outside the authored package before execution:
zero/nine silence, a short constant window, periods 3/6/8, exactly sixteen records
with EOF, a blank record after output, NUL/non-ASCII input, binary amplitudes, and
uncertainty close/reopen. Supplemental manifest SHA-256 is
`88643a8b16d5350d19912ada7740b94c317491a68afc5fa5dab3502044d64fc3`.
Their complete inputs, expected outputs and exits accompany the evidence packet.

## Fixed producer and executor

Fixed qualification uses recorded main producer
`93483ae50e172fc0096c9c2f5c16732b67c09da4`, integrated in this task's isolated
`candidate/new085` worktree. No Rust/compiler/library changes were authored.
The release build exited 0 in 12m25s with existing warnings, two build jobs,
pinned nightly-2026-09-04, target Linux x86_64, four-core CPU quota and 16GiB
memory limit. One Rust build and one Omega invocation ran at a time.

Fixed producer compiler SHA-256:
`59b34748058f987518f10034c9fd4b403da9f5696b21cf67d9ce19df93f61618`.
It reports `omega 0.1.0-dev (unrecorded; x86_64-unknown-linux-gnu)`; Git tree and
build receipts establish producer provenance without relying on that version text.
The nine standalone native builds produced identical executable SHA-256
`81bd9e7c9f8b5778ddc71e12868bb1b2a852febd476c1d61059a37ea8437f039`.

The original-base binary (`7d81ddcf66ffff8ccd41e7ed0b06dd7a0a5989e8c000d2ea46dd106c48327f00`)
failed native allocation with `NoCommonCandidate { function: 4, register: 10 }`.
The complete unchanged program was then carried to freshly fetched main and
executed successfully with its new compiler. Existing landed allocation recovery
resolved the witnessed failure; no scientific workaround or new compiler repair
was needed. The old failure and both binary identities are retained.

## Package acceptance

The user explicitly approved this sample in the additional 52-sample batch.
Before the one fresh acceptance edit, complete sources/modes, proposal origin,
accepted-lock absence and every declared review field matched the approved packet.
The stale original transaction was preserved, discarded through the documented
command, and re-prepared normally at the same original project origin. No proposal
or accepted lock was transplanted from the preparation projection.

The original-project current pending review SHA-256 was
`fff7ed8549ed744494e0d37bf98dba15c89844f5cd351086cf8a68043820dbb4`;
proposal SHA-256 was
`73d738fee99a329ab6b13f8971960309260569a466e0f4e1c618eaa205ac00f2`.
Exactly 286 approved rows were accepted: callable140, external-supply140,
terminal-permission3, dangerous-capability3. Full payloads include the disclosed
std Console/process and FilesystemHost/filesystem scope and application Console
input/output/exit. No permission, credential or golden-file changes were made.

The acceptance edit passed automatic approval review. Initial resume caught this
task's reference import generating a Python cache during source capture. That
single generated cache was preserved outside the source and removed; every frozen
byte/mode was verified again, and resume succeeded with the accepted rows unchanged.
The resulting original lock SHA-256 was
`3754b987802c4c3fb0900f02dbb201a0e87ceff2f4c23c95e42be8c734bc9b01`.

Qualification on93483 required the normal compiler identity refresh. It exited
0, reported zero changed source entries, and published with zero new pending
choices. All286 row keys and complete meaning payloads remained byte-identical.
Its lock SHA-256 was
`2352a3d4a2c8da6aa4e59542bf13269afdf1ede555aa08ebdb9d5da53d883d9d`.
The final documentation-only refresh is checked separately in the review packet.

Default sandbox root ownership is synthetic and fails cache custody. Individually
approved supported-context compiler executions retain UID1000 and real root UID0.
No ownership, custody guard, OS permission or resource backing was changed.
The earlier pre-approval denial and its evidence are preserved; no alternate origin
was used to evade it. The std source renderer hits its output limit, so complete
std sources and mode-aware pins supplement the rendered review.

## Commands and observed coverage

From the candidate worktree, with matching `OMEGA_LIBRARY_ROOT=source/library`
and private `XDG_CACHE_HOME`, the public entrypoints were:

```sh
./target/release/omega update --offline --project samples/cli/independent/voice_period_tracking --target linux_x86_64
./target/release/omega update --resume --offline --project samples/cli/independent/voice_period_tracking
./target/release/omega --check --target linux_x86_64 samples/cli/independent/voice_period_tracking/main.omg
./target/release/omega run --keep samples/cli/independent/voice_period_tracking/main.omg < CASE.input
./target/release/omega run --both --keep samples/cli/independent/voice_period_tracking/main.omg < CASE.input
./target/release/omega --check --target linux_x86_64 samples/cli/independent/voice_period_tracking/controls/type_valid/main.omg
./target/release/omega --check --target linux_x86_64 samples/cli/independent/voice_period_tracking/controls/type_mismatch/main.omg
```

Thirty public run invocations cover nine standalone native runs plus nine original
and twelve supplemental native/Terminal comparisons. Every expected stdout byte
and native exit matches. Every CLI Terminal comparison explicitly matches
termination; no decline is treated as a pass. A separate public-runtime capture
then independently matches all 21 Terminal stdout and stderr oracles. All commands, actual stdout/stderr,
input hashes, timings and retained artifact identities are in the evidence packet.
The authored `reference.py --omega ABS_BINARY` and `--both` commands in README
remain the portable nine-case verifier; verification never regenerates expectations.

Fixed producer checks exit0 for the application and positive twin, and exit1 for the
negative twin with exactly one diagnostic containing the pre-authored fragment
`stores a boolean into a i32` (compiler backticks preserved in expected.txt).
Infrastructure refusal and wrong-reason rejection are not accepted controls.

## Limits and integration status

The stable CLI exposes native streams and Terminal termination agreement. Separate
Terminal streams were captured with an external Rust validation consumer using the
same public package-manager preparation/native compilation and runtime execution
APIs used by the CLI and corpus runner. It compiles the already accepted original
project once, borrows the exact retained Terminal artifact and runtime provisions,
then supplies independent private domain exit sponsorship and finite input for
each case. Every actual stdout byte, empty stderr and explicit ProcessExit result
(0 or64) matches the frozen oracle. It rejects missing provisions, exhaustion,
non-completion and mismatches. No expected-output callback or new package
authority is supplied. Full source byte/mode/pin captures and the accepted lock
remain identical before and after this execution.

The supporting consumer, cases list, exact commands, build receipt, binary hash,
and all21 status/stdout/stderr/exit captures accompany the Library evidence. Its
release build exited0 in12m30s on the same recorded Rust tree with two jobs;
binary SHA-256 is
`c0104cd8b1533df0bb92880d0d06ea9191c088e5a52cb79f89c752f01629fb7e`.
It ran once in the individually approved compiler execution context, overall
exit0. This supplies separate Terminal output evidence in addition to the native
executions, not an inference from matching termination.

Only Linux x86_64 was validated. Windows, macOS and Linux arm64 bindings were not
executed on this host. The model is explicitly bounded synthetic PCM, not evidence
of accuracy on recorded human speech.

Source invariants and independently constructed boundary cases received self-review.
One explicitly authorized read-only independent reviewer consumed the preserved
Library packet on this executor, verifying the archive, patch/bundle trees, full
source modes/pins, all286 unchanged acceptance meanings, and all21 frozen
Fraction oracles and native captures. The separate Terminal capture route and
actual outputs also received independent review. No implementation handoff or
worker dependency was introduced. Final integration follows the coordinated
main-only landing workflow and requires exact-tree independent review. No remote publication or
remote-main proof is claimed by this record. Claim coordination timed out; its
pending ticket remains unconfirmed. The starting checkout and prior work are preserved.


## Integrated qualification on588499

After completing independent fixed-revision approval, main was observed once at
`588499422fe30e3ac5cf42ebd346bf435b6aebbe`. The coalesced delta changed exercised
checking, structural/reference lowering, integer selection, provider dispatch and
custody, native object/image emission, and Terminal reference operations. Independent
impact review required fresh scoped checks. Std admitted sources, application
scientific sources, controls and every frozen oracle remained unchanged; a full
baseline was not required for this sample-only authored change.

A fresh release compiler on Rust tree
`d5fdccac7c534f89eaa598b43b3ba346e65e4c95` built successfully in11m14s, two jobs,
same pinned toolchain/target/resources. CLI SHA-256:
`cf4a0359b004dd51686268e497c2f8c6d7ef658ee2c509138d089a0c117162da`.
Normal original-origin identity/source refresh exited0 with zero new choices.
All286 category/key/complete meaning payloads exactly match the original accepted
lock. The integrated lock SHA-256 before this final documentation update was
`e313d87e2b8ee43d9026f8f2854df8249e17e4d95d207aa6292c122fd6bed3a3`.
The final metadata-only refresh is retained separately with the same scope checks.

Application and positive controls check successfully. The complete negative twin
checks with exit1 and exactly one authored Boolean-to-i32 diagnostic. Ordinary
CLI native production at the accepted original origin emitted retained executable
SHA-256
`3772eb167b28399a74923d12753dd33cae66a8e091c9ebe047bb681e543ede8b`.
That exact executable then independently passed all21 frozen stdout, empty stderr
and process-exit oracles using sequential finite inputs.

The unchanged supporting Terminal consumer was rebuilt normally with Cargo on
this integrated producer in11m15s. Binary SHA-256:
`90488d49d45ba87137df2b301f37f7a8b66107b7e03dcf0e6eeb30773b51e988`.
A direct-link optimization was tried with exact Cargo dependency fingerprints;
the pinned toolchain rejected metadata-stub rlibs and the corresponding split
metadata invocation. Both failed attempts are preserved. Neither supplied runtime
evidence or modified compiler inputs; the successful normal Cargo consumer is
the one executed. Its ordinary accepted-origin compilation retained the exact
Terminal artifact and provisions, and all21 separate actual stdout, empty stderr
and explicit ProcessExit results matched the immutable oracles, overall exit0.
Full source byte/mode/pin captures and the accepted lock are unchanged before and
after execution. This closes integrated Terminal output coverage independently
of native output and termination agreement.

The supporting evidence includes exact argv/cwd/environment, actual outputs and
exits, compiler and artifact identities, source captures, all original failures,
independent impact/native/Terminal review, and exact-candidate landing receipt.
Only final validation prose and its hash differ from the executed application
inputs; no executable source, fixture, oracle or accepted meaning changed.

## Integrated qualification on3b5282

After exact738d8707 qualification and verified Library consumption, main was
observed once at `3b5282df452f7a15b37daafddcdd1e3213d2ea91`. Independent
impact review identified exercised frontend checking/reference access and native
primitive-access publication changes. The unchanged application was rebased
without conflicts; fresh scoped qualification ran outside the landing queue.
No executable source, oracle, provider or shared export was authored or changed.

On Rust tree `48d1ff04af7cfa9d0ea1ba071ab76442b4a108fb`, normal Cargo
release CLI build passed in10m33s and the separate supporting consumer build
passed in10m28s, both with two jobs and the same pinned toolchain/resources.
CLI SHA-256 is
`7d537ee0828be35aa7cbca2570a2292339c56d1c315e7e90cfc3cfb2bb128328`;
consumer SHA-256 is
`31cc5cf79c5eae405323d78873a4dccc84eab2c3983963c9e054a5bf9d89b168`.
Normal original-origin identity refresh passed, with zero changed source entries
and zero new choices. All286 complete category/key/meaning records remained
identical to the original accepted lock; pre-metadata lock SHA-256 is
`1947fa41b687cea9552ac10000272836d60cb5846da6bfb040edd4fc4aff7829`.

Fresh application and positive type checks passed. The complete negative control
rejected with exactly one intended Boolean-to-i32 diagnostic. Ordinary native
production emitted SHA-256
`3772eb167b28399a74923d12753dd33cae66a8e091c9ebe047bb681e543ede8b`;
all21 finite inputs matched exact stdout, empty stderr and exits0/64. The fresh
consumer independently captured all21 Terminal stdout/stderr/status/exit results
with actual ProcessExit completion, all matching the unchanged zero-tolerance
oracles. Its overall execution exited0. Full source bytes/modes/pins and the
accepted lock remained unchanged before and after both execution routes.

Complete exact commands, build and runtime receipts, separate outputs and exits,
source captures, preserved prior Library packet and independent review accompany
the final packet. Final prose/hash metadata are refreshed separately with the
same286 unchanged meanings. Publication and remote-main proof remain a separate
receipt, so this qualification record does not imply a remote publication.

## Integrated qualification on12953

The verified version1 Library packet preserved exact ec36ed3c qualification on
3b5282. Normal FIFO enqueue was rejected by GitHub GH007 because the local
commit identity used a private email. Status confirmed that ticket absent; no
publication occurred. Connected GitHub identity and existing public-main authors
verified the owner's noreply address. Only this task's commit identity and future
per-command queue identity were corrected; its source tree was unchanged.
No privacy setting, global Git configuration or package acceptance was changed.

A subsequent fixed observation at `12953abbc60fd1ac37ddc79b3d9aa0a804ba522a`
contained changes to exercised reference-shape construction and rooted operator
admission. Independent impact review required fresh scoped qualification.
Normal Cargo release CLI and consumer builds on Rust tree
`37bb8120d73ec87e3d1de3a27f865e15638b6d1c` passed in9m37s and9m29s,
respectively, with the same two jobs, pinned toolchain and executor resources.
CLI SHA-256 is
`605aacbe357197f62b61d5a47a39924aa8e1eec89bc118e8ced0db2a14b014b2`;
consumer SHA-256 is
`c9e37a0e8229f73057341a7bd1f6ecc58a1035879030d0e36f333390bca01e43`.
Offline Cargo updated the supporting consumer lock to include the existing local
calling-conventions dependency declared by incoming native-production; actual
consumer source, manifests and lock accompany the packet. Consumer Rust code,
direct dependencies, cases and all scientific fixtures/oracles remain unchanged.

Original-origin refresh passed with zero changed entries and no new choices;
all286 complete accepted meanings remained identical to the original lock.
Application and positive checks passed; the complete invalid twin rejected with
exactly one intended Boolean-to-i32 diagnostic. Fresh ordinary native production
emitted the same executable SHA-2563772eb167b28399a74923d12753dd33cae66a8e091c9ebe047bb681e543ede8b.
All21 native stdout/empty-stderr/exits and all21 independently captured Terminal
stdout/empty-stderr/ProcessExit results matched the frozen zero-tolerance oracles.
Source bytes/modes/pins and accepted lock were identical before and after both
execution routes. Final prose/hash identity refresh and exact-candidate review
are retained separately. No remote publication is implied by this record.

## Final fixed qualification onb1628233

Corrected-identity FIFO enqueue succeeded. The granted base was
`b16282336249cec2ed3df1edb3de22f641682bb6`; independent impact review
identified exercised mutable-record entry eligibility and reference-forwarding
borrow handling changes. The claim was released normally before qualification;
no long build or waived check consumed its lease. Prior exact9f33b25 and verified
Library version2 evidence remain preserved.

Fresh normal Cargo release CLI/consumer builds on Rust tree
`34c7d63475f92b337c50b22653996fab6d9c7656` passed in10m05s/10m02s,
same pinned toolchain, two jobs, target and resources. CLI SHA-256:
`3872d387112bfa209fc3fcc249bece1685f559dff18858b1b7d9c6a6aa08223e`.
Consumer SHA-256:
`72ec83849dc549f3dac6a48a62bf5d2c8437a29f78d0a207c0715b8b1b29777b`.
Original-origin refresh passed with no source changes or new choices; all286
complete original accepted meanings remained exact. All3 source controls and
all21 actual native stdout/empty-stderr/exits passed, using freshly produced
executable3772eb167b28399a74923d12753dd33cae66a8e091c9ebe047bb681e543ede8b.
The fresh consumer independently passed all21 actual Terminal stdout/empty-stderr
and explicit ProcessExit0/64 results, overall exit0. Before/after source bytes,
modes, full pins and accepted lock are identical. No scientific source, oracle
or shared export changed. Final prose/hash refresh, exact-SHA receipt, independent
approval and consuming Library verification are recorded separately. Publication
proof is separate and is not implied by these qualification results.

## Fresh qualification ona07ef172

The preceding waiting FIFO ticket was canceled normally after independent impact
review found changes to ordinary integer computation and argument occurrence
handling. Fresh qualification ran outside the queue on execution candidate
`9eaa11a0b745fce6b844f15cbb928e316650180b`, base
`a07ef1729d8644da3c13b358fe61a402132de3bb`, Rust tree
`b1442654cf74f72763692a771b0c1122ddba015d`. Normal Cargo release CLI and offline supporting
consumer builds passed in 682.486s and 676.580s,
respectively, with the same pinned toolchain, two jobs and recorded resources.
CLI SHA-256: `5abddd1dd7d201fb7e47869a6e1bfc9c6694e2a29be0e56f6f37017697727c43`.
Consumer SHA-256: `bfc726926ac926d4720afbec03669adfb5acfb7e0bcf8cb52aa5f2f904bac702`.
Fresh ordinary native program SHA-256: `3772eb167b28399a74923d12753dd33cae66a8e091c9ebe047bb681e543ede8b`.

Original-origin update passed with zero changed source entries and no new choices.
All 286 complete original accepted meanings remain identical. Application and
positive control checks passed; the complete invalid control produced exactly
one intended Boolean-to-i32 diagnostic. All 21 native captures and all 21 actual
Terminal stdout, empty stderr, explicit ProcessExit statuses and exit codes match
the frozen original and supplemental oracles at zero tolerance. Before/after
source bytes, modes, full pins and accepted lock are identical. The supporting
consumer source, actual manifests/lock, commands, receipts and captures accompany
the Library packet. No scientific source, oracle or shared export changed.
Final metadata refresh and integration review are retained separately. Later
integration may reuse this recorded producer only when independently assessed
governing inputs remain unchanged; it does not qualify an untested newer compiler.
Publication proof is separate.
