# Staged Delta compiler gate

Run `sh tests/delta/staged-compiler/run.sh` from the repository root. The gate
materializes the complete development and canonical compiler closures through
their role manifest, verifies their identities, and runs the selected Gamma
evaluator. The host frames inputs and compares bytes; it does not implement
Delta checking or lowering.

The retained cases compare exact receipts for nominal matches, recursion,
lists, `Bytes`, and forward/mutual nominal types. Further controls cover scope,
declaration lookup, checked arithmetic, immutable byte ropes, proper tail calls,
malformed programs, and canonical `ConformanceBytesV1` admission and execution.
The final declaration-scale control compiles 3,001 functions to the retained
78,271-byte receipt and executes its selected function. Each evaluator call
retains the existing 30-second watchdog.

## Emitter continuation regressions

The sibling `emitter_fixtures.py` owns eight fixed authored byte constructions:

- 248 nested ordinary calls, 250 nested `if` branches, and 250 distinct lets.
- One checked addition below 246 branches.
- A 248-field constructor and 128 nested nullary matches.
- A 64-binder payload match below 96 nullary matches.
- Twenty mixed `Int`/`Bytes`/nominal payload matches below 110 nullary matches.

The four scalar cases have independently authored exact Gamma receipts. The
nominal cases compare repeated compiler output rather than recreating nominal
lowering in the host. Every case must compile twice to identical bytes and its
generated Gamma must execute normally: byte `07`, except the mixed payload
case's byte `18`. These cases separate compiler traversal contexts from the
generated program's own nesting and runtime resources; a compiler success
followed by a generated-program resource failure does not pass.

This is bounded implementation evidence, not full emission-depth or resource
conformance and not closure of the Delta bootstrap edge. The separate
[frontend-boundary gate](../frontend-boundary/README.md) owns exact diagnostic
and expression-depth observations; the
[request-boundary gate](../request-boundary/README.md) owns DCREQ admission.
