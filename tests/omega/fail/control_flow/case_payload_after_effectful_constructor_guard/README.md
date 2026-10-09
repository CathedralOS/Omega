# Effectful constructor child retires the selected case

The first guard conjunct observes `Reading`. The next conjunct evaluates a
case constructor whose `value` child calls `clear`, replacing the same sum
with `Empty`. Membership establishes `Observed` for the constructed verdict,
not `Reading` for the mutated sample. Reading `context.sample.value`
must reject with `cannot prove selected case for payload access`.

Constructor stability depends on its evaluated children, not just its literal
shape. This control challenges recursive guard-stability reasoning without a
provider, standard-library dependency or private semantic setup.

Run `python3 tools/corpus_gate.py --filter control_flow/case_payload_after_effectful_constructor_guard`.
