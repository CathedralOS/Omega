# Ground equality checker implementation design

[Checker contract](proof_kernel.md) | [Beta encoding](../../../bootstrap/beta/LANGUAGE.md)

This specifies an implementation direction for the missing ordinary-Gamma
derivation checker. It is not an accepted checker, certificate, or new admission
of the Gamma evaluator. The [inner format](../../../bootstrap/gamma/derivation_checker/FORMAT.md)
assigns concrete wire fields; semantic indexes, executable formation checks,
the soundness argument, and certificate measurements remain acceptance work.

## First complete subject

The first certificate must establish this independently reconstructed root:

```text
encode_Beta(S, source_limit, output_limit) = Success(T)
```

`S` is the entire selected Beta source of the Gamma evaluator, including comments
and whitespace; `T` is its entire persisted Alpha tape. The artifact owner fixes
the Beta definition package, both raw subjects, the Beta encoding profile, and
this root independently of the certificate producer. Current source/tape
identities live in the [Gamma evaluator profile](../../../bootstrap/gamma/EVALUATOR_PROFILE.md);
they are observations, not permanent theorem constants.

The encoder is a transparent definition, not a primitive invoking Beta. Its
error-or-success result makes the root prove admission as well as encoding;
there is no unchecked well-formed-source premise. Byte comparison, assembler
agreement, a digest, or a proof of one instruction cannot discharge this root.
This is D9's encoding obligation, not a proof that the Beta-written evaluator
implements Gamma. Equal tapes have identical deterministic Alpha behavior only
under the same Alpha input and resource profile.

## Definition formation is not subject authority

The generic checker establishes that constructor and function definitions form
a well-defined total first-order theory. Separately, the artifact owner fixes
and audits the particular definitions that formalize Beta. The certificate may
not substitute another well-formed theory or select a weaker proposition.

A sound checker can prove a claim about the wrong encoder. Formation success or
a certificate-provided theory identity does not make a package authoritative.
The eventual accepted closure must retain the exact owner, theory, checker,
subjects, profiles, derivation, and disclosed admissions.

## Terms and conservative definitions

Terms are sorted first-order trees. Every constructor and defined function has
one fixed ordered argument-sort list and one result sort, with disjoint symbol
namespaces. Ground terms have no variables; only definition templates do.
There are no binders, higher-order functions, arbitrary assumptions, or declared
equality axioms. Applications must have exactly their declared arity and sorts.

Constructor sorts denote freely generated finite trees. Signatures mention only
declared sorts. Formation requires a finite inhabitant for each sort, checked by
a monotone reachability pass over constructor signatures; recursion alone does
not establish inhabitation. The initial function discipline is:

- Function symbols are fresh; they cannot redefine constructors or prior functions.
- A function has either one nonrecursive clause over distinct argument variables,
  or selects one argument for constructor case analysis.
- A case definition has exactly one clause per constructor of the selected
  argument's sort. That pattern is one constructor over distinct fresh variables;
  every other argument pattern is a distinct variable. Patterns are linear across
  the whole clause. Nested patterns require helper definitions.
- Clause bodies have the function's result sort and use only bound pattern
  variables. Every application is well sorted.
- Calls to other defined functions point strictly backward in definition order.
  Mutual recursion is absent.
- Direct self-calls occur only in case definitions. The selected argument must
  be an unchanged immediate child variable of the selected constructor pattern,
  of the required sort. A computed value or reconstructed parent is not a
  structural-decrease witness.

Other call arguments may change and use already admitted helpers. Termination
follows lexicographically from definition order and selected finite-tree size:
a helper call decreases order, and a self-call strictly shrinks that tree.
Disjoint complete cases give a unique result. Fresh definitions extend the free
constructor model conservatively, rather than equating distinct constructor
values. The implementation must connect this soundness argument to its actual
formation checks; acyclic term storage alone does not prove definition termination.

Unknown sorts, duplicate symbols, missing or repeated cases, unbound variables,
wrong sorts, forward dependencies, and nondecreasing recursion must fail formation
before any derivation can accept.

## Explicit proof rows

Every row concludes equality of two closed terms of the same sort:

| Rule | Required check |
| --- | --- |
| Reflexivity | Both sides are the same structural term. |
| Symmetry | An earlier checked row has exactly the reversed sides. |
| Transitivity | Earlier rows prove `a = b` and `b = c`; their middle terms agree structurally and the claimed result is `a = c`. |
| Congruence | Both sides apply the same symbol; an ordered earlier equality relates each corresponding argument pair, with exact arity and sorts. |
| Definition unfolding | The left side applies an admitted definition; the stated clause matches and checked substitution yields exactly the claimed right side. |

Earlier equalities may be reused, forming an explicit DAG. References point
strictly backward. Every supplied row is checked; forward, cyclic, missing, or
malformed rows cannot be excused because a later conclusion is valid. The final
equality must match the independently reconstructed root, not just a
certificate-selected conclusion index.

