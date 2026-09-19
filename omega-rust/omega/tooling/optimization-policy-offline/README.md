# Offline optimization policy

This tool trains and checks a deterministic reference policy against recorded
decisions. It does not run the compiler, an external model, or another process.
The public [optimization contract](../../../../wiki/spec/build/optimizations.md)
separates policy ranking from semantic and publication authority.

Start at [cost_threshold_policy.rs](src/cost_threshold_policy.rs) for the library:
it fits and replays the trained model, then validates, evaluates, and replays a
held-out report. [training.rs](src/cost_threshold_policy/training.rs) and
[evaluation.rs](src/cost_threshold_policy/evaluation.rs) own the algorithms;
their `replay.rs` children independently check the results. The
[CLI entrypoint](src/bin/optimization-policy-offline/main.rs) owns command dispatch,
input loading, and output publication through its command handlers.

## Commands

From the repository root on Windows or macOS, use
`mbx run -p optimization-policy-offline -- <command>` (or `cargo run` if `mbx`
is unavailable). The command vocabulary is:

```text
capture <output-corpus> <decision-log>...
train <input-corpus> <output-model>
evaluate <input-corpus> <input-model> <output-report>
regression <input-corpus> <input-model> <output-report>
create-regression-manifest <input-corpus> <input-model> <output-manifest>
check-regression-manifest <input-corpus> <input-model> <input-manifest>
```

Every route strictly admits complete inputs and publishes only canonical
validated bytes. Output commands create new files, refuse existing paths, and
remove an output they created if writing fails. Evaluation and regression have
fixed, distinct splits. Manifest creation is separate from read-only checking;
a check cannot update its own expected baseline.

## Data and evidence

[Corpus admission](src/corpus/mod.rs) consumes canonical V2 decision logs. The
upstream schema binds the source, selection, target, catalog, cost model, and
one row per validated candidate. Rows expose candidate identity, predicted
structural cost delta, scheduled analyses, and exact sorted proof/ownership/fact
identities. The action is a listed choice or supported skip. Corpus identity
separates the complete action-independent legal decision surface from the
identity-bound recorded-action label. Whole source identities are partitioned
deterministically into training, evaluation, and regression; downstream tools
must not create a second feature schema or split individual records across them.

[CostThresholdV1](src/cost_threshold_policy.rs) fits one signed threshold against
recorded-action agreement. Inference chooses the canonical minimum legal
candidate below that threshold or the supported model-free skip. Independent
replay checks training and evaluation, binding algorithm, split, model, and
report identities through strict codecs. Confusion counts, exact chosen-candidate
agreement, and the checked-i128 sum of selected predicted costs measure label
agreement, not runtime, size, compile-time, or semantic quality.

The [regression manifest](src/cost_threshold_policy/regression_manifest.rs)
binds corpus, model, algorithm, regression split, expected report, and complete
exact summary. Validation recomputes the report before comparing any expected
field. Neither its receipt nor a successful corpus/model check grants optimizer
replay, compiler activation, process execution, or publication authority.

The adjacent [bounded-process tool](../bounded-process/README.md)
provides containment and resource controls, not filesystem, executable,
credential, or network isolation.
