# Comparing shorter bootstrap chains

Purpose: reference notes on bootstrap topology, root auditability, checker
placement, and the first Omega compiler. This is not a proposed change or an
implementation request. Keep it while these comparisons are useful; delete it
when superseded by concrete measured work or no longer relevant.

The [selected chain](../../bootstrap/README.md) remains Alpha, Beta, Gamma,
Delta, Epsilon, and Omega. Its functional Gamma evaluator is Beta-authored;
the admitted Beta compiler and exact self-reconstruction remain part of the
root argument. These notes neither reopen rejected prototypes nor change
those decisions.

## Tradeoffs

Minimizing each selected rung separately can miss a cheaper complete chain.
Conversely, fewer rungs can move more compiler and proof complexity into the
trusted root. Artifact pedigree records what produced bytes; it does not by
itself make those bytes easy to audit. A larger opaque evaluator tape is not
made cheaper by treating its readable source as optional.

## Comparison axes

If a concrete compiler customer or proof obligation exposes a remaining cost,
compare the selected implementation and its simpler refactors against only the
relevant alternative:

| Alternative | Required evidence before selection |
| --- | --- |
| Compile the first functional language rather than interpret it | Lowering, layout, emission, and execution arguments together cost less than the evaluator route; faster execution alone is insufficient unless the evaluator is operationally unusable. |
| Have an earlier functional language implement Omega without an upper bridge | The complete first-Omega compiler and its resource/proof costs are smaller than the bridge language, its implementation, and both edges together. |
| Use a state-machine source style for the same compiler workload | Reduced runtime state and visible bounds outweigh added authored control flow and recursive-data machinery. This is not permission to restore the retired Delta dialect. |
| Replace Alpha with per-platform native seeds | Every native implementation and their equivalence argument together are cheaper to audit than Alpha plus its admitted artifacts. |

Root artifact, evaluator versus compiler, source style, checker placement, and
upper bridge are separate comparison axes, not a request to build every
combination. Terminal Psi is a product compiler boundary, not automatically a
bootstrap rung; a comparison using it must count the required lowerer and
authority contract.

Use the [whole-chain comparison contract](../../bootstrap/MINIMIZATION.md#constraints-on-comparisons).
For a serious candidate, independent reviewers receive the declared audit
package and the same bounded mutation exercise: instruction decoding,
arithmetic/allocation bounds, control transfer, parser acceptance, emission,
and proof admission. Record defect discovery and localization effort and
claims that cannot be checked locally. Toy results cannot select a route for
the complete compiler closure.

## Evidence needed for a future proposal

No candidate here has established lower complete audit cost. Which concrete
customer motivates a comparison, which admissions it removes, and whether its
complete resource behavior is feasible must be answered before selection.
Readable checker source alone does not prove its evaluator or compiler.

The [owner-escalation rules](../../bootstrap/MINIMIZATION.md#owner-escalation)
and [new-language approval boundary](../../AGENTS.md#scope-checkpoints) remain
binding, including for experimental replacement languages. A comparison is
not advance permission for an opcode, axiom, accelerator, weakened observation,
or parallel accepted chain. Rejected implementations and measurement diaries
belong in Git, not a permanent archive in these notes.