Unfolding checks one stated step. It does not run the compiler, search for a
rewrite, normalize both sides, or fill missing derivations. Substitution cannot
bind a fresh variable or replace the definition. Case selection inspects the
selected argument's explicit constructor syntax; a defined application must
first be rewritten by supplied proof steps, not silently evaluated to pick a
clause. Structural comparison cannot
use pair addresses or hash equality as semantic equality. Sharing may reduce
storage without changing the represented term.

Each retained rule needs use in the complete encoding certificate and positive
and mutation controls; remove an unused convenience rule. Quantifiers, induction,
open lemma schemas, coinduction, and Alpha transition-system rules are outside
this first finite-instance implementation.

## Complete Beta theory

The error-valued encoder must cover the full
[Beta contract](../../../bootstrap/beta/LANGUAGE.md): textual ASCII, separators,
comments, EOF, complete-token lowercase hexadecimal words and registers, every
opcode/operand/width, eight-byte little-endian words, `dw`, address assertions,
malformed input, source/output provisions, arithmetic overflow, and exact
source/output exhaustion. It must not formalize only the instruction bytes that
happen to occur in the first customer.

Finite constructor bit strings can represent bytes, words, and limits, with
transparent arithmetic definitions; large limits must not require millions of
unary successors. The checker gains no trusted arithmetic evaluator, assembler
primitive, source-pattern substitution, or tape-hash shortcut.

A compatible construction is structural recursion over source bytes with an
explicit encoder state and previously defined token/encoding helpers. Recursion
consumes the immediate source tail; a helper result cannot stand in for an
unchecked claim that a parser cursor advanced. This is a theory-construction
strategy, not permission for the checker to execute it instead of checking steps.

Closed intermediate equalities may retain source suffixes, encoder states, and
output fragments. Their composition must establish separate source/tape
adjacency, zero initial offsets, agreed intermediate states, exact endpoints,
and complete exhaustion. Trivia and assertions own source bytes but emit none.
No gap, overlap, dropped assertion, or unexamined suffix is accepted. The owner
fixes the composition theorem and endpoints; cuts are witnesses. Current Beta
is one-pass with numeric addresses, so D14's historical two-pass label/fixup
scaffolding is not introduced.

## Ownership and execution order

The intended generic source layout is `bootstrap/gamma/derivation_checker/`:

```text
checker.gamma       coordinate formation, derivation checking, and root checking
representation/     sorted symbols, templates, terms, and equalities
admission/          framed input and bounded row/extent decoding
formation/          constructor sorts and conservative definitions
comparison/         structural equality and checked substitution
derivation/         closed rule dispatch and individual checks
boundary/           exact observations and failure publication
```

The entrance stays small and sequences real checks. Artifact-specific Beta
definitions, root reconstruction, and untrusted proof production are separate
from generic checking. Do not create empty folders or stub accepted artifacts
to represent this layout. No host semantic helper or second kernel is introduced.

Implement in dependency order:

1. Implement the concrete term, template, proof-row, and input encodings with
   owner-controlled theory/root inputs distinct from certificate witnesses.
   Specify the result encoding, each finite resource, and its preflight before
   executable acceptance.
2. Implement formation, comparison, substitution, explicit rules, and exact-root
   publication in ordinary Gamma. Register malformed and exact/adjacent controls
   for every boundary as it becomes executable.
3. Author and audit the complete transparent Beta theory and owner-fixed root
   reconstruction. Produce the whole selected-source certificate through an
   untrusted source-owned producer on the selected lower chain, not a host parser
   or semantic script.
4. Check that certificate with the selected Beta-authored Gamma evaluator, then
   run mutation/resource controls and retain all exact identities and admissions.

## Acceptance and measurements

The complete selected Gamma-evaluator encoding certificate is the first P1
acceptance subject. Small rule tests cannot substitute for it. Mutations cover
source, tape, assertions, theory identity, rules, clause selection, substitution,
premises, sorts, arities, partition joints, endpoints, and final-root selection.
Invalid or exhausted inputs cannot publish acceptance, even after a valid prefix.

Measure certificate bytes, retained proof state, comparison/substitution scratch,
maximum semantic stack, allocation demand, and checking time against exact inputs.
The checker's finite profile is separate from Gamma's profile; host timeouts and
raw evaluator failures are not proof verdicts. Grammar recursion, references,
and repeated expansion require explicit bounded implementations.

If finite proofs are too large, measure sharing and checked closed-lemma
composition first. Any additional rule needs a concrete certificate customer
and soundness argument. A trusted assembly rule, unchecked recursive equations,
weakened root, host semantic stage, or another D12 escalation cannot be adopted
as a local implementation fix.
