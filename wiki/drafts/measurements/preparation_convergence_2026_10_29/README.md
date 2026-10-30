# First-time package preparation: invocation-set convergence

Retained a correctness repair: give realized invocation sets deterministic order
so the inference fixed-point test cannot mistake a permutation for a new effect.
The unchanged original operand-order control now completes package preparation,
compiles, and executes with native/Terminal agreement. No iteration cap, missing
effect, weaker provider-cycle check, source rewrite or package-policy bypass is
part of the repair.

## Fixed source and baseline

Parent: ITERATION-PERFORMANCE, continuing the user-assigned first-time package
preparation outcome. Base `cb087c46476f255397ba2e8cc3cbba4dd978c2ea` contains
the stack-overflow regression publication; its compiler source is unchanged
from main snapshot `f1058703209f782fc08fc621471257095a2f7f47`.
Baseline optimized dev executable SHA-256:
`5f0e2fc5dde252263b64b47a6dd593fb3a41c19585f78f45ccd15998b897501e`.

Before implementation, the unchanged `runtime_numeric_operand_order_exit`
first package update hit a 180 s bound. A fresh extended replay was stopped
after 438.34 s following a noninvasive stack sample in
`infer_synchronous_invocations`. Its matching PDB identity, logs, input hashes,
timeout and explicit stop are retained with the
[publication record](../operand_order_overflow_2026_10_29/publication/results.json).
The sampled replay is diagnostic, not a clean performance sample; the initial
timeout wrapper did not persist partial stdout/stderr. Neither was successful
compilation. These failures preceded the candidate edit.

This is a separate source/producer from the older `f1d86d2dff9` workbench
attribution, where first update took 54-71 s and settled checking dominated.
Do not compare those revisions as one optimization experiment. The std source
files and original root/Build inputs are identical between this candidate and
its recorded main baseline. Every trial uses a fresh project/cache, normal
review decisions and resume, and the authored result 0.

## Mechanism and scope

The inference loop reconstructs each machine's effect vectors, then compares
them with the previous iteration. Abstract targets already normalize their
ordering. Realized targets added in #794 only deduplicated insertion order.
Recursive discovery can permute a complete set forever. The six-node
[model](model.py) reproduces a period-two ordering cycle; canonical order
converges in four iterations. An independent graph-reachability calculation
checks that both contain exactly the same effects. That model establishes the
mechanism; the real compiler controls below establish customer behavior.

The ten-line change follows the existing abstract-target insertion pattern.
`extend_realized` orders by variant and exact symbol arena index or parameter
ordinal after inserting a new target. Distinct closed requirement applications,
abstract parameters and services remain distinct. No caller or target is
special-cased. All consumers retain and map every target to its exact provider
application; the accepted `invokes` ceiling and component-cycle obligations
remain in force. See [research and consumer review](research.md).

The fixed-point domain is a finite set of prepared target identities. With
canonical order, vector equality represents set equality; propagation adds
identities rather than counting changed permutations as progress. No new
source fixture is needed to manufacture a failure: the existing std-dependent
operand-order fixture is the demonstrated regression, with its exact inputs
pinned here. Existing invocation positive/reject fixtures cover preservation.

## Validation and result

**Evidence correction:** the original `run --both --target ...` calls were
compile-only, not Terminal observations. Their raw commands/logs are preserved.
[Corrected host replays](host-replay/README.md) now establish all nine actual
native/Terminal comparisons without `--target`, explicitly requiring both exit
reports. Terminal claims below refer to those new observations; the original
separate native executions and timing measurements remain valid.


Candidate optimized dev build: 5m21s. Executable SHA-256:
`5a596bcaa6fb117eefd8735c6bb545e0ebbde1cbf1b9b8c3ba79a586a6b16308`.
The source patch stayed identical from build through validation.

| Unchanged std-dependent control | First update | Resume | Compile | Native / Terminal |
| --- | ---: | ---: | ---: | --- |
| Operand order | 166.85 s, pending review | 4.56 s | 8.05 s | exit 0 / agreement |
| Single evaluation | 161.40 s, pending review | 4.16 s | 7.75 s | exit 0 / agreement |

Five candidate positive fixtures pass compile/native execution and separate
`run --both --keep`: the two originals, the minimized contract-query regression,
the reference scalar snapshot, and the copied-call snapshot through a join.
Six numeric/formation/mutation controls still produce their authored refusals.

Two provider positives run on both baseline and candidate: distinct applied
synchronous providers and a helper whose unused public permission must not
become a realized call. Each preserves its byte-identical native image and
passes Terminal agreement. Three provider rejects run on both binaries:
an applied provider cycle, a cycle hidden behind a helper, and an undeclared
synchronous edge. All retain the required diagnostic. The minimized contract
regression's image also matches its fresh publication-baseline image.

Counts: nine compile/native observations and nine separate Terminal/native
comparisons (five candidate controls plus two provider controls on both
binaries), and twelve expected rejects (six candidate controls plus three
provider controls on both binaries). Review-internal compiles and no-pending-
review setup refusals are not counted as execution observations or failures.
[Summary](summary.json), [quality](quality.json), [provider comparison](providers.json),
exact input/oracle hashes, raw logs and assertion-based [collector](collect.py)
are retained. Touched-file rustfmt, scoped whitespace checks, script parsing,
the model assertions and exact input/binary/output/patch assertions pass.

Decision: retain the convergence repair. It restores the observed stalled path;
there is no percentage speedup against an unfinished baseline, no throughput
claim, and no claim of optimal preparation time. The successful operand-order
update still spends 152.98 s in its two settled checks. The next performance
question is attribution inside those now-completing current-main checks; the
older workbench's numeric-association profile is a lead, not current-main proof.

Limits: live shared Windows x86-64 host; no full corpus or other-host runtime
sweep. This follow-up is a local commit, not part of CI-green PR #822 and not
published or landed by this iteration.

## Publication integration

PR #841 independently landed the identical convergence implementation plus a
cycle regression test. Resolving PR #822's actual text conflict against main
7516f0970de retains that complete main-owned source file byte for byte. The
remaining delta contains regression fixtures and historical evidence; it changes
no compiler or library inputs relative to that integration base. Historical
runtime observations above retain their original source identities. Fresh CI
checks the submitted integration; no new native/Terminal replay is claimed for it.
