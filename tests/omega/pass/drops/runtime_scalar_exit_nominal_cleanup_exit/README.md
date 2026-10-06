# Scalar return cleanup

From the repository root:

```sh
python3 tools/corpus_gate.py --both --filter drops/runtime_scalar_exit_nominal_cleanup_exit
```

The authored result is exit 0 and `XBACXBADXBACXBADXACXADAEAF` followed by a
newline. The corpus harness supplies explicit acceptance of this fixture's
reviewed custom Console provider; bare CLI execution does not supply that policy.
The provider's byte write promises normal termination so a cleanup hook may call
it. The fixture does not strengthen the standard Console contract.

`choose` evaluates the outgoing write (`X`), disposes locals in reverse order
(`BA`), then returns `C` or `D` for the caller to print. `choose_consuming` moves
the second local into the true arm's call: its callee performs `B`, and the
caller must perform only `A`. The false arm retains both locals.

`choose_selected` establishes a nominally cleaned marker between two owned
selection candidates. Its transported home is consumed on one return and
cleaned on the other; either path prints one `A`. `choose_payload` returns each
case's own payload after cleaning its local marker, producing `AEAF` across the
two calls. These compositions require exact selected-exit actions before the
scalar result join, not a union of dying roots after it.
