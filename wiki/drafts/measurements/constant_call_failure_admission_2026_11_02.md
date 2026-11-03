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
The focused corpus qualifies 32 fixtures: 16 positives check and 16 negatives
reject for their intended reasons. The saved calling-policy inspection and its
actual retained-program execution both match their oracles.

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

## Executed evidence

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

The first corpus attempt completed 29 applicable observations but stopped one
positive recursion control at an old compiler-identity lock. That setup refusal
receives no semantic credit. A normal update and actual execution passed, then
three fresh corpus observations completed the 32-fixture selection. The other
29 completed observations keep their original records. An earlier wrapper
misclassified the correct CLI exit 70; the correction uses its completed native
and Terminal observations without repeating the program.

The eight saved-program tests are retained from the R17 component run. Its
separate source-domain failure is not credited. Five source tests pass in R19;
the compiled patch is byte-identical to the later source checkpoint. The archive
keeps the failed intermediate tests, including the domain case that reached
integer division, and the subsequent full-entry-requirement repair.

## Bounded iteration repair

The layout update completes in 134.31 seconds, within the recorded 180-second
acceptance, and its runtime result remains 70. A slow intermediate version was
intentionally stopped after more than six minutes. Its live sample showed a
redundant whole-program caller check despite completed analysis showing no crash
routes. That analysis is reused only when there are no entry requirements. Every
recorded entry requirement is checked at the actual call.

Staged builds also co-select `package-source` to match the CLI's transitive
features. The final CLI build marks all four warmed compiler owners fresh;
the changed dependency takes 17.25 seconds and the final CLI command takes
73.61 seconds. The corpus runner takes 80.87 seconds. Source changes differ from
the earlier build, so these are measured durations and reuse evidence, not a
claimed like-for-like speedup ratio. No broader performance campaign is implied.

Independent exact-candidate review and normal main landing are publication
requirements; the raw execution results above do not substitute for them.
