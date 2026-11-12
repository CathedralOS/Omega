# Whole-chain minimization

## Objective

Develop competing bootstrap chains that construct the Omega compiler from an
explicitly documented, auditable seed, then use the resulting compiler to rebuild
itself reproducibly. Carry viable approaches through actual end-to-end execution.
The goal is a working, independently reviewable construction, not a collection of
language prototypes. This mandate supersedes the former fixed-topology,
full-language bootstrap-compiler and mandatory formal-certificate requirements.

Optimize the complete construction path: what each stage costs to implement,
what it lets its successor express, and what the whole chain costs to understand
and execute. Humans and LLMs must be able to trace semantics, control flow,
storage, dependencies and trust assumptions. Source size and stage count are
measurements, not objectives. Complexity near the seed affects every successor.

## Architectural authority

Implementers may change stage count, language facilities, representations,
interpreted versus compiled execution, VM versus native targets, and repository
organization. They may implement and adopt those changes within this mandate;
an architecture experiment does not need a separate owner ruling merely because
it differs from the existing chain. Omega's required semantics remain binding.

Variants are peers. Use separate worktrees and branches to advance materially
different approaches concurrently. No variant is designated selected during
exploration. Share useful discoveries and comparison inputs without requiring a
common implementation or premature convergence. A merged variant change is not
selection of that architecture. Current numbered source directories describe the
interpreted variant; their existence grants it no privileged status.

Use **bootstrap compiler** for the compiler that first builds the Omega compiler,
and **Omega compiler** for the product. Historical C/D labels and artifact paths
may remain in old records; they are not the public names. A bootstrap compiler
need not implement every Omega facility. Supported facilities preserve their
specified semantics; unsupported cases fail explicitly. File/hash allowlists,
source-specific recognizers and manufactured output do not count as compilation.

## Retention test

Every retained language feature, runtime mechanism, tool and sidecar must serve
an actual successor compiler or necessary invocation/validation. Compare deletion,
consolidation and a source refactor before adding machinery. Additional language
breadth must earn its implementation cost by simplifying successor construction.
Intermediate self-hosting is optional, not a goal in itself.

The chain primarily contains the source needed to construct its successors.
Avoid accumulating wrappers, duplicate authorities, compatibility layers or
abandoned experiments. Preserve useful failed approaches in Git history and
concise records. Keep any surviving proof research explicitly outside current
construction acceptance; do not expand it without an actual construction need.

<a id="constraints-on-comparisons"></a>

## Comparisons and execution evidence

Compare real compiler customers and required behavior. Bind each result to its
source revision, inputs, seed, implementation and execution profile. Architectural
variants may use different representations, capacities and trust assumptions;
disclose these differences rather than requiring identical internals. Record:

- implementation and specification burden, storage and control-flow reasoning;
- source/artifact sizes, measured memory and cumulative allocation where relevant;
- cold reconstruction time separately from ordinary development iteration;
- actual outputs, failures and independently reviewed semantic controls; and
- retained tooling and additional trusted components.

Do not infer correctness from line counts, matching hashes, successful execution,
or agreement between reviewers. Distinguish actual measurements, conservative
bounds, projections and unrun checks. Reproducibility means repeated construction
from the same declared inputs/profile produces the required deterministic result;
first and self-compiled generations need not be byte-identical.

<a id="private-capacity-changes"></a>

A measured one- or two-day cold bootstrap can be acceptable. Diagnose bottlenecks
and reasonable remedies before declaring an architecture infeasible. A host
watchdog expiration is an unfinished observation, not a language rejection.
Choose resource budgets before expensive runs and account for the shared host.
Private capacity tuning is delegated; preserve defined failure behavior and
validate changed boundaries instead of merely raising constants.

## Explicit acceleration

Jets, host development compilers and other accelerators may unblock development
or form a documented variant. Retain executable reference meaning, validate
correspondence on relevant positive and negative cases, disclose extra trusted
components and record actual inputs/results. A reference timeout does not prove
correspondence. Accelerated execution is not reference-chain execution, and an
accelerated result alone does not complete a source-owned construction route.

A tool that supplies source-language meaning is an implementation stage or an
accelerator and must be counted as such. Invocation, packing declared bytes and
reporting do not by themselves make a host tool a language implementation.

## Completion and continuation

A completed variant constructs the Omega compiler from its documented seed and
repository-owned sources, then uses that compiler to rebuild itself reproducibly.
Document all retained trust assumptions and any host/runtime prerequisites. Do
not relabel a partial compiler, toy program, identity check or prototype as this
result. Preserve language safety checks and explicit failure/publication behavior.

Formal certificates proving bootstrap correctness are outside current acceptance.
Clear semantics and boundaries leave room for stronger assurance later; they do
not require speculative proof infrastructure now. Omega's ordinary language
proofs, contracts and safety rules are unaffected by this scheduling decision.

Persist through difficult implementation work. An experiment may end when evidence
supports it: record what failed, what was tried, and what would justify
reconsideration. Neither its first obstacle nor historical investment decides its
fate. Continue viable alternatives and independent work.

## Owner escalation

Make engineering and architectural decisions autonomously within this mandate.
Record unresolved changes to required semantics, trust claims or the goal itself
in [OWNER_QUESTIONS_BOOTSTRAP.md](../OWNER_QUESTIONS_BOOTSTRAP.md), with evidence,
alternatives and a recommendation. Routine architecture choices, documented
accelerators and capacity adjustments already authorized here are not owner
questions. Do not claim guarantees unsupported by the actual execution route.
Continue independent work while genuine owner questions await answers.
