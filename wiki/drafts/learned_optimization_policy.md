# Exploring learned optimization policy

Purpose: explore whether learned ranking or bounded search would improve compiler
output enough to justify its costs. No concrete integration is proposed, and
these notes authorize no implementation. Keep them while the comparison is
useful; delete them when superseded by a concrete design or no longer relevant.

Affected subject: [optimization selection and validation](../spec/build/optimizations.md).

## Motivation

Deterministic heuristics may choose poorly for a particular workload. The
question is whether learned ranking or bounded search can improve measured
output quality enough to justify training, evaluation, and build costs without
changing semantic admission or making the baseline depend on a model.

## Possible investigation

One approach would use offline or explicitly selected build-time search over the
existing validated candidate interface. Evaluate decisions against a versioned workload
corpus, with source-separated training, evaluation, and regression sets. Emit a
fixed, identity-bound decision/result, not a runtime oracle. Compare measured
execution, size, and compilation costs against the model-free baseline rather
than treating agreement with recorded heuristic labels as optimization quality.

A learned cost model over typed operation/state graphs could rank these
candidates before expensive measurement. Whether such graph features improve
prediction is an empirical question, not a benefit established by preserving
semantics. Richer policy interfaces, graph-model inputs, and workload-specific
specialization or multiversioning are extensions to investigate, not existing
compiler contracts. They must retain exact candidate admission, reproducibility,
resource bounds, and the separate component-publication contract.

## Alternatives and research questions

Keep deterministic heuristics and the current offline reference model if useful
quality gains are not measured. Bounded model-free search can test the workload
and measurement protocol before adding learned models; this avoids assuming
that a uniform oracle must replace every pass-local decision.

The remaining questions are which concrete workloads and cost objectives justify
the work, the cost of candidate checking at search scale, the useful boundary
between high-level and target-specific features, and how specialized variants
would interact with code identity, deduplication, and component replacement.
The cost model's authority is not open: it ranks choices and never replaces
semantic validation. Arbitrary model-authored rewrites are outside the accepted
candidate interface.
