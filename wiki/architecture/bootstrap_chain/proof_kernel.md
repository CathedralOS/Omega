# Bootstrap derivation checker

[Chain overview](bootstrap_chain.md) | [Active work](../../../TASKS_BOOTSTRAP.md)

The selected chain has no Alpha-owned general proof kernel. The previous
imperative-Gamma implementation and its Alpha tape were retired with that
language.

The replacement is an ordinary Gamma program executed by the Beta-authored
Gamma evaluator. It is a tool beside compiler edges, not a language rung. Its
question is deliberately narrow:

```text
Does explicit derivation C prove independently reconstructed proposition P?
```

The checker may validate only the calculus needed by the selected
source-to-Alpha compiler-edge certificates. It does not search for proofs,
discover artifact obligations, parse compiler source to choose a proposition,
run compilers, or decide deployment policy.

## Trust boundary

Moving the checker into readable Gamma source does not prove the Gamma evaluator.
The evaluator remains a trusted Beta program. An artifact-specific
owner reconstructs the exact source/tape proposition; the checker validates the
supplied derivation under that proposition.

Every retained rule needs:

- one concrete compiler-edge theorem that uses it;
- exact syntax and semantics;
- bounded parser, value, and frame behavior;
- positive and mutation controls; and
- measured certificate-size consequences.

Unknown rules, malformed terms, missing premises, cyclic derivations,
wrong-subject evidence, and resource exhaustion cannot accept. Product proof
ambitions do not enlarge this bootstrap tool automatically.

An artifact-accepted checker and complete Beta encoding certificate are still
absent. The [concrete inner format](../../../bootstrap/gamma/derivation_checker/FORMAT.md)
assigns theory, template, ground-term, and proof-row fields without adding an
accepted artifact. The ordinary-Gamma
[outer request admission](../../../bootstrap/gamma/derivation_checker/REQUEST.md)
retains separate exact theory, proposition, and certificate spans; a framed
result is neither theory validation nor proof acceptance. A separate
[inner layout traversal](../../../bootstrap/gamma/derivation_checker/LAYOUT.md)
checks every physical record without admitting a theory or checking a proof.
[Theory formation](../../../bootstrap/gamma/derivation_checker/FORMATION.md)
then indexes and checks conservative definitions, including finite inhabitants,
scoped sorted templates, complete cases, and structural self-decrease. Its
`Formed` result is neither proof acceptance nor Beta subject authority.
[Ground-term validation](../../../bootstrap/gamma/derivation_checker/GROUND.md)
indexes owner and witness terms separately, checks their applications, and
requires well-sorted owner-only root references. `Grounded` does not compare
those roots for equality or check any proof row.
[Structural comparison](../../../bootstrap/gamma/derivation_checker/COMPARISON.md)
compares validated term syntax with completed-pair memoization and cumulative
session work. A structurally different result does not disprove theory equality;
[checked substitution](../../../bootstrap/gamma/derivation_checker/SUBSTITUTION.md)
validates a stated unfolding without evaluating functions or polluting structural
memoization. [Explicit derivation checking](../../../bootstrap/gamma/derivation_checker/CHECKING.md)
validates all rows and the final owner root under one cumulative resource profile.
Its generic Checked outcome proves equality under the supplied formed theory;
it does not authenticate that theory or root as the intended Beta subject. The
[ground equality implementation design](derivation_calculus.md)
specifies the first complete encoding subject, conservative definitions, explicit
proof checks, ownership, and implementation dependencies. It does not supply an
accepted checker or certificate. `TASKS_BOOTSTRAP.md` owns implementation and
full-subject acceptance.
