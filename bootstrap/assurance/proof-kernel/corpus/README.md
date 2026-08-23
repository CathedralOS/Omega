# Proof-kernel corpus

This directory owns declarative proof sources, shared proof libraries, and
deterministic fuzz/oracle generators. Corpus members exercise checker behavior;
they are neither checker implementations nor policy entry points.

- `proofs/` contains the theorem library and negative-control source material.
- `lib/` contains proof fragments shared by several theorem sources.
- `fuzz/` contains deterministic case generators and executable test oracles
  consumed by gates.
