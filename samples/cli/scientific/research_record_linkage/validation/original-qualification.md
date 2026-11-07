# NEW064 qualification evidence

The complete Omega application passed current-main qualification. Separate
independent reviews, exact final-tree approval, landing receipt and remote proof
are identity-bound artifacts in the final Library packet. Earlier-stage evidence
below is retained with its actual producer provenance.

## Recorded inputs and executor

Original frozen candidate `766b6d27ec54c6f69e07372c32ae3d6ade04a62f`, original
base `3e977af871e2dd7278f6b59c1900aaddb6f4a968`. Rebased onto main
`a4bd6c662c998ba13394c5889597a4ab2707fe0e7d`; compiler producer
`1017e30b0153c3333250dbc65d9b5fd38cce0e7d`. Final source hashes are in
SOURCE_SHA256.txt; exact candidate SHA, patch and landing receipt accompany
Library evidence. Golden conflicts preserved upstream rows and only this
candidate's previously recorded new rows. No golden recording occurred during
renewed qualification.

Linux x86-64, real compiler UID 1000, 17 GiB RAM, initially approximately
15 GiB available, 30 GiB free disk, no swap, five effective CPUs. Pinned Rust
nightly-2026-09-04, CARGO_BUILD_JOBS=4. Environment:
`source /workspace/cloud-setup/activate.sh`. Worktree:
`/workspace/Omega/.codex/worktrees/new064`, branch `samples/new064`.
Release compiler `target/release/omega` SHA-256:
`e4c8caa12c13dd4480415ac5c0a97e47933a25d27dd15a80391ede471e436fdf`.
Managed target:
`/workspace/cloud-setup/cache/mbx/targets/v1/57e1b5d53e5d4105b95c7b911f830906890549ea4bbffe4f368b336ae94ffe0e`.
Current-main build `mbx build -p omega --release` passed in 7m35s.

Default sandbox root ownership was synthetic and failed cache custody.
Per-command approved compiler execution retained UID 1000 and actual root
ownership. No ownership or custody guard changed. No additional workers or
numerics imports. One premature regression process overlapped formatter
preparation; it was immediately stopped, exit 143, before a verdict and is not
validation evidence. All subsequent compiler/executor operations were sequential.

## Frozen independent scientific oracle

All comparisons use exact integers: absolute and relative tolerances zero.
Original 29 cases were frozen before any Omega execution:

```text
a17fbadfb6e0bfda23b827e9aa3a30c49c43d7fefd403fa9e63cadaf1c009a51  reference.py
59f3b4882bc761a86cda3667f728be23458c3556ca70a1cd450838e720dc2c7c  data/frozen.json
```

Additional greedy challenge frozen before application execution:
`data/greedy_trap_frozen.json` SHA-256
`e75c6ca35165ccc83b5d41906860123364aee280669e5ecf6e86f23041a487ac`.
Rows `[9,7,0]`, `[10,6,0]`: greedy scores 15, unique optimum 17 with choices
`[2,1,0]`. Reference enumerates subsets/permutations independently of Omega's
base-four assignments. Model, reference, fixtures and expected results unchanged.

## Authoring and bounded compiler repair

A 15m07s original preparation stalled; current-main complete-source witness
also timed out at 90s. GDB showed repeated recursive expression analysis via
ordered increment queries rebuilding carrier meanings without relations.
The complete bounded unsigned-sum witness took 40.4s before its ordinary policy
diagnostic. Bounded repair prefers independent carrier proofs before relational
increment proofs, skipping positive-gap searches when no ordered relations exist.
Zero increases preserve operand eligibility; unsafe nested operands still reject.
The 15 focused regression cases retain seven checks and eight intended refusals.
No arithmetic or scientific guard was weakened.

