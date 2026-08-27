# `tests/lattice/corpus/` — the lattice's frozen Omega sample corpus

The trust gates (`source/on-ramp/omega-bootstrap/gates/omega-meaning.sh`,
`source/proof-kernel/gates/forall-sample.sh`,
`source/refinement/delta-omega-bootstrap/input-tv.sh`,
`source/proof-kernel/gates/math-contracts.sh`, …) verify meaning/proof-carrying
claims about real Omega programs. Those gates read from **here**, not from the top-level
`samples/`, on purpose: the trust foundation must be pinned to a stable corpus so that
sample churn on the product line (reorganizing, adding stdin prompts, changing exit codes)
cannot silently break — or hang — the verification gates. This is a deliberate snapshot;
update it intentionally, not by drift.
