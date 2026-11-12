# Stored references through returning calls

Run from the repository root:

```sh
omega run --both tests/omega/pass/calls/runtime_stored_reference_computation_argument_exit/main.omg
```

The program exits 0 after returning scalar helpers mutate the caller's original
array element and record field, leaving their siblings unchanged. A shared
stored reference observes its original scalar. An owned record moves its
reference field through a returning helper while disposing its separate spare
reference. A second helper selects the field into a local, mutates its sibling,
then returns the selected descriptor. Later helpers write through both returned
descriptors into the original scalars.

Both native and Terminal execution must produce that authored result. Borrowed
argument paths describe logical referents; they must not confuse a stored
descriptor's bytes with its backing storage. Returning an owned reference field
uses the ordinary checked projection transfer, not a new loan establishment.
