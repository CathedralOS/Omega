# Checker implementations

This directory contains executable realizations of the generic derivation
judgment. They are separated by implementation lineage so cross-checks do not
hide shared source:

- `beta/` is the seed-runnable authoritative low-rung implementation;
- `gamma/` is the independently written Gamma implementation and typed form;
- `reference/` is an untrusted executable reference used for differential
  diagnosis.

Proof construction, translation, corpora, and gate policy do not belong here.
