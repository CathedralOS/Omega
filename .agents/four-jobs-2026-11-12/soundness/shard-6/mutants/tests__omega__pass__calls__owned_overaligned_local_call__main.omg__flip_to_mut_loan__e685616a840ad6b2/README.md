# Owned over-aligned local calls

Run after ordinary package preparation:

```text
omega run tests/omega/pass/calls/owned_overaligned_local_call/main.omg
```

Two independently owned 32/64-aligned locals cross an ordinary by-value call;
the helper returns a separate 64-aligned record. Exit 0 requires every selected
member and both unchanged input records. This exercises local, outgoing-copy
and returned storage without a Console service. It does not replace the genuine
foreign callback or matching-host x86-64 invocation witnesses.
