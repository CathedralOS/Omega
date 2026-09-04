# Direct Beta Delta evaluator experiment

This experiment tests whether Gamma earns its place below the current recursive
Delta profile. It starts from the exact minimized direct Gamma evaluator and
adds data-declaration census, arbitrary-arity constructor values, and match arm
selection/binding directly in Beta.

The prototype executes the retained recursive `Nat`, two-field `List`,
three-field rope, and 3,001-function sources without first transforming
them to Gamma:

```text
Delta source -> direct Beta evaluator -> result
```

## Measurement

```text
							  Lines  Instructions  Labels  Control  Tape bytes
selected Gamma evaluator      1,325         1,065     165      479       6,934
matched direct Delta evaluator 2,019         1,655     262      836      11,004
selected Delta transformer    1,457 Gamma source lines
```

The prototype matches the selected staged compiler's earlier D92 structural
profile. It validates known nominal field types, constructor arity, exact
payload binder counts, constructor-owner agreement, and exhaustive
declaration-order arms before execution. The exact malformed suite rejects
quietly. Constructor-building and match-arm selection preserve inherited tail
position; a 100,000-node List construction and traversal completes with constant
function activation and call-context storage.

The selected transformer has since added a complete declaration-order and
exact namespace-uniqueness census plus lexical-scope validation. The retained
direct prototype has not, so current total line counts are deliberately not
presented as matched evidence.

## Finding

Direct Delta execution is feasible, but it is not an obvious trust reduction.
At the D92 matched structural coverage, the prototype is 694 low-level lines
and 4,070 tape bytes larger than the selected Gamma evaluator. It removes the
then-852-line higher-level transformer at the cost of moving constructor and match
semantics into the low-level root.

Neither route yet implements Delta's complete nominal type relation, checked
arithmetic, normative `Bytes`, or application profiles. Those shared gaps do not
make this experiment an admitted replacement.
