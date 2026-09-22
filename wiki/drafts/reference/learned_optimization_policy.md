# Exploring learned optimization policy

Purpose: retain possible far-future uses of the compiler's discrete optimization
choices. The current work is only to keep candidate construction, deterministic
selection, and independent validation separate. No model or training framework
is planned for the Rust reference compiler. These notes authorize no
implementation; any future investigation belongs to the Omega-written product
compiler and needs its own concrete justification. Delete these notes when a
measured workload justifies a concrete proposal or the product compiler takes
the question.

Affected subject: [optimization selection and validation](../../spec/build/optimizations.md).

## Motivation

Deterministic heuristics may choose poorly for a particular workload. The
question is whether learned ranking or bounded search can improve measured
output quality enough to justify training, evaluation, and build costs without
changing semantic admission or making the baseline depend on a model.

## Possible future investigation

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

Keep deterministic heuristics unless useful quality gains are measured. A future
bounded model-free search could test the workload and measurement protocol before
adding learned models; this avoids assuming that a uniform oracle must replace
every pass-local decision.

The remaining questions are which concrete workloads and cost objectives justify
the work, the cost of candidate checking at search scale, and the useful boundary
between high-level and target-specific features. The cost model's authority is
not open: it ranks choices and never replaces semantic validation. Arbitrary
model-authored rewrites are outside the accepted candidate interface.

## Identity of specialized variants

The publication contract already answers how a workload-specialized or
multiversioned body would interact with code identity, deduplication, and
component replacement:

- **Slot identity is unchanged.** Under
  [component publication](../../spec/build/component_publication.md), slot
  identity is the exact closed requirement application — never an artifact
  generation. Variant selection is a realization/binding input inside one
  slot, not a new slot: a specialized variant satisfies the same requirement
  application as the unspecialized body it replaces.
- **Body identity is distinct.** A variant is a distinct generated body: it
  carries its own content identity, its own semantic-subject identity, and
  evidence bound to its own bytes. Receipts do not transfer — a variant
  cannot inherit the unspecialized body's verification or admission record.
- **Deduplication is exact.** Deduplication applies to byte-identical bodies
  only. Merging two variants, or folding a variant into the unspecialized
  body, would substitute a different semantic subject and is prohibited;
  shared structure inside one variant is storage detail, not identity.
- **Replacement is ordinary.** A variant is admitted exactly like any other
  candidate: inside the slot's frozen replacement envelope (imports and
  authority, compatibility/observation profile, target semantics, resource
  ceilings, execution modalities, continuity). Specialization cannot widen
  the envelope, and workload fit grants no extra authority.
- **The decision is identity-bound.** Reproducibility requires the
  specialization choice to be part of the recorded composition inputs — a
  fixed, identity-bound decision/result — so rebuilding the same composition
  regenerates the same variant identity and substitution is detectable.
