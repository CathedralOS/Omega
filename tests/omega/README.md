# Omega language cases

Compiler cases are organized by outcome first, then by feature area.

- `pass/`: should compile, and runnable ones should behave as asserted
- `fail/`: should be rejected with the expected diagnostic fragment
- `run/`: ad hoc runnable probes and exploratory cases that are not part of the
  main pass/fail contract suite

Inside each bucket, prefer feature folders when a group becomes noisy:

- `arithmetic/`
- `borrows/`
- `calls/`
- `collections/`
- `constraints/`
- `control_flow/`
- `ownership/`
- `parameters/`
- `rewards/`
- `slices/`
- `storage/`
- `text/`
- `domains/`
- `dungeon/`
- `traits/`

These are not hard semantic silos. A case may mix features. The goal is
simply to keep the tree navigable and let the dominant pressure of the test
decide the folder.

Conventions:

- Keep each case self-contained in its own directory with `main.omg`.
- Add `platform/` shims only when the case needs host boundaries.
- Name cases by the behavior under test, not by the fix that motivated them.
- If a case graduates into a clearer feature family, move the directory and
  update the suite path rather than duplicating it.

Every fixture here is a test. `tools/corpus_gate.py` compiles each one through
the `corpus_runner` target and diffs its outcome record (checked or rejected,
diagnostics, expected-fragment match) against `corpus_outcomes.txt`; nothing
needs registering in a roster. That run checks fixtures only; `--native` builds
every pass and run fixture for the host, executes the run tier and `*_exit`
fixtures, and diffs against `corpus_native_<target>.txt`.
