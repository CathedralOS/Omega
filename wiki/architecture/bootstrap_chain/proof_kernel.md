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

The complete checker, concrete inner calculus encoding, and full executable
profile are currently absent. The ordinary-Gamma
[outer request admission](../../../bootstrap/gamma/derivation_checker/REQUEST.md)
retains separate exact theory, proposition, and certificate spans; a framed
result is neither theory validation nor proof acceptance. The
[ground equality implementation design](derivation_calculus.md)
specifies the first complete encoding subject, conservative definitions, explicit
proof checks, ownership, and implementation dependencies. It does not supply an
accepted checker or certificate. `TASKS_BOOTSTRAP.md` owns implementation and
full-subject acceptance.
