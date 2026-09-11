# Terminal Psi verification

This contract separates artifact reconstruction, certificate checking, and proof
search. It does not claim that the complete low-rung implementation exists.
See the [product](product.md), [encoding](encoding.md), and
[observation](observations.md) contracts.
Runtime cycles and grouped recursive certificates follow
[control flow and ranking](control_flow.md).

## General proof integration status

The responsibilities below are specified; the general mathematical calculus
and its checking bridge are not. [General proof authority](../../../OWNER_QUESTIONS.md#general-proof-pcc-authority)
owns that decision, depending on the [proof foundations](../proofs/contracts.md#undetermined-foundations).
The current product checker implements bounded rules, while the Gamma bootstrap
checker implements finite ground equality. Neither establishes the general
kernel or the universal schema-soundness proofs required below.

Existing obligation reconstruction and bounded certificate work can continue.
General theorem evidence must not acquire artifact authority merely because it
uses an existing certificate envelope or was accepted by source automation.

## Responsibilities

| Component | Responsibility | Cannot choose |
| --- | --- | --- |
| Producer | Emit canonical Terminal Psi and candidate evidence. | Which obligations the artifact must satisfy. |
| Artifact verifier | Decode and validate the module, then reconstruct its complete ordered obligations and premises. | A weaker question based on the evidence supplied. |
| Proof kernel | Check derivations against those exact questions and their permitted premises. | Additional assumptions or unrecorded admissions. |

The verifier reconstructs obligations from every operation and edge and retains
the fingerprinted author contracts before matching evidence. Missing obligations,
extra evidence, changed conclusions, wrong module or obligation identities, and
unauthorized admissions reject. A proof bundle is not an obligation manifest.

Every obligation binds its exact semantic subject. Supporting mathematical or
theory facts join a whole-artifact refinement claim only through checked bridges.
Subject, model or theory identity, semantics version, target capsule, observation
profile, bridge dependencies, and admissions participate in identity.

All-model consequence, truth in an exact constructed mathematical model, and
operational refinement of pinned transition systems are distinct subjects.
Using a theory theorem in one model requires checked model-satisfaction evidence.
Induction over generated values requires their intended construction, not merely
an arbitrary model satisfying some axioms. Supporting theories or alternative
proof calculi cannot replace the canonical operational question.

The verifier derives the observation profile from canonical semantics,
boundary/component contracts, and the consumer-selected typed schema. Exact
profile equality is the first replay gate. Cross-profile reuse requires a checked
canonical forgetting projection; profiles may be incomparable. The producer cannot
make an obligation vacuous by supplying an empty observer.

## Canonical semantic ledger

The authoritative generator is a total low-rung definition over canonical
Terminal bytes, not an AST decoded by a separately trusted producer. Its output
contains:

1. Validated types, identities, SSA, control flow, calls, contracts, and closed tags.
2. Each operation's direct mathematical denotation.
3. A canonical goal for each proof-bearing operation, edge, return, conservation
   event, contract, and admission site.
4. Premise introductions with origin, prerequisites, establishment point,
   value/place versions, validity scope, and invalidating events.
5. An acyclic logical-justification order covering each artifact node exactly once.

A direct execution of that definition, or a kernel-checked derivation of the same
total definition, establishes the ledger. Agreement with an optimized
implementation does not grant that implementation authority; disagreement rejects.

### Operation schemas

Each leaf operation has one exact row in a closed, typed, declarative schema:

| Row component | Required content |
| --- | --- |
| Formation | Operand and result well-formedness. |
| Meaning | Direct denotation and canonical goals. |
| Establishment | Facts available after discharge. |
| Behavior | Crash behavior and local logical-work/frontier effects. |

Missing or duplicate rows reject. Opaque callbacks are not schema definitions.
A new leaf expressible in the existing algebra adds a row. New control, effect,
validity, or frontier machinery requires an explicit revision of the ledger
algebra.

A call is a composition, not a primitive leaf denotation. Its separate row covers
target, result, positional arguments, requirements, structural transfer, successful
outcome, crash routes, evidence lifetime, logical work, and frontier policy.
Concrete module validation precedes obligation enumeration and importing callee
guarantees.

### Normalization boundary

The generator may evaluate closed mathematical expressions, decide bare
source-carrier inclusion, and omit schema-local vacuous bounds. It may retain
authored compound contracts, an operation's direct denotation, and checked
capture-free positional call substitution.

It does not compute symbolic interval reductions, affine summaries, or other
multi-operation conclusions. Three SSA definitions remain three local equations.
Untrusted automation may derive a summary from them, but must prove the original
canonical goal. Search strategy, discovered facts, or producer limits cannot
change obligation identity.

## Premise availability

A premise must dominate its use and remain valid on every path to it. Acyclic
logical ordering alone does not establish availability.

- An acyclic join may introduce a merge token only from matching valid tokens on
  every predecessor. A fact from one arm does not survive the join.
- Cyclic reconvergence requires checked invariant establishment and preservation,
  not the acyclic merge rule.
- Scalar range invariant arrival questions come from the semantic roster and
  every actual incoming edge, independently of the supplied proof bundle. Each
  question is the conjunction of the inclusive lower and upper bounds on the
  actual successor argument. Establishment and preservation must both check
  before the header's bounds may justify downstream safety obligations.
- Each successor applies its own argument-to-parameter substitution.
- A partial operation establishes its result equation only on its normal
  successor, after its safety obligation is discharged. That equation cannot
  prove the operation that creates it.

Call coverage reconstructs one exact obligation for every callee `requires`
clause. Instantiation independently checks arity, binder kinds/types,
capture-free positional substitution, pre/post versions, moves and reborrows,
outcome guards, crash routes, and evidence lifetime. Missing a clause and
substituting the wrong caller term are distinct failures.

### Entry facts and crash coverage

Entry hypotheses bind exact invocation formals or structural entry paths, with
their retained types and access. Current body values are not entry snapshots.
Named-state requirements do not become ambient invocation-entry assumptions.
A block parameter is a forwarded formal only when every incoming edge proves
the same typed origin; unknown, conflicting, or unestablished cyclic origins
supply no such equality.

Direct crash-site checking proves every asserted guard from independently
established entry and pre-terminator facts on every incoming path. A checked
contradiction may close an impossible path. Neither the site guard nor supplied
producer evidence becomes an assumption. All-crash bodies still retain their
entry contracts.

Entry-only call-ceiling coverage proves the same-cause union of published
alternatives without claiming an individual disjunct is true. Disjunction
elimination checks each branch under only its own alternative and discharges
that assumption. Semantic operation facts do not occupy requirement slots.

Boolean denotation and polarity consequences derive from the exact selected
operation, not its spelling or a desired callee requirement. Caller operands
keep caller operator meaning under substitution. Equality transport and integer
strict/inclusive conversions require their checked rules; proof traversal does
not silently reorder a canonical goal. Comparison-only normalization for crash
coverage does not change the codec's canonical ordering.

## Acceptance and trust

Every accepted fact must be re-decided by a specified total kernel judgment,
discharged by a checked certificate (carried or produced by a total certifying
procedure), or admitted at a sealed site accepted by the consuming profile.
Admission does not replace an ordinary derivable obligation. Search that may
time out or return unknown must carry a certificate for portable checking.

| Theorem family | Premises | Claim |
| --- | --- | --- |
| Safety / partial correctness | Exhaustive reconstruction, sound rows, valid premises, checked obligations. | No execution prefix violates the selected safety policy; completed outcomes satisfy their contracts. |
| Progress / total correctness | Well-founded orders, per-edge descent, complete SCC/call closure, accepted environmental progress premises. | Published termination guarantees hold. |

[Logical work](../resources/logical_work.md) proves neither theorem. Exhausting a
compiler-service budget is incomplete evaluation, not native behavior or a
termination argument.

Schema soundness theorems are universally quantified metatheory proved once in
the low-rung calculus, not repeated in each artifact. They bind exact schema,
state-model, mathematical-definition, operational-clause, and composition-theorem
digests. A checked conservative-extension theorem may transport an unaffected
row to a new semantics version; otherwise it must be reproved. This does not
introduce codec compatibility beyond the [encoding contract](encoding.md).

The trust ledger is a closed dependency graph. Every dependency terminates at a
registered root carrying kind, semantic subject, digest/version, owner, scope,
rationale, and accepting policy. Unknown, cyclic, unreachable, duplicate, or
noncanonical dependencies reject. Unproved generators, reductions, ledger
frameworks, leaf rows, and call-composition rows remain distinct dependencies.
Proving one row must not hide an unproved composition theorem.

Terminal denotation ends at its abstract execution model. Native lowering,
installation, ISA semantics, and hardware fidelity belong to the separate
native-refinement closure. The formal-target-to-silicon claim is deployment
evidence, not a reusable Terminal fact.

Reports distinguish checked facts, artifact-scoped admissions, and deployment
admissions; their union is not an unqualified `verified` result. Semantic module,
proof bundle, installation record, and debug/source maps retain separate
identities. A different proof does not rename the semantic module.
