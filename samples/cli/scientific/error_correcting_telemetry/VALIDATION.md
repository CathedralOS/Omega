# NEW084 validation evidence

Executor: Linux x86_64, UID 1000 in approved real-UID compiler context,
5 CPUs, 17 GiB RAM, no swap. One Rust build and one Omega command at a time.
Starting checkout `/workspace/Omega`, branch `work`, HEAD
`03e477fa424a9e6a2db7f49e17bf896ab3f76b48`, preserved without edits.
Private candidate branch `new084-telemetry`, worktree
`/workspace/Omega/.codex/worktrees/new084`, fetched-main base
`c24b141cb29b311e92de0a75b9609ec37e9bf28a`.

Compiler: current base built with:

```sh
source /workspace/cloud-setup/activate.sh
CARGO_TARGET_DIR=/workspace/new084-target CARGO_BUILD_JOBS=4 mbx build -p omega
```

Exit 0, 77 seconds. No Rust or shared library changes. Binary:
`/workspace/new084-target/debug/omega`, SHA256
`f63973aaab44ee895d39c2ae1097a1a36aa2c22ab3603d068b3733692117fd76`.
The available release binary was not used because its source revision was
unrecorded. Library selection is explicitly the candidate's `source/library`.
Target: existing `linux_x86_64` entry and Console providers from std.

Oracle frozen before the first compiler invocation, 2026-10-11 UTC:
`python3 samples/cli/independent/error_correcting_telemetry/reference.py` exited
0; its SHA256 was
`2f3c753c52f26848a15c2f0beb3acedbc16b50f318df86e37eef5d4dfec1ccb4`.
`data/oracle.json` records all exact fixture hashes and counts. All numerical
and output tolerances are zero. Reference interpolation proves 25 codewords,
minimum distance 3, parity rows and 675 within-radius transmissions.

Initial default-sandbox check stopped before source checking:
`git cache entry '/' is invalid: cache custody ancestry is owned by an unrelated user`.
The sandbox reports root as UID 65534. The supported per-command
`require_escalated` context was approved and cleared that obstacle without
changing ownership or custody guards. First source check rejected authored
use-after-transfer in `syndromes`; read-only frame use was corrected to borrowing.

Claim coordination: an off-board NEW084 claim timed out with uncertain remote
mutation, ticket `0645c667c83642c9b630ab2a34767d6e`. A later read-only status also
timed out. Registration is unconfirmed; isolated scope is retained. No landing
reservation or remote worker branch was created. Candidate is for independent
review, not publication.

Recovery verified on the same executor after a disconnect notification: shell
commands succeeded in the original worktree at the same base, all ten frozen
artifact hashes matched, and existing session handles remained usable. Polling
review session 64999 returned exit 3, two checked source packages, and an intact
pending review; optimized-build session 36534 continued and finished. No
replacement executor, filesystem permission mutation or custody change occurred.

Concrete package preparation:

```sh
export OMEGA_LIBRARY_ROOT=/workspace/Omega/.codex/worktrees/new084/source/library
/workspace/new084-target/debug/omega update --project samples/cli/independent/error_correcting_telemetry --target linux_x86_64
```

Exit 3, review required. Package keys:
`error_correcting_telemetry` =
`708da1b9c905877c6db6176686157079fed015fb40e2b424d6ce51c41b0a6e05`;
`omega_language_std` =
`86f9729da9717d453e53a72069a1c533124d02d91d80e1ad2e86b7a0122e63b7`.
Review candidate `670c4c5a81b0710833e36de29a3a7fec0e9b725edd4d3c8ab2cf44efecd9f591`;
comparison `06e36dff05556841f11144175c0bc602763e7b5214182c8955a2f024ca4abd1f`.
Review SHA256 `d8797c2d42de35d81acfaff0db0ac9b33f4f5508f1a54cc6360b26988415c96a`.
All 285 rows remain pending: 3 terminal permissions, 140 callable rows,
140 external supplies, and 2 dangerous-capability rows (process Console and
filesystem FilesystemHost). The source rendering limit prevented source-code
output for both packages. The handoff therefore includes exact reviewed app
sources and the tested-base std source archive for standalone audit. Review
identities are scoped to their generating source and paths, not transferable
approval. Documentation added after observation can require a fresh review.

Observed route commands, with the same library selection and binary:

```sh
/workspace/new084-target/debug/omega --check samples/cli/independent/error_correcting_telemetry/main.omg
/workspace/new084-target/debug/omega run samples/cli/independent/error_correcting_telemetry/main.omg < samples/cli/independent/error_correcting_telemetry/data/exhaustive.stdin
/workspace/new084-target/debug/omega run --both samples/cli/independent/error_correcting_telemetry/main.omg < samples/cli/independent/error_correcting_telemetry/data/exhaustive.stdin
/workspace/new084-target/debug/omega --check samples/cli/independent/error_correcting_telemetry/controls/out_of_bounds/main.omg
```

| Route | Exit | Observation |
| --- | --- | --- |
| Main check | 1 | std LinuxX86_64Application needs an accepted program-entry binding |
| Native | 200 | Same binding gate; no program output or execution |
| Terminal comparison | 200 | Same compilation gate; neither native nor Terminal ran |
| Intended-invalid bounds | 1 | Exact required `cannot prove index 4 is within length 4` diagnostic (with compiler backticks) |

The invalid control passes its rejection oracle. The reference/exhaustive finite
model checks pass. The application has **not** passed check/native/Terminal;
fixtures and reference calculations do not substitute for those routes.
The recovery instruction prohibited package acceptance; no acceptance was
attempted and no automatic-review rejection is claimed. A subsequent instruction
requires landing; explicit clarification about that earlier prohibition is
pending. There is no witnessed compiler gap requiring repair.

