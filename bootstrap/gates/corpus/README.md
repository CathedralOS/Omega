# `bootstrap/gates/corpus/` — the lattice's frozen Omega sample corpus

The trust gates (`bootstrap/omega-bootstrap/gates/omega-meaning.sh`,
`source/assurance/proof-kernel/gates/forall-sample.sh`,
`source/assurance/refinement/omega-bootstrap/input-tv.sh`,
`source/assurance/proof-kernel/gates/math-contracts.sh`, …) verify meaning/proof-carrying
claims about real Omega programs. Those gates read from **here**, not from the top-level
`samples/`, on purpose: the trust foundation must be pinned to a stable corpus so that
sample churn on the product line (reorganizing, adding stdin prompts, changing exit codes)
cannot silently break — or hang — the verification gates. This is a deliberate snapshot;
update it intentionally, not by drift.
