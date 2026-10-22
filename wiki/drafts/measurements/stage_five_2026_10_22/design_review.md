# Stage 05 design review after the lookup experiment

Follow-up: the [implemented cold-graph experiment](ordered_graph/README.md)
measured no previous Product judgment on either reuse probe, correcting the
initial reuse premise below. The owner selected compact symbolic graphs. The
bounded change passed its controls and reduced ordinary Stage 05 medians by
5.35% and 2.59% on two workloads; CHECKED-PACKAGE-REUSE remains open. This document
retains the earlier design hypotheses as history, not current measured facts.

2026-10-22, base `b428c347b51786c5f57d577d59413977d6cf5d9f`, Apple M4,
optimized dev compiler. The [completed lookup experiment](state_lookup/README.md)
retained no compiler change: neither variant established useful latency savings.
This review proposes the next experiment; it does not claim that Stage 05 has
been optimized or that the existing performance requirements are complete.

The larger design opportunity is to make checked dependency results reusable
without requiring equality of the entire importing program. For cold checking,
the distinct question is how much work recursive symbolic representations make
each observation, comparison and join perform. These require separate experiments.

## What the current workload measures

The unchanged `samples/cli/facts/length/local/main.omg --check` has a baseline
Stage 05 median of 544.274 ms in the first ordinary paired batch. Total median
is 668.540 ms. Attribution below uses only the five baseline observations in
[phase-timings.json](state_lookup/phase-timings.json); profiled and ordinary
observations are not pooled. Stage 05's profiled median is 543.385 ms.

| Inclusive scope | Median ms |
| --- | ---: |
| Typed-tree preparation | 109.434 |
| Typed-program validation/preparation | 87.214 |
| Building check facts | 188.530 |
| Checking those facts | 90.581 |
| Finalizing execution identities | 43.380 |
| Late selections, custody, types and boundaries | 21.963 |

The measured cost spans several responsibilities. These are inclusive medians,
not a partition to sum. Inside fact construction, actual flow-fact construction
takes 40.414 ms. The enclosing fixed-point region takes 93.278 ms. Two outer
iterations across two checks do not indicate runaway global convergence on
this input. Machine-group parallelism and batched occurrence environments
already exist. More workers or another worklist are not established remedies.

The validation counters report 153 std/core machine visits and four other
machine visits across two checks. The corresponding summed worker durations
are 166.099 and 2.724 ms; they overlap and are not wall-time savings available
to subtract. Neither count establishes that every dependency visit could be
reused under its actual semantic inputs. Retained input/program clones together
account for only about 2.34 ms in these profile medians.

## Choice 1: retain checked dependency graphs at their own boundary

The existing [CHECKED-PACKAGE-REUSE task](../../../../TASKS.md) already names
the appropriate outcome. Its older Windows timings and larger machine counts
are historical evidence; they are not measurements of this revision or workload.

`RetainedCheck::judges_preparation_input` compares preparation admission and the
whole typed program. That is a sound exact-reuse route, but an importing program
with additional local bodies is a different input. The proposed boundary is an
immutable dependency graph with its checked facts, plus the dependent's local
work and explicit correspondence to imported declarations, states and expressions.
Start with same-invocation ownership and borrowing, before persistence.

Keep the existing checking/fact owners. Body-dependent queries still need frozen
bodies: returned-reference origin, binding stability, observation freedom and
return ranges cannot be replaced by six coarse summaries. A call's actual values,
new generic substitutions and new local bodies retain their own obligations.
Package policy acceptance is not semantic checking evidence. Psi must continue
checking every target-scoped source body; target selection cannot prune that work.
Native realization still needs checked bodies or a valid Terminal product.

The key measurement is repeated *equivalent judgments*, not cache lookups or
matching names. Instrument check purpose and dependency identity, distinguishing
first check, reused judgment and checks required by changed inputs. Attribute
work to those categories. Existing checks of whole-input equality alone cannot
answer how much package-local work is reusable.