A second, serial Rust build used the documented optimized dev profile for later
checks, while the single Omega operation continued:

```sh
CARGO_TARGET_DIR=/workspace/new084-opt-target CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p omega
```

Exit 0, 7m31s, same untouched main source base. Binary
`/workspace/new084-opt-target/debug/omega`, SHA256
`a3c8560717f939e68925c50bbd26bf1e7f6dd9cdedeca95c57abad9385d90100`.
This binary has not been used to claim application acceptance.

`build/validation/blocked-results.json` retains commands, exits, and actual
main-route stderr/stdout files; the control stderr was transcribed exactly from
tool session 84890. The handoff evidence directory carries these files, both
compiler build logs, untouched review, row summary, and source-rendering warning.
Independent review completed against candidate
`afe285d35d61ca292069447b556067180b18aa31`: no blocking source/model defect,
all committed frozen blobs and finite cases independently confirmed. The report
is retained in the transferable evidence bundle. Its P2 finding is resolved by
explicitly labeling the public CLI route as native stdout plus Terminal exit
agreement, with Terminal byte equivalence still unverified. Its P3 wording
clarifies eighth-slot LF versus buffer exhaustion. These changes alter only
documentation and validation labels; application semantics and frozen artifacts
are unchanged.

The candidate still needs authorized package settlement, matching runtime
oracles, integrated validation and normal queue publication. Fetched main
`6121c00e4546376e663d396e443a4f20ee4d77f4` includes compiler changes; the existing
binaries and route observations belong to the tested base above. No rebase or
landing reservation is made while package settlement is explicitly paused.
Current-main corpus facilities expose interpreter stream oracles, but those
fixture admission facilities do not establish ordinary CLI package admission
and are not used to bypass the paused package review.


## Continuation after normal-review clarification

The same worktree was integrated onto main
`9fa24a5de5dab0498097bf2000eb6031754c09fd`. No Omega application source or frozen
oracle changed. A matching optimized dev compiler build passed in 6m59s, using
the same serial build command/target directory and two Cargo jobs. Its binary
SHA256 is `09226dfa70bb73e485ad15db77b2f2477ca4178389687e1ccbdf29b72af3d4be`.
The independent reviewer confirmed ordinary project acceptance is distinct from
a filesystem runtime grant: `dangerous_capability` is declaration-review evidence,
while dependency/Build occurrences do not confer root terminal authority.
Current review contains exactly three root Console permissions, 140 std callable
rows, 140 external-supply rows and two dangerous-capability disclosures. No
FilesystemHost root permission, restricted-build request or host permission
change was proposed. Exact current review SHA256:
`359f3b2af66030e1f56f6198b5f1773df6b0f51aa86294355df497eea9248087`.

The supported `omega update --resume --project
samples/cli/independent/error_correcting_telemetry` action was nevertheless
**rejected by automatic approval review before process creation**. The stated
reason was persistent acceptance of all 285 decisions, including dangerous
capability/external-supply rows, and its interpretation that retained instructions
prohibit package acceptance and require capability approvals to remain pending.
No lock was published; all 285 decision tokens were restored to pending. The
denied action was not retried and no alternate admission route was used.
Explicit exact-scope authorization, including a new attempt of this denied
action, is requested and remains pending.

Non-accepting observations with the matching compiler were recorded serially:
main check exits1 at the accepted-entry-binding gate; native and `run --both`
exit200 at the same gate, with no application output; the complete bounds
control exits1 specifically for ``cannot prove index `4` is within length 4``.
Retained evidence includes exact commands, actual stdout/stderr and durations,
the untouched review, proposed decisions, restored pending review and the
automatic-review rejection. Claims status again reports coordination unavailable;
the original uncertain claim is not a successful registration. No landing ticket
or remote publication was attempted.

`validate_terminal.py` now supplies a private host over ordinary package
preparation and the public Terminal runtime APIs. It checks all five original
frozen streams against one genuine retained artifact, including explicit typed
process exits and captured stdout/stderr. Independent review tightened its
process-exit requirement and source/build-receipt guards. Host build success is
validation-equipment evidence only: package acceptance and actual Terminal
stream agreement are still required. No host execution is used to bypass the
denied settlement. See the transferable evidence for the actual build receipt.


## Renewed original-origin qualification (2026-10-11)

The exact original transaction was reverified against candidate
`d468ff982a33720204468c1413c9bcf548374762`: complete bytes, physical/canonical
modes, app/std content pins, all 285 policy bodies and 12,012 typed fields,
original proposal identity and absent accepted lock matched preserved evidence.
Renewed user approval covered the previously denied disclosed batch. The retained
original invocation was admitted but exited 64 because `--resume` disallows
`--target`. The documented resume form on the same original proposal then
completed successfully (exit 0, 104.75 seconds), publishing two packages for
one Linux x86_64 target. Accepted lock SHA256:
`847d9948ca6e60160c957ede32cf83eac6be1a161411a9c2882a89780283fa32`.
No source, target or decision meaning changed during settlement.

The matching baseline compiler passed `validate.py`: check, five exact native
streams with statuses 0/0/0/3/3, focused public Terminal exit agreement, and the
complete invalid bounds control rejected for its required diagnostic. The
verified private Terminal host passed all five streams with exact stdout, empty
stderr, explicit process-exit completion and an unchanged genuine artifact.
These are witnessed execution results, superseding the historical admission
blocker above. Frozen model, data fixtures, reference and tolerance remain intact.

Current main was integrated without conflicts. Its compiler changes require
a new matching binary and affected validation; the final qualification receipt
and full captures are retained in the transferable Library packet. The entrypoint
now has the current sample-discovery expected-exit comment; this does not change
its executable semantics. Original denied and admitted transactions remain in
the packet, including the argument-syntax distinction.