Source checking then exposed ordinary authoring issues: implicit read state
collision and missing Console invocation/block/Trap contracts, corrected before
qualification. Native lowering later rejected non-tail recursion in the decimal
formatter. Documented runtime eligibility (wiki/spec/language/termination.md)
requires tail cycles. Iterative divisor growth/digit emission/shrinking now
implements exact decimal output, with proven nonzero divisors and growth bounds.
No compiler recursion feature or weakened check was introduced.

## Successful routes and commands

Commands are from repository root; OMEGA is the recorded absolute release
binary. Complete logs and runtime stdout/stderr are in the Library packet.

- `mbx clippy -p validation --release --no-deps`: exit 0 (existing warnings).
- `mbx check -p validation -p omega`: exit 0.
- `mbx test --release --no-run -p compiler --test corpus_runner --message-format=json`:
  exit 0; runner identity accompanies evidence, reused without another build.
- `OMEGA --check --verify --timings --target linux_x86_64 samples/cli/independent/research_record_linkage/main.omg`:
  fresh source check passed; formatter check exit 0 in 22.555s.
- `OMEGA --timings --target linux_x86_64 --build-dir /workspace/new064-renewed-acceptance-20261011/native --report-file /workspace/new064-renewed-acceptance-20261011/native-compile-report.txt samples/cli/independent/research_record_linkage/main.omg`:
  exit 0 in 22.953s. Real ELF `native/omega-program`, SHA-256
  `b7a6465104d34c3e502c1c6ab36829521b6a26abdfecf398c9285ac8423bc2a3`.
- `PYTHONDONTWRITEBYTECODE=1 python3 -B samples/cli/independent/research_record_linkage/validate.py --executable /workspace/new064-renewed-acceptance-20261011/native/omega-program`:
  all 39 cases passed exact 13-field stdout, empty stderr and authored exit.
- `PYTHONDONTWRITEBYTECODE=1 python3 -B samples/cli/independent/research_record_linkage/validate.py --terminal --omega ABSOLUTE_OMEGA`:
  all 19 cases passed actual `run --both`, native exact stdout and Terminal exit
  agreement. Terminal CLI exposes termination only; its computed field
  fingerprint is checked, not independently exposed Terminal stdout.
- `OMEGA_CORPUS_JOBS=1 python3 tools/corpus_gate.py --runner RUNNER --filter COMMA_SEPARATED_15_NAMES --json corpus-final.json`:
  clean, seven checked/eight rejected, zero wrong reasons/lost successes.
  Exact full filter and observations accompany corpus-final.json.
- Same runner `--both --filter runtime_bounded_unsigned_sum_chain_exit`:
  clean, actual native and Terminal exit 48, agrees. No --record.
- `OMEGA --check --target linux_x86_64 samples/cli/independent/research_record_linkage/controls/unproved_index/main.omg`:
  exit 1, exact `cannot prove index `position` is within length 3 in select::entry`.
  This complete intended-invalid source rejects for its bounds obligation.

Final header/comments, documentation and source manifest only follow the runtime
batch. Matching and parsing code, iterative formatter, compiler, provider and
all frozen inputs are unchanged. Runtime results remain applicable; final
ordinary identity refresh and fresh source verification are also performed.
The default empty-stdin smoke control expects INVALID input and exit 64.

## Original-origin package transaction

Prior denials and prepared-copy proposal are preserved as historical evidence.
Specific renewed authorization for NEW064's same original-origin 286-decision
scope admitted one retry. It executed `omega update --resume --project` at the
original worktree root and passed, exit 0 in 125s; two packages published.
Approved pending review SHA-256:
`b147db7849ec1a2b63910e1f1d842c61db67adc03fdccf4f7ee31658fb1f2a28`;
proposal `5b6cca6862027d6a373db0ccc41203721c2f46d5eb8cc7185b96fb5707f4f456`.
Initial accepted lock SHA-256:
`2a8d8b907ba9c44624e21efe4efd5e162d9b8904d0084f04fe64a4717bc296a1`.

