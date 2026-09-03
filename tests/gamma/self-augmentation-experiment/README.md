# Gamma self-augmentation experiment

This selected test proves the staged-bootstrap mechanism without the downgraded
concatenative language:

```text
Beta-written Gamma evaluator
  -> 85-line augmenter authored in Gamma
  -> richer Gamma source containing `const`
  -> ordinary Gamma source
  -> same Gamma evaluator
  -> result 42
```

The augmenter recognizes:

```text
const NAME INTEGER
```

and emits the ordinary Gamma declaration:

```text
(def NAME () Int INTEGER)
```

All ordinary lines pass through unchanged. The evaluator runs the augmenter with
`program.gamma1` as sealed input, checks the exact `program.gamma` receipt, then
evaluates that receipt to byte 42.

`const` is intentionally trivial. It proves that source functionality can move
upward into auditable Gamma without modifying Beta or the Gamma evaluator. The
next meaningful staged Delta experiment must add algebraic data and exhaustive
matching; that result decides whether scalar/effect Gamma is high-level enough
to escape low-level compiler work.
