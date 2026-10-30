# Invocation-set convergence research

2026-10-29. The prior fixed-workbench package profile attributes 53.5-53.9 s to
settled checks, not source rendering. Publication on main source `f1058703209`
instead hit a 180 s first-update bound; an extended replay was stopped after
438.34 s. A noninvasive stack sample located `infer_synchronous_invocations`.
The unchanged std source and root fixture do not explain that difference;
compiler revisions differ and their timings are not a controlled speedup pair.

Local inspection: #794 added a realized invocation vector beside the existing
abstract-target vector. Abstract insertion already canonicalizes its ordering;
realized insertion only deduplicates. The fixed-point test compares whole
vectors, so it can mistake a permutation for new semantic information.
The six-node model in the record repeats with period two, despite an unchanged
effect set. Sorting preserves the independently computed graph-reachability
sets and makes the same model converge. This model is a mechanism witness,
not proof that the actual compiler completes the customer program.

Exact query: `site.cs.cornell.edu dataflow analysis monotonic finite lattice fixed point worklist`.
Read the returned primary [Clang dataflow introduction](https://clang.llvm.org/docs/DataFlowAnalysisIntro.html),
Formalization, especially the finite-height monotonic iteration and worklist
discussion. It supports treating progress as growth of facts rather than
storage-order changes. Do not adopt its example widening or discard effects.
A worklist rewrite and hash-set replacement are unnecessary for this defect.

Candidate: use the existing abstract-target insertion pattern for the realized
set, sorting by variant and full symbol identity/parameter ordinal after an
insertion. All requirements and closed applications remain separate; no name,
trait family, library or target is recognized specially. The actual consumers
map every retained target through `exact_invocation_target`; no first-target
selection is introduced. The accepted contract is in
`wiki/spec/language/effects.md#direct-synchronous-invocation`: inferred helper
edges retain exact identities, public ceilings stay enforced, and selected
component-boundary cycles still reject.

Decisive check: the frozen original operand-order package completes ordinary
review, native execution and Terminal agreement, while numeric requirements,
mutation invalidation, distinct applied provider identities, hidden callback
edges and provider cycles remain enforced. Keep the historical hang, interrupted
diagnostic and all valid candidate runs. Do not quote a percentage improvement
against an unfinished baseline, or pool this newer-main repair with the older
workbench attribution.
