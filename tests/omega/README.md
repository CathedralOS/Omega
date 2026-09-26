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
- Cover each spec rule once. A pass case pins one feature or feature
  interaction the spec defines; a fail case pins one rejection rule. Before
  adding a case, look for one that already exercises the rule and extend it
  instead of adding a renamed variant, another integer width, a reordered
  argument list, or the same body with an extra statement. A regression for an
  incidental source arrangement belongs in the rule's existing case, not in a
  new directory.

Every fixture here is a test. `tools/corpus_gate.py` compiles each one through
the `corpus_runner` target and diffs its outcome record (checked or rejected,
diagnostics, expected-fragment match) against `corpus_outcomes.txt`; nothing
needs registering in a roster. That run checks fixtures only; `--native` builds
every pass and run fixture for the host, executes the run tier and `*_exit`
fixtures, and diffs against `corpus_native_<target>.txt`. `--interpret` also
runs the run tier and `*_exit` fixtures on the checked interpreter, and
`--both` runs both legs and records whether their exit codes agree; the
[testing spec](../../wiki/spec/build/testing.md#the-language-corpus) makes that
agreement a requirement.