Scope: std 140 callable, 140 external-supply, Console process and
FilesystemHost filesystem; app three Console Terminal permissions (input,
output, termination) and one Console process row. Whole inventories and
source/declaration links are supplied. Ordinary compiler/source refreshes add
zero decisions, with both complete accepted policy frames byte-for-byte equal.
No origin transplant, acceptance bypass, altered custody or new authority.
Accepted locks are evidence only and must not be copied to another origin.

Claim attempt ticket `8ecec37c1301485191186e1b7d95b947` failed with Git -9;
read-only reconciliation succeeded and found no registration. No duplicate
claim mutation or landing queue reservation was made. Required independent
review and coordinated main publication remain, with clean candidate retained.

## Direct output completion, 2026-10-11

The pinned continuation reused the exact recorded compiler and its matching
production rmeta/rlib build receipt. A private observation-only Rust consumer
compiled without dependency rebuilds, prepared the same accepted original root
and executed the report's exact canonical Terminal artifact with retained
provisions. All 39 inputs completed with their authored explicit Console exits;
all 39 stdout buffers matched every expected byte and all stderr buffers were
empty. Raw streams, semantic exits, canonical artifact, compiler identity,
observer/source/dependency hashes and exact associations are preserved in the
final Library packet. This closes direct Terminal output coverage without
calling the prior 19 exit-fingerprint comparisons output equivalence.

Fresh-context reviewer `/root/independent_review` approved the preserved
8774273f459d9c19725a03283d69a9a32abe024b tree after independently consuming the
Library packet and recomputing all 30 scientific cases. Its separate final-tree
review follows current-main reconciliation. Source/metadata refreshes continue
to preserve both accepted policy frames with zero new decisions.

## Final integrated qualification

One main fetch followed completed pinned qualification. Main
8ca55c8e7e42b09f0f30bde26e7594ba0f93ce08 changed other compiler semantics,
so a fresh producer c032164243705cc0d40ba485a59b0a88a3e87fca was built
in 3m29s. Its Omega SHA-256 is
822ee3e0c5c1a9f7b0d4a57eef316837500847a19d1aedc98994ace9bfc7ecbf.
The two proof-repair files were unchanged by incoming main; their demonstrated
repair and controls remain necessary. All incoming golden rows were preserved
and exactly the three previously recorded own lines added; no new recording.

Fresh --check --verify, native compilation, all 39 native exact outputs/exits,
all 39 direct admitted Terminal stdout/stderr/semantic exits, 15 focused compiler
regressions (7 checked/8 rejected), and native/Terminal arithmetic exit48 passed.
Both role-aware sample gates passed: the complete program's empty-stdin native
oracle and the separate correct-reason intended bounds rejection. The sample
native gate was explicitly given --target linux_x86_64. An initial invocation
selected its macos_arm64 default and was stopped before acceptance; its evidence
is excluded and the accepted Linux lock was verified unchanged. All ordinary
source/compiler refreshes preserve both complete accepted frames, zero new
decision meanings. Model, input parser, expected diagnostics, source modes and
scientific fixtures/oracles remain unchanged with zero tolerances.

The fresh current producer generates the exact same native ELF b7a6465104d34c3e
502c1c6ab36829521b6a26abdfecf398c9285ac8423bc2a3 and canonical Terminal
artifact 953a512cad847f588d72d517f74f80bf142ec4d448d6dff7dd6c222b0b0816bb.
Final edits after execution affect only validation consumers, documentation and
metadata; actual Omega bodies, providers, compiler Rust and all frozen inputs
are unchanged. Their behavioral evidence is reused explicitly, followed by
normal source identity refresh, fresh verification and exact-SHA build receipt.

Claim registration attempts remain failed. The fixed-ticket historical Git read
was killed with signal9; normal fresh registration also failed and read-only
reconciliation found no active NEW064 claim. No registration is fabricated.
The separate normal FIFO landing helper is mandatory for publication.
