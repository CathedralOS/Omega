# Bootstrap construction contract

[Whole-chain minimization](MINIMIZATION.md) owns the goal, architectural authority
and comparison policy. This contract defines what construction results mean.
The previous requirement for a single selected topology and formal correctness
certificates on every edge is superseded by that mandate. The Omega language's
ordinary proof checking and safety semantics are unchanged.

<a id="selected-execution-chain"></a>

## Variant construction routes

Variants are peers, each with an explicit source route, seed, execution target,
resource profile and completion evidence. The existing interpreted variant uses:

```text
Alpha VM + admitted Beta compiler tape
  -> Beta-written Gamma evaluator
  -> Gamma-written Delta compiler
  -> Delta-written Epsilon evaluator
  -> Epsilon-written bootstrap compiler
  -> Omega compiler built for Alpha
  -> Omega compiler recompiles its own source
```

Gamma evaluates the Delta compiler, whose output is Gamma source implementing
the Epsilon evaluator. That evaluator interprets the bootstrap compiler. This
route has no compiled bootstrap-compiler or Epsilon-evaluator tape. Its Alpha
serialization belongs to the Omega target. Alternative variants may compile
intermediate languages, change stages or use other execution targets.

The existing Alpha backend is a responsibility of the bootstrap compiler and the
Omega compiler; a Rust Alpha backend is not a prerequisite. Rust may remain a
development comparator and report a selected Alpha operation as unimplemented.
Native compilation must still handle an inactive Alpha Build binding according
to ordinary target-selection rules.

## Source subjects

The bootstrap compiler implements enough Omega to compile the actual Omega
compiler closure. Every supported facility preserves its specified meaning;
unsupported cases fail explicitly. This does not permit weakening the product
Omega language, accepting invalid programs, or recognizing only particular source
files/AST shapes. Conservative authoring and a documented coverage subset are
permitted; two complete independent Omega implementations are not required.

Each variant names its exact sources and dependencies. The existing interpreted
languages retain their declared ASCII source envelopes and manifest semantics
until that variant deliberately revises them and validates its consumers. Source
or manifest changes invalidate evidence about the old bytes.

The [compiler request](../wiki/spec/build/compiler_request.md) owns the product
compilation boundary. A diagnostic single-source route is not evidence of complete
package/Build admission. [Terminal Psi](../wiki/spec/terminal-psi/product.md)
remains a product boundary, not a mandatory extra bootstrap language.

<a id="refinement-and-admission"></a>

## Construction and acceptance

For each executed step, identify the source, executing implementation, seed,
inputs, target, resource profile and resulting output. Distinguish malformed
source, unsupported coverage, authored runtime failures, exhausted resources and
implementation defects. Publish executable output only after successful
construction; stale output never establishes success of a failed invocation.

A variant closes when its documented seed and repository sources construct the
Omega compiler, which rebuilds itself reproducibly under the same declared
source/target profile. Validate actual resulting compiler behavior, including
relevant rejection controls. The first compiled generation may emit different
bytes from its self-compiled successor; deterministic reconstruction within a
generation/profile remains required.

Hashes bind identity, tests exercise behavior, and a source audit explains trust.
None alone is a formal compiler-correctness theorem. Report observations and
assumptions precisely. Comparisons with Rust or another variant are useful
independent diagnostics, not permission to replace the claimed construction.

<a id="native-realization"></a>

## Acceleration and native execution

The [acceleration policy](MINIMIZATION.md#explicit-acceleration) permits explicit
jets and development compilers with reference validation and disclosed additional
trust. Their results must be labeled as accelerated. A host compiler that supplies
language meaning cannot be hidden as packing or invocation scaffolding. Retain
an executable reference route even when a complete cold run takes days.

Alpha seeds retain their written tape semantics, checks and platform-specific
implementation assumptions. Stamping a tape into its VM container does not
compile that program to native instructions. A native variant owns its instruction,
ABI and executable-format behavior and the added audit burden. Measured execution
on one host is not evidence of execution on a different host.

<a id="non-lockstep-refinement"></a>

## Future proof research

The retained [Gamma checker](proofs/checker/README.md) checks finite ground
relations. Its Beta encoding theorem would establish exact encoding of one source
subject to one tape, not that the evaluator implements Gamma. Executing a checker
through an evaluator does not independently prove that evaluator correct.

Formal source-to-execution refinement, proof certificates and translation
validation are future research, not current construction acceptance requirements.
Existing proof formats and rules still define any claim made using those tools;
removing the construction dependency does not make an unchecked certificate valid.
Do not add proof infrastructure merely to satisfy the former acceptance contract.