The smallest implementation experiment should retain one dependency's complete
checked graph within the same public CLI invocation and reuse only judgments
whose full inputs are established unchanged. Compare ordinary CLI timing with
reuse enabled and disabled, including correspondence/validation overhead. Reject
if the work remains necessary, preserving precision requires redoing it, or the
end-to-end savings fail to separate from run variation. Establish the removable
work ceiling from those measured judgments before estimating a speedup.

Use the existing std probe and selected touched corpus groups. Include controls
that change a dependency contract, caller actual interval, returned-reference
origin, generic substitution and admission inputs. Require the same acceptance,
diagnostics and native/Terminal behavior. Task closure still requires the full
published CHECKED-PACKAGE-REUSE acceptance; this review closes none of it.

## Choice 2: make cold symbolic checking operate on compact identities

`ValueEnvironment` contains vectors of facts plus string-keyed tree maps and
sets. `ordered_values::Operand` recursively owns boxed receivers and vectors of
operands, with structural equality and cloning. It already uses resolved symbols
and exact call occurrences; names are not its sole identity. String paths still
participate in legacy write invalidation. Sampling includes recursive operand
comparison, string comparison and allocation, but establishes no memory-bandwidth
or cache-latency limit.

A candidate representation is a per-body arena of observation terms, with
handle-based children and compact fact records keyed by those terms and exact
storage versions. Reuse existing canonical place/fact identities rather than
creating another competing place model. Preserve the distinction between
repeatable denotations and distinct effectful call occurrences, arithmetic
domains and carriers, incoming-path joins, and invalidation after overlapping
writes. Merely interning printed expressions would be unsound.

First count operand constructions, recursive comparisons, allocated bytes and
environment clones on the real workload. If they explain substantial cost,
prototype one measured hot relation family against the current implementation
as an independent oracle. Measure the arena's construction and lookup cost too.
This would improve cold checks if successful; package reuse alone cannot do so.
Do not replace every map or create a generic query framework speculatively.

`occurrence_value_environments` already batches requested expressions within a
state, and construction observations use that batch lazily. The next experiment
must target remaining duplication rather than rediscovering this existing repair.
Likewise, `CheckingContext` deliberately invalidates meaning-sensitive memos when
effect plans or selections change. Keeping all caches alive would change semantics.

## Research and decision

Exact searches on 2026-10-22:

- `site.rustc-dev-guide.rust-lang.org queries incremental compilation red green stable hashing`
- `site.clang.llvm.org data flow analysis introduction worklist basic blocks changed`

Read the [Rust compiler guide's incremental compilation chapter](https://rustc-dev-guide.rust-lang.org/queries/incremental-compilation-in-detail.html),
including dependency tracking, stable identities and fingerprint costs. Its
relevant lesson is explicit query inputs and dependencies; stable hashing and
persistence also cost time. This supports measuring a bounded retained-graph
boundary first, not importing a full query engine. Cross-process reuse cannot
identify declarations by their old process-local handle values.

Read [Clang's dataflow introduction](https://clang.llvm.org/docs/DataFlowAnalysisIntro.html),
especially fixed-point iteration and reprocessing changed inputs. That is a
useful general mechanism, but the current Omega counts do not establish excessive
outer iteration. No fresh worklist rewrite is justified by this source.

Recommendation: measure exact dependency judgment reuse next under the existing
task. Keep cold symbolic representation as a separately measured question.
Neither recommendation is a demonstrated speedup or hardware-limit diagnosis.
The completed experiment's raw observations, rejected patches, correctness
limits and replay instructions remain linked above.

Source anchors reviewed at the recorded base:

- [RetainedCheck input equality](../../../../omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checking.rs)
  and [driver reuse dispatch](../../../../omega-rust/psi/compilation/driver/src/source_checking.rs).
- [Fact-construction dependencies and fixed point](../../../../omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/facts.rs).
- [ValueEnvironment](../../../../omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/value_environment.rs)
  and [symbolic operands](../../../../omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/ordered_values.rs).
- [Batched observations](../../../../omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/guard_narrowing/arrivals.rs),
  [constructor batches](../../../../omega-rust/psi/compilation/checking/src/value_custody/struct_literals/observations.rs),
  and [memo invalidation](../../../../omega-rust/psi/compilation/checking/src/frozen_program.rs).
