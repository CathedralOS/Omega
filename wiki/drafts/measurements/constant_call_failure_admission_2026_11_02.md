# Reject unsafe constant calls before execution

A helper used to calculate a range limit can be safe for one input and trap for
another. The compiler proves the concrete call safe before evaluating the
constant. Dividing seven by one still supplies a bound of seven; changing only
the divisor to zero rejects before execution.

All nine [guarded-call places](../../../samples/cli/facts/provider_contract_endpoint/guarded_crash/README.md)
run with native and Terminal exit zero on macOS ARM64. Their nine twins reject
the `Trap` route through `limit`. The original crashing endpoint and both
layout-derived crashing-helper controls also reject during admission. An ordinary
runtime call with zero still produces native signal 5 and Terminal `Trap`.

The array and layout customers retain their authored exits zero and 70.
The recursion and unrelated-pending-condition controls also check and run at zero.
Before integration, the R20 corpus qualified 32 fixtures: 16 positives checked
and 16 invalid controls rejected. Its saved calling-policy inspection and actual
execution matched their oracles. On the integrated source, 15 positives check
and all 16 invalid controls reject. The saved-policy positive fails transport
on both the fixed main baseline and the candidate; it is not counted as current
execution success.

## Why the check is sufficient

The checker retains completed crash analysis with the exact machine and body
that established it. Source admission checks the supplied constructions and
every recorded machine and entry requirement. If a requirement or possible crash
remains, an ordinary caller checked at the actual arguments proves the whole
call. That completed check serves both precondition and failure admission.
Every recorded entry requirement is checked at the actual call. With no entry
requirements, completed analysis showing no crash routes can be reused.

This distinction matters for host arguments. Constructing a field does not prove
a separately recorded nonzero-domain membership. Tests establish that one is
accepted and zero is rejected before division, through both named and symbol
APIs. Authored preconditions have separate valid and invalid controls. Analysis showing no crash routes cannot establish either kind of input
requirement.

Before executing a saved program, admission checks the crash guards emitted for
its exact body. Every value referenced by a guard must belong to the entry being
admitted. Both ordinary compiler evaluation and private scalar-entry admission
use this check. Missing or competing body associations refuse. Invalid closed
operands cannot disappear behind a false conjunction or implication. Unknown
routes do not count as impossible.
The other effect, termination, resource and selection-authority checks remain
mandatory before interpretation.

## Execution before integration (R20)

| Programs or checks | Observed result |
| --- | --- |
| Nine safe fact places | Source checks and actual native/Terminal exit zero |
| Nine zero-divisor twins | Admission refusal naming `Trap` and `limit`; no interpreted trap |
| Original crashing endpoint | Same early `Trap` refusal through `limit` |
| Two layout-derived crashing helpers | Early `Trap` refusal through `forbidden` |
| Ordinary runtime zero divisor | Native signal 5; Terminal `Trap`; CLI exit 255 |
| Array-layout customer | Native and Terminal exit zero |
| Folded layout customer | Native, Terminal and CLI exit 70 |
| Recursion and unrelated-condition controls | Fresh normal update, source check and native/Terminal exit zero |
| Source admission Rust tests | Five pass, including input domains, authored preconditions and opaque target inputs |
| Saved-program guard Rust tests | Eight pass; their code and inputs are unchanged by the later source repair |
| Scoped corpus | 16 checked and 16 intended rejects, including real saved-policy execution |
| CLI and corpus-runner Clippy | Both commands exit zero |

Every positive fact binds all four hosted program entries. Native execution here
is measured only on macOS ARM64; bindings do not establish execution on the other
hosts. The full scalar-range migration remains open. This result discharges its
crashing-bound-helper requirement only.

The baseline is the frozen R13 source and oracle: all nine safe programs ran,
while their twins reached an interpreted `Trap` during constant calculation.
The original endpoint instead stopped at an unrelated missing-body error.
Those are baseline defects, not successful admission checks.

## Exact source and replay

Both optimized R20 producers use exact source
`2273a67852265ff8215d373b029f3be7532f275a`, with empty compiler-input patches and
verified opt-level 1:

- CLI SHA-256: `fd1d135387a326e11e676a23b128ab03ed637592e577d1b680d38a7579206d62`.
- Corpus runner SHA-256: `0a4556089bd8845aec67dec89afc8325a6fdf3b5440d0ebde3f9b9a6f6a0aa45`.

[Pinned source bundle, raw observations and replay inputs](https://github.com/CathedralOS/Omega/blob/c03441063d2c5488948a77f61ef41660565288bb/README.md)
are kept in a separate immutable evidence commit. The manifest binds the archive
and source bundle; its file index binds every archived input and receipt.
The `+wt` corpus stamps are preserved with the oracle patch and exact fixture,
expected-diagnostic and accepted-lock replay inputs. They are not relabeled as
clean source. All unselected golden blocks remain byte-identical and no fixture
keys are removed.

The first R20 corpus attempt completed 29 applicable observations but stopped one
positive recursion control at an old compiler-identity lock. That setup refusal
receives no semantic credit. A normal update and actual execution passed, then
three fresh corpus observations completed the 32-fixture selection. The other
29 completed observations keep their original records. An earlier wrapper
misclassified the correct CLI exit 70; the correction uses its completed native
and Terminal observations without repeating the program.

R20 reused eight saved-program tests from R17 and five source tests from R19.
R17's separate source-domain failure was not credited. The R19 compiled patch
was byte-identical to the later source checkpoint. The archive preserves failed
intermediate tests, including the domain case that reached integer division,
and the subsequent repair that checked every recorded entry requirement.

## Bounded iteration repair

The R20 layout update completed in 134.31 seconds, within the recorded 180-second
acceptance, and its runtime result remained 70. A slow intermediate version was
intentionally stopped after more than six minutes. Its live sample showed a
redundant whole-program caller check despite completed analysis showing no crash
routes. That analysis is reused only when there are no entry requirements. Every
recorded entry requirement is checked at the actual call.

Earlier R20 build steps included `package-source` so Cargo enabled the same
dependency features as the CLI. The final CLI build reused the four compiler
crates built in those preceding steps. The changed crate took 17.25 seconds,
the final CLI command took 73.61 seconds, and the corpus runner took 80.87 seconds. Source changes differ from
the earlier build, so these are measured durations and reuse evidence, not a
claimed like-for-like speedup ratio. No broader performance campaign is implied.

Independent exact-candidate review and normal main landing are publication
requirements; the raw execution results above do not substitute for them.

## Qualification after integration

A real overlap with the earlier source-checking repair required replaying this
change on main at `5209eac451caf43591135c2b453033e60b4df917`. The integrated
compiler source is `c5cc3d8749224ed1a76fa7abad1707dfd9f5dd1d`; its production
patch is unchanged by that replay. Generated constant calls with a possible crash now refuse
earlier during source checking, naming `limit()` and `Trap`. The two layout
helpers still refuse before execution through the later admission check.

Fresh integrated observations cover all nine positive places and nine twins,
all 13 Rust tests in one run, array and layout exits zero and 70, the ordinary
runtime Trap, recursion and unrelated-condition controls. Both CLI and runner
Clippy commands pass. The 32-fixture corpus supplies 15 checked positives and
16 intended rejects, plus one explicitly retained baseline failure.

That remaining positive fails while transporting the saved policy's artifact,
before failure admission. An optimized producer built from exact main at
`5209eac451caf43591135c2b453033e60b4df917` reproduces the identical diagnostic
with the same fixture/library paths and byte hashes. The diagnostic names a
Windows filesystem body because one failed body adaptation empties the shared
descriptor lookup; it does not identify that body as the original cause. The
full replay is retained. Its historical golden row stays unchanged from main;
the failure is neither credited as execution nor made an expected rejection.

The layout customer's source is unchanged. Its normal update required package
review after incoming main moved filesystem bindings into Build. Independent
inspection found the same native symbols and unchanged boundary contracts; the
fixture makes no filesystem calls. Ordinary reviewed resume passed, followed by
native and Terminal exit 70. The first source-checking phase took 165.36 seconds,
resume 78.29 seconds, and execution 50.58 seconds; these distinct phases are not
collapsed into the earlier R20 timing. Disk-full link attempts and subsequent
lossless storage repairs remain visible in the raw records.

Integrated optimized producer SHA-256 identities:

- CLI: `5ac449c900d6d3b72d89a5773c67b73768e5276cc9b148f4dc8c36aa00b2b475`.
- Runner: `3ab0bb66e77a5a5de9c9537c7bae9f20eb523af2d0a6791c941f67620b197c41`.
- Main baseline runner: `8dc24f641172af8488afccf04f5d7be07cf268a3bf69ef6de982b7883977e407`.

All retain verified opt-level 1. Integrated corpus stamps retain `+wt` together
with their diagnostic-oracle patch and exact replay inputs.

[Integrated raw results, exact-main comparison and source bundle](https://github.com/CathedralOS/Omega/blob/024779c6ffe4db42ec4ce3d9771f35b5377e36cd/README.md)
retain the successful scoped checks and the baseline failure as separate outcomes.
